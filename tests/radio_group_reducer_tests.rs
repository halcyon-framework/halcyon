// SPEC-HAL-130 RadioGroup Reducer Tests
// Unit tests for the pure semantic core.

use halcyon::widgets::radio_group::{
    RadioGroupEvent, RadioGroupState, RadioId, reduce_radio_group,
};

fn default_state() -> RadioGroupState {
    RadioGroupState {
        selected: None,
        disabled: false,
        focused: None,
        active: None,
    }
}

// ============================================================================
// Test: Select event
// ============================================================================

#[test]
fn select_sets_selected_from_none() {
    let mut state = default_state();
    let member = RadioId(1);

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Select(member));

    assert_eq!(state.selected, Some(member), "Select should set selected");
    assert_eq!(result, Some(Some(member)), "Should emit new selected value");
}

#[test]
fn select_changes_selected() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: None,
        active: None,
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Select(RadioId(2)));

    assert_eq!(
        state.selected,
        Some(RadioId(2)),
        "Select should change selected"
    );
    assert_eq!(
        result,
        Some(Some(RadioId(2))),
        "Should emit new selected value"
    );
}

#[test]
fn select_same_member_no_emission() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: None,
        active: None,
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Select(RadioId(1)));

    assert_eq!(
        state.selected,
        Some(RadioId(1)),
        "Selected should remain same"
    );
    assert_eq!(result, None, "No emission when selected unchanged");
}

#[test]
fn select_preserves_focused_and_active() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(2)),
        active: Some(RadioId(3)),
    };

    reduce_radio_group(&mut state, RadioGroupEvent::Select(RadioId(1)));

    assert_eq!(
        state.focused,
        Some(RadioId(2)),
        "Select should not change focused"
    );
    assert_eq!(
        state.active,
        Some(RadioId(3)),
        "Select should not change active"
    );
}

// ============================================================================
// Test: Activate event
// ============================================================================

#[test]
fn activate_sets_selected_and_clears_active() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(1)),
        active: Some(RadioId(1)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Activate(RadioId(1)));

    assert_eq!(
        state.selected,
        Some(RadioId(1)),
        "Activate should set selected"
    );
    assert_eq!(state.active, None, "Activate should clear active");
    assert_eq!(
        result,
        Some(Some(RadioId(1))),
        "Should emit new selected value"
    );
}

#[test]
fn activate_changes_selected() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: None,
        active: Some(RadioId(2)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Activate(RadioId(2)));

    assert_eq!(
        state.selected,
        Some(RadioId(2)),
        "Activate should change selected"
    );
    assert_eq!(
        result,
        Some(Some(RadioId(2))),
        "Should emit new selected value"
    );
}

#[test]
fn activate_same_member_no_emission() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: None,
        active: Some(RadioId(1)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Activate(RadioId(1)));

    assert_eq!(
        state.selected,
        Some(RadioId(1)),
        "Selected should remain same"
    );
    assert_eq!(state.active, None, "Active should be cleared");
    assert_eq!(result, None, "No emission when selected unchanged");
}

#[test]
fn activate_preserves_focused() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(2)),
        active: None,
    };

    reduce_radio_group(&mut state, RadioGroupEvent::Activate(RadioId(1)));

    assert_eq!(
        state.focused,
        Some(RadioId(2)),
        "Activate should not change focused"
    );
}

#[test]
fn activate_clears_active_even_for_different_member() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: None,
        active: Some(RadioId(2)),
    };

    reduce_radio_group(&mut state, RadioGroupEvent::Activate(RadioId(1)));

    assert_eq!(
        state.active, None,
        "Activate should clear active regardless of which member"
    );
}

// ============================================================================
// Test: Cancel event
// ============================================================================

#[test]
fn cancel_clears_active_for_matching_member() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: Some(RadioId(2)),
        active: Some(RadioId(2)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Cancel(RadioId(2)));

    assert_eq!(state.active, None, "Cancel should clear active");
    assert_eq!(
        state.selected,
        Some(RadioId(1)),
        "Cancel should not change selected"
    );
    assert_eq!(
        state.focused,
        Some(RadioId(2)),
        "Cancel should not change focused"
    );
    assert_eq!(result, None, "Cancel should not emit");
}

#[test]
fn cancel_no_effect_for_non_matching_member() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: None,
        active: Some(RadioId(2)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Cancel(RadioId(3)));

    assert_eq!(
        state.active,
        Some(RadioId(2)),
        "Cancel should not affect active for non-matching member"
    );
    assert_eq!(result, None, "Cancel should not emit");
}

#[test]
fn cancel_never_toggles_selected() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: None,
        active: Some(RadioId(1)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Cancel(RadioId(1)));

    assert_eq!(
        state.selected,
        Some(RadioId(1)),
        "Cancel should never change selected"
    );
    assert_eq!(result, None, "Cancel should not emit");
}

// ============================================================================
// Test: FocusGain event
// ============================================================================

