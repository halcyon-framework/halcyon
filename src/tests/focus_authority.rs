use crate::compositor::{Compositor, SceneKey, SurfaceId};
use crate::core::event::{FocusManager, InteractionDomainId, InteractionDomains, WindowId};
use crate::core::focus_authority::{
    FocusAuthorityError, FocusDomainAddition, FocusDomainId, FocusResolver,
    FocusTargetRegistration, InteractionDomainTransitionCapability,
};
use crate::core::math::RectF;
use crate::utils::id::TreeWidgetId;

fn domain(id: u64) -> FocusDomainId {
    FocusDomainId(id)
}

fn target(id: u64) -> TreeWidgetId {
    TreeWidgetId(id)
}

fn capability(manager: &FocusManager, generation: u64) -> InteractionDomainTransitionCapability {
    InteractionDomainTransitionCapability::issue(manager.interaction_domain_id(), generation)
        .unwrap()
}

fn install_domains(
    manager: &mut FocusManager,
    additions: &[FocusDomainAddition],
    replacement: Option<FocusDomainId>,
    generation: u64,
) {
    let resolver = FocusResolver::new();
    let capability = capability(manager, generation);
    let prepared = resolver
        .prepare_focus_domain_graph_mutation(
            &capability,
            manager,
            additions,
            &[],
            Vec::new(),
            replacement,
        )
        .unwrap();
    resolver.commit_focus_domain_graph_mutation(&capability, prepared);
}

fn register_targets(
    manager: &mut FocusManager,
    additions: &[FocusTargetRegistration],
    generation: u64,
) {
    let resolver = FocusResolver::new();
    let capability = capability(manager, generation);
    let prepared = resolver
        .prepare_focus_target_mutation(&capability, manager, additions, &[])
        .unwrap();
    resolver.commit_focus_target_mutation(&capability, prepared);
}

fn surface(compositor: &mut Compositor, key: u64) -> SurfaceId {
    compositor.create_surface(
        key as i32,
        RectF::new(0.0, 0.0, 100.0, 100.0),
        SceneKey::new(key),
    )
}

#[test]
fn interaction_domain_state_constructs_focus_manager_for_its_actual_domain() {
    let mut domains = InteractionDomains::new();
    let assigned = domains.register_window(WindowId(42));

    assert_ne!(assigned, InteractionDomainId::PRIMARY);
    assert_eq!(
        domains
            .state(assigned)
            .unwrap()
            .focus_manager
            .interaction_domain_id(),
        assigned
    );
}

#[test]
fn batch_graph_preparation_is_abortable_and_admits_parent_before_child_atomically() {
    let resolver = FocusResolver::new();
    let mut manager = FocusManager::new();
    let additions = [
        FocusDomainAddition::new(domain(1), None),
        FocusDomainAddition::new(domain(2), Some(domain(1))),
    ];
    let capability = capability(&manager, 1);
    let prepared = resolver
        .prepare_focus_domain_graph_mutation(
            &capability,
            &mut manager,
            &additions,
            &[],
            Vec::new(),
            Some(domain(2)),
        )
        .unwrap();
    let testimony = prepared.testimony();
    assert_eq!(testimony.additions(), &[domain(1), domain(2)]);
    assert_eq!(testimony.parent_before(domain(1)), None);
    assert_eq!(testimony.parent_after(domain(1)), Some(None));
    assert_eq!(testimony.parent_after(domain(2)), Some(Some(domain(1))));
    drop(prepared);

    assert_eq!(manager.focus_domain_count(), 0);
    assert_eq!(manager.active_focus_domain(), None);

    let capability = self::capability(&manager, 2);
    let prepared = resolver
        .prepare_focus_domain_graph_mutation(
            &capability,
            &mut manager,
            &additions,
            &[],
            Vec::new(),
            Some(domain(2)),
        )
        .unwrap();
    resolver.commit_focus_domain_graph_mutation(&capability, prepared);

    assert_eq!(manager.focus_domain_count(), 2);
    assert_eq!(
        manager.focus_domain_parent(domain(2)),
        Some(Some(domain(1)))
    );
    assert_eq!(manager.active_focus_domain(), Some(domain(2)));
    assert_eq!(manager.focused(), None);
}

