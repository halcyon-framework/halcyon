use super::*;
use crate::compositor::Compositor;
use crate::core::event::InteractionDomainId;
use crate::core::navigation_application::{
    NavigationApplicationCoordinator, StandaloneNavigationSnapshot,
    StandaloneNavigationSnapshotResult,
};
use crate::model::navigation_host::{
    GuardDecision, NavigationAuthorizationResult, NavigationExecutionError, NavigationGuard,
    NavigationGuardAuthorization, NavigationIntent, NavigationState,
};
use crate::widgets::screen_host::ScreenHost;
use std::any::Any;
use std::cell::Cell;

const HOME: NavigationNodeId = NavigationNodeId(1);
const CONFIRM: NavigationNodeId = NavigationNodeId(2);

struct Root {
    compositor: Compositor,
    surface: SurfaceId,
    orchestrator: NavigationSurfaceOrchestrator,
    app: NavigationApplicationCoordinator,
    // The root and the frame participant stay live through every transition.
    _binding: FrameProjectionBindingRegistry,
    _tree: WidgetTree,
}

impl Root {
    fn new(host: NavigationHost) -> Self {
        let bounds = RectF::new(0.0, 0.0, 800.0, 600.0);
        let mut compositor = Compositor::new(bounds);
        let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
        let orchestrator = NavigationSurfaceOrchestrator::new_with_existing_root_surface(
            host,
            bounds,
            surface,
            &mut compositor,
            Box::new(IncrementalSceneKeyAllocator::new()),
        )
        .with_window_projection(surface);
        let domains = InteractionDomains::new();
        let overlays = OverlayDomainCoordinator::new().unwrap();
        overlays
            .register_domain(&domains, InteractionDomainId::PRIMARY)
            .unwrap();
        let mut tree = WidgetTree::new();
        let screen_host = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let binding = FrameProjectionBindingRegistry::new(&overlays);
        let guard = overlays
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .unwrap();
        binding
            .install_or_replace(
                FrameProjectionAuthorityAccess::new(&domains, &overlays, &FocusResolver::new()),
                FrameProjectionWindowAccess::new(
                    WindowId::PRIMARY,
                    InteractionDomainId::PRIMARY,
                    &compositor,
                    &mut tree,
                ),
                ScreenHostProjectionKey::new(WindowId::PRIMARY, screen_host),
                FrameProjectionBinding::new(
                    InteractionDomainId::PRIMARY,
                    vec![NavigationProjectionSource::standalone(
                        InteractionDomainId::PRIMARY,
                        orchestrator.projection_handle(),
                    )],
                ),
                &guard,
            )
            .unwrap();
        drop(guard);
        Self {
            compositor,
            surface,
            orchestrator,
            app: NavigationApplicationCoordinator::new(overlays),
            _binding: binding,
            _tree: tree,
        }
    }

    fn snapshot(&self) -> StandaloneNavigationSnapshot {
        match self
            .app
            .standalone_snapshot(&self.orchestrator, InteractionDomainId::PRIMARY)
        {
            StandaloneNavigationSnapshotResult::Snapshot(snapshot) => snapshot,
            StandaloneNavigationSnapshotResult::Retry => panic!("expected stable root"),
        }
    }

    fn commit(
        &mut self,
        snapshot: &StandaloneNavigationSnapshot,
        result: NavigationAuthorizationResult,
    ) {
        let NavigationAuthorizationResult::Authorized(authorization) = result else {
            panic!("expected an authorized candidate, got {result:?}");
        };
        self.commit_authorized(snapshot, authorization);
    }

    fn commit_authorized(
        &mut self,
        snapshot: &StandaloneNavigationSnapshot,
        authorization: NavigationGuardAuthorization,
    ) {
        let prepared = self
            .app
            .prepare_standalone_navigation(
                &mut self.orchestrator,
                &mut self.compositor,
                snapshot,
                authorization,
                None,
            )
            .unwrap();
        assert!(prepared.staged_surfaces().is_empty());
        assert_eq!(prepared.next_active_surface(), self.surface);
        assert_eq!(prepared.commit().active_surface(), Some(self.surface));
        let projection = self.orchestrator.projection_handle().project().unwrap();
        assert_eq!(
            projection.navigation_surfaces,
            vec![self.surface; self.orchestrator.nav_host().path().len()]
        );
        assert_eq!(projection.active_surface, Some(self.surface));
        assert_eq!(self.compositor.snapshot_multi().iter().count(), 1);
    }

