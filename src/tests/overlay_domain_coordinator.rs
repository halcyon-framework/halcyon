use crate::compositor::SurfaceId;
use crate::core::event::{InteractionDomainId, InteractionDomains};
use crate::core::focus_authority::{FocusDomainAddition, FocusDomainId, FocusResolver};
use crate::core::overlay_domain_coordinator::{
    OverlayActivationAdmission, OverlayDisposition, OverlayDomainCoordinator, OverlayDomainError,
    OverlayFrameDescriptor, OverlayFrameId, OverlayOwnerId, OverlayOwnerLifecyclePort,
    OverlayOwnerLifecycleState, OverlayStackDescriptor, OverlayStackId, RouterEligibilityTestimony,
};

const OWNER: OverlayOwnerId = OverlayOwnerId(10);
const STACK: OverlayStackId = OverlayStackId(20);
const ROOT_FOCUS: FocusDomainId = FocusDomainId(30);
const CHILD_FOCUS: FocusDomainId = FocusDomainId(31);
const NESTED_FOCUS: FocusDomainId = FocusDomainId(32);
const BACKGROUND_FOCUS: FocusDomainId = FocusDomainId(29);
const NESTED_FRAME: OverlayFrameId = OverlayFrameId(42);

fn owner_port() -> OverlayOwnerLifecyclePort {
    OverlayOwnerLifecyclePort::new(
        OWNER,
        InteractionDomainId::PRIMARY,
        vec![
            OverlayStackDescriptor::new(
                STACK,
                vec![
                    OverlayFrameDescriptor::new(
                        OverlayFrameId(40),
                        ROOT_FOCUS,
                        SurfaceId::test_id(50),
                    ),
                    OverlayFrameDescriptor::new(
                        OverlayFrameId(41),
                        CHILD_FOCUS,
                        SurfaceId::test_id(51),
                    ),
                ],
            ),
            OverlayStackDescriptor::new(
                OverlayStackId(21),
                vec![OverlayFrameDescriptor::new(
                    NESTED_FRAME,
                    NESTED_FOCUS,
                    SurfaceId::test_id(52),
                )],
            ),
        ],
    )
    .unwrap()
}

fn setup(
    admission: OverlayActivationAdmission,
) -> (
    InteractionDomains,
    OverlayDomainCoordinator,
    OverlayOwnerLifecyclePort,
) {
    let mut interaction_domains = InteractionDomains::new();
    let coordinator = OverlayDomainCoordinator::new().unwrap();
    coordinator
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();

    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let manager = &mut interaction_domains.primary_state_mut().focus_manager;
    let prepared = FocusResolver::new()
        .prepare_focus_domain_graph_mutation(
            guard.focus_capability(),
            manager,
            &[
                FocusDomainAddition::new(BACKGROUND_FOCUS, None),
                FocusDomainAddition::new(ROOT_FOCUS, None),
                FocusDomainAddition::new(CHILD_FOCUS, Some(ROOT_FOCUS)),
                FocusDomainAddition::new(NESTED_FOCUS, Some(CHILD_FOCUS)),
            ],
            &[],
            vec![],
            None,
        )
        .unwrap();
    FocusResolver::new().commit_focus_domain_graph_mutation(guard.focus_capability(), prepared);
    let prepared = FocusResolver::new()
        .prepare_focus_domain_activation(guard.focus_capability(), manager, BACKGROUND_FOCUS)
        .unwrap();
    FocusResolver::new().commit_focus_domain_activation(guard.focus_capability(), prepared);
    drop(guard);

    let port = owner_port();
    coordinator.register_owner(port.clone(), admission).unwrap();
    (interaction_domains, coordinator, port)
}

#[test]
fn transition_guard_is_non_blocking_and_generation_bound() {
    let interaction_domains = InteractionDomains::new();
    let coordinator = OverlayDomainCoordinator::new().unwrap();
    coordinator
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();

    let first = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert!(matches!(
        coordinator.begin_domain_transition(InteractionDomainId::PRIMARY),
        Err(OverlayDomainError::Busy(InteractionDomainId::PRIMARY))
    ));
    let first_generation = first.generation();
    drop(first);
    let second = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert!(second.generation() > first_generation);
}