#[test]
fn graph_mutation_rejects_partial_subtree_removal_and_swaps_complete_trees() {
    let resolver = FocusResolver::new();
    let mut manager = FocusManager::new();
    install_domains(
        &mut manager,
        &[
            FocusDomainAddition::new(domain(1), None),
            FocusDomainAddition::new(domain(2), Some(domain(1))),
        ],
        Some(domain(2)),
        1,
    );

    let capability = capability(&manager, 2);
    assert!(matches!(
        resolver.prepare_focus_domain_graph_mutation(
            &capability,
            &mut manager,
            &[],
            &[domain(1)],
            Vec::new(),
            None,
        ),
        Err(FocusAuthorityError::IncompleteSubtreeRemoval(id)) if id == domain(2)
    ));
    assert_eq!(manager.focus_domain_count(), 2);

    let capability = self::capability(&manager, 3);
    let prepared = resolver
        .prepare_focus_domain_graph_mutation(
            &capability,
            &mut manager,
            &[
                FocusDomainAddition::new(domain(10), None),
                FocusDomainAddition::new(domain(11), Some(domain(10))),
            ],
            &[domain(1), domain(2)],
            Vec::new(),
            Some(domain(11)),
        )
        .unwrap();
    let testimony = prepared.testimony();
    assert_eq!(testimony.removals(), &[domain(1), domain(2)]);
    assert_eq!(testimony.parent_after(domain(11)), Some(Some(domain(10))));
    resolver.commit_focus_domain_graph_mutation(&capability, prepared);

    assert_eq!(manager.focus_domain_count(), 2);
    assert_eq!(manager.focus_domain_parent(domain(1)), None);
    assert_eq!(
        manager.focus_domain_parent(domain(11)),
        Some(Some(domain(10)))
    );
    assert_eq!(manager.active_focus_domain(), Some(domain(11)));
}

#[test]
fn target_claims_are_unique_and_projection_comes_from_the_authoritative_selection() {
    let resolver = FocusResolver::new();
    let mut manager = FocusManager::new();
    install_domains(
        &mut manager,
        &[
            FocusDomainAddition::new(domain(1), None),
            FocusDomainAddition::new(domain(2), None),
        ],
        Some(domain(1)),
        1,
    );
    let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 100.0, 100.0));
    let first_surface = surface(&mut compositor, 1);
    register_targets(
        &mut manager,
        &[FocusTargetRegistration::new(
            domain(1),
            target(7),
            Some(first_surface),
            true,
        )],
        2,
    );

    let capability = capability(&manager, 3);
    assert!(matches!(
        resolver.prepare_focus_target_mutation(
            &capability,
            &mut manager,
            &[FocusTargetRegistration::new(
                domain(2),
                target(7),
                None,
                true,
            )],
            &[],
        ),
        Err(FocusAuthorityError::DuplicateFocusTarget(id)) if id == target(7)
    ));

    let capability = self::capability(&manager, 4);
    assert_eq!(
        resolver
            .focus_projection_guarded(&capability, &manager, InteractionDomainId::PRIMARY,)
            .unwrap()
            .focused_surface,
        Some(first_surface)
    );
}

