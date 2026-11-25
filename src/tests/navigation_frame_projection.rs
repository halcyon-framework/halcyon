use crate::compositor::{Compositor, FrameProjectionError, ProjectedSurfaceView, SceneKey};
use crate::core::event::{InteractionDomainId, InteractionDomains, WindowId};
use crate::core::focus_authority::{
    FocusDomainAddition, FocusDomainId, FocusResolver, FocusTargetRegistration,
};
use crate::core::frame_projection_binding_registry::{
    FrameProjectionAuthorityAccess, FrameProjectionBinding, FrameProjectionBindingError,
    FrameProjectionBindingRegistry, FrameProjectionPullError, FrameProjectionPullOutcome,
    FrameProjectionRuntimeAccess, FrameProjectionWindowAccess, NavigationProjectionSource,
    ScreenHostProjectionKey, pull_navigation_frame_projection,
    pull_navigation_frame_projection_with_bootstrap,
};
use crate::core::math::RectF;
use crate::core::navigation_application::NavigationApplicationCoordinator;
use crate::core::navigation_domain_router::NavigationRootFocusBindingPortId;
use crate::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationRootId, NavigationRootRegistry,
    NavigationSurfaceOrchestrator,
};
use crate::core::overlay_domain_coordinator::{
    OverlayDomainCoordinator, OverlayFrameDescriptor, OverlayFrameId, OverlayOwnerId,
    OverlayOwnerLifecyclePort, OverlayStackDescriptor, OverlayStackId,
};
use crate::core::widget_tree::WidgetTree;
use crate::model::navigation_host::{
    NavigationDecision, NavigationHost, NavigationIntent, NavigationNodeId,
};
use crate::utils::id::TreeWidgetId;
use crate::widgets::label::Label;
use crate::widgets::screen_host::ScreenHost;

fn viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