#[test]
fn focus_gain_sets_focused() {
    let mut state = default_state();

    let result = reduce_radio_group(&mut state, RadioGroupEvent::FocusGain(RadioId(1)));

    assert_eq!(
        state.focused,
        Some(RadioId(1)),
        "FocusGain should set focused"
    );
    assert_eq!(result, None, "FocusGain should not emit");
}

#[test]
fn focus_gain_transfers_focus() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(1)),
        active: None,
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::FocusGain(RadioId(2)));

    assert_eq!(
        state.focused,
        Some(RadioId(2)),
        "FocusGain should transfer focus"
    );
    assert_eq!(result, None, "FocusGain should not emit");
}

#[test]
fn focus_gain_preserves_selected_and_active() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: None,
        active: Some(RadioId(2)),
    };

    reduce_radio_group(&mut state, RadioGroupEvent::FocusGain(RadioId(3)));

    assert_eq!(
        state.selected,
        Some(RadioId(1)),
        "FocusGain should not change selected"
    );
    assert_eq!(
        state.active,
        Some(RadioId(2)),
        "FocusGain should not change active"
    );
}

#[test]
fn focus_gain_never_toggles_selected() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: None,
        active: None,
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::FocusGain(RadioId(1)));

    assert_eq!(
        state.selected, None,
        "FocusGain should never change selected"
    );
    assert_eq!(result, None, "FocusGain should not emit");
}

// ============================================================================
// Test: FocusLoss event
// ============================================================================

#[test]
fn focus_loss_clears_focused_for_matching_member() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(1)),
        active: None,
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::FocusLoss(RadioId(1)));

    assert_eq!(state.focused, None, "FocusLoss should clear focused");
    assert_eq!(result, None, "FocusLoss should not emit");
}

#[test]
fn focus_loss_clears_active_if_losing_member_was_armed() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: Some(RadioId(2)),
        active: Some(RadioId(2)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::FocusLoss(RadioId(2)));

    assert_eq!(state.focused, None, "FocusLoss should clear focused");
    assert_eq!(
        state.active, None,
        "FocusLoss should clear active if losing member was armed"
    );
    assert_eq!(result, None, "FocusLoss should not emit");
}

#[test]
fn focus_loss_preserves_active_for_different_member() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(1)),
        active: Some(RadioId(2)),
    };

    reduce_radio_group(&mut state, RadioGroupEvent::FocusLoss(RadioId(1)));

    assert_eq!(
        state.active,
        Some(RadioId(2)),
        "FocusLoss should preserve active for different member"
    );
}

#[test]
fn focus_loss_no_effect_for_non_matching_member() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(1)),
        active: Some(RadioId(1)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::FocusLoss(RadioId(2)));

    assert_eq!(
        state.focused,
        Some(RadioId(1)),
        "FocusLoss should not affect focused for non-matching member"
    );
    assert_eq!(
        state.active,
        Some(RadioId(1)),
        "FocusLoss should not affect active for non-matching member"
    );
    assert_eq!(result, None, "FocusLoss should not emit");
}

#[test]
fn focus_loss_never_toggles_selected() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: Some(RadioId(1)),
        active: None,
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::FocusLoss(RadioId(1)));

    assert_eq!(
        state.selected,
        Some(RadioId(1)),
        "FocusLoss should never change selected"
    );
    assert_eq!(result, None, "FocusLoss should not emit");
}

// ============================================================================
// Test: Disabled behavior
// ============================================================================

#[test]
fn disabled_ignores_select() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: true,
        focused: None,
        active: None,
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Select(RadioId(1)));

    assert_eq!(state.selected, None, "Disabled: Select should be no-op");
    assert_eq!(result, None, "Disabled: Select should not emit");
}

#[test]
fn disabled_ignores_activate() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: true,
        focused: None,
        active: Some(RadioId(1)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Activate(RadioId(1)));

    assert_eq!(state.selected, None, "Disabled: Activate should be no-op");
    assert_eq!(
        state.active,
        Some(RadioId(1)),
        "Disabled: Activate should not clear active"
    );
    assert_eq!(result, None, "Disabled: Activate should not emit");
}

#[test]
fn disabled_ignores_cancel() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: true,
        focused: None,
        active: Some(RadioId(1)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Cancel(RadioId(1)));

    assert_eq!(
        state.active,
        Some(RadioId(1)),
        "Disabled: Cancel should be no-op"
    );
    assert_eq!(result, None, "Disabled: Cancel should not emit");
}

#[test]
fn disabled_ignores_focus_gain() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: true,
        focused: None,
        active: None,
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::FocusGain(RadioId(1)));

    assert_eq!(state.focused, None, "Disabled: FocusGain should be no-op");
    assert_eq!(result, None, "Disabled: FocusGain should not emit");
}

#[test]
fn disabled_ignores_focus_loss() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: true,
        focused: Some(RadioId(1)),
        active: Some(RadioId(1)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::FocusLoss(RadioId(1)));

    assert_eq!(
        state.focused,
        Some(RadioId(1)),
        "Disabled: FocusLoss should be no-op"
    );
    assert_eq!(
        state.active,
        Some(RadioId(1)),
        "Disabled: FocusLoss should not clear active"
    );
    assert_eq!(result, None, "Disabled: FocusLoss should not emit");
}

