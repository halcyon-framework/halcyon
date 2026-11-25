use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::compositor::{
    Compositor, CompositorIdentity, FrameProjectionError, NavigationFrameProjection,
    NavigationProjectionContext,
};
use crate::core::event::{InteractionDomainId, InteractionDomains, WindowId};
use crate::core::focus_authority::{FocusAuthorityError, FocusResolver};
use crate::core::navigation_domain_router::{NavigationDomainRouter, NavigationDomainRouterError};
use crate::core::navigation_orchestrator::{
    FrameProjectionParticipationError, NavigationProjectionError, NavigationRootId,
    NavigationRootRegistry, NavigationSurfaceOrchestratorHandle,
    PreparedFrameProjectionParticipation,
};
use crate::core::overlay_domain_coordinator::{
    DomainTransitionGuardToken, OverlayDomainCoordinator, OverlayDomainError,
};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::widgets::screen_host::ScreenHost;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameProjectionEpoch(u64);

impl FrameProjectionEpoch {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScreenHostProjectionKey {
    pub window_id: WindowId,
    pub node_id: NodeId,
}

impl ScreenHostProjectionKey {
    pub const fn new(window_id: WindowId, node_id: NodeId) -> Self {
        Self { window_id, node_id }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationProjectionSource {
    StandaloneOrchestrator {
        bound_domain_id: InteractionDomainId,
        orchestrator: NavigationSurfaceOrchestratorHandle,
    },
    RegisteredRoot(NavigationRootId),
}

impl NavigationProjectionSource {
    pub fn standalone(
        bound_domain_id: InteractionDomainId,
        orchestrator: NavigationSurfaceOrchestratorHandle,
    ) -> Self {
        Self::StandaloneOrchestrator {
            bound_domain_id,
            orchestrator,
        }
    }

    pub const fn registered_root(root_id: NavigationRootId) -> Self {
        Self::RegisteredRoot(root_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameProjectionBinding {
    interaction_domain_id: InteractionDomainId,
    navigation_sources: Vec<NavigationProjectionSource>,
}

impl FrameProjectionBinding {
    pub fn new(
        interaction_domain_id: InteractionDomainId,
        navigation_sources: Vec<NavigationProjectionSource>,
    ) -> Self {
        Self {
            interaction_domain_id,
            navigation_sources,
        }
    }

    pub const fn interaction_domain_id(&self) -> InteractionDomainId {
        self.interaction_domain_id
    }

    pub fn navigation_sources(&self) -> &[NavigationProjectionSource] {
        &self.navigation_sources
    }

    pub fn references_root(&self, root_id: NavigationRootId) -> bool {
        self.navigation_sources.iter().any(
            |source| matches!(source, NavigationProjectionSource::RegisteredRoot(id) if *id == root_id),
        )
    }

    fn registered_roots(&self) -> HashSet<NavigationRootId> {
        self.navigation_sources
            .iter()
            .filter_map(|source| match source {
                NavigationProjectionSource::RegisteredRoot(root_id) => Some(*root_id),
                NavigationProjectionSource::StandaloneOrchestrator { .. } => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedFrameProjectionBinding {
    epoch: FrameProjectionEpoch,
    binding: FrameProjectionBinding,
    participants: Vec<NavigationSurfaceOrchestratorHandle>,
}

impl VersionedFrameProjectionBinding {
    pub const fn epoch(&self) -> FrameProjectionEpoch {
        self.epoch
    }

    pub const fn binding(&self) -> &FrameProjectionBinding {
        &self.binding
    }
}

#[derive(Clone, Copy)]
pub struct FrameProjectionAuthorityAccess<'a> {
    interaction_domains: &'a InteractionDomains,
    navigation_root_registry: Option<&'a NavigationRootRegistry>,
    navigation_domain_router: Option<&'a NavigationDomainRouter>,
    overlay_domain_coordinator: &'a OverlayDomainCoordinator,
    focus_resolver: &'a FocusResolver,
}

impl<'a> FrameProjectionAuthorityAccess<'a> {
    pub const fn new(
        interaction_domains: &'a InteractionDomains,
        overlay_domain_coordinator: &'a OverlayDomainCoordinator,
        focus_resolver: &'a FocusResolver,
    ) -> Self {
        Self {
            interaction_domains,
            navigation_root_registry: None,
            navigation_domain_router: None,
            overlay_domain_coordinator,
            focus_resolver,
        }
    }

    pub const fn with_registered_roots(
        mut self,
        navigation_root_registry: &'a NavigationRootRegistry,
        navigation_domain_router: &'a NavigationDomainRouter,
    ) -> Self {
        self.navigation_root_registry = Some(navigation_root_registry);
        self.navigation_domain_router = Some(navigation_domain_router);
        self
    }
}

pub struct FrameProjectionWindowAccess<'a> {
    pub window_id: WindowId,
    pub interaction_domain_id: InteractionDomainId,
    compositor_identity: CompositorIdentity,
    pub widget_tree: &'a mut WidgetTree,
}

impl<'a> FrameProjectionWindowAccess<'a> {
    pub fn new(
        window_id: WindowId,
        interaction_domain_id: InteractionDomainId,
        compositor: &Compositor,
        widget_tree: &'a mut WidgetTree,
    ) -> Self {
        Self {
            window_id,
            interaction_domain_id,
            compositor_identity: compositor.identity(),
            widget_tree,
        }
    }
}

pub struct FrameProjectionRuntimeAccess<'a> {
    pub bindings: &'a mut FrameProjectionBindingRegistry,
    pub authorities: FrameProjectionAuthorityAccess<'a>,
    pub compositor: &'a Compositor,
    pub window: FrameProjectionWindowAccess<'a>,
}

impl<'a> FrameProjectionRuntimeAccess<'a> {
    pub fn new(
        bindings: &'a mut FrameProjectionBindingRegistry,
        authorities: FrameProjectionAuthorityAccess<'a>,
        compositor: &'a Compositor,
        window: FrameProjectionWindowAccess<'a>,
    ) -> Self {
        Self {
            bindings,
            authorities,
            compositor,
            window,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameProjectionPullOutcome {
    NoBinding,
    Published { epoch: FrameProjectionEpoch },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameProjectionPullError {
    RetryNextFrame,
    Binding(FrameProjectionBindingError),
    Overlay(OverlayDomainError),
    Focus(FocusAuthorityError),
    Navigation(NavigationProjectionError),
    RegisteredRootFocusMismatch {
        root_id: NavigationRootId,
        projected: Option<crate::compositor::SurfaceId>,
        bound: crate::compositor::SurfaceId,
    },
    Compositor(FrameProjectionError),
}

impl From<FrameProjectionBindingError> for FrameProjectionPullError {
    fn from(value: FrameProjectionBindingError) -> Self {
        Self::Binding(value)
    }
}

impl From<FocusAuthorityError> for FrameProjectionPullError {
    fn from(value: FocusAuthorityError) -> Self {
        Self::Focus(value)
    }
}

impl From<NavigationProjectionError> for FrameProjectionPullError {
    fn from(value: NavigationProjectionError) -> Self {
        Self::Navigation(value)
    }
}

impl From<FrameProjectionError> for FrameProjectionPullError {
    fn from(value: FrameProjectionError) -> Self {
        Self::Compositor(value)
    }
}

pub fn pull_navigation_frame_projection(
    runtime: FrameProjectionRuntimeAccess<'_>,
    key: ScreenHostProjectionKey,
) -> Result<FrameProjectionPullOutcome, FrameProjectionPullError> {
    let bootstrap = runtime.bindings.guard_identity(key);
    pull_navigation_frame_projection_with_bootstrap(runtime, key, bootstrap)
}

pub(crate) fn pull_navigation_frame_projection_with_bootstrap(
    mut runtime: FrameProjectionRuntimeAccess<'_>,
    key: ScreenHostProjectionKey,
    bootstrap: Option<(InteractionDomainId, FrameProjectionEpoch)>,
) -> Result<FrameProjectionPullOutcome, FrameProjectionPullError> {
    let Some((domain_id, epoch_hint)) = bootstrap else {
        return Ok(FrameProjectionPullOutcome::NoBinding);
    };
    let guard = runtime
        .authorities
        .overlay_domain_coordinator
        .begin_domain_transition(domain_id)
        .map_err(FrameProjectionPullError::Overlay)?;
    let versioned = runtime.bindings.binding_guarded(&guard, key)?;
    let Some(versioned) = versioned else {
        return Err(FrameProjectionPullError::RetryNextFrame);
    };
    if versioned.epoch != epoch_hint || versioned.binding.interaction_domain_id != domain_id {
        return Err(FrameProjectionPullError::RetryNextFrame);
    }

    runtime
        .bindings
        .validate_window_identity(&runtime.window, key, domain_id)?;
    match runtime
        .authorities
        .interaction_domains
        .domain_for_window(runtime.window.window_id)
    {
        Some(actual) if actual == domain_id => {}
        Some(_) => {
            return Err(FrameProjectionBindingError::WindowDomainMismatch.into());
        }
        None => {
            return Err(
                FrameProjectionBindingError::UnknownWindowDomain(runtime.window.window_id).into(),
            );
        }
    }
    if let Err(error) = FrameProjectionBindingRegistry::validate_screen_host(&runtime.window, key) {
        if matches!(
            error,
            FrameProjectionBindingError::MissingScreenHost(_)
                | FrameProjectionBindingError::WrongScreenHost(_)
        ) {
            let error = runtime
                .bindings
                .invalidate_missing_screen_host(key, &guard)
                .expect_err("orphan invalidation reports the repaired lifecycle breach");
            return Err(error.into());
        }
        return Err(error.into());
    }

    let resolved = (|| {
        let binding = versioned.binding();
        let mut navigation_contexts = Vec::with_capacity(binding.navigation_sources.len());
        for source in &binding.navigation_sources {
            let projection = match source {
                NavigationProjectionSource::StandaloneOrchestrator {
                    bound_domain_id,
                    orchestrator,
                } => {
                    if *bound_domain_id != domain_id {
                        return Err(FrameProjectionBindingError::StandaloneDomainMismatch.into());
                    }
                    if orchestrator.compositor_identity() != runtime.window.compositor_identity {
                        return Err(FrameProjectionBindingError::WrongCompositor.into());
                    }
                    if !orchestrator.is_live() {
                        return Err(FrameProjectionBindingError::SourceUnavailable.into());
                    }
                    orchestrator.project()?
                }
                NavigationProjectionSource::RegisteredRoot(root_id) => {
                    let registry = runtime
                        .authorities
                        .navigation_root_registry
                        .ok_or(FrameProjectionBindingError::MissingNavigationRootRegistry)?;
                    let router = runtime
                        .authorities
                        .navigation_domain_router
                        .ok_or(FrameProjectionBindingError::MissingNavigationDomainRouter)?;
                    let root = registry
                        .root(*root_id)
                        .ok_or(FrameProjectionBindingError::UnknownRoot(*root_id))?;
                    if root.compositor_identity() != runtime.window.compositor_identity {
                        return Err(FrameProjectionBindingError::WrongCompositor.into());
                    }
                    let focus_binding = router
                        .validate_projection_root_guarded(&guard, *root_id)
                        .map_err(FrameProjectionBindingError::Router)?;
                    let projection = root.project()?;
                    if projection.active_surface != Some(focus_binding.navigation_surface_id) {
                        return Err(FrameProjectionPullError::RegisteredRootFocusMismatch {
                            root_id: *root_id,
                            projected: projection.active_surface,
                            bound: focus_binding.navigation_surface_id,
                        });
                    }
                    projection
                }
            };
            navigation_contexts.push(NavigationProjectionContext {
                navigation_surfaces: projection.navigation_surfaces,
                active_surface: projection.active_surface,
            });
        }

        let overlay_surfaces = runtime
            .authorities
            .overlay_domain_coordinator
            .active_projection_guarded(&guard)
            .map_err(FrameProjectionPullError::Overlay)?
            .map(|projection| projection.surfaces_bottom_first)
            .unwrap_or_default();
        let interaction_domain = runtime
            .authorities
            .interaction_domains
            .state(domain_id)
            .ok_or(FrameProjectionBindingError::UnknownWindowDomain(
                runtime.window.window_id,
            ))?;
        let focus = runtime
            .authorities
            .focus_resolver
            .focus_projection_guarded(
                guard.focus_capability(),
                &interaction_domain.focus_manager,
                domain_id,
            )?;
        let frame_projection = NavigationFrameProjection::new(
            runtime.window.compositor_identity,
            navigation_contexts,
            overlay_surfaces,
            focus.focused_surface,
        );
        let view = runtime
            .compositor
            .apply_frame_projection(&frame_projection)?;

        let current = runtime.bindings.binding_guarded(&guard, key)?;
        if current.as_ref().map(VersionedFrameProjectionBinding::epoch) != Some(epoch_hint)
            || current
                .as_ref()
                .map(|current| current.binding.interaction_domain_id)
                != Some(domain_id)
        {
            return Err(FrameProjectionPullError::RetryNextFrame);
        }
        Ok(view)
    })();

    match resolved {
        Ok(view) => {
            FrameProjectionBindingRegistry::screen_host_mut(&mut runtime.window, key)?
                .set_projected_view(view);
            Ok(FrameProjectionPullOutcome::Published { epoch: epoch_hint })
        }
        Err(FrameProjectionPullError::RetryNextFrame) => {
            Err(FrameProjectionPullError::RetryNextFrame)
        }
        Err(error) => {
            FrameProjectionBindingRegistry::screen_host_mut(&mut runtime.window, key)
                .expect("guarded live ScreenHost changed during projection pull")
                .clear_projected_view();
            Err(error)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameProjectionBindingError {
    GuardMismatch,
    GuardDomainMismatch {
        expected: InteractionDomainId,
        actual: InteractionDomainId,
    },
    WindowMismatch,
    WindowDomainMismatch,
    UnknownWindowDomain(WindowId),
    MissingScreenHost(ScreenHostProjectionKey),
    WrongScreenHost(ScreenHostProjectionKey),
    MissingScreenHostInvalidated(ScreenHostProjectionKey),
    UnknownBinding(ScreenHostProjectionKey),
    StaleEpoch {
        expected: FrameProjectionEpoch,
        actual: FrameProjectionEpoch,
    },
    BindingDomainImmutable,
    Busy(ScreenHostProjectionKey),
    MissingNavigationRootRegistry,
    MissingNavigationDomainRouter,
    UnknownRoot(NavigationRootId),
    SourceUnavailable,
    StandaloneDomainMismatch,
    WrongCompositor,
    ReverseIndexMismatch,
    Participation(FrameProjectionParticipationError),
    Exhausted,
    Router(NavigationDomainRouterError),
}

impl From<NavigationDomainRouterError> for FrameProjectionBindingError {
    fn from(value: NavigationDomainRouterError) -> Self {
        Self::Router(value)
    }
}

impl From<FrameProjectionParticipationError> for FrameProjectionBindingError {
    fn from(value: FrameProjectionParticipationError) -> Self {
        Self::Participation(value)
    }
}

#[derive(Debug)]
struct FrameProjectionBindingRegistryState {
    bindings: HashMap<ScreenHostProjectionKey, VersionedFrameProjectionBinding>,
    root_consumers: HashMap<NavigationRootId, HashSet<ScreenHostProjectionKey>>,
    domain_consumers: HashMap<InteractionDomainId, HashSet<ScreenHostProjectionKey>>,
    next_epoch: u64,
    pending_preparations: HashMap<ScreenHostProjectionKey, u64>,
}

impl FrameProjectionBindingRegistryState {
    fn reserve_epoch(&mut self) -> Result<FrameProjectionEpoch, FrameProjectionBindingError> {
        let epoch = FrameProjectionEpoch(self.next_epoch);
        self.next_epoch = self
            .next_epoch
            .checked_add(1)
            .ok_or(FrameProjectionBindingError::Exhausted)?;
        Ok(epoch)
    }

    fn add_reverse_indexes(
        &mut self,
        key: ScreenHostProjectionKey,
        binding: &FrameProjectionBinding,
    ) {
        let domain_inserted = self
            .domain_consumers
            .entry(binding.interaction_domain_id)
            .or_default()
            .insert(key);
        assert!(
            domain_inserted,
            "projection domain index already contained key"
        );
        for root_id in binding.registered_roots() {
            let root_inserted = self.root_consumers.entry(root_id).or_default().insert(key);
            assert!(root_inserted, "projection root index already contained key");
        }
    }

    fn remove_reverse_indexes(
        &mut self,
        key: ScreenHostProjectionKey,
        binding: &FrameProjectionBinding,
    ) {
        let remove_domain = {
            let consumers = self
                .domain_consumers
                .get_mut(&binding.interaction_domain_id)
                .expect("projection domain index disappeared");
            assert!(consumers.remove(&key), "projection domain index missed key");
            consumers.is_empty()
        };
        if remove_domain {
            self.domain_consumers.remove(&binding.interaction_domain_id);
        }
        for root_id in binding.registered_roots() {
            let remove_root = {
                let consumers = self
                    .root_consumers
                    .get_mut(&root_id)
                    .expect("projection root index disappeared");
                assert!(consumers.remove(&key), "projection root index missed key");
                consumers.is_empty()
            };
            if remove_root {
                self.root_consumers.remove(&root_id);
            }
        }
    }

    fn install_binding(
        &mut self,
        key: ScreenHostProjectionKey,
        versioned: VersionedFrameProjectionBinding,
    ) {
        assert!(
            !self.bindings.contains_key(&key),
            "projection binding unexpectedly replaced"
        );
        self.add_reverse_indexes(key, &versioned.binding);
        let previous = self.bindings.insert(key, versioned);
        assert!(previous.is_none());
    }

    fn reverse_indexes_match(
        &self,
        key: ScreenHostProjectionKey,
        binding: &FrameProjectionBinding,
    ) -> bool {
        let domain_membership_matches = self.domain_consumers.iter().all(|(domain_id, keys)| {
            keys.contains(&key) == (*domain_id == binding.interaction_domain_id)
        });
        let registered_roots = binding.registered_roots();
        let root_membership_matches = self
            .root_consumers
            .iter()
            .all(|(root_id, keys)| keys.contains(&key) == registered_roots.contains(root_id));
        domain_membership_matches
            && root_membership_matches
            && self
                .domain_consumers
                .get(&binding.interaction_domain_id)
                .is_some_and(|keys| keys.contains(&key))
            && registered_roots.iter().all(|root_id| {
                self.root_consumers
                    .get(root_id)
                    .is_some_and(|keys| keys.contains(&key))
            })
    }

    fn remove_binding(&mut self, key: ScreenHostProjectionKey) -> VersionedFrameProjectionBinding {
        let removed = self
            .bindings
            .remove(&key)
            .expect("prepared projection binding must remain installed");
        self.remove_reverse_indexes(key, &removed.binding);
        removed
    }
}

#[derive(Debug, Clone)]
pub struct FrameProjectionBindingRegistry {
    coordinator_identity: u64,
    inner: Rc<RefCell<FrameProjectionBindingRegistryState>>,
}

static NEXT_PROJECTION_PREPARATION_IDENTITY: AtomicU64 = AtomicU64::new(1);

fn allocate_projection_preparation_identity() -> Result<u64, FrameProjectionBindingError> {
    NEXT_PROJECTION_PREPARATION_IDENTITY
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| FrameProjectionBindingError::Exhausted)
}

impl FrameProjectionBindingRegistry {
    pub fn new(coordinator: &OverlayDomainCoordinator) -> Self {
        Self {
            coordinator_identity: coordinator.identity(),
            inner: Rc::new(RefCell::new(FrameProjectionBindingRegistryState {
                bindings: HashMap::new(),
                root_consumers: HashMap::new(),
                domain_consumers: HashMap::new(),
                next_epoch: 1,
                pending_preparations: HashMap::new(),
            })),
        }
    }

    pub fn guard_identity(
        &self,
        key: ScreenHostProjectionKey,
    ) -> Option<(InteractionDomainId, FrameProjectionEpoch)> {
        self.inner
            .borrow()
            .bindings
            .get(&key)
            .map(|versioned| (versioned.binding.interaction_domain_id, versioned.epoch))
    }

    pub fn install_or_replace(
        &self,
        authorities: FrameProjectionAuthorityAccess<'_>,
        mut window: FrameProjectionWindowAccess<'_>,
        key: ScreenHostProjectionKey,
        binding: FrameProjectionBinding,
        guard: &DomainTransitionGuardToken,
    ) -> Result<FrameProjectionEpoch, FrameProjectionBindingError> {
        if let Some((domain_id, expected_epoch)) = self.guard_identity(key) {
            if domain_id != binding.interaction_domain_id {
                return Err(FrameProjectionBindingError::BindingDomainImmutable);
            }
            let prepared = self.prepare_remove_or_replace_guarded(
                authorities,
                &window,
                key,
                expected_epoch,
                Some(binding),
                guard,
            )?;
            return Ok(self
                .commit_remove_or_replace_guarded(&mut window, guard, prepared)
                .expect("prepared projection replacement has an epoch"));
        }

        let participants = self.validate_binding(authorities, &window, key, &binding, guard)?;
        let participant_changes = Self::prepare_participant_changes(&[], &participants, guard)?;
        let epoch = {
            let mut state = self.inner.borrow_mut();
            if state.pending_preparations.contains_key(&key) {
                return Err(FrameProjectionBindingError::Busy(key));
            }
            state.reserve_epoch()?
        };
        Self::screen_host_mut(&mut window, key)?.clear_projected_view();
        self.inner.borrow_mut().install_binding(
            key,
            VersionedFrameProjectionBinding {
                epoch,
                binding,
                participants,
            },
        );
        for prepared in participant_changes {
            prepared.commit();
        }
        Ok(epoch)
    }

    pub fn remove(
        &self,
        mut window: FrameProjectionWindowAccess<'_>,
        key: ScreenHostProjectionKey,
        guard: &DomainTransitionGuardToken,
    ) -> Result<(), FrameProjectionBindingError> {
        let current = self
            .inner
            .borrow()
            .bindings
            .get(&key)
            .cloned()
            .ok_or(FrameProjectionBindingError::UnknownBinding(key))?;
        let domain_id = current.binding.interaction_domain_id;
        let expected_epoch = current.epoch;
        self.validate_guard(guard, domain_id)?;
        self.validate_window_identity(&window, key, domain_id)?;
        Self::validate_screen_host(&window, key)?;
        let participant_changes =
            Self::prepare_participant_changes(&current.participants, &[], guard)?;
        let prepared =
            self.reserve_prepared_mutation(key, expected_epoch, None, participant_changes, guard)?;
        self.commit_remove_or_replace_guarded(&mut window, guard, prepared);
        Ok(())
    }

    pub fn prepare_remove_or_replace_guarded(
        &self,
        authorities: FrameProjectionAuthorityAccess<'_>,
        window: &FrameProjectionWindowAccess<'_>,
        key: ScreenHostProjectionKey,
        expected_epoch: FrameProjectionEpoch,
        replacement: Option<FrameProjectionBinding>,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedFrameProjectionBindingMutation, FrameProjectionBindingError> {
        let current = {
            let state = self.inner.borrow();
            let current = state
                .bindings
                .get(&key)
                .ok_or(FrameProjectionBindingError::UnknownBinding(key))?;
            if !state.reverse_indexes_match(key, &current.binding) {
                return Err(FrameProjectionBindingError::ReverseIndexMismatch);
            }
            current.clone()
        };
        if current.epoch != expected_epoch {
            return Err(FrameProjectionBindingError::StaleEpoch {
                expected: expected_epoch,
                actual: current.epoch,
            });
        }
        self.validate_guard(guard, current.binding.interaction_domain_id)?;
        self.validate_window(
            authorities,
            window,
            key,
            current.binding.interaction_domain_id,
        )?;
        let replacement = if let Some(replacement) = replacement {
            if replacement.interaction_domain_id != current.binding.interaction_domain_id {
                return Err(FrameProjectionBindingError::BindingDomainImmutable);
            }
            let participants =
                self.resolve_participants(authorities, window, &replacement, guard)?;
            Some((replacement, participants))
        } else {
            None
        };
        let replacement_participants = replacement
            .as_ref()
            .map_or(&[][..], |(_, participants)| participants.as_slice());
        let participant_changes = Self::prepare_participant_changes(
            &current.participants,
            replacement_participants,
            guard,
        )?;
        self.reserve_prepared_mutation(key, expected_epoch, replacement, participant_changes, guard)
    }

    pub fn commit_remove_or_replace_guarded(
        &self,
        window: &mut FrameProjectionWindowAccess<'_>,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedFrameProjectionBindingMutation,
    ) -> Option<FrameProjectionEpoch> {
        assert!(
            Rc::ptr_eq(&self.inner, &prepared.registry.inner),
            "projection mutation belongs to another registry"
        );
        assert_eq!(
            self.coordinator_identity,
            prepared.registry.coordinator_identity
        );
        assert_eq!(guard.coordinator_identity(), self.coordinator_identity);
        assert_eq!(guard.interaction_domain_id(), prepared.domain_id);
        assert_eq!(guard.generation(), prepared.guard_generation);
        assert_eq!(window.window_id, prepared.key.window_id);
        assert_eq!(window.interaction_domain_id, prepared.domain_id);
        Self::screen_host_mut(window, prepared.key)
            .expect("prepared ScreenHost must remain live under the domain guard")
            .clear_projected_view();

        let result_epoch = prepared
            .replacement
            .as_ref()
            .map(|replacement| replacement.epoch);
        let mut state = self.inner.borrow_mut();
        assert_eq!(
            state.pending_preparations.get(&prepared.key),
            Some(&prepared.preparation_identity)
        );
        let current = state
            .bindings
            .get(&prepared.key)
            .expect("prepared projection binding disappeared");
        assert_eq!(current.epoch, prepared.expected_epoch);
        state.remove_binding(prepared.key);
        if let Some(replacement) = prepared.replacement.take() {
            state.install_binding(prepared.key, replacement);
        }
        state.pending_preparations.remove(&prepared.key);
        drop(state);
        for participant_change in prepared.participant_changes.drain(..) {
            participant_change.commit();
        }
        prepared.committed = true;
        result_epoch
    }

    pub fn invalidate_missing_screen_host(
        &self,
        key: ScreenHostProjectionKey,
        guard: &DomainTransitionGuardToken,
    ) -> Result<(), FrameProjectionBindingError> {
        let current = self
            .inner
            .borrow()
            .bindings
            .get(&key)
            .cloned()
            .ok_or(FrameProjectionBindingError::UnknownBinding(key))?;
        let domain_id = current.binding.interaction_domain_id;
        self.validate_guard(guard, domain_id)?;
        let participant_changes =
            Self::prepare_participant_changes(&current.participants, &[], guard)?;
        let mut state = self.inner.borrow_mut();
        if state.pending_preparations.contains_key(&key) {
            return Err(FrameProjectionBindingError::Busy(key));
        }
        state.remove_binding(key);
        drop(state);
        for participant_change in participant_changes {
            participant_change.commit();
        }
        Err(FrameProjectionBindingError::MissingScreenHostInvalidated(
            key,
        ))
    }

    pub fn root_consumers(
        &self,
        guard: &DomainTransitionGuardToken,
        root_id: NavigationRootId,
    ) -> Result<HashSet<ScreenHostProjectionKey>, FrameProjectionBindingError> {
        self.validate_guard_identity(guard)?;
        let state = self.inner.borrow();
        let consumers = state
            .root_consumers
            .get(&root_id)
            .cloned()
            .unwrap_or_default();
        let expected: HashSet<_> = state
            .bindings
            .iter()
            .filter_map(|(key, versioned)| {
                (versioned.binding.interaction_domain_id == guard.interaction_domain_id()
                    && versioned.binding.registered_roots().contains(&root_id))
                .then_some(*key)
            })
            .collect();
        if consumers != expected {
            return Err(FrameProjectionBindingError::ReverseIndexMismatch);
        }
        Ok(consumers)
    }

    pub fn domain_consumers(
        &self,
        guard: &DomainTransitionGuardToken,
        domain_id: InteractionDomainId,
    ) -> Result<HashSet<ScreenHostProjectionKey>, FrameProjectionBindingError> {
        self.validate_guard(guard, domain_id)?;
        let state = self.inner.borrow();
        let consumers = state
            .domain_consumers
            .get(&domain_id)
            .cloned()
            .unwrap_or_default();
        let expected: HashSet<_> = state
            .bindings
            .iter()
            .filter_map(|(key, versioned)| {
                (versioned.binding.interaction_domain_id == domain_id).then_some(*key)
            })
            .collect();
        if consumers != expected {
            return Err(FrameProjectionBindingError::ReverseIndexMismatch);
        }
        Ok(consumers)
    }

    pub fn binding_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        key: ScreenHostProjectionKey,
    ) -> Result<Option<VersionedFrameProjectionBinding>, FrameProjectionBindingError> {
        self.validate_guard_identity(guard)?;
        let binding = self.inner.borrow().bindings.get(&key).cloned();
        if let Some(binding) = binding.as_ref()
            && binding.binding.interaction_domain_id != guard.interaction_domain_id()
        {
            return Err(FrameProjectionBindingError::GuardDomainMismatch {
                expected: binding.binding.interaction_domain_id,
                actual: guard.interaction_domain_id(),
            });
        }
        Ok(binding)
    }

    fn reserve_prepared_mutation(
        &self,
        key: ScreenHostProjectionKey,
        expected_epoch: FrameProjectionEpoch,
        replacement: Option<(
            FrameProjectionBinding,
            Vec<NavigationSurfaceOrchestratorHandle>,
        )>,
        participant_changes: Vec<PreparedFrameProjectionParticipation>,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedFrameProjectionBindingMutation, FrameProjectionBindingError> {
        let preparation_identity = allocate_projection_preparation_identity()?;
        let mut state = self.inner.borrow_mut();
        let current = state
            .bindings
            .get(&key)
            .ok_or(FrameProjectionBindingError::UnknownBinding(key))?;
        if current.epoch != expected_epoch {
            return Err(FrameProjectionBindingError::StaleEpoch {
                expected: expected_epoch,
                actual: current.epoch,
            });
        }
        let domain_id = current.binding.interaction_domain_id;
        if state.pending_preparations.contains_key(&key) {
            return Err(FrameProjectionBindingError::Busy(key));
        }
        let replacement = match replacement {
            Some((binding, participants)) => Some(VersionedFrameProjectionBinding {
                epoch: state.reserve_epoch()?,
                binding,
                participants,
            }),
            None => None,
        };
        state.pending_preparations.insert(key, preparation_identity);
        Ok(PreparedFrameProjectionBindingMutation {
            registry: self.clone(),
            key,
            domain_id,
            guard_generation: guard.generation(),
            preparation_identity,
            expected_epoch,
            replacement,
            participant_changes,
            committed: false,
        })
    }

    fn validate_binding(
        &self,
        authorities: FrameProjectionAuthorityAccess<'_>,
        window: &FrameProjectionWindowAccess<'_>,
        key: ScreenHostProjectionKey,
        binding: &FrameProjectionBinding,
        guard: &DomainTransitionGuardToken,
    ) -> Result<Vec<NavigationSurfaceOrchestratorHandle>, FrameProjectionBindingError> {
        self.validate_guard(guard, binding.interaction_domain_id)?;
        self.validate_window(authorities, window, key, binding.interaction_domain_id)?;
        self.resolve_participants(authorities, window, binding, guard)
    }

    fn validate_window(
        &self,
        authorities: FrameProjectionAuthorityAccess<'_>,
        window: &FrameProjectionWindowAccess<'_>,
        key: ScreenHostProjectionKey,
        domain_id: InteractionDomainId,
    ) -> Result<(), FrameProjectionBindingError> {
        self.validate_window_identity(window, key, domain_id)?;
        match authorities
            .interaction_domains
            .domain_for_window(window.window_id)
        {
            Some(actual) if actual == domain_id => {}
            Some(_) => return Err(FrameProjectionBindingError::WindowDomainMismatch),
            None => {
                return Err(FrameProjectionBindingError::UnknownWindowDomain(
                    window.window_id,
                ));
            }
        }
        Self::validate_screen_host(window, key)
    }

    fn validate_window_identity(
        &self,
        window: &FrameProjectionWindowAccess<'_>,
        key: ScreenHostProjectionKey,
        domain_id: InteractionDomainId,
    ) -> Result<(), FrameProjectionBindingError> {
        if key.window_id != window.window_id {
            return Err(FrameProjectionBindingError::WindowMismatch);
        }
        if window.interaction_domain_id != domain_id {
            return Err(FrameProjectionBindingError::WindowDomainMismatch);
        }
        Ok(())
    }

    fn resolve_participants(
        &self,
        authorities: FrameProjectionAuthorityAccess<'_>,
        window: &FrameProjectionWindowAccess<'_>,
        binding: &FrameProjectionBinding,
        guard: &DomainTransitionGuardToken,
    ) -> Result<Vec<NavigationSurfaceOrchestratorHandle>, FrameProjectionBindingError> {
        let mut participants = Vec::new();
        for source in &binding.navigation_sources {
            let participant = match source {
                NavigationProjectionSource::StandaloneOrchestrator {
                    bound_domain_id,
                    orchestrator,
                } => {
                    if *bound_domain_id != binding.interaction_domain_id {
                        return Err(FrameProjectionBindingError::StandaloneDomainMismatch);
                    }
                    if orchestrator.compositor_identity() != window.compositor_identity {
                        return Err(FrameProjectionBindingError::WrongCompositor);
                    }
                    if !orchestrator.is_live() {
                        return Err(FrameProjectionBindingError::SourceUnavailable);
                    }
                    orchestrator.clone()
                }
                NavigationProjectionSource::RegisteredRoot(root_id) => {
                    let registry = authorities
                        .navigation_root_registry
                        .ok_or(FrameProjectionBindingError::MissingNavigationRootRegistry)?;
                    let router = authorities
                        .navigation_domain_router
                        .ok_or(FrameProjectionBindingError::MissingNavigationDomainRouter)?;
                    let root = registry
                        .root(*root_id)
                        .ok_or(FrameProjectionBindingError::UnknownRoot(*root_id))?;
                    if root.compositor_identity() != window.compositor_identity {
                        return Err(FrameProjectionBindingError::WrongCompositor);
                    }
                    router.validate_projection_root_guarded(guard, *root_id)?;
                    root.projection_handle()
                }
            };
            if !participants.contains(&participant) {
                participants.push(participant);
            }
        }
        Ok(participants)
    }

    fn prepare_participant_changes(
        current: &[NavigationSurfaceOrchestratorHandle],
        replacement: &[NavigationSurfaceOrchestratorHandle],
        guard: &DomainTransitionGuardToken,
    ) -> Result<Vec<PreparedFrameProjectionParticipation>, FrameProjectionBindingError> {
        let mut prepared = Vec::new();
        for participant in current {
            if !replacement.contains(participant) {
                prepared.push(participant.prepare_frame_projection_participation_remove(guard)?);
            }
        }
        for participant in replacement {
            if !current.contains(participant) {
                prepared.push(participant.prepare_frame_projection_participation_add(guard)?);
            }
        }
        Ok(prepared)
    }

    fn validate_guard(
        &self,
        guard: &DomainTransitionGuardToken,
        domain_id: InteractionDomainId,
    ) -> Result<(), FrameProjectionBindingError> {
        self.validate_guard_identity(guard)?;
        if guard.interaction_domain_id() != domain_id {
            return Err(FrameProjectionBindingError::GuardDomainMismatch {
                expected: domain_id,
                actual: guard.interaction_domain_id(),
            });
        }
        Ok(())
    }

    fn validate_guard_identity(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<(), FrameProjectionBindingError> {
        if guard.coordinator_identity() != self.coordinator_identity {
            return Err(FrameProjectionBindingError::GuardMismatch);
        }
        Ok(())
    }

    fn validate_screen_host(
        window: &FrameProjectionWindowAccess<'_>,
        key: ScreenHostProjectionKey,
    ) -> Result<(), FrameProjectionBindingError> {
        let node = window
            .widget_tree
            .node(key.node_id)
            .ok_or(FrameProjectionBindingError::MissingScreenHost(key))?;
        if node.widget.as_any().downcast_ref::<ScreenHost>().is_none() {
            return Err(FrameProjectionBindingError::WrongScreenHost(key));
        }
        Ok(())
    }

    fn screen_host_mut<'a>(
        window: &'a mut FrameProjectionWindowAccess<'_>,
        key: ScreenHostProjectionKey,
    ) -> Result<&'a mut ScreenHost, FrameProjectionBindingError> {
        let node = window
            .widget_tree
            .node_mut(key.node_id)
            .ok_or(FrameProjectionBindingError::MissingScreenHost(key))?;
        node.widget
            .as_any_mut()
            .downcast_mut::<ScreenHost>()
            .ok_or(FrameProjectionBindingError::WrongScreenHost(key))
    }
}

#[derive(Debug)]
pub struct PreparedFrameProjectionBindingMutation {
    registry: FrameProjectionBindingRegistry,
    key: ScreenHostProjectionKey,
    domain_id: InteractionDomainId,
    guard_generation: u64,
    preparation_identity: u64,
    expected_epoch: FrameProjectionEpoch,
    replacement: Option<VersionedFrameProjectionBinding>,
    participant_changes: Vec<PreparedFrameProjectionParticipation>,
    committed: bool,
}

impl PreparedFrameProjectionBindingMutation {
    pub const fn key(&self) -> ScreenHostProjectionKey {
        self.key
    }
}

impl Drop for PreparedFrameProjectionBindingMutation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut state = self.registry.inner.borrow_mut();
        if state.pending_preparations.get(&self.key) == Some(&self.preparation_identity) {
            state.pending_preparations.remove(&self.key);
        }
    }
}