fn screen_host(tree: &WidgetTree, key: ScreenHostProjectionKey) -> &ScreenHost {
    tree.node(key.node_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ScreenHost>()
        .unwrap()
}

fn screen_host_mut(tree: &mut WidgetTree, key: ScreenHostProjectionKey) -> &mut ScreenHost {
    tree.node_mut(key.node_id)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScreenHost>()
        .unwrap()
}

struct StandaloneHarness {
    interaction_domains: InteractionDomains,
    overlay_domains: OverlayDomainCoordinator,
    focus_resolver: FocusResolver,
    projection_bindings: FrameProjectionBindingRegistry,
    compositor: Compositor,
    orchestrator: NavigationSurfaceOrchestrator,
    tree: WidgetTree,
    key: ScreenHostProjectionKey,
}

impl StandaloneHarness {
    fn new(include_source: bool) -> Self {
        let interaction_domains = InteractionDomains::new();
        let overlay_domains = OverlayDomainCoordinator::new().unwrap();
        overlay_domains
            .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
            .unwrap();
        let focus_resolver = FocusResolver::new();
        let mut projection_bindings = FrameProjectionBindingRegistry::new(&overlay_domains);
        let mut compositor = Compositor::new(viewport());
        let orchestrator = NavigationSurfaceOrchestrator::new(
            NavigationHost::new(NavigationNodeId(1)),
            viewport(),
            &mut compositor,
            Box::new(IncrementalSceneKeyAllocator::new()),
        );
        let mut tree = WidgetTree::new();
        let node = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let key = ScreenHostProjectionKey::new(WindowId::PRIMARY, node);
        let sources = if include_source {
            vec![NavigationProjectionSource::standalone(
                InteractionDomainId::PRIMARY,
                orchestrator.projection_handle(),
            )]
        } else {
            Vec::new()
        };
        let guard = overlay_domains
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .unwrap();
        projection_bindings
            .install_or_replace(
                FrameProjectionAuthorityAccess::new(
                    &interaction_domains,
                    &overlay_domains,
                    &focus_resolver,
                ),
                FrameProjectionWindowAccess::new(
                    WindowId::PRIMARY,
                    InteractionDomainId::PRIMARY,
                    &compositor,
                    &mut tree,
                ),
                key,
                FrameProjectionBinding::new(InteractionDomainId::PRIMARY, sources),
                &guard,
            )
            .unwrap();
        drop(guard);
        Self {
            interaction_domains,
            overlay_domains,
            focus_resolver,
            projection_bindings,
            compositor,
            orchestrator,
            tree,
            key,
        }
    }

    fn pull(&mut self) -> Result<FrameProjectionPullOutcome, FrameProjectionPullError> {
        pull_navigation_frame_projection(
            FrameProjectionRuntimeAccess::new(
                &mut self.projection_bindings,
                FrameProjectionAuthorityAccess::new(
                    &self.interaction_domains,
                    &self.overlay_domains,
                    &self.focus_resolver,
                ),
                &self.compositor,
                FrameProjectionWindowAccess::new(
                    WindowId::PRIMARY,
                    InteractionDomainId::PRIMARY,
                    &self.compositor,
                    &mut self.tree,
                ),
            ),
            self.key,
        )
    }
}

#[test]
fn frame_projection_publishes_zero_and_one_context_without_inference() {
    let mut zero = StandaloneHarness::new(false);
    assert!(matches!(
        zero.pull(),
        Ok(FrameProjectionPullOutcome::Published { .. })
    ));
    let view = screen_host(&zero.tree, zero.key).projected_view().unwrap();
    assert!(view.navigation_contexts.is_empty());
    assert!(view.overlay_surfaces.is_empty());
    assert_eq!(view.focused_surface, None);

    let mut one = StandaloneHarness::new(true);
    assert!(matches!(
        one.pull(),
        Ok(FrameProjectionPullOutcome::Published { .. })
    ));
    let expected = one.orchestrator.project().unwrap();
    let view = screen_host(&one.tree, one.key).projected_view().unwrap();
    assert_eq!(view.navigation_contexts.len(), 1);
    assert_eq!(
        view.navigation_contexts[0]
            .navigation_surfaces
            .iter()
            .map(|surface| surface.id)
            .collect::<Vec<_>>(),
        expected.navigation_surfaces
    );
    assert_eq!(
        view.navigation_contexts[0].active_surface,
        expected.active_surface
    );
}

#[test]
fn busy_guard_and_stale_bootstrap_do_not_clear_current_view() {
    let mut harness = StandaloneHarness::new(true);
    harness.pull().unwrap();
    let guard = harness
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert!(matches!(
        harness.pull(),
        Err(FrameProjectionPullError::Overlay(
            crate::core::overlay_domain_coordinator::OverlayDomainError::Busy(
                InteractionDomainId::PRIMARY
            )
        ))
    ));
    assert!(
        screen_host(&harness.tree, harness.key)
            .projected_view()
            .is_some()
    );
    drop(guard);

    let stale_bootstrap = harness.projection_bindings.guard_identity(harness.key);
    let guard = harness
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    harness
        .projection_bindings
        .install_or_replace(
            FrameProjectionAuthorityAccess::new(
                &harness.interaction_domains,
                &harness.overlay_domains,
                &harness.focus_resolver,
            ),
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &harness.compositor,
                &mut harness.tree,
            ),
            harness.key,
            FrameProjectionBinding::new(
                InteractionDomainId::PRIMARY,
                vec![NavigationProjectionSource::standalone(
                    InteractionDomainId::PRIMARY,
                    harness.orchestrator.projection_handle(),
                )],
            ),
            &guard,
        )
        .unwrap();
    drop(guard);
    screen_host_mut(&mut harness.tree, harness.key).set_projected_view(ProjectedSurfaceView {
        navigation_contexts: Vec::new(),
        overlay_surfaces: Vec::new(),
        focused_surface: None,
    });
    let result = pull_navigation_frame_projection_with_bootstrap(
        FrameProjectionRuntimeAccess::new(
            &mut harness.projection_bindings,
            FrameProjectionAuthorityAccess::new(
                &harness.interaction_domains,
                &harness.overlay_domains,
                &harness.focus_resolver,
            ),
            &harness.compositor,
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &harness.compositor,
                &mut harness.tree,
            ),
        ),
        harness.key,
        stale_bootstrap,
    );
    assert_eq!(result, Err(FrameProjectionPullError::RetryNextFrame));
    assert!(
        screen_host(&harness.tree, harness.key)
            .projected_view()
            .is_some()
    );
}

