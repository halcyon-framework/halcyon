// SPEC-HAL-228: NavigationHost — Navigation State Authority & Resolution
// SPEC-HAL-231: Application Navigation Guard and Resolution Override
// Semantic navigation mechanics only: no widgets, surfaces, focus, or rendering.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Stable application route identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NavigationNodeId(pub u32);

/// Complete restorable identity for one semantic navigation destination.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NavigationEntry {
    node_id: NavigationNodeId,
    restoration_key: Option<String>,
    parameters: Vec<(String, String)>,
}

impl NavigationEntry {
    pub fn new(node_id: NavigationNodeId) -> Self {
        Self {
            node_id,
            restoration_key: None,
            parameters: Vec::new(),
        }
    }

    pub fn with_restoration_key(mut self, restoration_key: impl Into<String>) -> Self {
        self.restoration_key = Some(restoration_key.into());
        self
    }

    pub fn with_parameter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        match self
            .parameters
            .binary_search_by(|(existing, _)| existing.cmp(&key))
        {
            Ok(index) => self.parameters[index].1 = value,
            Err(index) => self.parameters.insert(index, (key, value)),
        }
        self
    }

    pub fn node_id(&self) -> NavigationNodeId {
        self.node_id
    }

    pub fn restoration_key(&self) -> Option<&str> {
        self.restoration_key.as_deref()
    }

    pub fn parameters(&self) -> &[(String, String)] {
        &self.parameters
    }
}

impl From<NavigationNodeId> for NavigationEntry {
    fn from(node_id: NavigationNodeId) -> Self {
        Self::new(node_id)
    }
}

/// Explicit request to navigate within one NavigationHost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationIntent {
    NavigateTo(NavigationEntry),
    NavigatePath(Vec<NavigationEntry>),
    NavigateUp,
    HistoryBack,
    HistoryForward,
    /// Legacy spelling for structural retreat. It is never history traversal.
    #[deprecated(note = "use NavigationIntent::NavigateUp")]
    NavigateBack,
}

/// One committed navigation state. Current is always derived from the final path entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationState {
    path: Vec<NavigationEntry>,
}

impl NavigationState {
    fn from_valid_path(path: Vec<NavigationEntry>) -> Self {
        debug_assert!(!path.is_empty());
        Self { path }
    }

    pub fn current(&self) -> &NavigationEntry {
        self.path
            .last()
            .expect("NavigationState paths are never empty")
    }

    pub fn path(&self) -> &[NavigationEntry] {
        &self.path
    }
}

/// Checked, non-reused generation of NavigationHost mechanics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NavigationMechanicsGeneration(u64);

impl NavigationMechanicsGeneration {
    pub fn get(self) -> u64 {
        self.0
    }

    fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Optional application-owned guard-context generation bound to an authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NavigationGuardContextGeneration(u64);

impl NavigationGuardContextGeneration {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

/// Coherent immutable projection for navigation controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationControlProjection {
    pub path: Vec<NavigationEntry>,
    pub current: NavigationEntry,
    pub can_navigate_up: bool,
    pub can_history_back: bool,
    pub can_history_forward: bool,
}

/// Guard decision for one fully resolved candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardDecision {
    Allow,
    Deny,
    ReplaceWith(NavigationIntent),
}

/// Application-owned navigation permission seam.
pub trait NavigationGuard {
    fn evaluate(
        &self,
        requested_intent: &NavigationIntent,
        evaluated_intent: &NavigationIntent,
        from: &NavigationState,
        to: &NavigationState,
        context: Option<&dyn Any>,
    ) -> GuardDecision;
}

/// Final semantic result of navigation execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationOutcome {
    Committed { redirected: bool },
    Denied,
    Unavailable,
    NoOp,
}

