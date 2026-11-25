use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::compositor::SurfaceId;
use crate::core::event::{InteractionDomainId, InteractionDomains};
use crate::core::focus_authority::{FocusDomainGraphMutationTestimony, FocusDomainId};
use crate::core::navigation_orchestrator::NavigationRootId;
use crate::core::overlay_domain_coordinator::{
    DomainTransitionGuardToken, OverlayDomainCoordinator, OverlayDomainError, OverlayOwnerId,
    RouterEligibilityTestimony,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NavigationRootFocusBindingPortId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NavigationRootFocusBindingGeneration(u64);

impl NavigationRootFocusBindingGeneration {
    pub const fn get(self) -> u64 {
        self.0
    }

    fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavigationRootFocusBindingCandidateIdentity {
    pub focus_binding_port_id: NavigationRootFocusBindingPortId,
    pub generation: NavigationRootFocusBindingGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationRootFocusBindingCandidate {
    Stable(NavigationRootFocusBindingCandidateIdentity),
    Retry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationRootFocusBindingSnapshot {
    pub focus_binding_port_id: NavigationRootFocusBindingPortId,
    pub navigation_surface_id: SurfaceId,
    pub focus_domain_id: FocusDomainId,
    pub owned_focus_domains: HashSet<FocusDomainId>,
    pub generation: NavigationRootFocusBindingGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavigationRootFocusBindingReplacement {
    pub expected_surface: SurfaceId,
    pub expected_focus_domain: FocusDomainId,
    pub next_surface: SurfaceId,
    pub next_focus_domain: FocusDomainId,
}

impl NavigationRootFocusBindingReplacement {
    pub const fn new(
        expected_surface: SurfaceId,
        expected_focus_domain: FocusDomainId,
        next_surface: SurfaceId,
        next_focus_domain: FocusDomainId,
    ) -> Self {
        Self {
            expected_surface,
            expected_focus_domain,
            next_surface,
            next_focus_domain,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationRootFocusBindingError {
    GuardMismatch,
    WrongInteractionDomain {
        expected: InteractionDomainId,
        actual: InteractionDomainId,
    },
    EmptyOwnedFocusDomains,
    CurrentFocusDomainNotOwned(FocusDomainId),
    UnknownFocusDomain(FocusDomainId),
    IncompleteOwnedFocusTree(FocusDomainId),
    GraphRemovalDuringAdmission(FocusDomainId),
    InvalidMembershipAddition(FocusDomainId),
    InvalidMembershipRemoval(FocusDomainId),
    ExpectedSurfaceMismatch {
        expected: SurfaceId,
        actual: SurfaceId,
    },
    ExpectedFocusDomainMismatch {
        expected: FocusDomainId,
        actual: FocusDomainId,
    },
    StaleGeneration,
    Busy,
    Retired,
    ActiveReplacementMismatch {
        expected: FocusDomainId,
        actual: Option<FocusDomainId>,
    },
    PreparedValueMismatch,
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DomainGuardBinding {
    coordinator_identity: u64,
    interaction_domain_id: InteractionDomainId,
    generation: u64,
}

impl DomainGuardBinding {
    fn from_guard(guard: &DomainTransitionGuardToken) -> Self {
        Self {
            coordinator_identity: guard.coordinator_identity(),
            interaction_domain_id: guard.interaction_domain_id(),
            generation: guard.generation(),
        }
    }
}

#[derive(Debug)]
struct FocusBindingState {
    navigation_surface_id: SurfaceId,
    focus_domain_id: FocusDomainId,
    owned_focus_domains: HashSet<FocusDomainId>,
    generation: NavigationRootFocusBindingGeneration,
    last_preparation_identity: u64,
    last_focus_graph_preparation_identity: Option<u64>,
    live_preparation: Option<u64>,
    retired: bool,
}

#[derive(Debug)]
struct FocusBindingPortInner {
    port_id: NavigationRootFocusBindingPortId,
    root_id: NavigationRootId,
    admission_preparation_identity: u64,
    admission_focus_graph_preparation_identity: u64,
    coordinator_identity: u64,
    interaction_domain_id: InteractionDomainId,
    state: RefCell<FocusBindingState>,
}

#[derive(Debug, Clone)]
pub struct NavigationRootFocusBindingPort {
    inner: Rc<FocusBindingPortInner>,
}

impl NavigationRootFocusBindingPort {
    pub fn prepare_create_guarded<'a>(
        guard: &DomainTransitionGuardToken,
        prepared_root_id: NavigationRootId,
        focus_binding_port_id: NavigationRootFocusBindingPortId,
        initial_navigation_surface_id: SurfaceId,
        initial_focus_domain_id: FocusDomainId,
        prepared_initial_focus_graph_testimony: FocusDomainGraphMutationTestimony<'a>,
    ) -> Result<PreparedRootFocusBindingPortAdmission, NavigationRootFocusBindingError> {
        let guard_binding = DomainGuardBinding::from_guard(guard);
        if prepared_initial_focus_graph_testimony.interaction_domain_id()
            != guard_binding.interaction_domain_id
        {
            return Err(NavigationRootFocusBindingError::WrongInteractionDomain {
                expected: guard_binding.interaction_domain_id,
                actual: prepared_initial_focus_graph_testimony.interaction_domain_id(),
            });
        }
        if let Some(domain_id) = prepared_initial_focus_graph_testimony
            .removals()
            .first()
            .copied()
        {
            return Err(NavigationRootFocusBindingError::GraphRemovalDuringAdmission(domain_id));
        }
        let owned_focus_domains = prepared_initial_focus_graph_testimony
            .additions()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        if owned_focus_domains.is_empty() {
            return Err(NavigationRootFocusBindingError::EmptyOwnedFocusDomains);
        }
        if !owned_focus_domains.contains(&initial_focus_domain_id) {
            return Err(NavigationRootFocusBindingError::CurrentFocusDomainNotOwned(
                initial_focus_domain_id,
            ));
        }
        validate_complete_owned_trees(
            &owned_focus_domains,
            prepared_initial_focus_graph_testimony.parents_after(),
        )?;
        let preparation_identity = allocate_preparation_identity()?;
        Ok(PreparedRootFocusBindingPortAdmission {
            guard_binding,
            preparation_identity,
            focus_graph_preparation_identity: prepared_initial_focus_graph_testimony
                .preparation_identity(),
            root_id: prepared_root_id,
            port_id: focus_binding_port_id,
            navigation_surface_id: initial_navigation_surface_id,
            focus_domain_id: initial_focus_domain_id,
            owned_focus_domains,
        })
    }

    pub fn commit_create_guarded(
        guard: &DomainTransitionGuardToken,
        prepared: PreparedRootFocusBindingPortAdmission,
    ) -> Self {
        assert_eq!(
            prepared.guard_binding,
            DomainGuardBinding::from_guard(guard),
            "root focus-binding admission guard changed after preparation"
        );
        Self {
            inner: Rc::new(FocusBindingPortInner {
                port_id: prepared.port_id,
                root_id: prepared.root_id,
                admission_preparation_identity: prepared.preparation_identity,
                admission_focus_graph_preparation_identity: prepared
                    .focus_graph_preparation_identity,
                coordinator_identity: prepared.guard_binding.coordinator_identity,
                interaction_domain_id: prepared.guard_binding.interaction_domain_id,
                state: RefCell::new(FocusBindingState {
                    navigation_surface_id: prepared.navigation_surface_id,
                    focus_domain_id: prepared.focus_domain_id,
                    owned_focus_domains: prepared.owned_focus_domains,
                    generation: NavigationRootFocusBindingGeneration(1),
                    last_preparation_identity: prepared.preparation_identity,
                    last_focus_graph_preparation_identity: Some(
                        prepared.focus_graph_preparation_identity,
                    ),
                    live_preparation: None,
                    retired: false,
                }),
            }),
        }
    }

    pub fn port_id(&self) -> NavigationRootFocusBindingPortId {
        self.inner.port_id
    }

    pub fn root_id(&self) -> NavigationRootId {
        self.inner.root_id
    }

    pub fn interaction_domain_id(&self) -> InteractionDomainId {
        self.inner.interaction_domain_id
    }

    pub fn candidate_identity(&self) -> NavigationRootFocusBindingCandidate {
        let Ok(state) = self.inner.state.try_borrow() else {
            return NavigationRootFocusBindingCandidate::Retry;
        };
        if state.live_preparation.is_some() || state.retired {
            return NavigationRootFocusBindingCandidate::Retry;
        }
        NavigationRootFocusBindingCandidate::Stable(NavigationRootFocusBindingCandidateIdentity {
            focus_binding_port_id: self.inner.port_id,
            generation: state.generation,
        })
    }

    pub fn snapshot_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<NavigationRootFocusBindingSnapshot, NavigationRootFocusBindingError> {
        self.validate_guard(guard)?;
        let state = self.inner.state.borrow();
        if state.retired {
            return Err(NavigationRootFocusBindingError::Retired);
        }
        if state.live_preparation.is_some() {
            return Err(NavigationRootFocusBindingError::Busy);
        }
        Ok(NavigationRootFocusBindingSnapshot {
            focus_binding_port_id: self.inner.port_id,
            navigation_surface_id: state.navigation_surface_id,
            focus_domain_id: state.focus_domain_id,
            owned_focus_domains: state.owned_focus_domains.clone(),
            generation: state.generation,
        })
    }

    fn snapshot_candidate(
        &self,
        expected: NavigationRootFocusBindingCandidateIdentity,
    ) -> Option<NavigationRootFocusBindingSnapshot> {
        let state = self.inner.state.try_borrow().ok()?;
        if state.live_preparation.is_some()
            || state.retired
            || self.inner.port_id != expected.focus_binding_port_id
            || state.generation != expected.generation
        {
            return None;
        }
        Some(NavigationRootFocusBindingSnapshot {
            focus_binding_port_id: self.inner.port_id,
            navigation_surface_id: state.navigation_surface_id,
            focus_domain_id: state.focus_domain_id,
            owned_focus_domains: state.owned_focus_domains.clone(),
            generation: state.generation,
        })
    }

    pub fn prepare_replace_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        expected_surface: SurfaceId,
        next_surface: SurfaceId,
        next_focus_domain: FocusDomainId,
    ) -> Result<PreparedRootFocusBindingReplacement, NavigationRootFocusBindingError> {
        self.validate_guard(guard)?;
        let mut state = self.inner.state.borrow_mut();
        self.validate_mutable_state(&state)?;
        if state.navigation_surface_id != expected_surface {
            return Err(NavigationRootFocusBindingError::ExpectedSurfaceMismatch {
                expected: expected_surface,
                actual: state.navigation_surface_id,
            });
        }
        if !state.owned_focus_domains.contains(&next_focus_domain) {
            return Err(NavigationRootFocusBindingError::CurrentFocusDomainNotOwned(
                next_focus_domain,
            ));
        }
        let next_generation = state
            .generation
            .checked_next()
            .ok_or(NavigationRootFocusBindingError::Exhausted)?;
        let preparation_identity = allocate_preparation_identity()?;
        state.live_preparation = Some(preparation_identity);
        Ok(PreparedRootFocusBindingReplacement {
            port: self.clone(),
            guard_binding: DomainGuardBinding::from_guard(guard),
            preparation_identity,
            expected_generation: state.generation,
            next_generation,
            expected_surface,
            expected_focus_domain: state.focus_domain_id,
            next_surface,
            next_focus_domain,
            completed: false,
        })
    }

    pub fn commit_replace_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedRootFocusBindingReplacement,
    ) {
        self.assert_prepared_port(&prepared.port);
        assert_eq!(
            prepared.guard_binding,
            DomainGuardBinding::from_guard(guard),
            "root focus-binding replacement guard changed after preparation"
        );
        let mut state = self.inner.state.borrow_mut();
        assert_eq!(state.live_preparation, Some(prepared.preparation_identity));
        assert_eq!(state.generation, prepared.expected_generation);
        assert_eq!(state.navigation_surface_id, prepared.expected_surface);
        assert_eq!(state.focus_domain_id, prepared.expected_focus_domain);
        state.navigation_surface_id = prepared.next_surface;
        state.focus_domain_id = prepared.next_focus_domain;
        state.generation = prepared.next_generation;
        state.last_preparation_identity = prepared.preparation_identity;
        state.last_focus_graph_preparation_identity = None;
        state.live_preparation = None;
        prepared.completed = true;
    }

    pub fn prepare_owned_domain_change_guarded<'a>(
        &self,
        guard: &DomainTransitionGuardToken,
        expected_generation: NavigationRootFocusBindingGeneration,
        graph_testimony: FocusDomainGraphMutationTestimony<'a>,
        optional_current_binding_replacement: Option<NavigationRootFocusBindingReplacement>,
    ) -> Result<PreparedRootOwnedFocusDomainChange, NavigationRootFocusBindingError> {
        self.validate_guard(guard)?;
        if graph_testimony.interaction_domain_id() != self.inner.interaction_domain_id {
            return Err(NavigationRootFocusBindingError::WrongInteractionDomain {
                expected: self.inner.interaction_domain_id,
                actual: graph_testimony.interaction_domain_id(),
            });
        }
        let mut state = self.inner.state.borrow_mut();
        self.validate_mutable_state(&state)?;
        if state.generation != expected_generation {
            return Err(NavigationRootFocusBindingError::StaleGeneration);
        }

        let before_owned_focus_domains = state.owned_focus_domains.clone();
        let mut after_owned_focus_domains = before_owned_focus_domains.clone();
        for domain_id in graph_testimony.removals() {
            if !after_owned_focus_domains.remove(domain_id) {
                return Err(NavigationRootFocusBindingError::InvalidMembershipRemoval(
                    *domain_id,
                ));
            }
        }
        for domain_id in graph_testimony.additions() {
            if !after_owned_focus_domains.insert(*domain_id) {
                return Err(NavigationRootFocusBindingError::InvalidMembershipAddition(
                    *domain_id,
                ));
            }
        }
        if after_owned_focus_domains.is_empty() {
            return Err(NavigationRootFocusBindingError::EmptyOwnedFocusDomains);
        }
        validate_complete_owned_trees(&after_owned_focus_domains, graph_testimony.parents_after())?;

        let (next_surface, next_focus_domain) = match optional_current_binding_replacement {
            Some(replacement) => {
                if replacement.expected_surface != state.navigation_surface_id {
                    return Err(NavigationRootFocusBindingError::ExpectedSurfaceMismatch {
                        expected: replacement.expected_surface,
                        actual: state.navigation_surface_id,
                    });
                }
                if replacement.expected_focus_domain != state.focus_domain_id {
                    return Err(
                        NavigationRootFocusBindingError::ExpectedFocusDomainMismatch {
                            expected: replacement.expected_focus_domain,
                            actual: state.focus_domain_id,
                        },
                    );
                }
                if !after_owned_focus_domains.contains(&replacement.next_focus_domain) {
                    return Err(NavigationRootFocusBindingError::CurrentFocusDomainNotOwned(
                        replacement.next_focus_domain,
                    ));
                }
                if graph_testimony.removals().contains(&state.focus_domain_id)
                    && graph_testimony.active_focus_domain_before() == Some(state.focus_domain_id)
                    && graph_testimony.active_focus_domain_after()
                        != Some(replacement.next_focus_domain)
                {
                    return Err(NavigationRootFocusBindingError::ActiveReplacementMismatch {
                        expected: replacement.next_focus_domain,
                        actual: graph_testimony.active_focus_domain_after(),
                    });
                }
                (replacement.next_surface, replacement.next_focus_domain)
            }
            None => {
                if !after_owned_focus_domains.contains(&state.focus_domain_id) {
                    return Err(NavigationRootFocusBindingError::CurrentFocusDomainNotOwned(
                        state.focus_domain_id,
                    ));
                }
                (state.navigation_surface_id, state.focus_domain_id)
            }
        };

        let next_generation = state
            .generation
            .checked_next()
            .ok_or(NavigationRootFocusBindingError::Exhausted)?;
        let preparation_identity = allocate_preparation_identity()?;
        state.live_preparation = Some(preparation_identity);
        Ok(PreparedRootOwnedFocusDomainChange {
            port: self.clone(),
            guard_binding: DomainGuardBinding::from_guard(guard),
            preparation_identity,
            focus_graph_preparation_identity: graph_testimony.preparation_identity(),
            expected_generation,
            next_generation,
            before_owned_focus_domains,
            after_owned_focus_domains,
            next_surface,
            next_focus_domain,
            completed: false,
        })
    }

    pub fn commit_owned_domain_change_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedRootOwnedFocusDomainChange,
    ) {
        self.assert_prepared_port(&prepared.port);
        assert_eq!(
            prepared.guard_binding,
            DomainGuardBinding::from_guard(guard),
            "root-owned focus-domain change guard changed after preparation"
        );
        let mut state = self.inner.state.borrow_mut();
        assert_eq!(state.live_preparation, Some(prepared.preparation_identity));
        assert_eq!(state.generation, prepared.expected_generation);
        assert_eq!(
            state.owned_focus_domains,
            prepared.before_owned_focus_domains
        );
        state.owned_focus_domains = prepared.after_owned_focus_domains.clone();
        state.navigation_surface_id = prepared.next_surface;
        state.focus_domain_id = prepared.next_focus_domain;
        state.generation = prepared.next_generation;
        state.last_preparation_identity = prepared.preparation_identity;
        state.last_focus_graph_preparation_identity =
            Some(prepared.focus_graph_preparation_identity);
        state.live_preparation = None;
        prepared.completed = true;
    }

    pub fn prepare_retire_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        graph_testimony: FocusDomainGraphMutationTestimony<'_>,
    ) -> Result<PreparedRootFocusBindingRetirement, NavigationRootFocusBindingError> {
        self.prepare_retire_inner(guard, graph_testimony, true)
    }

    pub(crate) fn prepare_retire_for_domain_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        graph_testimony: FocusDomainGraphMutationTestimony<'_>,
    ) -> Result<PreparedRootFocusBindingRetirement, NavigationRootFocusBindingError> {
        self.prepare_retire_inner(guard, graph_testimony, false)
    }

    fn prepare_retire_inner(
        &self,
        guard: &DomainTransitionGuardToken,
        graph_testimony: FocusDomainGraphMutationTestimony<'_>,
        require_exact_graph: bool,
    ) -> Result<PreparedRootFocusBindingRetirement, NavigationRootFocusBindingError> {
        self.validate_guard(guard)?;
        if graph_testimony.interaction_domain_id() != self.inner.interaction_domain_id {
            return Err(NavigationRootFocusBindingError::WrongInteractionDomain {
                expected: self.inner.interaction_domain_id,
                actual: graph_testimony.interaction_domain_id(),
            });
        }
        let mut state = self.inner.state.borrow_mut();
        self.validate_mutable_state(&state)?;
        let retiring_domains = graph_testimony
            .removals()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        if !graph_testimony.additions().is_empty()
            || (require_exact_graph && retiring_domains != state.owned_focus_domains)
            || (!require_exact_graph && !state.owned_focus_domains.is_subset(&retiring_domains))
        {
            return Err(NavigationRootFocusBindingError::PreparedValueMismatch);
        }
        let next_generation = state
            .generation
            .checked_next()
            .ok_or(NavigationRootFocusBindingError::Exhausted)?;
        let preparation_identity = allocate_preparation_identity()?;
        state.live_preparation = Some(preparation_identity);
        Ok(PreparedRootFocusBindingRetirement {
            port: self.clone(),
            guard_binding: DomainGuardBinding::from_guard(guard),
            preparation_identity,
            focus_graph_preparation_identity: graph_testimony.preparation_identity(),
            expected_generation: state.generation,
            next_generation,
            navigation_surface_id: state.navigation_surface_id,
            focus_domain_id: state.focus_domain_id,
            owned_focus_domains: state.owned_focus_domains.clone(),
            completed: false,
        })
    }

    pub fn commit_retire_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedRootFocusBindingRetirement,
    ) {
        self.assert_prepared_port(&prepared.port);
        assert_eq!(
            prepared.guard_binding,
            DomainGuardBinding::from_guard(guard),
            "root focus-binding retirement guard changed after preparation"
        );
        let mut state = self.inner.state.borrow_mut();
        assert_eq!(state.live_preparation, Some(prepared.preparation_identity));
        assert_eq!(state.generation, prepared.expected_generation);
        assert_eq!(state.navigation_surface_id, prepared.navigation_surface_id);
        assert_eq!(state.focus_domain_id, prepared.focus_domain_id);
        assert_eq!(state.owned_focus_domains, prepared.owned_focus_domains);
        state.owned_focus_domains.clear();
        state.generation = prepared.next_generation;
        state.last_preparation_identity = prepared.preparation_identity;
        state.last_focus_graph_preparation_identity =
            Some(prepared.focus_graph_preparation_identity);
        state.live_preparation = None;
        state.retired = true;
        prepared.completed = true;
    }

    fn validate_guard(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<(), NavigationRootFocusBindingError> {
        if guard.coordinator_identity() != self.inner.coordinator_identity {
            return Err(NavigationRootFocusBindingError::GuardMismatch);
        }
        if guard.interaction_domain_id() != self.inner.interaction_domain_id {
            return Err(NavigationRootFocusBindingError::WrongInteractionDomain {
                expected: self.inner.interaction_domain_id,
                actual: guard.interaction_domain_id(),
            });
        }
        Ok(())
    }

    fn validate_mutable_state(
        &self,
        state: &FocusBindingState,
    ) -> Result<(), NavigationRootFocusBindingError> {
        if state.retired {
            return Err(NavigationRootFocusBindingError::Retired);
        }
        if state.live_preparation.is_some() {
            return Err(NavigationRootFocusBindingError::Busy);
        }
        Ok(())
    }

    fn assert_prepared_port(&self, prepared: &Self) {
        assert!(
            Rc::ptr_eq(&self.inner, &prepared.inner),
            "prepared root focus-binding value belongs to another port"
        );
    }
}

#[derive(Debug)]
pub struct PreparedRootFocusBindingPortAdmission {
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    focus_graph_preparation_identity: u64,
    root_id: NavigationRootId,
    port_id: NavigationRootFocusBindingPortId,
    navigation_surface_id: SurfaceId,
    focus_domain_id: FocusDomainId,
    owned_focus_domains: HashSet<FocusDomainId>,
}

impl PreparedRootFocusBindingPortAdmission {
    pub fn testimony(&self) -> PreparedRootFocusBindingPortAdmissionTestimony<'_> {
        PreparedRootFocusBindingPortAdmissionTestimony {
            guard_binding: self.guard_binding,
            preparation_identity: self.preparation_identity,
            focus_graph_preparation_identity: self.focus_graph_preparation_identity,
            root_id: self.root_id,
            port_id: self.port_id,
            navigation_surface_id: self.navigation_surface_id,
            focus_domain_id: self.focus_domain_id,
            owned_focus_domains: &self.owned_focus_domains,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PreparedRootFocusBindingPortAdmissionTestimony<'a> {
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    focus_graph_preparation_identity: u64,
    root_id: NavigationRootId,
    port_id: NavigationRootFocusBindingPortId,
    navigation_surface_id: SurfaceId,
    focus_domain_id: FocusDomainId,
    owned_focus_domains: &'a HashSet<FocusDomainId>,
}

#[derive(Debug)]
pub struct PreparedRootFocusBindingReplacement {
    port: NavigationRootFocusBindingPort,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    expected_generation: NavigationRootFocusBindingGeneration,
    next_generation: NavigationRootFocusBindingGeneration,
    expected_surface: SurfaceId,
    expected_focus_domain: FocusDomainId,
    next_surface: SurfaceId,
    next_focus_domain: FocusDomainId,
    completed: bool,
}

impl PreparedRootFocusBindingReplacement {
    pub fn testimony(&self) -> PreparedRootFocusBindingReplacementTestimony<'_> {
        PreparedRootFocusBindingReplacementTestimony {
            port: &self.port,
            guard_binding: self.guard_binding,
            preparation_identity: self.preparation_identity,
            expected_generation: self.expected_generation,
            next_generation: self.next_generation,
            expected_surface: self.expected_surface,
            expected_focus_domain: self.expected_focus_domain,
            next_surface: self.next_surface,
            next_focus_domain: self.next_focus_domain,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PreparedRootFocusBindingReplacementTestimony<'a> {
    port: &'a NavigationRootFocusBindingPort,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    expected_generation: NavigationRootFocusBindingGeneration,
    next_generation: NavigationRootFocusBindingGeneration,
    expected_surface: SurfaceId,
    expected_focus_domain: FocusDomainId,
    next_surface: SurfaceId,
    next_focus_domain: FocusDomainId,
}

impl Drop for PreparedRootFocusBindingReplacement {
    fn drop(&mut self) {
        if !self.completed {
            release_port_preparation(&self.port, self.preparation_identity);
        }
    }
}

#[derive(Debug)]
pub struct PreparedRootOwnedFocusDomainChange {
    port: NavigationRootFocusBindingPort,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    focus_graph_preparation_identity: u64,
    expected_generation: NavigationRootFocusBindingGeneration,
    next_generation: NavigationRootFocusBindingGeneration,
    before_owned_focus_domains: HashSet<FocusDomainId>,
    after_owned_focus_domains: HashSet<FocusDomainId>,
    next_surface: SurfaceId,
    next_focus_domain: FocusDomainId,
    completed: bool,
}

impl PreparedRootOwnedFocusDomainChange {
    pub fn testimony(&self) -> PreparedRootOwnedFocusDomainChangeTestimony<'_> {
        PreparedRootOwnedFocusDomainChangeTestimony {
            guard_binding: self.guard_binding,
            preparation_identity: self.preparation_identity,
            focus_graph_preparation_identity: self.focus_graph_preparation_identity,
            port_id: self.port.inner.port_id,
            root_id: self.port.inner.root_id,
            expected_generation: self.expected_generation,
            next_generation: self.next_generation,
            before_owned_focus_domains: &self.before_owned_focus_domains,
            after_owned_focus_domains: &self.after_owned_focus_domains,
            next_surface: self.next_surface,
            next_focus_domain: self.next_focus_domain,
        }
    }
}

impl Drop for PreparedRootOwnedFocusDomainChange {
    fn drop(&mut self) {
        if !self.completed {
            release_port_preparation(&self.port, self.preparation_identity);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PreparedRootOwnedFocusDomainChangeTestimony<'a> {
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    focus_graph_preparation_identity: u64,
    port_id: NavigationRootFocusBindingPortId,
    root_id: NavigationRootId,
    expected_generation: NavigationRootFocusBindingGeneration,
    next_generation: NavigationRootFocusBindingGeneration,
    before_owned_focus_domains: &'a HashSet<FocusDomainId>,
    after_owned_focus_domains: &'a HashSet<FocusDomainId>,
    next_surface: SurfaceId,
    next_focus_domain: FocusDomainId,
}

#[derive(Debug)]
pub struct PreparedRootFocusBindingRetirement {
    port: NavigationRootFocusBindingPort,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    focus_graph_preparation_identity: u64,
    expected_generation: NavigationRootFocusBindingGeneration,
    next_generation: NavigationRootFocusBindingGeneration,
    navigation_surface_id: SurfaceId,
    focus_domain_id: FocusDomainId,
    owned_focus_domains: HashSet<FocusDomainId>,
    completed: bool,
}

impl PreparedRootFocusBindingRetirement {
    pub fn testimony(&self) -> PreparedRootFocusBindingRetirementTestimony<'_> {
        PreparedRootFocusBindingRetirementTestimony {
            guard_binding: self.guard_binding,
            preparation_identity: self.preparation_identity,
            focus_graph_preparation_identity: self.focus_graph_preparation_identity,
            port_id: self.port.inner.port_id,
            root_id: self.port.inner.root_id,
            generation: self.expected_generation,
            next_generation: self.next_generation,
            navigation_surface_id: self.navigation_surface_id,
            focus_domain_id: self.focus_domain_id,
            owned_focus_domains: &self.owned_focus_domains,
        }
    }
}

impl Drop for PreparedRootFocusBindingRetirement {
    fn drop(&mut self) {
        if !self.completed {
            release_port_preparation(&self.port, self.preparation_identity);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PreparedRootFocusBindingRetirementTestimony<'a> {
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    focus_graph_preparation_identity: u64,
    port_id: NavigationRootFocusBindingPortId,
    root_id: NavigationRootId,
    generation: NavigationRootFocusBindingGeneration,
    next_generation: NavigationRootFocusBindingGeneration,
    navigation_surface_id: SurfaceId,
    focus_domain_id: FocusDomainId,
    owned_focus_domains: &'a HashSet<FocusDomainId>,
}

fn release_port_preparation(port: &NavigationRootFocusBindingPort, preparation_identity: u64) {
    let mut state = port.inner.state.borrow_mut();
    if state.live_preparation == Some(preparation_identity) {
        state.live_preparation = None;
    }
}

fn validate_complete_owned_trees(
    owned: &HashSet<FocusDomainId>,
    parents: &HashMap<FocusDomainId, Option<FocusDomainId>>,
) -> Result<(), NavigationRootFocusBindingError> {
    for domain_id in owned {
        if !parents.contains_key(domain_id) {
            return Err(NavigationRootFocusBindingError::UnknownFocusDomain(
                *domain_id,
            ));
        }
        let mut cursor = *domain_id;
        let mut visited = HashSet::new();
        while let Some(parent) = parents.get(&cursor).copied().flatten() {
            if !visited.insert(cursor) {
                return Err(NavigationRootFocusBindingError::IncompleteOwnedFocusTree(
                    *domain_id,
                ));
            }
            if !owned.contains(&parent) {
                return Err(NavigationRootFocusBindingError::IncompleteOwnedFocusTree(
                    *domain_id,
                ));
            }
            cursor = parent;
        }
    }
    for candidate in parents.keys().copied().filter(|id| !owned.contains(id)) {
        let mut cursor = candidate;
        let mut visited = HashSet::new();
        while let Some(parent) = parents.get(&cursor).copied().flatten() {
            if !visited.insert(cursor) {
                break;
            }
            if owned.contains(&parent) {
                return Err(NavigationRootFocusBindingError::IncompleteOwnedFocusTree(
                    candidate,
                ));
            }
            cursor = parent;
        }
    }
    Ok(())
}

static NEXT_PREPARATION_IDENTITY: AtomicU64 = AtomicU64::new(1);

fn allocate_preparation_identity() -> Result<u64, NavigationRootFocusBindingError> {
    NEXT_PREPARATION_IDENTITY
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| NavigationRootFocusBindingError::Exhausted)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NavigationRoutingGeneration(u64);

impl NavigationRoutingGeneration {
    pub const fn get(self) -> u64 {
        self.0
    }

    fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationRoutingSnapshotPhase {
    Stable,
    Committing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelegationFrame {
    pub source_root: NavigationRootId,
    pub target_root: NavigationRootId,
}

#[derive(Debug, Clone)]
struct NavigationRootBinding {
    overlay_owner_id: OverlayOwnerId,
    focus_binding_port: NavigationRootFocusBindingPort,
    routable: bool,
}

#[derive(Debug)]
struct InteractionDomainNavigationState {
    roots: HashMap<NavigationRootId, NavigationRootBinding>,
    focus_binding_port_claims: HashMap<NavigationRootFocusBindingPortId, NavigationRootId>,
    focus_domain_claims: HashMap<FocusDomainId, NavigationRootId>,
    primed_root: Option<NavigationRootId>,
    delegation_stack: Vec<DelegationFrame>,
    state_generation: NavigationRoutingGeneration,
    snapshot_phase: NavigationRoutingSnapshotPhase,
    pending_preparation: Option<u64>,
    eligibility_lease: Option<Rc<Cell<bool>>>,
}

impl InteractionDomainNavigationState {
    fn new() -> Self {
        Self {
            roots: HashMap::new(),
            focus_binding_port_claims: HashMap::new(),
            focus_domain_claims: HashMap::new(),
            primed_root: None,
            delegation_stack: Vec::new(),
            state_generation: NavigationRoutingGeneration(1),
            snapshot_phase: NavigationRoutingSnapshotPhase::Stable,
            pending_preparation: None,
            eligibility_lease: None,
        }
    }

    fn effective_root(&self) -> Option<NavigationRootId> {
        self.delegation_stack
            .last()
            .map(|frame| frame.target_root)
            .or(self.primed_root)
    }

    fn root_is_suspended(&self, root_id: NavigationRootId) -> bool {
        self.delegation_stack
            .iter()
            .any(|frame| frame.source_root == root_id)
    }

    fn release_expired_lease(&mut self) {
        if self
            .eligibility_lease
            .as_ref()
            .is_some_and(|lease| !lease.get())
        {
            self.eligibility_lease = None;
        }
    }

    fn prepare_mutation(&mut self) -> Result<(), NavigationDomainRouterError> {
        self.release_expired_lease();
        if self.snapshot_phase != NavigationRoutingSnapshotPhase::Stable
            || self.pending_preparation.is_some()
            || self
                .eligibility_lease
                .as_ref()
                .is_some_and(|lease| lease.get())
        {
            return Err(NavigationDomainRouterError::Busy);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct NavigationDomainRouterInner {
    coordinator: OverlayDomainCoordinator,
    domains: RefCell<HashMap<InteractionDomainId, InteractionDomainNavigationState>>,
}

#[derive(Debug, Clone)]
pub struct NavigationDomainRouter {
    inner: Rc<NavigationDomainRouterInner>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationDomainRouterError {
    UnknownInteractionDomain(InteractionDomainId),
    DuplicateInteractionDomain(InteractionDomainId),
    DomainNotEmpty(InteractionDomainId),
    UnknownRoot(NavigationRootId),
    DuplicateRoot(NavigationRootId),
    DuplicateOverlayOwner(OverlayOwnerId),
    UnknownOverlayOwner(OverlayOwnerId),
    DuplicateFocusBindingPort(NavigationRootFocusBindingPortId),
    DuplicateFocusDomain(FocusDomainId),
    RootDomainMismatch,
    PreparedValueMismatch,
    FocusGraphMismatch,
    StaleGeneration,
    SuspendedRoot(NavigationRootId),
    DelegationActive,
    DelegationUnavailable,
    InvalidDelegationSource {
        expected: Option<NavigationRootId>,
        actual: NavigationRootId,
    },
    DelegationCycle(NavigationRootId),
    RoutingUnavailable,
    Busy,
    Exhausted,
    Overlay(OverlayDomainError),
}

#[derive(Debug)]
pub struct PreparedRootAssociation {
    router: NavigationDomainRouter,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    focus_binding_preparation_identity: u64,
    focus_graph_preparation_identity: u64,
    root_id: NavigationRootId,
    overlay_owner_id: OverlayOwnerId,
    focus_binding_port_id: NavigationRootFocusBindingPortId,
    navigation_surface_id: SurfaceId,
    focus_domain_id: FocusDomainId,
    owned_focus_domains: HashSet<FocusDomainId>,
    expected_generation: NavigationRoutingGeneration,
    next_generation: NavigationRoutingGeneration,
    begun: bool,
    applied: bool,
    completed: bool,
}

#[derive(Debug)]
pub struct PreparedRoutedNavigationRouterCommit {
    router: NavigationDomainRouter,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    root_id: NavigationRootId,
    focus_binding_port: NavigationRootFocusBindingPort,
    focus_binding_preparation_identity: u64,
    expected_port_generation: NavigationRootFocusBindingGeneration,
    next_port_generation: NavigationRootFocusBindingGeneration,
    next_surface: SurfaceId,
    next_focus_domain: FocusDomainId,
    expected_router_generation: NavigationRoutingGeneration,
    next_router_generation: NavigationRoutingGeneration,
    begun: bool,
    completed: bool,
}

#[derive(Debug)]
pub struct PreparedDelegationPush {
    router: NavigationDomainRouter,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    frame: DelegationFrame,
    source_owner_id: OverlayOwnerId,
    target_owner_id: OverlayOwnerId,
    destination_focus_binding_port: NavigationRootFocusBindingPort,
    destination_focus_binding: NavigationRootFocusBindingSnapshot,
    expected_router_generation: NavigationRoutingGeneration,
    next_router_generation: NavigationRoutingGeneration,
    begun: bool,
    completed: bool,
}

impl PreparedDelegationPush {
    pub const fn frame(&self) -> DelegationFrame {
        self.frame
    }

    pub const fn source_owner_id(&self) -> OverlayOwnerId {
        self.source_owner_id
    }

    pub const fn target_owner_id(&self) -> OverlayOwnerId {
        self.target_owner_id
    }

    pub const fn destination_focus_binding(&self) -> &NavigationRootFocusBindingSnapshot {
        &self.destination_focus_binding
    }
}

impl Drop for PreparedDelegationPush {
    fn drop(&mut self) {
        if !self.completed && !self.begun {
            release_router_preparation(
                &self.router,
                self.guard_binding.interaction_domain_id,
                self.preparation_identity,
            );
        }
    }
}

#[derive(Debug)]
pub struct PreparedDelegationReturn {
    router: NavigationDomainRouter,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    frame: DelegationFrame,
    source_owner_id: OverlayOwnerId,
    target_owner_id: OverlayOwnerId,
    destination_focus_binding_port: NavigationRootFocusBindingPort,
    destination_focus_binding: NavigationRootFocusBindingSnapshot,
    expected_router_generation: NavigationRoutingGeneration,
    next_router_generation: NavigationRoutingGeneration,
    begun: bool,
    completed: bool,
}

impl PreparedDelegationReturn {
    pub const fn frame(&self) -> DelegationFrame {
        self.frame
    }

    pub const fn source_owner_id(&self) -> OverlayOwnerId {
        self.source_owner_id
    }

    pub const fn target_owner_id(&self) -> OverlayOwnerId {
        self.target_owner_id
    }

    pub const fn destination_focus_binding(&self) -> &NavigationRootFocusBindingSnapshot {
        &self.destination_focus_binding
    }
}

impl Drop for PreparedDelegationReturn {
    fn drop(&mut self) {
        if !self.completed && !self.begun {
            release_router_preparation(
                &self.router,
                self.guard_binding.interaction_domain_id,
                self.preparation_identity,
            );
        }
    }
}

impl Drop for PreparedRoutedNavigationRouterCommit {
    fn drop(&mut self) {
        if !self.completed && !self.begun {
            release_router_preparation(
                &self.router,
                self.guard_binding.interaction_domain_id,
                self.preparation_identity,
            );
        }
    }
}

impl Drop for PreparedRootAssociation {
    fn drop(&mut self) {
        if !self.completed && !self.begun {
            release_router_preparation(
                &self.router,
                self.guard_binding.interaction_domain_id,
                self.preparation_identity,
            );
        }
    }
}

#[derive(Debug)]
pub struct PreparedRootFocusMembershipChange {
    router: NavigationDomainRouter,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    port_preparation_identity: u64,
    focus_graph_preparation_identity: u64,
    root_id: NavigationRootId,
    focus_binding_port_id: NavigationRootFocusBindingPortId,
    expected_port_generation: NavigationRootFocusBindingGeneration,
    next_port_generation: NavigationRootFocusBindingGeneration,
    before_owned_focus_domains: HashSet<FocusDomainId>,
    after_owned_focus_domains: HashSet<FocusDomainId>,
    next_surface: SurfaceId,
    next_focus_domain: FocusDomainId,
    expected_router_generation: NavigationRoutingGeneration,
    next_router_generation: NavigationRoutingGeneration,
    begun: bool,
    applied: bool,
    completed: bool,
}

#[derive(Debug)]
pub struct PreparedRootRetirement {
    router: NavigationDomainRouter,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    port_preparation_identity: u64,
    focus_graph_preparation_identity: u64,
    root_id: NavigationRootId,
    overlay_owner_id: OverlayOwnerId,
    focus_binding_port_id: NavigationRootFocusBindingPortId,
    expected_port_generation: NavigationRootFocusBindingGeneration,
    next_port_generation: NavigationRootFocusBindingGeneration,
    retiring_focus_domains: HashSet<FocusDomainId>,
    replacement_primed_root: Option<NavigationRootId>,
    expected_delegation_stack: Vec<DelegationFrame>,
    remaining_delegation_len: usize,
    unwound_delegation_frames_top_first: Vec<DelegationFrame>,
    current_effective_root: Option<NavigationRootId>,
    final_effective_root: Option<NavigationRootId>,
    expected_router_generation: NavigationRoutingGeneration,
    next_router_generation: NavigationRoutingGeneration,
    begun: bool,
    claims_applied: bool,
    routing_applied: bool,
    association_removed: bool,
    completed: bool,
}

#[derive(Debug)]
pub(crate) struct PreparedNavigationDomainRetirement {
    router: NavigationDomainRouter,
    guard_binding: DomainGuardBinding,
    preparation_identity: u64,
    roots: Vec<PreparedNavigationDomainRootRetirement>,
    retiring_focus_domains: HashSet<FocusDomainId>,
    expected_primed_root: Option<NavigationRootId>,
    expected_delegation_stack: Vec<DelegationFrame>,
    expected_router_generation: NavigationRoutingGeneration,
    next_router_generation: NavigationRoutingGeneration,
    begun: bool,
    claims_applied: bool,
    routing_applied: bool,
    associations_removed: bool,
    completed: bool,
}

#[derive(Debug)]
struct PreparedNavigationDomainRootRetirement {
    root_id: NavigationRootId,
    overlay_owner_id: OverlayOwnerId,
    focus_binding_port_id: NavigationRootFocusBindingPortId,
    port_preparation_identity: u64,
    focus_graph_preparation_identity: u64,
    expected_port_generation: NavigationRootFocusBindingGeneration,
    next_port_generation: NavigationRootFocusBindingGeneration,
    owned_focus_domains: HashSet<FocusDomainId>,
}

impl PreparedRootRetirement {
    pub const fn overlay_owner_id(&self) -> OverlayOwnerId {
        self.overlay_owner_id
    }

    pub fn unwound_delegation_frames_top_first(&self) -> &[DelegationFrame] {
        &self.unwound_delegation_frames_top_first
    }
}

impl Drop for PreparedRootRetirement {
    fn drop(&mut self) {
        if !self.completed && !self.begun {
            release_router_preparation(
                &self.router,
                self.guard_binding.interaction_domain_id,
                self.preparation_identity,
            );
        }
    }
}

impl PreparedNavigationDomainRetirement {
    pub(crate) fn unwound_delegation_frames_top_first(&self) -> Vec<DelegationFrame> {
        self.expected_delegation_stack
            .iter()
            .rev()
            .copied()
            .collect()
    }
}

impl Drop for PreparedNavigationDomainRetirement {
    fn drop(&mut self) {
        if !self.completed && !self.begun {
            release_router_preparation(
                &self.router,
                self.guard_binding.interaction_domain_id,
                self.preparation_identity,
            );
        }
    }
}

impl Drop for PreparedRootFocusMembershipChange {
    fn drop(&mut self) {
        if !self.completed && !self.begun {
            release_router_preparation(
                &self.router,
                self.guard_binding.interaction_domain_id,
                self.preparation_identity,
            );
        }
    }
}

impl From<OverlayDomainError> for NavigationDomainRouterError {
    fn from(value: OverlayDomainError) -> Self {
        Self::Overlay(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedNavigationSnapshot {
    pub interaction_domain_id: InteractionDomainId,
    pub root_id: NavigationRootId,
    pub overlay_owner_id: OverlayOwnerId,
    pub router_generation: NavigationRoutingGeneration,
    pub focus_binding: NavigationRootFocusBindingSnapshot,
    pub explicitly_scoped: bool,
}

#[derive(Debug, Clone)]
pub struct NavigationRootRetirementSnapshot {
    interaction_domain_id: InteractionDomainId,
    root_id: NavigationRootId,
    overlay_owner_id: OverlayOwnerId,
    focus_binding_port: NavigationRootFocusBindingPort,
    focus_binding: NavigationRootFocusBindingSnapshot,
    router_generation: NavigationRoutingGeneration,
    primed: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct NavigationDomainRetirementSnapshot {
    roots: Vec<NavigationRootRetirementSnapshot>,
    router_generation: NavigationRoutingGeneration,
}

impl NavigationDomainRetirementSnapshot {
    pub(crate) fn roots(&self) -> &[NavigationRootRetirementSnapshot] {
        &self.roots
    }

    pub(crate) const fn router_generation(&self) -> NavigationRoutingGeneration {
        self.router_generation
    }
}

#[derive(Debug, Clone)]
pub struct NavigationRootDelegationRetirementPlan {
    guard_binding: DomainGuardBinding,
    root_id: NavigationRootId,
    expected_router_generation: NavigationRoutingGeneration,
    expected_primed_root: Option<NavigationRootId>,
    replacement_primed_root: Option<NavigationRootId>,
    expected_delegation_stack: Vec<DelegationFrame>,
    remaining_delegation_len: usize,
    unwound_delegation_frames_top_first: Vec<DelegationFrame>,
    current_effective_root: Option<NavigationRootId>,
    final_effective_root: Option<NavigationRootId>,
}

impl NavigationRootDelegationRetirementPlan {
    pub fn unwound_frames_top_first(&self) -> &[DelegationFrame] {
        &self.unwound_delegation_frames_top_first
    }

    pub const fn current_effective_root(&self) -> Option<NavigationRootId> {
        self.current_effective_root
    }

    pub const fn final_effective_root(&self) -> Option<NavigationRootId> {
        self.final_effective_root
    }

    pub const fn requires_delegation_unwind(&self) -> bool {
        !self.unwound_delegation_frames_top_first.is_empty()
    }
}

impl NavigationRootRetirementSnapshot {
    pub const fn interaction_domain_id(&self) -> InteractionDomainId {
        self.interaction_domain_id
    }

    pub const fn root_id(&self) -> NavigationRootId {
        self.root_id
    }

    pub const fn overlay_owner_id(&self) -> OverlayOwnerId {
        self.overlay_owner_id
    }

    pub fn focus_binding_port(&self) -> &NavigationRootFocusBindingPort {
        &self.focus_binding_port
    }

    pub const fn focus_binding(&self) -> &NavigationRootFocusBindingSnapshot {
        &self.focus_binding
    }

    pub const fn router_generation(&self) -> NavigationRoutingGeneration {
        self.router_generation
    }

    pub const fn is_primed(&self) -> bool {
        self.primed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutedNavigationCandidate {
    Snapshot(RoutedNavigationSnapshot),
    RoutingUnavailable,
    Retry,
}

impl NavigationDomainRouter {
    pub fn new(coordinator: OverlayDomainCoordinator) -> Self {
        Self {
            inner: Rc::new(NavigationDomainRouterInner {
                coordinator,
                domains: RefCell::new(HashMap::new()),
            }),
        }
    }

    pub fn register_domain(
        &self,
        interaction_domains: &InteractionDomains,
        interaction_domain_id: InteractionDomainId,
    ) -> Result<(), NavigationDomainRouterError> {
        if interaction_domains.state(interaction_domain_id).is_none() {
            return Err(NavigationDomainRouterError::UnknownInteractionDomain(
                interaction_domain_id,
            ));
        }
        let guard = self
            .inner
            .coordinator
            .begin_domain_transition(interaction_domain_id)?;
        self.inner.coordinator.validate_transition_guard(&guard)?;
        let mut domains = self.inner.domains.borrow_mut();
        if domains.contains_key(&interaction_domain_id) {
            return Err(NavigationDomainRouterError::DuplicateInteractionDomain(
                interaction_domain_id,
            ));
        }
        domains.insert(
            interaction_domain_id,
            InteractionDomainNavigationState::new(),
        );
        Ok(())
    }

    pub fn validate_root_admission_absence_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        root_id: NavigationRootId,
        overlay_owner_id: OverlayOwnerId,
        focus_binding_port_id: NavigationRootFocusBindingPortId,
    ) -> Result<(), NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let mut domains = self.inner.domains.borrow_mut();
        let domain = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        domain.prepare_mutation()?;
        if domains
            .values()
            .any(|state| state.roots.contains_key(&root_id))
        {
            return Err(NavigationDomainRouterError::DuplicateRoot(root_id));
        }
        if domains.values().any(|state| {
            state
                .roots
                .values()
                .any(|binding| binding.overlay_owner_id == overlay_owner_id)
        }) {
            return Err(NavigationDomainRouterError::DuplicateOverlayOwner(
                overlay_owner_id,
            ));
        }
        if domains.values().any(|state| {
            state
                .focus_binding_port_claims
                .contains_key(&focus_binding_port_id)
        }) {
            return Err(NavigationDomainRouterError::DuplicateFocusBindingPort(
                focus_binding_port_id,
            ));
        }
        Ok(())
    }

    pub fn route_snapshot(
        &self,
        interaction_domain_id: InteractionDomainId,
        explicitly_scoped_root: Option<NavigationRootId>,
    ) -> RoutedNavigationCandidate {
        let (generation, root_id, owner_id, port) = {
            let domains = self.inner.domains.borrow();
            let Some(state) = domains.get(&interaction_domain_id) else {
                return RoutedNavigationCandidate::RoutingUnavailable;
            };
            if state.snapshot_phase != NavigationRoutingSnapshotPhase::Stable {
                return RoutedNavigationCandidate::Retry;
            }
            let Some(root_id) = explicitly_scoped_root.or_else(|| state.effective_root()) else {
                return RoutedNavigationCandidate::RoutingUnavailable;
            };
            if state.root_is_suspended(root_id) {
                return RoutedNavigationCandidate::RoutingUnavailable;
            }
            let Some(binding) = state.roots.get(&root_id) else {
                return RoutedNavigationCandidate::RoutingUnavailable;
            };
            if !binding.routable {
                return RoutedNavigationCandidate::RoutingUnavailable;
            }
            (
                state.state_generation,
                root_id,
                binding.overlay_owner_id,
                binding.focus_binding_port.clone(),
            )
        };

        let NavigationRootFocusBindingCandidate::Stable(before_identity) =
            port.candidate_identity()
        else {
            return RoutedNavigationCandidate::Retry;
        };
        let Some(focus_binding) = port.snapshot_candidate(before_identity) else {
            return RoutedNavigationCandidate::Retry;
        };

        {
            let domains = self.inner.domains.borrow();
            let Some(state) = domains.get(&interaction_domain_id) else {
                return RoutedNavigationCandidate::Retry;
            };
            let binding_is_same = state.roots.get(&root_id).is_some_and(|binding| {
                binding.overlay_owner_id == owner_id
                    && binding.routable
                    && binding.focus_binding_port.port_id() == focus_binding.focus_binding_port_id
            });
            if state.snapshot_phase != NavigationRoutingSnapshotPhase::Stable
                || state.state_generation != generation
                || state.root_is_suspended(root_id)
                || !binding_is_same
                || explicitly_scoped_root.is_none() && state.effective_root() != Some(root_id)
            {
                return RoutedNavigationCandidate::Retry;
            }
        }

        if port.candidate_identity() != NavigationRootFocusBindingCandidate::Stable(before_identity)
        {
            return RoutedNavigationCandidate::Retry;
        }
        RoutedNavigationCandidate::Snapshot(RoutedNavigationSnapshot {
            interaction_domain_id,
            root_id,
            overlay_owner_id: owner_id,
            router_generation: generation,
            focus_binding,
            explicitly_scoped: explicitly_scoped_root.is_some(),
        })
    }

    pub fn validate_projection_root_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        root_id: NavigationRootId,
    ) -> Result<NavigationRootFocusBindingSnapshot, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let domains = self.inner.domains.borrow();
        let state = domains.get(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        if state.snapshot_phase != NavigationRoutingSnapshotPhase::Stable
            || state.pending_preparation.is_some()
            || state
                .eligibility_lease
                .as_ref()
                .is_some_and(|lease| lease.get())
        {
            return Err(NavigationDomainRouterError::Busy);
        }
        let binding = state
            .roots
            .get(&root_id)
            .ok_or(NavigationDomainRouterError::UnknownRoot(root_id))?;
        if !binding.routable {
            return Err(NavigationDomainRouterError::RoutingUnavailable);
        }
        let snapshot = binding
            .focus_binding_port
            .snapshot_guarded(guard)
            .map_err(|error| match error {
                NavigationRootFocusBindingError::Busy => NavigationDomainRouterError::Busy,
                _ => NavigationDomainRouterError::RoutingUnavailable,
            })?;
        if snapshot.owned_focus_domains.is_empty() {
            return Err(NavigationDomainRouterError::RoutingUnavailable);
        }
        Ok(snapshot)
    }

    pub fn root_retirement_snapshot_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        root_id: NavigationRootId,
    ) -> Result<NavigationRootRetirementSnapshot, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let (overlay_owner_id, focus_binding_port, router_generation, primed) = {
            let domains = self.inner.domains.borrow();
            let state = domains.get(&guard.interaction_domain_id()).ok_or(
                NavigationDomainRouterError::UnknownInteractionDomain(
                    guard.interaction_domain_id(),
                ),
            )?;
            if state.snapshot_phase != NavigationRoutingSnapshotPhase::Stable
                || state.pending_preparation.is_some()
                || state
                    .eligibility_lease
                    .as_ref()
                    .is_some_and(|lease| lease.get())
            {
                return Err(NavigationDomainRouterError::Busy);
            }
            let binding = state
                .roots
                .get(&root_id)
                .ok_or(NavigationDomainRouterError::UnknownRoot(root_id))?;
            if !binding.routable {
                return Err(NavigationDomainRouterError::RoutingUnavailable);
            }
            (
                binding.overlay_owner_id,
                binding.focus_binding_port.clone(),
                state.state_generation,
                state.primed_root == Some(root_id),
            )
        };
        let focus_binding = focus_binding_port
            .snapshot_guarded(guard)
            .map_err(|_| NavigationDomainRouterError::PreparedValueMismatch)?;
        Ok(NavigationRootRetirementSnapshot {
            interaction_domain_id: guard.interaction_domain_id(),
            root_id,
            overlay_owner_id,
            focus_binding_port,
            focus_binding,
            router_generation,
            primed,
        })
    }

    pub(crate) fn domain_retirement_snapshot_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<NavigationDomainRetirementSnapshot, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let (mut root_ids, router_generation) = {
            let domains = self.inner.domains.borrow();
            let state = domains.get(&guard.interaction_domain_id()).ok_or(
                NavigationDomainRouterError::UnknownInteractionDomain(
                    guard.interaction_domain_id(),
                ),
            )?;
            if state.snapshot_phase != NavigationRoutingSnapshotPhase::Stable
                || state.pending_preparation.is_some()
                || state
                    .eligibility_lease
                    .as_ref()
                    .is_some_and(|lease| lease.get())
            {
                return Err(NavigationDomainRouterError::Busy);
            }
            (
                state.roots.keys().copied().collect::<Vec<_>>(),
                state.state_generation,
            )
        };
        root_ids.sort_by_key(|root_id| root_id.0);
        let roots = root_ids
            .into_iter()
            .map(|root_id| self.root_retirement_snapshot_guarded(guard, root_id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(NavigationDomainRetirementSnapshot {
            roots,
            router_generation,
        })
    }

    pub fn root_delegation_retirement_plan_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        root_id: NavigationRootId,
        replacement_primed_root: Option<NavigationRootId>,
    ) -> Result<NavigationRootDelegationRetirementPlan, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        state.prepare_mutation()?;
        if !state
            .roots
            .get(&root_id)
            .is_some_and(|binding| binding.routable)
        {
            return Err(NavigationDomainRouterError::UnknownRoot(root_id));
        }

        let retiring_primed_root = state.primed_root == Some(root_id);
        match (retiring_primed_root, replacement_primed_root) {
            (false, Some(_)) => return Err(NavigationDomainRouterError::PreparedValueMismatch),
            (true, Some(replacement)) => {
                if replacement == root_id
                    || !state
                        .roots
                        .get(&replacement)
                        .is_some_and(|binding| binding.routable)
                {
                    return Err(NavigationDomainRouterError::UnknownRoot(replacement));
                }
            }
            _ => {}
        }

        let first_reference = state
            .delegation_stack
            .iter()
            .position(|frame| frame.source_root == root_id || frame.target_root == root_id);
        if retiring_primed_root && !state.delegation_stack.is_empty() && first_reference != Some(0)
        {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        let remaining_delegation_len = first_reference.unwrap_or(state.delegation_stack.len());
        let mut unwound_delegation_frames_top_first =
            state.delegation_stack[remaining_delegation_len..].to_vec();
        unwound_delegation_frames_top_first.reverse();
        let next_primed_root = if retiring_primed_root {
            replacement_primed_root
        } else {
            state.primed_root
        };
        let final_effective_root = state.delegation_stack[..remaining_delegation_len]
            .last()
            .map(|frame| frame.target_root)
            .or(next_primed_root);

        Ok(NavigationRootDelegationRetirementPlan {
            guard_binding: DomainGuardBinding::from_guard(guard),
            root_id,
            expected_router_generation: state.state_generation,
            expected_primed_root: state.primed_root,
            replacement_primed_root,
            expected_delegation_stack: state.delegation_stack.clone(),
            remaining_delegation_len,
            unwound_delegation_frames_top_first,
            current_effective_root: state.effective_root(),
            final_effective_root,
        })
    }

    pub(crate) fn focus_binding_port_for_routed_snapshot_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        snapshot: &RoutedNavigationSnapshot,
    ) -> Result<NavigationRootFocusBindingPort, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        if snapshot.interaction_domain_id != guard.interaction_domain_id() {
            return Err(NavigationDomainRouterError::RootDomainMismatch);
        }
        let port = {
            let mut domains = self.inner.domains.borrow_mut();
            let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
                NavigationDomainRouterError::UnknownInteractionDomain(
                    guard.interaction_domain_id(),
                ),
            )?;
            state.prepare_mutation()?;
            if state.state_generation != snapshot.router_generation {
                return Err(NavigationDomainRouterError::StaleGeneration);
            }
            if state.root_is_suspended(snapshot.root_id) {
                return Err(NavigationDomainRouterError::SuspendedRoot(snapshot.root_id));
            }
            if !snapshot.explicitly_scoped && state.effective_root() != Some(snapshot.root_id) {
                return Err(NavigationDomainRouterError::StaleGeneration);
            }
            let binding = state
                .roots
                .get(&snapshot.root_id)
                .ok_or(NavigationDomainRouterError::UnknownRoot(snapshot.root_id))?;
            if !binding.routable
                || binding.overlay_owner_id != snapshot.overlay_owner_id
                || binding.focus_binding_port.port_id()
                    != snapshot.focus_binding.focus_binding_port_id
            {
                return Err(NavigationDomainRouterError::PreparedValueMismatch);
            }
            binding.focus_binding_port.clone()
        };
        if port.snapshot_guarded(guard).map_err(|error| match error {
            NavigationRootFocusBindingError::StaleGeneration => {
                NavigationDomainRouterError::StaleGeneration
            }
            _ => NavigationDomainRouterError::PreparedValueMismatch,
        })? != snapshot.focus_binding
        {
            return Err(NavigationDomainRouterError::StaleGeneration);
        }
        Ok(port)
    }

    pub fn prepare_routed_navigation_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        snapshot: &RoutedNavigationSnapshot,
        replacement: PreparedRootFocusBindingReplacementTestimony<'_>,
    ) -> Result<PreparedRoutedNavigationRouterCommit, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        if snapshot.interaction_domain_id != guard.interaction_domain_id()
            || replacement.guard_binding != DomainGuardBinding::from_guard(guard)
        {
            return Err(NavigationDomainRouterError::RootDomainMismatch);
        }
        if replacement.port.root_id() != snapshot.root_id
            || replacement.port.port_id() != snapshot.focus_binding.focus_binding_port_id
            || replacement.expected_generation != snapshot.focus_binding.generation
            || replacement.expected_surface != snapshot.focus_binding.navigation_surface_id
            || replacement.expected_focus_domain != snapshot.focus_binding.focus_domain_id
        {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }

        let mut domains = self.inner.domains.borrow_mut();
        let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        state.prepare_mutation()?;
        if state.state_generation != snapshot.router_generation {
            return Err(NavigationDomainRouterError::StaleGeneration);
        }
        if state.root_is_suspended(snapshot.root_id) {
            return Err(NavigationDomainRouterError::SuspendedRoot(snapshot.root_id));
        }
        if !snapshot.explicitly_scoped && state.effective_root() != Some(snapshot.root_id) {
            return Err(NavigationDomainRouterError::StaleGeneration);
        }
        let binding = state
            .roots
            .get(&snapshot.root_id)
            .ok_or(NavigationDomainRouterError::UnknownRoot(snapshot.root_id))?;
        if !binding.routable
            || binding.overlay_owner_id != snapshot.overlay_owner_id
            || !Rc::ptr_eq(&binding.focus_binding_port.inner, &replacement.port.inner)
        {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        let next_router_generation = state
            .state_generation
            .checked_next()
            .ok_or(NavigationDomainRouterError::Exhausted)?;
        let preparation_identity = allocate_router_preparation_identity()?;
        state.pending_preparation = Some(preparation_identity);
        Ok(PreparedRoutedNavigationRouterCommit {
            router: self.clone(),
            guard_binding: DomainGuardBinding::from_guard(guard),
            preparation_identity,
            root_id: snapshot.root_id,
            focus_binding_port: replacement.port.clone(),
            focus_binding_preparation_identity: replacement.preparation_identity,
            expected_port_generation: replacement.expected_generation,
            next_port_generation: replacement.next_generation,
            next_surface: replacement.next_surface,
            next_focus_domain: replacement.next_focus_domain,
            expected_router_generation: snapshot.router_generation,
            next_router_generation,
            begun: false,
            completed: false,
        })
    }

    pub fn begin_routed_navigation_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedRoutedNavigationRouterCommit,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared routed-navigation domain must remain registered");
        assert_eq!(state.snapshot_phase, NavigationRoutingSnapshotPhase::Stable);
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(state.state_generation, prepared.expected_router_generation);
        assert!(!prepared.begun);
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Committing;
        prepared.begun = true;
    }

    pub fn finish_routed_navigation_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedRoutedNavigationRouterCommit,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.begun);
        let port_state = prepared.focus_binding_port.inner.state.borrow();
        assert_eq!(port_state.generation, prepared.next_port_generation);
        assert_eq!(
            port_state.last_preparation_identity,
            prepared.focus_binding_preparation_identity
        );
        assert_eq!(port_state.navigation_surface_id, prepared.next_surface);
        assert_eq!(port_state.focus_domain_id, prepared.next_focus_domain);
        assert_eq!(
            prepared.expected_port_generation.checked_next(),
            Some(prepared.next_port_generation)
        );
        drop(port_state);

        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared routed-navigation domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(state.state_generation, prepared.expected_router_generation);
        assert!(state.roots.contains_key(&prepared.root_id));
        state.state_generation = prepared.next_router_generation;
        state.pending_preparation = None;
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Stable;
        prepared.completed = true;
    }

    pub fn prepare_delegation_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        source_root: NavigationRootId,
        target_root: NavigationRootId,
    ) -> Result<PreparedDelegationPush, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        state.prepare_mutation()?;
        if state.effective_root() != Some(source_root) {
            return Err(NavigationDomainRouterError::InvalidDelegationSource {
                expected: state.effective_root(),
                actual: source_root,
            });
        }
        if source_root == target_root
            || state
                .delegation_stack
                .iter()
                .any(|frame| frame.source_root == target_root || frame.target_root == target_root)
        {
            return Err(NavigationDomainRouterError::DelegationCycle(target_root));
        }
        let source_binding = state
            .roots
            .get(&source_root)
            .ok_or(NavigationDomainRouterError::UnknownRoot(source_root))?;
        if !source_binding.routable || state.root_is_suspended(source_root) {
            return Err(NavigationDomainRouterError::SuspendedRoot(source_root));
        }
        let source_owner_id = source_binding.overlay_owner_id;
        let target_binding = state
            .roots
            .get(&target_root)
            .ok_or(NavigationDomainRouterError::UnknownRoot(target_root))?;
        if !target_binding.routable {
            return Err(NavigationDomainRouterError::RoutingUnavailable);
        }
        if state.root_is_suspended(target_root) {
            return Err(NavigationDomainRouterError::SuspendedRoot(target_root));
        }
        let target_owner_id = target_binding.overlay_owner_id;
        let destination_focus_binding_port = target_binding.focus_binding_port.clone();
        self.inner
            .coordinator
            .owner_registration_generation(source_owner_id)?;
        self.inner
            .coordinator
            .owner_registration_generation(target_owner_id)?;
        let destination_focus_binding = destination_focus_binding_port
            .snapshot_guarded(guard)
            .map_err(|_| NavigationDomainRouterError::PreparedValueMismatch)?;
        let next_router_generation = state
            .state_generation
            .checked_next()
            .ok_or(NavigationDomainRouterError::Exhausted)?;
        let preparation_identity = allocate_router_preparation_identity()?;
        state.pending_preparation = Some(preparation_identity);
        Ok(PreparedDelegationPush {
            router: self.clone(),
            guard_binding: DomainGuardBinding::from_guard(guard),
            preparation_identity,
            frame: DelegationFrame {
                source_root,
                target_root,
            },
            source_owner_id,
            target_owner_id,
            destination_focus_binding_port,
            destination_focus_binding,
            expected_router_generation: state.state_generation,
            next_router_generation,
            begun: false,
            completed: false,
        })
    }

    pub fn begin_delegation_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedDelegationPush,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared delegation domain must remain registered");
        assert_eq!(state.snapshot_phase, NavigationRoutingSnapshotPhase::Stable);
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(state.state_generation, prepared.expected_router_generation);
        assert_eq!(state.effective_root(), Some(prepared.frame.source_root));
        assert!(!prepared.begun);
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Committing;
        prepared.begun = true;
    }

    pub fn finish_delegation_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedDelegationPush,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.begun);
        let destination_focus_state = prepared.destination_focus_binding_port.inner.state.borrow();
        assert!(!destination_focus_state.retired);
        assert_eq!(
            destination_focus_state.generation,
            prepared.destination_focus_binding.generation
        );
        assert_eq!(
            destination_focus_state.navigation_surface_id,
            prepared.destination_focus_binding.navigation_surface_id
        );
        assert_eq!(
            destination_focus_state.focus_domain_id,
            prepared.destination_focus_binding.focus_domain_id
        );
        drop(destination_focus_state);

        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared delegation domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(state.state_generation, prepared.expected_router_generation);
        assert_eq!(state.effective_root(), Some(prepared.frame.source_root));
        let source = state
            .roots
            .get(&prepared.frame.source_root)
            .expect("prepared delegation source must remain resident");
        assert!(source.routable);
        assert_eq!(source.overlay_owner_id, prepared.source_owner_id);
        let target = state
            .roots
            .get(&prepared.frame.target_root)
            .expect("prepared delegation target must remain resident");
        assert!(target.routable);
        assert_eq!(target.overlay_owner_id, prepared.target_owner_id);
        state.delegation_stack.push(prepared.frame);
        state.state_generation = prepared.next_router_generation;
        state.pending_preparation = None;
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Stable;
        prepared.completed = true;
    }

    pub fn prepare_delegation_return_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedDelegationReturn, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        state.prepare_mutation()?;
        let frame = state
            .delegation_stack
            .last()
            .copied()
            .ok_or(NavigationDomainRouterError::DelegationUnavailable)?;
        assert_eq!(state.effective_root(), Some(frame.target_root));
        let source_binding = state
            .roots
            .get(&frame.source_root)
            .ok_or(NavigationDomainRouterError::UnknownRoot(frame.source_root))?;
        if !source_binding.routable || !state.root_is_suspended(frame.source_root) {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        let source_owner_id = source_binding.overlay_owner_id;
        let destination_focus_binding_port = source_binding.focus_binding_port.clone();
        let target_binding = state
            .roots
            .get(&frame.target_root)
            .ok_or(NavigationDomainRouterError::UnknownRoot(frame.target_root))?;
        if !target_binding.routable || state.root_is_suspended(frame.target_root) {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        let target_owner_id = target_binding.overlay_owner_id;
        self.inner
            .coordinator
            .owner_registration_generation(source_owner_id)?;
        self.inner
            .coordinator
            .owner_registration_generation(target_owner_id)?;
        let destination_focus_binding = destination_focus_binding_port
            .snapshot_guarded(guard)
            .map_err(|_| NavigationDomainRouterError::PreparedValueMismatch)?;
        let next_router_generation = state
            .state_generation
            .checked_next()
            .ok_or(NavigationDomainRouterError::Exhausted)?;
        let preparation_identity = allocate_router_preparation_identity()?;
        state.pending_preparation = Some(preparation_identity);
        Ok(PreparedDelegationReturn {
            router: self.clone(),
            guard_binding: DomainGuardBinding::from_guard(guard),
            preparation_identity,
            frame,
            source_owner_id,
            target_owner_id,
            destination_focus_binding_port,
            destination_focus_binding,
            expected_router_generation: state.state_generation,
            next_router_generation,
            begun: false,
            completed: false,
        })
    }

    pub fn begin_delegation_return_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedDelegationReturn,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared delegation-return domain must remain registered");
        assert_eq!(state.snapshot_phase, NavigationRoutingSnapshotPhase::Stable);
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(state.state_generation, prepared.expected_router_generation);
        assert_eq!(state.delegation_stack.last(), Some(&prepared.frame));
        assert!(!prepared.begun);
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Committing;
        prepared.begun = true;
    }

    pub fn finish_delegation_return_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedDelegationReturn,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.begun);
        let destination_focus_state = prepared.destination_focus_binding_port.inner.state.borrow();
        assert!(!destination_focus_state.retired);
        assert_eq!(
            destination_focus_state.generation,
            prepared.destination_focus_binding.generation
        );
        assert_eq!(
            destination_focus_state.navigation_surface_id,
            prepared.destination_focus_binding.navigation_surface_id
        );
        assert_eq!(
            destination_focus_state.focus_domain_id,
            prepared.destination_focus_binding.focus_domain_id
        );
        drop(destination_focus_state);

        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared delegation-return domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(state.state_generation, prepared.expected_router_generation);
        assert_eq!(state.delegation_stack.last(), Some(&prepared.frame));
        let source = state
            .roots
            .get(&prepared.frame.source_root)
            .expect("prepared return source must remain resident");
        assert!(source.routable);
        assert_eq!(source.overlay_owner_id, prepared.source_owner_id);
        let target = state
            .roots
            .get(&prepared.frame.target_root)
            .expect("prepared return target must remain resident");
        assert!(target.routable);
        assert_eq!(target.overlay_owner_id, prepared.target_owner_id);
        assert_eq!(state.delegation_stack.pop(), Some(prepared.frame));
        state.state_generation = prepared.next_router_generation;
        state.pending_preparation = None;
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Stable;
        prepared.completed = true;
    }

    pub fn overlay_activation_eligibility_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        overlay_owner_id: OverlayOwnerId,
    ) -> Result<RouterEligibilityTestimony, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        state.prepare_mutation()?;
        let (root_id, binding) = state
            .roots
            .iter()
            .find(|(_, binding)| binding.overlay_owner_id == overlay_owner_id)
            .ok_or(NavigationDomainRouterError::UnknownOverlayOwner(
                overlay_owner_id,
            ))?;
        if state.root_is_suspended(*root_id) {
            return Err(NavigationDomainRouterError::SuspendedRoot(*root_id));
        }
        if !binding.routable {
            return Err(NavigationDomainRouterError::UnknownRoot(*root_id));
        }
        let snapshot = binding
            .focus_binding_port
            .snapshot_guarded(guard)
            .map_err(|_| NavigationDomainRouterError::PreparedValueMismatch)?;
        let owner_registration_generation = self
            .inner
            .coordinator
            .owner_registration_generation(overlay_owner_id)?;
        let lease = Rc::new(Cell::new(true));
        state.eligibility_lease = Some(lease.clone());
        Ok(RouterEligibilityTestimony::issue_leased(
            &self.inner.coordinator,
            guard.interaction_domain_id(),
            overlay_owner_id,
            owner_registration_generation,
            state.state_generation.get(),
            snapshot.generation.get(),
            snapshot.owned_focus_domains,
            lease,
        ))
    }

    pub fn prime_root_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        root_id: NavigationRootId,
    ) -> Result<NavigationRoutingGeneration, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        state.prepare_mutation()?;
        if !state
            .roots
            .get(&root_id)
            .is_some_and(|binding| binding.routable)
        {
            return Err(NavigationDomainRouterError::UnknownRoot(root_id));
        }
        if !state.delegation_stack.is_empty() {
            return Err(NavigationDomainRouterError::DelegationActive);
        }
        if state.primed_root == Some(root_id) {
            return Ok(state.state_generation);
        }
        let next_generation = state
            .state_generation
            .checked_next()
            .ok_or(NavigationDomainRouterError::Exhausted)?;
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Committing;
        state.primed_root = Some(root_id);
        state.state_generation = next_generation;
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Stable;
        Ok(next_generation)
    }

    fn validate_guard(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<(), NavigationDomainRouterError> {
        self.inner.coordinator.validate_transition_guard(guard)?;
        Ok(())
    }
}