#[test]
fn unresolved_surface_clears_without_partial_publication() {
    let mut harness = StandaloneHarness::new(true);
    harness.pull().unwrap();
    let root_surface = harness.orchestrator.project().unwrap().navigation_surfaces[0];
    assert!(harness.compositor.destroy_surface(root_surface));
    assert!(matches!(
        harness.pull(),
        Err(FrameProjectionPullError::Compositor(
            FrameProjectionError::MissingNavigationSurface { .. }
        ))
    ));
    assert!(
        screen_host(&harness.tree, harness.key)
            .projected_view()
            .is_none()
    );
}

#[test]
fn missing_and_wrong_type_screen_hosts_are_invalidated_under_guard() {
    let mut missing = StandaloneHarness::new(true);
    missing.tree.remove_node(missing.key.node_id);
    assert_eq!(
        missing.pull(),
        Err(FrameProjectionPullError::Binding(
            FrameProjectionBindingError::MissingScreenHostInvalidated(missing.key)
        ))
    );
    assert_eq!(
        missing.projection_bindings.guard_identity(missing.key),
        None
    );

    let mut wrong = StandaloneHarness::new(true);
    wrong
        .tree
        .replace_widget_only(wrong.key.node_id, Box::new(Label::new("wrong")));
    assert_eq!(
        wrong.pull(),
        Err(FrameProjectionPullError::Binding(
            FrameProjectionBindingError::MissingScreenHostInvalidated(wrong.key)
        ))
    );
    assert_eq!(wrong.projection_bindings.guard_identity(wrong.key), None);
}