/// Compatibility result used by the legacy direct dispatch façade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDecision {
    Allow,
    Deny,
    Redirect,
    Unavailable,
    NoOp,
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationSnapshotError {
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationPreparationError {
    WrongHost,
    StaleGeneration,
    StaleGuardContext,
    FrameProjectionGuardRequired,
    FrameProjectionGuardMismatch,
    Busy,
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationExecutionError {
    Retry,
}

#[derive(Debug, Clone)]
struct NavigationMechanics {
    history: Vec<NavigationState>,
    cursor: usize,
    generation: NavigationMechanicsGeneration,
}

impl NavigationMechanics {
    fn current_state(&self) -> &NavigationState {
        &self.history[self.cursor]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NavigationCommitParticipationIdentity {
    coordinator_identity: u64,
    interaction_domain_id: u64,
}

impl NavigationCommitParticipationIdentity {
    pub(crate) const fn new(coordinator_identity: u64, interaction_domain_id: u64) -> Self {
        Self {
            coordinator_identity,
            interaction_domain_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavigationCommitParticipationDelta {
    Add,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NavigationCommitParticipation {
    identity: NavigationCommitParticipationIdentity,
    binding_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingNavigationCommitParticipation {
    identity: NavigationCommitParticipationIdentity,
    delta: NavigationCommitParticipationDelta,
}

#[derive(Debug, Default)]
struct NavigationCommitParticipationState {
    current: Option<NavigationCommitParticipation>,
    pending: HashMap<u64, PendingNavigationCommitParticipation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavigationCommitParticipationError {
    Busy,
    GuardRequired,
    GuardMismatch,
    MissingClaim,
    Exhausted,
}

#[derive(Debug, Clone)]
pub(crate) struct NavigationCommitParticipationGate {
    state: Rc<RefCell<NavigationCommitParticipationState>>,
    live_host_preparation: Rc<Cell<Option<u64>>>,
}

impl NavigationCommitParticipationGate {
    fn effective_identity_and_count(
        state: &NavigationCommitParticipationState,
    ) -> Result<
        (Option<NavigationCommitParticipationIdentity>, usize),
        NavigationCommitParticipationError,
    > {
        let mut identity = state.current.map(|current| current.identity);
        let mut count = state.current.map_or(0, |current| current.binding_count);
        for pending in state.pending.values() {
            if identity.is_some_and(|identity| identity != pending.identity) {
                return Err(NavigationCommitParticipationError::GuardMismatch);
            }
            identity = Some(pending.identity);
            count = match pending.delta {
                NavigationCommitParticipationDelta::Add => count
                    .checked_add(1)
                    .ok_or(NavigationCommitParticipationError::Exhausted)?,
                NavigationCommitParticipationDelta::Remove => count
                    .checked_sub(1)
                    .ok_or(NavigationCommitParticipationError::MissingClaim)?,
            };
        }
        Ok((identity, count))
    }

    pub(crate) fn validate(
        &self,
        supplied: Option<NavigationCommitParticipationIdentity>,
    ) -> Result<Option<NavigationCommitParticipationIdentity>, NavigationCommitParticipationError>
    {
        let state = self.state.borrow();
        let (required, _) = Self::effective_identity_and_count(&state)?;
        let Some(required) = required else {
            return Ok(None);
        };
        let Some(supplied) = supplied else {
            return Err(NavigationCommitParticipationError::GuardRequired);
        };
        if supplied != required {
            return Err(NavigationCommitParticipationError::GuardMismatch);
        }
        Ok(Some(required))
    }

    pub(crate) fn prepare_add(
        &self,
        preparation_identity: u64,
        identity: NavigationCommitParticipationIdentity,
    ) -> Result<(), NavigationCommitParticipationError> {
        self.prepare_change(
            preparation_identity,
            identity,
            NavigationCommitParticipationDelta::Add,
        )
    }

    pub(crate) fn prepare_remove(
        &self,
        preparation_identity: u64,
        identity: NavigationCommitParticipationIdentity,
    ) -> Result<(), NavigationCommitParticipationError> {
        self.prepare_change(
            preparation_identity,
            identity,
            NavigationCommitParticipationDelta::Remove,
        )
    }

    fn prepare_change(
        &self,
        preparation_identity: u64,
        identity: NavigationCommitParticipationIdentity,
        delta: NavigationCommitParticipationDelta,
    ) -> Result<(), NavigationCommitParticipationError> {
        if self.live_host_preparation.get().is_some() {
            return Err(NavigationCommitParticipationError::Busy);
        }
        let mut state = self.state.borrow_mut();
        let (effective_identity, effective_count) = Self::effective_identity_and_count(&state)?;
        if effective_identity.is_some_and(|current| current != identity) {
            return Err(NavigationCommitParticipationError::GuardMismatch);
        }
        match delta {
            NavigationCommitParticipationDelta::Add => {
                effective_count
                    .checked_add(1)
                    .ok_or(NavigationCommitParticipationError::Exhausted)?;
            }
            NavigationCommitParticipationDelta::Remove if effective_count == 0 => {
                return Err(NavigationCommitParticipationError::MissingClaim);
            }
            NavigationCommitParticipationDelta::Remove => {}
        }
        let previous = state.pending.insert(
            preparation_identity,
            PendingNavigationCommitParticipation { identity, delta },
        );
        assert!(previous.is_none());
        Ok(())
    }

    pub(crate) fn commit(&self, preparation_identity: u64) {
        let mut state = self.state.borrow_mut();
        let pending = state
            .pending
            .remove(&preparation_identity)
            .expect("prepared navigation commit participation disappeared");
        match pending.delta {
            NavigationCommitParticipationDelta::Add => {
                let binding_count = match state.current {
                    Some(current) => {
                        assert_eq!(current.identity, pending.identity);
                        current
                            .binding_count
                            .checked_add(1)
                            .expect("navigation commit participation count exhausted")
                    }
                    None => 1,
                };
                state.current = Some(NavigationCommitParticipation {
                    identity: pending.identity,
                    binding_count,
                });
            }
            NavigationCommitParticipationDelta::Remove => {
                let current = state
                    .current
                    .expect("navigation commit participation claim disappeared");
                assert_eq!(current.identity, pending.identity);
                assert!(current.binding_count > 0);
                state.current = if current.binding_count == 1 {
                    None
                } else {
                    Some(NavigationCommitParticipation {
                        identity: current.identity,
                        binding_count: current.binding_count - 1,
                    })
                };
            }
        }
    }

    pub(crate) fn abort(&self, preparation_identity: u64) {
        self.state
            .borrow_mut()
            .pending
            .remove(&preparation_identity);
    }
}

#[derive(Debug, Clone)]
pub struct NavigationMechanicsSnapshot {
    host_identity: u64,
    history: Vec<NavigationState>,
    cursor: usize,
    generation: NavigationMechanicsGeneration,
}

impl NavigationMechanicsSnapshot {
    pub fn generation(&self) -> NavigationMechanicsGeneration {
        self.generation
    }

    pub fn state(&self) -> &NavigationState {
        &self.history[self.cursor]
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn history_cursor(&self) -> usize {
        self.cursor
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn history_state(&self, cursor: usize) -> Option<&NavigationState> {
        self.history.get(cursor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NavigationCandidateIdentity(u64);

#[derive(Debug)]
pub struct ResolvedNavigationCandidate {
    identity: NavigationCandidateIdentity,
    host_identity: u64,
    mechanics_generation: NavigationMechanicsGeneration,
    requested_intent: NavigationIntent,
    evaluated_intent: NavigationIntent,
    from: NavigationState,
    to: NavigationState,
    prospective_history: Vec<NavigationState>,
    prospective_cursor: usize,
}

impl ResolvedNavigationCandidate {
    pub fn mechanics_generation(&self) -> NavigationMechanicsGeneration {
        self.mechanics_generation
    }

    pub fn requested_intent(&self) -> &NavigationIntent {
        &self.requested_intent
    }

    pub fn evaluated_intent(&self) -> &NavigationIntent {
        &self.evaluated_intent
    }

    pub fn from(&self) -> &NavigationState {
        &self.from
    }

    pub fn to(&self) -> &NavigationState {
        &self.to
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn prospective_cursor(&self) -> usize {
        self.prospective_cursor
    }
}

#[derive(Debug)]
pub enum NavigationResolution {
    Candidate(ResolvedNavigationCandidate),
    Unavailable,
    NoOp,
    Retry,
}

#[derive(Debug)]
pub struct NavigationGuardAuthorization {
    candidate: ResolvedNavigationCandidate,
    guard_context_generation: Option<NavigationGuardContextGeneration>,
    redirected: bool,
}

impl NavigationGuardAuthorization {
    pub fn candidate(&self) -> &ResolvedNavigationCandidate {
        &self.candidate
    }
}

#[derive(Debug)]
pub enum NavigationAuthorizationResult {
    Authorized(NavigationGuardAuthorization),
    Denied,
    Unavailable,
    NoOp,
    Retry,
}

static NEXT_HOST_IDENTITY: AtomicU64 = AtomicU64::new(1);
static NEXT_CANDIDATE_IDENTITY: AtomicU64 = AtomicU64::new(1);
static NEXT_LEASE_IDENTITY: AtomicU64 = AtomicU64::new(1);

fn allocate_identity(counter: &AtomicU64) -> Option<u64> {
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .ok()
}

/// Single source of truth for one context's navigation mechanics.
pub struct NavigationHost {
    host_identity: u64,
    mechanics: Rc<RefCell<NavigationMechanics>>,
    live_preparation: Rc<Cell<Option<u64>>>,
    commit_participation: NavigationCommitParticipationGate,
    guard: Option<Box<dyn NavigationGuard>>,
}

const MAX_GUARD_EVALUATIONS: usize = 8;

impl NavigationHost {
    pub fn new(root: impl Into<NavigationEntry>) -> Self {
        let root = root.into();
        let root_state = NavigationState::from_valid_path(vec![root]);
        let live_preparation = Rc::new(Cell::new(None));
        Self {
            host_identity: allocate_identity(&NEXT_HOST_IDENTITY)
                .expect("NavigationHost identity space exhausted"),
            mechanics: Rc::new(RefCell::new(NavigationMechanics {
                history: vec![root_state],
                cursor: 0,
                generation: NavigationMechanicsGeneration(1),
            })),
            commit_participation: NavigationCommitParticipationGate {
                state: Rc::new(RefCell::new(NavigationCommitParticipationState::default())),
                live_host_preparation: Rc::clone(&live_preparation),
            },
            live_preparation,
            guard: None,
        }
    }

    pub(crate) fn commit_participation_gate(&self) -> NavigationCommitParticipationGate {
        self.commit_participation.clone()
    }

    pub fn set_guard(&mut self, guard: Box<dyn NavigationGuard>) {
        self.guard = Some(guard);
    }

    pub fn clear_guard(&mut self) {
        self.guard = None;
    }

    pub fn snapshot(&self) -> Result<NavigationMechanicsSnapshot, NavigationSnapshotError> {
        let mechanics = self.mechanics.borrow();
        Ok(NavigationMechanicsSnapshot {
            host_identity: self.host_identity,
            history: mechanics.history.clone(),
            cursor: mechanics.cursor,
            generation: mechanics.generation,
        })
    }

    pub fn resolve(
        &self,
        snapshot: &NavigationMechanicsSnapshot,
        intent: NavigationIntent,
    ) -> NavigationResolution {
        if snapshot.host_identity != self.host_identity {
            return NavigationResolution::Retry;
        }
        Self::resolve_from_snapshot(snapshot, intent.clone(), intent, None)
    }

    pub fn authorize(
        &self,
        snapshot: &NavigationMechanicsSnapshot,
        intent: NavigationIntent,
        context: Option<&dyn Any>,
        guard_context_generation: Option<NavigationGuardContextGeneration>,
    ) -> NavigationAuthorizationResult {
        match self.resolve(snapshot, intent) {
            NavigationResolution::Candidate(candidate) => {
                self.authorize_candidate(snapshot, candidate, context, guard_context_generation)
            }
            NavigationResolution::Unavailable => NavigationAuthorizationResult::Unavailable,
            NavigationResolution::NoOp => NavigationAuthorizationResult::NoOp,
            NavigationResolution::Retry => NavigationAuthorizationResult::Retry,
        }
    }

    /// One directional intent to a confirmed cursor, including multi-entry
    /// traversals. The caller qualifies the external observation separately.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn authorize_history_cursor(
        &self,
        snapshot: &NavigationMechanicsSnapshot,
        cursor: usize,
    ) -> NavigationAuthorizationResult {
        use std::cmp::Ordering::{Equal, Greater, Less};

        if snapshot.host_identity != self.host_identity {
            return NavigationAuthorizationResult::Retry;
        }
        let intent = match cursor.cmp(&snapshot.cursor) {
            Equal => return NavigationAuthorizationResult::NoOp,
            Less => NavigationIntent::HistoryBack,
            Greater => NavigationIntent::HistoryForward,
        };
        match Self::resolve_from_snapshot(snapshot, intent.clone(), intent, Some(cursor)) {
            NavigationResolution::Candidate(candidate) => {
                self.authorize_candidate(snapshot, candidate, None, None)
            }
            NavigationResolution::Unavailable => NavigationAuthorizationResult::Unavailable,
            NavigationResolution::NoOp => NavigationAuthorizationResult::NoOp,
            NavigationResolution::Retry => NavigationAuthorizationResult::Retry,
        }
    }

    /// Admit the initial route against the normal guard before any projection
    /// is published. The root remains the structural ancestor, with one
    /// initial committed history slot.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn authorize_initial_route(
        &self,
        snapshot: &NavigationMechanicsSnapshot,
        entry: NavigationEntry,
    ) -> NavigationAuthorizationResult {
        if snapshot.generation != NavigationMechanicsGeneration(1)
            || snapshot.history.len() != 1
        {
            return NavigationAuthorizationResult::Unavailable;
        }
        match self.authorize(snapshot, NavigationIntent::NavigateTo(entry), None, None) {
            NavigationAuthorizationResult::Authorized(mut authorization) => {
                let candidate = &mut authorization.candidate;
                candidate.prospective_history = vec![candidate.to.clone()];
                candidate.prospective_cursor = 0;
                NavigationAuthorizationResult::Authorized(authorization)
            }
            refusal => refusal,
        }
    }

    pub fn authorize_candidate(
        &self,
        snapshot: &NavigationMechanicsSnapshot,
        mut candidate: ResolvedNavigationCandidate,
        context: Option<&dyn Any>,
        guard_context_generation: Option<NavigationGuardContextGeneration>,
    ) -> NavigationAuthorizationResult {
        if snapshot.host_identity != self.host_identity
            || candidate.host_identity != self.host_identity
            || candidate.mechanics_generation != snapshot.generation
            || candidate.requested_intent != candidate.evaluated_intent
        {
            return NavigationAuthorizationResult::Retry;
        }

        let requested_intent = candidate.requested_intent.clone();
        let mut redirected = false;

        for evaluation_index in 0..MAX_GUARD_EVALUATIONS {
            let decision = match &self.guard {
                Some(guard) => guard.evaluate(
                    &requested_intent,
                    &candidate.evaluated_intent,
                    &candidate.from,
                    &candidate.to,
                    context,
                ),
                None => GuardDecision::Allow,
            };

            match decision {
                GuardDecision::Allow => {
                    return NavigationAuthorizationResult::Authorized(
                        NavigationGuardAuthorization {
                            candidate,
                            guard_context_generation,
                            redirected,
                        },
                    );
                }
                GuardDecision::Deny => return NavigationAuthorizationResult::Denied,
                GuardDecision::ReplaceWith(replacement) => {
                    if evaluation_index + 1 == MAX_GUARD_EVALUATIONS {
                        return NavigationAuthorizationResult::Denied;
                    }
                    redirected = true;
                    candidate = match Self::resolve_from_snapshot(
                        snapshot,
                        requested_intent.clone(),
                        replacement,
                        None,
                    ) {
                        NavigationResolution::Candidate(candidate) => candidate,
                        NavigationResolution::Unavailable => {
                            return NavigationAuthorizationResult::Unavailable;
                        }
                        NavigationResolution::NoOp => {
                            return NavigationAuthorizationResult::NoOp;
                        }
                        NavigationResolution::Retry => {
                            return NavigationAuthorizationResult::Retry;
                        }
                    };
                }
            }
        }

        NavigationAuthorizationResult::Denied
    }

    pub fn prepare_commit(
        &self,
        authorization: NavigationGuardAuthorization,
        expected_generation: NavigationMechanicsGeneration,
        current_guard_context_generation: Option<NavigationGuardContextGeneration>,
    ) -> Result<PreparedNavigationCommit, NavigationPreparationError> {
        self.prepare_commit_inner(
            authorization,
            expected_generation,
            current_guard_context_generation,
            None,
        )
    }

    pub(crate) fn prepare_commit_guarded(
        &self,
        authorization: NavigationGuardAuthorization,
        expected_generation: NavigationMechanicsGeneration,
        current_guard_context_generation: Option<NavigationGuardContextGeneration>,
        participation: NavigationCommitParticipationIdentity,
    ) -> Result<PreparedNavigationCommit, NavigationPreparationError> {
        self.prepare_commit_inner(
            authorization,
            expected_generation,
            current_guard_context_generation,
            Some(participation),
        )
    }

    fn prepare_commit_inner(
        &self,
        authorization: NavigationGuardAuthorization,
        expected_generation: NavigationMechanicsGeneration,
        current_guard_context_generation: Option<NavigationGuardContextGeneration>,
        participation: Option<NavigationCommitParticipationIdentity>,
    ) -> Result<PreparedNavigationCommit, NavigationPreparationError> {
        if authorization.candidate.host_identity != self.host_identity {
            return Err(NavigationPreparationError::WrongHost);
        }
        if authorization.guard_context_generation != current_guard_context_generation {
            return Err(NavigationPreparationError::StaleGuardContext);
        }
        if self.live_preparation.get().is_some() {
            return Err(NavigationPreparationError::Busy);
        }
        self.commit_participation
            .validate(participation)
            .map_err(|error| match error {
                NavigationCommitParticipationError::GuardRequired => {
                    NavigationPreparationError::FrameProjectionGuardRequired
                }
                NavigationCommitParticipationError::GuardMismatch
                | NavigationCommitParticipationError::MissingClaim => {
                    NavigationPreparationError::FrameProjectionGuardMismatch
                }
                NavigationCommitParticipationError::Busy => NavigationPreparationError::Busy,
                NavigationCommitParticipationError::Exhausted => {
                    NavigationPreparationError::Exhausted
                }
            })?;

        let mechanics = self.mechanics.borrow();
        if mechanics.generation != expected_generation
            || authorization.candidate.mechanics_generation != expected_generation
        {
            return Err(NavigationPreparationError::StaleGeneration);
        }
        let next_generation = mechanics
            .generation
            .checked_next()
            .ok_or(NavigationPreparationError::Exhausted)?;
        drop(mechanics);

        let lease_identity =
            allocate_identity(&NEXT_LEASE_IDENTITY).ok_or(NavigationPreparationError::Exhausted)?;
        self.live_preparation.set(Some(lease_identity));

        let candidate = authorization.candidate;
        Ok(PreparedNavigationCommit {
            mechanics: Rc::clone(&self.mechanics),
            live_preparation: Rc::clone(&self.live_preparation),
            lease_identity,
            expected_generation,
            next_generation,
            candidate_identity: candidate.identity,
            from: candidate.from,
            to: candidate.to,
            prospective_history: Some(candidate.prospective_history),
            prospective_cursor: candidate.prospective_cursor,
            redirected: authorization.redirected,
            completed: false,
        })
    }

    pub fn execute(
        &self,
        intent: NavigationIntent,
        context: Option<&dyn Any>,
        guard_context_generation: Option<NavigationGuardContextGeneration>,
    ) -> Result<NavigationOutcome, NavigationExecutionError> {
        let snapshot = self
            .snapshot()
            .map_err(|_| NavigationExecutionError::Retry)?;
        match self.authorize(&snapshot, intent, context, guard_context_generation) {
            NavigationAuthorizationResult::Authorized(authorization) => {
                let prepared = self
                    .prepare_commit(
                        authorization,
                        snapshot.generation(),
                        guard_context_generation,
                    )
                    .map_err(|_| NavigationExecutionError::Retry)?;
                Ok(prepared.commit())
            }
            NavigationAuthorizationResult::Denied => Ok(NavigationOutcome::Denied),
            NavigationAuthorizationResult::Unavailable => Ok(NavigationOutcome::Unavailable),
            NavigationAuthorizationResult::NoOp => Ok(NavigationOutcome::NoOp),
            NavigationAuthorizationResult::Retry => Err(NavigationExecutionError::Retry),
        }
    }

    /// Legacy standalone façade. Compound surface/focus routing must use prepared APIs.
    pub fn decide(&mut self, intent: NavigationIntent) -> NavigationDecision {
        self.decide_guarded(intent, None)
    }

    /// Legacy standalone façade with opaque guard context.
    pub fn decide_with_context(
        &mut self,
        intent: NavigationIntent,
        context: &dyn Any,
    ) -> NavigationDecision {
        self.decide_guarded(intent, Some(context))
    }

    fn decide_guarded(
        &self,
        intent: NavigationIntent,
        context: Option<&dyn Any>,
    ) -> NavigationDecision {
        match self.execute(intent, context, None) {
            Ok(NavigationOutcome::Committed { redirected: false }) => NavigationDecision::Allow,
            Ok(NavigationOutcome::Committed { redirected: true }) => NavigationDecision::Redirect,
            Ok(NavigationOutcome::Denied) => NavigationDecision::Deny,
            Ok(NavigationOutcome::Unavailable) => NavigationDecision::Unavailable,
            Ok(NavigationOutcome::NoOp) => NavigationDecision::NoOp,
            Err(NavigationExecutionError::Retry) => NavigationDecision::Retry,
        }
    }

    /// Legacy standalone façade returning the resulting committed state.
    pub fn apply(&mut self, intent: NavigationIntent) -> NavigationState {
        let _ = self.decide(intent);
        self.state()
    }

    /// Legacy standalone façade returning the resulting committed state.
    pub fn apply_with_context(
        &mut self,
        intent: NavigationIntent,
        context: &dyn Any,
    ) -> NavigationState {
        let _ = self.decide_with_context(intent, context);
        self.state()
    }

    pub fn state(&self) -> NavigationState {
        self.mechanics.borrow().current_state().clone()
    }

    pub fn current_entry(&self) -> NavigationEntry {
        self.mechanics.borrow().current_state().current().clone()
    }

    pub fn current(&self) -> NavigationNodeId {
        self.mechanics.borrow().current_state().current().node_id()
    }

    pub fn path_entries(&self) -> Vec<NavigationEntry> {
        self.mechanics.borrow().current_state().path().to_vec()
    }

    pub fn path(&self) -> Vec<NavigationNodeId> {
        self.mechanics
            .borrow()
            .current_state()
            .path()
            .iter()
            .map(NavigationEntry::node_id)
            .collect()
    }

    pub fn control_projection(&self) -> NavigationControlProjection {
        let mechanics = self.mechanics.borrow();
        let state = mechanics.current_state();
        NavigationControlProjection {
            path: state.path().to_vec(),
            current: state.current().clone(),
            can_navigate_up: state.path().len() > 1,
            can_history_back: mechanics.cursor > 0,
            can_history_forward: mechanics.cursor + 1 < mechanics.history.len(),
        }
    }

    fn resolve_from_snapshot(
        snapshot: &NavigationMechanicsSnapshot,
        requested_intent: NavigationIntent,
        evaluated_intent: NavigationIntent,
        exact_history_cursor: Option<usize>,
    ) -> NavigationResolution {
        let from = snapshot.state().clone();
        let history_cursor = match &evaluated_intent {
            NavigationIntent::HistoryBack => {
                exact_history_cursor.or_else(|| snapshot.cursor.checked_sub(1))
            }
            NavigationIntent::HistoryForward => {
                exact_history_cursor.or_else(|| snapshot.cursor.checked_add(1))
            }
            _ => None,
        };
        let to = match &evaluated_intent {
            NavigationIntent::NavigateTo(entry) => {
                if from.current() == entry {
                    return NavigationResolution::NoOp;
                }
                let mut path = from.path().to_vec();
                if let Some(position) = path.iter().rposition(|candidate| candidate == entry) {
                    path.truncate(position + 1);
                } else {
                    path.push(entry.clone());
                }
                NavigationState::from_valid_path(path)
            }
            NavigationIntent::NavigatePath(path) => {
                if path.is_empty() || path.first() != snapshot.history[0].path().first() {
                    return NavigationResolution::Unavailable;
                }
                if path
                    .iter()
                    .enumerate()
                    .any(|(index, entry)| path[..index].contains(entry))
                {
                    return NavigationResolution::Unavailable;
                }
                if path.as_slice() == from.path() {
                    return NavigationResolution::NoOp;
                }
                NavigationState::from_valid_path(path.clone())
            }
            NavigationIntent::NavigateUp => {
                if from.path().len() == 1 {
                    return NavigationResolution::Unavailable;
                }
                let mut path = from.path().to_vec();
                path.pop();
                NavigationState::from_valid_path(path)
            }
            #[allow(deprecated)]
            NavigationIntent::NavigateBack => {
                if from.path().len() == 1 {
                    return NavigationResolution::Unavailable;
                }
                let mut path = from.path().to_vec();
                path.pop();
                NavigationState::from_valid_path(path)
            }
            NavigationIntent::HistoryBack | NavigationIntent::HistoryForward => {
                let Some(state) = history_cursor.and_then(|cursor| snapshot.history.get(cursor))
                else {
                    return NavigationResolution::Unavailable;
                };
                state.clone()
            }
        };

        let (prospective_history, prospective_cursor) = if let Some(cursor) = history_cursor {
            (snapshot.history.clone(), cursor)
        } else {
            let mut history = snapshot.history[..=snapshot.cursor].to_vec();
            history.push(to.clone());
            let cursor = history.len() - 1;
            (history, cursor)
        };

        let Some(identity) = allocate_identity(&NEXT_CANDIDATE_IDENTITY) else {
            return NavigationResolution::Retry;
        };

        NavigationResolution::Candidate(ResolvedNavigationCandidate {
            identity: NavigationCandidateIdentity(identity),
            host_identity: snapshot.host_identity,
            mechanics_generation: snapshot.generation,
            requested_intent,
            evaluated_intent,
            from,
            to,
            prospective_history,
            prospective_cursor,
        })
    }
}

/// Single-use non-fallible host mechanics commit prepared from exact authorization.
#[derive(Debug)]
pub struct PreparedNavigationCommit {
    mechanics: Rc<RefCell<NavigationMechanics>>,
    live_preparation: Rc<Cell<Option<u64>>>,
    lease_identity: u64,
    expected_generation: NavigationMechanicsGeneration,
    next_generation: NavigationMechanicsGeneration,
    candidate_identity: NavigationCandidateIdentity,
    from: NavigationState,
    to: NavigationState,
    prospective_history: Option<Vec<NavigationState>>,
    prospective_cursor: usize,
    redirected: bool,
    completed: bool,
}

impl PreparedNavigationCommit {
    pub fn from(&self) -> &NavigationState {
        &self.from
    }

    pub fn to(&self) -> &NavigationState {
        &self.to
    }

    pub fn expected_generation(&self) -> NavigationMechanicsGeneration {
        self.expected_generation
    }

    pub fn commit(mut self) -> NavigationOutcome {
        debug_assert_eq!(self.live_preparation.get(), Some(self.lease_identity));
        let mut mechanics = self.mechanics.borrow_mut();
        debug_assert_eq!(mechanics.generation, self.expected_generation);
        mechanics.history = self
            .prospective_history
            .take()
            .expect("prepared navigation commit is single-use");
        mechanics.cursor = self.prospective_cursor;
        mechanics.generation = self.next_generation;
        drop(mechanics);

        self.live_preparation.set(None);
        self.completed = true;
        let _ = self.candidate_identity;
        NavigationOutcome::Committed {
            redirected: self.redirected,
        }
    }

    pub fn abort(self) {}
}

impl Drop for PreparedNavigationCommit {
    fn drop(&mut self) {
        if !self.completed && self.live_preparation.get() == Some(self.lease_identity) {
            self.live_preparation.set(None);
        }
    }
}