#[test]
fn nested_overlay_restoration_is_lifo_and_restores_the_exact_previous_target() {
    let resolver = FocusResolver::new();
    let mut manager = FocusManager::new();
    install_domains(
        &mut manager,
        &[
            FocusDomainAddition::new(domain(1), None),
            FocusDomainAddition::new(domain(2), Some(domain(1))),
            FocusDomainAddition::new(domain(3), Some(domain(2))),
        ],
        Some(domain(1)),
        1,
    );
    let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 100.0, 100.0));
    let base_fallback_surface = surface(&mut compositor, 1);
    let base_selected_surface = surface(&mut compositor, 2);
    let overlay_surface = surface(&mut compositor, 3);
    let nested_surface = surface(&mut compositor, 4);
    register_targets(
        &mut manager,
        &[
            FocusTargetRegistration::new(domain(1), target(10), Some(base_fallback_surface), true),
            FocusTargetRegistration::new(domain(1), target(20), Some(base_selected_surface), true),
            FocusTargetRegistration::new(domain(2), target(30), Some(overlay_surface), true),
            FocusTargetRegistration::new(domain(3), target(40), Some(nested_surface), true),
        ],
        2,
    );
    let capability = capability(&manager, 3);
    let prepared = resolver
        .prepare_focus_target_selection(&capability, &mut manager, target(20))
        .unwrap();
    resolver.commit_focus_target_selection(&capability, prepared);

    let capability = self::capability(&manager, 4);
    let prepared = resolver
        .prepare_overlay_focus_activation(&capability, &mut manager, &[domain(2), domain(3)])
        .unwrap();
    let mut tokens = resolver.commit_overlay_focus_activation(&capability, prepared);
    assert_eq!(manager.restoration_depth(), 2);
    assert_eq!(manager.active_focus_domain(), Some(domain(3)));
    assert_eq!(manager.focused(), Some(target(40)));

    let nested_token = tokens.pop().unwrap();
    let capability = self::capability(&manager, 5);
    let prepared = resolver
        .prepare_restoration_tokens(&capability, &mut manager, vec![nested_token])
        .unwrap();
    resolver.submit_restoration_tokens_guarded(&capability, prepared);
    assert_eq!(manager.restoration_depth(), 1);
    assert_eq!(manager.active_focus_domain(), Some(domain(2)));
    assert_eq!(manager.focused(), Some(target(30)));

    let outer_token = tokens.pop().unwrap();
    let capability = self::capability(&manager, 6);
    let prepared = resolver
        .prepare_restoration_tokens(&capability, &mut manager, vec![outer_token])
        .unwrap();
    resolver.submit_restoration_tokens_guarded(&capability, prepared);
    assert_eq!(manager.restoration_depth(), 0);
    assert_eq!(manager.active_focus_domain(), Some(domain(1)));
    assert_eq!(manager.focused(), Some(target(20)));
    assert!(tokens.is_empty());

    let capability = self::capability(&manager, 7);
    assert_eq!(
        resolver
            .focus_projection_guarded(&capability, &manager, InteractionDomainId::PRIMARY,)
            .unwrap()
            .focused_surface,
        Some(base_selected_surface)
    );
}

#[test]
fn restoration_falls_back_deterministically_when_the_exact_target_was_invalidated() {
    let resolver = FocusResolver::new();
    let mut manager = FocusManager::new();
    install_domains(
        &mut manager,
        &[
            FocusDomainAddition::new(domain(1), None),
            FocusDomainAddition::new(domain(2), Some(domain(1))),
        ],
        Some(domain(1)),
        1,
    );
    register_targets(
        &mut manager,
        &[
            FocusTargetRegistration::new(domain(1), target(10), None, true),
            FocusTargetRegistration::new(domain(1), target(20), None, true),
            FocusTargetRegistration::new(domain(2), target(30), None, true),
        ],
        2,
    );
    let capability = capability(&manager, 3);
    let prepared = resolver
        .prepare_focus_target_selection(&capability, &mut manager, target(20))
        .unwrap();
    resolver.commit_focus_target_selection(&capability, prepared);
    let capability = self::capability(&manager, 4);
    let prepared = resolver
        .prepare_overlay_focus_activation(&capability, &mut manager, &[domain(2)])
        .unwrap();
    let token = resolver
        .commit_overlay_focus_activation(&capability, prepared)
        .pop()
        .unwrap();

    let capability = self::capability(&manager, 5);
    let prepared = resolver
        .prepare_focus_target_mutation(&capability, &mut manager, &[], &[target(20)])
        .unwrap();
    resolver.commit_focus_target_mutation(&capability, prepared);
    let capability = self::capability(&manager, 6);
    let prepared = resolver
        .prepare_restoration_tokens(&capability, &mut manager, vec![token])
        .unwrap();
    resolver.submit_restoration_tokens_guarded(&capability, prepared);

    assert_eq!(manager.active_focus_domain(), Some(domain(1)));
    assert_eq!(manager.focused(), Some(target(10)));
}

