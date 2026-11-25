use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::compositor::SurfaceId;
use crate::core::event::{InteractionDomainId, InteractionDomains};
use crate::core::focus_authority::{
    FocusAuthorityError, FocusDomainAddition, FocusDomainGraphMutationTestimony, FocusDomainId,
    FocusResolver, FocusRestorationToken, InteractionDomainTransitionCapability,
    PreparedCompleteFocusDomainRetirement, PreparedFocusDomainGraphMutation,
    PreparedFocusRestoration, PreparedOverlayFocusActivation, PreparedOverlayFocusExit,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverlayOwnerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverlayStackId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverlayFrameId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayActivationAdmission {
    DomainLocal,
    RouterEligibilityRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayDisposition {
    Suspend,
    Dismiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayOwnerLifecycleState {
    Inactive,
    Active { stack_id: OverlayStackId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayFrameDescriptor {
    pub frame_id: OverlayFrameId,
    pub focus_domain_id: FocusDomainId,
    pub surface_id: SurfaceId,
}

impl OverlayFrameDescriptor {
    pub const fn new(
        frame_id: OverlayFrameId,
        focus_domain_id: FocusDomainId,
        surface_id: SurfaceId,
    ) -> Self {
        Self {
            frame_id,
            focus_domain_id,
            surface_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayStackDescriptor {
    pub stack_id: OverlayStackId,
    pub frames_bottom_first: Vec<OverlayFrameDescriptor>,
}

impl OverlayStackDescriptor {
    pub fn new(stack_id: OverlayStackId, frames_bottom_first: Vec<OverlayFrameDescriptor>) -> Self {
        Self {
            stack_id,
            frames_bottom_first,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayProjection {
    pub owner_id: OverlayOwnerId,
    pub stack_id: OverlayStackId,
    pub surfaces_bottom_first: Vec<SurfaceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveOverlayStackBinding {
    pub owner_id: OverlayOwnerId,
    pub stack_id: OverlayStackId,
    pub generation: u64,
}

/// Sealed owner-issued mechanical exit plan for one prepared overlay deactivation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PreparedOverlaySurfaceExitTestimony<'a> {
    owner_id: OverlayOwnerId,
    stack_id: OverlayStackId,
    disposition: OverlayDisposition,
    surfaces_top_first: &'a [SurfaceId],
}

impl<'a> PreparedOverlaySurfaceExitTestimony<'a> {
    pub(crate) const fn owner_id(self) -> OverlayOwnerId {
        self.owner_id
    }

    pub(crate) const fn stack_id(self) -> OverlayStackId {
        self.stack_id
    }

    pub(crate) const fn disposition(self) -> OverlayDisposition {
        self.disposition
    }

    pub(crate) const fn surfaces_top_first(self) -> &'a [SurfaceId] {
        self.surfaces_top_first
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayDomainError {
    Busy(InteractionDomainId),
    UnknownInteractionDomain(InteractionDomainId),
    DuplicateInteractionDomain(InteractionDomainId),
    DuplicateOwner(OverlayOwnerId),
    UnknownOwner(OverlayOwnerId),
    OwnerDomainMismatch {
        owner_id: OverlayOwnerId,
        expected: InteractionDomainId,
        actual: InteractionDomainId,
    },
    DuplicateStack(OverlayStackId),
    UnknownStack(OverlayStackId),
    EmptyStack(OverlayStackId),
    DuplicateFrame(OverlayFrameId),
    UnknownFrame(OverlayFrameId),
    FrameAlreadyActive(OverlayFrameId),
    CannotPopRootFrame(OverlayStackId),
    DuplicateFocusDomain(FocusDomainId),
    DuplicateSurface(SurfaceId),
    OwnerAlreadyRegistered(OverlayOwnerId),
    OwnerNotRegistered(OverlayOwnerId),
    OwnerBusy(OverlayOwnerId),
    OwnerAlreadyActive(OverlayOwnerId),
    OwnerInactive(OverlayOwnerId),
    ActiveStackConflict(InteractionDomainId),
    ActiveBindingMismatch(InteractionDomainId),
    RetirementRequiresDismiss(OverlayOwnerId),
    DomainHasRegisteredOwners(InteractionDomainId),
    DomainHasActiveStack(InteractionDomainId),
    MissingRouterEligibility(OverlayOwnerId),
    ApplicationLifecycleRequired(OverlayOwnerId),
    RouterEligibilityMismatch(OverlayOwnerId),
    TransitionGuardMismatch,
    PreparedValueMismatch,
    Focus(FocusAuthorityError),
    Exhausted,
}

impl From<FocusAuthorityError> for OverlayDomainError {
    fn from(value: FocusAuthorityError) -> Self {
        Self::Focus(value)
    }
}

#[derive(Debug)]
struct ActiveOwnerStack {
    stack_id: OverlayStackId,
    frames_bottom_first: Vec<OverlayFrameDescriptor>,
    tokens_top_first: Option<Vec<FocusRestorationToken>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerPreparation {
    Activation(u64),
    Deactivation(u64),
    NestedPush(u64),
    NestedPop(u64),
    Unregistration(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortRegistration {
    Unregistered,
    Prepared(u64),
    Registered(u64),
}

#[derive(Debug)]
struct OverlayOwnerLifecycle {
    stacks: HashMap<OverlayStackId, OverlayStackDescriptor>,
    active: Option<ActiveOwnerStack>,
    preparation: Option<OwnerPreparation>,
    unregistration_after_deactivation: Option<u64>,
    next_preparation_generation: u64,
    registration: PortRegistration,
}

#[derive(Debug, Clone)]
pub struct OverlayOwnerLifecyclePort {
    owner_id: OverlayOwnerId,
    interaction_domain_id: InteractionDomainId,
    inner: Rc<RefCell<OverlayOwnerLifecycle>>,
}

impl OverlayOwnerLifecyclePort {
    pub fn new(
        owner_id: OverlayOwnerId,
        interaction_domain_id: InteractionDomainId,
        stacks: Vec<OverlayStackDescriptor>,
    ) -> Result<Self, OverlayDomainError> {
        let mut by_id = HashMap::with_capacity(stacks.len());
        let mut frame_ids = HashSet::new();
        let mut surfaces = HashSet::new();
        for stack in stacks {
            if stack.frames_bottom_first.is_empty() {
                return Err(OverlayDomainError::EmptyStack(stack.stack_id));
            }
            if by_id.contains_key(&stack.stack_id) {
                return Err(OverlayDomainError::DuplicateStack(stack.stack_id));
            }
            let mut focus_domains = HashSet::new();
            for frame in &stack.frames_bottom_first {
                if !frame_ids.insert(frame.frame_id) {
                    return Err(OverlayDomainError::DuplicateFrame(frame.frame_id));
                }
                if !focus_domains.insert(frame.focus_domain_id) {
                    return Err(OverlayDomainError::DuplicateFocusDomain(
                        frame.focus_domain_id,
                    ));
                }
                if !surfaces.insert(frame.surface_id) {
                    return Err(OverlayDomainError::DuplicateSurface(frame.surface_id));
                }
            }
            by_id.insert(stack.stack_id, stack);
        }
        Ok(Self {
            owner_id,
            interaction_domain_id,
            inner: Rc::new(RefCell::new(OverlayOwnerLifecycle {
                stacks: by_id,
                active: None,
                preparation: None,
                unregistration_after_deactivation: None,
                next_preparation_generation: 1,
                registration: PortRegistration::Unregistered,
            })),
        })
    }

    pub const fn owner_id(&self) -> OverlayOwnerId {
        self.owner_id
    }

    pub const fn interaction_domain_id(&self) -> InteractionDomainId {
        self.interaction_domain_id
    }

    pub fn lifecycle_state(&self) -> OverlayOwnerLifecycleState {
        match self.inner.borrow().active.as_ref() {
            Some(active) => OverlayOwnerLifecycleState::Active {
                stack_id: active.stack_id,
            },
            None => OverlayOwnerLifecycleState::Inactive,
        }
    }

    /// Register an owner-local stack while no stack or lifecycle preparation is active.
    pub fn register_inactive_stack(
        &self,
        stack: OverlayStackDescriptor,
    ) -> Result<(), OverlayDomainError> {
        let mut owner = self.inner.borrow_mut();
        if owner.preparation.is_some() || owner.active.is_some() {
            return Err(OverlayDomainError::OwnerBusy(self.owner_id));
        }
        if stack.frames_bottom_first.is_empty() {
            return Err(OverlayDomainError::EmptyStack(stack.stack_id));
        }
        if owner.stacks.contains_key(&stack.stack_id) {
            return Err(OverlayDomainError::DuplicateStack(stack.stack_id));
        }

        let existing_frame_ids = owner
            .stacks
            .values()
            .flat_map(|existing| existing.frames_bottom_first.iter())
            .map(|frame| frame.frame_id)
            .collect::<HashSet<_>>();
        let existing_surfaces = owner
            .stacks
            .values()
            .flat_map(|existing| existing.frames_bottom_first.iter())
            .map(|frame| frame.surface_id)
            .collect::<HashSet<_>>();
        let mut frame_ids = HashSet::new();
        let mut focus_domains = HashSet::new();
        let mut surfaces = HashSet::new();
        for frame in &stack.frames_bottom_first {
            if existing_frame_ids.contains(&frame.frame_id) || !frame_ids.insert(frame.frame_id) {
                return Err(OverlayDomainError::DuplicateFrame(frame.frame_id));
            }
            if !focus_domains.insert(frame.focus_domain_id) {
                return Err(OverlayDomainError::DuplicateFocusDomain(
                    frame.focus_domain_id,
                ));
            }
            if existing_surfaces.contains(&frame.surface_id) || !surfaces.insert(frame.surface_id) {
                return Err(OverlayDomainError::DuplicateSurface(frame.surface_id));
            }
        }
        owner.stacks.insert(stack.stack_id, stack);
        Ok(())
    }

    fn reserve_registration(&self, coordinator_identity: u64) -> Result<(), OverlayDomainError> {
        let mut owner = self.inner.borrow_mut();
        match owner.registration {
            PortRegistration::Unregistered => {
                owner.registration = PortRegistration::Prepared(coordinator_identity);
                Ok(())
            }
            PortRegistration::Prepared(_) | PortRegistration::Registered(_) => {
                Err(OverlayDomainError::OwnerAlreadyRegistered(self.owner_id))
            }
        }
    }

    fn abort_registration(&self, coordinator_identity: u64) {
        let mut owner = self.inner.borrow_mut();
        if owner.registration == PortRegistration::Prepared(coordinator_identity) {
            owner.registration = PortRegistration::Unregistered;
        }
    }

    fn commit_registration(&self, coordinator_identity: u64) {
        let mut owner = self.inner.borrow_mut();
        assert_eq!(
            owner.registration,
            PortRegistration::Prepared(coordinator_identity),
            "overlay owner registration changed after preparation"
        );
        owner.registration = PortRegistration::Registered(coordinator_identity);
    }

    fn begin_activation(
        &self,
        coordinator_identity: u64,
        stack_id: OverlayStackId,
    ) -> Result<PreparedOwnerActivation, OverlayDomainError> {
        let mut owner = self.inner.borrow_mut();
        if owner.registration != PortRegistration::Registered(coordinator_identity) {
            return Err(OverlayDomainError::OwnerNotRegistered(self.owner_id));
        }
        if owner.preparation.is_some() {
            return Err(OverlayDomainError::OwnerBusy(self.owner_id));
        }
        if owner.active.is_some() {
            return Err(OverlayDomainError::OwnerAlreadyActive(self.owner_id));
        }
        let stack = owner
            .stacks
            .get(&stack_id)
            .cloned()
            .ok_or(OverlayDomainError::UnknownStack(stack_id))?;
        let generation = owner.next_preparation_generation;
        owner.next_preparation_generation = generation
            .checked_add(1)
            .ok_or(OverlayDomainError::Exhausted)?;
        owner.preparation = Some(OwnerPreparation::Activation(generation));
        Ok(PreparedOwnerActivation {
            port: self.clone(),
            generation,
            stack,
            committed: false,
        })
    }

    fn begin_deactivation(
        &self,
        coordinator_identity: u64,
        stack_id: OverlayStackId,
    ) -> Result<PreparedOwnerDeactivation, OverlayDomainError> {
        let mut owner = self.inner.borrow_mut();
        if owner.registration != PortRegistration::Registered(coordinator_identity) {
            return Err(OverlayDomainError::OwnerNotRegistered(self.owner_id));
        }
        if owner.preparation.is_some() {
            return Err(OverlayDomainError::OwnerBusy(self.owner_id));
        }
        let generation = owner.next_preparation_generation;
        let next_preparation_generation = generation
            .checked_add(1)
            .ok_or(OverlayDomainError::Exhausted)?;
        let active = owner
            .active
            .as_mut()
            .ok_or(OverlayDomainError::OwnerInactive(self.owner_id))?;
        if active.stack_id != stack_id {
            return Err(OverlayDomainError::ActiveBindingMismatch(
                self.interaction_domain_id,
            ));
        }
        let tokens_top_first = active
            .tokens_top_first
            .take()
            .expect("active overlay owner must hold focus tokens outside preparation");
        let surfaces_top_first = active
            .frames_bottom_first
            .iter()
            .rev()
            .map(|frame| frame.surface_id)
            .collect();
        owner.next_preparation_generation = next_preparation_generation;
        owner.preparation = Some(OwnerPreparation::Deactivation(generation));
        Ok(PreparedOwnerDeactivation {
            port: self.clone(),
            generation,
            stack_id,
            surfaces_top_first,
            tokens_top_first: Some(tokens_top_first),
            committed: false,
        })
    }

    fn begin_nested_push(
        &self,
        coordinator_identity: u64,
        stack_id: OverlayStackId,
        frame_id: OverlayFrameId,
    ) -> Result<PreparedOwnerNestedPush, OverlayDomainError> {
        let mut owner = self.inner.borrow_mut();
        if owner.registration != PortRegistration::Registered(coordinator_identity) {
            return Err(OverlayDomainError::OwnerNotRegistered(self.owner_id));
        }
        if owner.preparation.is_some() {
            return Err(OverlayDomainError::OwnerBusy(self.owner_id));
        }
        let frame = owner
            .stacks
            .values()
            .flat_map(|stack| stack.frames_bottom_first.iter())
            .find(|frame| frame.frame_id == frame_id)
            .copied()
            .ok_or(OverlayDomainError::UnknownFrame(frame_id))?;
        let active = owner
            .active
            .as_ref()
            .ok_or(OverlayDomainError::OwnerInactive(self.owner_id))?;
        if active.stack_id != stack_id {
            return Err(OverlayDomainError::ActiveBindingMismatch(
                self.interaction_domain_id,
            ));
        }
        if active
            .frames_bottom_first
            .iter()
            .any(|active_frame| active_frame.frame_id == frame_id)
        {
            return Err(OverlayDomainError::FrameAlreadyActive(frame_id));
        }
        let generation = owner.next_preparation_generation;
        owner.next_preparation_generation = generation
            .checked_add(1)
            .ok_or(OverlayDomainError::Exhausted)?;
        owner.preparation = Some(OwnerPreparation::NestedPush(generation));
        Ok(PreparedOwnerNestedPush {
            port: self.clone(),
            generation,
            stack_id,
            frame,
            committed: false,
        })
    }

    fn begin_nested_pop(
        &self,
        coordinator_identity: u64,
        stack_id: OverlayStackId,
    ) -> Result<PreparedOwnerNestedPop, OverlayDomainError> {
        let mut owner = self.inner.borrow_mut();
        if owner.registration != PortRegistration::Registered(coordinator_identity) {
            return Err(OverlayDomainError::OwnerNotRegistered(self.owner_id));
        }
        if owner.preparation.is_some() {
            return Err(OverlayDomainError::OwnerBusy(self.owner_id));
        }
        let generation = owner.next_preparation_generation;
        let next_preparation_generation = generation
            .checked_add(1)
            .ok_or(OverlayDomainError::Exhausted)?;
        let active = owner
            .active
            .as_mut()
            .ok_or(OverlayDomainError::OwnerInactive(self.owner_id))?;
        if active.stack_id != stack_id {
            return Err(OverlayDomainError::ActiveBindingMismatch(
                self.interaction_domain_id,
            ));
        }
        if active.frames_bottom_first.len() <= 1 {
            return Err(OverlayDomainError::CannotPopRootFrame(stack_id));
        }
        let frame = *active
            .frames_bottom_first
            .last()
            .expect("non-empty active overlay stack must have a top frame");
        let token = active
            .tokens_top_first
            .as_mut()
            .expect("active overlay owner must hold focus tokens outside preparation")
            .remove(0);
        let remaining_focus_domains = active.frames_bottom_first
            [..active.frames_bottom_first.len() - 1]
            .iter()
            .map(|frame| frame.focus_domain_id)
            .collect();
        owner.next_preparation_generation = next_preparation_generation;
        owner.preparation = Some(OwnerPreparation::NestedPop(generation));
        Ok(PreparedOwnerNestedPop {
            port: self.clone(),
            generation,
            stack_id,
            frame,
            token: Some(token),
            remaining_focus_domains,
            committed: false,
        })
    }

    fn projection_for(&self, stack_id: OverlayStackId) -> OverlayProjection {
        let owner = self.inner.borrow();
        let active = owner
            .active
            .as_ref()
            .expect("published overlay binding must have active owner state");
        assert_eq!(
            active.stack_id, stack_id,
            "published overlay stack changed without coordinator commit"
        );
        OverlayProjection {
            owner_id: self.owner_id,
            stack_id,
            surfaces_bottom_first: active
                .frames_bottom_first
                .iter()
                .map(|frame| frame.surface_id)
                .collect(),
        }
    }

    fn reserve_unregistration(&self, coordinator_identity: u64) -> Result<u64, OverlayDomainError> {
        let mut owner = self.inner.borrow_mut();
        if owner.registration != PortRegistration::Registered(coordinator_identity) {
            return Err(OverlayDomainError::OwnerNotRegistered(self.owner_id));
        }
        if owner.preparation.is_some() {
            return Err(OverlayDomainError::OwnerBusy(self.owner_id));
        }
        if owner.active.is_some() {
            return Err(OverlayDomainError::OwnerAlreadyActive(self.owner_id));
        }
        let generation = owner.next_preparation_generation;
        owner.next_preparation_generation = generation
            .checked_add(1)
            .ok_or(OverlayDomainError::Exhausted)?;
        owner.preparation = Some(OwnerPreparation::Unregistration(generation));
        Ok(generation)
    }

    fn reserve_unregistration_after_deactivation(
        &self,
        coordinator_identity: u64,
        deactivation_generation: u64,
    ) -> Result<u64, OverlayDomainError> {
        let mut owner = self.inner.borrow_mut();
        if owner.registration != PortRegistration::Registered(coordinator_identity) {
            return Err(OverlayDomainError::OwnerNotRegistered(self.owner_id));
        }
        if owner.preparation != Some(OwnerPreparation::Deactivation(deactivation_generation))
            || owner.active.is_none()
            || owner.unregistration_after_deactivation.is_some()
        {
            return Err(OverlayDomainError::OwnerBusy(self.owner_id));
        }
        let generation = owner.next_preparation_generation;
        owner.next_preparation_generation = generation
            .checked_add(1)
            .ok_or(OverlayDomainError::Exhausted)?;
        owner.unregistration_after_deactivation = Some(generation);
        Ok(generation)
    }

    fn abort_unregistration(&self, generation: u64) {
        let mut owner = self.inner.borrow_mut();
        if owner.unregistration_after_deactivation == Some(generation) {
            owner.unregistration_after_deactivation = None;
            return;
        }
        if owner.preparation == Some(OwnerPreparation::Unregistration(generation)) {
            owner.preparation = None;
        }
    }

    fn commit_unregistration(&self, coordinator_identity: u64, generation: u64) {
        let mut owner = self.inner.borrow_mut();
        assert_eq!(
            owner.registration,
            PortRegistration::Registered(coordinator_identity),
            "overlay owner registration changed after unregistration preparation"
        );
        assert_eq!(
            owner.preparation,
            Some(OwnerPreparation::Unregistration(generation)),
            "overlay owner unregistration changed after preparation"
        );
        assert!(
            owner.active.is_none(),
            "overlay owner became active after unregistration preparation"
        );
        owner.preparation = None;
        owner.registration = PortRegistration::Unregistered;
    }
}

#[derive(Debug)]
struct PreparedOwnerActivation {
    port: OverlayOwnerLifecyclePort,
    generation: u64,
    stack: OverlayStackDescriptor,
    committed: bool,
}

impl PreparedOwnerActivation {
    fn focus_domains_bottom_first(&self) -> Vec<FocusDomainId> {
        self.stack
            .frames_bottom_first
            .iter()
            .map(|frame| frame.focus_domain_id)
            .collect()
    }

    fn commit(mut self, mut tokens_bottom_first: Vec<FocusRestorationToken>) {
        let mut owner = self.port.inner.borrow_mut();
        assert_eq!(
            owner.preparation,
            Some(OwnerPreparation::Activation(self.generation)),
            "overlay owner activation changed after preparation"
        );
        assert!(
            owner.active.is_none(),
            "overlay owner became active after preparation"
        );
        tokens_bottom_first.reverse();
        owner.active = Some(ActiveOwnerStack {
            stack_id: self.stack.stack_id,
            frames_bottom_first: self.stack.frames_bottom_first.clone(),
            tokens_top_first: Some(tokens_bottom_first),
        });
        owner.preparation = None;
        self.committed = true;
    }
}

impl Drop for PreparedOwnerActivation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut owner = self.port.inner.borrow_mut();
        if owner.preparation == Some(OwnerPreparation::Activation(self.generation)) {
            owner.preparation = None;
        }
    }
}

#[derive(Debug)]
struct PreparedOwnerDeactivation {
    port: OverlayOwnerLifecyclePort,
    generation: u64,
    stack_id: OverlayStackId,
    surfaces_top_first: Vec<SurfaceId>,
    tokens_top_first: Option<Vec<FocusRestorationToken>>,
    committed: bool,
}

#[derive(Debug)]
struct PreparedOwnerNestedPush {
    port: OverlayOwnerLifecyclePort,
    generation: u64,
    stack_id: OverlayStackId,
    frame: OverlayFrameDescriptor,
    committed: bool,
}

impl PreparedOwnerNestedPush {
    fn commit(mut self, mut tokens_bottom_first: Vec<FocusRestorationToken>) {
        assert_eq!(
            tokens_bottom_first.len(),
            1,
            "one nested overlay frame must produce one focus token"
        );
        let token = tokens_bottom_first
            .pop()
            .expect("nested overlay focus token count was validated");
        let mut owner = self.port.inner.borrow_mut();
        assert_eq!(
            owner.preparation,
            Some(OwnerPreparation::NestedPush(self.generation)),
            "overlay nested push changed after preparation"
        );
        let active = owner
            .active
            .as_mut()
            .expect("prepared nested push must remain active");
        assert_eq!(
            active.stack_id, self.stack_id,
            "active overlay stack changed after nested push preparation"
        );
        active.frames_bottom_first.push(self.frame);
        active
            .tokens_top_first
            .as_mut()
            .expect("active overlay owner must retain focus tokens")
            .insert(0, token);
        owner.preparation = None;
        self.committed = true;
    }
}

impl Drop for PreparedOwnerNestedPush {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut owner = self.port.inner.borrow_mut();
        if owner.preparation == Some(OwnerPreparation::NestedPush(self.generation)) {
            owner.preparation = None;
        }
    }
}

#[derive(Debug)]
struct PreparedOwnerNestedPop {
    port: OverlayOwnerLifecyclePort,
    generation: u64,
    stack_id: OverlayStackId,
    frame: OverlayFrameDescriptor,
    token: Option<FocusRestorationToken>,
    remaining_focus_domains: HashSet<FocusDomainId>,
    committed: bool,
}

impl PreparedOwnerNestedPop {
    fn token(&self) -> &FocusRestorationToken {
        self.token
            .as_ref()
            .expect("prepared nested pop must retain its focus token")
    }

    fn commit(mut self) {
        let mut owner = self.port.inner.borrow_mut();
        assert_eq!(
            owner.preparation,
            Some(OwnerPreparation::NestedPop(self.generation)),
            "overlay nested pop changed after preparation"
        );
        let active = owner
            .active
            .as_mut()
            .expect("prepared nested pop must remain active");
        assert_eq!(
            active.stack_id, self.stack_id,
            "active overlay stack changed after nested pop preparation"
        );
        assert_eq!(
            active.frames_bottom_first.last(),
            Some(&self.frame),
            "active overlay top frame changed after nested pop preparation"
        );
        active.frames_bottom_first.pop();
        self.token.take();
        owner.preparation = None;
        self.committed = true;
    }
}

impl Drop for PreparedOwnerNestedPop {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut owner = self.port.inner.borrow_mut();
        if owner.preparation == Some(OwnerPreparation::NestedPop(self.generation)) {
            let active = owner
                .active
                .as_mut()
                .expect("aborted nested pop must restore an active owner");
            active
                .tokens_top_first
                .as_mut()
                .expect("aborted nested pop must restore owner focus tokens")
                .insert(
                    0,
                    self.token
                        .take()
                        .expect("aborted nested pop must retain its focus token"),
                );
            owner.preparation = None;
        }
    }
}

impl PreparedOwnerDeactivation {
    fn tokens_top_first(&self) -> &[FocusRestorationToken] {
        self.tokens_top_first
            .as_deref()
            .expect("prepared overlay deactivation must retain focus tokens")
    }

    fn surfaces_top_first(&self) -> &[SurfaceId] {
        &self.surfaces_top_first
    }

    fn commit(mut self, disposition: OverlayDisposition) {
        let mut owner = self.port.inner.borrow_mut();
        assert_eq!(
            owner.preparation,
            Some(OwnerPreparation::Deactivation(self.generation)),
            "overlay owner deactivation changed after preparation"
        );
        let dismissed_frame_ids = {
            let active = owner
                .active
                .as_ref()
                .expect("prepared overlay deactivation must remain semantically active");
            assert_eq!(
                active.stack_id, self.stack_id,
                "active overlay stack changed after deactivation preparation"
            );
            assert!(
                active.tokens_top_first.is_none(),
                "prepared deactivation tokens returned before commit"
            );
            (disposition == OverlayDisposition::Dismiss).then(|| {
                active
                    .frames_bottom_first
                    .iter()
                    .map(|frame| frame.frame_id)
                    .collect::<HashSet<_>>()
            })
        };
        self.tokens_top_first.take();
        owner.active = None;
        if let Some(dismissed_frame_ids) = dismissed_frame_ids {
            for stack in owner.stacks.values_mut() {
                stack
                    .frames_bottom_first
                    .retain(|frame| !dismissed_frame_ids.contains(&frame.frame_id));
            }
            owner
                .stacks
                .retain(|_, stack| !stack.frames_bottom_first.is_empty());
        }
        owner.preparation = owner
            .unregistration_after_deactivation
            .take()
            .map(OwnerPreparation::Unregistration);
        self.committed = true;
    }
}

impl Drop for PreparedOwnerDeactivation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut owner = self.port.inner.borrow_mut();
        if owner.preparation == Some(OwnerPreparation::Deactivation(self.generation)) {
            let active = owner
                .active
                .as_mut()
                .expect("aborted overlay deactivation must restore active owner state");
            active.tokens_top_first = self.tokens_top_first.take();
            owner.preparation = None;
        }
    }
}

#[derive(Debug)]
struct DomainGuardState {
    held_generation: Option<u64>,
    next_generation: u64,
}

#[derive(Debug)]
struct OverlayDomainRecord {
    guard: Rc<RefCell<DomainGuardState>>,
    active_binding: Option<ActiveOverlayStackBinding>,
    next_binding_generation: u64,
}

#[derive(Debug, Clone)]
struct RegisteredOwner {
    port: OverlayOwnerLifecyclePort,
    admission: OverlayActivationAdmission,
    registration_generation: u64,
}

#[derive(Debug)]
struct CoordinatorState {
    identity: u64,
    domains: HashMap<InteractionDomainId, OverlayDomainRecord>,
    owners: HashMap<OverlayOwnerId, RegisteredOwner>,
    reserved_domains: HashSet<InteractionDomainId>,
    reserved_owners: HashMap<OverlayOwnerId, InteractionDomainId>,
    retiring_domains: HashSet<InteractionDomainId>,
    retiring_owners: HashSet<OverlayOwnerId>,
    next_owner_registration_generation: u64,
}

static NEXT_COORDINATOR_IDENTITY: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct OverlayDomainCoordinator {
    inner: Rc<RefCell<CoordinatorState>>,
}

impl OverlayDomainCoordinator {
    pub fn new() -> Result<Self, OverlayDomainError> {
        let identity = NEXT_COORDINATOR_IDENTITY
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| OverlayDomainError::Exhausted)?;
        Ok(Self {
            inner: Rc::new(RefCell::new(CoordinatorState {
                identity,
                domains: HashMap::new(),
                owners: HashMap::new(),
                reserved_domains: HashSet::new(),
                reserved_owners: HashMap::new(),
                retiring_domains: HashSet::new(),
                retiring_owners: HashSet::new(),
                next_owner_registration_generation: 1,
            })),
        })
    }

    pub(crate) fn identity(&self) -> u64 {
        self.inner.borrow().identity
    }

    pub fn prepare_domain_registration(
        &self,
        interaction_domains: &InteractionDomains,
        domain_id: InteractionDomainId,
    ) -> Result<PreparedOverlayDomainRegistration, OverlayDomainError> {
        if interaction_domains.state(domain_id).is_none() {
            return Err(OverlayDomainError::UnknownInteractionDomain(domain_id));
        }
        let mut state = self.inner.borrow_mut();
        if state.domains.contains_key(&domain_id)
            || state.retiring_domains.contains(&domain_id)
            || !state.reserved_domains.insert(domain_id)
        {
            return Err(OverlayDomainError::DuplicateInteractionDomain(domain_id));
        }
        Ok(PreparedOverlayDomainRegistration {
            coordinator: self.clone(),
            domain_id,
            committed: false,
        })
    }

    pub fn commit_domain_registration(&self, mut prepared: PreparedOverlayDomainRegistration) {
        self.assert_prepared_coordinator(&prepared.coordinator);
        let mut state = self.inner.borrow_mut();
        assert!(
            state.reserved_domains.remove(&prepared.domain_id),
            "overlay domain reservation changed after preparation"
        );
        assert!(
            state
                .domains
                .insert(
                    prepared.domain_id,
                    OverlayDomainRecord {
                        guard: Rc::new(RefCell::new(DomainGuardState {
                            held_generation: None,
                            next_generation: 1,
                        })),
                        active_binding: None,
                        next_binding_generation: 1,
                    },
                )
                .is_none(),
            "overlay domain became registered after preparation"
        );
        prepared.committed = true;
    }

    pub fn register_domain(
        &self,
        interaction_domains: &InteractionDomains,
        domain_id: InteractionDomainId,
    ) -> Result<(), OverlayDomainError> {
        let prepared = self.prepare_domain_registration(interaction_domains, domain_id)?;
        self.commit_domain_registration(prepared);
        Ok(())
    }

    pub fn prepare_owner_registration(
        &self,
        port: OverlayOwnerLifecyclePort,
        admission: OverlayActivationAdmission,
    ) -> Result<PreparedOverlayOwnerRegistration, OverlayDomainError> {
        let mut state = self.inner.borrow_mut();
        if !state.domains.contains_key(&port.interaction_domain_id) {
            return Err(OverlayDomainError::UnknownInteractionDomain(
                port.interaction_domain_id,
            ));
        }
        if state.retiring_domains.contains(&port.interaction_domain_id) {
            return Err(OverlayDomainError::Busy(port.interaction_domain_id));
        }
        let registration_generation = state.next_owner_registration_generation;
        let next_registration_generation = registration_generation
            .checked_add(1)
            .ok_or(OverlayDomainError::Exhausted)?;
        if state.owners.contains_key(&port.owner_id)
            || state.retiring_owners.contains(&port.owner_id)
            || state.reserved_owners.contains_key(&port.owner_id)
        {
            return Err(OverlayDomainError::DuplicateOwner(port.owner_id));
        }
        state
            .reserved_owners
            .insert(port.owner_id, port.interaction_domain_id);
        state.next_owner_registration_generation = next_registration_generation;
        if let Err(error) = port.reserve_registration(state.identity) {
            state.reserved_owners.remove(&port.owner_id);
            return Err(error);
        }
        Ok(PreparedOverlayOwnerRegistration {
            coordinator: self.clone(),
            port,
            admission,
            registration_generation,
            committed: false,
        })
    }

    pub fn commit_owner_registration(&self, mut prepared: PreparedOverlayOwnerRegistration) {
        self.assert_prepared_coordinator(&prepared.coordinator);
        let mut state = self.inner.borrow_mut();
        assert_eq!(
            state.reserved_owners.remove(&prepared.port.owner_id),
            Some(prepared.port.interaction_domain_id),
            "overlay owner reservation changed after preparation"
        );
        prepared.port.commit_registration(state.identity);
        assert!(
            state
                .owners
                .insert(
                    prepared.port.owner_id,
                    RegisteredOwner {
                        port: prepared.port.clone(),
                        admission: prepared.admission,
                        registration_generation: prepared.registration_generation,
                    },
                )
                .is_none(),
            "overlay owner became registered after preparation"
        );
        prepared.committed = true;
    }

    pub fn register_owner(
        &self,
        port: OverlayOwnerLifecyclePort,
        admission: OverlayActivationAdmission,
    ) -> Result<(), OverlayDomainError> {
        let prepared = self.prepare_owner_registration(port, admission)?;
        self.commit_owner_registration(prepared);
        Ok(())
    }

    pub(crate) fn prepare_owner_unregistration(
        &self,
        guard: &DomainTransitionGuardToken,
        owner_id: OverlayOwnerId,
    ) -> Result<PreparedOverlayOwnerUnregistration, OverlayDomainError> {
        self.validated_state(guard)?;
        let owner = {
            let mut state = self.inner.borrow_mut();
            let owner = state
                .owners
                .get(&owner_id)
                .cloned()
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if owner.port.interaction_domain_id != guard.domain_id {
                return Err(OverlayDomainError::OwnerDomainMismatch {
                    owner_id,
                    expected: owner.port.interaction_domain_id,
                    actual: guard.domain_id,
                });
            }
            if state
                .domains
                .get(&guard.domain_id)
                .and_then(|domain| domain.active_binding)
                .is_some_and(|binding| binding.owner_id == owner_id)
            {
                return Err(OverlayDomainError::OwnerAlreadyActive(owner_id));
            }
            if !state.retiring_owners.insert(owner_id) {
                return Err(OverlayDomainError::OwnerBusy(owner_id));
            }
            owner
        };
        let generation = match owner
            .port
            .reserve_unregistration(guard.coordinator_identity)
        {
            Ok(generation) => generation,
            Err(error) => {
                self.inner.borrow_mut().retiring_owners.remove(&owner_id);
                return Err(error);
            }
        };
        Ok(PreparedOverlayOwnerUnregistration {
            coordinator: self.clone(),
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            owner_id,
            port: owner.port,
            generation,
            committed: false,
        })
    }

    pub fn prepare_owner_unregistration_after_focus_retirement_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        focus_retirement: &PreparedOverlayFocusGraphTransition<'_>,
    ) -> Result<PreparedOverlayOwnerUnregistration, OverlayDomainError> {
        self.assert_prepared_transition(
            guard,
            focus_retirement.coordinator_identity,
            focus_retirement.domain_id,
            focus_retirement.guard_generation,
        );
        let owner_id = focus_retirement.binding.owner_id;
        let owner = {
            let mut state = self.inner.borrow_mut();
            let owner = state
                .owners
                .get(&owner_id)
                .cloned()
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if !Rc::ptr_eq(&owner.port.inner, &focus_retirement.owner.port.inner)
                || state
                    .domains
                    .get(&guard.domain_id)
                    .and_then(|domain| domain.active_binding)
                    != Some(focus_retirement.binding)
            {
                return Err(OverlayDomainError::PreparedValueMismatch);
            }
            if !state.retiring_owners.insert(owner_id) {
                return Err(OverlayDomainError::OwnerBusy(owner_id));
            }
            owner
        };
        let generation = match owner.port.reserve_unregistration_after_deactivation(
            guard.coordinator_identity,
            focus_retirement.owner.generation,
        ) {
            Ok(generation) => generation,
            Err(error) => {
                self.inner.borrow_mut().retiring_owners.remove(&owner_id);
                return Err(error);
            }
        };
        Ok(PreparedOverlayOwnerUnregistration {
            coordinator: self.clone(),
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            owner_id,
            port: owner.port,
            generation,
            committed: false,
        })
    }

    pub(crate) fn prepare_owner_unregistration_after_complete_domain_retirement_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        focus_retirement: &PreparedOverlayCompleteDomainRetirement,
    ) -> Result<PreparedOverlayOwnerUnregistration, OverlayDomainError> {
        self.assert_prepared_transition(
            guard,
            focus_retirement.coordinator_identity,
            focus_retirement.domain_id,
            focus_retirement.guard_generation,
        );
        let owner_id = focus_retirement.binding.owner_id;
        let owner = {
            let mut state = self.inner.borrow_mut();
            let owner = state
                .owners
                .get(&owner_id)
                .cloned()
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if !Rc::ptr_eq(&owner.port.inner, &focus_retirement.owner.port.inner)
                || state
                    .domains
                    .get(&guard.domain_id)
                    .and_then(|domain| domain.active_binding)
                    != Some(focus_retirement.binding)
            {
                return Err(OverlayDomainError::PreparedValueMismatch);
            }
            if !state.retiring_owners.insert(owner_id) {
                return Err(OverlayDomainError::OwnerBusy(owner_id));
            }
            owner
        };
        let generation = match owner.port.reserve_unregistration_after_deactivation(
            guard.coordinator_identity,
            focus_retirement.owner.generation,
        ) {
            Ok(generation) => generation,
            Err(error) => {
                self.inner.borrow_mut().retiring_owners.remove(&owner_id);
                return Err(error);
            }
        };
        Ok(PreparedOverlayOwnerUnregistration {
            coordinator: self.clone(),
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            owner_id,
            port: owner.port,
            generation,
            committed: false,
        })
    }

    pub(crate) fn commit_owner_unregistration(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedOverlayOwnerUnregistration,
    ) {
        self.assert_prepared_transition(
            guard,
            prepared.coordinator_identity,
            prepared.domain_id,
            prepared.guard_generation,
        );
        let mut state = self.inner.borrow_mut();
        assert!(
            state.retiring_owners.remove(&prepared.owner_id),
            "overlay owner retirement reservation changed after preparation"
        );
        let removed = state
            .owners
            .remove(&prepared.owner_id)
            .expect("prepared overlay owner must remain registered");
        assert!(
            Rc::ptr_eq(&removed.port.inner, &prepared.port.inner),
            "overlay owner lifecycle port changed after unregistration preparation"
        );
        prepared
            .port
            .commit_unregistration(state.identity, prepared.generation);
        prepared.committed = true;
    }

    pub fn unregister_owner(&self, owner_id: OverlayOwnerId) -> Result<(), OverlayDomainError> {
        let domain_id = {
            let state = self.inner.borrow();
            let owner = state
                .owners
                .get(&owner_id)
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if owner.admission == OverlayActivationAdmission::RouterEligibilityRequired {
                return Err(OverlayDomainError::ApplicationLifecycleRequired(owner_id));
            }
            owner.port.interaction_domain_id
        };
        let guard = self.begin_domain_transition(domain_id)?;
        let prepared = self.prepare_owner_unregistration(&guard, owner_id)?;
        self.commit_owner_unregistration(&guard, prepared);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn prepare_domain_unregistration(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<PreparedOverlayDomainUnregistration, OverlayDomainError> {
        self.validated_state(guard)?;
        let mut state = self.inner.borrow_mut();
        let domain = state
            .domains
            .get(&guard.domain_id)
            .expect("validated transition guard domain must remain registered");
        if domain.active_binding.is_some() {
            return Err(OverlayDomainError::DomainHasActiveStack(guard.domain_id));
        }
        if state
            .owners
            .values()
            .any(|owner| owner.port.interaction_domain_id == guard.domain_id)
            || state
                .reserved_owners
                .values()
                .any(|domain_id| *domain_id == guard.domain_id)
            || !state.retiring_owners.is_empty()
        {
            return Err(OverlayDomainError::DomainHasRegisteredOwners(
                guard.domain_id,
            ));
        }
        if !state.retiring_domains.insert(guard.domain_id) {
            return Err(OverlayDomainError::Busy(guard.domain_id));
        }
        Ok(PreparedOverlayDomainUnregistration {
            coordinator: self.clone(),
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            committed: false,
        })
    }

    pub(crate) fn prepare_domain_unregistration_after_owner_reservations(
        &self,
        guard: &DomainTransitionGuardToken,
        owner_unregistrations: &[&PreparedOverlayOwnerUnregistration],
        active_focus_retirement: Option<&PreparedOverlayCompleteDomainRetirement>,
    ) -> Result<PreparedOverlayDomainUnregistration, OverlayDomainError> {
        self.validated_state(guard)?;
        let mut prepared_owner_ids = HashSet::new();
        for prepared in owner_unregistrations {
            if prepared.coordinator_identity != guard.coordinator_identity
                || prepared.domain_id != guard.domain_id
                || prepared.guard_generation != guard.generation
                || !prepared_owner_ids.insert(prepared.owner_id)
            {
                return Err(OverlayDomainError::PreparedValueMismatch);
            }
        }
        let mut state = self.inner.borrow_mut();
        let domain = state
            .domains
            .get(&guard.domain_id)
            .expect("validated transition guard domain must remain registered");
        let registered_owner_ids = state
            .owners
            .iter()
            .filter_map(|(owner_id, owner)| {
                (owner.port.interaction_domain_id == guard.domain_id).then_some(*owner_id)
            })
            .collect::<HashSet<_>>();
        if registered_owner_ids != prepared_owner_ids
            || prepared_owner_ids
                .iter()
                .any(|owner_id| !state.retiring_owners.contains(owner_id))
            || state
                .reserved_owners
                .values()
                .any(|domain_id| *domain_id == guard.domain_id)
        {
            return Err(OverlayDomainError::DomainHasRegisteredOwners(
                guard.domain_id,
            ));
        }
        match (domain.active_binding, active_focus_retirement) {
            (None, None) => {}
            (Some(binding), Some(prepared))
                if prepared.coordinator_identity == guard.coordinator_identity
                    && prepared.domain_id == guard.domain_id
                    && prepared.guard_generation == guard.generation
                    && prepared.binding == binding
                    && prepared_owner_ids.contains(&binding.owner_id) => {}
            _ => return Err(OverlayDomainError::PreparedValueMismatch),
        }
        if !state.retiring_domains.insert(guard.domain_id) {
            return Err(OverlayDomainError::Busy(guard.domain_id));
        }
        Ok(PreparedOverlayDomainUnregistration {
            coordinator: self.clone(),
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            committed: false,
        })
    }

    pub(crate) fn commit_domain_unregistration(
        &self,
        guard: &DomainTransitionGuardToken,
        mut prepared: PreparedOverlayDomainUnregistration,
    ) {
        self.assert_prepared_transition(
            guard,
            prepared.coordinator_identity,
            prepared.domain_id,
            prepared.guard_generation,
        );
        let mut state = self.inner.borrow_mut();
        assert!(
            state.retiring_domains.remove(&prepared.domain_id),
            "overlay domain retirement reservation changed after preparation"
        );
        let domain = state
            .domains
            .remove(&prepared.domain_id)
            .expect("prepared overlay domain must remain registered");
        assert!(
            domain.active_binding.is_none(),
            "overlay domain became active after unregistration preparation"
        );
        prepared.committed = true;
    }

    #[cfg(test)]
    pub(crate) fn unregister_domain(
        &self,
        domain_id: InteractionDomainId,
    ) -> Result<(), OverlayDomainError> {
        let guard = self.begin_domain_transition(domain_id)?;
        let prepared = self.prepare_domain_unregistration(&guard)?;
        self.commit_domain_unregistration(&guard, prepared);
        Ok(())
    }

    pub fn begin_domain_transition(
        &self,
        domain_id: InteractionDomainId,
    ) -> Result<DomainTransitionGuardToken, OverlayDomainError> {
        let (identity, guard) = {
            let state = self.inner.borrow();
            let domain = state
                .domains
                .get(&domain_id)
                .ok_or(OverlayDomainError::UnknownInteractionDomain(domain_id))?;
            (state.identity, domain.guard.clone())
        };
        let mut guard_state = guard.borrow_mut();
        if guard_state.held_generation.is_some() {
            return Err(OverlayDomainError::Busy(domain_id));
        }
        let generation = guard_state.next_generation;
        guard_state.next_generation = generation
            .checked_add(1)
            .ok_or(OverlayDomainError::Exhausted)?;
        let capability = InteractionDomainTransitionCapability::issue(domain_id, generation)?;
        guard_state.held_generation = Some(generation);
        drop(guard_state);
        Ok(DomainTransitionGuardToken {
            coordinator_identity: identity,
            domain_id,
            generation,
            guard,
            capability,
        })
    }

    pub fn prepare_activation<'a>(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &'a mut InteractionDomains,
        owner_id: OverlayOwnerId,
        stack_id: OverlayStackId,
        router_eligibility: Option<RouterEligibilityTestimony>,
    ) -> Result<PreparedOverlayActivation<'a>, OverlayDomainError> {
        let owner = {
            let state = self.validated_state(guard)?;
            let owner = state
                .owners
                .get(&owner_id)
                .cloned()
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if owner.port.interaction_domain_id != guard.domain_id {
                return Err(OverlayDomainError::OwnerDomainMismatch {
                    owner_id,
                    expected: owner.port.interaction_domain_id,
                    actual: guard.domain_id,
                });
            }
            let domain = state
                .domains
                .get(&guard.domain_id)
                .expect("validated transition guard domain must remain registered");
            if domain.active_binding.is_some() {
                return Err(OverlayDomainError::ActiveStackConflict(guard.domain_id));
            }
            owner
        };
        let binding_generation = self.reserve_binding_generation(guard)?;
        let owner_prepared = owner
            .port
            .begin_activation(guard.coordinator_identity, stack_id)?;
        let focus_domains = owner_prepared.focus_domains_bottom_first();
        self.validate_router_eligibility(
            &owner,
            guard.coordinator_identity,
            focus_domains.iter().copied().collect(),
            router_eligibility.as_ref(),
        )?;
        let manager = &mut interaction_domains
            .state_mut(guard.domain_id)
            .ok_or(OverlayDomainError::UnknownInteractionDomain(
                guard.domain_id,
            ))?
            .focus_manager;
        let focus_prepared = FocusResolver::new().prepare_overlay_focus_activation(
            &guard.capability,
            manager,
            &focus_domains,
        )?;
        Ok(PreparedOverlayActivation {
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            binding_generation,
            owner: owner_prepared,
            focus: focus_prepared,
        })
    }

    pub fn commit_activation(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: PreparedOverlayActivation<'_>,
    ) {
        self.assert_prepared_transition(
            guard,
            prepared.coordinator_identity,
            prepared.domain_id,
            prepared.guard_generation,
        );
        let owner_id = prepared.owner.port.owner_id;
        let stack_id = prepared.owner.stack.stack_id;
        let tokens =
            FocusResolver::new().commit_overlay_focus_activation(&guard.capability, prepared.focus);
        prepared.owner.commit(tokens);
        let mut state = self.inner.borrow_mut();
        let domain = state
            .domains
            .get_mut(&guard.domain_id)
            .expect("prepared activation domain must remain registered");
        assert!(
            domain.active_binding.is_none(),
            "active overlay binding appeared after preparation"
        );
        assert_eq!(
            domain.next_binding_generation,
            prepared
                .binding_generation
                .checked_add(1)
                .expect("prepared overlay binding generation must have a successor"),
            "overlay binding generation changed after preparation"
        );
        domain.active_binding = Some(ActiveOverlayStackBinding {
            owner_id,
            stack_id,
            generation: prepared.binding_generation,
        });
    }

    pub fn activate_owner(
        &self,
        interaction_domains: &mut InteractionDomains,
        owner_id: OverlayOwnerId,
        stack_id: OverlayStackId,
        router_eligibility: Option<RouterEligibilityTestimony>,
    ) -> Result<(), OverlayDomainError> {
        let domain_id = self.owner_domain(owner_id)?;
        let guard = self.begin_domain_transition(domain_id)?;
        let prepared = self.prepare_activation(
            &guard,
            interaction_domains,
            owner_id,
            stack_id,
            router_eligibility,
        )?;
        self.commit_activation(&guard, prepared);
        Ok(())
    }

    pub fn prepare_nested_frame_push<'a>(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &'a mut InteractionDomains,
        owner_id: OverlayOwnerId,
        frame_id: OverlayFrameId,
        router_eligibility: Option<RouterEligibilityTestimony>,
    ) -> Result<PreparedOverlayNestedFramePush<'a>, OverlayDomainError> {
        let (owner, binding) = {
            let state = self.validated_state(guard)?;
            let owner = state
                .owners
                .get(&owner_id)
                .cloned()
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if owner.port.interaction_domain_id != guard.domain_id {
                return Err(OverlayDomainError::OwnerDomainMismatch {
                    owner_id,
                    expected: owner.port.interaction_domain_id,
                    actual: guard.domain_id,
                });
            }
            let domain = state
                .domains
                .get(&guard.domain_id)
                .expect("validated transition guard domain must remain registered");
            let binding = domain
                .active_binding
                .ok_or(OverlayDomainError::OwnerInactive(owner_id))?;
            if binding.owner_id != owner_id {
                return Err(OverlayDomainError::ActiveBindingMismatch(guard.domain_id));
            }
            (owner, binding)
        };
        let next_binding_generation = self.reserve_binding_generation(guard)?;
        let owner_prepared =
            owner
                .port
                .begin_nested_push(guard.coordinator_identity, binding.stack_id, frame_id)?;
        let expected_focus_domains = {
            let lifecycle = owner.port.inner.borrow();
            let mut domains = lifecycle
                .active
                .as_ref()
                .expect("prepared nested push owner must remain active")
                .frames_bottom_first
                .iter()
                .map(|frame| frame.focus_domain_id)
                .collect::<HashSet<_>>();
            domains.insert(owner_prepared.frame.focus_domain_id);
            domains
        };
        self.validate_router_eligibility(
            &owner,
            guard.coordinator_identity,
            expected_focus_domains,
            router_eligibility.as_ref(),
        )?;
        let manager = &mut interaction_domains
            .state_mut(guard.domain_id)
            .ok_or(OverlayDomainError::UnknownInteractionDomain(
                guard.domain_id,
            ))?
            .focus_manager;
        let focus_prepared = FocusResolver::new().prepare_overlay_focus_activation(
            &guard.capability,
            manager,
            &[owner_prepared.frame.focus_domain_id],
        )?;
        Ok(PreparedOverlayNestedFramePush {
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            binding,
            next_binding_generation,
            owner: owner_prepared,
            focus: focus_prepared,
        })
    }

    pub fn commit_nested_frame_push(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: PreparedOverlayNestedFramePush<'_>,
    ) {
        self.assert_prepared_transition(
            guard,
            prepared.coordinator_identity,
            prepared.domain_id,
            prepared.guard_generation,
        );
        let tokens =
            FocusResolver::new().commit_overlay_focus_activation(&guard.capability, prepared.focus);
        prepared.owner.commit(tokens);
        self.commit_binding_generation(guard, prepared.binding, prepared.next_binding_generation);
    }

    pub fn push_nested_frame(
        &self,
        interaction_domains: &mut InteractionDomains,
        owner_id: OverlayOwnerId,
        frame_id: OverlayFrameId,
        router_eligibility: Option<RouterEligibilityTestimony>,
    ) -> Result<(), OverlayDomainError> {
        let domain_id = self.owner_domain(owner_id)?;
        let guard = self.begin_domain_transition(domain_id)?;
        let prepared = self.prepare_nested_frame_push(
            &guard,
            interaction_domains,
            owner_id,
            frame_id,
            router_eligibility,
        )?;
        self.commit_nested_frame_push(&guard, prepared);
        Ok(())
    }

    pub fn prepare_nested_frame_pop<'a>(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &'a mut InteractionDomains,
        owner_id: OverlayOwnerId,
        router_eligibility: Option<RouterEligibilityTestimony>,
    ) -> Result<PreparedOverlayNestedFramePop<'a>, OverlayDomainError> {
        let (owner, binding) = {
            let state = self.validated_state(guard)?;
            let owner = state
                .owners
                .get(&owner_id)
                .cloned()
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if owner.port.interaction_domain_id != guard.domain_id {
                return Err(OverlayDomainError::OwnerDomainMismatch {
                    owner_id,
                    expected: owner.port.interaction_domain_id,
                    actual: guard.domain_id,
                });
            }
            let domain = state
                .domains
                .get(&guard.domain_id)
                .expect("validated transition guard domain must remain registered");
            let binding = domain
                .active_binding
                .ok_or(OverlayDomainError::OwnerInactive(owner_id))?;
            if binding.owner_id != owner_id {
                return Err(OverlayDomainError::ActiveBindingMismatch(guard.domain_id));
            }
            (owner, binding)
        };
        let next_binding_generation = self.reserve_binding_generation(guard)?;
        let owner_prepared = owner
            .port
            .begin_nested_pop(guard.coordinator_identity, binding.stack_id)?;
        self.validate_router_eligibility(
            &owner,
            guard.coordinator_identity,
            owner_prepared.remaining_focus_domains.clone(),
            router_eligibility.as_ref(),
        )?;
        let manager = &mut interaction_domains
            .state_mut(guard.domain_id)
            .ok_or(OverlayDomainError::UnknownInteractionDomain(
                guard.domain_id,
            ))?
            .focus_manager;
        let focus_prepared = FocusResolver::new().prepare_restoration_token_refs(
            &guard.capability,
            manager,
            std::slice::from_ref(owner_prepared.token()),
        )?;
        Ok(PreparedOverlayNestedFramePop {
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            binding,
            next_binding_generation,
            owner: owner_prepared,
            focus: focus_prepared,
        })
    }

    pub fn commit_nested_frame_pop(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: PreparedOverlayNestedFramePop<'_>,
    ) {
        self.assert_prepared_transition(
            guard,
            prepared.coordinator_identity,
            prepared.domain_id,
            prepared.guard_generation,
        );
        prepared.owner.commit();
        FocusResolver::new().submit_restoration_tokens_guarded(&guard.capability, prepared.focus);
        self.commit_binding_generation(guard, prepared.binding, prepared.next_binding_generation);
    }

    pub fn pop_nested_frame(
        &self,
        interaction_domains: &mut InteractionDomains,
        owner_id: OverlayOwnerId,
        router_eligibility: Option<RouterEligibilityTestimony>,
    ) -> Result<(), OverlayDomainError> {
        let domain_id = self.owner_domain(owner_id)?;
        let guard = self.begin_domain_transition(domain_id)?;
        let prepared = self.prepare_nested_frame_pop(
            &guard,
            interaction_domains,
            owner_id,
            router_eligibility,
        )?;
        self.commit_nested_frame_pop(&guard, prepared);
        Ok(())
    }

    pub fn prepare_deactivation<'a>(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &'a mut InteractionDomains,
        owner_id: OverlayOwnerId,
        disposition: OverlayDisposition,
    ) -> Result<PreparedOverlayDeactivation<'a>, OverlayDomainError> {
        let (owner, binding) = {
            let state = self.validated_state(guard)?;
            let owner = state
                .owners
                .get(&owner_id)
                .cloned()
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if owner.port.interaction_domain_id != guard.domain_id {
                return Err(OverlayDomainError::OwnerDomainMismatch {
                    owner_id,
                    expected: owner.port.interaction_domain_id,
                    actual: guard.domain_id,
                });
            }
            let binding = state
                .domains
                .get(&guard.domain_id)
                .and_then(|domain| domain.active_binding)
                .ok_or(OverlayDomainError::OwnerInactive(owner_id))?;
            if binding.owner_id != owner_id {
                return Err(OverlayDomainError::ActiveBindingMismatch(guard.domain_id));
            }
            (owner, binding)
        };
        let owner_prepared = owner
            .port
            .begin_deactivation(guard.coordinator_identity, binding.stack_id)?;
        let manager = &mut interaction_domains
            .state_mut(guard.domain_id)
            .ok_or(OverlayDomainError::UnknownInteractionDomain(
                guard.domain_id,
            ))?
            .focus_manager;
        let focus_prepared = FocusResolver::new().prepare_restoration_token_refs(
            &guard.capability,
            manager,
            owner_prepared.tokens_top_first(),
        )?;
        Ok(PreparedOverlayDeactivation {
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            binding,
            disposition,
            owner: owner_prepared,
            focus: focus_prepared,
        })
    }

    pub fn commit_deactivation(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: PreparedOverlayDeactivation<'_>,
    ) {
        self.assert_prepared_transition(
            guard,
            prepared.coordinator_identity,
            prepared.domain_id,
            prepared.guard_generation,
        );
        let PreparedOverlayDeactivation {
            binding,
            disposition,
            owner,
            focus,
            ..
        } = prepared;
        owner.commit(disposition);
        FocusResolver::new().submit_restoration_tokens_guarded(&guard.capability, focus);
        let mut state = self.inner.borrow_mut();
        let domain = state
            .domains
            .get_mut(&guard.domain_id)
            .expect("prepared deactivation domain must remain registered");
        assert_eq!(
            domain.active_binding,
            Some(binding),
            "active overlay binding changed after deactivation preparation"
        );
        domain.active_binding = None;
    }

    pub fn deactivate_owner(
        &self,
        interaction_domains: &mut InteractionDomains,
        owner_id: OverlayOwnerId,
        disposition: OverlayDisposition,
    ) -> Result<(), OverlayDomainError> {
        let domain_id = self.owner_domain(owner_id)?;
        let guard = self.begin_domain_transition(domain_id)?;
        let prepared =
            self.prepare_deactivation(&guard, interaction_domains, owner_id, disposition)?;
        self.commit_deactivation(&guard, prepared);
        Ok(())
    }

    pub fn prepare_deactivation_to_domain<'a>(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &'a mut InteractionDomains,
        owner_id: OverlayOwnerId,
        disposition: OverlayDisposition,
        next_focus_domain: FocusDomainId,
    ) -> Result<PreparedOverlayDeactivationToDomain<'a>, OverlayDomainError> {
        let (owner, binding) = {
            let state = self.validated_state(guard)?;
            let owner = state
                .owners
                .get(&owner_id)
                .cloned()
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if owner.port.interaction_domain_id != guard.domain_id {
                return Err(OverlayDomainError::OwnerDomainMismatch {
                    owner_id,
                    expected: owner.port.interaction_domain_id,
                    actual: guard.domain_id,
                });
            }
            let binding = state
                .domains
                .get(&guard.domain_id)
                .and_then(|domain| domain.active_binding)
                .ok_or(OverlayDomainError::OwnerInactive(owner_id))?;
            if binding.owner_id != owner_id {
                return Err(OverlayDomainError::ActiveBindingMismatch(guard.domain_id));
            }
            (owner, binding)
        };
        let owner_prepared = owner
            .port
            .begin_deactivation(guard.coordinator_identity, binding.stack_id)?;
        let manager = &mut interaction_domains
            .state_mut(guard.domain_id)
            .ok_or(OverlayDomainError::UnknownInteractionDomain(
                guard.domain_id,
            ))?
            .focus_manager;
        let focus_prepared = FocusResolver::new().prepare_overlay_focus_exit_to_domain_refs(
            &guard.capability,
            manager,
            owner_prepared.tokens_top_first(),
            next_focus_domain,
        )?;
        Ok(PreparedOverlayDeactivationToDomain {
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            binding,
            disposition,
            owner: owner_prepared,
            focus: focus_prepared,
        })
    }

    pub fn commit_deactivation_to_domain(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: PreparedOverlayDeactivationToDomain<'_>,
    ) {
        self.assert_prepared_transition(
            guard,
            prepared.coordinator_identity,
            prepared.domain_id,
            prepared.guard_generation,
        );
        let PreparedOverlayDeactivationToDomain {
            binding,
            disposition,
            owner,
            focus,
            ..
        } = prepared;
        owner.commit(disposition);
        FocusResolver::new().commit_overlay_focus_exit_to_domain(&guard.capability, focus);
        let mut state = self.inner.borrow_mut();
        let domain = state
            .domains
            .get_mut(&guard.domain_id)
            .expect("compound overlay exit domain must remain registered");
        assert_eq!(
            domain.active_binding,
            Some(binding),
            "active overlay binding changed after compound exit preparation"
        );
        domain.active_binding = None;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_deactivate_for_focus_graph_mutation_guarded<'a>(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &'a mut InteractionDomains,
        owner_id: OverlayOwnerId,
        disposition: OverlayDisposition,
        parent_before_child_additions: &[FocusDomainAddition],
        complete_subtree_removals: &[FocusDomainId],
        optional_replacement_active_domain: Option<FocusDomainId>,
    ) -> Result<PreparedOverlayFocusGraphTransition<'a>, OverlayDomainError> {
        self.prepare_deactivate_for_focus_graph_mutation_inner(
            guard,
            interaction_domains,
            owner_id,
            disposition,
            parent_before_child_additions,
            complete_subtree_removals,
            optional_replacement_active_domain,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_deactivate_for_focus_graph_mutation_inner<'a>(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &'a mut InteractionDomains,
        owner_id: OverlayOwnerId,
        disposition: OverlayDisposition,
        parent_before_child_additions: &[FocusDomainAddition],
        complete_subtree_removals: &[FocusDomainId],
        optional_replacement_active_domain: Option<FocusDomainId>,
        exact_final_active_domain: Option<Option<FocusDomainId>>,
    ) -> Result<PreparedOverlayFocusGraphTransition<'a>, OverlayDomainError> {
        let (owner, binding) = {
            let state = self.validated_state(guard)?;
            let owner = state
                .owners
                .get(&owner_id)
                .cloned()
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if owner.port.interaction_domain_id != guard.domain_id {
                return Err(OverlayDomainError::OwnerDomainMismatch {
                    owner_id,
                    expected: owner.port.interaction_domain_id,
                    actual: guard.domain_id,
                });
            }
            let binding = state
                .domains
                .get(&guard.domain_id)
                .and_then(|domain| domain.active_binding)
                .ok_or(OverlayDomainError::OwnerInactive(owner_id))?;
            if binding.owner_id != owner_id {
                return Err(OverlayDomainError::ActiveBindingMismatch(guard.domain_id));
            }
            (owner, binding)
        };
        let owner_prepared = owner
            .port
            .begin_deactivation(guard.coordinator_identity, binding.stack_id)?;
        let manager = &mut interaction_domains
            .state_mut(guard.domain_id)
            .ok_or(OverlayDomainError::UnknownInteractionDomain(
                guard.domain_id,
            ))?
            .focus_manager;
        let resolver = FocusResolver::new();
        let focus_graph = match exact_final_active_domain {
            Some(final_active_domain) => resolver.prepare_focus_domain_graph_transition_refs(
                &guard.capability,
                manager,
                parent_before_child_additions,
                complete_subtree_removals,
                owner_prepared.tokens_top_first(),
                final_active_domain,
            )?,
            None => resolver.prepare_focus_domain_graph_mutation_refs(
                &guard.capability,
                manager,
                parent_before_child_additions,
                complete_subtree_removals,
                owner_prepared.tokens_top_first(),
                optional_replacement_active_domain,
            )?,
        };
        Ok(PreparedOverlayFocusGraphTransition {
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            binding,
            disposition,
            owner: owner_prepared,
            focus_graph,
        })
    }

    pub fn prepare_deactivate_for_focus_retirement_guarded<'a>(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &'a mut InteractionDomains,
        owner_id: OverlayOwnerId,
        disposition: OverlayDisposition,
        retiring_focus_domains: &[FocusDomainId],
        optional_replacement_active_domain: Option<FocusDomainId>,
    ) -> Result<PreparedOverlayFocusGraphTransition<'a>, OverlayDomainError> {
        if disposition != OverlayDisposition::Dismiss {
            return Err(OverlayDomainError::RetirementRequiresDismiss(owner_id));
        }
        self.prepare_deactivate_for_focus_graph_mutation_guarded(
            guard,
            interaction_domains,
            owner_id,
            disposition,
            &[],
            retiring_focus_domains,
            optional_replacement_active_domain,
        )
    }

    pub(crate) fn prepare_deactivate_for_focus_retirement_transition_guarded<'a>(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &'a mut InteractionDomains,
        owner_id: OverlayOwnerId,
        disposition: OverlayDisposition,
        retiring_focus_domains: &[FocusDomainId],
        exact_final_active_domain: Option<FocusDomainId>,
    ) -> Result<PreparedOverlayFocusGraphTransition<'a>, OverlayDomainError> {
        if disposition != OverlayDisposition::Dismiss {
            return Err(OverlayDomainError::RetirementRequiresDismiss(owner_id));
        }
        self.prepare_deactivate_for_focus_graph_mutation_inner(
            guard,
            interaction_domains,
            owner_id,
            disposition,
            &[],
            retiring_focus_domains,
            None,
            Some(exact_final_active_domain),
        )
    }

    pub(crate) fn prepare_complete_domain_retirement_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &InteractionDomains,
        owner_id: OverlayOwnerId,
        retiring_focus_domains: &[FocusDomainId],
    ) -> Result<PreparedOverlayCompleteDomainRetirement, OverlayDomainError> {
        let (owner, binding) = {
            let state = self.validated_state(guard)?;
            let owner = state
                .owners
                .get(&owner_id)
                .cloned()
                .ok_or(OverlayDomainError::UnknownOwner(owner_id))?;
            if owner.port.interaction_domain_id != guard.domain_id {
                return Err(OverlayDomainError::OwnerDomainMismatch {
                    owner_id,
                    expected: owner.port.interaction_domain_id,
                    actual: guard.domain_id,
                });
            }
            let binding = state
                .domains
                .get(&guard.domain_id)
                .and_then(|domain| domain.active_binding)
                .ok_or(OverlayDomainError::OwnerInactive(owner_id))?;
            if binding.owner_id != owner_id {
                return Err(OverlayDomainError::ActiveBindingMismatch(guard.domain_id));
            }
            (owner, binding)
        };
        let owner_prepared = owner
            .port
            .begin_deactivation(guard.coordinator_identity, binding.stack_id)?;
        let manager = &interaction_domains
            .state(guard.domain_id)
            .ok_or(OverlayDomainError::UnknownInteractionDomain(
                guard.domain_id,
            ))?
            .focus_manager;
        let focus = FocusResolver::new().prepare_complete_focus_domain_retirement(
            &guard.capability,
            manager,
            retiring_focus_domains,
            owner_prepared.tokens_top_first(),
        )?;
        Ok(PreparedOverlayCompleteDomainRetirement {
            coordinator_identity: guard.coordinator_identity,
            domain_id: guard.domain_id,
            guard_generation: guard.generation,
            binding,
            owner: owner_prepared,
            focus,
        })
    }

    pub(crate) fn commit_complete_domain_retirement_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        interaction_domains: &mut InteractionDomains,
        prepared: PreparedOverlayCompleteDomainRetirement,
    ) {
        self.assert_prepared_transition(
            guard,
            prepared.coordinator_identity,
            prepared.domain_id,
            prepared.guard_generation,
        );
        let PreparedOverlayCompleteDomainRetirement {
            binding,
            owner,
            focus,
            ..
        } = prepared;
        owner.commit(OverlayDisposition::Dismiss);
        let manager = &mut interaction_domains
            .state_mut(guard.domain_id)
            .expect("prepared complete focus retirement domain must remain registered")
            .focus_manager;
        FocusResolver::new().commit_complete_focus_domain_retirement(
            &guard.capability,
            manager,
            focus,
        );
        let mut state = self.inner.borrow_mut();
        let domain = state
            .domains
            .get_mut(&guard.domain_id)
            .expect("prepared complete overlay retirement domain must remain registered");
        assert_eq!(
            domain.active_binding,
            Some(binding),
            "active overlay binding changed after complete domain retirement preparation"
        );
        domain.active_binding = None;
    }

    pub fn commit_deactivate_for_focus_graph_mutation_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: PreparedOverlayFocusGraphTransition<'_>,
    ) {
        self.assert_prepared_transition(
            guard,
            prepared.coordinator_identity,
            prepared.domain_id,
            prepared.guard_generation,
        );
        let PreparedOverlayFocusGraphTransition {
            binding,
            disposition,
            owner,
            focus_graph,
            ..
        } = prepared;
        owner.commit(disposition);
        FocusResolver::new().commit_focus_domain_graph_mutation(&guard.capability, focus_graph);
        let mut state = self.inner.borrow_mut();
        let domain = state
            .domains
            .get_mut(&guard.domain_id)
            .expect("compound overlay graph domain must remain registered");
        assert_eq!(
            domain.active_binding,
            Some(binding),
            "active overlay binding changed after compound graph preparation"
        );
        domain.active_binding = None;
    }

    pub fn commit_deactivate_for_focus_retirement_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
        prepared: PreparedOverlayFocusGraphTransition<'_>,
    ) {
        self.commit_deactivate_for_focus_graph_mutation_guarded(guard, prepared);
    }

    pub fn deactivate_owner_to_domain(
        &self,
        interaction_domains: &mut InteractionDomains,
        owner_id: OverlayOwnerId,
        disposition: OverlayDisposition,
        next_focus_domain: FocusDomainId,
    ) -> Result<(), OverlayDomainError> {
        let domain_id = self.owner_domain(owner_id)?;
        let guard = self.begin_domain_transition(domain_id)?;
        let prepared = self.prepare_deactivation_to_domain(
            &guard,
            interaction_domains,
            owner_id,
            disposition,
            next_focus_domain,
        )?;
        self.commit_deactivation_to_domain(&guard, prepared);
        Ok(())
    }

    pub fn active_binding_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<Option<ActiveOverlayStackBinding>, OverlayDomainError> {
        let state = self.validated_state(guard)?;
        Ok(state
            .domains
            .get(&guard.domain_id)
            .expect("validated transition guard domain must remain registered")
            .active_binding)
    }

    pub(crate) fn domain_owner_ids_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<Vec<OverlayOwnerId>, OverlayDomainError> {
        let state = self.validated_state(guard)?;
        let mut owner_ids = state
            .owners
            .iter()
            .filter_map(|(owner_id, owner)| {
                (owner.port.interaction_domain_id == guard.domain_id).then_some(*owner_id)
            })
            .collect::<Vec<_>>();
        owner_ids.sort_by_key(|owner_id| owner_id.0);
        Ok(owner_ids)
    }

    pub fn active_projection_guarded(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<Option<OverlayProjection>, OverlayDomainError> {
        let state = self.validated_state(guard)?;
        let binding = match state
            .domains
            .get(&guard.domain_id)
            .expect("validated transition guard domain must remain registered")
            .active_binding
        {
            Some(binding) => binding,
            None => return Ok(None),
        };
        let owner = state
            .owners
            .get(&binding.owner_id)
            .expect("published overlay binding owner must remain registered");
        Ok(Some(owner.port.projection_for(binding.stack_id)))
    }

    pub fn owner_registration_generation(
        &self,
        owner_id: OverlayOwnerId,
    ) -> Result<u64, OverlayDomainError> {
        self.inner
            .borrow()
            .owners
            .get(&owner_id)
            .map(|owner| owner.registration_generation)
            .ok_or(OverlayDomainError::UnknownOwner(owner_id))
    }

    pub(crate) fn validate_transition_guard(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<(), OverlayDomainError> {
        self.validated_state(guard).map(|_| ())
    }

    fn owner_domain(
        &self,
        owner_id: OverlayOwnerId,
    ) -> Result<InteractionDomainId, OverlayDomainError> {
        self.inner
            .borrow()
            .owners
            .get(&owner_id)
            .map(|owner| owner.port.interaction_domain_id)
            .ok_or(OverlayDomainError::UnknownOwner(owner_id))
    }

    fn validated_state<'a>(
        &'a self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<std::cell::Ref<'a, CoordinatorState>, OverlayDomainError> {
        let state = self.inner.borrow();
        if state.identity != guard.coordinator_identity {
            return Err(OverlayDomainError::TransitionGuardMismatch);
        }
        let domain = state
            .domains
            .get(&guard.domain_id)
            .ok_or(OverlayDomainError::TransitionGuardMismatch)?;
        if !Rc::ptr_eq(&domain.guard, &guard.guard)
            || domain.guard.borrow().held_generation != Some(guard.generation)
        {
            return Err(OverlayDomainError::TransitionGuardMismatch);
        }
        Ok(state)
    }

    fn validate_router_eligibility(
        &self,
        owner: &RegisteredOwner,
        coordinator_identity: u64,
        expected_domains: HashSet<FocusDomainId>,
        testimony: Option<&RouterEligibilityTestimony>,
    ) -> Result<(), OverlayDomainError> {
        if owner.admission == OverlayActivationAdmission::DomainLocal {
            return Ok(());
        }
        let testimony = testimony.ok_or(OverlayDomainError::MissingRouterEligibility(
            owner.port.owner_id,
        ))?;
        if testimony.coordinator_identity != coordinator_identity
            || testimony.interaction_domain_id != owner.port.interaction_domain_id
            || testimony.owner_id != owner.port.owner_id
            || testimony.owner_registration_generation != owner.registration_generation
            || !expected_domains.is_subset(&testimony.focus_domains)
        {
            return Err(OverlayDomainError::RouterEligibilityMismatch(
                owner.port.owner_id,
            ));
        }
        Ok(())
    }

    fn commit_binding_generation(
        &self,
        guard: &DomainTransitionGuardToken,
        previous: ActiveOverlayStackBinding,
        next_generation: u64,
    ) {
        let mut state = self.inner.borrow_mut();
        let domain = state
            .domains
            .get_mut(&guard.domain_id)
            .expect("prepared nested transition domain must remain registered");
        assert_eq!(
            domain.active_binding,
            Some(previous),
            "active overlay binding changed after nested transition preparation"
        );
        assert_eq!(
            domain.next_binding_generation,
            next_generation
                .checked_add(1)
                .expect("prepared overlay binding generation must have a successor"),
            "overlay binding generation changed after nested transition preparation"
        );
        domain.active_binding = Some(ActiveOverlayStackBinding {
            generation: next_generation,
            ..previous
        });
    }

    fn reserve_binding_generation(
        &self,
        guard: &DomainTransitionGuardToken,
    ) -> Result<u64, OverlayDomainError> {
        self.validated_state(guard)?;
        let mut state = self.inner.borrow_mut();
        let domain = state
            .domains
            .get_mut(&guard.domain_id)
            .expect("validated transition guard domain must remain registered");
        let generation = domain.next_binding_generation;
        domain.next_binding_generation = generation
            .checked_add(1)
            .ok_or(OverlayDomainError::Exhausted)?;
        Ok(generation)
    }

    fn assert_prepared_coordinator(&self, prepared: &Self) {
        assert!(
            Rc::ptr_eq(&self.inner, &prepared.inner),
            "prepared overlay value belongs to another coordinator"
        );
    }

    fn assert_prepared_transition(
        &self,
        guard: &DomainTransitionGuardToken,
        coordinator_identity: u64,
        domain_id: InteractionDomainId,
        guard_generation: u64,
    ) {
        assert_eq!(
            guard.coordinator_identity, coordinator_identity,
            "prepared overlay transition coordinator changed"
        );
        assert_eq!(
            guard.domain_id, domain_id,
            "prepared overlay transition domain changed"
        );
        assert_eq!(
            guard.generation, guard_generation,
            "prepared overlay transition guard changed"
        );
        self.validated_state(guard)
            .expect("prepared overlay transition guard is no longer live");
    }
}

impl Default for OverlayDomainCoordinator {
    fn default() -> Self {
        Self::new().expect("overlay coordinator identity space exhausted")
    }
}

#[derive(Debug)]
pub struct DomainTransitionGuardToken {
    coordinator_identity: u64,
    domain_id: InteractionDomainId,
    generation: u64,
    guard: Rc<RefCell<DomainGuardState>>,
    capability: InteractionDomainTransitionCapability,
}

impl DomainTransitionGuardToken {
    pub const fn interaction_domain_id(&self) -> InteractionDomainId {
        self.domain_id
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) const fn coordinator_identity(&self) -> u64 {
        self.coordinator_identity
    }

    pub(crate) fn focus_capability(&self) -> &InteractionDomainTransitionCapability {
        &self.capability
    }
}

impl Drop for DomainTransitionGuardToken {
    fn drop(&mut self) {
        let mut guard = self.guard.borrow_mut();
        if guard.held_generation == Some(self.generation) {
            guard.held_generation = None;
        }
    }
}

#[derive(Debug)]
pub struct RouterEligibilityTestimony {
    coordinator_identity: u64,
    interaction_domain_id: InteractionDomainId,
    owner_id: OverlayOwnerId,
    owner_registration_generation: u64,
    router_generation: u64,
    root_binding_generation: u64,
    focus_domains: HashSet<FocusDomainId>,
    _lease: Option<RouterGenerationLease>,
}

#[derive(Debug)]
struct RouterGenerationLease {
    live: Rc<Cell<bool>>,
}

impl Drop for RouterGenerationLease {
    fn drop(&mut self) {
        self.live.set(false);
    }
}

impl RouterEligibilityTestimony {
    #[cfg(test)]
    pub(crate) fn issue(
        coordinator: &OverlayDomainCoordinator,
        interaction_domain_id: InteractionDomainId,
        owner_id: OverlayOwnerId,
        owner_registration_generation: u64,
        router_generation: u64,
        root_binding_generation: u64,
        focus_domains: impl IntoIterator<Item = FocusDomainId>,
    ) -> Self {
        Self {
            coordinator_identity: coordinator.inner.borrow().identity,
            interaction_domain_id,
            owner_id,
            owner_registration_generation,
            router_generation,
            root_binding_generation,
            focus_domains: focus_domains.into_iter().collect(),
            _lease: None,
        }
    }

    pub(crate) fn issue_leased(
        coordinator: &OverlayDomainCoordinator,
        interaction_domain_id: InteractionDomainId,
        owner_id: OverlayOwnerId,
        owner_registration_generation: u64,
        router_generation: u64,
        root_binding_generation: u64,
        focus_domains: impl IntoIterator<Item = FocusDomainId>,
        live: Rc<Cell<bool>>,
    ) -> Self {
        Self {
            coordinator_identity: coordinator.inner.borrow().identity,
            interaction_domain_id,
            owner_id,
            owner_registration_generation,
            router_generation,
            root_binding_generation,
            focus_domains: focus_domains.into_iter().collect(),
            _lease: Some(RouterGenerationLease { live }),
        }
    }

    pub const fn router_generation(&self) -> u64 {
        self.router_generation
    }

    pub const fn root_binding_generation(&self) -> u64 {
        self.root_binding_generation
    }
}

#[derive(Debug)]
pub struct PreparedOverlayDomainRegistration {
    coordinator: OverlayDomainCoordinator,
    domain_id: InteractionDomainId,
    committed: bool,
}

impl Drop for PreparedOverlayDomainRegistration {
    fn drop(&mut self) {
        if !self.committed {
            self.coordinator
                .inner
                .borrow_mut()
                .reserved_domains
                .remove(&self.domain_id);
        }
    }
}

#[derive(Debug)]
pub struct PreparedOverlayOwnerRegistration {
    coordinator: OverlayDomainCoordinator,
    port: OverlayOwnerLifecyclePort,
    admission: OverlayActivationAdmission,
    registration_generation: u64,
    committed: bool,
}

impl PreparedOverlayOwnerRegistration {
    pub const fn owner_id(&self) -> OverlayOwnerId {
        self.port.owner_id
    }

    pub const fn interaction_domain_id(&self) -> InteractionDomainId {
        self.port.interaction_domain_id
    }
}

impl Drop for PreparedOverlayOwnerRegistration {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let identity = self.coordinator.inner.borrow().identity;
        self.coordinator
            .inner
            .borrow_mut()
            .reserved_owners
            .remove(&self.port.owner_id);
        self.port.abort_registration(identity);
    }
}

