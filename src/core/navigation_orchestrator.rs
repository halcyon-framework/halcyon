use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::compositor::{
    Compositor, CompositorIdentity, PreparedSurfaceBatch, SceneKey, SurfaceBatchPreparationError,
    SurfaceId as CompositorSurfaceId, SurfaceMutationBatch, SurfaceVisibility,
};
use crate::core::math::RectF;
use crate::core::overlay_domain_coordinator::{
    DomainTransitionGuardToken, OverlayDisposition, OverlayOwnerId, OverlayStackId,
    PreparedOverlaySurfaceExitTestimony,
};
use crate::model::navigation_host::{
    NavigationAuthorizationResult, NavigationCommitParticipationError,
    NavigationCommitParticipationGate, NavigationCommitParticipationIdentity, NavigationDecision,
    NavigationEntry, NavigationGuardAuthorization, NavigationGuardContextGeneration,
    NavigationHost, NavigationIntent, NavigationMechanicsGeneration, NavigationNodeId,
    NavigationOutcome, NavigationPreparationError, PreparedNavigationCommit,
};

/// Allocates unique SceneKeys for compositor surfaces.
pub trait SceneKeyAllocator {
    fn allocate(&mut self) -> SceneKey;
}

/// Default allocator: increments from `SceneKey::BASE.get() + 1`.
pub struct IncrementalSceneKeyAllocator {
    next: u64,
}

impl IncrementalSceneKeyAllocator {
    pub fn new() -> Self {
        Self {
            next: SceneKey::BASE.get() + 1,
        }
    }
}

impl SceneKeyAllocator for IncrementalSceneKeyAllocator {
    fn allocate(&mut self) -> SceneKey {
        let key = SceneKey::new(self.next);
        self.next += 1;
        key
    }
}

/// Describes the step transition resulting from a navigation dispatch.
/// Mechanically derived from (old_path, new_path) prefix-diff (SPEC-HAL-233).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepTransition {
    /// Path extended by exactly one node. Forward progression (step N → N+1).
    Advance,
    /// Path shortened by exactly one node. Backward regression (step N → N-1).
    Retreat,
    /// Path diverged beyond single-step advance/retreat.
    /// Covers multi-step jumps and redirected replacements.
    Replace {
        /// Nodes removed from old path tail.
        removed: usize,
        /// Nodes added to new path tail.
        added: usize,
    },
    /// Paths identical. No surface mutation.
    NoOp,
}

/// Focus transition resulting from a navigation dispatch (SPEC-HAL-235).
/// Mechanically derived from (old_focused, new_focused) after reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTransition {
    /// Surface gained focus (no prior focus holder).
    Gained { surface: CompositorSurfaceId },
    /// Surface lost focus (no new focus holder).
    Lost { surface: CompositorSurfaceId },
    /// Focus moved from one surface to another.
    Transferred {
        from: CompositorSurfaceId,
        to: CompositorSurfaceId,
    },
    /// All focus removed. Unused until orchestrator shutdown API exists (future SPEC).
    Cleared,
}

/// Overlay kind for navigation-scoped overlay surfaces (SPEC-HAL-236).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Modal,
    Popover,
    ContextMenu,
}

/// Per-root z-band size. Each root gets Z_BAND_SIZE z-indices for navigation + overlay surfaces (SPEC-HAL-237).
const Z_BAND_SIZE: i32 = 1000;

/// Offset within a root's z-band where overlay surfaces begin.
/// Navigation surfaces: [z_base, z_base + OVERLAY_Z_OFFSET)
/// Overlay surfaces: [z_base + OVERLAY_Z_OFFSET, z_base + Z_BAND_SIZE)
const OVERLAY_Z_OFFSET: i32 = 500;

/// Unique identifier for a navigation root in a multiroot registry (SPEC-HAL-237).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavigationRootId(pub u32);

/// Why a navigation-owned surface was terminated (SPEC-HAL-234).
/// Orchestrator-local: not exposed to compositor. Used for enforcement and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceTerminationReason {
    /// Single step backward (removed == 1, added == 0).
    StepRetreat,
    /// Multi-step path change (removed > 1, or removed > 0 with added > 0).
    StepReplace,
    /// Path collapsed to root only.
    RootReset,
    /// Host teardown (future: explicit orchestrator shutdown).
    HostTeardown,
    /// Overlay explicitly closed via close_overlay() (SPEC-HAL-236).
    OverlayClosed,
    /// Overlay auto-terminated by navigation path change (SPEC-HAL-236 INV-236-N1).
    OverlayReplaced,
}

/// Result of an orchestrated navigation dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestratedNavigationResult {
    pub decision: NavigationDecision,
    pub current_node: NavigationNodeId,
    pub active_surface: Option<CompositorSurfaceId>,
    pub step_transition: StepTransition,
    pub terminated_surfaces: Vec<(CompositorSurfaceId, SurfaceTerminationReason)>,
    pub focus_transition: Option<FocusTransition>,
}

/// Read-only projection of orchestrator state for passive consumers (SPEC-HAL-246).
/// Pure snapshot. No authority. No behavior. No mutation capability.
/// Reconstructible every frame without loss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationProjection {
    /// Ordered navigation surfaces from path bottom to top.
    /// Index 0 = root surface. Index N-1 = active navigation surface.
    /// Directly sourced from the committed projection path through the
    /// entry-to-surface map in path order.
    pub navigation_surfaces: Vec<CompositorSurfaceId>,
    /// Currently active (topmost visible) navigation surface.
    /// Directly sourced from orchestrator's active_surface field.
    pub active_surface: Option<CompositorSurfaceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationProjectionError {
    SourceUnavailable,
    Busy,
    PathMappingMismatch,
    ActiveSurfaceMismatch,
}

/// Checked, non-reused generation of one orchestrator's lifecycle/mapping state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NavigationOrchestrationGeneration(u64);

impl NavigationOrchestrationGeneration {
    pub fn get(self) -> u64 {
        self.0
    }

    fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Semantic lifecycle state owned by NavigationSurfaceOrchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationSurfaceLifecycle {
    Created,
    Active,
    Inactive,
    Terminating,
    Destroyed,
}

#[derive(Debug, Clone)]
struct NavigationSurfaceRecord {
    surface: CompositorSurfaceId,
    lifecycle: NavigationSurfaceLifecycle,
}

#[derive(Debug, Clone)]
struct NavigationOverlayRecord {
    surface: CompositorSurfaceId,
    kind: OverlayKind,
    host_surface: CompositorSurfaceId,
}

#[derive(Debug)]
struct NavigationSurfaceMechanics {
    // The ordinary App owns one render target for the whole window.
    window_surface: Option<CompositorSurfaceId>,
    projection_path: Vec<NavigationEntry>,
    entry_surface_map: HashMap<NavigationEntry, NavigationSurfaceRecord>,
    active_surface: Option<CompositorSurfaceId>,
    focused_surface: Option<CompositorSurfaceId>,
    overlay_stack: Vec<NavigationOverlayRecord>,
    generation: NavigationOrchestrationGeneration,
    live_preparation: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationSurfacePreparationError {
    Busy,
    WrongCompositor,
    StaleGeneration,
    FrameProjectionGuardRequired,
    FrameProjectionGuardMismatch,
    HostStateMismatch,
    RootMutationForbidden,
    MissingSurface(CompositorSurfaceId),
    ExternalOverlayAttached {
        surface: CompositorSurfaceId,
        overlay: CompositorSurfaceId,
    },
    OverlayExitSurfaceMissing {
        owner_id: OverlayOwnerId,
        stack_id: OverlayStackId,
        surface: CompositorSurfaceId,
    },
    OverlayExitSurfaceOutsideRoot {
        owner_id: OverlayOwnerId,
        stack_id: OverlayStackId,
        surface: CompositorSurfaceId,
    },
    OverlayExitOwnershipConflict(CompositorSurfaceId),
    MechanicalBatch(SurfaceBatchPreparationError),
    Exhausted,
}

impl From<SurfaceBatchPreparationError> for NavigationSurfacePreparationError {
    fn from(error: SurfaceBatchPreparationError) -> Self {
        Self::MechanicalBatch(error)
    }
}

/// Opaque identity of the exact prepared lifecycle result that may be published.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceTransitionPublicationBinding {
    transition_identity: u64,
    generation: NavigationOrchestrationGeneration,
}

/// Sealed proof that external overlay/focus/binding detach or replacement has
/// entered the same non-fallible publication sequence.
#[derive(Debug)]
pub(crate) struct SurfaceTransitionPublicationCapability {
    binding: SurfaceTransitionPublicationBinding,
    participation: Option<NavigationCommitParticipationIdentity>,
}

impl SurfaceTransitionPublicationCapability {
    pub(crate) fn new(
        prepared: &PreparedNavigationSurfaceTransition<'_>,
        guard: Option<&DomainTransitionGuardToken>,
    ) -> Self {
        let participation = prepared.frame_projection_participation;
        if let Some(required) = participation {
            assert_eq!(
                guard.map(navigation_commit_participation_identity),
                Some(required),
                "surface publication guard does not match frame participation"
            );
        }
        Self {
            binding: prepared.publication_binding(),
            participation,
        }
    }

    pub(crate) fn for_overlay_exit(binding: SurfaceTransitionPublicationBinding) -> Self {
        Self {
            binding,
            participation: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommittedNavigationSurfaceTransition {
    pub active_surface: Option<CompositorSurfaceId>,
    pub step_transition: StepTransition,
    pub terminated_surfaces: Vec<(CompositorSurfaceId, SurfaceTerminationReason)>,
    pub focus_transition: Option<FocusTransition>,
}

/// Bridges NavigationHost decisions to compositor surface mutations.
///
/// Translates Allow/Deny/Redirect decisions from the navigation model layer
/// into real compositor surface create/destroy/visibility changes, without
/// violating compositor semantic-freedom (SPEC-HAL-150).
///
/// NavigationHost is semantic-only (state + guard). This orchestrator is
/// the sole owner of compositor surface lifecycle for navigation.
pub struct NavigationSurfaceOrchestrator {
    nav_host: NavigationHost,
    mechanics: Rc<RefCell<NavigationSurfaceMechanics>>,
    liveness: Rc<()>,
    compositor_identity: CompositorIdentity,
    z_base: i32,
    scene_key_allocator: Box<dyn SceneKeyAllocator>,
    viewport: RectF,
}

#[derive(Clone)]
pub struct NavigationSurfaceOrchestratorHandle {
    mechanics: Rc<RefCell<NavigationSurfaceMechanics>>,
    commit_participation: NavigationCommitParticipationGate,
    liveness: Weak<()>,
    compositor_identity: CompositorIdentity,
}

impl std::fmt::Debug for NavigationSurfaceOrchestratorHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NavigationSurfaceOrchestratorHandle")
            .field("identity", &(Rc::as_ptr(&self.mechanics) as usize))
            .field("compositor_identity", &self.compositor_identity)
            .finish()
    }
}

impl PartialEq for NavigationSurfaceOrchestratorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.compositor_identity == other.compositor_identity
            && Rc::ptr_eq(&self.mechanics, &other.mechanics)
    }
}

impl Eq for NavigationSurfaceOrchestratorHandle {}

impl NavigationSurfaceOrchestratorHandle {
    pub const fn compositor_identity(&self) -> CompositorIdentity {
        self.compositor_identity
    }

    pub(crate) fn is_live(&self) -> bool {
        if self.liveness.upgrade().is_none() {
            return false;
        }
        let mechanics = self.mechanics.borrow();
        !mechanics.entry_surface_map.is_empty() && mechanics.active_surface.is_some()
    }