impl NavigationDomainRouter {
    pub fn prepare_root_association_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared_root_id: NavigationRootId,
        overlay_owner_id: OverlayOwnerId,
        port_testimony: PreparedRootFocusBindingPortAdmissionTestimony<'_>,
        graph_testimony: FocusDomainGraphMutationTestimony<'_>,
    ) -> Result<PreparedRootAssociation, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let guard_binding = DomainGuardBinding::from_guard(guard);
        if port_testimony.guard_binding != guard_binding
            || port_testimony.root_id != prepared_root_id
            || graph_testimony.interaction_domain_id() != guard.interaction_domain_id()
        {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        if port_testimony.focus_graph_preparation_identity != graph_testimony.preparation_identity()
            || !graph_testimony.removals().is_empty()
        {
            return Err(NavigationDomainRouterError::FocusGraphMismatch);
        }
        let graph_owned = graph_testimony
            .additions()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        if graph_owned != *port_testimony.owned_focus_domains
            || !graph_owned.contains(&port_testimony.focus_domain_id)
        {
            return Err(NavigationDomainRouterError::FocusGraphMismatch);
        }

        let mut domains = self.inner.domains.borrow_mut();
        if domains
            .values()
            .any(|state| state.roots.contains_key(&prepared_root_id))
        {
            return Err(NavigationDomainRouterError::DuplicateRoot(prepared_root_id));
        }
        if domains.values().any(|state| {
            state
                .roots
                .values()
                .any(|binding| binding.overlay_owner_id == overlay_owner_id)
        }) {
            return Err(NavigationDomainRouterError::DuplicateOverlayOwner(
                overlay_owner_id,
            ));
        }
        if domains.values().any(|state| {
            state
                .focus_binding_port_claims
                .contains_key(&port_testimony.port_id)
        }) {
            return Err(NavigationDomainRouterError::DuplicateFocusBindingPort(
                port_testimony.port_id,
            ));
        }
        if let Some(domain_id) = graph_owned.iter().copied().find(|domain_id| {
            domains
                .values()
                .any(|state| state.focus_domain_claims.contains_key(domain_id))
        }) {
            return Err(NavigationDomainRouterError::DuplicateFocusDomain(domain_id));
        }
        let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        state.prepare_mutation()?;
        let next_generation = state
            .state_generation
            .checked_next()
            .ok_or(NavigationDomainRouterError::Exhausted)?;
        let preparation_identity = allocate_router_preparation_identity()?;
        state.pending_preparation = Some(preparation_identity);
        Ok(PreparedRootAssociation {
            router: self.clone(),
            guard_binding,
            preparation_identity,
            focus_binding_preparation_identity: port_testimony.preparation_identity,
            focus_graph_preparation_identity: graph_testimony.preparation_identity(),
            root_id: prepared_root_id,
            overlay_owner_id,
            focus_binding_port_id: port_testimony.port_id,
            navigation_surface_id: port_testimony.navigation_surface_id,
            focus_domain_id: port_testimony.focus_domain_id,
            owned_focus_domains: graph_owned,
            expected_generation: state.state_generation,
            next_generation,
            begun: false,
            applied: false,
            completed: false,
        })
    }

    pub fn begin_root_association_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedRootAssociation,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(state.snapshot_phase, NavigationRoutingSnapshotPhase::Stable);
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(state.state_generation, prepared.expected_generation);
        assert!(!prepared.begun);
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Committing;
        prepared.begun = true;
    }

    pub fn apply_root_association_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedRootAssociation,
        newly_committed_focus_binding_port: NavigationRootFocusBindingPort,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.begun && !prepared.applied);
        assert_eq!(
            newly_committed_focus_binding_port.root_id(),
            prepared.root_id
        );
        assert_eq!(
            newly_committed_focus_binding_port.port_id(),
            prepared.focus_binding_port_id
        );
        assert_eq!(
            newly_committed_focus_binding_port
                .inner
                .admission_preparation_identity,
            prepared.focus_binding_preparation_identity
        );
        assert_eq!(
            newly_committed_focus_binding_port
                .inner
                .admission_focus_graph_preparation_identity,
            prepared.focus_graph_preparation_identity
        );
        assert_eq!(
            newly_committed_focus_binding_port.interaction_domain_id(),
            guard.interaction_domain_id()
        );
        let snapshot = newly_committed_focus_binding_port
            .snapshot_guarded(guard)
            .expect("new focus-binding port must match its committed admission");
        assert_eq!(
            snapshot.navigation_surface_id,
            prepared.navigation_surface_id
        );
        assert_eq!(snapshot.focus_domain_id, prepared.focus_domain_id);
        assert_eq!(snapshot.owned_focus_domains, prepared.owned_focus_domains);
        assert_eq!(snapshot.generation, NavigationRootFocusBindingGeneration(1));
        self.inner
            .coordinator
            .owner_registration_generation(prepared.overlay_owner_id)
            .expect("prepared overlay owner registration must commit before router association");

        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert!(
            state
                .roots
                .insert(
                    prepared.root_id,
                    NavigationRootBinding {
                        overlay_owner_id: prepared.overlay_owner_id,
                        focus_binding_port: newly_committed_focus_binding_port,
                        routable: true,
                    },
                )
                .is_none()
        );
        assert!(
            state
                .focus_binding_port_claims
                .insert(prepared.focus_binding_port_id, prepared.root_id)
                .is_none()
        );
        for domain_id in &prepared.owned_focus_domains {
            assert!(
                state
                    .focus_domain_claims
                    .insert(*domain_id, prepared.root_id)
                    .is_none()
            );
        }
        prepared.applied = true;
    }

    pub fn finish_root_association_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedRootAssociation,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.begun && prepared.applied);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        state.state_generation = prepared.next_generation;
        state.pending_preparation = None;
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Stable;
        prepared.completed = true;
    }

    pub fn prepare_root_focus_membership_change_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        root_id: NavigationRootId,
        expected_router_generation: NavigationRoutingGeneration,
        port_testimony: PreparedRootOwnedFocusDomainChangeTestimony<'_>,
        graph_testimony: FocusDomainGraphMutationTestimony<'_>,
    ) -> Result<PreparedRootFocusMembershipChange, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let guard_binding = DomainGuardBinding::from_guard(guard);
        if port_testimony.guard_binding != guard_binding
            || port_testimony.root_id != root_id
            || graph_testimony.interaction_domain_id() != guard.interaction_domain_id()
        {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        if port_testimony.focus_graph_preparation_identity != graph_testimony.preparation_identity()
        {
            return Err(NavigationDomainRouterError::FocusGraphMismatch);
        }
        let mut computed_after = port_testimony.before_owned_focus_domains.clone();
        for domain_id in graph_testimony.removals() {
            if !computed_after.remove(domain_id) {
                return Err(NavigationDomainRouterError::FocusGraphMismatch);
            }
        }
        for domain_id in graph_testimony.additions() {
            if !computed_after.insert(*domain_id) {
                return Err(NavigationDomainRouterError::FocusGraphMismatch);
            }
        }
        if computed_after != *port_testimony.after_owned_focus_domains {
            return Err(NavigationDomainRouterError::FocusGraphMismatch);
        }

        let mut domains = self.inner.domains.borrow_mut();
        let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        state.prepare_mutation()?;
        if state.state_generation != expected_router_generation {
            return Err(NavigationDomainRouterError::StaleGeneration);
        }
        let binding = state
            .roots
            .get(&root_id)
            .ok_or(NavigationDomainRouterError::UnknownRoot(root_id))?;
        if binding.focus_binding_port.port_id() != port_testimony.port_id {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        let current_claims = state
            .focus_domain_claims
            .iter()
            .filter_map(|(domain_id, owner)| (*owner == root_id).then_some(*domain_id))
            .collect::<HashSet<_>>();
        if current_claims != *port_testimony.before_owned_focus_domains
            || port_testimony
                .before_owned_focus_domains
                .iter()
                .any(|domain_id| state.focus_domain_claims.get(domain_id) != Some(&root_id))
        {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        if let Some(domain_id) = port_testimony
            .after_owned_focus_domains
            .difference(port_testimony.before_owned_focus_domains)
            .copied()
            .find(|domain_id| state.focus_domain_claims.contains_key(domain_id))
        {
            return Err(NavigationDomainRouterError::DuplicateFocusDomain(domain_id));
        }
        let next_router_generation = state
            .state_generation
            .checked_next()
            .ok_or(NavigationDomainRouterError::Exhausted)?;
        let preparation_identity = allocate_router_preparation_identity()?;
        state.pending_preparation = Some(preparation_identity);
        Ok(PreparedRootFocusMembershipChange {
            router: self.clone(),
            guard_binding,
            preparation_identity,
            port_preparation_identity: port_testimony.preparation_identity,
            focus_graph_preparation_identity: graph_testimony.preparation_identity(),
            root_id,
            focus_binding_port_id: port_testimony.port_id,
            expected_port_generation: port_testimony.expected_generation,
            next_port_generation: port_testimony.next_generation,
            before_owned_focus_domains: port_testimony.before_owned_focus_domains.clone(),
            after_owned_focus_domains: port_testimony.after_owned_focus_domains.clone(),
            next_surface: port_testimony.next_surface,
            next_focus_domain: port_testimony.next_focus_domain,
            expected_router_generation,
            next_router_generation,
            begun: false,
            applied: false,
            completed: false,
        })
    }

    pub fn begin_root_focus_membership_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedRootFocusMembershipChange,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(state.snapshot_phase, NavigationRoutingSnapshotPhase::Stable);
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(state.state_generation, prepared.expected_router_generation);
        assert!(!prepared.begun);
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Committing;
        prepared.begun = true;
    }

    pub fn apply_root_focus_membership_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedRootFocusMembershipChange,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.begun && !prepared.applied);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        let binding = state
            .roots
            .get(&prepared.root_id)
            .expect("prepared root association must remain resident");
        assert_eq!(
            binding.focus_binding_port.port_id(),
            prepared.focus_binding_port_id
        );
        let snapshot = binding
            .focus_binding_port
            .snapshot_guarded(guard)
            .expect("focus-binding membership commit must precede router claim publication");
        {
            let port_state = binding.focus_binding_port.inner.state.borrow();
            assert_eq!(
                port_state.last_preparation_identity,
                prepared.port_preparation_identity
            );
            assert_eq!(
                port_state.last_focus_graph_preparation_identity,
                Some(prepared.focus_graph_preparation_identity)
            );
        }
        assert_eq!(
            prepared.expected_port_generation.checked_next(),
            Some(prepared.next_port_generation)
        );
        assert_eq!(snapshot.generation, prepared.next_port_generation);
        assert_eq!(snapshot.navigation_surface_id, prepared.next_surface);
        assert_eq!(snapshot.focus_domain_id, prepared.next_focus_domain);
        assert_eq!(
            snapshot.owned_focus_domains,
            prepared.after_owned_focus_domains
        );
        for domain_id in prepared
            .before_owned_focus_domains
            .difference(&prepared.after_owned_focus_domains)
        {
            assert_eq!(
                state.focus_domain_claims.remove(domain_id),
                Some(prepared.root_id)
            );
        }
        for domain_id in prepared
            .after_owned_focus_domains
            .difference(&prepared.before_owned_focus_domains)
        {
            assert!(
                state
                    .focus_domain_claims
                    .insert(*domain_id, prepared.root_id)
                    .is_none()
            );
        }
        prepared.applied = true;
    }

    pub fn finish_root_focus_membership_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedRootFocusMembershipChange,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.begun && prepared.applied);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        state.state_generation = prepared.next_router_generation;
        state.pending_preparation = None;
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Stable;
        prepared.completed = true;
    }

    pub fn prepare_root_retirement_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        root_id: NavigationRootId,
        expected_router_generation: NavigationRoutingGeneration,
        port_testimony: PreparedRootFocusBindingRetirementTestimony<'_>,
        graph_testimony: FocusDomainGraphMutationTestimony<'_>,
        delegation_plan: &NavigationRootDelegationRetirementPlan,
    ) -> Result<PreparedRootRetirement, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let guard_binding = DomainGuardBinding::from_guard(guard);
        if port_testimony.guard_binding != guard_binding
            || port_testimony.root_id != root_id
            || graph_testimony.interaction_domain_id() != guard.interaction_domain_id()
            || delegation_plan.guard_binding != guard_binding
            || delegation_plan.root_id != root_id
            || delegation_plan.expected_router_generation != expected_router_generation
        {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        let graph_removals = graph_testimony
            .removals()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        if port_testimony.focus_graph_preparation_identity != graph_testimony.preparation_identity()
            || !graph_testimony.additions().is_empty()
            || graph_removals != *port_testimony.owned_focus_domains
        {
            return Err(NavigationDomainRouterError::FocusGraphMismatch);
        }

        let mut domains = self.inner.domains.borrow_mut();
        let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        state.prepare_mutation()?;
        if state.state_generation != expected_router_generation {
            return Err(NavigationDomainRouterError::StaleGeneration);
        }
        if state.primed_root != delegation_plan.expected_primed_root
            || state.delegation_stack != delegation_plan.expected_delegation_stack
            || delegation_plan.remaining_delegation_len > state.delegation_stack.len()
        {
            return Err(NavigationDomainRouterError::StaleGeneration);
        }
        let binding = state
            .roots
            .get(&root_id)
            .ok_or(NavigationDomainRouterError::UnknownRoot(root_id))?;
        if !binding.routable || binding.focus_binding_port.port_id() != port_testimony.port_id {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        {
            let port_state = binding.focus_binding_port.inner.state.borrow();
            if port_state.live_preparation != Some(port_testimony.preparation_identity)
                || port_state.generation != port_testimony.generation
                || port_state.navigation_surface_id != port_testimony.navigation_surface_id
                || port_state.focus_domain_id != port_testimony.focus_domain_id
                || port_state.owned_focus_domains != *port_testimony.owned_focus_domains
            {
                return Err(NavigationDomainRouterError::PreparedValueMismatch);
            }
        }
        let current_claims = state
            .focus_domain_claims
            .iter()
            .filter_map(|(domain_id, owner)| (*owner == root_id).then_some(*domain_id))
            .collect::<HashSet<_>>();
        if current_claims != *port_testimony.owned_focus_domains
            || state.focus_binding_port_claims.get(&port_testimony.port_id) != Some(&root_id)
        {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        match (
            state.primed_root == Some(root_id),
            delegation_plan.replacement_primed_root,
        ) {
            (false, Some(_)) => return Err(NavigationDomainRouterError::PreparedValueMismatch),
            (true, Some(replacement)) => {
                if replacement == root_id
                    || !state
                        .roots
                        .get(&replacement)
                        .is_some_and(|binding| binding.routable)
                {
                    return Err(NavigationDomainRouterError::UnknownRoot(replacement));
                }
            }
            _ => {}
        }
        let next_router_generation = state
            .state_generation
            .checked_next()
            .ok_or(NavigationDomainRouterError::Exhausted)?;
        let preparation_identity = allocate_router_preparation_identity()?;
        state.pending_preparation = Some(preparation_identity);
        Ok(PreparedRootRetirement {
            router: self.clone(),
            guard_binding,
            preparation_identity,
            port_preparation_identity: port_testimony.preparation_identity,
            focus_graph_preparation_identity: graph_testimony.preparation_identity(),
            root_id,
            overlay_owner_id: binding.overlay_owner_id,
            focus_binding_port_id: port_testimony.port_id,
            expected_port_generation: port_testimony.generation,
            next_port_generation: port_testimony.next_generation,
            retiring_focus_domains: graph_removals,
            replacement_primed_root: delegation_plan.replacement_primed_root,
            expected_delegation_stack: delegation_plan.expected_delegation_stack.clone(),
            remaining_delegation_len: delegation_plan.remaining_delegation_len,
            unwound_delegation_frames_top_first: delegation_plan
                .unwound_delegation_frames_top_first
                .clone(),
            current_effective_root: delegation_plan.current_effective_root,
            final_effective_root: delegation_plan.final_effective_root,
            expected_router_generation,
            next_router_generation,
            begun: false,
            claims_applied: false,
            routing_applied: false,
            association_removed: false,
            completed: false,
        })
    }

    pub(crate) fn prepare_domain_retirement_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        expected_router_generation: NavigationRoutingGeneration,
        port_testimonies: &[PreparedRootFocusBindingRetirementTestimony<'_>],
        graph_testimony: FocusDomainGraphMutationTestimony<'_>,
    ) -> Result<PreparedNavigationDomainRetirement, NavigationDomainRouterError> {
        self.validate_guard(guard)?;
        let guard_binding = DomainGuardBinding::from_guard(guard);
        if graph_testimony.interaction_domain_id() != guard.interaction_domain_id()
            || !graph_testimony.additions().is_empty()
            || graph_testimony.active_focus_domain_after().is_some()
            || !graph_testimony.parents_after().is_empty()
        {
            return Err(NavigationDomainRouterError::FocusGraphMismatch);
        }
        let graph_removals = graph_testimony
            .removals()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains.get_mut(&guard.interaction_domain_id()).ok_or(
            NavigationDomainRouterError::UnknownInteractionDomain(guard.interaction_domain_id()),
        )?;
        state.prepare_mutation()?;
        let claimed_focus_domains = state
            .focus_domain_claims
            .keys()
            .copied()
            .collect::<HashSet<_>>();
        if state.state_generation != expected_router_generation
            || !claimed_focus_domains.is_subset(&graph_removals)
            || port_testimonies.len() != state.roots.len()
        {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }

        let mut seen_roots = HashSet::new();
        let mut seen_ports = HashSet::new();
        let mut testified_domains = HashSet::new();
        let mut roots = Vec::with_capacity(port_testimonies.len());
        for testimony in port_testimonies {
            if testimony.guard_binding != guard_binding
                || testimony.focus_graph_preparation_identity
                    != graph_testimony.preparation_identity()
                || !seen_roots.insert(testimony.root_id)
                || !seen_ports.insert(testimony.port_id)
            {
                return Err(NavigationDomainRouterError::PreparedValueMismatch);
            }
            let binding = state
                .roots
                .get(&testimony.root_id)
                .ok_or(NavigationDomainRouterError::UnknownRoot(testimony.root_id))?;
            if !binding.routable
                || binding.focus_binding_port.port_id() != testimony.port_id
                || state.focus_binding_port_claims.get(&testimony.port_id)
                    != Some(&testimony.root_id)
            {
                return Err(NavigationDomainRouterError::PreparedValueMismatch);
            }
            let port_state = binding.focus_binding_port.inner.state.borrow();
            if port_state.live_preparation != Some(testimony.preparation_identity)
                || port_state.generation != testimony.generation
                || port_state.navigation_surface_id != testimony.navigation_surface_id
                || port_state.focus_domain_id != testimony.focus_domain_id
                || port_state.owned_focus_domains != *testimony.owned_focus_domains
            {
                return Err(NavigationDomainRouterError::PreparedValueMismatch);
            }
            drop(port_state);
            for focus_domain_id in testimony.owned_focus_domains {
                if state.focus_domain_claims.get(focus_domain_id) != Some(&testimony.root_id)
                    || !testified_domains.insert(*focus_domain_id)
                {
                    return Err(NavigationDomainRouterError::PreparedValueMismatch);
                }
            }
            roots.push(PreparedNavigationDomainRootRetirement {
                root_id: testimony.root_id,
                overlay_owner_id: binding.overlay_owner_id,
                focus_binding_port_id: testimony.port_id,
                port_preparation_identity: testimony.preparation_identity,
                focus_graph_preparation_identity: testimony.focus_graph_preparation_identity,
                expected_port_generation: testimony.generation,
                next_port_generation: testimony.next_generation,
                owned_focus_domains: testimony.owned_focus_domains.clone(),
            });
        }
        if testified_domains != claimed_focus_domains
            || seen_ports
                != state
                    .focus_binding_port_claims
                    .keys()
                    .copied()
                    .collect::<HashSet<_>>()
            || seen_roots != state.roots.keys().copied().collect::<HashSet<_>>()
        {
            return Err(NavigationDomainRouterError::PreparedValueMismatch);
        }
        roots.sort_by_key(|root| root.root_id.0);
        let next_router_generation = state
            .state_generation
            .checked_next()
            .ok_or(NavigationDomainRouterError::Exhausted)?;
        let preparation_identity = allocate_router_preparation_identity()?;
        state.pending_preparation = Some(preparation_identity);
        Ok(PreparedNavigationDomainRetirement {
            router: self.clone(),
            guard_binding,
            preparation_identity,
            roots,
            retiring_focus_domains: claimed_focus_domains,
            expected_primed_root: state.primed_root,
            expected_delegation_stack: state.delegation_stack.clone(),
            expected_router_generation,
            next_router_generation,
            begun: false,
            claims_applied: false,
            routing_applied: false,
            associations_removed: false,
            completed: false,
        })
    }

    pub fn begin_root_retirement_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedRootRetirement,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(state.snapshot_phase, NavigationRoutingSnapshotPhase::Stable);
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(state.state_generation, prepared.expected_router_generation);
        assert!(!prepared.begun);
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Committing;
        prepared.begun = true;
    }

    pub fn apply_root_retirement_claims_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedRootRetirement,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.begun && !prepared.claims_applied);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        let binding = state
            .roots
            .get(&prepared.root_id)
            .expect("prepared retiring root must remain associated");
        let port_state = binding.focus_binding_port.inner.state.borrow();
        assert!(port_state.retired);
        assert_eq!(
            prepared.expected_port_generation.checked_next(),
            Some(prepared.next_port_generation)
        );
        assert_eq!(port_state.generation, prepared.next_port_generation);
        assert_eq!(
            port_state.last_preparation_identity,
            prepared.port_preparation_identity
        );
        assert_eq!(
            port_state.last_focus_graph_preparation_identity,
            Some(prepared.focus_graph_preparation_identity)
        );
        assert!(port_state.owned_focus_domains.is_empty());
        drop(port_state);
        for domain_id in &prepared.retiring_focus_domains {
            assert_eq!(
                state.focus_domain_claims.remove(domain_id),
                Some(prepared.root_id)
            );
        }
        assert_eq!(
            state
                .focus_binding_port_claims
                .remove(&prepared.focus_binding_port_id),
            Some(prepared.root_id)
        );
        prepared.claims_applied = true;
    }

    pub fn apply_root_routing_retirement_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedRootRetirement,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.claims_applied && !prepared.routing_applied);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        let binding = state
            .roots
            .get_mut(&prepared.root_id)
            .expect("prepared retiring root must remain associated");
        assert!(binding.routable);
        binding.routable = false;
        assert_eq!(state.delegation_stack, prepared.expected_delegation_stack);
        assert_eq!(state.effective_root(), prepared.current_effective_root);
        state
            .delegation_stack
            .truncate(prepared.remaining_delegation_len);
        if state.primed_root == Some(prepared.root_id) {
            state.primed_root = prepared.replacement_primed_root;
        }
        assert_eq!(state.effective_root(), prepared.final_effective_root);
        prepared.routing_applied = true;
    }

    pub fn remove_root_association_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedRootRetirement,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.routing_applied && !prepared.association_removed);
        assert_eq!(
            self.inner
                .coordinator
                .owner_registration_generation(prepared.overlay_owner_id),
            Err(OverlayDomainError::UnknownOwner(prepared.overlay_owner_id)),
            "overlay owner must be unregistered before router association removal"
        );
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        let removed = state
            .roots
            .remove(&prepared.root_id)
            .expect("prepared retiring root must remain associated");
        assert_eq!(removed.overlay_owner_id, prepared.overlay_owner_id);
        assert!(!removed.routable);
        prepared.association_removed = true;
    }

    pub fn finish_root_retirement_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedRootRetirement,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.association_removed);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        state.state_generation = prepared.next_router_generation;
        state.pending_preparation = None;
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Stable;
        prepared.completed = true;
    }

    pub(crate) fn begin_domain_retirement_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedNavigationDomainRetirement,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(state.snapshot_phase, NavigationRoutingSnapshotPhase::Stable);
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(state.state_generation, prepared.expected_router_generation);
        assert!(!prepared.begun);
        state.snapshot_phase = NavigationRoutingSnapshotPhase::Committing;
        prepared.begun = true;
    }

    pub(crate) fn apply_domain_retirement_claims_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedNavigationDomainRetirement,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.begun && !prepared.claims_applied);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        for root in &prepared.roots {
            let binding = state
                .roots
                .get(&root.root_id)
                .expect("prepared retiring root must remain associated");
            let port_state = binding.focus_binding_port.inner.state.borrow();
            assert!(port_state.retired);
            assert_eq!(
                root.expected_port_generation.checked_next(),
                Some(root.next_port_generation)
            );
            assert_eq!(port_state.generation, root.next_port_generation);
            assert_eq!(
                port_state.last_preparation_identity,
                root.port_preparation_identity
            );
            assert_eq!(
                port_state.last_focus_graph_preparation_identity,
                Some(root.focus_graph_preparation_identity)
            );
            assert!(port_state.owned_focus_domains.is_empty());
        }
        assert_eq!(
            state
                .focus_domain_claims
                .keys()
                .copied()
                .collect::<HashSet<_>>(),
            prepared.retiring_focus_domains
        );
        for root in &prepared.roots {
            for focus_domain_id in &root.owned_focus_domains {
                assert_eq!(
                    state.focus_domain_claims.remove(focus_domain_id),
                    Some(root.root_id)
                );
            }
            assert_eq!(
                state
                    .focus_binding_port_claims
                    .remove(&root.focus_binding_port_id),
                Some(root.root_id)
            );
        }
        assert!(state.focus_domain_claims.is_empty());
        assert!(state.focus_binding_port_claims.is_empty());
        prepared.claims_applied = true;
    }

    pub(crate) fn apply_domain_routing_retirement_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedNavigationDomainRetirement,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.claims_applied && !prepared.routing_applied);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(state.primed_root, prepared.expected_primed_root);
        assert_eq!(state.delegation_stack, prepared.expected_delegation_stack);
        for root in &prepared.roots {
            let binding = state
                .roots
                .get_mut(&root.root_id)
                .expect("prepared retiring root must remain associated");
            assert!(binding.routable);
            binding.routable = false;
        }
        state.primed_root = None;
        state.delegation_stack.clear();
        prepared.routing_applied = true;
    }

    pub(crate) fn remove_domain_root_associations_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: &mut PreparedNavigationDomainRetirement,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.routing_applied && !prepared.associations_removed);
        for root in &prepared.roots {
            assert_eq!(
                self.inner
                    .coordinator
                    .owner_registration_generation(root.overlay_owner_id),
                Err(OverlayDomainError::UnknownOwner(root.overlay_owner_id)),
                "overlay owner must be unregistered before domain root removal"
            );
        }
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get_mut(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        for root in &prepared.roots {
            let removed = state
                .roots
                .remove(&root.root_id)
                .expect("prepared retiring root must remain associated");
            assert_eq!(removed.overlay_owner_id, root.overlay_owner_id);
            assert!(!removed.routable);
        }
        assert!(state.roots.is_empty());
        prepared.associations_removed = true;
    }

    pub(crate) fn finish_domain_retirement_commit_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedNavigationDomainRetirement,
    ) {
        self.assert_prepared_router(&prepared.router);
        self.assert_prepared_guard(guard, prepared.guard_binding);
        assert!(prepared.associations_removed);
        let mut domains = self.inner.domains.borrow_mut();
        let state = domains
            .get(&guard.interaction_domain_id())
            .expect("prepared router domain must remain registered");
        assert_eq!(
            state.snapshot_phase,
            NavigationRoutingSnapshotPhase::Committing
        );
        assert_eq!(
            state.pending_preparation,
            Some(prepared.preparation_identity)
        );
        assert_eq!(
            prepared.expected_router_generation.checked_next(),
            Some(prepared.next_router_generation)
        );
        assert!(state.roots.is_empty());
        assert!(state.focus_binding_port_claims.is_empty());
        assert!(state.focus_domain_claims.is_empty());
        assert!(state.primed_root.is_none());
        assert!(state.delegation_stack.is_empty());
        domains.remove(&guard.interaction_domain_id());
        prepared.completed = true;
    }

    fn assert_prepared_router(&self, prepared: &Self) {
        assert!(
            Rc::ptr_eq(&self.inner, &prepared.inner),
            "prepared navigation-domain value belongs to another router"
        );
    }

    fn assert_prepared_guard(
        &self,
        guard: &DomainTransitionGuardToken,
        expected: DomainGuardBinding,
    ) {
        self.validate_guard(guard)
            .expect("navigation-domain transition guard must remain live");
        assert_eq!(expected, DomainGuardBinding::from_guard(guard));
    }
}

fn release_router_preparation(
    router: &NavigationDomainRouter,
    interaction_domain_id: InteractionDomainId,
    preparation_identity: u64,
) {
    let mut domains = router.inner.domains.borrow_mut();
    if let Some(state) = domains.get_mut(&interaction_domain_id)
        && state.snapshot_phase == NavigationRoutingSnapshotPhase::Stable
        && state.pending_preparation == Some(preparation_identity)
    {
        state.pending_preparation = None;
    }
}

static NEXT_ROUTER_PREPARATION_IDENTITY: AtomicU64 = AtomicU64::new(1);

fn allocate_router_preparation_identity() -> Result<u64, NavigationDomainRouterError> {
    NEXT_ROUTER_PREPARATION_IDENTITY
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| NavigationDomainRouterError::Exhausted)
}
