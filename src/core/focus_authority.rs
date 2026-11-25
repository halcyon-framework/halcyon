use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::compositor::SurfaceId;
use crate::core::event::{FocusManager, InteractionDomainId};
use crate::utils::id::TreeWidgetId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FocusDomainId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusDomainAddition {
    pub domain_id: FocusDomainId,
    pub parent: Option<FocusDomainId>,
}

impl FocusDomainAddition {
    pub const fn new(domain_id: FocusDomainId, parent: Option<FocusDomainId>) -> Self {
        Self { domain_id, parent }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusTargetRegistration {
    pub domain_id: FocusDomainId,
    pub target: TreeWidgetId,
    pub surface: Option<SurfaceId>,
    pub eligible: bool,
}

impl FocusTargetRegistration {
    pub const fn new(
        domain_id: FocusDomainId,
        target: TreeWidgetId,
        surface: Option<SurfaceId>,
        eligible: bool,
    ) -> Self {
        Self {
            domain_id,
            target,
            surface,
            eligible,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusProjection {
    pub focused_surface: Option<SurfaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusAuthorityError {
    CapabilityMismatch,
    UnknownFocusDomain(FocusDomainId),
    DuplicateFocusDomain(FocusDomainId),
    ParentUnavailable(FocusDomainId),
    ParentRemoved(FocusDomainId),
    IncompleteSubtreeRemoval(FocusDomainId),
    ReplacementInsideRemovedSubtree(FocusDomainId),
    UnexpectedReplacement(FocusDomainId),
    DuplicateFocusTarget(TreeWidgetId),
    UnknownFocusTarget(TreeWidgetId),
    FocusTargetOutsideActiveDomain(TreeWidgetId),
    RestorationTokenMismatch,
    RestorationFrameBlocked,
    PeerRestorationDependency {
        activated_domain: FocusDomainId,
        superseded_domain: FocusDomainId,
    },
    InvalidOverlayDomainOrder(FocusDomainId),
    InconsistentFocusState,
    Exhausted,
}

#[derive(Debug, Clone)]
pub(crate) struct FocusTargetRecord {
    pub(crate) surface_id: Option<SurfaceId>,
    pub(crate) eligible: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct FocusDomainRecord {
    pub(crate) interaction_domain_id: InteractionDomainId,
    pub(crate) parent: Option<FocusDomainId>,
    pub(crate) targets: HashMap<TreeWidgetId, FocusTargetRecord>,
}

#[derive(Debug, Clone)]
pub(crate) struct FocusRestorationFrame {
    generation: u64,
    activated_domain: FocusDomainId,
    superseded_domain: Option<FocusDomainId>,
    superseded_target: Option<TreeWidgetId>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FocusRestorationToken {
    interaction_domain_id: InteractionDomainId,
    generation: u64,
    activated_domain: FocusDomainId,
    superseded_domain: Option<FocusDomainId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InteractionDomainTransitionBinding {
    interaction_domain_id: InteractionDomainId,
    guard_generation: u64,
    capability_identity: u64,
}

/// Non-cloneable call-scoped proof issued by the interaction-domain coordinator.
#[derive(Debug)]
pub struct InteractionDomainTransitionCapability {
    binding: InteractionDomainTransitionBinding,
}

static NEXT_INTERACTION_DOMAIN_CAPABILITY_IDENTITY: AtomicU64 = AtomicU64::new(1);

impl InteractionDomainTransitionCapability {
    pub(crate) fn issue(
        interaction_domain_id: InteractionDomainId,
        guard_generation: u64,
    ) -> Result<Self, FocusAuthorityError> {
        let capability_identity = NEXT_INTERACTION_DOMAIN_CAPABILITY_IDENTITY
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| FocusAuthorityError::Exhausted)?;
        Ok(Self {
            binding: InteractionDomainTransitionBinding {
                interaction_domain_id,
                guard_generation,
                capability_identity,
            },
        })
    }

    pub fn interaction_domain_id(&self) -> InteractionDomainId {
        self.binding.interaction_domain_id
    }

    pub fn guard_generation(&self) -> u64 {
        self.binding.guard_generation
    }
}

#[derive(Debug)]
struct PreparedFocusState<'a> {
    manager: &'a mut FocusManager,
    capability_binding: InteractionDomainTransitionBinding,
    focus_domains: HashMap<FocusDomainId, FocusDomainRecord>,
    active_focus_domain: Option<FocusDomainId>,
    focused: Option<TreeWidgetId>,
    restoration_stack: Vec<FocusRestorationFrame>,
    next_restoration_generation: u64,
}

impl PreparedFocusState<'_> {
    fn commit(self, capability: &InteractionDomainTransitionCapability) {
        assert_eq!(
            self.capability_binding, capability.binding,
            "focus commit capability changed after preparation"
        );
        self.manager.focus_domains = self.focus_domains;
        self.manager
            .commit_focus_domain(self.active_focus_domain, self.focused);
        self.manager.restoration_stack = self.restoration_stack;
        self.manager.next_restoration_generation = self.next_restoration_generation;
    }
}

#[derive(Debug)]
pub struct PreparedFocusDomainActivation<'a> {
    state: PreparedFocusState<'a>,
}

#[derive(Debug)]
pub struct PreparedOverlayFocusActivation<'a> {
    state: PreparedFocusState<'a>,
    tokens: Vec<FocusRestorationToken>,
}

#[derive(Debug)]
pub struct PreparedOverlayFocusExit<'a> {
    state: PreparedFocusState<'a>,
}

#[derive(Debug)]
pub struct PreparedFocusRestoration<'a> {
    state: PreparedFocusState<'a>,
}

#[derive(Debug)]
pub struct PreparedFocusTargetMutation<'a> {
    state: PreparedFocusState<'a>,
}

#[derive(Debug)]
pub struct PreparedFocusTargetSelection<'a> {
    state: PreparedFocusState<'a>,
}