    pub(crate) fn prepare_frame_projection_participation_add(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedFrameProjectionParticipation, FrameProjectionParticipationError> {
        if !self.is_live() {
            return Err(FrameProjectionParticipationError::SourceUnavailable);
        }
        self.prepare_frame_projection_participation(guard, true)
    }

    pub(crate) fn prepare_frame_projection_participation_remove(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedFrameProjectionParticipation, FrameProjectionParticipationError> {
        self.prepare_frame_projection_participation(guard, false)
    }

    fn prepare_frame_projection_participation(
        &self,
        guard: &DomainTransitionGuardToken,
        add: bool,
    ) -> Result<PreparedFrameProjectionParticipation, FrameProjectionParticipationError> {
        let preparation_identity = allocate_surface_transition_identity()
            .ok_or(FrameProjectionParticipationError::Exhausted)?;
        if self.mechanics.borrow().live_preparation.is_some() {
            return Err(FrameProjectionParticipationError::Busy);
        }
        let identity = navigation_commit_participation_identity(guard);
        let result = if add {
            self.commit_participation
                .prepare_add(preparation_identity, identity)
        } else {
            self.commit_participation
                .prepare_remove(preparation_identity, identity)
        };
        result.map_err(FrameProjectionParticipationError::from_commit_error)?;
        Ok(PreparedFrameProjectionParticipation {
            commit_participation: self.commit_participation.clone(),
            preparation_identity,
            committed: false,
        })
    }

    pub fn project(&self) -> Result<NavigationProjection, NavigationProjectionError> {
        if self.liveness.upgrade().is_none() {
            return Err(NavigationProjectionError::SourceUnavailable);
        }
        let mechanics = self
            .mechanics
            .try_borrow()
            .map_err(|_| NavigationProjectionError::Busy)?;
        Self::project_mechanics(&mechanics)
    }

    fn project_mechanics(
        mechanics: &NavigationSurfaceMechanics,
    ) -> Result<NavigationProjection, NavigationProjectionError> {
        if mechanics.live_preparation.is_some() {
            return Err(NavigationProjectionError::Busy);
        }
        if mechanics.projection_path.is_empty()
            || mechanics.entry_surface_map.len() != mechanics.projection_path.len()
        {
            return Err(NavigationProjectionError::PathMappingMismatch);
        }
        let navigation_surfaces = mechanics
            .projection_path
            .iter()
            .map(|entry| {
                mechanics
                    .entry_surface_map
                    .get(entry)
                    .map(|record| record.surface)
                    .ok_or(NavigationProjectionError::PathMappingMismatch)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if mechanics.active_surface != navigation_surfaces.last().copied() {
            return Err(NavigationProjectionError::ActiveSurfaceMismatch);
        }
        if let Some(surface) = mechanics.window_surface {
            if navigation_surfaces.iter().any(|candidate| *candidate != surface) {
                return Err(NavigationProjectionError::PathMappingMismatch);
            }
        }
        Ok(NavigationProjection {
            navigation_surfaces,
            active_surface: mechanics.active_surface,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameProjectionParticipationError {
    SourceUnavailable,
    Busy,
    GuardMismatch,
    MissingClaim,
    Exhausted,
}

impl FrameProjectionParticipationError {
    fn from_commit_error(error: NavigationCommitParticipationError) -> Self {
        match error {
            NavigationCommitParticipationError::Busy => Self::Busy,
            NavigationCommitParticipationError::GuardRequired
            | NavigationCommitParticipationError::GuardMismatch => Self::GuardMismatch,
            NavigationCommitParticipationError::MissingClaim => Self::MissingClaim,
            NavigationCommitParticipationError::Exhausted => Self::Exhausted,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PreparedFrameProjectionParticipation {
    commit_participation: NavigationCommitParticipationGate,
    preparation_identity: u64,
    committed: bool,
}

impl PreparedFrameProjectionParticipation {
    pub(crate) fn commit(mut self) {
        self.commit_participation.commit(self.preparation_identity);
        self.committed = true;
    }
}

impl Drop for PreparedFrameProjectionParticipation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        self.commit_participation.abort(self.preparation_identity);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameProjectionPublicationGuardError {
    Required,
    Mismatch,
}

fn validate_frame_projection_publication_guard(
    commit_participation: &NavigationCommitParticipationGate,
    guard: Option<&DomainTransitionGuardToken>,
) -> Result<Option<NavigationCommitParticipationIdentity>, FrameProjectionPublicationGuardError> {
    commit_participation
        .validate(guard.map(navigation_commit_participation_identity))
        .map_err(|error| match error {
            NavigationCommitParticipationError::GuardRequired => {
                FrameProjectionPublicationGuardError::Required
            }
            NavigationCommitParticipationError::Busy
            | NavigationCommitParticipationError::GuardMismatch
            | NavigationCommitParticipationError::MissingClaim
            | NavigationCommitParticipationError::Exhausted => {
                FrameProjectionPublicationGuardError::Mismatch
            }
        })
}

fn navigation_commit_participation_identity(
    guard: &DomainTransitionGuardToken,
) -> NavigationCommitParticipationIdentity {
    NavigationCommitParticipationIdentity::new(
        guard.coordinator_identity(),
        guard.interaction_domain_id().0,
    )
}

static NEXT_SURFACE_TRANSITION_IDENTITY: AtomicU64 = AtomicU64::new(1);

fn allocate_surface_transition_identity() -> Option<u64> {
    NEXT_SURFACE_TRANSITION_IDENTITY
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .ok()
}

impl NavigationSurfaceOrchestrator {
    pub fn projection_handle(&self) -> NavigationSurfaceOrchestratorHandle {
        NavigationSurfaceOrchestratorHandle {
            mechanics: Rc::clone(&self.mechanics),
            commit_participation: self.nav_host.commit_participation_gate(),
            liveness: Rc::downgrade(&self.liveness),
            compositor_identity: self.compositor_identity,
        }
    }

    pub const fn compositor_identity(&self) -> CompositorIdentity {
        self.compositor_identity
    }

    /// Create an orchestrator and its initially active root surface.
    pub fn new(
        nav_host: NavigationHost,
        viewport: RectF,
        compositor: &mut Compositor,
        scene_key_allocator: Box<dyn SceneKeyAllocator>,
    ) -> Self {
        Self::new_with_z_base(nav_host, viewport, compositor, scene_key_allocator, 0)
    }

    /// Bind an existing compositor surface as the root entry's surface.
    pub fn new_with_existing_root_surface(
        nav_host: NavigationHost,
        viewport: RectF,
        root_surface: CompositorSurfaceId,
        compositor: &mut Compositor,
        mut scene_key_allocator: Box<dyn SceneKeyAllocator>,
    ) -> Self {
        assert!(
            compositor.has_surface(root_surface),
            "existing navigation root surface must exist"
        );

        let initial_path = nav_host.path_entries();
        let last_index = initial_path.len() - 1;
        let mut entry_surface_map = HashMap::new();

        for (depth, entry) in initial_path.iter().enumerate() {
            let surface = if depth == 0 {
                root_surface
            } else {
                let scene_key = scene_key_allocator.allocate();
                compositor.create_hidden_surface(depth as i32, viewport, scene_key)
            };
            compositor.set_surface_z_index(surface, depth as i32);
            compositor.set_surface_bounds(surface, viewport);
            let lifecycle = if depth == last_index {
                compositor.set_surface_visibility(surface, SurfaceVisibility::Visible);
                NavigationSurfaceLifecycle::Active
            } else {
                compositor.set_surface_visibility(surface, SurfaceVisibility::Hidden);
                NavigationSurfaceLifecycle::Inactive
            };
            entry_surface_map.insert(
                entry.clone(),
                NavigationSurfaceRecord { surface, lifecycle },
            );
        }

        let active_surface = initial_path
            .last()
            .and_then(|entry| entry_surface_map.get(entry))
            .map(|record| record.surface);

        Self {
            nav_host,
            mechanics: Rc::new(RefCell::new(NavigationSurfaceMechanics {
                window_surface: None,
                projection_path: initial_path,
                entry_surface_map,
                active_surface,
                focused_surface: active_surface,
                overlay_stack: Vec::new(),
                generation: NavigationOrchestrationGeneration(1),
                live_preparation: None,
            })),
            liveness: Rc::new(()),
            compositor_identity: compositor.identity(),
            z_base: 0,
            scene_key_allocator,
            viewport,
        }
    }

    /// The App realizes the current route inside its existing ScreenHost. The
    /// prepared navigation transaction retains the window's render target.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn with_window_projection(self, surface: CompositorSurfaceId) -> Self {
        {
            let mut mechanics = self.mechanics.borrow_mut();
            assert_eq!(mechanics.projection_path.len(), 1);
            assert_eq!(mechanics.active_surface, Some(surface));
            mechanics.window_surface = Some(surface);
        }
        self
    }

    /// Create an orchestrator with an explicit root-local z-band.
    fn new_with_z_base(
        nav_host: NavigationHost,
        viewport: RectF,
        compositor: &mut Compositor,
        mut scene_key_allocator: Box<dyn SceneKeyAllocator>,
        z_base: i32,
    ) -> Self {
        let initial_path = nav_host.path_entries();
        let last_index = initial_path.len() - 1;
        let mut entry_surface_map = HashMap::new();

        for (depth, entry) in initial_path.iter().enumerate() {
            let scene_key = scene_key_allocator.allocate();
            let surface =
                compositor.create_hidden_surface(z_base + depth as i32, viewport, scene_key);
            let lifecycle = if depth == last_index {
                compositor.set_surface_visibility(surface, SurfaceVisibility::Visible);
                NavigationSurfaceLifecycle::Active
            } else {
                NavigationSurfaceLifecycle::Inactive
            };
            entry_surface_map.insert(
                entry.clone(),
                NavigationSurfaceRecord { surface, lifecycle },
            );
        }

        let active_surface = initial_path
            .last()
            .and_then(|entry| entry_surface_map.get(entry))
            .map(|record| record.surface);

        Self {
            nav_host,
            mechanics: Rc::new(RefCell::new(NavigationSurfaceMechanics {
                window_surface: None,
                projection_path: initial_path,
                entry_surface_map,
                active_surface,
                focused_surface: active_surface,
                overlay_stack: Vec::new(),
                generation: NavigationOrchestrationGeneration(1),
                live_preparation: None,
            })),
            liveness: Rc::new(()),
            compositor_identity: compositor.identity(),
            z_base,
            scene_key_allocator,
            viewport,
        }
    }

    fn new_hidden_with_z_base(
        nav_host: NavigationHost,
        viewport: RectF,
        compositor: &mut Compositor,
        mut scene_key_allocator: Box<dyn SceneKeyAllocator>,
        z_base: i32,
    ) -> (Self, Vec<CompositorSurfaceId>) {
        let initial_path = nav_host.path_entries();
        let mut entry_surface_map = HashMap::new();
        let mut staged_surfaces = Vec::with_capacity(initial_path.len());

        for (depth, entry) in initial_path.iter().enumerate() {
            let scene_key = scene_key_allocator.allocate();
            let surface =
                compositor.create_hidden_surface(z_base + depth as i32, viewport, scene_key);
            staged_surfaces.push(surface);
            entry_surface_map.insert(
                entry.clone(),
                NavigationSurfaceRecord {
                    surface,
                    lifecycle: NavigationSurfaceLifecycle::Created,
                },
            );
        }

        (
            Self {
                nav_host,
                mechanics: Rc::new(RefCell::new(NavigationSurfaceMechanics {
                    window_surface: None,
                    projection_path: initial_path,
                    entry_surface_map,
                    active_surface: None,
                    focused_surface: None,
                    overlay_stack: Vec::new(),
                    generation: NavigationOrchestrationGeneration(1),
                    live_preparation: None,
                })),
                liveness: Rc::new(()),
                compositor_identity: compositor.identity(),
                z_base,
                scene_key_allocator,
                viewport,
            },
            staged_surfaces,
        )
    }

    fn activate_prepared_root(&mut self) {
        let path = self.nav_host.path_entries();
        let mut mechanics = self.mechanics.borrow_mut();
        for (depth, entry) in path.iter().enumerate() {
            let record = mechanics
                .entry_surface_map
                .get_mut(entry)
                .expect("prepared root mapping is complete");
            record.lifecycle = if depth + 1 == path.len() {
                NavigationSurfaceLifecycle::Active
            } else {
                NavigationSurfaceLifecycle::Inactive
            };
        }
        let active = path
            .last()
            .and_then(|entry| mechanics.entry_surface_map.get(entry))
            .map(|record| record.surface);
        mechanics.active_surface = active;
        mechanics.focused_surface = active;
    }

    /// Compatibility dispatch lowered through the prepared host and surface seams.
    pub fn dispatch(
        &mut self,
        intent: NavigationIntent,
        compositor: &mut Compositor,
    ) -> OrchestratedNavigationResult {
        self.dispatch_guarded(intent, compositor, None)
    }

    /// Compatibility dispatch with opaque application guard context.
    pub fn dispatch_with_context(
        &mut self,
        intent: NavigationIntent,
        compositor: &mut Compositor,
        context: &dyn Any,
    ) -> OrchestratedNavigationResult {
        self.dispatch_guarded(intent, compositor, Some(context))
    }

    fn dispatch_guarded(
        &mut self,
        intent: NavigationIntent,
        compositor: &mut Compositor,
        context: Option<&dyn Any>,
    ) -> OrchestratedNavigationResult {
        let snapshot = match self.nav_host.snapshot() {
            Ok(snapshot) => snapshot,
            Err(_) => return self.no_mutation_result(NavigationDecision::Retry, false),
        };
        let authorization = self.nav_host.authorize(&snapshot, intent, context, None);
        let authorization = match authorization {
            NavigationAuthorizationResult::Authorized(authorization) => authorization,
            NavigationAuthorizationResult::Denied => {
                return self.no_mutation_result(NavigationDecision::Deny, false);
            }
            NavigationAuthorizationResult::Unavailable => {
                return self.no_mutation_result(NavigationDecision::Unavailable, false);
            }
            NavigationAuthorizationResult::NoOp => {
                return self.no_mutation_result(NavigationDecision::NoOp, true);
            }
            NavigationAuthorizationResult::Retry => {
                return self.no_mutation_result(NavigationDecision::Retry, false);
            }
        };

        let prepared_host =
            match self
                .nav_host
                .prepare_commit(authorization, snapshot.generation(), None)
            {
                Ok(prepared) => prepared,
                Err(_) => return self.no_mutation_result(NavigationDecision::Retry, false),
            };
        let expected_generation = self.orchestration_generation();
        let prepared_surfaces = match self.prepare_navigation_surface_transition(
            &prepared_host,
            expected_generation,
            compositor,
        ) {
            Ok(prepared) => prepared,
            Err(_) => return self.no_mutation_result(NavigationDecision::Retry, false),
        };
        let publication = SurfaceTransitionPublicationCapability::new(&prepared_surfaces, None);

        let decision = match prepared_host.commit() {
            NavigationOutcome::Committed { redirected: false } => NavigationDecision::Allow,
            NavigationOutcome::Committed { redirected: true } => NavigationDecision::Redirect,
            NavigationOutcome::Denied
            | NavigationOutcome::Unavailable
            | NavigationOutcome::NoOp => unreachable!("prepared host commit always commits"),
        };
        let committed = prepared_surfaces.commit_guarded(publication);

        OrchestratedNavigationResult {
            decision,
            current_node: self.nav_host.current(),
            active_surface: committed.active_surface,
            step_transition: committed.step_transition,
            terminated_surfaces: committed.terminated_surfaces,
            focus_transition: committed.focus_transition,
        }
    }

    fn no_mutation_result(
        &self,
        decision: NavigationDecision,
        expose_active_surface: bool,
    ) -> OrchestratedNavigationResult {
        OrchestratedNavigationResult {
            decision,
            current_node: self.nav_host.current(),
            active_surface: expose_active_surface
                .then(|| self.mechanics.borrow().active_surface)
                .flatten(),
            step_transition: StepTransition::NoOp,
            terminated_surfaces: Vec::new(),
            focus_transition: None,
        }
    }

    /// Read-only: current active node.
    pub fn current_node(&self) -> NavigationNodeId {
        self.nav_host.current()
    }

    /// Compatibility lookup for the deepest current-path entry with this node ID.
    pub fn surface_for_node(&self, node: NavigationNodeId) -> Option<CompositorSurfaceId> {
        let mechanics = self.mechanics.borrow();
        self.nav_host
            .path_entries()
            .iter()
            .rev()
            .find(|entry| entry.node_id() == node)
            .and_then(|entry| mechanics.entry_surface_map.get(entry))
            .map(|record| record.surface)
    }

    /// Exact complete-entry to surface lookup.
    pub fn surface_for_entry(&self, entry: &NavigationEntry) -> Option<CompositorSurfaceId> {
        self.mechanics
            .borrow()
            .entry_surface_map
            .get(entry)
            .map(|record| record.surface)
    }

    /// Read-only: reference to underlying NavigationHost.
    pub fn nav_host(&self) -> &NavigationHost {
        &self.nav_host
    }

    /// Mutable access to NavigationHost (for set_guard/clear_guard).
    pub fn nav_host_mut(&mut self) -> &mut NavigationHost {
        &mut self.nav_host
    }

    pub(crate) fn prepare_host_commit_guarded(
        &self,
        authorization: NavigationGuardAuthorization,
        expected_generation: NavigationMechanicsGeneration,
        current_guard_context_generation: Option<NavigationGuardContextGeneration>,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedNavigationCommit, NavigationPreparationError> {
        self.nav_host.prepare_commit_guarded(
            authorization,
            expected_generation,
            current_guard_context_generation,
            navigation_commit_participation_identity(guard),
        )
    }

    /// Current step index (0-based position of active node in path).
    /// Derived from path length, not stored redundantly (SPEC-HAL-233 §5).
    pub fn step_index(&self) -> usize {
        self.nav_host.path().len().saturating_sub(1)
    }

    /// Total number of steps in current navigation path.
    /// Derived from path length, not stored redundantly (SPEC-HAL-233 §5).
    pub fn step_count(&self) -> usize {
        self.nav_host.path_entries().len()
    }

    pub fn orchestration_generation(&self) -> NavigationOrchestrationGeneration {
        self.mechanics.borrow().generation
    }

    pub fn surface_lifecycle(
        &self,
        surface: CompositorSurfaceId,
    ) -> Option<NavigationSurfaceLifecycle> {
        let mechanics = self.mechanics.borrow();
        mechanics
            .entry_surface_map
            .values()
            .find(|record| record.surface == surface)
            .map(|record| record.lifecycle)
    }

    /// Read-only: currently focused compositor surface (SPEC-HAL-235).
    /// Always follows the active navigation surface.
    pub fn focused_surface(&self) -> Option<CompositorSurfaceId> {
        self.mechanics.borrow().focused_surface
    }

    /// Open an overlay surface above the active navigation surface (SPEC-HAL-236).
    /// Overlay is pushed onto the LIFO overlay stack. Focus transfers to overlay.
    pub fn open_overlay(
        &mut self,
        kind: OverlayKind,
        compositor: &mut Compositor,
    ) -> CompositorSurfaceId {
        self.try_open_overlay(kind, compositor)
            .expect("legacy overlay mutation attempted while orchestration was busy")
    }

    pub fn try_open_overlay(
        &mut self,
        kind: OverlayKind,
        compositor: &mut Compositor,
    ) -> Result<CompositorSurfaceId, NavigationSurfacePreparationError> {
        let (host_surface, depth, next_generation) = {
            let mechanics = self.mechanics.borrow();
            if mechanics.live_preparation.is_some() {
                return Err(NavigationSurfacePreparationError::Busy);
            }
            let host_surface = mechanics
                .active_surface
                .ok_or(NavigationSurfacePreparationError::HostStateMismatch)?;
            let next_generation = mechanics
                .generation
                .checked_next()
                .ok_or(NavigationSurfacePreparationError::Exhausted)?;
            (host_surface, mechanics.overlay_stack.len(), next_generation)
        };
        if compositor.identity() != self.compositor_identity {
            return Err(NavigationSurfacePreparationError::WrongCompositor);
        }
        let scene_key = self.scene_key_allocator.allocate();
        let z = self.z_base + OVERLAY_Z_OFFSET + depth as i32;
        let surface_id = compositor.create_hidden_surface(z, self.viewport, scene_key);
        compositor.set_surface_visibility(surface_id, SurfaceVisibility::Visible);
        let mut mechanics = self.mechanics.borrow_mut();
        mechanics.overlay_stack.push(NavigationOverlayRecord {
            surface: surface_id,
            kind,
            host_surface,
        });
        mechanics.focused_surface = Some(surface_id);
        mechanics.generation = next_generation;
        Ok(surface_id)
    }

    /// Close an overlay surface. Destroyed through the lifecycle choke point (SPEC-HAL-234).
    /// Returns the terminated surface and reason, or None if surface not in overlay stack.
    pub fn close_overlay(
        &mut self,
        surface_id: CompositorSurfaceId,
        compositor: &mut Compositor,
    ) -> Option<(CompositorSurfaceId, SurfaceTerminationReason)> {
        self.try_close_overlay(surface_id, compositor)
            .expect("legacy overlay mutation attempted while orchestration was busy")
    }

    pub fn try_close_overlay(
        &mut self,
        surface_id: CompositorSurfaceId,
        compositor: &mut Compositor,
    ) -> Result<
        Option<(CompositorSurfaceId, SurfaceTerminationReason)>,
        NavigationSurfacePreparationError,
    > {
        let (position, next_generation) = {
            let mechanics = self.mechanics.borrow();
            if mechanics.live_preparation.is_some() {
                return Err(NavigationSurfacePreparationError::Busy);
            }
            let Some(position) = mechanics
                .overlay_stack
                .iter()
                .position(|record| record.surface == surface_id)
            else {
                return Ok(None);
            };
            let next_generation = mechanics
                .generation
                .checked_next()
                .ok_or(NavigationSurfacePreparationError::Exhausted)?;
            (position, next_generation)
        };
        if compositor.identity() != self.compositor_identity {
            return Err(NavigationSurfacePreparationError::WrongCompositor);
        }

        if !compositor.destroy_surface(surface_id) {
            return Err(NavigationSurfacePreparationError::MissingSurface(
                surface_id,
            ));
        }
        let mut mechanics = self.mechanics.borrow_mut();
        mechanics.overlay_stack.remove(position);
        mechanics.focused_surface = mechanics
            .overlay_stack
            .last()
            .map(|record| record.surface)
            .or(mechanics.active_surface);
        mechanics.generation = next_generation;
        Ok(Some((surface_id, SurfaceTerminationReason::OverlayClosed)))
    }

    /// Read-only: topmost overlay surface (SPEC-HAL-236).
    pub fn active_overlay(&self) -> Option<CompositorSurfaceId> {
        self.mechanics
            .borrow()
            .overlay_stack
            .last()
            .map(|record| record.surface)
    }

    pub fn active_overlay_kind(&self) -> Option<OverlayKind> {
        self.mechanics
            .borrow()
            .overlay_stack
            .last()
            .map(|record| record.kind)
    }

    pub fn overlay_host_surface(
        &self,
        overlay: CompositorSurfaceId,
    ) -> Option<CompositorSurfaceId> {
        self.mechanics
            .borrow()
            .overlay_stack
            .iter()
            .find(|record| record.surface == overlay)
            .map(|record| record.host_surface)
    }

    /// Read-only: number of overlays in the stack (SPEC-HAL-236).
    pub fn overlay_stack_depth(&self) -> usize {
        self.mechanics.borrow().overlay_stack.len()
    }

    /// Read-only projection of orchestrator state (SPEC-HAL-246).
    /// Pull-only. No side effects. No caching. No compositor calls.
    /// Returns owned snapshot. Two consecutive calls without intervening mutation return identical values.
    pub fn project(&self) -> Result<NavigationProjection, NavigationProjectionError> {
        let path = self.nav_host.path_entries();
        let mechanics = self
            .mechanics
            .try_borrow()
            .map_err(|_| NavigationProjectionError::Busy)?;
        if mechanics.projection_path != path {
            return Err(NavigationProjectionError::PathMappingMismatch);
        }
        NavigationSurfaceOrchestratorHandle::project_mechanics(&mechanics)
    }

    /// Prepare the complete surface delta for one already-authorized host commit.
    pub fn prepare_navigation_surface_transition<'a>(
        &mut self,
        host_commit: &PreparedNavigationCommit,
        expected_generation: NavigationOrchestrationGeneration,
        compositor: &'a mut Compositor,
    ) -> Result<PreparedNavigationSurfaceTransition<'a>, NavigationSurfacePreparationError> {
        self.prepare_navigation_surface_transition_inner(
            host_commit,
            expected_generation,
            compositor,
            None,
            None,
        )
    }

    pub(crate) fn prepare_navigation_surface_transition_guarded<'a>(
        &mut self,
        host_commit: &PreparedNavigationCommit,
        expected_generation: NavigationOrchestrationGeneration,
        compositor: &'a mut Compositor,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedNavigationSurfaceTransition<'a>, NavigationSurfacePreparationError> {
        self.prepare_navigation_surface_transition_inner(
            host_commit,
            expected_generation,
            compositor,
            None,
            Some(guard),
        )
    }

    pub(crate) fn prepare_navigation_surface_transition_with_overlay_exit<'a>(
        &mut self,
        host_commit: &PreparedNavigationCommit,
        expected_generation: NavigationOrchestrationGeneration,
        compositor: &'a mut Compositor,
        overlay_exit: PreparedOverlaySurfaceExitTestimony<'_>,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedNavigationSurfaceTransition<'a>, NavigationSurfacePreparationError> {
        self.prepare_navigation_surface_transition_inner(
            host_commit,
            expected_generation,
            compositor,
            Some(overlay_exit),
            Some(guard),
        )
    }

    pub(crate) fn prepare_overlay_surface_exit<'a>(
        &mut self,
        expected_generation: NavigationOrchestrationGeneration,
        compositor: &'a mut Compositor,
        overlay_exit: PreparedOverlaySurfaceExitTestimony<'_>,
    ) -> Result<PreparedNavigationOverlaySurfaceExit<'a>, NavigationSurfacePreparationError> {
        if compositor.identity() != self.compositor_identity {
            return Err(NavigationSurfacePreparationError::WrongCompositor);
        }
        let next_generation = {
            let mechanics = self.mechanics.borrow();
            if mechanics.live_preparation.is_some() {
                return Err(NavigationSurfacePreparationError::Busy);
            }
            if mechanics.generation != expected_generation {
                return Err(NavigationSurfacePreparationError::StaleGeneration);
            }
            let navigation_surfaces = mechanics
                .entry_surface_map
                .values()
                .map(|record| record.surface)
                .collect::<HashSet<_>>();
            let compatibility_overlays = mechanics
                .overlay_stack
                .iter()
                .map(|record| record.surface)
                .collect::<HashSet<_>>();
            let mut exit_surfaces = HashSet::new();
            for surface in overlay_exit.surfaces_top_first().iter().copied() {
                if !exit_surfaces.insert(surface) || compatibility_overlays.contains(&surface) {
                    return Err(
                        NavigationSurfacePreparationError::OverlayExitOwnershipConflict(surface),
                    );
                }
                if !compositor.has_surface(surface) {
                    return Err(
                        NavigationSurfacePreparationError::OverlayExitSurfaceMissing {
                            owner_id: overlay_exit.owner_id(),
                            stack_id: overlay_exit.stack_id(),
                            surface,
                        },
                    );
                }
                let Some(parent) = compositor.overlay_parent(surface) else {
                    return Err(
                        NavigationSurfacePreparationError::OverlayExitSurfaceOutsideRoot {
                            owner_id: overlay_exit.owner_id(),
                            stack_id: overlay_exit.stack_id(),
                            surface,
                        },
                    );
                };
                if !navigation_surfaces.contains(&parent) {
                    return Err(
                        NavigationSurfacePreparationError::OverlayExitSurfaceOutsideRoot {
                            owner_id: overlay_exit.owner_id(),
                            stack_id: overlay_exit.stack_id(),
                            surface,
                        },
                    );
                }
            }
            mechanics
                .generation
                .checked_next()
                .ok_or(NavigationSurfacePreparationError::Exhausted)?
        };

        let transition_identity = allocate_surface_transition_identity()
            .ok_or(NavigationSurfacePreparationError::Exhausted)?;
        let mut mechanical_batch = SurfaceMutationBatch::new();
        let mut terminated_surfaces = Vec::new();
        for surface in overlay_exit.surfaces_top_first().iter().copied() {
            match overlay_exit.disposition() {
                OverlayDisposition::Suspend => {
                    mechanical_batch.set_visibility(surface, SurfaceVisibility::Hidden);
                }
                OverlayDisposition::Dismiss => {
                    mechanical_batch.destroy(surface);
                    terminated_surfaces.push((surface, SurfaceTerminationReason::OverlayClosed));
                }
            }
        }
        compositor.validate_surface_batch(&mechanical_batch)?;
        {
            let mut mechanics = self.mechanics.borrow_mut();
            if mechanics.live_preparation.is_some() {
                return Err(NavigationSurfacePreparationError::Busy);
            }
            if mechanics.generation != expected_generation {
                return Err(NavigationSurfacePreparationError::StaleGeneration);
            }
            mechanics.live_preparation = Some(transition_identity);
        }
        let prepared_batch = compositor
            .prepare_surface_batch(mechanical_batch)
            .expect("overlay surface batch changed after validation without compositor access");
        Ok(PreparedNavigationOverlaySurfaceExit {
            mechanics: Rc::clone(&self.mechanics),
            batch: Some(prepared_batch),
            transition_identity,
            expected_generation,
            next_generation,
            terminated_surfaces,
            completed: false,
        })
    }

    fn prepare_navigation_surface_transition_inner<'a>(
        &mut self,
        host_commit: &PreparedNavigationCommit,
        expected_generation: NavigationOrchestrationGeneration,
        compositor: &'a mut Compositor,
        overlay_exit: Option<PreparedOverlaySurfaceExitTestimony<'_>>,
        guard: Option<&DomainTransitionGuardToken>,
    ) -> Result<PreparedNavigationSurfaceTransition<'a>, NavigationSurfacePreparationError> {
        if compositor.identity() != self.compositor_identity {
            return Err(NavigationSurfacePreparationError::WrongCompositor);
        }
        if self.nav_host.state() != *host_commit.from() {
            return Err(NavigationSurfacePreparationError::HostStateMismatch);
        }
        let commit_participation = self.nav_host.commit_participation_gate();

        let from_path = host_commit.from().path();
        let to_path = host_commit.to().path();
        let prefix_len = from_path
            .iter()
            .zip(to_path.iter())
            .take_while(|(from, to)| from == to)
            .count();
        if prefix_len == 0 {
            return Err(NavigationSurfacePreparationError::RootMutationForbidden);
        }

        let (old_map, old_focused, old_overlays, next_generation, frame_projection_participation, window_surface) = {
            let mechanics = self.mechanics.borrow();
            if mechanics.live_preparation.is_some() {
                return Err(NavigationSurfacePreparationError::Busy);
            }
            let frame_projection_participation =
                validate_frame_projection_publication_guard(&commit_participation, guard).map_err(
                    |error| match error {
                        FrameProjectionPublicationGuardError::Required => {
                            NavigationSurfacePreparationError::FrameProjectionGuardRequired
                        }
                        FrameProjectionPublicationGuardError::Mismatch => {
                            NavigationSurfacePreparationError::FrameProjectionGuardMismatch
                        }
                    },
                )?;
            if mechanics.generation != expected_generation {
                return Err(NavigationSurfacePreparationError::StaleGeneration);
            }
            if mechanics.entry_surface_map.len() != from_path.len()
                || from_path
                    .iter()
                    .any(|entry| !mechanics.entry_surface_map.contains_key(entry))
            {
                return Err(NavigationSurfacePreparationError::HostStateMismatch);
            }
            let expected_active = mechanics
                .entry_surface_map
                .get(host_commit.from().current())
                .map(|record| record.surface);
            if mechanics.active_surface != expected_active {
                return Err(NavigationSurfacePreparationError::HostStateMismatch);
            }
            let next_generation = mechanics
                .generation
                .checked_next()
                .ok_or(NavigationSurfacePreparationError::Exhausted)?;
            (
                mechanics.entry_surface_map.clone(),
                mechanics.focused_surface,
                mechanics.overlay_stack.clone(),
                next_generation,
                frame_projection_participation,
                mechanics.window_surface,
            )
        };

        for record in old_map.values() {
            if !compositor.has_surface(record.surface) {
                return Err(NavigationSurfacePreparationError::MissingSurface(
                    record.surface,
                ));
            }
        }
        for overlay in &old_overlays {
            if !compositor.has_surface(overlay.surface) {
                return Err(NavigationSurfacePreparationError::MissingSurface(
                    overlay.surface,
                ));
            }
        }

        let navigation_surfaces = old_map
            .values()
            .map(|record| record.surface)
            .collect::<HashSet<_>>();
        let compatibility_overlays = old_overlays
            .iter()
            .map(|record| record.surface)
            .collect::<HashSet<_>>();
        let mut exit_surfaces = HashSet::new();
        if let Some(exit) = overlay_exit {
            for surface in exit.surfaces_top_first().iter().copied() {
                if !exit_surfaces.insert(surface) || compatibility_overlays.contains(&surface) {
                    return Err(
                        NavigationSurfacePreparationError::OverlayExitOwnershipConflict(surface),
                    );
                }
                if !compositor.has_surface(surface) {
                    return Err(
                        NavigationSurfacePreparationError::OverlayExitSurfaceMissing {
                            owner_id: exit.owner_id(),
                            stack_id: exit.stack_id(),
                            surface,
                        },
                    );
                }
                let Some(parent) = compositor.overlay_parent(surface) else {
                    return Err(
                        NavigationSurfacePreparationError::OverlayExitSurfaceOutsideRoot {
                            owner_id: exit.owner_id(),
                            stack_id: exit.stack_id(),
                            surface,
                        },
                    );
                };
                if !navigation_surfaces.contains(&parent) {
                    return Err(
                        NavigationSurfacePreparationError::OverlayExitSurfaceOutsideRoot {
                            owner_id: exit.owner_id(),
                            stack_id: exit.stack_id(),
                            surface,
                        },
                    );
                }
            }
        }

        let retired_path = if window_surface.is_some() {
            &[][..]
        } else {
            &from_path[prefix_len..]
        };
        for entry in retired_path.iter().rev() {
            let surface = old_map
                .get(entry)
                .ok_or(NavigationSurfacePreparationError::HostStateMismatch)?
                .surface;
            for overlay in compositor.overlays_of(surface) {
                let exits_before_parent = exit_surfaces.contains(&overlay)
                    && overlay_exit
                        .is_some_and(|exit| exit.disposition() == OverlayDisposition::Dismiss);
                if !exits_before_parent {
                    return Err(NavigationSurfacePreparationError::ExternalOverlayAttached {
                        surface,
                        overlay,
                    });
                }
            }
        }

        let transition_identity = allocate_surface_transition_identity()
            .ok_or(NavigationSurfacePreparationError::Exhausted)?;
        let removed = from_path.len() - prefix_len;
        let added = to_path.len() - prefix_len;
        let step_transition = match (removed, added) {
            (0, 0) => StepTransition::NoOp,
            (0, 1) => StepTransition::Advance,
            (1, 0) => StepTransition::Retreat,
            _ => StepTransition::Replace { removed, added },
        };
        if step_transition == StepTransition::NoOp && window_surface.is_none() {
            return Err(NavigationSurfacePreparationError::HostStateMismatch);
        }

        let mut new_map = HashMap::new();
        for entry in &to_path[..prefix_len] {
            let record = old_map
                .get(entry)
                .ok_or(NavigationSurfacePreparationError::HostStateMismatch)?;
            new_map.insert(entry.clone(), record.clone());
        }

        let mut staged_surfaces = Vec::with_capacity(added);
        for (offset, entry) in to_path[prefix_len..].iter().enumerate() {
            let surface = if let Some(surface) = window_surface {
                surface
            } else {
                let scene_key = self.scene_key_allocator.allocate();
                let surface = compositor.create_hidden_surface(
                    self.z_base + (prefix_len + offset) as i32,
                    self.viewport,
                    scene_key,
                );
                staged_surfaces.push(surface);
                surface
            };
            new_map.insert(
                entry.clone(),
                NavigationSurfaceRecord {
                    surface,
                    lifecycle: NavigationSurfaceLifecycle::Created,
                },
            );
        }

        let new_active = new_map
            .get(host_commit.to().current())
            .map(|record| record.surface)
            .ok_or(NavigationSurfacePreparationError::HostStateMismatch)?;
        for (depth, entry) in to_path.iter().enumerate() {
            let record = new_map
                .get_mut(entry)
                .ok_or(NavigationSurfacePreparationError::HostStateMismatch)?;
            record.lifecycle = if depth + 1 == to_path.len() {
                NavigationSurfaceLifecycle::Active
            } else {
                NavigationSurfaceLifecycle::Inactive
            };
        }

        let mut mechanical_batch = SurfaceMutationBatch::new();
        if window_surface.is_none() {
            for (depth, entry) in to_path.iter().enumerate() {
                let surface = new_map[entry].surface;
                mechanical_batch.set_z_index(surface, self.z_base + depth as i32);
                mechanical_batch.set_bounds(surface, self.viewport);
            }
        }
        mechanical_batch.set_visibility(new_active, SurfaceVisibility::Visible);
        if window_surface.is_none() {
            for entry in to_path.iter().take(to_path.len() - 1) {
                mechanical_batch.set_visibility(new_map[entry].surface, SurfaceVisibility::Hidden);
            }
        }

        let dismissed_overlay_count = overlay_exit
            .filter(|exit| exit.disposition() == OverlayDisposition::Dismiss)
            .map_or(0, |exit| exit.surfaces_top_first().len());
        let mut terminated_surfaces =
            Vec::with_capacity(old_overlays.len() + dismissed_overlay_count + removed);
        for overlay in old_overlays.iter().rev() {
            mechanical_batch.destroy(overlay.surface);
            terminated_surfaces.push((overlay.surface, SurfaceTerminationReason::OverlayReplaced));
        }
        if let Some(exit) = overlay_exit {
            for surface in exit.surfaces_top_first().iter().copied() {
                match exit.disposition() {
                    OverlayDisposition::Suspend => {
                        mechanical_batch.set_visibility(surface, SurfaceVisibility::Hidden);
                    }
                    OverlayDisposition::Dismiss => {
                        mechanical_batch.destroy(surface);
                        terminated_surfaces
                            .push((surface, SurfaceTerminationReason::OverlayClosed));
                    }
                }
            }
        }
        if removed > 0 {
            let reason = Self::classify_termination_reason(removed, added, to_path.len());
            for entry in retired_path.iter().rev() {
                let surface = old_map[entry].surface;
                mechanical_batch.destroy(surface);
                terminated_surfaces.push((surface, reason));
            }
        }

        if let Err(error) = compositor.validate_surface_batch(&mechanical_batch) {
            for surface in staged_surfaces {
                compositor.destroy_surface(surface);
            }
            return Err(error.into());
        }

        {
            let mut mechanics = self.mechanics.borrow_mut();
            if mechanics.live_preparation.is_some() {
                for surface in staged_surfaces {
                    compositor.destroy_surface(surface);
                }
                return Err(NavigationSurfacePreparationError::Busy);
            }
            if validate_frame_projection_publication_guard(&commit_participation, guard).map_err(
                |error| match error {
                    FrameProjectionPublicationGuardError::Required => {
                        NavigationSurfacePreparationError::FrameProjectionGuardRequired
                    }
                    FrameProjectionPublicationGuardError::Mismatch => {
                        NavigationSurfacePreparationError::FrameProjectionGuardMismatch
                    }
                },
            )? != frame_projection_participation
            {
                for surface in staged_surfaces {
                    compositor.destroy_surface(surface);
                }
                return Err(NavigationSurfacePreparationError::FrameProjectionGuardMismatch);
            }
            if mechanics.generation != expected_generation {
                for surface in staged_surfaces {
                    compositor.destroy_surface(surface);
                }
                return Err(NavigationSurfacePreparationError::StaleGeneration);
            }
            mechanics.live_preparation = Some(transition_identity);
        }

        let prepared_batch = compositor
            .prepare_surface_batch(mechanical_batch)
            .expect("surface batch changed after validation without compositor access");

        Ok(PreparedNavigationSurfaceTransition {
            mechanics: Rc::clone(&self.mechanics),
            batch: Some(prepared_batch),
            transition_identity,
            expected_generation,
            next_generation,
            new_map: Some(new_map),
            new_projection_path: Some(to_path.to_vec()),
            new_active: Some(new_active),
            new_focused: Some(new_active),
            new_overlay_stack: Some(Vec::new()),
            staged_surfaces,
            step_transition,
            terminated_surfaces,
            old_focused,
            frame_projection_participation,
            completed: false,
        })
    }

    /// Classify why surfaces are being terminated, from prefix-diff shape (SPEC-HAL-234).
    /// Mechanically derivable — no guessing.
    fn classify_termination_reason(
        removed: usize,
        added: usize,
        new_path_len: usize,
    ) -> SurfaceTerminationReason {
        if removed == 1 && added == 0 {
            SurfaceTerminationReason::StepRetreat
        } else if new_path_len == 1 {
            SurfaceTerminationReason::RootReset
        } else {
            SurfaceTerminationReason::StepReplace
        }
    }
}

/// Exclusive prepared ordinary navigation-surface lifecycle transition.
pub struct PreparedNavigationSurfaceTransition<'a> {
    mechanics: Rc<RefCell<NavigationSurfaceMechanics>>,
    batch: Option<PreparedSurfaceBatch<'a>>,
    transition_identity: u64,
    expected_generation: NavigationOrchestrationGeneration,
    next_generation: NavigationOrchestrationGeneration,
    new_map: Option<HashMap<NavigationEntry, NavigationSurfaceRecord>>,
    new_projection_path: Option<Vec<NavigationEntry>>,
    new_active: Option<CompositorSurfaceId>,
    new_focused: Option<CompositorSurfaceId>,
    new_overlay_stack: Option<Vec<NavigationOverlayRecord>>,
    staged_surfaces: Vec<CompositorSurfaceId>,
    step_transition: StepTransition,
    terminated_surfaces: Vec<(CompositorSurfaceId, SurfaceTerminationReason)>,
    old_focused: Option<CompositorSurfaceId>,
    frame_projection_participation: Option<NavigationCommitParticipationIdentity>,
    completed: bool,
}

impl PreparedNavigationSurfaceTransition<'_> {
    pub fn publication_binding(&self) -> SurfaceTransitionPublicationBinding {
        SurfaceTransitionPublicationBinding {
            transition_identity: self.transition_identity,
            generation: self.expected_generation,
        }
    }

    pub fn staged_surfaces(&self) -> &[CompositorSurfaceId] {
        &self.staged_surfaces
    }

    pub fn next_active_surface(&self) -> CompositorSurfaceId {
        self.new_active
            .expect("prepared navigation transition must publish one active surface")
    }

    pub fn staged_surface_visibility(
        &self,
        surface: CompositorSurfaceId,
    ) -> Option<SurfaceVisibility> {
        self.batch
            .as_ref()
            .and_then(|batch| batch.surface_visibility(surface))
    }

    pub fn terminating_surfaces(&self) -> &[(CompositorSurfaceId, SurfaceTerminationReason)] {
        &self.terminated_surfaces
    }

    pub fn step_transition(&self) -> StepTransition {
        self.step_transition
    }

    pub fn abort(mut self) {
        self.abort_inner();
    }

    pub(crate) fn commit_guarded(
        mut self,
        capability: SurfaceTransitionPublicationCapability,
    ) -> CommittedNavigationSurfaceTransition {
        assert_eq!(
            capability.binding,
            self.publication_binding(),
            "surface publication capability does not match prepared transition"
        );
        assert_eq!(
            capability.participation, self.frame_projection_participation,
            "surface publication capability does not prove frame participation"
        );

        let batch = self
            .batch
            .take()
            .expect("prepared surface batch is single-use");
        let _ = batch.commit();

        let focus_transition = Self::derive_focus_transition(self.old_focused, self.new_focused);
        let mut mechanics = self.mechanics.borrow_mut();
        assert_eq!(
            mechanics.live_preparation,
            Some(self.transition_identity),
            "prepared surface transition lost its exclusive lease"
        );
        assert_eq!(
            mechanics.generation, self.expected_generation,
            "prepared surface transition generation changed under lease"
        );
        mechanics.entry_surface_map = self
            .new_map
            .take()
            .expect("prepared surface mapping is single-use");
        mechanics.projection_path = self
            .new_projection_path
            .take()
            .expect("prepared projection path is single-use");
        mechanics.active_surface = self.new_active;
        mechanics.focused_surface = self.new_focused;
        mechanics.overlay_stack = self
            .new_overlay_stack
            .take()
            .expect("prepared overlay result is single-use");
        mechanics.generation = self.next_generation;
        mechanics.live_preparation = None;
        drop(mechanics);

        self.completed = true;
        self.staged_surfaces.clear();
        CommittedNavigationSurfaceTransition {
            active_surface: self.new_active,
            step_transition: self.step_transition,
            terminated_surfaces: self.terminated_surfaces.clone(),
            focus_transition,
        }
    }

    fn derive_focus_transition(
        old: Option<CompositorSurfaceId>,
        new: Option<CompositorSurfaceId>,
    ) -> Option<FocusTransition> {
        match (old, new) {
            (Some(from), Some(to)) if from != to => Some(FocusTransition::Transferred { from, to }),
            (None, Some(surface)) => Some(FocusTransition::Gained { surface }),
            (Some(surface), None) => Some(FocusTransition::Lost { surface }),
            _ => None,
        }
    }

    fn abort_inner(&mut self) {
        if self.completed {
            return;
        }
        if let Some(batch) = self.batch.take() {
            let compositor = batch.abort();
            for surface in self.staged_surfaces.drain(..) {
                let removed = compositor.destroy_surface(surface);
                debug_assert!(removed, "hidden staged surface disappeared before abort");
            }
        }
        let mut mechanics = self.mechanics.borrow_mut();
        if mechanics.live_preparation == Some(self.transition_identity) {
            mechanics.live_preparation = None;
        }
        self.completed = true;
    }
}

impl Drop for PreparedNavigationSurfaceTransition<'_> {
    fn drop(&mut self) {
        self.abort_inner();
    }
}

/// Exclusive prepared mechanical exit for an externally owned overlay stack
/// when the navigation path itself does not change.
pub(crate) struct PreparedNavigationOverlaySurfaceExit<'a> {
    mechanics: Rc<RefCell<NavigationSurfaceMechanics>>,
    batch: Option<PreparedSurfaceBatch<'a>>,
    transition_identity: u64,
    expected_generation: NavigationOrchestrationGeneration,
    next_generation: NavigationOrchestrationGeneration,
    terminated_surfaces: Vec<(CompositorSurfaceId, SurfaceTerminationReason)>,
    completed: bool,
}

impl PreparedNavigationOverlaySurfaceExit<'_> {
    pub(crate) fn publication_binding(&self) -> SurfaceTransitionPublicationBinding {
        SurfaceTransitionPublicationBinding {
            transition_identity: self.transition_identity,
            generation: self.expected_generation,
        }
    }