#[test]
fn retirement_rejects_a_live_peer_restoration_frame_until_the_peer_exits() {
    let resolver = FocusResolver::new();
    let mut manager = FocusManager::new();
    install_domains(
        &mut manager,
        &[
            FocusDomainAddition::new(domain(1), None),
            FocusDomainAddition::new(domain(10), None),
            FocusDomainAddition::new(domain(11), Some(domain(10))),
        ],
        Some(domain(1)),
        1,
    );
    let capability = capability(&manager, 2);
    let prepared = resolver
        .prepare_overlay_focus_activation(&capability, &mut manager, &[domain(11)])
        .unwrap();
    let peer_token = resolver
        .commit_overlay_focus_activation(&capability, prepared)
        .pop()
        .unwrap();

    let capability = self::capability(&manager, 3);
    assert!(matches!(
        resolver.prepare_focus_domain_retirement(
            &capability,
            &mut manager,
            &[domain(1)],
            Vec::new(),
            None,
        ),
        Err(FocusAuthorityError::PeerRestorationDependency {
            activated_domain,
            superseded_domain,
        }) if activated_domain == domain(11) && superseded_domain == domain(1)
    ));
    assert_eq!(manager.focus_domain_count(), 3);
    assert_eq!(manager.restoration_depth(), 1);
    assert_eq!(manager.active_focus_domain(), Some(domain(11)));

    let capability = self::capability(&manager, 4);
    let prepared = resolver
        .prepare_overlay_focus_exit_to_domain(
            &capability,
            &mut manager,
            vec![peer_token],
            domain(10),
        )
        .unwrap();
    resolver.commit_overlay_focus_exit_to_domain(&capability, prepared);
    assert_eq!(manager.active_focus_domain(), Some(domain(10)));
    assert_eq!(manager.restoration_depth(), 0);

    let capability = self::capability(&manager, 5);
    let prepared = resolver
        .prepare_focus_domain_retirement(&capability, &mut manager, &[domain(1)], Vec::new(), None)
        .unwrap();
    resolver.commit_focus_domain_retirement(&capability, prepared);
    assert_eq!(manager.focus_domain_count(), 2);
    assert_eq!(manager.active_focus_domain(), Some(domain(10)));
}

#[test]
fn owner_tokens_allow_active_overlay_subtree_retirement_without_intermediate_restore() {
    let resolver = FocusResolver::new();
    let mut manager = FocusManager::new();
    install_domains(
        &mut manager,
        &[
            FocusDomainAddition::new(domain(1), None),
            FocusDomainAddition::new(domain(2), Some(domain(1))),
        ],
        Some(domain(1)),
        1,
    );
    let capability = capability(&manager, 2);
    let prepared = resolver
        .prepare_overlay_focus_activation(&capability, &mut manager, &[domain(2)])
        .unwrap();
    let token = resolver
        .commit_overlay_focus_activation(&capability, prepared)
        .pop()
        .unwrap();
    let capability = self::capability(&manager, 3);
    let prepared = resolver
        .prepare_focus_domain_retirement(
            &capability,
            &mut manager,
            &[domain(1), domain(2)],
            vec![token],
            None,
        )
        .unwrap();
    resolver.commit_focus_domain_retirement(&capability, prepared);

    assert_eq!(manager.focus_domain_count(), 0);
    assert_eq!(manager.active_focus_domain(), None);
    assert_eq!(manager.focused(), None);
    assert_eq!(manager.restoration_depth(), 0);
}

#[test]
fn wrong_interaction_domain_capability_rejects_without_focus_mutation() {
    let resolver = FocusResolver::new();
    let mut manager = FocusManager::new();
    let wrong = InteractionDomainTransitionCapability::issue(InteractionDomainId(99), 1).unwrap();

    assert!(matches!(
        resolver.prepare_focus_domain_graph_mutation(
            &wrong,
            &mut manager,
            &[FocusDomainAddition::new(domain(1), None)],
            &[],
            Vec::new(),
            Some(domain(1)),
        ),
        Err(FocusAuthorityError::CapabilityMismatch)
    ));
    assert_eq!(manager.focus_domain_count(), 0);
    assert_eq!(manager.focused(), None);
}

#[test]
fn overlay_generation_exhaustion_rejects_the_complete_activation_without_mutation() {
    let resolver = FocusResolver::new();
    let mut manager = FocusManager::new();
    install_domains(
        &mut manager,
        &[
            FocusDomainAddition::new(domain(1), None),
            FocusDomainAddition::new(domain(2), Some(domain(1))),
        ],
        Some(domain(1)),
        1,
    );
    manager.next_restoration_generation = u64::MAX;
    let capability = capability(&manager, 2);

    assert!(matches!(
        resolver.prepare_overlay_focus_activation(&capability, &mut manager, &[domain(2)],),
        Err(FocusAuthorityError::Exhausted)
    ));
    assert_eq!(manager.active_focus_domain(), Some(domain(1)));
    assert_eq!(manager.restoration_depth(), 0);
    assert_eq!(manager.next_restoration_generation, u64::MAX);
}