#[derive(Debug)]
pub struct PreparedOverlayOwnerUnregistration {
    coordinator: OverlayDomainCoordinator,
    coordinator_identity: u64,
    domain_id: InteractionDomainId,
    guard_generation: u64,
    owner_id: OverlayOwnerId,
    port: OverlayOwnerLifecyclePort,
    generation: u64,
    committed: bool,
}

impl Drop for PreparedOverlayOwnerUnregistration {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        self.coordinator
            .inner
            .borrow_mut()
            .retiring_owners
            .remove(&self.owner_id);
        self.port.abort_unregistration(self.generation);
    }
}

#[derive(Debug)]
pub struct PreparedOverlayDomainUnregistration {
    coordinator: OverlayDomainCoordinator,
    coordinator_identity: u64,
    domain_id: InteractionDomainId,
    guard_generation: u64,
    committed: bool,
}

impl Drop for PreparedOverlayDomainUnregistration {
    fn drop(&mut self) {
        if !self.committed {
            self.coordinator
                .inner
                .borrow_mut()
                .retiring_domains
                .remove(&self.domain_id);
        }
    }
}

#[derive(Debug)]
pub struct PreparedOverlayActivation<'a> {
    coordinator_identity: u64,
    domain_id: InteractionDomainId,
    guard_generation: u64,
    binding_generation: u64,
    owner: PreparedOwnerActivation,
    focus: PreparedOverlayFocusActivation<'a>,
}

