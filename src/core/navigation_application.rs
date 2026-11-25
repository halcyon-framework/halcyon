use std::any::Any;
use std::collections::HashSet;

use crate::compositor::Compositor;
use crate::core::event::{
    InteractionDomainId, InteractionDomains, PreparedInteractionDomainUnregistration, WindowId,
};
use crate::core::focus_authority::{
    FocusAuthorityError, FocusDomainAddition, FocusDomainGraphMutationTestimony, FocusDomainId,
    FocusResolver, PreparedCompleteFocusDomainRetirement, PreparedFocusDomainActivation,
    PreparedFocusDomainGraphMutation, PreparedFocusDomainRetirement,
};
use crate::core::frame_projection_binding_registry::{
    FrameProjectionAuthorityAccess, FrameProjectionBinding, FrameProjectionBindingError,
    FrameProjectionBindingRegistry, FrameProjectionWindowAccess,
    PreparedFrameProjectionBindingMutation, ScreenHostProjectionKey,
};
use crate::core::math::RectF;
use crate::core::navigation_domain_router::{
    DelegationFrame, NavigationDomainRouter, NavigationDomainRouterError,
    NavigationRootFocusBindingError, NavigationRootFocusBindingPort,
    NavigationRootFocusBindingPortId, NavigationRootFocusBindingReplacement,
    PreparedDelegationPush, PreparedDelegationReturn,
    PreparedNavigationDomainRetirement as PreparedNavigationDomainRouterRetirement,
    PreparedRootAssociation, PreparedRootFocusBindingPortAdmission,
    PreparedRootFocusBindingReplacement, PreparedRootFocusBindingRetirement,
    PreparedRootFocusMembershipChange, PreparedRootOwnedFocusDomainChange, PreparedRootRetirement,
    PreparedRoutedNavigationRouterCommit, RoutedNavigationCandidate,
    RoutedNavigationSnapshot as RouterRoutedNavigationSnapshot,
};
use crate::core::navigation_orchestrator::{
    CommittedNavigationSurfaceTransition, DomainRootSurfaceTeardownRequest,
    DomainSurfaceTeardownOverlayExit, NavigationOrchestrationGeneration, NavigationRootId,
    NavigationRootPreparationError, NavigationRootRegistry, NavigationSurfaceOrchestrator,
    NavigationSurfacePreparationError, PreparedDomainSurfaceTeardown,
    PreparedNavigationOverlaySurfaceExit, PreparedNavigationSurfaceTransition,
    PreparedRegistryRoot, PreparedRootSurfaceTeardown, RootAdmissionPublicationCapability,
    RootSurfaceTeardownPublicationCapability, RootTeardownPeerOverlayExit, SceneKeyAllocator,
    SurfaceTerminationReason, SurfaceTransitionPublicationCapability,
};
use crate::core::overlay_domain_coordinator::{
    ActiveOverlayStackBinding, DomainTransitionGuardToken, OverlayActivationAdmission,
    OverlayDisposition, OverlayDomainCoordinator, OverlayDomainError, OverlayOwnerLifecyclePort,
    PreparedOverlayCompleteDomainRetirement, PreparedOverlayDeactivationToDomain,
    PreparedOverlayDomainUnregistration, PreparedOverlayFocusGraphTransition,
    PreparedOverlayOwnerRegistration, PreparedOverlayOwnerUnregistration,
};
use crate::model::navigation_host::{
    NavigationAuthorizationResult, NavigationGuardAuthorization, NavigationGuardContextGeneration,
    NavigationHost, NavigationIntent, NavigationMechanicsSnapshot, NavigationOutcome,
    NavigationPreparationError, PreparedNavigationCommit,
};

#[derive(Debug, Clone)]
pub struct RoutedNavigationSnapshot {
    routing: RouterRoutedNavigationSnapshot,
    host: NavigationMechanicsSnapshot,
    orchestration_generation: NavigationOrchestrationGeneration,
}

impl RoutedNavigationSnapshot {
    pub fn interaction_domain_id(&self) -> InteractionDomainId {
        self.routing.interaction_domain_id
    }

    pub fn root_id(&self) -> NavigationRootId {
        self.routing.root_id
    }

    pub fn host_snapshot(&self) -> &NavigationMechanicsSnapshot {
        &self.host
    }

    pub fn orchestration_generation(&self) -> NavigationOrchestrationGeneration {
        self.orchestration_generation
    }
}

#[derive(Debug, Clone)]
pub enum RoutedNavigationSnapshotResult {
    Snapshot(RoutedNavigationSnapshot),
    RoutingUnavailable,
    Retry,
}

#[derive(Debug, Clone)]
pub struct StandaloneNavigationSnapshot {
    interaction_domain_id: InteractionDomainId,
    host: NavigationMechanicsSnapshot,
    orchestration_generation: NavigationOrchestrationGeneration,
}

impl StandaloneNavigationSnapshot {
    pub fn interaction_domain_id(&self) -> InteractionDomainId {
        self.interaction_domain_id
    }

    pub fn host_snapshot(&self) -> &NavigationMechanicsSnapshot {
        &self.host
    }

    pub fn orchestration_generation(&self) -> NavigationOrchestrationGeneration {
        self.orchestration_generation
    }
}

#[derive(Debug, Clone)]
pub enum StandaloneNavigationSnapshotResult {
    Snapshot(StandaloneNavigationSnapshot),
    Retry,
}

#[derive(Debug)]
pub enum StandaloneNavigationPreparationError {
    ActiveOverlayRequiresExit(ActiveOverlayStackBinding),
    Host(NavigationPreparationError),
    Surface(NavigationSurfacePreparationError),
    Overlay(OverlayDomainError),
}

impl From<NavigationPreparationError> for StandaloneNavigationPreparationError {
    fn from(value: NavigationPreparationError) -> Self {
        Self::Host(value)
    }
}

impl From<NavigationSurfacePreparationError> for StandaloneNavigationPreparationError {
    fn from(value: NavigationSurfacePreparationError) -> Self {
        Self::Surface(value)
    }
}

impl From<OverlayDomainError> for StandaloneNavigationPreparationError {
    fn from(value: OverlayDomainError) -> Self {
        Self::Overlay(value)
    }
}

#[derive(Debug)]
pub enum RoutedNavigationPreparationError {
    RegistryRootUnavailable(NavigationRootId),
    EmptyFocusGraphChange,
    ActiveOverlayRequiresExit(ActiveOverlayStackBinding),
    ActivePeerOverlay(ActiveOverlayStackBinding),
    Host(NavigationPreparationError),
    Surface(NavigationSurfacePreparationError),
    Overlay(OverlayDomainError),
    Focus(FocusAuthorityError),
    FocusBinding(NavigationRootFocusBindingError),
    Router(NavigationDomainRouterError),
}

impl From<NavigationPreparationError> for RoutedNavigationPreparationError {
    fn from(value: NavigationPreparationError) -> Self {
        Self::Host(value)
    }
}

impl From<NavigationSurfacePreparationError> for RoutedNavigationPreparationError {
    fn from(value: NavigationSurfacePreparationError) -> Self {
        Self::Surface(value)
    }
}

impl From<OverlayDomainError> for RoutedNavigationPreparationError {
    fn from(value: OverlayDomainError) -> Self {
        Self::Overlay(value)
    }
}

impl From<FocusAuthorityError> for RoutedNavigationPreparationError {
    fn from(value: FocusAuthorityError) -> Self {
        Self::Focus(value)
    }
}

impl From<NavigationRootFocusBindingError> for RoutedNavigationPreparationError {
    fn from(value: NavigationRootFocusBindingError) -> Self {
        Self::FocusBinding(value)
    }
}