#[derive(Debug)]
pub struct PreparedFocusDomainGraphMutation<'a> {
    preparation_identity: u64,
    state: PreparedFocusState<'a>,
    before_parents: HashMap<FocusDomainId, Option<FocusDomainId>>,
    after_parents: HashMap<FocusDomainId, Option<FocusDomainId>>,
    additions: Vec<FocusDomainId>,
    removals: Vec<FocusDomainId>,
    before_active_focus_domain: Option<FocusDomainId>,
    after_active_focus_domain: Option<FocusDomainId>,
}

impl PreparedFocusDomainGraphMutation<'_> {
    pub fn testimony(&self) -> FocusDomainGraphMutationTestimony<'_> {
        FocusDomainGraphMutationTestimony {
            preparation_identity: self.preparation_identity,
            capability_binding: self.state.capability_binding,
            before_parents: &self.before_parents,
            after_parents: &self.after_parents,
            additions: &self.additions,
            removals: &self.removals,
            before_active_focus_domain: self.before_active_focus_domain,
            after_active_focus_domain: self.after_active_focus_domain,
        }
    }
}

#[derive(Debug)]
pub struct PreparedFocusDomainRetirement<'a> {
    graph: PreparedFocusDomainGraphMutation<'a>,
}

#[derive(Debug)]
pub(crate) struct PreparedCompleteFocusDomainRetirement {
    preparation_identity: u64,
    capability_binding: InteractionDomainTransitionBinding,
    before_parents: HashMap<FocusDomainId, Option<FocusDomainId>>,
    after_parents: HashMap<FocusDomainId, Option<FocusDomainId>>,
    removals: Vec<FocusDomainId>,
    before_active_focus_domain: Option<FocusDomainId>,
    before_focused: Option<TreeWidgetId>,
    before_restoration_frames: Vec<(
        u64,
        FocusDomainId,
        Option<FocusDomainId>,
        Option<TreeWidgetId>,
    )>,
    next_restoration_generation: u64,
}

impl PreparedCompleteFocusDomainRetirement {
    pub(crate) fn testimony(&self) -> FocusDomainGraphMutationTestimony<'_> {
        FocusDomainGraphMutationTestimony {
            preparation_identity: self.preparation_identity,
            capability_binding: self.capability_binding,
            before_parents: &self.before_parents,
            after_parents: &self.after_parents,
            additions: &[],
            removals: &self.removals,
            before_active_focus_domain: self.before_active_focus_domain,
            after_active_focus_domain: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FocusGraphActiveTransition {
    PreserveOrReplaceIfRemoved(Option<FocusDomainId>),
    Exact(Option<FocusDomainId>),
}

impl PreparedFocusDomainRetirement<'_> {
    pub fn testimony(&self) -> FocusDomainGraphMutationTestimony<'_> {
        self.graph.testimony()
    }
}

/// Borrowed ancestry/membership testimony sealed to one prepared graph mutation.
#[derive(Debug)]
pub struct FocusDomainGraphMutationTestimony<'a> {
    preparation_identity: u64,
    capability_binding: InteractionDomainTransitionBinding,
    before_parents: &'a HashMap<FocusDomainId, Option<FocusDomainId>>,
    after_parents: &'a HashMap<FocusDomainId, Option<FocusDomainId>>,
    additions: &'a [FocusDomainId],
    removals: &'a [FocusDomainId],
    before_active_focus_domain: Option<FocusDomainId>,
    after_active_focus_domain: Option<FocusDomainId>,
}

impl FocusDomainGraphMutationTestimony<'_> {
    pub(crate) fn preparation_identity(&self) -> u64 {
        self.preparation_identity
    }

    pub fn interaction_domain_id(&self) -> InteractionDomainId {
        self.capability_binding.interaction_domain_id
    }

    pub fn parent_before(&self, domain_id: FocusDomainId) -> Option<Option<FocusDomainId>> {
        self.before_parents.get(&domain_id).copied()
    }

    pub fn parent_after(&self, domain_id: FocusDomainId) -> Option<Option<FocusDomainId>> {
        self.after_parents.get(&domain_id).copied()
    }

    pub fn additions(&self) -> &[FocusDomainId] {
        self.additions
    }

    pub fn removals(&self) -> &[FocusDomainId] {
        self.removals
    }

    pub(crate) fn parents_after(&self) -> &HashMap<FocusDomainId, Option<FocusDomainId>> {
        self.after_parents
    }

    pub(crate) fn active_focus_domain_before(&self) -> Option<FocusDomainId> {
        self.before_active_focus_domain
    }

    pub(crate) fn active_focus_domain_after(&self) -> Option<FocusDomainId> {
        self.after_active_focus_domain
    }
}

#[derive(Debug, Default)]
pub struct FocusResolver;

impl FocusResolver {
    pub const fn new() -> Self {
        Self
    }

    pub fn focus_projection_guarded(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &FocusManager,
        interaction_domain_id: InteractionDomainId,
    ) -> Result<FocusProjection, FocusAuthorityError> {
        Self::validate_capability(capability, manager)?;
        if interaction_domain_id != manager.interaction_domain_id {
            return Err(FocusAuthorityError::CapabilityMismatch);
        }
        let focused_surface =
            manager
                .active_focus_domain
                .zip(manager.focused)
                .and_then(|(domain_id, target)| {
                    manager
                        .focus_domains
                        .get(&domain_id)
                        .and_then(|domain| domain.targets.get(&target))
                        .filter(|record| record.eligible)
                        .and_then(|record| record.surface_id)
                });
        Ok(FocusProjection { focused_surface })
    }