#[derive(Debug)]
pub struct PreparedOverlayDeactivation<'a> {
    coordinator_identity: u64,
    domain_id: InteractionDomainId,
    guard_generation: u64,
    binding: ActiveOverlayStackBinding,
    disposition: OverlayDisposition,
    owner: PreparedOwnerDeactivation,
    focus: PreparedFocusRestoration<'a>,
}

#[derive(Debug)]
pub struct PreparedOverlayDeactivationToDomain<'a> {
    coordinator_identity: u64,
    domain_id: InteractionDomainId,
    guard_generation: u64,
    binding: ActiveOverlayStackBinding,
    disposition: OverlayDisposition,
    owner: PreparedOwnerDeactivation,
    focus: PreparedOverlayFocusExit<'a>,
}

impl PreparedOverlayDeactivationToDomain<'_> {
    pub(crate) fn surface_exit_testimony(&self) -> PreparedOverlaySurfaceExitTestimony<'_> {
        PreparedOverlaySurfaceExitTestimony {
            owner_id: self.binding.owner_id,
            stack_id: self.binding.stack_id,
            disposition: self.disposition,
            surfaces_top_first: self.owner.surfaces_top_first(),
        }
    }
}

#[derive(Debug)]
pub struct PreparedOverlayFocusGraphTransition<'a> {
    coordinator_identity: u64,
    domain_id: InteractionDomainId,
    guard_generation: u64,
    binding: ActiveOverlayStackBinding,
    disposition: OverlayDisposition,
    owner: PreparedOwnerDeactivation,
    focus_graph: PreparedFocusDomainGraphMutation<'a>,
}