#[test]
fn activation_and_deactivation_publish_one_coherent_stack() {
    let (mut interaction_domains, coordinator, port) =
        setup(OverlayActivationAdmission::DomainLocal);

    coordinator
        .activate_owner(&mut interaction_domains, OWNER, STACK, None)
        .unwrap();
    assert_eq!(
        port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active { stack_id: STACK }
    );
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let projection = coordinator
        .active_projection_guarded(&guard)
        .unwrap()
        .unwrap();
    assert_eq!(projection.owner_id, OWNER);
    assert_eq!(projection.stack_id, STACK);
    assert_eq!(
        projection.surfaces_bottom_first,
        vec![SurfaceId::test_id(50), SurfaceId::test_id(51)]
    );
    drop(guard);

    coordinator
        .deactivate_owner(&mut interaction_domains, OWNER, OverlayDisposition::Suspend)
        .unwrap();
    assert_eq!(port.lifecycle_state(), OverlayOwnerLifecycleState::Inactive);
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert_eq!(coordinator.active_projection_guarded(&guard).unwrap(), None);
}

#[test]
fn aborted_activation_releases_owner_preparation_without_publishing() {
    let (mut interaction_domains, coordinator, port) =
        setup(OverlayActivationAdmission::DomainLocal);
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let prepared = coordinator
        .prepare_activation(&guard, &mut interaction_domains, OWNER, STACK, None)
        .unwrap();
    drop(prepared);
    assert_eq!(port.lifecycle_state(), OverlayOwnerLifecycleState::Inactive);
    assert_eq!(coordinator.active_binding_guarded(&guard).unwrap(), None);
    drop(guard);

    coordinator
        .activate_owner(&mut interaction_domains, OWNER, STACK, None)
        .unwrap();
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert!(
        coordinator
            .active_binding_guarded(&guard)
            .unwrap()
            .unwrap()
            .generation
            > 1
    );
}

#[test]
fn aborted_deactivation_returns_focus_tokens_to_the_active_owner() {
    let (mut interaction_domains, coordinator, port) =
        setup(OverlayActivationAdmission::DomainLocal);
    coordinator
        .activate_owner(&mut interaction_domains, OWNER, STACK, None)
        .unwrap();

    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let prepared = coordinator
        .prepare_deactivation(
            &guard,
            &mut interaction_domains,
            OWNER,
            OverlayDisposition::Dismiss,
        )
        .unwrap();
    drop(prepared);
    assert_eq!(
        port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active { stack_id: STACK }
    );
    assert!(
        coordinator
            .active_binding_guarded(&guard)
            .unwrap()
            .is_some()
    );
    drop(guard);

    coordinator
        .deactivate_owner(&mut interaction_domains, OWNER, OverlayDisposition::Dismiss)
        .unwrap();
}

#[test]
fn focus_retirement_rejects_suspend_without_mutating_owner_or_focus() {
    let (mut interaction_domains, coordinator, port) =
        setup(OverlayActivationAdmission::DomainLocal);
    coordinator
        .activate_owner(&mut interaction_domains, OWNER, STACK, None)
        .unwrap();

    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let before_focus = interaction_domains
        .primary_state()
        .focus_manager
        .active_focus_domain();
    let result = coordinator.prepare_deactivate_for_focus_retirement_guarded(
        &guard,
        &mut interaction_domains,
        OWNER,
        OverlayDisposition::Suspend,
        &[ROOT_FOCUS, CHILD_FOCUS, NESTED_FOCUS],
        Some(BACKGROUND_FOCUS),
    );

    assert!(matches!(
        result,
        Err(OverlayDomainError::RetirementRequiresDismiss(OWNER))
    ));
    assert_eq!(
        port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active { stack_id: STACK }
    );
    assert!(
        coordinator
            .active_binding_guarded(&guard)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        before_focus
    );
}

#[test]
fn nested_push_and_pop_update_focus_and_projection_as_one_guarded_transition() {
    let (mut interaction_domains, coordinator, _) = setup(OverlayActivationAdmission::DomainLocal);
    coordinator
        .activate_owner(&mut interaction_domains, OWNER, STACK, None)
        .unwrap();

    let before = {
        let guard = coordinator
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .unwrap();
        coordinator.active_binding_guarded(&guard).unwrap().unwrap()
    };
    coordinator
        .push_nested_frame(&mut interaction_domains, OWNER, NESTED_FRAME, None)
        .unwrap();
    let (after_push, projection) = {
        let guard = coordinator
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .unwrap();
        (
            coordinator.active_binding_guarded(&guard).unwrap().unwrap(),
            coordinator
                .active_projection_guarded(&guard)
                .unwrap()
                .unwrap(),
        )
    };
    assert!(after_push.generation > before.generation);
    assert_eq!(
        projection.surfaces_bottom_first,
        vec![
            SurfaceId::test_id(50),
            SurfaceId::test_id(51),
            SurfaceId::test_id(52),
        ]
    );

    coordinator
        .pop_nested_frame(&mut interaction_domains, OWNER, None)
        .unwrap();
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let after_pop = coordinator.active_binding_guarded(&guard).unwrap().unwrap();
    assert!(after_pop.generation > after_push.generation);
    assert_eq!(
        coordinator
            .active_projection_guarded(&guard)
            .unwrap()
            .unwrap()
            .surfaces_bottom_first,
        vec![SurfaceId::test_id(50), SurfaceId::test_id(51)]
    );
}