impl From<NavigationDomainRouterError> for RoutedNavigationPreparationError {
    fn from(value: NavigationDomainRouterError) -> Self {
        Self::Router(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationDelegationError {
    UnknownInteractionDomain(InteractionDomainId),
    RegistryRootUnavailable(NavigationRootId),
    RegistryFocusSurfaceMismatch(NavigationRootId),
    ActivePeerOverlay(ActiveOverlayStackBinding),
    Surface(NavigationSurfacePreparationError),
    Overlay(OverlayDomainError),
    Focus(FocusAuthorityError),
    Router(NavigationDomainRouterError),
}

impl From<NavigationSurfacePreparationError> for NavigationDelegationError {
    fn from(value: NavigationSurfacePreparationError) -> Self {
        Self::Surface(value)
    }
}

impl From<OverlayDomainError> for NavigationDelegationError {
    fn from(value: OverlayDomainError) -> Self {
        Self::Overlay(value)
    }
}

impl From<FocusAuthorityError> for NavigationDelegationError {
    fn from(value: FocusAuthorityError) -> Self {
        Self::Focus(value)
    }
}

impl From<NavigationDomainRouterError> for NavigationDelegationError {
    fn from(value: NavigationDomainRouterError) -> Self {
        Self::Router(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationRootAdmissionError {
    UnknownInteractionDomain(InteractionDomainId),
    OwnerInteractionDomainMismatch,
    Registry(NavigationRootPreparationError),
    Overlay(OverlayDomainError),
    Focus(FocusAuthorityError),
    FocusBinding(NavigationRootFocusBindingError),
    Router(NavigationDomainRouterError),
}

impl From<NavigationRootPreparationError> for NavigationRootAdmissionError {
    fn from(value: NavigationRootPreparationError) -> Self {
        Self::Registry(value)
    }
}

impl From<OverlayDomainError> for NavigationRootAdmissionError {
    fn from(value: OverlayDomainError) -> Self {
        Self::Overlay(value)
    }
}

impl From<FocusAuthorityError> for NavigationRootAdmissionError {
    fn from(value: FocusAuthorityError) -> Self {
        Self::Focus(value)
    }
}

impl From<NavigationRootFocusBindingError> for NavigationRootAdmissionError {
    fn from(value: NavigationRootFocusBindingError) -> Self {
        Self::FocusBinding(value)
    }
}

impl From<NavigationDomainRouterError> for NavigationRootAdmissionError {
    fn from(value: NavigationDomainRouterError) -> Self {
        Self::Router(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameProjectionRootConsumerPlan {
    key: ScreenHostProjectionKey,
    replacement: Option<FrameProjectionBinding>,
}

impl FrameProjectionRootConsumerPlan {
    pub const fn remove(key: ScreenHostProjectionKey) -> Self {
        Self {
            key,
            replacement: None,
        }
    }

    pub fn replace(key: ScreenHostProjectionKey, replacement: FrameProjectionBinding) -> Self {
        Self {
            key,
            replacement: Some(replacement),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationRootRetirementError {
    UnknownInteractionDomain(InteractionDomainId),
    RegistryRootUnavailable(NavigationRootId),
    RegistryFocusSurfaceMismatch(NavigationRootId),
    DuplicateProjectionWindow(WindowId),
    DuplicateProjectionPlan(ScreenHostProjectionKey),
    ProjectionPlanMismatch,
    MissingProjectionWindow(WindowId),
    ReplacementReferencesRetiringRoot(ScreenHostProjectionKey),
    ActivePeerOverlayWithReplacement(ActiveOverlayStackBinding),
    ActiveDelegationOverlayOwnerMismatch {
        expected_owner: crate::core::overlay_domain_coordinator::OverlayOwnerId,
        active: ActiveOverlayStackBinding,
    },
    OverlaySurfaceMismatch,
    Registry(NavigationRootPreparationError),
    Projection(FrameProjectionBindingError),
    Overlay(OverlayDomainError),
    Focus(FocusAuthorityError),
    FocusBinding(NavigationRootFocusBindingError),
    Router(NavigationDomainRouterError),
}

impl From<NavigationRootPreparationError> for NavigationRootRetirementError {
    fn from(value: NavigationRootPreparationError) -> Self {
        Self::Registry(value)
    }
}

impl From<FrameProjectionBindingError> for NavigationRootRetirementError {
    fn from(value: FrameProjectionBindingError) -> Self {
        Self::Projection(value)
    }
}

impl From<OverlayDomainError> for NavigationRootRetirementError {
    fn from(value: OverlayDomainError) -> Self {
        Self::Overlay(value)
    }
}

impl From<FocusAuthorityError> for NavigationRootRetirementError {
    fn from(value: FocusAuthorityError) -> Self {
        Self::Focus(value)
    }
}

impl From<NavigationRootFocusBindingError> for NavigationRootRetirementError {
    fn from(value: NavigationRootFocusBindingError) -> Self {
        Self::FocusBinding(value)
    }
}

impl From<NavigationDomainRouterError> for NavigationRootRetirementError {
    fn from(value: NavigationDomainRouterError) -> Self {
        Self::Router(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationDomainRetirementError {
    UnknownInteractionDomain(InteractionDomainId),
    InteractionDomainWindowMismatch(InteractionDomainId),
    RegistryRootUnavailable(NavigationRootId),
    RegistryFocusSurfaceMismatch(NavigationRootId),
    DuplicateProjectionWindow(WindowId),
    MissingProjectionWindow(WindowId),
    ActiveOverlayOwnerUnavailable(ActiveOverlayStackBinding),
    RegistryRootSetMismatch,
    Registry(NavigationRootPreparationError),
    Projection(FrameProjectionBindingError),
    Overlay(OverlayDomainError),
    Focus(FocusAuthorityError),
    FocusBinding(NavigationRootFocusBindingError),
    Router(NavigationDomainRouterError),
}

impl From<NavigationRootPreparationError> for NavigationDomainRetirementError {
    fn from(value: NavigationRootPreparationError) -> Self {
        Self::Registry(value)
    }
}

impl From<FrameProjectionBindingError> for NavigationDomainRetirementError {
    fn from(value: FrameProjectionBindingError) -> Self {
        Self::Projection(value)
    }
}

impl From<OverlayDomainError> for NavigationDomainRetirementError {
    fn from(value: OverlayDomainError) -> Self {
        Self::Overlay(value)
    }
}

impl From<FocusAuthorityError> for NavigationDomainRetirementError {
    fn from(value: FocusAuthorityError) -> Self {
        Self::Focus(value)
    }
}

impl From<NavigationRootFocusBindingError> for NavigationDomainRetirementError {
    fn from(value: NavigationRootFocusBindingError) -> Self {
        Self::FocusBinding(value)
    }
}

impl From<NavigationDomainRouterError> for NavigationDomainRetirementError {
    fn from(value: NavigationDomainRouterError) -> Self {
        Self::Router(value)
    }
}

#[derive(Debug, Clone)]
pub struct NavigationApplicationCoordinator {
    overlay_domains: OverlayDomainCoordinator,
    navigation_domains: NavigationDomainRouter,
}

impl NavigationApplicationCoordinator {
    pub fn new(overlay_domains: OverlayDomainCoordinator) -> Self {
        Self {
            navigation_domains: NavigationDomainRouter::new(overlay_domains.clone()),
            overlay_domains,
        }
    }

    pub fn overlay_domains(&self) -> &OverlayDomainCoordinator {
        &self.overlay_domains
    }

    pub fn navigation_domains(&self) -> &NavigationDomainRouter {
        &self.navigation_domains
    }

    pub fn standalone_snapshot(
        &self,
        orchestrator: &NavigationSurfaceOrchestrator,
        interaction_domain_id: InteractionDomainId,
    ) -> StandaloneNavigationSnapshotResult {
        let host = match orchestrator.nav_host().snapshot() {
            Ok(snapshot) => snapshot,
            Err(_) => return StandaloneNavigationSnapshotResult::Retry,
        };
        let orchestration_generation = orchestrator.orchestration_generation();
        let second_host_generation = match orchestrator.nav_host().snapshot() {
            Ok(snapshot) => snapshot.generation(),
            Err(_) => return StandaloneNavigationSnapshotResult::Retry,
        };
        if second_host_generation != host.generation()
            || orchestrator.orchestration_generation() != orchestration_generation
        {
            return StandaloneNavigationSnapshotResult::Retry;
        }
        StandaloneNavigationSnapshotResult::Snapshot(StandaloneNavigationSnapshot {
            interaction_domain_id,
            host,
            orchestration_generation,
        })
    }

    pub fn authorize_standalone_navigation(
        &self,
        orchestrator: &NavigationSurfaceOrchestrator,
        snapshot: &StandaloneNavigationSnapshot,
        intent: NavigationIntent,
        context: Option<&dyn Any>,
        guard_context_generation: Option<NavigationGuardContextGeneration>,
    ) -> NavigationAuthorizationResult {
        let current_host_generation = match orchestrator.nav_host().snapshot() {
            Ok(current) => current.generation(),
            Err(_) => return NavigationAuthorizationResult::Retry,
        };
        if current_host_generation != snapshot.host.generation()
            || orchestrator.orchestration_generation() != snapshot.orchestration_generation
        {
            return NavigationAuthorizationResult::Retry;
        }
        orchestrator
            .nav_host()
            .authorize(&snapshot.host, intent, context, guard_context_generation)
    }

    pub fn prepare_standalone_navigation<'compositor>(
        &self,
        orchestrator: &mut NavigationSurfaceOrchestrator,
        compositor: &'compositor mut Compositor,
        snapshot: &StandaloneNavigationSnapshot,
        authorization: NavigationGuardAuthorization,
        current_guard_context_generation: Option<NavigationGuardContextGeneration>,
    ) -> Result<
        PreparedStandaloneNavigationTransaction<'compositor>,
        StandaloneNavigationPreparationError,
    > {
        let guard = self
            .overlay_domains
            .begin_domain_transition(snapshot.interaction_domain_id)?;
        if let Some(active_overlay) = self.overlay_domains.active_binding_guarded(&guard)? {
            return Err(
                StandaloneNavigationPreparationError::ActiveOverlayRequiresExit(active_overlay),
            );
        }
        if orchestrator.orchestration_generation() != snapshot.orchestration_generation {
            return Err(StandaloneNavigationPreparationError::Surface(
                NavigationSurfacePreparationError::StaleGeneration,
            ));
        }
        let host_commit = orchestrator.prepare_host_commit_guarded(
            authorization,
            snapshot.host.generation(),
            current_guard_context_generation,
            &guard,
        )?;
        let surface_transition = orchestrator.prepare_navigation_surface_transition_guarded(
            &host_commit,
            snapshot.orchestration_generation,
            compositor,
            &guard,
        )?;
        Ok(PreparedStandaloneNavigationTransaction {
            surface_transition: Some(surface_transition),
            host_commit: Some(host_commit),
            guard: Some(guard),
            completed: false,
        })
    }

    pub fn route_snapshot(
        &self,
        registry: &NavigationRootRegistry,
        interaction_domain_id: InteractionDomainId,
        explicitly_scoped_root: Option<NavigationRootId>,
    ) -> RoutedNavigationSnapshotResult {
        let routing = match self
            .navigation_domains
            .route_snapshot(interaction_domain_id, explicitly_scoped_root)
        {
            RoutedNavigationCandidate::Snapshot(snapshot) => snapshot,
            RoutedNavigationCandidate::RoutingUnavailable => {
                return RoutedNavigationSnapshotResult::RoutingUnavailable;
            }
            RoutedNavigationCandidate::Retry => return RoutedNavigationSnapshotResult::Retry,
        };
        let Some(root) = registry.root(routing.root_id) else {
            return RoutedNavigationSnapshotResult::Retry;
        };
        let host = match root.nav_host().snapshot() {
            Ok(snapshot) => snapshot,
            Err(_) => return RoutedNavigationSnapshotResult::Retry,
        };
        let orchestration_generation = root.orchestration_generation();

        let second_routing = self
            .navigation_domains
            .route_snapshot(interaction_domain_id, explicitly_scoped_root);
        if second_routing != RoutedNavigationCandidate::Snapshot(routing.clone()) {
            return RoutedNavigationSnapshotResult::Retry;
        }
        let Some(second_root) = registry.root(routing.root_id) else {
            return RoutedNavigationSnapshotResult::Retry;
        };
        let second_host_generation = match second_root.nav_host().snapshot() {
            Ok(snapshot) => snapshot.generation(),
            Err(_) => return RoutedNavigationSnapshotResult::Retry,
        };
        if second_host_generation != host.generation()
            || second_root.orchestration_generation() != orchestration_generation
        {
            return RoutedNavigationSnapshotResult::Retry;
        }

        RoutedNavigationSnapshotResult::Snapshot(RoutedNavigationSnapshot {
            routing,
            host,
            orchestration_generation,
        })
    }

    pub fn authorize_routed_navigation(
        &self,
        registry: &NavigationRootRegistry,
        snapshot: &RoutedNavigationSnapshot,
        intent: NavigationIntent,
        context: Option<&dyn Any>,
        guard_context_generation: Option<NavigationGuardContextGeneration>,
    ) -> NavigationAuthorizationResult {
        let scoped_root = snapshot
            .routing
            .explicitly_scoped
            .then_some(snapshot.routing.root_id);
        if self
            .navigation_domains
            .route_snapshot(snapshot.routing.interaction_domain_id, scoped_root)
            != RoutedNavigationCandidate::Snapshot(snapshot.routing.clone())
        {
            return NavigationAuthorizationResult::Retry;
        }
        let Some(root) = registry.root(snapshot.routing.root_id) else {
            return NavigationAuthorizationResult::Retry;
        };
        let current_host_generation = match root.nav_host().snapshot() {
            Ok(current) => current.generation(),
            Err(_) => return NavigationAuthorizationResult::Retry,
        };
        if current_host_generation != snapshot.host.generation()
            || root.orchestration_generation() != snapshot.orchestration_generation
        {
            return NavigationAuthorizationResult::Retry;
        }
        root.nav_host()
            .authorize(&snapshot.host, intent, context, guard_context_generation)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_routed_navigation<'compositor, 'focus>(
        &self,
        interaction_domains: &'focus mut InteractionDomains,
        registry: &mut NavigationRootRegistry,
        compositor: &'compositor mut Compositor,
        snapshot: &RoutedNavigationSnapshot,
        authorization: NavigationGuardAuthorization,
        current_guard_context_generation: Option<NavigationGuardContextGeneration>,
        target_focus_domain: FocusDomainId,
    ) -> Result<
        PreparedRoutedNavigationTransaction<'compositor, 'focus>,
        RoutedNavigationPreparationError,
    > {
        let guard = self
            .overlay_domains
            .begin_domain_transition(snapshot.routing.interaction_domain_id)?;
        let focus_binding_port = self
            .navigation_domains
            .focus_binding_port_for_routed_snapshot_guarded(&guard, &snapshot.routing)?;
        if let Some(active_overlay) = self.overlay_domains.active_binding_guarded(&guard)? {
            return Err(RoutedNavigationPreparationError::ActiveOverlayRequiresExit(
                active_overlay,
            ));
        }

        let root = registry.root_mut(snapshot.routing.root_id).ok_or(
            RoutedNavigationPreparationError::RegistryRootUnavailable(snapshot.routing.root_id),
        )?;
        if root.orchestration_generation() != snapshot.orchestration_generation {
            return Err(RoutedNavigationPreparationError::Surface(
                NavigationSurfacePreparationError::StaleGeneration,
            ));
        }
        let host_commit = root.prepare_host_commit_guarded(
            authorization,
            snapshot.host.generation(),
            current_guard_context_generation,
            &guard,
        )?;
        let surface_transition = root.prepare_navigation_surface_transition_guarded(
            &host_commit,
            snapshot.orchestration_generation,
            compositor,
            &guard,
        )?;
        let binding_replacement = focus_binding_port.prepare_replace_guarded(
            &guard,
            snapshot.routing.focus_binding.navigation_surface_id,
            surface_transition.next_active_surface(),
            target_focus_domain,
        )?;
        let focus_manager = &mut interaction_domains
            .state_mut(snapshot.routing.interaction_domain_id)
            .ok_or(FocusAuthorityError::CapabilityMismatch)?
            .focus_manager;
        let focus_activation = FocusResolver::new().prepare_focus_domain_activation(
            guard.focus_capability(),
            focus_manager,
            target_focus_domain,
        )?;
        let router_commit = self.navigation_domains.prepare_routed_navigation_guarded(
            &guard,
            &snapshot.routing,
            binding_replacement.testimony(),
        )?;

        Ok(PreparedRoutedNavigationTransaction {
            application: self.clone(),
            router_commit: Some(router_commit),
            focus_activation: Some(focus_activation),
            focus_binding_port,
            binding_replacement: Some(binding_replacement),
            surface_transition: Some(surface_transition),
            host_commit: Some(host_commit),
            guard: Some(guard),
            completed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_routed_navigation_with_focus_graph<'compositor, 'focus>(
        &self,
        interaction_domains: &'focus mut InteractionDomains,
        registry: &mut NavigationRootRegistry,
        compositor: &'compositor mut Compositor,
        snapshot: &RoutedNavigationSnapshot,
        authorization: NavigationGuardAuthorization,
        current_guard_context_generation: Option<NavigationGuardContextGeneration>,
        focus_domain_additions: &[FocusDomainAddition],
        complete_focus_subtree_removals: &[FocusDomainId],
        optional_replacement_active_domain: Option<FocusDomainId>,
        next_focus_domain: FocusDomainId,
    ) -> Result<
        PreparedRoutedNavigationFocusGraphTransaction<'compositor, 'focus>,
        RoutedNavigationPreparationError,
    > {
        if focus_domain_additions.is_empty() && complete_focus_subtree_removals.is_empty() {
            return Err(RoutedNavigationPreparationError::EmptyFocusGraphChange);
        }
        let guard = self
            .overlay_domains
            .begin_domain_transition(snapshot.routing.interaction_domain_id)?;
        let focus_binding_port = self
            .navigation_domains
            .focus_binding_port_for_routed_snapshot_guarded(&guard, &snapshot.routing)?;
        if let Some(active_overlay) = self.overlay_domains.active_binding_guarded(&guard)? {
            return Err(RoutedNavigationPreparationError::ActiveOverlayRequiresExit(
                active_overlay,
            ));
        }

        let root = registry.root_mut(snapshot.routing.root_id).ok_or(
            RoutedNavigationPreparationError::RegistryRootUnavailable(snapshot.routing.root_id),
        )?;
        if root.orchestration_generation() != snapshot.orchestration_generation {
            return Err(RoutedNavigationPreparationError::Surface(
                NavigationSurfacePreparationError::StaleGeneration,
            ));
        }
        let host_commit = root.prepare_host_commit_guarded(
            authorization,
            snapshot.host.generation(),
            current_guard_context_generation,
            &guard,
        )?;
        let surface_transition = root.prepare_navigation_surface_transition_guarded(
            &host_commit,
            snapshot.orchestration_generation,
            compositor,
            &guard,
        )?;
        let focus_manager = &mut interaction_domains
            .state_mut(snapshot.routing.interaction_domain_id)
            .ok_or(FocusAuthorityError::CapabilityMismatch)?
            .focus_manager;
        let focus_graph = FocusResolver::new().prepare_focus_domain_graph_mutation(
            guard.focus_capability(),
            focus_manager,
            focus_domain_additions,
            complete_focus_subtree_removals,
            vec![],
            optional_replacement_active_domain,
        )?;
        let binding_change = focus_binding_port.prepare_owned_domain_change_guarded(
            &guard,
            snapshot.routing.focus_binding.generation,
            focus_graph.testimony(),
            Some(NavigationRootFocusBindingReplacement::new(
                snapshot.routing.focus_binding.navigation_surface_id,
                snapshot.routing.focus_binding.focus_domain_id,
                surface_transition.next_active_surface(),
                next_focus_domain,
            )),
        )?;
        let router_commit = self
            .navigation_domains
            .prepare_root_focus_membership_change_guarded(
                &guard,
                snapshot.routing.root_id,
                snapshot.routing.router_generation,
                binding_change.testimony(),
                focus_graph.testimony(),
            )?;

        Ok(PreparedRoutedNavigationFocusGraphTransaction {
            application: self.clone(),
            router_commit: Some(router_commit),
            focus_graph: Some(focus_graph),
            focus_binding_port,
            binding_change: Some(binding_change),
            surface_transition: Some(surface_transition),
            host_commit: Some(host_commit),
            guard: Some(guard),
            completed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_routed_navigation_with_overlay_exit<'compositor, 'focus>(
        &self,
        interaction_domains: &'focus mut InteractionDomains,
        registry: &mut NavigationRootRegistry,
        compositor: &'compositor mut Compositor,
        snapshot: &RoutedNavigationSnapshot,
        authorization: NavigationGuardAuthorization,
        current_guard_context_generation: Option<NavigationGuardContextGeneration>,
        disposition: OverlayDisposition,
        target_focus_domain: FocusDomainId,
    ) -> Result<
        PreparedRoutedNavigationOverlayExitTransaction<'compositor, 'focus>,
        RoutedNavigationPreparationError,
    > {
        let guard = self
            .overlay_domains
            .begin_domain_transition(snapshot.routing.interaction_domain_id)?;
        let focus_binding_port = self
            .navigation_domains
            .focus_binding_port_for_routed_snapshot_guarded(&guard, &snapshot.routing)?;
        let active_overlay = self.overlay_domains.active_binding_guarded(&guard)?.ok_or(
            OverlayDomainError::OwnerInactive(snapshot.routing.overlay_owner_id),
        )?;
        if active_overlay.owner_id != snapshot.routing.overlay_owner_id {
            return Err(RoutedNavigationPreparationError::ActivePeerOverlay(
                active_overlay,
            ));
        }
        let overlay_exit = self.overlay_domains.prepare_deactivation_to_domain(
            &guard,
            interaction_domains,
            snapshot.routing.overlay_owner_id,
            disposition,
            target_focus_domain,
        )?;

        let root = registry.root_mut(snapshot.routing.root_id).ok_or(
            RoutedNavigationPreparationError::RegistryRootUnavailable(snapshot.routing.root_id),
        )?;
        if root.orchestration_generation() != snapshot.orchestration_generation {
            return Err(RoutedNavigationPreparationError::Surface(
                NavigationSurfacePreparationError::StaleGeneration,
            ));
        }
        let host_commit = root.prepare_host_commit_guarded(
            authorization,
            snapshot.host.generation(),
            current_guard_context_generation,
            &guard,
        )?;
        let surface_transition = root.prepare_navigation_surface_transition_with_overlay_exit(
            &host_commit,
            snapshot.orchestration_generation,
            compositor,
            overlay_exit.surface_exit_testimony(),
            &guard,
        )?;
        let binding_replacement = focus_binding_port.prepare_replace_guarded(
            &guard,
            snapshot.routing.focus_binding.navigation_surface_id,
            surface_transition.next_active_surface(),
            target_focus_domain,
        )?;
        let router_commit = self.navigation_domains.prepare_routed_navigation_guarded(
            &guard,
            &snapshot.routing,
            binding_replacement.testimony(),
        )?;

        Ok(PreparedRoutedNavigationOverlayExitTransaction {
            application: self.clone(),
            router_commit: Some(router_commit),
            overlay_exit: Some(overlay_exit),
            focus_binding_port,
            binding_replacement: Some(binding_replacement),
            surface_transition: Some(surface_transition),
            host_commit: Some(host_commit),
            guard: Some(guard),
            completed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_routed_navigation_with_overlay_and_focus_graph<'compositor, 'focus>(
        &self,
        interaction_domains: &'focus mut InteractionDomains,
        registry: &mut NavigationRootRegistry,
        compositor: &'compositor mut Compositor,
        snapshot: &RoutedNavigationSnapshot,
        authorization: NavigationGuardAuthorization,
        current_guard_context_generation: Option<NavigationGuardContextGeneration>,
        disposition: OverlayDisposition,
        focus_domain_additions: &[FocusDomainAddition],
        complete_focus_subtree_removals: &[FocusDomainId],
        optional_replacement_active_domain: Option<FocusDomainId>,
        next_focus_domain: FocusDomainId,
    ) -> Result<
        PreparedRoutedNavigationOverlayFocusGraphTransaction<'compositor, 'focus>,
        RoutedNavigationPreparationError,
    > {
        if focus_domain_additions.is_empty() && complete_focus_subtree_removals.is_empty() {
            return Err(RoutedNavigationPreparationError::EmptyFocusGraphChange);
        }
        let guard = self
            .overlay_domains
            .begin_domain_transition(snapshot.routing.interaction_domain_id)?;
        let focus_binding_port = self
            .navigation_domains
            .focus_binding_port_for_routed_snapshot_guarded(&guard, &snapshot.routing)?;
        let active_overlay = self.overlay_domains.active_binding_guarded(&guard)?.ok_or(
            OverlayDomainError::OwnerInactive(snapshot.routing.overlay_owner_id),
        )?;
        if active_overlay.owner_id != snapshot.routing.overlay_owner_id {
            return Err(RoutedNavigationPreparationError::ActivePeerOverlay(
                active_overlay,
            ));
        }
        let overlay_graph = self
            .overlay_domains
            .prepare_deactivate_for_focus_graph_mutation_guarded(
                &guard,
                interaction_domains,
                snapshot.routing.overlay_owner_id,
                disposition,
                focus_domain_additions,
                complete_focus_subtree_removals,
                optional_replacement_active_domain,
            )?;

        let root = registry.root_mut(snapshot.routing.root_id).ok_or(
            RoutedNavigationPreparationError::RegistryRootUnavailable(snapshot.routing.root_id),
        )?;
        if root.orchestration_generation() != snapshot.orchestration_generation {
            return Err(RoutedNavigationPreparationError::Surface(
                NavigationSurfacePreparationError::StaleGeneration,
            ));
        }
        let host_commit = root.prepare_host_commit_guarded(
            authorization,
            snapshot.host.generation(),
            current_guard_context_generation,
            &guard,
        )?;
        let surface_transition = root.prepare_navigation_surface_transition_with_overlay_exit(
            &host_commit,
            snapshot.orchestration_generation,
            compositor,
            overlay_graph.surface_exit_testimony(),
            &guard,
        )?;
        let binding_change = focus_binding_port.prepare_owned_domain_change_guarded(
            &guard,
            snapshot.routing.focus_binding.generation,
            overlay_graph.focus_graph_testimony(),
            Some(NavigationRootFocusBindingReplacement::new(
                snapshot.routing.focus_binding.navigation_surface_id,
                snapshot.routing.focus_binding.focus_domain_id,
                surface_transition.next_active_surface(),
                next_focus_domain,
            )),
        )?;
        let router_commit = self
            .navigation_domains
            .prepare_root_focus_membership_change_guarded(
                &guard,
                snapshot.routing.root_id,
                snapshot.routing.router_generation,
                binding_change.testimony(),
                overlay_graph.focus_graph_testimony(),
            )?;

        Ok(PreparedRoutedNavigationOverlayFocusGraphTransaction {
            application: self.clone(),
            router_commit: Some(router_commit),
            overlay_graph: Some(overlay_graph),
            focus_binding_port,
            binding_change: Some(binding_change),
            surface_transition: Some(surface_transition),
            host_commit: Some(host_commit),
            guard: Some(guard),
            completed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_delegation<'compositor, 'focus>(
        &self,
        interaction_domains: &'focus mut InteractionDomains,
        registry: &mut NavigationRootRegistry,
        compositor: &'compositor mut Compositor,
        interaction_domain_id: InteractionDomainId,
        source_root: NavigationRootId,
        target_root: NavigationRootId,
        source_overlay_disposition: OverlayDisposition,
    ) -> Result<PreparedNavigationDelegation<'compositor, 'focus>, NavigationDelegationError> {
        if interaction_domains.state(interaction_domain_id).is_none() {
            return Err(NavigationDelegationError::UnknownInteractionDomain(
                interaction_domain_id,
            ));
        }
        let guard = self
            .overlay_domains
            .begin_domain_transition(interaction_domain_id)?;
        let prepared_router =
            self.navigation_domains
                .prepare_delegation_guarded(&guard, source_root, target_root)?;
        let frame = prepared_router.frame();
        let source_generation = registry
            .root(frame.source_root)
            .map(|root| root.orchestration_generation())
            .ok_or(NavigationDelegationError::RegistryRootUnavailable(
                frame.source_root,
            ))?;
        let target_root_orchestrator = registry.root(frame.target_root).ok_or(
            NavigationDelegationError::RegistryRootUnavailable(frame.target_root),
        )?;
        if target_root_orchestrator.surface_for_node(target_root_orchestrator.current_node())
            != Some(
                prepared_router
                    .destination_focus_binding()
                    .navigation_surface_id,
            )
        {
            return Err(NavigationDelegationError::RegistryFocusSurfaceMismatch(
                frame.target_root,
            ));
        }
        let target_focus_domain = prepared_router.destination_focus_binding().focus_domain_id;
        let active_overlay = self.overlay_domains.active_binding_guarded(&guard)?;
        let (focus_transition, surface_exit) = match active_overlay {
            None => {
                let focus_manager = &mut interaction_domains
                    .state_mut(interaction_domain_id)
                    .expect("delegation domain was validated")
                    .focus_manager;
                let activation = FocusResolver::new().prepare_focus_domain_activation(
                    guard.focus_capability(),
                    focus_manager,
                    target_focus_domain,
                )?;
                (PreparedNavigationContextFocus::Activation(activation), None)
            }
            Some(active) if active.owner_id == prepared_router.source_owner_id() => {
                let overlay_exit = self.overlay_domains.prepare_deactivation_to_domain(
                    &guard,
                    interaction_domains,
                    active.owner_id,
                    source_overlay_disposition,
                    target_focus_domain,
                )?;
                let surface_exit = registry
                    .root_mut(frame.source_root)
                    .ok_or(NavigationDelegationError::RegistryRootUnavailable(
                        frame.source_root,
                    ))?
                    .prepare_overlay_surface_exit(
                        source_generation,
                        compositor,
                        overlay_exit.surface_exit_testimony(),
                    )?;
                (
                    PreparedNavigationContextFocus::OverlayExit(overlay_exit),
                    Some(surface_exit),
                )
            }
            Some(active) if active.owner_id == prepared_router.target_owner_id() => (
                PreparedNavigationContextFocus::RetainedDestinationOverlay,
                None,
            ),
            Some(active) => return Err(NavigationDelegationError::ActivePeerOverlay(active)),
        };
        Ok(PreparedNavigationDelegation {
            application: self.clone(),
            router: Some(prepared_router),
            focus_transition: Some(focus_transition),
            surface_exit,
            guard: Some(guard),
            completed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_delegation_return<'compositor, 'focus>(
        &self,
        interaction_domains: &'focus mut InteractionDomains,
        registry: &mut NavigationRootRegistry,
        compositor: &'compositor mut Compositor,
        interaction_domain_id: InteractionDomainId,
        target_overlay_disposition: OverlayDisposition,
    ) -> Result<PreparedNavigationDelegationReturn<'compositor, 'focus>, NavigationDelegationError>
    {
        if interaction_domains.state(interaction_domain_id).is_none() {
            return Err(NavigationDelegationError::UnknownInteractionDomain(
                interaction_domain_id,
            ));
        }
        let guard = self
            .overlay_domains
            .begin_domain_transition(interaction_domain_id)?;
        let prepared_router = self
            .navigation_domains
            .prepare_delegation_return_guarded(&guard)?;
        let frame = prepared_router.frame();
        let target_generation = registry
            .root(frame.target_root)
            .map(|root| root.orchestration_generation())
            .ok_or(NavigationDelegationError::RegistryRootUnavailable(
                frame.target_root,
            ))?;
        let source_root_orchestrator = registry.root(frame.source_root).ok_or(
            NavigationDelegationError::RegistryRootUnavailable(frame.source_root),
        )?;
        if source_root_orchestrator.surface_for_node(source_root_orchestrator.current_node())
            != Some(
                prepared_router
                    .destination_focus_binding()
                    .navigation_surface_id,
            )
        {
            return Err(NavigationDelegationError::RegistryFocusSurfaceMismatch(
                frame.source_root,
            ));
        }
        let source_focus_domain = prepared_router.destination_focus_binding().focus_domain_id;
        let active_overlay = self.overlay_domains.active_binding_guarded(&guard)?;
        let (focus_transition, surface_exit) = match active_overlay {
            None => {
                let focus_manager = &mut interaction_domains
                    .state_mut(interaction_domain_id)
                    .expect("delegation-return domain was validated")
                    .focus_manager;
                let activation = FocusResolver::new().prepare_focus_domain_activation(
                    guard.focus_capability(),
                    focus_manager,
                    source_focus_domain,
                )?;
                (PreparedNavigationContextFocus::Activation(activation), None)
            }
            Some(active) if active.owner_id == prepared_router.target_owner_id() => {
                let overlay_exit = self.overlay_domains.prepare_deactivation_to_domain(
                    &guard,
                    interaction_domains,
                    active.owner_id,
                    target_overlay_disposition,
                    source_focus_domain,
                )?;
                let surface_exit = registry
                    .root_mut(frame.target_root)
                    .ok_or(NavigationDelegationError::RegistryRootUnavailable(
                        frame.target_root,
                    ))?
                    .prepare_overlay_surface_exit(
                        target_generation,
                        compositor,
                        overlay_exit.surface_exit_testimony(),
                    )?;
                (
                    PreparedNavigationContextFocus::OverlayExit(overlay_exit),
                    Some(surface_exit),
                )
            }
            Some(active) => return Err(NavigationDelegationError::ActivePeerOverlay(active)),
        };
        Ok(PreparedNavigationDelegationReturn {
            application: self.clone(),
            router: Some(prepared_router),
            focus_transition: Some(focus_transition),
            surface_exit,
            guard: Some(guard),
            completed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_root_admission<'registry, 'compositor, 'focus>(
        &self,
        interaction_domains: &'focus mut InteractionDomains,
        registry: &'registry mut NavigationRootRegistry,
        compositor: &'compositor mut Compositor,
        interaction_domain_id: InteractionDomainId,
        root_id: NavigationRootId,
        host: NavigationHost,
        viewport: RectF,
        allocator: Box<dyn SceneKeyAllocator>,
        overlay_owner_port: OverlayOwnerLifecyclePort,
        focus_binding_port_id: NavigationRootFocusBindingPortId,
        initial_focus_domain_additions: &[FocusDomainAddition],
        initial_focus_domain_id: FocusDomainId,
    ) -> Result<
        PreparedNavigationRootAdmission<'registry, 'compositor, 'focus>,
        NavigationRootAdmissionError,
    > {
        if interaction_domains.state(interaction_domain_id).is_none() {
            return Err(NavigationRootAdmissionError::UnknownInteractionDomain(
                interaction_domain_id,
            ));
        }
        if overlay_owner_port.interaction_domain_id() != interaction_domain_id {
            return Err(NavigationRootAdmissionError::OwnerInteractionDomainMismatch);
        }
        let guard = self
            .overlay_domains
            .begin_domain_transition(interaction_domain_id)?;
        self.navigation_domains
            .validate_root_admission_absence_guarded(
                &guard,
                root_id,
                overlay_owner_port.owner_id(),
                focus_binding_port_id,
            )?;

        let mut prepared_root =
            registry.prepare_create_root(root_id, host, viewport, compositor, allocator)?;
        prepared_root.prepare_application_participation(&guard)?;
        let prepared_initial_surface = prepared_root.current_surface();
        let prepared_owner = self.overlay_domains.prepare_owner_registration(
            overlay_owner_port,
            OverlayActivationAdmission::RouterEligibilityRequired,
        )?;
        let focus_manager = &mut interaction_domains
            .state_mut(interaction_domain_id)
            .expect("interaction domain was validated before root preparation")
            .focus_manager;
        let optional_initial_activation = focus_manager
            .active_focus_domain()
            .is_none()
            .then_some(initial_focus_domain_id);
        let resolver = FocusResolver::new();
        let prepared_focus_graph = resolver.prepare_focus_domain_graph_mutation(
            guard.focus_capability(),
            focus_manager,
            initial_focus_domain_additions,
            &[],
            vec![],
            optional_initial_activation,
        )?;
        let prepared_focus_binding = NavigationRootFocusBindingPort::prepare_create_guarded(
            &guard,
            prepared_root.root_id(),
            focus_binding_port_id,
            prepared_initial_surface,
            initial_focus_domain_id,
            prepared_focus_graph.testimony(),
        )?;
        let prepared_router_association =
            self.navigation_domains.prepare_root_association_guarded(
                &guard,
                prepared_root.root_id(),
                prepared_owner.owner_id(),
                prepared_focus_binding.testimony(),
                prepared_focus_graph.testimony(),
            )?;

        Ok(PreparedNavigationRootAdmission {
            application: self.clone(),
            prepared_router_association: Some(prepared_router_association),
            prepared_focus_binding: Some(prepared_focus_binding),
            prepared_focus_graph: Some(prepared_focus_graph),
            prepared_owner: Some(prepared_owner),
            prepared_root: Some(prepared_root),
            guard: Some(guard),
            completed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_root_retirement<'registry, 'compositor, 'focus, 'windows>(
        &self,
        interaction_domains: &'focus mut InteractionDomains,
        registry: &'registry mut NavigationRootRegistry,
        compositor: &'compositor mut Compositor,
        projection_bindings: &FrameProjectionBindingRegistry,
        mut projection_windows: Vec<FrameProjectionWindowAccess<'windows>>,
        projection_plans: Vec<FrameProjectionRootConsumerPlan>,
        interaction_domain_id: InteractionDomainId,
        root_id: NavigationRootId,
        replacement_primed_root: Option<NavigationRootId>,
    ) -> Result<
        PreparedNavigationRootRetirement<'registry, 'compositor, 'focus, 'windows>,
        NavigationRootRetirementError,
    > {
        if interaction_domains.state(interaction_domain_id).is_none() {
            return Err(NavigationRootRetirementError::UnknownInteractionDomain(
                interaction_domain_id,
            ));
        }
        let guard = self
            .overlay_domains
            .begin_domain_transition(interaction_domain_id)?;
        let delegation_plan = self
            .navigation_domains
            .root_delegation_retirement_plan_guarded(&guard, root_id, replacement_primed_root)?;
        let retirement_snapshot = self
            .navigation_domains
            .root_retirement_snapshot_guarded(&guard, root_id)?;
        let root = registry.root(root_id).ok_or(
            NavigationRootRetirementError::RegistryRootUnavailable(root_id),
        )?;
        let root_generation = root.orchestration_generation();
        if root.surface_for_node(root.current_node())
            != Some(retirement_snapshot.focus_binding().navigation_surface_id)
        {
            return Err(NavigationRootRetirementError::RegistryFocusSurfaceMismatch(
                root_id,
            ));
        }
        let delegation_current = if delegation_plan.requires_delegation_unwind() {
            let current_root = delegation_plan.current_effective_root().ok_or(
                NavigationRootRetirementError::Router(
                    NavigationDomainRouterError::PreparedValueMismatch,
                ),
            )?;
            if current_root == root_id {
                Some((
                    current_root,
                    retirement_snapshot.overlay_owner_id(),
                    root_generation,
                ))
            } else {
                let current = self
                    .navigation_domains
                    .root_retirement_snapshot_guarded(&guard, current_root)?;
                let current_registry_root = registry.root(current_root).ok_or(
                    NavigationRootRetirementError::RegistryRootUnavailable(current_root),
                )?;
                if current_registry_root.surface_for_node(current_registry_root.current_node())
                    != Some(current.focus_binding().navigation_surface_id)
                {
                    return Err(NavigationRootRetirementError::RegistryFocusSurfaceMismatch(
                        current_root,
                    ));
                }
                Some((
                    current_root,
                    current.overlay_owner_id(),
                    current_registry_root.orchestration_generation(),
                ))
            }
        } else {
            None
        };
        let final_focus_root = if delegation_plan.requires_delegation_unwind() {
            delegation_plan.final_effective_root()
        } else {
            replacement_primed_root
        };
        let final_effective_focus_domain = match final_focus_root {
            Some(final_root) if final_root != root_id => {
                let replacement = self
                    .navigation_domains
                    .root_retirement_snapshot_guarded(&guard, final_root)?;
                let replacement_root = registry.root(final_root).ok_or(
                    NavigationRootRetirementError::RegistryRootUnavailable(final_root),
                )?;
                if replacement_root.surface_for_node(replacement_root.current_node())
                    != Some(replacement.focus_binding().navigation_surface_id)
                {
                    return Err(NavigationRootRetirementError::RegistryFocusSurfaceMismatch(
                        final_root,
                    ));
                }
                Some(replacement.focus_binding().focus_domain_id)
            }
            Some(_) => {
                return Err(NavigationRootRetirementError::Router(
                    NavigationDomainRouterError::PreparedValueMismatch,
                ));
            }
            None => None,
        };

        let mut window_ids = HashSet::new();
        for window in &projection_windows {
            if window.interaction_domain_id != interaction_domain_id {
                return Err(NavigationRootRetirementError::Projection(
                    FrameProjectionBindingError::WindowDomainMismatch,
                ));
            }
            if !window_ids.insert(window.window_id) {
                return Err(NavigationRootRetirementError::DuplicateProjectionWindow(
                    window.window_id,
                ));
            }
        }
        let consumers = projection_bindings.root_consumers(&guard, root_id)?;
        let mut plan_keys = HashSet::new();
        for plan in &projection_plans {
            if !plan_keys.insert(plan.key) {
                return Err(NavigationRootRetirementError::DuplicateProjectionPlan(
                    plan.key,
                ));
            }
        }
        if plan_keys != consumers {
            return Err(NavigationRootRetirementError::ProjectionPlanMismatch);
        }

        let focus_resolver = FocusResolver::new();
        let authorities = FrameProjectionAuthorityAccess::new(
            interaction_domains,
            &self.overlay_domains,
            &focus_resolver,
        )
        .with_registered_roots(registry, &self.navigation_domains);
        let mut projection_mutations = Vec::with_capacity(projection_plans.len());
        for plan in projection_plans {
            if plan
                .replacement
                .as_ref()
                .is_some_and(|binding| binding.references_root(root_id))
            {
                return Err(
                    NavigationRootRetirementError::ReplacementReferencesRetiringRoot(plan.key),
                );
            }
            let window = projection_windows
                .iter()
                .find(|window| window.window_id == plan.key.window_id)
                .ok_or(NavigationRootRetirementError::MissingProjectionWindow(
                    plan.key.window_id,
                ))?;
            let (_, epoch) = projection_bindings
                .guard_identity(plan.key)
                .ok_or(FrameProjectionBindingError::UnknownBinding(plan.key))?;
            let prepared = projection_bindings.prepare_remove_or_replace_guarded(
                authorities,
                window,
                plan.key,
                epoch,
                plan.replacement,
                &guard,
            )?;
            projection_mutations.push(prepared);
        }

        let mut retiring_focus_domains = retirement_snapshot
            .focus_binding()
            .owned_focus_domains
            .iter()
            .copied()
            .collect::<Vec<_>>();
        retiring_focus_domains.sort_by_key(|domain_id| domain_id.0);
        let replacement_focus_domain = interaction_domains
            .state(interaction_domain_id)
            .and_then(|state| state.focus_manager.active_focus_domain())
            .filter(|active_domain| retiring_focus_domains.contains(active_domain))
            .and(final_effective_focus_domain);
        let active_overlay = self.overlay_domains.active_binding_guarded(&guard)?;
        let prepared_focus_retirement = if delegation_plan.requires_delegation_unwind() {
            match active_overlay {
                Some(active) => {
                    let (_, expected_owner, _) =
                        delegation_current.ok_or(NavigationRootRetirementError::Router(
                            NavigationDomainRouterError::PreparedValueMismatch,
                        ))?;
                    if active.owner_id != expected_owner {
                        return Err(
                            NavigationRootRetirementError::ActiveDelegationOverlayOwnerMismatch {
                                expected_owner,
                                active,
                            },
                        );
                    }
                    let prepared = self
                        .overlay_domains
                        .prepare_deactivate_for_focus_retirement_transition_guarded(
                            &guard,
                            interaction_domains,
                            active.owner_id,
                            OverlayDisposition::Dismiss,
                            &retiring_focus_domains,
                            final_effective_focus_domain,
                        )?;
                    if active.owner_id == retirement_snapshot.overlay_owner_id() {
                        PreparedNavigationFocusRetirement::RetiringOwnerOverlay(prepared)
                    } else {
                        PreparedNavigationFocusRetirement::PeerOverlay(prepared)
                    }
                }
                None => {
                    let focus_manager = &mut interaction_domains
                        .state_mut(interaction_domain_id)
                        .expect("retirement domain was validated")
                        .focus_manager;
                    PreparedNavigationFocusRetirement::Direct(
                        FocusResolver::new().prepare_focus_domain_retirement_transition_refs(
                            guard.focus_capability(),
                            focus_manager,
                            &retiring_focus_domains,
                            &[],
                            final_effective_focus_domain,
                        )?,
                    )
                }
            }
        } else {
            match active_overlay {
                Some(active) if active.owner_id == retirement_snapshot.overlay_owner_id() => {
                    let prepared = self
                        .overlay_domains
                        .prepare_deactivate_for_focus_retirement_guarded(
                            &guard,
                            interaction_domains,
                            active.owner_id,
                            OverlayDisposition::Dismiss,
                            &retiring_focus_domains,
                            replacement_focus_domain,
                        )?;
                    PreparedNavigationFocusRetirement::RetiringOwnerOverlay(prepared)
                }
                Some(active) => {
                    if replacement_primed_root.is_some() {
                        return Err(
                            NavigationRootRetirementError::ActivePeerOverlayWithReplacement(active),
                        );
                    }
                    let focus_manager = &mut interaction_domains
                        .state_mut(interaction_domain_id)
                        .expect("retirement domain was validated")
                        .focus_manager;
                    PreparedNavigationFocusRetirement::Direct(
                        FocusResolver::new().prepare_focus_domain_retirement(
                            guard.focus_capability(),
                            focus_manager,
                            &retiring_focus_domains,
                            vec![],
                            None,
                        )?,
                    )
                }
                None => {
                    let focus_manager = &mut interaction_domains
                        .state_mut(interaction_domain_id)
                        .expect("retirement domain was validated")
                        .focus_manager;
                    PreparedNavigationFocusRetirement::Direct(
                        FocusResolver::new().prepare_focus_domain_retirement(
                            guard.focus_capability(),
                            focus_manager,
                            &retiring_focus_domains,
                            vec![],
                            replacement_focus_domain,
                        )?,
                    )
                }
            }
        };
        let focus_binding_port = retirement_snapshot.focus_binding_port().clone();
        let prepared_focus_binding_retirement = focus_binding_port
            .prepare_retire_guarded(&guard, prepared_focus_retirement.testimony())?;
        let prepared_router_retirement = self.navigation_domains.prepare_root_retirement_guarded(
            &guard,
            root_id,
            retirement_snapshot.router_generation(),
            prepared_focus_binding_retirement.testimony(),
            prepared_focus_retirement.testimony(),
            &delegation_plan,
        )?;
        let prepared_owner_unregistration = match &prepared_focus_retirement {
            PreparedNavigationFocusRetirement::RetiringOwnerOverlay(prepared) => self
                .overlay_domains
                .prepare_owner_unregistration_after_focus_retirement_guarded(&guard, prepared)?,
            PreparedNavigationFocusRetirement::Direct(_)
            | PreparedNavigationFocusRetirement::PeerOverlay(_) => self
                .overlay_domains
                .prepare_owner_unregistration(&guard, retirement_snapshot.overlay_owner_id())?,
        };
        let retiring_root_overlay_exit = match &prepared_focus_retirement {
            PreparedNavigationFocusRetirement::RetiringOwnerOverlay(prepared) => {
                Some(prepared.surface_exit_testimony())
            }
            PreparedNavigationFocusRetirement::Direct(_)
            | PreparedNavigationFocusRetirement::PeerOverlay(_) => None,
        };
        let peer_overlay_exit = match &prepared_focus_retirement {
            PreparedNavigationFocusRetirement::PeerOverlay(prepared) => {
                let (peer_root, _, expected_generation) = delegation_current
                    .expect("prepared peer overlay retirement has a delegated current root");
                Some(RootTeardownPeerOverlayExit {
                    root_id: peer_root,
                    expected_generation,
                    testimony: prepared.surface_exit_testimony(),
                })
            }
            PreparedNavigationFocusRetirement::Direct(_)
            | PreparedNavigationFocusRetirement::RetiringOwnerOverlay(_) => None,
        };
        let prepared_surface_teardown = registry.prepare_root_surface_teardown_with_overlay_exits(
            root_id,
            root_generation,
            compositor,
            retiring_root_overlay_exit,
            peer_overlay_exit,
            &guard,
        )?;

        projection_windows.sort_by_key(|window| window.window_id.0);
        projection_mutations.sort_by_key(|prepared| {
            let key = prepared.key();
            (key.window_id.0, key.node_id.0)
        });
        Ok(PreparedNavigationRootRetirement {
            application: self.clone(),
            root_id,
            projection_bindings: projection_bindings.clone(),
            projection_windows,
            projection_mutations,
            prepared_router_retirement: Some(prepared_router_retirement),
            focus_binding_port,
            prepared_focus_binding_retirement: Some(prepared_focus_binding_retirement),
            prepared_focus_retirement: Some(prepared_focus_retirement),
            prepared_owner_unregistration: Some(prepared_owner_unregistration),
            prepared_surface_teardown: Some(prepared_surface_teardown),
            guard: Some(guard),
            completed: false,
        })
    }

    pub fn prepare_domain_retirement<'registry, 'compositor, 'domains, 'windows>(
        &self,
        interaction_domains: &'domains mut InteractionDomains,
        registry: &'registry mut NavigationRootRegistry,
        compositor: &'compositor mut Compositor,
        projection_bindings: &FrameProjectionBindingRegistry,
        mut projection_windows: Vec<FrameProjectionWindowAccess<'windows>>,
        interaction_domain_id: InteractionDomainId,
    ) -> Result<
        PreparedNavigationDomainRetirement<'registry, 'compositor, 'domains, 'windows>,
        NavigationDomainRetirementError,
    > {
        if interaction_domains.state(interaction_domain_id).is_none() {
            return Err(NavigationDomainRetirementError::UnknownInteractionDomain(
                interaction_domain_id,
            ));
        }
        let guard = self
            .overlay_domains
            .begin_domain_transition(interaction_domain_id)?;
        let prepared_interaction_domain_unregistration = interaction_domains
            .prepare_domain_unregistration(interaction_domain_id)
            .ok_or(
                NavigationDomainRetirementError::InteractionDomainWindowMismatch(
                    interaction_domain_id,
                ),
            )?;
        let retirement_snapshot = self
            .navigation_domains
            .domain_retirement_snapshot_guarded(&guard)?;

        let mut surface_requests = Vec::with_capacity(retirement_snapshot.roots().len());
        for root_snapshot in retirement_snapshot.roots() {
            let root = registry.root(root_snapshot.root_id()).ok_or(
                NavigationDomainRetirementError::RegistryRootUnavailable(root_snapshot.root_id()),
            )?;
            if root.surface_for_node(root.current_node())
                != Some(root_snapshot.focus_binding().navigation_surface_id)
            {
                return Err(
                    NavigationDomainRetirementError::RegistryFocusSurfaceMismatch(
                        root_snapshot.root_id(),
                    ),
                );
            }
            surface_requests.push(DomainRootSurfaceTeardownRequest {
                root_id: root_snapshot.root_id(),
                expected_generation: root.orchestration_generation(),
            });
        }

        let mut window_ids = HashSet::new();
        for window in &projection_windows {
            if window.interaction_domain_id != interaction_domain_id {
                return Err(NavigationDomainRetirementError::Projection(
                    FrameProjectionBindingError::WindowDomainMismatch,
                ));
            }
            if !window_ids.insert(window.window_id) {
                return Err(NavigationDomainRetirementError::DuplicateProjectionWindow(
                    window.window_id,
                ));
            }
        }
        let mut consumers = projection_bindings
            .domain_consumers(&guard, interaction_domain_id)?
            .into_iter()
            .collect::<Vec<_>>();
        consumers.sort_by_key(|key| (key.window_id.0, key.node_id.0));
        let focus_resolver = FocusResolver::new();
        let authorities = FrameProjectionAuthorityAccess::new(
            interaction_domains,
            &self.overlay_domains,
            &focus_resolver,
        )
        .with_registered_roots(registry, &self.navigation_domains);
        let mut projection_mutations = Vec::with_capacity(consumers.len());
        for key in consumers {
            let window = projection_windows
                .iter()
                .find(|window| window.window_id == key.window_id)
                .ok_or(NavigationDomainRetirementError::MissingProjectionWindow(
                    key.window_id,
                ))?;
            let (_, epoch) = projection_bindings
                .guard_identity(key)
                .ok_or(FrameProjectionBindingError::UnknownBinding(key))?;
            projection_mutations.push(projection_bindings.prepare_remove_or_replace_guarded(
                authorities,
                window,
                key,
                epoch,
                None,
                &guard,
            )?);
        }

        let mut retiring_focus_domains = interaction_domains
            .state(interaction_domain_id)
            .expect("retiring interaction domain was validated")
            .focus_manager
            .focus_domains
            .keys()
            .copied()
            .collect::<Vec<_>>();
        retiring_focus_domains.sort_by_key(|focus_domain_id| focus_domain_id.0);
        let active_overlay = self.overlay_domains.active_binding_guarded(&guard)?;
        let prepared_focus_retirement = match active_overlay {
            Some(active) => {
                if !retirement_snapshot
                    .roots()
                    .iter()
                    .any(|root| root.overlay_owner_id() == active.owner_id)
                {
                    return Err(
                        NavigationDomainRetirementError::ActiveOverlayOwnerUnavailable(active),
                    );
                }
                PreparedNavigationDomainFocusRetirement::Overlay(
                    self.overlay_domains
                        .prepare_complete_domain_retirement_guarded(
                            &guard,
                            interaction_domains,
                            active.owner_id,
                            &retiring_focus_domains,
                        )?,
                )
            }
            None => PreparedNavigationDomainFocusRetirement::Direct(
                FocusResolver::new().prepare_complete_focus_domain_retirement(
                    guard.focus_capability(),
                    &interaction_domains
                        .state(interaction_domain_id)
                        .expect("retiring interaction domain was validated")
                        .focus_manager,
                    &retiring_focus_domains,
                    &[],
                )?,
            ),
        };

        let mut prepared_focus_bindings = Vec::with_capacity(retirement_snapshot.roots().len());
        for root_snapshot in retirement_snapshot.roots() {
            let port = root_snapshot.focus_binding_port().clone();
            let prepared = port
                .prepare_retire_for_domain_guarded(&guard, prepared_focus_retirement.testimony())?;
            prepared_focus_bindings.push((port, prepared));
        }
        let port_testimonies = prepared_focus_bindings
            .iter()
            .map(|(_, prepared)| prepared.testimony())
            .collect::<Vec<_>>();
        let prepared_router_retirement =
            self.navigation_domains.prepare_domain_retirement_guarded(
                &guard,
                retirement_snapshot.router_generation(),
                &port_testimonies,
                prepared_focus_retirement.testimony(),
            )?;

        let active_owner = prepared_focus_retirement.active_owner_id();
        let owner_ids = self.overlay_domains.domain_owner_ids_guarded(&guard)?;
        let mut prepared_owner_unregistrations = Vec::with_capacity(owner_ids.len());
        for owner_id in owner_ids {
            let prepared = if active_owner == Some(owner_id) {
                let PreparedNavigationDomainFocusRetirement::Overlay(focus_retirement) =
                    &prepared_focus_retirement
                else {
                    unreachable!("active domain owner requires compound overlay retirement");
                };
                self.overlay_domains
                    .prepare_owner_unregistration_after_complete_domain_retirement_guarded(
                        &guard,
                        focus_retirement,
                    )?
            } else {
                self.overlay_domains
                    .prepare_owner_unregistration(&guard, owner_id)?
            };
            prepared_owner_unregistrations.push(prepared);
        }
        let owner_unregistration_refs = prepared_owner_unregistrations.iter().collect::<Vec<_>>();
        let prepared_overlay_domain_unregistration = self
            .overlay_domains
            .prepare_domain_unregistration_after_owner_reservations(
                &guard,
                &owner_unregistration_refs,
                prepared_focus_retirement.overlay_retirement(),
            )?;

        let active_overlay_exit =
            if let PreparedNavigationDomainFocusRetirement::Overlay(prepared) =
                &prepared_focus_retirement
            {
                let root_id = retirement_snapshot
                    .roots()
                    .iter()
                    .find(|root| root.overlay_owner_id() == prepared.owner_id())
                    .expect("prepared active owner must retain its router root")
                    .root_id();
                Some(DomainSurfaceTeardownOverlayExit {
                    root_id,
                    testimony: prepared.surface_exit_testimony(),
                })
            } else {
                None
            };
        let prepared_surface_teardown = registry.prepare_domain_surface_teardown(
            surface_requests,
            compositor,
            active_overlay_exit,
            &guard,
        )?;

        projection_windows.sort_by_key(|window| window.window_id.0);
        projection_mutations.sort_by_key(|prepared| {
            let key = prepared.key();
            (key.window_id.0, key.node_id.0)
        });
        Ok(PreparedNavigationDomainRetirement {
            application: self.clone(),
            interaction_domain_id,
            interaction_domains,
            prepared_interaction_domain_unregistration: Some(
                prepared_interaction_domain_unregistration,
            ),
            projection_bindings: projection_bindings.clone(),
            projection_windows,
            projection_mutations,
            prepared_router_retirement: Some(prepared_router_retirement),
            prepared_focus_bindings,
            prepared_focus_retirement: Some(prepared_focus_retirement),
            prepared_owner_unregistrations,
            prepared_overlay_domain_unregistration: Some(prepared_overlay_domain_unregistration),
            prepared_surface_teardown: Some(prepared_surface_teardown),
            guard: Some(guard),
            completed: false,
        })
    }
}

enum PreparedNavigationContextFocus<'focus> {
    Activation(PreparedFocusDomainActivation<'focus>),
    OverlayExit(PreparedOverlayDeactivationToDomain<'focus>),
    RetainedDestinationOverlay,
}

pub struct PreparedNavigationDelegation<'compositor, 'focus> {
    application: NavigationApplicationCoordinator,
    router: Option<PreparedDelegationPush>,
    focus_transition: Option<PreparedNavigationContextFocus<'focus>>,
    surface_exit: Option<PreparedNavigationOverlaySurfaceExit<'compositor>>,
    guard: Option<DomainTransitionGuardToken>,
    completed: bool,
}

impl PreparedNavigationDelegation<'_, '_> {
    pub fn frame(&self) -> DelegationFrame {
        self.router
            .as_ref()
            .expect("prepared delegation router value is live")
            .frame()
    }

    pub fn commit(mut self) -> CommittedNavigationDelegation {
        let guard = self
            .guard
            .as_ref()
            .expect("prepared delegation guard is live");
        self.application
            .navigation_domains
            .begin_delegation_commit_guarded(
                guard,
                self.router
                    .as_mut()
                    .expect("prepared delegation router value is live"),
            );
        match self
            .focus_transition
            .take()
            .expect("prepared delegation focus result is live")
        {
            PreparedNavigationContextFocus::Activation(prepared) => {
                FocusResolver::new()
                    .commit_focus_domain_activation(guard.focus_capability(), prepared);
            }
            PreparedNavigationContextFocus::OverlayExit(prepared) => {
                self.application
                    .overlay_domains
                    .commit_deactivation_to_domain(guard, prepared);
            }
            PreparedNavigationContextFocus::RetainedDestinationOverlay => {}
        }
        let terminated_surfaces = self.surface_exit.take().map_or_else(Vec::new, |prepared| {
            let publication = SurfaceTransitionPublicationCapability::for_overlay_exit(
                prepared.publication_binding(),
            );
            prepared.commit_guarded(publication)
        });
        let frame = self
            .router
            .as_ref()
            .expect("prepared delegation router value is live")
            .frame();
        self.application
            .navigation_domains
            .finish_delegation_commit_guarded(
                guard,
                self.router
                    .take()
                    .expect("prepared delegation router value is live"),
            );
        self.completed = true;
        self.guard.take();
        CommittedNavigationDelegation {
            frame,
            terminated_surfaces,
        }
    }
}

impl Drop for PreparedNavigationDelegation<'_, '_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.router.take();
        self.surface_exit.take();
        self.focus_transition.take();
        self.guard.take();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedNavigationDelegation {
    frame: DelegationFrame,
    terminated_surfaces: Vec<(crate::compositor::SurfaceId, SurfaceTerminationReason)>,
}

impl CommittedNavigationDelegation {
    pub const fn frame(&self) -> DelegationFrame {
        self.frame
    }

    pub fn terminated_surfaces(
        &self,
    ) -> &[(crate::compositor::SurfaceId, SurfaceTerminationReason)] {
        &self.terminated_surfaces
    }
}

pub struct PreparedNavigationDelegationReturn<'compositor, 'focus> {
    application: NavigationApplicationCoordinator,
    router: Option<PreparedDelegationReturn>,
    focus_transition: Option<PreparedNavigationContextFocus<'focus>>,
    surface_exit: Option<PreparedNavigationOverlaySurfaceExit<'compositor>>,
    guard: Option<DomainTransitionGuardToken>,
    completed: bool,
}

impl PreparedNavigationDelegationReturn<'_, '_> {
    pub fn frame(&self) -> DelegationFrame {
        self.router
            .as_ref()
            .expect("prepared delegation-return router value is live")
            .frame()
    }

    pub fn commit(mut self) -> CommittedNavigationDelegationReturn {
        let guard = self
            .guard
            .as_ref()
            .expect("prepared delegation-return guard is live");
        self.application
            .navigation_domains
            .begin_delegation_return_commit_guarded(
                guard,
                self.router
                    .as_mut()
                    .expect("prepared delegation-return router value is live"),
            );
        match self
            .focus_transition
            .take()
            .expect("prepared delegation-return focus result is live")
        {
            PreparedNavigationContextFocus::Activation(prepared) => {
                FocusResolver::new()
                    .commit_focus_domain_activation(guard.focus_capability(), prepared);
            }
            PreparedNavigationContextFocus::OverlayExit(prepared) => {
                self.application
                    .overlay_domains
                    .commit_deactivation_to_domain(guard, prepared);
            }
            PreparedNavigationContextFocus::RetainedDestinationOverlay => {
                unreachable!("delegation return cannot retain a target overlay")
            }
        }
        let terminated_surfaces = self.surface_exit.take().map_or_else(Vec::new, |prepared| {
            let publication = SurfaceTransitionPublicationCapability::for_overlay_exit(
                prepared.publication_binding(),
            );
            prepared.commit_guarded(publication)
        });
        let frame = self
            .router
            .as_ref()
            .expect("prepared delegation-return router value is live")
            .frame();
        self.application
            .navigation_domains
            .finish_delegation_return_commit_guarded(
                guard,
                self.router
                    .take()
                    .expect("prepared delegation-return router value is live"),
            );
        self.completed = true;
        self.guard.take();
        CommittedNavigationDelegationReturn {
            frame,
            terminated_surfaces,
        }
    }
}

impl Drop for PreparedNavigationDelegationReturn<'_, '_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.router.take();
        self.surface_exit.take();
        self.focus_transition.take();
        self.guard.take();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedNavigationDelegationReturn {
    frame: DelegationFrame,
    terminated_surfaces: Vec<(crate::compositor::SurfaceId, SurfaceTerminationReason)>,
}

impl CommittedNavigationDelegationReturn {
    pub const fn frame(&self) -> DelegationFrame {
        self.frame
    }

    pub fn terminated_surfaces(
        &self,
    ) -> &[(crate::compositor::SurfaceId, SurfaceTerminationReason)] {
        &self.terminated_surfaces
    }
}

enum PreparedNavigationFocusRetirement<'focus> {
    Direct(PreparedFocusDomainRetirement<'focus>),
    RetiringOwnerOverlay(PreparedOverlayFocusGraphTransition<'focus>),
    PeerOverlay(PreparedOverlayFocusGraphTransition<'focus>),
}

impl PreparedNavigationFocusRetirement<'_> {
    fn testimony(&self) -> crate::core::focus_authority::FocusDomainGraphMutationTestimony<'_> {
        match self {
            Self::Direct(prepared) => prepared.testimony(),
            Self::RetiringOwnerOverlay(prepared) | Self::PeerOverlay(prepared) => {
                prepared.focus_graph_testimony()
            }
        }
    }
}

pub struct PreparedNavigationRootRetirement<'registry, 'compositor, 'focus, 'windows> {
    application: NavigationApplicationCoordinator,
    root_id: NavigationRootId,
    projection_bindings: FrameProjectionBindingRegistry,
    projection_windows: Vec<FrameProjectionWindowAccess<'windows>>,
    projection_mutations: Vec<PreparedFrameProjectionBindingMutation>,
    prepared_router_retirement: Option<PreparedRootRetirement>,
    focus_binding_port: NavigationRootFocusBindingPort,
    prepared_focus_binding_retirement: Option<PreparedRootFocusBindingRetirement>,
    prepared_focus_retirement: Option<PreparedNavigationFocusRetirement<'focus>>,
    prepared_owner_unregistration: Option<PreparedOverlayOwnerUnregistration>,
    prepared_surface_teardown: Option<PreparedRootSurfaceTeardown<'registry, 'compositor>>,
    guard: Option<DomainTransitionGuardToken>,
    completed: bool,
}

impl PreparedNavigationRootRetirement<'_, '_, '_, '_> {
    pub const fn root_id(&self) -> NavigationRootId {
        self.root_id
    }

    pub fn commit(mut self) -> CommittedNavigationRootRetirement {
        let guard = self
            .guard
            .as_ref()
            .expect("prepared root-retirement guard is live");
        let router_retirement = self
            .prepared_router_retirement
            .as_mut()
            .expect("prepared router retirement is live");
        let unwound_delegation_frames = router_retirement
            .unwound_delegation_frames_top_first()
            .to_vec();
        self.application
            .navigation_domains
            .begin_root_retirement_commit_guarded(guard, router_retirement);

        match self
            .prepared_focus_retirement
            .take()
            .expect("prepared focus retirement is live")
        {
            PreparedNavigationFocusRetirement::Direct(prepared) => {
                FocusResolver::new()
                    .commit_focus_domain_retirement(guard.focus_capability(), prepared);
            }
            PreparedNavigationFocusRetirement::RetiringOwnerOverlay(prepared)
            | PreparedNavigationFocusRetirement::PeerOverlay(prepared) => {
                self.application
                    .overlay_domains
                    .commit_deactivate_for_focus_retirement_guarded(guard, prepared);
            }
        }
        self.focus_binding_port.commit_retire_guarded(
            guard,
            self.prepared_focus_binding_retirement
                .take()
                .expect("prepared root focus-binding retirement is live"),
        );
        self.application
            .navigation_domains
            .apply_root_retirement_claims_guarded(guard, router_retirement);

        for prepared in self.projection_mutations.drain(..) {
            let key = prepared.key();
            let window = self
                .projection_windows
                .iter_mut()
                .find(|window| window.window_id == key.window_id)
                .expect("prepared projection consumer window must remain supplied");
            self.projection_bindings
                .commit_remove_or_replace_guarded(window, guard, prepared);
        }
        self.application
            .navigation_domains
            .apply_root_routing_retirement_guarded(guard, router_retirement);
        self.application
            .overlay_domains
            .commit_owner_unregistration(
                guard,
                self.prepared_owner_unregistration
                    .take()
                    .expect("prepared overlay owner unregistration is live"),
            );
        self.application
            .navigation_domains
            .remove_root_association_guarded(guard, router_retirement);

        let prepared_surface_teardown = self
            .prepared_surface_teardown
            .take()
            .expect("prepared root surface teardown is live");
        let teardown_capability =
            RootSurfaceTeardownPublicationCapability::new(&prepared_surface_teardown, Some(guard));
        let terminated_surfaces = prepared_surface_teardown.commit_guarded(teardown_capability);
        self.application
            .navigation_domains
            .finish_root_retirement_commit_guarded(
                guard,
                self.prepared_router_retirement
                    .take()
                    .expect("prepared router retirement is live"),
            );
        self.completed = true;
        self.guard.take();
        CommittedNavigationRootRetirement {
            root_id: self.root_id,
            terminated_surfaces,
            unwound_delegation_frames,
        }
    }
}

impl Drop for PreparedNavigationRootRetirement<'_, '_, '_, '_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.prepared_router_retirement.take();
        self.prepared_focus_binding_retirement.take();
        self.prepared_owner_unregistration.take();
        self.prepared_focus_retirement.take();
        self.prepared_surface_teardown.take();
        self.projection_mutations.clear();
        self.guard.take();
    }
}

enum PreparedNavigationDomainFocusRetirement {
    Direct(PreparedCompleteFocusDomainRetirement),
    Overlay(PreparedOverlayCompleteDomainRetirement),
}

impl PreparedNavigationDomainFocusRetirement {
    fn testimony(&self) -> FocusDomainGraphMutationTestimony<'_> {
        match self {
            Self::Direct(prepared) => prepared.testimony(),
            Self::Overlay(prepared) => prepared.focus_graph_testimony(),
        }
    }

    fn active_owner_id(&self) -> Option<crate::core::overlay_domain_coordinator::OverlayOwnerId> {
        match self {
            Self::Direct(_) => None,
            Self::Overlay(prepared) => Some(prepared.owner_id()),
        }
    }

    fn overlay_retirement(&self) -> Option<&PreparedOverlayCompleteDomainRetirement> {
        match self {
            Self::Direct(_) => None,
            Self::Overlay(prepared) => Some(prepared),
        }
    }
}

pub struct PreparedNavigationDomainRetirement<'registry, 'compositor, 'domains, 'windows> {
    application: NavigationApplicationCoordinator,
    interaction_domain_id: InteractionDomainId,
    interaction_domains: &'domains mut InteractionDomains,
    prepared_interaction_domain_unregistration: Option<PreparedInteractionDomainUnregistration>,
    projection_bindings: FrameProjectionBindingRegistry,
    projection_windows: Vec<FrameProjectionWindowAccess<'windows>>,
    projection_mutations: Vec<PreparedFrameProjectionBindingMutation>,
    prepared_router_retirement: Option<PreparedNavigationDomainRouterRetirement>,
    prepared_focus_bindings: Vec<(
        NavigationRootFocusBindingPort,
        PreparedRootFocusBindingRetirement,
    )>,
    prepared_focus_retirement: Option<PreparedNavigationDomainFocusRetirement>,
    prepared_owner_unregistrations: Vec<PreparedOverlayOwnerUnregistration>,
    prepared_overlay_domain_unregistration: Option<PreparedOverlayDomainUnregistration>,
    prepared_surface_teardown: Option<PreparedDomainSurfaceTeardown<'registry, 'compositor>>,
    guard: Option<DomainTransitionGuardToken>,
    completed: bool,
}

impl PreparedNavigationDomainRetirement<'_, '_, '_, '_> {
    pub const fn interaction_domain_id(&self) -> InteractionDomainId {
        self.interaction_domain_id
    }

    pub fn commit(mut self) -> CommittedNavigationDomainRetirement {
        let guard = self
            .guard
            .as_ref()
            .expect("prepared domain-retirement guard is live");
        let router_retirement = self
            .prepared_router_retirement
            .as_mut()
            .expect("prepared router domain retirement is live");
        let root_ids = self
            .prepared_focus_bindings
            .iter()
            .map(|(port, _)| port.root_id())
            .collect::<Vec<_>>();
        let unwound_delegation_frames = router_retirement.unwound_delegation_frames_top_first();
        self.application
            .navigation_domains
            .begin_domain_retirement_commit_guarded(guard, router_retirement);

        match self
            .prepared_focus_retirement
            .take()
            .expect("prepared complete focus retirement is live")
        {
            PreparedNavigationDomainFocusRetirement::Direct(prepared) => {
                let manager = &mut self
                    .interaction_domains
                    .state_mut(self.interaction_domain_id)
                    .expect("prepared interaction domain must remain registered")
                    .focus_manager;
                FocusResolver::new().commit_complete_focus_domain_retirement(
                    guard.focus_capability(),
                    manager,
                    prepared,
                );
            }
            PreparedNavigationDomainFocusRetirement::Overlay(prepared) => {
                self.application
                    .overlay_domains
                    .commit_complete_domain_retirement_guarded(
                        guard,
                        self.interaction_domains,
                        prepared,
                    );
            }
        }
        for (port, prepared) in self.prepared_focus_bindings.drain(..) {
            port.commit_retire_guarded(guard, prepared);
        }
        self.application
            .navigation_domains
            .apply_domain_retirement_claims_guarded(guard, router_retirement);

        for prepared in self.projection_mutations.drain(..) {
            let key = prepared.key();
            let window = self
                .projection_windows
                .iter_mut()
                .find(|window| window.window_id == key.window_id)
                .expect("prepared domain projection window must remain supplied");
            self.projection_bindings
                .commit_remove_or_replace_guarded(window, guard, prepared);
        }
        self.application
            .navigation_domains
            .apply_domain_routing_retirement_guarded(guard, router_retirement);
        for prepared in self.prepared_owner_unregistrations.drain(..) {
            self.application
                .overlay_domains
                .commit_owner_unregistration(guard, prepared);
        }
        self.application
            .navigation_domains
            .remove_domain_root_associations_guarded(guard, router_retirement);
        let terminated_surfaces = self
            .prepared_surface_teardown
            .take()
            .expect("prepared domain surface teardown is live")
            .commit_guarded(guard);
        self.application
            .navigation_domains
            .finish_domain_retirement_commit_guarded(
                guard,
                self.prepared_router_retirement
                    .take()
                    .expect("prepared router domain retirement is live"),
            );
        self.application
            .overlay_domains
            .commit_domain_unregistration(
                guard,
                self.prepared_overlay_domain_unregistration
                    .take()
                    .expect("prepared overlay domain unregistration is live"),
            );
        self.interaction_domains.commit_domain_unregistration(
            self.prepared_interaction_domain_unregistration
                .take()
                .expect("prepared interaction-domain unregistration is live"),
        );
        self.completed = true;
        self.guard.take();
        CommittedNavigationDomainRetirement {
            interaction_domain_id: self.interaction_domain_id,
            root_ids,
            terminated_surfaces,
            unwound_delegation_frames,
        }
    }
}

impl Drop for PreparedNavigationDomainRetirement<'_, '_, '_, '_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.prepared_router_retirement.take();
        self.prepared_focus_bindings.clear();
        self.prepared_owner_unregistrations.clear();
        self.prepared_overlay_domain_unregistration.take();
        self.prepared_focus_retirement.take();
        self.prepared_surface_teardown.take();
        self.projection_mutations.clear();
        self.guard.take();
    }
}

#[derive(Debug, Clone)]
pub struct CommittedNavigationDomainRetirement {
    pub interaction_domain_id: InteractionDomainId,
    pub root_ids: Vec<NavigationRootId>,
    pub terminated_surfaces: Vec<(crate::compositor::SurfaceId, SurfaceTerminationReason)>,
    pub unwound_delegation_frames: Vec<DelegationFrame>,
}

#[derive(Debug, Clone)]
pub struct CommittedNavigationRootRetirement {
    pub root_id: NavigationRootId,
    pub terminated_surfaces: Vec<(crate::compositor::SurfaceId, SurfaceTerminationReason)>,
    pub unwound_delegation_frames: Vec<DelegationFrame>,
}

pub struct PreparedStandaloneNavigationTransaction<'compositor> {
    surface_transition: Option<PreparedNavigationSurfaceTransition<'compositor>>,
    host_commit: Option<PreparedNavigationCommit>,
    guard: Option<DomainTransitionGuardToken>,
    completed: bool,
}

impl PreparedStandaloneNavigationTransaction<'_> {
    pub fn staged_surfaces(&self) -> &[crate::compositor::SurfaceId] {
        self.surface_transition
            .as_ref()
            .expect("prepared standalone surface transition is live")
            .staged_surfaces()
    }

    pub fn next_active_surface(&self) -> crate::compositor::SurfaceId {
        self.surface_transition
            .as_ref()
            .expect("prepared standalone surface transition is live")
            .next_active_surface()
    }

    pub fn commit(mut self) -> CommittedStandaloneNavigation {
        let guard = self
            .guard
            .as_ref()
            .expect("prepared standalone-navigation guard is live");
        let outcome = self
            .host_commit
            .take()
            .expect("prepared standalone host commit is live")
            .commit();
        let surface_transition = self
            .surface_transition
            .take()
            .expect("prepared standalone surface transition is live");
        let publication =
            SurfaceTransitionPublicationCapability::new(&surface_transition, Some(guard));
        let surface_transition = surface_transition.commit_guarded(publication);
        self.completed = true;
        self.guard.take();
        CommittedStandaloneNavigation {
            outcome,
            surface_transition,
        }
    }
}

impl Drop for PreparedStandaloneNavigationTransaction<'_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.surface_transition.take();
        self.host_commit.take();
        self.guard.take();
    }
}

#[derive(Debug, Clone)]
pub struct CommittedStandaloneNavigation {
    pub outcome: NavigationOutcome,
    surface_transition: CommittedNavigationSurfaceTransition,
}

impl CommittedStandaloneNavigation {
    pub fn active_surface(&self) -> Option<crate::compositor::SurfaceId> {
        self.surface_transition.active_surface
    }

    pub fn terminated_surfaces(
        &self,
    ) -> &[(
        crate::compositor::SurfaceId,
        crate::core::navigation_orchestrator::SurfaceTerminationReason,
    )] {
        &self.surface_transition.terminated_surfaces
    }
}

pub struct PreparedRoutedNavigationTransaction<'compositor, 'focus> {
    application: NavigationApplicationCoordinator,
    router_commit: Option<PreparedRoutedNavigationRouterCommit>,
    focus_activation: Option<PreparedFocusDomainActivation<'focus>>,
    focus_binding_port: NavigationRootFocusBindingPort,
    binding_replacement: Option<PreparedRootFocusBindingReplacement>,
    surface_transition: Option<PreparedNavigationSurfaceTransition<'compositor>>,
    host_commit: Option<PreparedNavigationCommit>,
    guard: Option<DomainTransitionGuardToken>,
    completed: bool,
}

impl PreparedRoutedNavigationTransaction<'_, '_> {
    pub fn staged_surfaces(&self) -> &[crate::compositor::SurfaceId] {
        self.surface_transition
            .as_ref()
            .expect("prepared routed surface transition is live")
            .staged_surfaces()
    }

    pub fn next_active_surface(&self) -> crate::compositor::SurfaceId {
        self.surface_transition
            .as_ref()
            .expect("prepared routed surface transition is live")
            .next_active_surface()
    }

    pub fn commit(mut self) -> CommittedRoutedNavigation {
        let guard = self
            .guard
            .as_ref()
            .expect("prepared routed-navigation guard is live");
        self.application
            .navigation_domains
            .begin_routed_navigation_commit_guarded(
                guard,
                self.router_commit
                    .as_mut()
                    .expect("prepared routed router commit is live"),
            );
        FocusResolver::new().commit_focus_domain_activation(
            guard.focus_capability(),
            self.focus_activation
                .take()
                .expect("prepared routed focus activation is live"),
        );
        self.focus_binding_port.commit_replace_guarded(
            guard,
            self.binding_replacement
                .take()
                .expect("prepared routed focus binding replacement is live"),
        );
        let outcome = self
            .host_commit
            .take()
            .expect("prepared routed host commit is live")
            .commit();
        let surface_transition = self
            .surface_transition
            .take()
            .expect("prepared routed surface transition is live");
        let publication =
            SurfaceTransitionPublicationCapability::new(&surface_transition, Some(guard));
        let surface_transition = surface_transition.commit_guarded(publication);
        self.application
            .navigation_domains
            .finish_routed_navigation_commit_guarded(
                guard,
                self.router_commit
                    .take()
                    .expect("prepared routed router commit is live"),
            );
        self.completed = true;
        self.guard.take();
        CommittedRoutedNavigation {
            outcome,
            surface_transition,
        }
    }
}

impl Drop for PreparedRoutedNavigationTransaction<'_, '_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.router_commit.take();
        self.focus_activation.take();
        self.binding_replacement.take();
        self.surface_transition.take();
        self.host_commit.take();
        self.guard.take();
    }
}

pub struct PreparedRoutedNavigationOverlayExitTransaction<'compositor, 'focus> {
    application: NavigationApplicationCoordinator,
    router_commit: Option<PreparedRoutedNavigationRouterCommit>,
    overlay_exit: Option<PreparedOverlayDeactivationToDomain<'focus>>,
    focus_binding_port: NavigationRootFocusBindingPort,
    binding_replacement: Option<PreparedRootFocusBindingReplacement>,
    surface_transition: Option<PreparedNavigationSurfaceTransition<'compositor>>,
    host_commit: Option<PreparedNavigationCommit>,
    guard: Option<DomainTransitionGuardToken>,
    completed: bool,
}

impl PreparedRoutedNavigationOverlayExitTransaction<'_, '_> {
    pub fn staged_surfaces(&self) -> &[crate::compositor::SurfaceId] {
        self.surface_transition
            .as_ref()
            .expect("prepared routed overlay surface transition is live")
            .staged_surfaces()
    }

    pub fn next_active_surface(&self) -> crate::compositor::SurfaceId {
        self.surface_transition
            .as_ref()
            .expect("prepared routed overlay surface transition is live")
            .next_active_surface()
    }

    pub fn commit(mut self) -> CommittedRoutedNavigation {
        let guard = self
            .guard
            .as_ref()
            .expect("prepared routed overlay guard is live");
        self.application
            .navigation_domains
            .begin_routed_navigation_commit_guarded(
                guard,
                self.router_commit
                    .as_mut()
                    .expect("prepared routed overlay router commit is live"),
            );
        self.application
            .overlay_domains
            .commit_deactivation_to_domain(
                guard,
                self.overlay_exit
                    .take()
                    .expect("prepared routed overlay exit is live"),
            );
        self.focus_binding_port.commit_replace_guarded(
            guard,
            self.binding_replacement
                .take()
                .expect("prepared routed overlay focus binding replacement is live"),
        );
        let outcome = self
            .host_commit
            .take()
            .expect("prepared routed overlay host commit is live")
            .commit();
        let surface_transition = self
            .surface_transition
            .take()
            .expect("prepared routed overlay surface transition is live");
        let publication =
            SurfaceTransitionPublicationCapability::new(&surface_transition, Some(guard));
        let surface_transition = surface_transition.commit_guarded(publication);
        self.application
            .navigation_domains
            .finish_routed_navigation_commit_guarded(
                guard,
                self.router_commit
                    .take()
                    .expect("prepared routed overlay router commit is live"),
            );
        self.completed = true;
        self.guard.take();
        CommittedRoutedNavigation {
            outcome,
            surface_transition,
        }
    }
}

impl Drop for PreparedRoutedNavigationOverlayExitTransaction<'_, '_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.router_commit.take();
        self.binding_replacement.take();
        self.surface_transition.take();
        self.host_commit.take();
        self.overlay_exit.take();
        self.guard.take();
    }
}

pub struct PreparedRoutedNavigationOverlayFocusGraphTransaction<'compositor, 'focus> {
    application: NavigationApplicationCoordinator,
    router_commit: Option<PreparedRootFocusMembershipChange>,
    overlay_graph: Option<PreparedOverlayFocusGraphTransition<'focus>>,
    focus_binding_port: NavigationRootFocusBindingPort,
    binding_change: Option<PreparedRootOwnedFocusDomainChange>,
    surface_transition: Option<PreparedNavigationSurfaceTransition<'compositor>>,
    host_commit: Option<PreparedNavigationCommit>,
    guard: Option<DomainTransitionGuardToken>,
    completed: bool,
}

impl PreparedRoutedNavigationOverlayFocusGraphTransaction<'_, '_> {
    pub fn staged_surfaces(&self) -> &[crate::compositor::SurfaceId] {
        self.surface_transition
            .as_ref()
            .expect("prepared routed overlay graph surface transition is live")
            .staged_surfaces()
    }

    pub fn next_active_surface(&self) -> crate::compositor::SurfaceId {
        self.surface_transition
            .as_ref()
            .expect("prepared routed overlay graph surface transition is live")
            .next_active_surface()
    }

    pub fn commit(mut self) -> CommittedRoutedNavigation {
        let guard = self
            .guard
            .as_ref()
            .expect("prepared routed overlay graph guard is live");
        self.application
            .navigation_domains
            .begin_root_focus_membership_commit_guarded(
                guard,
                self.router_commit
                    .as_mut()
                    .expect("prepared routed overlay graph router commit is live"),
            );
        self.application
            .overlay_domains
            .commit_deactivate_for_focus_graph_mutation_guarded(
                guard,
                self.overlay_graph
                    .take()
                    .expect("prepared routed overlay focus graph is live"),
            );
        self.focus_binding_port.commit_owned_domain_change_guarded(
            guard,
            self.binding_change
                .take()
                .expect("prepared routed overlay binding membership change is live"),
        );
        self.application
            .navigation_domains
            .apply_root_focus_membership_commit_guarded(
                guard,
                self.router_commit
                    .as_mut()
                    .expect("prepared routed overlay graph router commit is live"),
            );
        let outcome = self
            .host_commit
            .take()
            .expect("prepared routed overlay graph host commit is live")
            .commit();
        let surface_transition = self
            .surface_transition
            .take()
            .expect("prepared routed overlay graph surface transition is live");
        let publication =
            SurfaceTransitionPublicationCapability::new(&surface_transition, Some(guard));
        let surface_transition = surface_transition.commit_guarded(publication);
        self.application
            .navigation_domains
            .finish_root_focus_membership_commit_guarded(
                guard,
                self.router_commit
                    .take()
                    .expect("prepared routed overlay graph router commit is live"),
            );
        self.completed = true;
        self.guard.take();
        CommittedRoutedNavigation {
            outcome,
            surface_transition,
        }
    }
}

impl Drop for PreparedRoutedNavigationOverlayFocusGraphTransaction<'_, '_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.router_commit.take();
        self.binding_change.take();
        self.surface_transition.take();
        self.host_commit.take();
        self.overlay_graph.take();
        self.guard.take();
    }
}

#[derive(Debug, Clone)]
pub struct CommittedRoutedNavigation {
    pub outcome: NavigationOutcome,
    surface_transition: CommittedNavigationSurfaceTransition,
}

impl CommittedRoutedNavigation {
    pub fn active_surface(&self) -> Option<crate::compositor::SurfaceId> {
        self.surface_transition.active_surface
    }

    pub fn terminated_surfaces(
        &self,
    ) -> &[(
        crate::compositor::SurfaceId,
        crate::core::navigation_orchestrator::SurfaceTerminationReason,
    )] {
        &self.surface_transition.terminated_surfaces
    }
}

pub struct PreparedRoutedNavigationFocusGraphTransaction<'compositor, 'focus> {
    application: NavigationApplicationCoordinator,
    router_commit: Option<PreparedRootFocusMembershipChange>,
    focus_graph: Option<PreparedFocusDomainGraphMutation<'focus>>,
    focus_binding_port: NavigationRootFocusBindingPort,
    binding_change: Option<PreparedRootOwnedFocusDomainChange>,
    surface_transition: Option<PreparedNavigationSurfaceTransition<'compositor>>,
    host_commit: Option<PreparedNavigationCommit>,
    guard: Option<DomainTransitionGuardToken>,
    completed: bool,
}

impl PreparedRoutedNavigationFocusGraphTransaction<'_, '_> {
    pub fn staged_surfaces(&self) -> &[crate::compositor::SurfaceId] {
        self.surface_transition
            .as_ref()
            .expect("prepared routed graph surface transition is live")
            .staged_surfaces()
    }

    pub fn next_active_surface(&self) -> crate::compositor::SurfaceId {
        self.surface_transition
            .as_ref()
            .expect("prepared routed graph surface transition is live")
            .next_active_surface()
    }

    pub fn commit(mut self) -> CommittedRoutedNavigation {
        let guard = self
            .guard
            .as_ref()
            .expect("prepared routed graph guard is live");
        self.application
            .navigation_domains
            .begin_root_focus_membership_commit_guarded(
                guard,
                self.router_commit
                    .as_mut()
                    .expect("prepared routed graph router commit is live"),
            );
        FocusResolver::new().commit_focus_domain_graph_mutation(
            guard.focus_capability(),
            self.focus_graph
                .take()
                .expect("prepared routed focus graph is live"),
        );
        self.focus_binding_port.commit_owned_domain_change_guarded(
            guard,
            self.binding_change
                .take()
                .expect("prepared routed binding membership change is live"),
        );
        self.application
            .navigation_domains
            .apply_root_focus_membership_commit_guarded(
                guard,
                self.router_commit
                    .as_mut()
                    .expect("prepared routed graph router commit is live"),
            );
        let outcome = self
            .host_commit
            .take()
            .expect("prepared routed graph host commit is live")
            .commit();
        let surface_transition = self
            .surface_transition
            .take()
            .expect("prepared routed graph surface transition is live");
        let publication =
            SurfaceTransitionPublicationCapability::new(&surface_transition, Some(guard));
        let surface_transition = surface_transition.commit_guarded(publication);
        self.application
            .navigation_domains
            .finish_root_focus_membership_commit_guarded(
                guard,
                self.router_commit
                    .take()
                    .expect("prepared routed graph router commit is live"),
            );
        self.completed = true;
        self.guard.take();
        CommittedRoutedNavigation {
            outcome,
            surface_transition,
        }
    }
}

impl Drop for PreparedRoutedNavigationFocusGraphTransaction<'_, '_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.router_commit.take();
        self.focus_graph.take();
        self.binding_change.take();
        self.surface_transition.take();
        self.host_commit.take();
        self.guard.take();
    }
}

pub struct PreparedNavigationRootAdmission<'registry, 'compositor, 'focus> {
    application: NavigationApplicationCoordinator,
    prepared_router_association: Option<PreparedRootAssociation>,
    prepared_focus_binding: Option<PreparedRootFocusBindingPortAdmission>,
    prepared_focus_graph: Option<PreparedFocusDomainGraphMutation<'focus>>,
    prepared_owner: Option<PreparedOverlayOwnerRegistration>,
    prepared_root: Option<PreparedRegistryRoot<'registry, 'compositor>>,
    guard: Option<DomainTransitionGuardToken>,
    completed: bool,
}

impl PreparedNavigationRootAdmission<'_, '_, '_> {
    pub fn root_id(&self) -> NavigationRootId {
        self.prepared_root
            .as_ref()
            .expect("prepared root admission is live")
            .root_id()
    }

    pub fn prepared_surface(&self) -> crate::compositor::SurfaceId {
        self.prepared_root
            .as_ref()
            .expect("prepared root admission is live")
            .current_surface()
    }

    pub fn prepared_surface_visibility(&self) -> Option<crate::compositor::SurfaceVisibility> {
        self.prepared_root
            .as_ref()
            .expect("prepared root admission is live")
            .current_surface_visibility()
    }

    pub fn commit(mut self) -> CommittedNavigationRootAdmission {
        let guard = self
            .guard
            .as_ref()
            .expect("prepared root admission guard is live");
        let association = self
            .prepared_router_association
            .as_mut()
            .expect("prepared router association is live");
        self.application
            .navigation_domains
            .begin_root_association_commit_guarded(guard, association);

        self.application.overlay_domains.commit_owner_registration(
            self.prepared_owner
                .take()
                .expect("prepared owner registration is live"),
        );
        FocusResolver::new().commit_focus_domain_graph_mutation(
            guard.focus_capability(),
            self.prepared_focus_graph
                .take()
                .expect("prepared focus graph is live"),
        );
        let focus_binding = NavigationRootFocusBindingPort::commit_create_guarded(
            guard,
            self.prepared_focus_binding
                .take()
                .expect("prepared focus binding is live"),
        );
        self.application
            .navigation_domains
            .apply_root_association_commit_guarded(guard, association, focus_binding.clone());

        let prepared_root = self
            .prepared_root
            .take()
            .expect("prepared registry root is live");
        let root_capability =
            RootAdmissionPublicationCapability::new(prepared_root.publication_binding());
        let root_id = prepared_root.commit_guarded(root_capability);
        self.application
            .navigation_domains
            .finish_root_association_commit_guarded(
                guard,
                self.prepared_router_association
                    .take()
                    .expect("prepared router association is live"),
            );
        self.completed = true;
        self.guard.take();
        CommittedNavigationRootAdmission {
            root_id,
            focus_binding,
        }
    }
}

impl Drop for PreparedNavigationRootAdmission<'_, '_, '_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.prepared_router_association.take();
        self.prepared_focus_binding.take();
        self.prepared_focus_graph.take();
        self.prepared_owner.take();
        self.prepared_root.take();
        self.guard.take();
    }
}

#[derive(Debug, Clone)]
pub struct CommittedNavigationRootAdmission {
    pub root_id: NavigationRootId,
    pub focus_binding: NavigationRootFocusBindingPort,
}