#[test]
fn registered_multiroot_pull_preserves_binding_order_overlay_and_focus_authority() {
    const ROOT_ONE: NavigationRootId = NavigationRootId(1);
    const ROOT_TWO: NavigationRootId = NavigationRootId(2);
    const OWNER_ONE: OverlayOwnerId = OverlayOwnerId(11);
    const OWNER_TWO: OverlayOwnerId = OverlayOwnerId(12);
    const PORT_ONE: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(21);
    const PORT_TWO: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(22);
    const BASE_ONE: FocusDomainId = FocusDomainId(31);
    const OVERLAY_ONE: FocusDomainId = FocusDomainId(32);
    const BASE_TWO: FocusDomainId = FocusDomainId(33);
    const STACK: OverlayStackId = OverlayStackId(41);

    let mut interaction_domains = InteractionDomains::new();
    let overlay_domains = OverlayDomainCoordinator::new().unwrap();
    overlay_domains
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let application = NavigationApplicationCoordinator::new(overlay_domains.clone());
    application
        .navigation_domains()
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let mut roots = NavigationRootRegistry::new();
    let mut compositor = Compositor::new(viewport());
    let owner_one =
        OverlayOwnerLifecyclePort::new(OWNER_ONE, InteractionDomainId::PRIMARY, vec![]).unwrap();
    application
        .prepare_root_admission(
            &mut interaction_domains,
            &mut roots,
            &mut compositor,
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            NavigationHost::new(NavigationNodeId(1)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            owner_one.clone(),
            PORT_ONE,
            &[
                FocusDomainAddition::new(BASE_ONE, None),
                FocusDomainAddition::new(OVERLAY_ONE, Some(BASE_ONE)),
            ],
            BASE_ONE,
        )
        .unwrap()
        .commit();
    application
        .prepare_root_admission(
            &mut interaction_domains,
            &mut roots,
            &mut compositor,
            InteractionDomainId::PRIMARY,
            ROOT_TWO,
            NavigationHost::new(NavigationNodeId(2)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            OverlayOwnerLifecyclePort::new(OWNER_TWO, InteractionDomainId::PRIMARY, vec![])
                .unwrap(),
            PORT_TWO,
            &[FocusDomainAddition::new(BASE_TWO, None)],
            BASE_TWO,
        )
        .unwrap()
        .commit();

    let root_one_surface = roots
        .root(ROOT_ONE)
        .unwrap()
        .project()
        .unwrap()
        .active_surface
        .unwrap();
    let root_two_surface = roots
        .root(ROOT_TWO)
        .unwrap()
        .project()
        .unwrap()
        .active_surface
        .unwrap();
    let overlay_surface = compositor
        .create_overlay_surface(root_one_surface, 100, viewport(), SceneKey::new(100))
        .unwrap();
    owner_one
        .register_inactive_stack(OverlayStackDescriptor::new(
            STACK,
            vec![OverlayFrameDescriptor::new(
                OverlayFrameId(51),
                OVERLAY_ONE,
                overlay_surface,
            )],
        ))
        .unwrap();
    let focus_resolver = FocusResolver::new();
    let guard = overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let prepared_targets = focus_resolver
        .prepare_focus_target_mutation(
            guard.focus_capability(),
            &mut interaction_domains
                .state_mut(InteractionDomainId::PRIMARY)
                .unwrap()
                .focus_manager,
            &[
                FocusTargetRegistration::new(
                    BASE_ONE,
                    TreeWidgetId(61),
                    Some(root_one_surface),
                    true,
                ),
                FocusTargetRegistration::new(
                    OVERLAY_ONE,
                    TreeWidgetId(62),
                    Some(overlay_surface),
                    true,
                ),
            ],
            &[],
        )
        .unwrap();
    focus_resolver.commit_focus_target_mutation(guard.focus_capability(), prepared_targets);
    let eligibility = application
        .navigation_domains()
        .overlay_activation_eligibility_guarded(&guard, OWNER_ONE)
        .unwrap();
    let prepared_overlay = overlay_domains
        .prepare_activation(
            &guard,
            &mut interaction_domains,
            OWNER_ONE,
            STACK,
            Some(eligibility),
        )
        .unwrap();
    overlay_domains.commit_activation(&guard, prepared_overlay);
    drop(guard);

    let mut tree = WidgetTree::new();
    let node = tree.create_root(Box::new(ScreenHost::new()), None, 0);
    let key = ScreenHostProjectionKey::new(WindowId::PRIMARY, node);
    let mut projection_bindings = FrameProjectionBindingRegistry::new(&overlay_domains);
    let guard = overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    projection_bindings
        .install_or_replace(
            FrameProjectionAuthorityAccess::new(
                &interaction_domains,
                &overlay_domains,
                &focus_resolver,
            )
            .with_registered_roots(&roots, application.navigation_domains()),
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &compositor,
                &mut tree,
            ),
            key,
            FrameProjectionBinding::new(
                InteractionDomainId::PRIMARY,
                vec![
                    NavigationProjectionSource::registered_root(ROOT_TWO),
                    NavigationProjectionSource::registered_root(ROOT_ONE),
                ],
            ),
            &guard,
        )
        .unwrap();
    drop(guard);

    let outcome = pull_navigation_frame_projection(
        FrameProjectionRuntimeAccess::new(
            &mut projection_bindings,
            FrameProjectionAuthorityAccess::new(
                &interaction_domains,
                &overlay_domains,
                &focus_resolver,
            )
            .with_registered_roots(&roots, application.navigation_domains()),
            &compositor,
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &compositor,
                &mut tree,
            ),
        ),
        key,
    )
    .unwrap();
    assert!(matches!(
        outcome,
        FrameProjectionPullOutcome::Published { .. }
    ));
    let view = screen_host(&tree, key).projected_view().unwrap();
    assert_eq!(view.navigation_contexts.len(), 2);
    assert_eq!(
        view.navigation_contexts[0].active_surface,
        Some(root_two_surface)
    );
    assert_eq!(
        view.navigation_contexts[1].active_surface,
        Some(root_one_surface)
    );
    assert_eq!(view.overlay_surfaces, vec![overlay_surface]);
    assert_eq!(view.focused_surface, Some(overlay_surface));

    let direct = roots
        .dispatch(
            ROOT_ONE,
            NavigationIntent::NavigateTo(NavigationNodeId(3).into()),
            &mut compositor,
        )
        .unwrap();
    assert_eq!(direct.decision, NavigationDecision::Retry);
    assert!(matches!(
        pull_navigation_frame_projection(
            FrameProjectionRuntimeAccess::new(
                &mut projection_bindings,
                FrameProjectionAuthorityAccess::new(
                    &interaction_domains,
                    &overlay_domains,
                    &focus_resolver,
                )
                .with_registered_roots(&roots, application.navigation_domains()),
                &compositor,
                FrameProjectionWindowAccess::new(
                    WindowId::PRIMARY,
                    InteractionDomainId::PRIMARY,
                    &compositor,
                    &mut tree,
                ),
            ),
            key,
        ),
        Ok(FrameProjectionPullOutcome::Published { .. })
    ));
    let view = screen_host(&tree, key).projected_view().unwrap();
    assert_eq!(view.navigation_contexts.len(), 2);
    assert_eq!(
        view.navigation_contexts[1].active_surface,
        Some(root_one_surface)
    );
}