#[derive(Debug)]
pub(crate) struct PreparedOverlayCompleteDomainRetirement {
    coordinator_identity: u64,
    domain_id: InteractionDomainId,
    guard_generation: u64,
    binding: ActiveOverlayStackBinding,
    owner: PreparedOwnerDeactivation,
    focus: PreparedCompleteFocusDomainRetirement,
}

impl PreparedOverlayCompleteDomainRetirement {
    pub(crate) fn owner_id(&self) -> OverlayOwnerId {
        self.binding.owner_id
    }

    pub(crate) fn focus_graph_testimony(&self) -> FocusDomainGraphMutationTestimony<'_> {
        self.focus.testimony()
    }

    pub(crate) fn surface_exit_testimony(&self) -> PreparedOverlaySurfaceExitTestimony<'_> {
        PreparedOverlaySurfaceExitTestimony {
            owner_id: self.binding.owner_id,
            stack_id: self.binding.stack_id,
            disposition: OverlayDisposition::Dismiss,
            surfaces_top_first: self.owner.surfaces_top_first(),
        }
    }
}

impl PreparedOverlayFocusGraphTransition<'_> {
    pub fn focus_graph_testimony(&self) -> FocusDomainGraphMutationTestimony<'_> {
        self.focus_graph.testimony()
    }

    pub(crate) fn surface_exit_testimony(&self) -> PreparedOverlaySurfaceExitTestimony<'_> {
        PreparedOverlaySurfaceExitTestimony {
            owner_id: self.binding.owner_id,
            stack_id: self.binding.stack_id,
            disposition: self.disposition,
            surfaces_top_first: self.owner.surfaces_top_first(),
        }
    }
}

#[derive(Debug)]
pub struct PreparedOverlayNestedFramePush<'a> {
    coordinator_identity: u64,
    domain_id: InteractionDomainId,
    guard_generation: u64,
    binding: ActiveOverlayStackBinding,
    next_binding_generation: u64,
    owner: PreparedOwnerNestedPush,
    focus: PreparedOverlayFocusActivation<'a>,
}

#[derive(Debug)]
pub struct PreparedOverlayNestedFramePop<'a> {
    coordinator_identity: u64,
    domain_id: InteractionDomainId,
    guard_generation: u64,
    binding: ActiveOverlayStackBinding,
    next_binding_generation: u64,
    owner: PreparedOwnerNestedPop,
    focus: PreparedFocusRestoration<'a>,
}