    fn navigate(&mut self, node: NavigationNodeId) {
        let snapshot = self.snapshot();
        let result = self.app.authorize_standalone_navigation(
            &self.orchestrator,
            &snapshot,
            NavigationIntent::NavigateTo(node.into()),
            None,
            None,
        );
        self.commit(&snapshot, result);
    }
}

#[test]
fn web_history_deep_link_keeps_one_initial_slot_and_structural_up() {
    let mut root = Root::new(NavigationHost::new(HOME));
    let snapshot = root.snapshot();
    let result = root
        .orchestrator
        .nav_host()
        .authorize_initial_route(snapshot.host_snapshot(), CONFIRM.into());
    root.commit(&snapshot, result);
    let host = root.orchestrator.nav_host();
    let before = host.snapshot().unwrap();
    assert_eq!(before.history_len(), 1);
    assert_eq!(before.history_cursor(), 0);
    assert_eq!(host.path(), [HOME, CONFIRM]);
    let projection = root.orchestrator.projection_handle().project().unwrap();
    assert_eq!(projection.navigation_surfaces, [root.surface, root.surface]);
    assert!(host.control_projection().can_navigate_up);
    assert!(!host.control_projection().can_history_back);
    assert!(matches!(
        host.authorize_initial_route(&before, HOME.into()),
        NavigationAuthorizationResult::Unavailable,
    ));
    let snapshot = root.snapshot();
    let result = root.app.authorize_standalone_navigation(
        &root.orchestrator,
        &snapshot,
        NavigationIntent::NavigateUp,
        None,
        None,
    );
    root.commit(&snapshot, result);
    let host = root.orchestrator.nav_host();
    assert_eq!(host.current(), HOME);
    let projection = root.orchestrator.projection_handle().project().unwrap();
    assert_eq!(projection.navigation_surfaces, [root.surface]);
    assert!(host.control_projection().can_history_back);
}

struct DirectionGuard {
    deny_back: Rc<Cell<bool>>,
}

impl NavigationGuard for DirectionGuard {
    fn evaluate(
        &self,
        requested: &NavigationIntent,
        _: &NavigationIntent,
        _: &NavigationState,
        _: &NavigationState,
        _: Option<&dyn Any>,
    ) -> GuardDecision {
        if self.deny_back.get() && *requested == NavigationIntent::HistoryBack {
            GuardDecision::Deny
        } else {
            GuardDecision::Allow
        }
    }
}

#[test]
fn web_history_multi_entry_traversal_respects_guard_and_frame_participant() {
    let deny_back = Rc::new(Cell::new(false));
    let mut host = NavigationHost::new(HOME);
    host.set_guard(Box::new(DirectionGuard {
        deny_back: deny_back.clone(),
    }));
    let mut root = Root::new(host);
    root.navigate(CONFIRM);
    root.navigate(HOME);

    let snapshot = root.snapshot();
    let source = snapshot.host_snapshot();
    let host = root.orchestrator.nav_host();
    assert_eq!(source.history_cursor(), 2);
    assert_eq!(source.history_state(0), Some(source.state()));
    assert!(matches!(
        host.authorize_history_cursor(source, usize::MAX),
        NavigationAuthorizationResult::Unavailable,
    ));
    assert!(matches!(
        NavigationHost::new(HOME).authorize_history_cursor(source, 0),
        NavigationAuthorizationResult::Retry,
    ));
    deny_back.set(true);
    assert!(matches!(
        host.authorize_history_cursor(source, 0),
        NavigationAuthorizationResult::Denied,
    ));
    assert_eq!(host.snapshot().unwrap().history_cursor(), 2);
    deny_back.set(false);
    let result = host.authorize_history_cursor(source, 0);
    let NavigationAuthorizationResult::Authorized(authorization) = result else {
        panic!("expected exact Back authorization");
    };
    assert_eq!(
        authorization.candidate().requested_intent(),
        &NavigationIntent::HistoryBack
    );
    assert_eq!(authorization.candidate().prospective_cursor(), 0);
    root.commit_authorized(&snapshot, authorization);
    let host = root.orchestrator.nav_host();
    assert_eq!(host.snapshot().unwrap().history_cursor(), 0);
    assert_eq!(
        host.execute(NavigationIntent::HistoryForward, None, None),
        Err(NavigationExecutionError::Retry),
    );
    let snapshot = root.snapshot();
    let result = host.authorize_history_cursor(snapshot.host_snapshot(), 2);
    root.commit(&snapshot, result);
    let after = root.orchestrator.nav_host().snapshot().unwrap();
    assert_eq!(after.history_len(), 3);
    assert_eq!(after.history_cursor(), 2);
}