    pub(crate) fn commit_guarded(
        mut self,
        capability: SurfaceTransitionPublicationCapability,
    ) -> Vec<(CompositorSurfaceId, SurfaceTerminationReason)> {
        assert_eq!(
            capability.binding,
            self.publication_binding(),
            "overlay surface publication capability does not match prepared exit"
        );
        assert_eq!(
            capability.participation, None,
            "overlay-only publication cannot carry frame participation"
        );
        let batch = self
            .batch
            .take()
            .expect("prepared overlay surface batch is single-use");
        let _ = batch.commit();
        let mut mechanics = self.mechanics.borrow_mut();
        assert_eq!(
            mechanics.live_preparation,
            Some(self.transition_identity),
            "prepared overlay exit lost its exclusive lease"
        );
        assert_eq!(mechanics.generation, self.expected_generation);
        mechanics.generation = self.next_generation;
        mechanics.live_preparation = None;
        drop(mechanics);
        self.completed = true;
        self.terminated_surfaces.clone()
    }

    fn abort_inner(&mut self) {
        if self.completed {
            return;
        }
        if let Some(batch) = self.batch.take() {
            let _ = batch.abort();
        }
        let mut mechanics = self.mechanics.borrow_mut();
        if mechanics.live_preparation == Some(self.transition_identity) {
            mechanics.live_preparation = None;
        }
        self.completed = true;
    }
}