    pub fn prepare_focus_domain_activation<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        focus_domain_id: FocusDomainId,
    ) -> Result<PreparedFocusDomainActivation<'a>, FocusAuthorityError> {
        let binding = Self::validate_capability(capability, manager)?;
        if !manager.focus_domains.contains_key(&focus_domain_id) {
            return Err(FocusAuthorityError::UnknownFocusDomain(focus_domain_id));
        }
        let preferred = (manager.active_focus_domain == Some(focus_domain_id))
            .then_some(manager.focused)
            .flatten();
        let focused = Self::resolve_target(&manager.focus_domains, focus_domain_id, preferred)?;
        Ok(PreparedFocusDomainActivation {
            state: Self::prepared_state(
                manager,
                binding,
                None,
                Some(focus_domain_id),
                focused,
                None,
                None,
            ),
        })
    }

    pub fn commit_focus_domain_activation(
        &self,
        capability: &InteractionDomainTransitionCapability,
        prepared: PreparedFocusDomainActivation<'_>,
    ) {
        prepared.state.commit(capability);
    }

    pub fn prepare_overlay_focus_activation<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        focus_domains_bottom_first: &[FocusDomainId],
    ) -> Result<PreparedOverlayFocusActivation<'a>, FocusAuthorityError> {
        let binding = Self::validate_capability(capability, manager)?;
        Self::validate_overlay_activation_order(manager, focus_domains_bottom_first)?;
        let frame_count = u64::try_from(focus_domains_bottom_first.len())
            .map_err(|_| FocusAuthorityError::Exhausted)?;
        let next_generation = manager
            .next_restoration_generation
            .checked_add(frame_count)
            .ok_or(FocusAuthorityError::Exhausted)?;

        let mut restoration_stack = manager.restoration_stack.clone();
        let mut active_domain = manager.active_focus_domain;
        let mut focused = manager.focused;
        let mut tokens = Vec::with_capacity(focus_domains_bottom_first.len());
        for (offset, activated_domain) in focus_domains_bottom_first.iter().copied().enumerate() {
            let generation = manager.next_restoration_generation
                + u64::try_from(offset).map_err(|_| FocusAuthorityError::Exhausted)?;
            let frame = FocusRestorationFrame {
                generation,
                activated_domain,
                superseded_domain: active_domain,
                superseded_target: focused,
            };
            tokens.push(FocusRestorationToken {
                interaction_domain_id: manager.interaction_domain_id,
                generation,
                activated_domain,
                superseded_domain: active_domain,
            });
            restoration_stack.push(frame);
            active_domain = Some(activated_domain);
            focused = Self::resolve_target(&manager.focus_domains, activated_domain, None)?;
        }

        Ok(PreparedOverlayFocusActivation {
            state: Self::prepared_state(
                manager,
                binding,
                None,
                active_domain,
                focused,
                Some(restoration_stack),
                Some(next_generation),
            ),
            tokens,
        })
    }

    pub fn commit_overlay_focus_activation(
        &self,
        capability: &InteractionDomainTransitionCapability,
        prepared: PreparedOverlayFocusActivation<'_>,
    ) -> Vec<FocusRestorationToken> {
        let PreparedOverlayFocusActivation { state, tokens } = prepared;
        state.commit(capability);
        tokens
    }

    pub fn prepare_overlay_focus_exit_to_domain<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        top_first_restoration_tokens: Vec<FocusRestorationToken>,
        next_focus_domain: FocusDomainId,
    ) -> Result<PreparedOverlayFocusExit<'a>, FocusAuthorityError> {
        self.prepare_overlay_focus_exit_to_domain_refs(
            capability,
            manager,
            &top_first_restoration_tokens,
            next_focus_domain,
        )
    }

    pub(crate) fn prepare_overlay_focus_exit_to_domain_refs<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        top_first_restoration_tokens: &[FocusRestorationToken],
        next_focus_domain: FocusDomainId,
    ) -> Result<PreparedOverlayFocusExit<'a>, FocusAuthorityError> {
        let binding = Self::validate_capability(capability, manager)?;
        if !manager.focus_domains.contains_key(&next_focus_domain) {
            return Err(FocusAuthorityError::UnknownFocusDomain(next_focus_domain));
        }
        let matched = Self::validate_top_tokens(manager, top_first_restoration_tokens)?;
        for frame in &matched.frames_top_first {
            if next_focus_domain == frame.activated_domain
                || Self::is_descendant(
                    &manager.focus_domains,
                    next_focus_domain,
                    frame.activated_domain,
                )
            {
                return Err(FocusAuthorityError::InvalidOverlayDomainOrder(
                    next_focus_domain,
                ));
            }
        }
        let mut restoration_stack = manager.restoration_stack.clone();
        restoration_stack.truncate(matched.remaining_len);
        let focused = Self::resolve_target(&manager.focus_domains, next_focus_domain, None)?;
        Ok(PreparedOverlayFocusExit {
            state: Self::prepared_state(
                manager,
                binding,
                None,
                Some(next_focus_domain),
                focused,
                Some(restoration_stack),
                None,
            ),
        })
    }

    pub fn commit_overlay_focus_exit_to_domain(
        &self,
        capability: &InteractionDomainTransitionCapability,
        prepared: PreparedOverlayFocusExit<'_>,
    ) {
        prepared.state.commit(capability);
    }

    pub fn prepare_restoration_tokens<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        top_first_tokens: Vec<FocusRestorationToken>,
    ) -> Result<PreparedFocusRestoration<'a>, FocusAuthorityError> {
        self.prepare_restoration_token_refs(capability, manager, &top_first_tokens)
    }

    pub(crate) fn prepare_restoration_token_refs<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        top_first_tokens: &[FocusRestorationToken],
    ) -> Result<PreparedFocusRestoration<'a>, FocusAuthorityError> {
        let binding = Self::validate_capability(capability, manager)?;
        let matched = Self::validate_top_tokens(manager, top_first_tokens)?;
        let mut restoration_stack = manager.restoration_stack.clone();
        restoration_stack.truncate(matched.remaining_len);
        let (active_domain, focused) = match matched.frames_top_first.last() {
            Some(frame) => match frame.superseded_domain {
                Some(domain_id) => {
                    if !manager.focus_domains.contains_key(&domain_id) {
                        return Err(FocusAuthorityError::UnknownFocusDomain(domain_id));
                    }
                    let focused = Self::resolve_target(
                        &manager.focus_domains,
                        domain_id,
                        frame.superseded_target,
                    )?;
                    (Some(domain_id), focused)
                }
                None => (None, None),
            },
            None => (manager.active_focus_domain, manager.focused),
        };
        Ok(PreparedFocusRestoration {
            state: Self::prepared_state(
                manager,
                binding,
                None,
                active_domain,
                focused,
                Some(restoration_stack),
                None,
            ),
        })
    }

    pub fn submit_restoration_tokens_guarded(
        &self,
        capability: &InteractionDomainTransitionCapability,
        prepared: PreparedFocusRestoration<'_>,
    ) {
        prepared.state.commit(capability);
    }

    pub fn prepare_focus_target_mutation<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        additions: &[FocusTargetRegistration],
        removals: &[TreeWidgetId],
    ) -> Result<PreparedFocusTargetMutation<'a>, FocusAuthorityError> {
        let binding = Self::validate_capability(capability, manager)?;
        let mut focus_domains = manager.focus_domains.clone();
        let mut removed_targets = HashSet::new();
        for target in removals.iter().copied() {
            if !removed_targets.insert(target) {
                return Err(FocusAuthorityError::UnknownFocusTarget(target));
            }
            let Some((_, domain)) = focus_domains
                .iter_mut()
                .find(|(_, domain)| domain.targets.contains_key(&target))
            else {
                return Err(FocusAuthorityError::UnknownFocusTarget(target));
            };
            domain.targets.remove(&target);
        }

        let mut added_targets = HashSet::new();
        for addition in additions {
            if !added_targets.insert(addition.target)
                || focus_domains
                    .values()
                    .any(|domain| domain.targets.contains_key(&addition.target))
            {
                return Err(FocusAuthorityError::DuplicateFocusTarget(addition.target));
            }
            let domain = focus_domains
                .get_mut(&addition.domain_id)
                .ok_or(FocusAuthorityError::UnknownFocusDomain(addition.domain_id))?;
            domain.targets.insert(
                addition.target,
                FocusTargetRecord {
                    surface_id: addition.surface,
                    eligible: addition.eligible,
                },
            );
        }

        let active_focus_domain = manager.active_focus_domain;
        let focused = match active_focus_domain {
            Some(domain_id) => Self::resolve_target(&focus_domains, domain_id, manager.focused)?,
            None => None,
        };
        Ok(PreparedFocusTargetMutation {
            state: Self::prepared_state(
                manager,
                binding,
                Some(focus_domains),
                active_focus_domain,
                focused,
                None,
                None,
            ),
        })
    }

    pub fn commit_focus_target_mutation(
        &self,
        capability: &InteractionDomainTransitionCapability,
        prepared: PreparedFocusTargetMutation<'_>,
    ) {
        prepared.state.commit(capability);
    }

    pub fn prepare_focus_target_selection<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        target: TreeWidgetId,
    ) -> Result<PreparedFocusTargetSelection<'a>, FocusAuthorityError> {
        let binding = Self::validate_capability(capability, manager)?;
        let active_focus_domain = manager
            .active_focus_domain
            .ok_or(FocusAuthorityError::FocusTargetOutsideActiveDomain(target))?;
        let target_is_eligible = manager
            .focus_domains
            .get(&active_focus_domain)
            .and_then(|domain| domain.targets.get(&target))
            .is_some_and(|record| record.eligible);
        if !target_is_eligible {
            return Err(FocusAuthorityError::FocusTargetOutsideActiveDomain(target));
        }
        Ok(PreparedFocusTargetSelection {
            state: Self::prepared_state(
                manager,
                binding,
                None,
                Some(active_focus_domain),
                Some(target),
                None,
                None,
            ),
        })
    }

    pub fn commit_focus_target_selection(
        &self,
        capability: &InteractionDomainTransitionCapability,
        prepared: PreparedFocusTargetSelection<'_>,
    ) {
        prepared.state.commit(capability);
    }

    pub fn prepare_focus_domain_graph_mutation<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        parent_before_child_additions: &[FocusDomainAddition],
        complete_subtree_removals: &[FocusDomainId],
        top_first_overlay_restoration_tokens: Vec<FocusRestorationToken>,
        optional_replacement_active_domain: Option<FocusDomainId>,
    ) -> Result<PreparedFocusDomainGraphMutation<'a>, FocusAuthorityError> {
        self.prepare_focus_domain_graph_mutation_refs(
            capability,
            manager,
            parent_before_child_additions,
            complete_subtree_removals,
            &top_first_overlay_restoration_tokens,
            optional_replacement_active_domain,
        )
    }

    pub(crate) fn prepare_focus_domain_graph_mutation_refs<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        parent_before_child_additions: &[FocusDomainAddition],
        complete_subtree_removals: &[FocusDomainId],
        top_first_overlay_restoration_tokens: &[FocusRestorationToken],
        optional_replacement_active_domain: Option<FocusDomainId>,
    ) -> Result<PreparedFocusDomainGraphMutation<'a>, FocusAuthorityError> {
        let binding = Self::validate_capability(capability, manager)?;
        Self::prepare_graph_mutation(
            manager,
            binding,
            parent_before_child_additions,
            complete_subtree_removals,
            top_first_overlay_restoration_tokens,
            FocusGraphActiveTransition::PreserveOrReplaceIfRemoved(
                optional_replacement_active_domain,
            ),
        )
    }

    pub(crate) fn prepare_focus_domain_graph_transition_refs<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        parent_before_child_additions: &[FocusDomainAddition],
        complete_subtree_removals: &[FocusDomainId],
        top_first_overlay_restoration_tokens: &[FocusRestorationToken],
        exact_final_active_domain: Option<FocusDomainId>,
    ) -> Result<PreparedFocusDomainGraphMutation<'a>, FocusAuthorityError> {
        let binding = Self::validate_capability(capability, manager)?;
        Self::prepare_graph_mutation(
            manager,
            binding,
            parent_before_child_additions,
            complete_subtree_removals,
            top_first_overlay_restoration_tokens,
            FocusGraphActiveTransition::Exact(exact_final_active_domain),
        )
    }

    pub fn prepare_focus_domain_graph_transition<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        parent_before_child_additions: &[FocusDomainAddition],
        complete_subtree_removals: &[FocusDomainId],
        top_first_overlay_restoration_tokens: Vec<FocusRestorationToken>,
        exact_final_active_domain: Option<FocusDomainId>,
    ) -> Result<PreparedFocusDomainGraphMutation<'a>, FocusAuthorityError> {
        self.prepare_focus_domain_graph_transition_refs(
            capability,
            manager,
            parent_before_child_additions,
            complete_subtree_removals,
            &top_first_overlay_restoration_tokens,
            exact_final_active_domain,
        )
    }

    pub fn commit_focus_domain_graph_mutation(
        &self,
        capability: &InteractionDomainTransitionCapability,
        prepared: PreparedFocusDomainGraphMutation<'_>,
    ) {
        prepared.state.commit(capability);
    }

    pub fn prepare_focus_domain_retirement<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        retiring_focus_domains: &[FocusDomainId],
        top_first_overlay_restoration_tokens: Vec<FocusRestorationToken>,
        optional_replacement_focus_domain: Option<FocusDomainId>,
    ) -> Result<PreparedFocusDomainRetirement<'a>, FocusAuthorityError> {
        self.prepare_focus_domain_retirement_refs(
            capability,
            manager,
            retiring_focus_domains,
            &top_first_overlay_restoration_tokens,
            optional_replacement_focus_domain,
        )
    }

    pub(crate) fn prepare_focus_domain_retirement_refs<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        retiring_focus_domains: &[FocusDomainId],
        top_first_overlay_restoration_tokens: &[FocusRestorationToken],
        optional_replacement_focus_domain: Option<FocusDomainId>,
    ) -> Result<PreparedFocusDomainRetirement<'a>, FocusAuthorityError> {
        let binding = Self::validate_capability(capability, manager)?;
        let graph = Self::prepare_graph_mutation(
            manager,
            binding,
            &[],
            retiring_focus_domains,
            top_first_overlay_restoration_tokens,
            FocusGraphActiveTransition::PreserveOrReplaceIfRemoved(
                optional_replacement_focus_domain,
            ),
        )?;
        Ok(PreparedFocusDomainRetirement { graph })
    }

    pub(crate) fn prepare_focus_domain_retirement_transition_refs<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        retiring_focus_domains: &[FocusDomainId],
        top_first_overlay_restoration_tokens: &[FocusRestorationToken],
        exact_final_active_domain: Option<FocusDomainId>,
    ) -> Result<PreparedFocusDomainRetirement<'a>, FocusAuthorityError> {
        let binding = Self::validate_capability(capability, manager)?;
        let graph = Self::prepare_graph_mutation(
            manager,
            binding,
            &[],
            retiring_focus_domains,
            top_first_overlay_restoration_tokens,
            FocusGraphActiveTransition::Exact(exact_final_active_domain),
        )?;
        Ok(PreparedFocusDomainRetirement { graph })
    }

    pub fn prepare_focus_domain_retirement_transition<'a>(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &'a mut FocusManager,
        retiring_focus_domains: &[FocusDomainId],
        top_first_overlay_restoration_tokens: Vec<FocusRestorationToken>,
        exact_final_active_domain: Option<FocusDomainId>,
    ) -> Result<PreparedFocusDomainRetirement<'a>, FocusAuthorityError> {
        self.prepare_focus_domain_retirement_transition_refs(
            capability,
            manager,
            retiring_focus_domains,
            &top_first_overlay_restoration_tokens,
            exact_final_active_domain,
        )
    }

    pub fn commit_focus_domain_retirement(
        &self,
        capability: &InteractionDomainTransitionCapability,
        prepared: PreparedFocusDomainRetirement<'_>,
    ) {
        prepared.graph.state.commit(capability);
    }

    pub(crate) fn prepare_complete_focus_domain_retirement(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &FocusManager,
        retiring_focus_domains: &[FocusDomainId],
        top_first_overlay_restoration_tokens: &[FocusRestorationToken],
    ) -> Result<PreparedCompleteFocusDomainRetirement, FocusAuthorityError> {
        let capability_binding = Self::validate_capability(capability, manager)?;
        let removal_set = Self::validated_domain_set(retiring_focus_domains)?;
        let complete_domain_set = manager
            .focus_domains
            .keys()
            .copied()
            .collect::<HashSet<_>>();
        if removal_set != complete_domain_set {
            if let Some(missing) = complete_domain_set.difference(&removal_set).next().copied() {
                return Err(FocusAuthorityError::IncompleteSubtreeRemoval(missing));
            }
            let unknown = removal_set
                .difference(&complete_domain_set)
                .next()
                .copied()
                .expect("unequal focus-domain sets must have one differing member");
            return Err(FocusAuthorityError::UnknownFocusDomain(unknown));
        }
        let matched = Self::validate_top_tokens(manager, top_first_overlay_restoration_tokens)?;
        if matched.remaining_len != 0 {
            return Err(FocusAuthorityError::RestorationFrameBlocked);
        }
        if manager
            .active_focus_domain
            .is_some_and(|domain_id| !removal_set.contains(&domain_id))
        {
            return Err(FocusAuthorityError::InconsistentFocusState);
        }
        let preparation_identity = NEXT_FOCUS_GRAPH_PREPARATION_IDENTITY
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| FocusAuthorityError::Exhausted)?;
        Ok(PreparedCompleteFocusDomainRetirement {
            preparation_identity,
            capability_binding,
            before_parents: Self::parent_map(&manager.focus_domains),
            after_parents: HashMap::new(),
            removals: retiring_focus_domains.to_vec(),
            before_active_focus_domain: manager.active_focus_domain,
            before_focused: manager.focused,
            before_restoration_frames: manager
                .restoration_stack
                .iter()
                .map(|frame| {
                    (
                        frame.generation,
                        frame.activated_domain,
                        frame.superseded_domain,
                        frame.superseded_target,
                    )
                })
                .collect(),
            next_restoration_generation: manager.next_restoration_generation,
        })
    }

    pub(crate) fn commit_complete_focus_domain_retirement(
        &self,
        capability: &InteractionDomainTransitionCapability,
        manager: &mut FocusManager,
        prepared: PreparedCompleteFocusDomainRetirement,
    ) {
        assert_eq!(
            prepared.capability_binding, capability.binding,
            "complete focus-domain retirement capability changed after preparation"
        );
        assert_eq!(
            Self::parent_map(&manager.focus_domains),
            prepared.before_parents,
            "focus-domain graph changed after complete retirement preparation"
        );
        assert_eq!(
            manager.active_focus_domain, prepared.before_active_focus_domain,
            "active focus domain changed after complete retirement preparation"
        );
        assert_eq!(
            manager.focused, prepared.before_focused,
            "focused target changed after complete retirement preparation"
        );
        assert_eq!(
            manager
                .restoration_stack
                .iter()
                .map(|frame| {
                    (
                        frame.generation,
                        frame.activated_domain,
                        frame.superseded_domain,
                        frame.superseded_target,
                    )
                })
                .collect::<Vec<_>>(),
            prepared.before_restoration_frames,
            "focus restoration stack changed after complete retirement preparation"
        );
        manager.focus_domains.clear();
        manager.commit_focus_domain(None, None);
        manager.restoration_stack.clear();
        manager.next_restoration_generation = prepared.next_restoration_generation;
    }

    fn prepare_graph_mutation<'a>(
        manager: &'a mut FocusManager,
        binding: InteractionDomainTransitionBinding,
        additions: &[FocusDomainAddition],
        removals: &[FocusDomainId],
        restoration_tokens: &[FocusRestorationToken],
        active_transition: FocusGraphActiveTransition,
    ) -> Result<PreparedFocusDomainGraphMutation<'a>, FocusAuthorityError> {
        let before_parents = Self::parent_map(&manager.focus_domains);
        let before_active_focus_domain = manager.active_focus_domain;
        let mut focus_domains = manager.focus_domains.clone();
        let removal_set = Self::validated_domain_set(removals)?;
        for domain_id in &removal_set {
            if !focus_domains.contains_key(domain_id) {
                return Err(FocusAuthorityError::UnknownFocusDomain(*domain_id));
            }
        }

        let mut addition_ids = HashSet::new();
        for addition in additions {
            if removal_set.contains(&addition.domain_id) {
                return Err(FocusAuthorityError::DuplicateFocusDomain(
                    addition.domain_id,
                ));
            }
            if !addition_ids.insert(addition.domain_id)
                || focus_domains.contains_key(&addition.domain_id)
            {
                return Err(FocusAuthorityError::DuplicateFocusDomain(
                    addition.domain_id,
                ));
            }
            if let Some(parent) = addition.parent {
                if removal_set.contains(&parent) {
                    return Err(FocusAuthorityError::ParentRemoved(parent));
                }
                if !focus_domains.contains_key(&parent) {
                    return Err(FocusAuthorityError::ParentUnavailable(parent));
                }
            }
            focus_domains.insert(
                addition.domain_id,
                FocusDomainRecord {
                    interaction_domain_id: manager.interaction_domain_id,
                    parent: addition.parent,
                    targets: HashMap::new(),
                },
            );
        }

        for domain_id in focus_domains.keys().copied().collect::<Vec<_>>() {
            if !removal_set.contains(&domain_id)
                && Self::has_ancestor_in_set(&focus_domains, domain_id, &removal_set)
            {
                return Err(FocusAuthorityError::IncompleteSubtreeRemoval(domain_id));
            }
        }

        let mut restoration_stack = manager.restoration_stack.clone();
        match active_transition {
            FocusGraphActiveTransition::PreserveOrReplaceIfRemoved(_) => {
                let affected_frame_start = Self::validate_graph_restoration_disposition(
                    manager,
                    &removal_set,
                    restoration_tokens,
                )?;
                restoration_stack.truncate(affected_frame_start);
            }
            FocusGraphActiveTransition::Exact(_) => {
                let matched = Self::validate_top_tokens(manager, restoration_tokens)?;
                restoration_stack.truncate(matched.remaining_len);
                Self::validate_remaining_graph_restoration(&restoration_stack, &removal_set)?;
            }
        }

        for domain_id in &removal_set {
            focus_domains.remove(domain_id);
        }

        let (active_focus_domain, focused) = match active_transition {
            FocusGraphActiveTransition::PreserveOrReplaceIfRemoved(replacement) => {
                let active_is_removed = manager
                    .active_focus_domain
                    .is_some_and(|domain_id| removal_set.contains(&domain_id));
                if active_is_removed || manager.active_focus_domain.is_none() {
                    match replacement {
                        Some(domain_id) => {
                            if removal_set.contains(&domain_id) {
                                return Err(FocusAuthorityError::ReplacementInsideRemovedSubtree(
                                    domain_id,
                                ));
                            }
                            if !focus_domains.contains_key(&domain_id) {
                                return Err(FocusAuthorityError::UnknownFocusDomain(domain_id));
                            }
                            let focused = Self::resolve_target(&focus_domains, domain_id, None)?;
                            (Some(domain_id), focused)
                        }
                        None => (None, None),
                    }
                } else {
                    if let Some(domain_id) = replacement {
                        return Err(FocusAuthorityError::UnexpectedReplacement(domain_id));
                    }
                    let active = manager.active_focus_domain;
                    let focused = match active {
                        Some(domain_id) => {
                            let resolved =
                                Self::resolve_target(&focus_domains, domain_id, manager.focused)?;
                            if resolved != manager.focused {
                                return Err(FocusAuthorityError::InconsistentFocusState);
                            }
                            resolved
                        }
                        None => None,
                    };
                    (active, focused)
                }
            }
            FocusGraphActiveTransition::Exact(final_active_domain) => {
                if let Some(frame) = restoration_stack.last() {
                    if final_active_domain != Some(frame.activated_domain) {
                        return Err(FocusAuthorityError::RestorationFrameBlocked);
                    }
                }
                match final_active_domain {
                    Some(domain_id) => {
                        if removal_set.contains(&domain_id) {
                            return Err(FocusAuthorityError::ReplacementInsideRemovedSubtree(
                                domain_id,
                            ));
                        }
                        if !focus_domains.contains_key(&domain_id) {
                            return Err(FocusAuthorityError::UnknownFocusDomain(domain_id));
                        }
                        let preferred = (manager.active_focus_domain == Some(domain_id))
                            .then_some(manager.focused)
                            .flatten();
                        let focused = Self::resolve_target(&focus_domains, domain_id, preferred)?;
                        (Some(domain_id), focused)
                    }
                    None => (None, None),
                }
            }
        };

        let after_parents = Self::parent_map(&focus_domains);
        let preparation_identity = NEXT_FOCUS_GRAPH_PREPARATION_IDENTITY
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| FocusAuthorityError::Exhausted)?;
        Ok(PreparedFocusDomainGraphMutation {
            preparation_identity,
            state: Self::prepared_state(
                manager,
                binding,
                Some(focus_domains),
                active_focus_domain,
                focused,
                Some(restoration_stack),
                None,
            ),
            before_parents,
            after_parents,
            additions: additions
                .iter()
                .map(|addition| addition.domain_id)
                .collect(),
            removals: removals.to_vec(),
            before_active_focus_domain,
            after_active_focus_domain: active_focus_domain,
        })
    }

    fn validate_capability(
        capability: &InteractionDomainTransitionCapability,
        manager: &FocusManager,
    ) -> Result<InteractionDomainTransitionBinding, FocusAuthorityError> {
        if capability.binding.interaction_domain_id != manager.interaction_domain_id {
            return Err(FocusAuthorityError::CapabilityMismatch);
        }
        let mut claimed_targets = HashSet::new();
        for record in manager.focus_domains.values() {
            if record.interaction_domain_id != manager.interaction_domain_id
                || record
                    .targets
                    .keys()
                    .any(|target| !claimed_targets.insert(*target))
            {
                return Err(FocusAuthorityError::InconsistentFocusState);
            }
        }
        match manager.active_focus_domain {
            Some(domain_id) => {
                let domain = manager
                    .focus_domains
                    .get(&domain_id)
                    .ok_or(FocusAuthorityError::InconsistentFocusState)?;
                if manager
                    .focused
                    .is_some_and(|target| !domain.targets.get(&target).is_some_and(|r| r.eligible))
                {
                    return Err(FocusAuthorityError::InconsistentFocusState);
                }
            }
            None if manager.focused.is_some() => {
                return Err(FocusAuthorityError::InconsistentFocusState);
            }
            None => {}
        }
        if let Some(frame) = manager.restoration_stack.last() {
            if manager.active_focus_domain != Some(frame.activated_domain) {
                return Err(FocusAuthorityError::InconsistentFocusState);
            }
        }
        Ok(capability.binding)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepared_state<'a>(
        manager: &'a mut FocusManager,
        capability_binding: InteractionDomainTransitionBinding,
        focus_domains: Option<HashMap<FocusDomainId, FocusDomainRecord>>,
        active_focus_domain: Option<FocusDomainId>,
        focused: Option<TreeWidgetId>,
        restoration_stack: Option<Vec<FocusRestorationFrame>>,
        next_restoration_generation: Option<u64>,
    ) -> PreparedFocusState<'a> {
        PreparedFocusState {
            focus_domains: focus_domains.unwrap_or_else(|| manager.focus_domains.clone()),
            active_focus_domain,
            focused,
            restoration_stack: restoration_stack
                .unwrap_or_else(|| manager.restoration_stack.clone()),
            next_restoration_generation: next_restoration_generation
                .unwrap_or(manager.next_restoration_generation),
            manager,
            capability_binding,
        }
    }

    fn resolve_target(
        focus_domains: &HashMap<FocusDomainId, FocusDomainRecord>,
        domain_id: FocusDomainId,
        preferred: Option<TreeWidgetId>,
    ) -> Result<Option<TreeWidgetId>, FocusAuthorityError> {
        let domain = focus_domains
            .get(&domain_id)
            .ok_or(FocusAuthorityError::UnknownFocusDomain(domain_id))?;
        if let Some(target) = preferred {
            if domain
                .targets
                .get(&target)
                .is_some_and(|record| record.eligible)
            {
                return Ok(Some(target));
            }
        }
        Ok(domain
            .targets
            .iter()
            .filter(|(_, record)| record.eligible)
            .map(|(target, _)| *target)
            .min_by_key(TreeWidgetId::raw))
    }

    fn validate_overlay_activation_order(
        manager: &FocusManager,
        domains: &[FocusDomainId],
    ) -> Result<(), FocusAuthorityError> {
        for domain_id in domains {
            if !manager.focus_domains.contains_key(domain_id) {
                return Err(FocusAuthorityError::UnknownFocusDomain(*domain_id));
            }
        }
        let mut predecessor = None;
        for domain_id in domains.iter().copied() {
            if let Some(parent) = predecessor {
                if domain_id == parent
                    || !Self::is_descendant(&manager.focus_domains, domain_id, parent)
                {
                    return Err(FocusAuthorityError::InvalidOverlayDomainOrder(domain_id));
                }
            }
            predecessor = Some(domain_id);
        }
        Ok(())
    }

    fn is_descendant(
        domains: &HashMap<FocusDomainId, FocusDomainRecord>,
        candidate: FocusDomainId,
        ancestor: FocusDomainId,
    ) -> bool {
        let mut cursor = domains.get(&candidate).and_then(|record| record.parent);
        let mut visited = HashSet::new();
        while let Some(domain_id) = cursor {
            if domain_id == ancestor {
                return true;
            }
            if !visited.insert(domain_id) {
                return false;
            }
            cursor = domains.get(&domain_id).and_then(|record| record.parent);
        }
        false
    }

    fn validated_domain_set(
        domains: &[FocusDomainId],
    ) -> Result<HashSet<FocusDomainId>, FocusAuthorityError> {
        let mut result = HashSet::with_capacity(domains.len());
        for domain_id in domains.iter().copied() {
            if !result.insert(domain_id) {
                return Err(FocusAuthorityError::DuplicateFocusDomain(domain_id));
            }
        }
        Ok(result)
    }

    fn has_ancestor_in_set(
        domains: &HashMap<FocusDomainId, FocusDomainRecord>,
        domain_id: FocusDomainId,
        candidates: &HashSet<FocusDomainId>,
    ) -> bool {
        let mut cursor = domains.get(&domain_id).and_then(|record| record.parent);
        let mut visited = HashSet::new();
        while let Some(parent) = cursor {
            if candidates.contains(&parent) {
                return true;
            }
            if !visited.insert(parent) {
                return false;
            }
            cursor = domains.get(&parent).and_then(|record| record.parent);
        }
        false
    }

    fn parent_map(
        domains: &HashMap<FocusDomainId, FocusDomainRecord>,
    ) -> HashMap<FocusDomainId, Option<FocusDomainId>> {
        domains
            .iter()
            .map(|(domain_id, record)| (*domain_id, record.parent))
            .collect()
    }

    fn validate_graph_restoration_disposition(
        manager: &FocusManager,
        removal_set: &HashSet<FocusDomainId>,
        tokens: &[FocusRestorationToken],
    ) -> Result<usize, FocusAuthorityError> {
        for frame in &manager.restoration_stack {
            if !removal_set.contains(&frame.activated_domain) {
                if let Some(superseded_domain) = frame.superseded_domain {
                    if removal_set.contains(&superseded_domain) {
                        return Err(FocusAuthorityError::PeerRestorationDependency {
                            activated_domain: frame.activated_domain,
                            superseded_domain,
                        });
                    }
                }
            }
        }

        let affected = |frame: &FocusRestorationFrame| {
            removal_set.contains(&frame.activated_domain)
                || frame
                    .superseded_domain
                    .is_some_and(|domain_id| removal_set.contains(&domain_id))
        };
        let first_affected = manager.restoration_stack.iter().position(affected);
        let affected_count = match first_affected {
            Some(start) => {
                if manager.restoration_stack[start..]
                    .iter()
                    .any(|frame| !affected(frame))
                {
                    return Err(FocusAuthorityError::RestorationFrameBlocked);
                }
                manager.restoration_stack.len() - start
            }
            None => 0,
        };
        if affected_count != tokens.len() {
            return Err(FocusAuthorityError::RestorationTokenMismatch);
        }
        for (token, frame) in tokens
            .iter()
            .zip(manager.restoration_stack.iter().rev().take(affected_count))
        {
            Self::validate_token(manager.interaction_domain_id, token, frame)?;
        }
        Ok(first_affected.unwrap_or(manager.restoration_stack.len()))
    }

    fn validate_remaining_graph_restoration(
        restoration_stack: &[FocusRestorationFrame],
        removal_set: &HashSet<FocusDomainId>,
    ) -> Result<(), FocusAuthorityError> {
        for frame in restoration_stack {
            if removal_set.contains(&frame.activated_domain) {
                return Err(FocusAuthorityError::RestorationFrameBlocked);
            }
            if let Some(superseded_domain) = frame.superseded_domain {
                if removal_set.contains(&superseded_domain) {
                    return Err(FocusAuthorityError::PeerRestorationDependency {
                        activated_domain: frame.activated_domain,
                        superseded_domain,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_top_tokens<'a>(
        manager: &'a FocusManager,
        tokens: &[FocusRestorationToken],
    ) -> Result<MatchedRestorationFrames<'a>, FocusAuthorityError> {
        if tokens.len() > manager.restoration_stack.len() {
            return Err(FocusAuthorityError::RestorationTokenMismatch);
        }
        if let Some(top) = manager.restoration_stack.last() {
            if manager.active_focus_domain != Some(top.activated_domain) {
                return Err(FocusAuthorityError::InconsistentFocusState);
            }
        } else if !tokens.is_empty() {
            return Err(FocusAuthorityError::RestorationTokenMismatch);
        }
        let frames_top_first = manager
            .restoration_stack
            .iter()
            .rev()
            .take(tokens.len())
            .collect::<Vec<_>>();
        for (token, frame) in tokens.iter().zip(&frames_top_first) {
            Self::validate_token(manager.interaction_domain_id, token, frame)?;
        }
        Ok(MatchedRestorationFrames {
            remaining_len: manager.restoration_stack.len() - tokens.len(),
            frames_top_first,
        })
    }

    fn validate_token(
        interaction_domain_id: InteractionDomainId,
        token: &FocusRestorationToken,
        frame: &FocusRestorationFrame,
    ) -> Result<(), FocusAuthorityError> {
        if token.interaction_domain_id != interaction_domain_id
            || token.generation != frame.generation
            || token.activated_domain != frame.activated_domain
            || token.superseded_domain != frame.superseded_domain
        {
            return Err(FocusAuthorityError::RestorationTokenMismatch);
        }
        Ok(())
    }
}

static NEXT_FOCUS_GRAPH_PREPARATION_IDENTITY: AtomicU64 = AtomicU64::new(1);

struct MatchedRestorationFrames<'a> {
    remaining_len: usize,
    frames_top_first: Vec<&'a FocusRestorationFrame>,
}

impl FocusManager {
    pub fn interaction_domain_id(&self) -> InteractionDomainId {
        self.interaction_domain_id
    }

    pub fn active_focus_domain(&self) -> Option<FocusDomainId> {
        self.active_focus_domain
    }

    pub fn focus_domain_parent(
        &self,
        focus_domain_id: FocusDomainId,
    ) -> Option<Option<FocusDomainId>> {
        self.focus_domains
            .get(&focus_domain_id)
            .map(|record| record.parent)
    }

    pub fn focus_domain_count(&self) -> usize {
        self.focus_domains.len()
    }

    pub fn restoration_depth(&self) -> usize {
        self.restoration_stack.len()
    }
}