#[test]
fn compound_exit_selects_the_next_focus_domain_without_intermediate_restoration() {
    let (mut interaction_domains, coordinator, port) =
        setup(OverlayActivationAdmission::DomainLocal);
    coordinator
        .activate_owner(&mut interaction_domains, OWNER, STACK, None)
        .unwrap();

    coordinator
        .deactivate_owner_to_domain(
            &mut interaction_domains,
            OWNER,
            OverlayDisposition::Dismiss,
            BACKGROUND_FOCUS,
        )
        .unwrap();

    assert_eq!(port.lifecycle_state(), OverlayOwnerLifecycleState::Inactive);
    assert_eq!(
        interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(BACKGROUND_FOCUS)
    );
    assert_eq!(
        interaction_domains
            .primary_state()
            .focus_manager
            .restoration_depth(),
        0
    );
}

#[test]
fn owner_and_domain_unregistration_are_prepared_and_reject_live_state() {
    let (mut interaction_domains, coordinator, port) =
        setup(OverlayActivationAdmission::DomainLocal);
    coordinator
        .activate_owner(&mut interaction_domains, OWNER, STACK, None)
        .unwrap();
    assert_eq!(
        coordinator.unregister_owner(OWNER),
        Err(OverlayDomainError::OwnerAlreadyActive(OWNER))
    );
    coordinator
        .deactivate_owner(&mut interaction_domains, OWNER, OverlayDisposition::Dismiss)
        .unwrap();

    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let prepared = coordinator
        .prepare_owner_unregistration(&guard, OWNER)
        .unwrap();
    drop(prepared);
    drop(guard);
    port.register_inactive_stack(OverlayStackDescriptor::new(
        STACK,
        vec![
            OverlayFrameDescriptor::new(OverlayFrameId(43), ROOT_FOCUS, SurfaceId::test_id(53)),
            OverlayFrameDescriptor::new(OverlayFrameId(44), CHILD_FOCUS, SurfaceId::test_id(54)),
        ],
    ))
    .unwrap();
    coordinator
        .activate_owner(&mut interaction_domains, OWNER, STACK, None)
        .unwrap();
    coordinator
        .deactivate_owner(&mut interaction_domains, OWNER, OverlayDisposition::Dismiss)
        .unwrap();

    coordinator.unregister_owner(OWNER).unwrap();
    coordinator
        .unregister_domain(InteractionDomainId::PRIMARY)
        .unwrap();
    assert!(matches!(
        coordinator.begin_domain_transition(InteractionDomainId::PRIMARY),
        Err(OverlayDomainError::UnknownInteractionDomain(
            InteractionDomainId::PRIMARY
        ))
    ));
}

#[test]
fn router_managed_owner_requires_exact_call_scoped_eligibility() {
    let (mut interaction_domains, coordinator, _) =
        setup(OverlayActivationAdmission::RouterEligibilityRequired);
    assert_eq!(
        coordinator.activate_owner(&mut interaction_domains, OWNER, STACK, None),
        Err(OverlayDomainError::MissingRouterEligibility(OWNER))
    );

    let registration_generation = coordinator.owner_registration_generation(OWNER).unwrap();
    let eligibility = RouterEligibilityTestimony::issue(
        &coordinator,
        InteractionDomainId::PRIMARY,
        OWNER,
        registration_generation,
        77,
        88,
        [ROOT_FOCUS, CHILD_FOCUS],
    );
    assert_eq!(eligibility.router_generation(), 77);
    assert_eq!(eligibility.root_binding_generation(), 88);
    coordinator
        .activate_owner(&mut interaction_domains, OWNER, STACK, Some(eligibility))
        .unwrap();
}