impl Drop for PreparedNavigationOverlaySurfaceExit<'_> {
    fn drop(&mut self) {
        self.abort_inner();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationRootPreparationError {
    DuplicateRoot(NavigationRootId),
    UnknownRoot(NavigationRootId),
    Busy,
    WrongCompositor,
    StaleGeneration,
    FrameProjectionGuardRequired,
    FrameProjectionGuardMismatch,
    MissingSurface(CompositorSurfaceId),
    ExternalOverlayAttached {
        surface: CompositorSurfaceId,
        overlay: CompositorSurfaceId,
    },
    OverlayExitMustDismiss(OverlayOwnerId),
    OverlayExitSurfaceMissing {
        owner_id: OverlayOwnerId,
        stack_id: OverlayStackId,
        surface: CompositorSurfaceId,
    },
    OverlayExitSurfaceOutsideRoot {
        owner_id: OverlayOwnerId,
        stack_id: OverlayStackId,
        surface: CompositorSurfaceId,
    },
    OverlayExitOwnershipConflict(CompositorSurfaceId),
    MechanicalBatch(SurfaceBatchPreparationError),
    Exhausted,
}

impl From<SurfaceBatchPreparationError> for NavigationRootPreparationError {
    fn from(error: SurfaceBatchPreparationError) -> Self {
        Self::MechanicalBatch(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootAdmissionPublicationBinding {
    root_id: NavigationRootId,
    preparation_identity: u64,
    band_index: i32,
}

#[derive(Debug)]
pub(crate) struct RootAdmissionPublicationCapability {
    binding: RootAdmissionPublicationBinding,
}

impl RootAdmissionPublicationCapability {
    pub(crate) fn new(binding: RootAdmissionPublicationBinding) -> Self {
        Self { binding }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootSurfaceTeardownPublicationBinding {
    root_id: NavigationRootId,
    preparation_identity: u64,
    generation: NavigationOrchestrationGeneration,
    surfaces: Vec<CompositorSurfaceId>,
}

#[derive(Debug)]
pub(crate) struct RootSurfaceTeardownPublicationCapability {
    binding: RootSurfaceTeardownPublicationBinding,
    participation: Option<NavigationCommitParticipationIdentity>,
}

impl RootSurfaceTeardownPublicationCapability {
    pub(crate) fn new(
        prepared: &PreparedRootSurfaceTeardown<'_, '_>,
        guard: Option<&DomainTransitionGuardToken>,
    ) -> Self {
        let participation = prepared.frame_projection_participation;
        if let Some(required) = participation {
            assert_eq!(
                guard.map(navigation_commit_participation_identity),
                Some(required),
                "root teardown guard does not match frame participation"
            );
        }
        Self {
            binding: prepared.publication_binding(),
            participation,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RootTeardownPeerOverlayExit<'a> {
    pub root_id: NavigationRootId,
    pub expected_generation: NavigationOrchestrationGeneration,
    pub testimony: PreparedOverlaySurfaceExitTestimony<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DomainRootSurfaceTeardownRequest {
    pub root_id: NavigationRootId,
    pub expected_generation: NavigationOrchestrationGeneration,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DomainSurfaceTeardownOverlayExit<'a> {
    pub root_id: NavigationRootId,
    pub testimony: PreparedOverlaySurfaceExitTestimony<'a>,
}

/// Hidden, non-resident navigation root prepared for compound admission.
pub struct PreparedRegistryRoot<'registry, 'compositor> {
    registry: &'registry mut NavigationRootRegistry,
    batch: Option<PreparedSurfaceBatch<'compositor>>,
    orchestrator: Option<NavigationSurfaceOrchestrator>,
    root_id: NavigationRootId,
    preparation_identity: u64,
    band_index: i32,
    staged_surfaces: Vec<CompositorSurfaceId>,
    application_participation: Option<(NavigationCommitParticipationGate, u64)>,
    completed: bool,
}

impl PreparedRegistryRoot<'_, '_> {
    pub fn root_id(&self) -> NavigationRootId {
        self.root_id
    }

    pub fn current_entry(&self) -> NavigationEntry {
        self.orchestrator
            .as_ref()
            .expect("prepared root is live")
            .nav_host
            .current_entry()
    }

    pub fn current_surface(&self) -> CompositorSurfaceId {
        let orchestrator = self.orchestrator.as_ref().expect("prepared root is live");
        let entry = orchestrator.nav_host.current_entry();
        orchestrator
            .mechanics
            .borrow()
            .entry_surface_map
            .get(&entry)
            .expect("prepared root current surface exists")
            .surface
    }

    pub fn current_surface_visibility(&self) -> Option<SurfaceVisibility> {
        let surface = self.current_surface();
        self.batch
            .as_ref()
            .and_then(|batch| batch.surface_visibility(surface))
    }

    pub(crate) fn prepare_application_participation(
        &mut self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<(), NavigationRootPreparationError> {
        if self.application_participation.is_some() {
            return Err(NavigationRootPreparationError::Busy);
        }
        let preparation_identity = allocate_surface_transition_identity()
            .ok_or(NavigationRootPreparationError::Exhausted)?;
        let gate = self
            .orchestrator
            .as_ref()
            .expect("prepared root is live")
            .nav_host
            .commit_participation_gate();
        gate.prepare_add(
            preparation_identity,
            navigation_commit_participation_identity(guard),
        )
        .map_err(|error| match error {
            NavigationCommitParticipationError::Busy => NavigationRootPreparationError::Busy,
            NavigationCommitParticipationError::GuardRequired
            | NavigationCommitParticipationError::GuardMismatch
            | NavigationCommitParticipationError::MissingClaim => {
                NavigationRootPreparationError::FrameProjectionGuardMismatch
            }
            NavigationCommitParticipationError::Exhausted => {
                NavigationRootPreparationError::Exhausted
            }
        })?;
        self.application_participation = Some((gate, preparation_identity));
        Ok(())
    }

    pub fn publication_binding(&self) -> RootAdmissionPublicationBinding {
        RootAdmissionPublicationBinding {
            root_id: self.root_id,
            preparation_identity: self.preparation_identity,
            band_index: self.band_index,
        }
    }

    pub fn abort(mut self) {
        self.abort_inner();
    }

    pub(crate) fn commit_guarded(
        mut self,
        capability: RootAdmissionPublicationCapability,
    ) -> NavigationRootId {
        assert_eq!(
            capability.binding,
            self.publication_binding(),
            "root admission capability does not match prepared root"
        );
        let batch = self
            .batch
            .take()
            .expect("prepared root batch is single-use");
        let _ = batch.commit();
        let mut orchestrator = self
            .orchestrator
            .take()
            .expect("prepared root orchestrator is single-use");
        if let Some((gate, preparation_identity)) = self.application_participation.take() {
            gate.commit(preparation_identity);
        }
        orchestrator.activate_prepared_root();
        match self.registry.compositor_identity {
            Some(identity) => assert_eq!(identity, orchestrator.compositor_identity),
            None => self.registry.compositor_identity = Some(orchestrator.compositor_identity),
        }
        let replaced = self.registry.roots.insert(self.root_id, orchestrator);
        assert!(replaced.is_none(), "prepared root ID became resident");
        self.staged_surfaces.clear();
        self.completed = true;
        self.root_id
    }

    fn abort_inner(&mut self) {
        if self.completed {
            return;
        }
        if let Some(batch) = self.batch.take() {
            let compositor = batch.abort();
            for surface in self.staged_surfaces.drain(..).rev() {
                let removed = compositor.destroy_surface(surface);
                debug_assert!(removed, "hidden prepared-root surface disappeared");
            }
        }
        if let Some((gate, preparation_identity)) = self.application_participation.take() {
            gate.abort(preparation_identity);
        }
        self.orchestrator.take();
        self.registry.release_band(self.band_index);
        self.completed = true;
    }
}

impl Drop for PreparedRegistryRoot<'_, '_> {
    fn drop(&mut self) {
        self.abort_inner();
    }
}

/// Prepared destruction of every surface in one externally detached live root.
pub struct PreparedRootSurfaceTeardown<'registry, 'compositor> {
    registry: &'registry mut NavigationRootRegistry,
    mechanics: Rc<RefCell<NavigationSurfaceMechanics>>,
    batch: Option<PreparedSurfaceBatch<'compositor>>,
    root_id: NavigationRootId,
    preparation_identity: u64,
    expected_generation: NavigationOrchestrationGeneration,
    next_generation: NavigationOrchestrationGeneration,
    band_index: i32,
    terminated_surfaces: Vec<(CompositorSurfaceId, SurfaceTerminationReason)>,
    peer_overlay_exit: Option<PreparedRootTeardownPeerOverlayExit>,
    frame_projection_participation: Option<NavigationCommitParticipationIdentity>,
    completed: bool,
}

pub(crate) struct PreparedDomainSurfaceTeardown<'registry, 'compositor> {
    registry: &'registry mut NavigationRootRegistry,
    batch: Option<PreparedSurfaceBatch<'compositor>>,
    roots: Vec<PreparedDomainRootSurfaceTeardown>,
    terminated_surfaces: Vec<(CompositorSurfaceId, SurfaceTerminationReason)>,
    coordinator_identity: u64,
    interaction_domain_id: crate::core::event::InteractionDomainId,
    guard_generation: u64,
    preparation_identity: u64,
    completed: bool,
}

struct PreparedDomainRootSurfaceTeardown {
    root_id: NavigationRootId,
    mechanics: Rc<RefCell<NavigationSurfaceMechanics>>,
    expected_generation: NavigationOrchestrationGeneration,
    next_generation: NavigationOrchestrationGeneration,
    band_index: i32,
    frame_projection_participation: Option<NavigationCommitParticipationIdentity>,
}

impl PreparedDomainSurfaceTeardown<'_, '_> {
    pub(crate) fn commit_guarded(
        mut self,
        guard: &DomainTransitionGuardToken,
    ) -> Vec<(CompositorSurfaceId, SurfaceTerminationReason)> {
        assert_eq!(self.coordinator_identity, guard.coordinator_identity());
        assert_eq!(self.interaction_domain_id, guard.interaction_domain_id());
        assert_eq!(self.guard_generation, guard.generation());
        let participation = navigation_commit_participation_identity(guard);
        for root in &self.roots {
            if let Some(required) = root.frame_projection_participation {
                assert_eq!(
                    required, participation,
                    "domain teardown guard does not match frame participation"
                );
            }
        }
        let batch = self
            .batch
            .take()
            .expect("prepared domain teardown batch is single-use");
        let _ = batch.commit();
        for root in &self.roots {
            let mut mechanics = root.mechanics.borrow_mut();
            assert_eq!(
                mechanics.live_preparation,
                Some(self.preparation_identity),
                "prepared domain root teardown lost its exclusive lease"
            );
            assert_eq!(mechanics.generation, root.expected_generation);
            mechanics.entry_surface_map.clear();
            mechanics.overlay_stack.clear();
            mechanics.active_surface = None;
            mechanics.focused_surface = None;
            mechanics.generation = root.next_generation;
            mechanics.live_preparation = None;
        }
        for root in &self.roots {
            let removed = self.registry.roots.remove(&root.root_id);
            assert!(
                removed.is_some(),
                "prepared domain teardown root disappeared"
            );
            self.registry.release_band(root.band_index);
        }
        self.completed = true;
        self.terminated_surfaces.clone()
    }

    fn abort_inner(&mut self) {
        if self.completed {
            return;
        }
        if let Some(batch) = self.batch.take() {
            let _ = batch.abort();
        }
        for root in &self.roots {
            let mut mechanics = root.mechanics.borrow_mut();
            if mechanics.live_preparation == Some(self.preparation_identity) {
                mechanics.live_preparation = None;
            }
        }
        self.completed = true;
    }
}

impl Drop for PreparedDomainSurfaceTeardown<'_, '_> {
    fn drop(&mut self) {
        self.abort_inner();
    }
}

struct PreparedRootTeardownPeerOverlayExit {
    mechanics: Rc<RefCell<NavigationSurfaceMechanics>>,
    expected_generation: NavigationOrchestrationGeneration,
    next_generation: NavigationOrchestrationGeneration,
}

impl PreparedRootSurfaceTeardown<'_, '_> {
    pub fn publication_binding(&self) -> RootSurfaceTeardownPublicationBinding {
        RootSurfaceTeardownPublicationBinding {
            root_id: self.root_id,
            preparation_identity: self.preparation_identity,
            generation: self.expected_generation,
            surfaces: self
                .terminated_surfaces
                .iter()
                .map(|(surface, _)| *surface)
                .collect(),
        }
    }

    pub fn terminating_surfaces(&self) -> &[(CompositorSurfaceId, SurfaceTerminationReason)] {
        &self.terminated_surfaces
    }

    pub fn abort(mut self) {
        self.abort_inner();
    }

    pub(crate) fn commit_guarded(
        mut self,
        capability: RootSurfaceTeardownPublicationCapability,
    ) -> Vec<(CompositorSurfaceId, SurfaceTerminationReason)> {
        assert_eq!(
            capability.binding,
            self.publication_binding(),
            "root teardown capability does not match prepared root"
        );
        assert_eq!(
            capability.participation, self.frame_projection_participation,
            "root teardown capability does not prove frame participation"
        );
        let batch = self
            .batch
            .take()
            .expect("prepared root teardown batch is single-use");
        let _ = batch.commit();

        if let Some(peer_overlay_exit) = self.peer_overlay_exit.take() {
            let mut peer_mechanics = peer_overlay_exit.mechanics.borrow_mut();
            assert_eq!(
                peer_mechanics.live_preparation,
                Some(self.preparation_identity),
                "prepared peer overlay exit lost its exclusive lease"
            );
            assert_eq!(
                peer_mechanics.generation,
                peer_overlay_exit.expected_generation
            );
            peer_mechanics.generation = peer_overlay_exit.next_generation;
            peer_mechanics.live_preparation = None;
        }

        let mut mechanics = self.mechanics.borrow_mut();
        assert_eq!(
            mechanics.live_preparation,
            Some(self.preparation_identity),
            "prepared root teardown lost its exclusive lease"
        );
        assert_eq!(mechanics.generation, self.expected_generation);
        mechanics.entry_surface_map.clear();
        mechanics.overlay_stack.clear();
        mechanics.active_surface = None;
        mechanics.focused_surface = None;
        mechanics.generation = self.next_generation;
        mechanics.live_preparation = None;
        drop(mechanics);

        let removed = self.registry.roots.remove(&self.root_id);
        assert!(removed.is_some(), "prepared teardown root disappeared");
        self.registry.release_band(self.band_index);
        self.completed = true;
        self.terminated_surfaces.clone()
    }

    fn abort_inner(&mut self) {
        if self.completed {
            return;
        }
        if let Some(batch) = self.batch.take() {
            let _ = batch.abort();
        }
        if let Some(peer_overlay_exit) = self.peer_overlay_exit.take() {
            let mut peer_mechanics = peer_overlay_exit.mechanics.borrow_mut();
            if peer_mechanics.live_preparation == Some(self.preparation_identity) {
                peer_mechanics.live_preparation = None;
            }
        }
        let mut mechanics = self.mechanics.borrow_mut();
        if mechanics.live_preparation == Some(self.preparation_identity) {
            mechanics.live_preparation = None;
        }
        self.completed = true;
    }
}

impl Drop for PreparedRootSurfaceTeardown<'_, '_> {
    fn drop(&mut self) {
        self.abort_inner();
    }
}

/// Registry for multiple independent navigation roots sharing a compositor (SPEC-HAL-237).
/// Each root is an authority island: no cross-root surface ownership, focus, or overlay leakage.
/// Z-index bands are partitioned per-root to prevent cross-root collisions.
pub struct NavigationRootRegistry {
    roots: HashMap<NavigationRootId, NavigationSurfaceOrchestrator>,
    compositor_identity: Option<CompositorIdentity>,
    next_band_index: i32,
    free_band_indices: Vec<i32>,
}

impl NavigationRootRegistry {
    pub fn new() -> Self {
        Self {
            roots: HashMap::new(),
            compositor_identity: None,
            next_band_index: 0,
            free_band_indices: Vec::new(),
        }
    }

    pub const fn compositor_identity(&self) -> Option<CompositorIdentity> {
        self.compositor_identity
    }

    fn allocate_band(&mut self) -> Result<i32, NavigationRootPreparationError> {
        if let Some(index) = self.free_band_indices.pop() {
            return Ok(index);
        }
        let index = self.next_band_index;
        self.next_band_index = self
            .next_band_index
            .checked_add(1)
            .ok_or(NavigationRootPreparationError::Exhausted)?;
        Ok(index)
    }

    fn release_band(&mut self, band_index: i32) {
        if !self.free_band_indices.contains(&band_index) {
            self.free_band_indices.push(band_index);
        }
    }

    pub fn prepare_create_root<'registry, 'compositor>(
        &'registry mut self,
        root_id: NavigationRootId,
        host: NavigationHost,
        viewport: RectF,
        compositor: &'compositor mut Compositor,
        allocator: Box<dyn SceneKeyAllocator>,
    ) -> Result<PreparedRegistryRoot<'registry, 'compositor>, NavigationRootPreparationError> {
        if self.roots.contains_key(&root_id) {
            return Err(NavigationRootPreparationError::DuplicateRoot(root_id));
        }
        if self
            .compositor_identity
            .is_some_and(|identity| identity != compositor.identity())
        {
            return Err(NavigationRootPreparationError::WrongCompositor);
        }
        let preparation_identity = allocate_surface_transition_identity()
            .ok_or(NavigationRootPreparationError::Exhausted)?;
        let band_index = self.allocate_band()?;
        let Some(z_base) = band_index.checked_mul(Z_BAND_SIZE) else {
            self.release_band(band_index);
            return Err(NavigationRootPreparationError::Exhausted);
        };
        let (orchestrator, staged_surfaces) = NavigationSurfaceOrchestrator::new_hidden_with_z_base(
            host, viewport, compositor, allocator, z_base,
        );

        let path = orchestrator.nav_host.path_entries();
        let mechanics = orchestrator.mechanics.borrow();
        let current_surface = mechanics
            .entry_surface_map
            .get(orchestrator.nav_host.state().current())
            .expect("prepared root current surface exists")
            .surface;
        let mut mechanical_batch = SurfaceMutationBatch::new();
        for (depth, entry) in path.iter().enumerate() {
            let surface = mechanics.entry_surface_map[entry].surface;
            mechanical_batch.set_z_index(surface, z_base + depth as i32);
            mechanical_batch.set_bounds(surface, viewport);
            mechanical_batch.set_visibility(
                surface,
                if surface == current_surface {
                    SurfaceVisibility::Visible
                } else {
                    SurfaceVisibility::Hidden
                },
            );
        }
        drop(mechanics);

        if let Err(error) = compositor.validate_surface_batch(&mechanical_batch) {
            for surface in staged_surfaces.into_iter().rev() {
                compositor.destroy_surface(surface);
            }
            self.release_band(band_index);
            return Err(error.into());
        }
        let batch = compositor
            .prepare_surface_batch(mechanical_batch)
            .expect("prepared-root batch changed after validation");

        Ok(PreparedRegistryRoot {
            registry: self,
            batch: Some(batch),
            orchestrator: Some(orchestrator),
            root_id,
            preparation_identity,
            band_index,
            staged_surfaces,
            application_participation: None,
            completed: false,
        })
    }

    /// Compatibility root creation lowered through hidden prepared admission.
    pub fn create_root(
        &mut self,
        root_id: NavigationRootId,
        host: NavigationHost,
        viewport: RectF,
        compositor: &mut Compositor,
        allocator: Box<dyn SceneKeyAllocator>,
    ) {
        let prepared = self
            .prepare_create_root(root_id, host, viewport, compositor, allocator)
            .expect("duplicate or exhausted navigation root admission");
        let capability = RootAdmissionPublicationCapability::new(prepared.publication_binding());
        let _ = prepared.commit_guarded(capability);
    }

    pub fn prepare_root_surface_teardown<'registry, 'compositor>(
        &'registry mut self,
        root_id: NavigationRootId,
        expected_generation: NavigationOrchestrationGeneration,
        compositor: &'compositor mut Compositor,
    ) -> Result<PreparedRootSurfaceTeardown<'registry, 'compositor>, NavigationRootPreparationError>
    {
        self.prepare_root_surface_teardown_inner(
            root_id,
            expected_generation,
            compositor,
            None,
            None,
            None,
        )
    }

    pub(crate) fn prepare_domain_surface_teardown<'registry, 'compositor>(
        &'registry mut self,
        mut requests: Vec<DomainRootSurfaceTeardownRequest>,
        compositor: &'compositor mut Compositor,
        active_overlay_exit: Option<DomainSurfaceTeardownOverlayExit<'_>>,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedDomainSurfaceTeardown<'registry, 'compositor>, NavigationRootPreparationError>
    {
        if self
            .compositor_identity
            .is_some_and(|identity| identity != compositor.identity())
        {
            return Err(NavigationRootPreparationError::WrongCompositor);
        }
        requests.sort_by_key(|request| request.root_id.0);
        if requests
            .windows(2)
            .any(|pair| pair[0].root_id == pair[1].root_id)
        {
            return Err(NavigationRootPreparationError::DuplicateRoot(
                requests
                    .windows(2)
                    .find(|pair| pair[0].root_id == pair[1].root_id)
                    .expect("duplicate root request was detected")[0]
                    .root_id,
            ));
        }

        struct Candidate {
            root_id: NavigationRootId,
            mechanics: Rc<RefCell<NavigationSurfaceMechanics>>,
            commit_participation: NavigationCommitParticipationGate,
            frame_projection_participation: Option<NavigationCommitParticipationIdentity>,
            expected_generation: NavigationOrchestrationGeneration,
            next_generation: NavigationOrchestrationGeneration,
            band_index: i32,
            external_overlays: Vec<CompositorSurfaceId>,
            overlays: Vec<CompositorSurfaceId>,
            navigation_surfaces: Vec<CompositorSurfaceId>,
        }

        let mut candidates = Vec::with_capacity(requests.len());
        let mut all_scheduled_surfaces = HashSet::new();
        for request in requests {
            let orchestrator = self
                .roots
                .get(&request.root_id)
                .ok_or(NavigationRootPreparationError::UnknownRoot(request.root_id))?;
            if orchestrator.compositor_identity != compositor.identity() {
                return Err(NavigationRootPreparationError::WrongCompositor);
            }
            let commit_participation = orchestrator.nav_host.commit_participation_gate();
            let frame_projection_participation =
                validate_frame_projection_publication_guard(&commit_participation, Some(guard))
                    .map_err(|error| match error {
                        FrameProjectionPublicationGuardError::Required => {
                            NavigationRootPreparationError::FrameProjectionGuardRequired
                        }
                        FrameProjectionPublicationGuardError::Mismatch => {
                            NavigationRootPreparationError::FrameProjectionGuardMismatch
                        }
                    })?;
            let path = orchestrator.nav_host.path_entries();
            let mechanics_ref = Rc::clone(&orchestrator.mechanics);
            let (overlays, navigation_surfaces, next_generation, band_index) = {
                let mechanics = mechanics_ref.borrow();
                if mechanics.live_preparation.is_some() {
                    return Err(NavigationRootPreparationError::Busy);
                }
                if mechanics.generation != request.expected_generation {
                    return Err(NavigationRootPreparationError::StaleGeneration);
                }
                let next_generation = mechanics
                    .generation
                    .checked_next()
                    .ok_or(NavigationRootPreparationError::Exhausted)?;
                let overlays = mechanics
                    .overlay_stack
                    .iter()
                    .rev()
                    .map(|record| record.surface)
                    .collect::<Vec<_>>();
                let navigation_surfaces = path
                    .iter()
                    .rev()
                    .map(|entry| {
                        mechanics
                            .entry_surface_map
                            .get(entry)
                            .expect("live root mapping is complete")
                            .surface
                    })
                    .collect::<Vec<_>>();
                (
                    overlays,
                    navigation_surfaces,
                    next_generation,
                    orchestrator.z_base / Z_BAND_SIZE,
                )
            };
            let navigation_surface_set =
                navigation_surfaces.iter().copied().collect::<HashSet<_>>();
            let compatibility_overlay_set = overlays.iter().copied().collect::<HashSet<_>>();
            let mut authorized_overlays = compatibility_overlay_set.clone();
            let mut external_overlays = Vec::new();
            if let Some(exit) = active_overlay_exit.filter(|exit| exit.root_id == request.root_id) {
                if exit.testimony.disposition() != OverlayDisposition::Dismiss {
                    return Err(NavigationRootPreparationError::OverlayExitMustDismiss(
                        exit.testimony.owner_id(),
                    ));
                }
                for surface in exit.testimony.surfaces_top_first().iter().copied() {
                    if !authorized_overlays.insert(surface) {
                        if compatibility_overlay_set.contains(&surface) {
                            continue;
                        }
                        return Err(
                            NavigationRootPreparationError::OverlayExitOwnershipConflict(surface),
                        );
                    }
                    if !compositor.has_surface(surface) {
                        return Err(NavigationRootPreparationError::OverlayExitSurfaceMissing {
                            owner_id: exit.testimony.owner_id(),
                            stack_id: exit.testimony.stack_id(),
                            surface,
                        });
                    }
                    let Some(parent) = compositor.overlay_parent(surface) else {
                        return Err(
                            NavigationRootPreparationError::OverlayExitSurfaceOutsideRoot {
                                owner_id: exit.testimony.owner_id(),
                                stack_id: exit.testimony.stack_id(),
                                surface,
                            },
                        );
                    };
                    if !navigation_surface_set.contains(&parent) {
                        return Err(
                            NavigationRootPreparationError::OverlayExitSurfaceOutsideRoot {
                                owner_id: exit.testimony.owner_id(),
                                stack_id: exit.testimony.stack_id(),
                                surface,
                            },
                        );
                    }
                    external_overlays.push(surface);
                }
            }
            for surface in overlays.iter().chain(&navigation_surfaces) {
                if !compositor.has_surface(*surface) {
                    return Err(NavigationRootPreparationError::MissingSurface(*surface));
                }
            }
            for surface in &navigation_surfaces {
                if let Some(overlay) = compositor
                    .overlays_of(*surface)
                    .iter()
                    .copied()
                    .find(|overlay| !authorized_overlays.contains(overlay))
                {
                    return Err(NavigationRootPreparationError::ExternalOverlayAttached {
                        surface: *surface,
                        overlay,
                    });
                }
            }
            for surface in external_overlays
                .iter()
                .chain(&overlays)
                .chain(&navigation_surfaces)
            {
                if !all_scheduled_surfaces.insert(*surface) {
                    return Err(
                        NavigationRootPreparationError::OverlayExitOwnershipConflict(*surface),
                    );
                }
            }
            candidates.push(Candidate {
                root_id: request.root_id,
                mechanics: mechanics_ref,
                commit_participation,
                frame_projection_participation,
                expected_generation: request.expected_generation,
                next_generation,
                band_index,
                external_overlays,
                overlays,
                navigation_surfaces,
            });
        }
        if active_overlay_exit.is_some_and(|exit| {
            !candidates
                .iter()
                .any(|candidate| candidate.root_id == exit.root_id)
        }) {
            return Err(NavigationRootPreparationError::UnknownRoot(
                active_overlay_exit
                    .expect("missing active overlay root was detected")
                    .root_id,
            ));
        }

        let preparation_identity = allocate_surface_transition_identity()
            .ok_or(NavigationRootPreparationError::Exhausted)?;
        let mut mechanical_batch = SurfaceMutationBatch::new();
        let mut terminated_surfaces = Vec::new();
        for candidate in &candidates {
            for surface in &candidate.external_overlays {
                mechanical_batch.destroy(*surface);
                terminated_surfaces.push((*surface, SurfaceTerminationReason::OverlayClosed));
            }
            for surface in candidate
                .overlays
                .iter()
                .chain(&candidate.navigation_surfaces)
            {
                mechanical_batch.destroy(*surface);
                terminated_surfaces.push((*surface, SurfaceTerminationReason::HostTeardown));
            }
        }
        compositor.validate_surface_batch(&mechanical_batch)?;
        let mut leased: Vec<Rc<RefCell<NavigationSurfaceMechanics>>> = Vec::new();
        for candidate in &candidates {
            let mut mechanics = candidate.mechanics.borrow_mut();
            let participation = validate_frame_projection_publication_guard(
                &candidate.commit_participation,
                Some(guard),
            )
            .map_err(|error| match error {
                FrameProjectionPublicationGuardError::Required => {
                    NavigationRootPreparationError::FrameProjectionGuardRequired
                }
                FrameProjectionPublicationGuardError::Mismatch => {
                    NavigationRootPreparationError::FrameProjectionGuardMismatch
                }
            });
            if mechanics.live_preparation.is_some()
                || mechanics.generation != candidate.expected_generation
                || participation.as_ref().ok() != Some(&candidate.frame_projection_participation)
            {
                for mechanics in leased {
                    let mut mechanics: std::cell::RefMut<'_, NavigationSurfaceMechanics> =
                        mechanics.borrow_mut();
                    if mechanics.live_preparation == Some(preparation_identity) {
                        mechanics.live_preparation = None;
                    }
                }
                return Err(match participation {
                    Err(error) => error,
                    Ok(_) => NavigationRootPreparationError::Busy,
                });
            }
            mechanics.live_preparation = Some(preparation_identity);
            leased.push(Rc::clone(&candidate.mechanics));
        }
        let batch = compositor
            .prepare_surface_batch(mechanical_batch)
            .expect("domain teardown batch changed after validation");
        Ok(PreparedDomainSurfaceTeardown {
            registry: self,
            batch: Some(batch),
            roots: candidates
                .into_iter()
                .map(|candidate| PreparedDomainRootSurfaceTeardown {
                    root_id: candidate.root_id,
                    mechanics: candidate.mechanics,
                    expected_generation: candidate.expected_generation,
                    next_generation: candidate.next_generation,
                    band_index: candidate.band_index,
                    frame_projection_participation: candidate.frame_projection_participation,
                })
                .collect(),
            terminated_surfaces,
            coordinator_identity: guard.coordinator_identity(),
            interaction_domain_id: guard.interaction_domain_id(),
            guard_generation: guard.generation(),
            preparation_identity,
            completed: false,
        })
    }

    pub(crate) fn prepare_root_surface_teardown_with_overlay_exits<'registry, 'compositor>(
        &'registry mut self,
        root_id: NavigationRootId,
        expected_generation: NavigationOrchestrationGeneration,
        compositor: &'compositor mut Compositor,
        retiring_root_overlay_exit: Option<PreparedOverlaySurfaceExitTestimony<'_>>,
        peer_overlay_exit: Option<RootTeardownPeerOverlayExit<'_>>,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedRootSurfaceTeardown<'registry, 'compositor>, NavigationRootPreparationError>
    {
        self.prepare_root_surface_teardown_inner(
            root_id,
            expected_generation,
            compositor,
            retiring_root_overlay_exit,
            peer_overlay_exit,
            Some(guard),
        )
    }

    fn prepare_root_surface_teardown_inner<'registry, 'compositor>(
        &'registry mut self,
        root_id: NavigationRootId,
        expected_generation: NavigationOrchestrationGeneration,
        compositor: &'compositor mut Compositor,
        retiring_root_overlay_exit: Option<PreparedOverlaySurfaceExitTestimony<'_>>,
        peer_overlay_exit: Option<RootTeardownPeerOverlayExit<'_>>,
        guard: Option<&DomainTransitionGuardToken>,
    ) -> Result<PreparedRootSurfaceTeardown<'registry, 'compositor>, NavigationRootPreparationError>
    {
        if self
            .compositor_identity
            .is_some_and(|identity| identity != compositor.identity())
        {
            return Err(NavigationRootPreparationError::WrongCompositor);
        }
        let orchestrator = self
            .roots
            .get(&root_id)
            .ok_or(NavigationRootPreparationError::UnknownRoot(root_id))?;
        if orchestrator.compositor_identity != compositor.identity() {
            return Err(NavigationRootPreparationError::WrongCompositor);
        }
        let commit_participation = orchestrator.nav_host.commit_participation_gate();
        let path = orchestrator.nav_host.path_entries();
        let band_index = orchestrator.z_base / Z_BAND_SIZE;
        let mechanics_ref = Rc::clone(&orchestrator.mechanics);
        let (overlays, navigation_surfaces, next_generation, frame_projection_participation) = {
            let mechanics = mechanics_ref.borrow();
            if mechanics.live_preparation.is_some() {
                return Err(NavigationRootPreparationError::Busy);
            }
            let frame_projection_participation =
                validate_frame_projection_publication_guard(&commit_participation, guard).map_err(
                    |error| match error {
                        FrameProjectionPublicationGuardError::Required => {
                            NavigationRootPreparationError::FrameProjectionGuardRequired
                        }
                        FrameProjectionPublicationGuardError::Mismatch => {
                            NavigationRootPreparationError::FrameProjectionGuardMismatch
                        }
                    },
                )?;
            if mechanics.generation != expected_generation {
                return Err(NavigationRootPreparationError::StaleGeneration);
            }
            let next_generation = mechanics
                .generation
                .checked_next()
                .ok_or(NavigationRootPreparationError::Exhausted)?;
            let overlays = mechanics
                .overlay_stack
                .iter()
                .rev()
                .map(|record| record.surface)
                .collect::<Vec<_>>();
            let navigation_surfaces = path
                .iter()
                .rev()
                .map(|entry| {
                    mechanics
                        .entry_surface_map
                        .get(entry)
                        .expect("live root mapping is complete")
                        .surface
                })
                .collect::<Vec<_>>();
            (
                overlays,
                navigation_surfaces,
                next_generation,
                frame_projection_participation,
            )
        };

        let navigation_surface_set = navigation_surfaces.iter().copied().collect::<HashSet<_>>();
        let compatibility_overlay_set = overlays.iter().copied().collect::<HashSet<_>>();
        let mut retiring_external_overlays = Vec::new();
        let mut authorized_retiring_overlays = compatibility_overlay_set.clone();
        if let Some(exit) = retiring_root_overlay_exit {
            if exit.disposition() != OverlayDisposition::Dismiss {
                return Err(NavigationRootPreparationError::OverlayExitMustDismiss(
                    exit.owner_id(),
                ));
            }
            for surface in exit.surfaces_top_first().iter().copied() {
                if !authorized_retiring_overlays.insert(surface) {
                    if compatibility_overlay_set.contains(&surface) {
                        continue;
                    }
                    return Err(
                        NavigationRootPreparationError::OverlayExitOwnershipConflict(surface),
                    );
                }
                if !compositor.has_surface(surface) {
                    return Err(NavigationRootPreparationError::OverlayExitSurfaceMissing {
                        owner_id: exit.owner_id(),
                        stack_id: exit.stack_id(),
                        surface,
                    });
                }
                let Some(parent) = compositor.overlay_parent(surface) else {
                    return Err(
                        NavigationRootPreparationError::OverlayExitSurfaceOutsideRoot {
                            owner_id: exit.owner_id(),
                            stack_id: exit.stack_id(),
                            surface,
                        },
                    );
                };
                if !navigation_surface_set.contains(&parent) {
                    return Err(
                        NavigationRootPreparationError::OverlayExitSurfaceOutsideRoot {
                            owner_id: exit.owner_id(),
                            stack_id: exit.stack_id(),
                            surface,
                        },
                    );
                }
                retiring_external_overlays.push(surface);
            }
        }

        for surface in overlays.iter().chain(&navigation_surfaces) {
            if !compositor.has_surface(*surface) {
                return Err(NavigationRootPreparationError::MissingSurface(*surface));
            }
        }
        for surface in &navigation_surfaces {
            if let Some(overlay) = compositor
                .overlays_of(*surface)
                .iter()
                .copied()
                .find(|overlay| !authorized_retiring_overlays.contains(overlay))
            {
                return Err(NavigationRootPreparationError::ExternalOverlayAttached {
                    surface: *surface,
                    overlay,
                });
            }
        }

        let prepared_peer_overlay_exit = if let Some(peer_exit) = peer_overlay_exit {
            if peer_exit.root_id == root_id {
                return Err(NavigationRootPreparationError::UnknownRoot(
                    peer_exit.root_id,
                ));
            }
            if peer_exit.testimony.disposition() != OverlayDisposition::Dismiss {
                return Err(NavigationRootPreparationError::OverlayExitMustDismiss(
                    peer_exit.testimony.owner_id(),
                ));
            }
            let peer_orchestrator = self.roots.get(&peer_exit.root_id).ok_or(
                NavigationRootPreparationError::UnknownRoot(peer_exit.root_id),
            )?;
            if peer_orchestrator.compositor_identity != compositor.identity() {
                return Err(NavigationRootPreparationError::WrongCompositor);
            }
            let peer_mechanics = Rc::clone(&peer_orchestrator.mechanics);
            let peer_next_generation = {
                let mechanics = peer_mechanics.borrow();
                if mechanics.live_preparation.is_some() {
                    return Err(NavigationRootPreparationError::Busy);
                }
                if mechanics.generation != peer_exit.expected_generation {
                    return Err(NavigationRootPreparationError::StaleGeneration);
                }
                let peer_navigation_surfaces = mechanics
                    .entry_surface_map
                    .values()
                    .map(|record| record.surface)
                    .collect::<HashSet<_>>();
                let peer_compatibility_overlays = mechanics
                    .overlay_stack
                    .iter()
                    .map(|record| record.surface)
                    .collect::<HashSet<_>>();
                let mut seen = HashSet::new();
                for surface in peer_exit.testimony.surfaces_top_first().iter().copied() {
                    if !seen.insert(surface)
                        || peer_compatibility_overlays.contains(&surface)
                        || authorized_retiring_overlays.contains(&surface)
                    {
                        return Err(
                            NavigationRootPreparationError::OverlayExitOwnershipConflict(surface),
                        );
                    }
                    if !compositor.has_surface(surface) {
                        return Err(NavigationRootPreparationError::OverlayExitSurfaceMissing {
                            owner_id: peer_exit.testimony.owner_id(),
                            stack_id: peer_exit.testimony.stack_id(),
                            surface,
                        });
                    }
                    let Some(parent) = compositor.overlay_parent(surface) else {
                        return Err(
                            NavigationRootPreparationError::OverlayExitSurfaceOutsideRoot {
                                owner_id: peer_exit.testimony.owner_id(),
                                stack_id: peer_exit.testimony.stack_id(),
                                surface,
                            },
                        );
                    };
                    if !peer_navigation_surfaces.contains(&parent) {
                        return Err(
                            NavigationRootPreparationError::OverlayExitSurfaceOutsideRoot {
                                owner_id: peer_exit.testimony.owner_id(),
                                stack_id: peer_exit.testimony.stack_id(),
                                surface,
                            },
                        );
                    }
                }
                mechanics
                    .generation
                    .checked_next()
                    .ok_or(NavigationRootPreparationError::Exhausted)?
            };
            Some((
                peer_mechanics,
                peer_exit.expected_generation,
                peer_next_generation,
                peer_exit.testimony.surfaces_top_first().to_vec(),
            ))
        } else {
            None
        };

        let preparation_identity = allocate_surface_transition_identity()
            .ok_or(NavigationRootPreparationError::Exhausted)?;
        let mut mechanical_batch = SurfaceMutationBatch::new();
        let peer_surface_count = prepared_peer_overlay_exit
            .as_ref()
            .map_or(0, |(_, _, _, surfaces)| surfaces.len());
        let mut terminated_surfaces = Vec::with_capacity(
            peer_surface_count
                + retiring_external_overlays.len()
                + overlays.len()
                + navigation_surfaces.len(),
        );
        if let Some((_, _, _, surfaces)) = &prepared_peer_overlay_exit {
            for surface in surfaces.iter().copied() {
                mechanical_batch.destroy(surface);
                terminated_surfaces.push((surface, SurfaceTerminationReason::OverlayClosed));
            }
        }
        for surface in retiring_external_overlays
            .into_iter()
            .chain(overlays)
            .chain(navigation_surfaces)
        {
            mechanical_batch.destroy(surface);
            terminated_surfaces.push((surface, SurfaceTerminationReason::HostTeardown));
        }
        compositor.validate_surface_batch(&mechanical_batch)?;
        {
            let mut mechanics = mechanics_ref.borrow_mut();
            if mechanics.live_preparation.is_some() {
                return Err(NavigationRootPreparationError::Busy);
            }
            if validate_frame_projection_publication_guard(&commit_participation, guard).map_err(
                |error| match error {
                    FrameProjectionPublicationGuardError::Required => {
                        NavigationRootPreparationError::FrameProjectionGuardRequired
                    }
                    FrameProjectionPublicationGuardError::Mismatch => {
                        NavigationRootPreparationError::FrameProjectionGuardMismatch
                    }
                },
            )? != frame_projection_participation
            {
                return Err(NavigationRootPreparationError::FrameProjectionGuardMismatch);
            }
            if mechanics.generation != expected_generation {
                return Err(NavigationRootPreparationError::StaleGeneration);
            }
            mechanics.live_preparation = Some(preparation_identity);
        }
        if let Some((peer_mechanics, expected, _, _)) = &prepared_peer_overlay_exit {
            let mut mechanics = peer_mechanics.borrow_mut();
            if mechanics.live_preparation.is_some() || mechanics.generation != *expected {
                let mut retiring_mechanics = mechanics_ref.borrow_mut();
                retiring_mechanics.live_preparation = None;
                return Err(NavigationRootPreparationError::Busy);
            }
            mechanics.live_preparation = Some(preparation_identity);
        }
        let batch = compositor
            .prepare_surface_batch(mechanical_batch)
            .expect("root teardown batch changed after validation");

        let peer_overlay_exit = prepared_peer_overlay_exit.map(
            |(mechanics, expected_generation, next_generation, _)| {
                PreparedRootTeardownPeerOverlayExit {
                    mechanics,
                    expected_generation,
                    next_generation,
                }
            },
        );

        Ok(PreparedRootSurfaceTeardown {
            registry: self,
            mechanics: mechanics_ref,
            batch: Some(batch),
            root_id,
            preparation_identity,
            expected_generation,
            next_generation,
            band_index,
            terminated_surfaces,
            peer_overlay_exit,
            frame_projection_participation,
            completed: false,
        })
    }

    /// Compatibility teardown lowered through the explicit whole-root path.
    pub fn destroy_root(
        &mut self,
        root_id: NavigationRootId,
        compositor: &mut Compositor,
    ) -> Vec<(CompositorSurfaceId, SurfaceTerminationReason)> {
        let Some(generation) = self
            .roots
            .get(&root_id)
            .map(NavigationSurfaceOrchestrator::orchestration_generation)
        else {
            return Vec::new();
        };
        let prepared = self
            .prepare_root_surface_teardown(root_id, generation, compositor)
            .expect("navigation root teardown preparation failed");
        let capability = RootSurfaceTeardownPublicationCapability::new(&prepared, None);
        prepared.commit_guarded(capability)
    }

    /// Dispatch a navigation intent to a specific root.
    /// Returns None if root_id is not registered.
    pub fn dispatch(
        &mut self,
        root_id: NavigationRootId,
        intent: NavigationIntent,
        compositor: &mut Compositor,
    ) -> Option<OrchestratedNavigationResult> {
        self.roots
            .get_mut(&root_id)
            .map(|orch| orch.dispatch(intent, compositor))
    }

    /// Read-only: reference to a root's orchestrator.
    pub fn root(&self, root_id: NavigationRootId) -> Option<&NavigationSurfaceOrchestrator> {
        self.roots.get(&root_id)
    }

    /// Mutable reference to a root's orchestrator (for overlay operations, guard changes, etc.).
    pub fn root_mut(
        &mut self,
        root_id: NavigationRootId,
    ) -> Option<&mut NavigationSurfaceOrchestrator> {
        self.roots.get_mut(&root_id)
    }

    /// Number of registered roots.
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }
}