#[test]
fn disabled_preserves_selected() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: true,
        focused: None,
        active: None,
    };

    reduce_radio_group(&mut state, RadioGroupEvent::Select(RadioId(2)));

    assert_eq!(
        state.selected,
        Some(RadioId(1)),
        "Disabled: selected should be preserved (INV-G6)"
    );
}

// ============================================================================
// Test: Emission only on selected delta
// ============================================================================

#[test]
fn emission_only_on_selected_delta_select() {
    let mut state = default_state();

    // First select: should emit
    let result1 = reduce_radio_group(&mut state, RadioGroupEvent::Select(RadioId(1)));
    assert!(result1.is_some(), "Select changing selected should emit");

    // Same select: should not emit
    let result2 = reduce_radio_group(&mut state, RadioGroupEvent::Select(RadioId(1)));
    assert!(
        result2.is_none(),
        "Select not changing selected should not emit"
    );

    // Different select: should emit
    let result3 = reduce_radio_group(&mut state, RadioGroupEvent::Select(RadioId(2)));
    assert!(result3.is_some(), "Select changing selected should emit");
}

#[test]
fn emission_only_on_selected_delta_activate() {
    let mut state = default_state();

    // First activate: should emit
    let result1 = reduce_radio_group(&mut state, RadioGroupEvent::Activate(RadioId(1)));
    assert!(result1.is_some(), "Activate changing selected should emit");

    // Same activate: should not emit
    let result2 = reduce_radio_group(&mut state, RadioGroupEvent::Activate(RadioId(1)));
    assert!(
        result2.is_none(),
        "Activate not changing selected should not emit"
    );
}

#[test]
fn no_emission_on_focus_or_cancel() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: None,
        active: Some(RadioId(1)),
    };

    let r1 = reduce_radio_group(&mut state, RadioGroupEvent::FocusGain(RadioId(1)));
    assert_eq!(r1, None, "FocusGain should never emit");

    let r2 = reduce_radio_group(&mut state, RadioGroupEvent::Cancel(RadioId(1)));
    assert_eq!(r2, None, "Cancel should never emit");

    state.focused = Some(RadioId(1));
    let r3 = reduce_radio_group(&mut state, RadioGroupEvent::FocusLoss(RadioId(1)));
    assert_eq!(r3, None, "FocusLoss should never emit");
}

// ============================================================================
// Test: Active never survives Cancel or FocusLoss
// ============================================================================

#[test]
fn active_cleared_by_cancel_matching() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(1)),
        active: Some(RadioId(1)),
    };

    reduce_radio_group(&mut state, RadioGroupEvent::Cancel(RadioId(1)));

    assert_eq!(
        state.active, None,
        "Cancel should clear active for matching member"
    );
}

#[test]
fn active_cleared_by_focus_loss_matching() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(1)),
        active: Some(RadioId(1)),
    };

    reduce_radio_group(&mut state, RadioGroupEvent::FocusLoss(RadioId(1)));

    assert_eq!(
        state.active, None,
        "FocusLoss should clear active for matching member"
    );
}

#[test]
fn active_cleared_by_activate() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: None,
        active: Some(RadioId(1)),
    };

    reduce_radio_group(&mut state, RadioGroupEvent::Activate(RadioId(1)));

    assert_eq!(state.active, None, "Activate should clear active");
}

// ============================================================================
// Test: Activate commits + clears active atomically
// ============================================================================

#[test]
fn activate_commits_and_clears_active_atomically() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(1)),
        active: Some(RadioId(1)),
    };

    let result = reduce_radio_group(&mut state, RadioGroupEvent::Activate(RadioId(1)));

    assert_eq!(
        state.selected,
        Some(RadioId(1)),
        "Activate should commit selection"
    );
    assert_eq!(state.active, None, "Activate should clear active");
    assert_eq!(
        result,
        Some(Some(RadioId(1))),
        "Activate should emit new selection"
    );
}

// ============================================================================
// Test: State shape preserves mutual exclusion (at-most-one)
// ============================================================================

#[test]
fn focus_gain_implicitly_replaces_previous_focus() {
    let mut state = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(1)),
        active: None,
    };

    reduce_radio_group(&mut state, RadioGroupEvent::FocusGain(RadioId(2)));

    // By state shape, focused can only hold one value
    assert_eq!(
        state.focused,
        Some(RadioId(2)),
        "Focus should be transferred to new member"
    );
}

#[test]
fn select_implicitly_replaces_previous_selection() {
    let mut state = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: None,
        active: None,
    };

    reduce_radio_group(&mut state, RadioGroupEvent::Select(RadioId(2)));

    // By state shape, selected can only hold one value (mutual exclusion)
    assert_eq!(
        state.selected,
        Some(RadioId(2)),
        "Selection should be transferred to new member"
    );
}
