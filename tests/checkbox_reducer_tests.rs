// SPEC-HAL-111 Checkbox Reducer Tests
// Unit tests for the pure semantic core.

use halcyon::widgets::checkbox::{CheckboxEvent, CheckboxState, reduce_checkbox};

fn default_state() -> CheckboxState {
    CheckboxState {
        checked: false,
        disabled: false,
        focused: false,
        active: false,
    }
}

// ============================================================================
// Test 1: Activate toggles once
// ============================================================================

#[test]
fn activate_toggles_checked_false_to_true() {
    let mut state = default_state();
    assert!(!state.checked);

    let result = reduce_checkbox(&mut state, CheckboxEvent::Activate);

    assert!(state.checked, "Activate should toggle checked to true");
    assert_eq!(result, Some(true), "Should emit new checked value");
}

#[test]
fn activate_toggles_checked_true_to_false() {
    let mut state = CheckboxState {
        checked: true,
        disabled: false,
        focused: false,
        active: false,
    };

    let result = reduce_checkbox(&mut state, CheckboxEvent::Activate);

    assert!(!state.checked, "Activate should toggle checked to false");
    assert_eq!(result, Some(false), "Should emit new checked value");
}

// ============================================================================
// Test 2: Activate emits exactly once
// ============================================================================

#[test]
fn activate_emits_exactly_once() {
    let mut state = default_state();

    let result = reduce_checkbox(&mut state, CheckboxEvent::Activate);
    assert_eq!(result, Some(true), "First Activate should emit");

    let result2 = reduce_checkbox(&mut state, CheckboxEvent::Activate);
    assert_eq!(
        result2,
        Some(false),
        "Second Activate should emit (toggled back)"
    );
}

// ============================================================================
// Test 3: Cancel never toggles
// ============================================================================

#[test]
fn cancel_never_toggles_when_unchecked() {
    let mut state = CheckboxState {
        checked: false,
        disabled: false,
        focused: false,
        active: true,
    };

    let result = reduce_checkbox(&mut state, CheckboxEvent::Cancel);

    assert!(!state.checked, "Cancel should never toggle checked");
    assert_eq!(result, None, "Cancel should not emit");
}

#[test]
fn cancel_never_toggles_when_checked() {
    let mut state = CheckboxState {
        checked: true,
        disabled: false,
        focused: false,
        active: true,
    };

    let result = reduce_checkbox(&mut state, CheckboxEvent::Cancel);

    assert!(state.checked, "Cancel should never toggle checked");
    assert_eq!(result, None, "Cancel should not emit");
}

// ============================================================================
// Test 4: FocusLoss clears active
// ============================================================================

#[test]
fn focus_loss_clears_active() {
    let mut state = CheckboxState {
        checked: false,
        disabled: false,
        focused: true,
        active: true,
    };

    let result = reduce_checkbox(&mut state, CheckboxEvent::FocusLoss);

    assert!(!state.active, "FocusLoss should clear active");
    assert!(!state.focused, "FocusLoss should clear focused");
    assert_eq!(result, None, "FocusLoss should not emit");
}

// ============================================================================
// Test 5: Disabled ignores all events
// ============================================================================

#[test]
fn disabled_ignores_activate() {
    let mut state = CheckboxState {
        checked: false,
        disabled: true,
        focused: false,
        active: false,
    };

    let result = reduce_checkbox(&mut state, CheckboxEvent::Activate);

    assert!(!state.checked, "Disabled: Activate should be no-op");
    assert_eq!(result, None, "Disabled: Activate should not emit");
}

#[test]
fn disabled_ignores_cancel() {
    let mut state = CheckboxState {
        checked: false,
        disabled: true,
        focused: false,
        active: true,
    };
    let active_before = state.active;

    let result = reduce_checkbox(&mut state, CheckboxEvent::Cancel);

    assert_eq!(
        state.active, active_before,
        "Disabled: Cancel should be no-op"
    );
    assert_eq!(result, None, "Disabled: Cancel should not emit");
}

#[test]
fn disabled_ignores_focus_gain() {
    let mut state = CheckboxState {
        checked: false,
        disabled: true,
        focused: false,
        active: false,
    };

    let result = reduce_checkbox(&mut state, CheckboxEvent::FocusGain);

    assert!(!state.focused, "Disabled: FocusGain should be no-op");
    assert_eq!(result, None, "Disabled: FocusGain should not emit");
}

#[test]
fn disabled_ignores_focus_loss() {
    let mut state = CheckboxState {
        checked: false,
        disabled: true,
        focused: true,
        active: true,
    };
    let focused_before = state.focused;
    let active_before = state.active;

    let result = reduce_checkbox(&mut state, CheckboxEvent::FocusLoss);

    assert_eq!(
        state.focused, focused_before,
        "Disabled: FocusLoss should be no-op"
    );
    assert_eq!(
        state.active, active_before,
        "Disabled: FocusLoss should be no-op"
    );
    assert_eq!(result, None, "Disabled: FocusLoss should not emit");
}

// ============================================================================
// Test 6: Focus change never toggles
// ============================================================================

#[test]
fn focus_gain_never_toggles() {
    let mut state = CheckboxState {
        checked: false,
        disabled: false,
        focused: false,
        active: false,
    };

    let result = reduce_checkbox(&mut state, CheckboxEvent::FocusGain);

    assert!(!state.checked, "FocusGain should never toggle checked");
    assert!(state.focused, "FocusGain should set focused = true");
    assert_eq!(result, None, "FocusGain should not emit");
}

#[test]
fn focus_loss_never_toggles() {
    let mut state = CheckboxState {
        checked: true,
        disabled: false,
        focused: true,
        active: false,
    };

    let result = reduce_checkbox(&mut state, CheckboxEvent::FocusLoss);

    assert!(state.checked, "FocusLoss should never toggle checked");
    assert!(!state.focused, "FocusLoss should set focused = false");
    assert_eq!(result, None, "FocusLoss should not emit");
}

// ============================================================================
// Test 7: Active never survives Cancel or FocusLoss
// ============================================================================

#[test]
fn active_cleared_by_cancel() {
    let mut state = CheckboxState {
        checked: false,
        disabled: false,
        focused: true,
        active: true,
    };

    reduce_checkbox(&mut state, CheckboxEvent::Cancel);

    assert!(!state.active, "Cancel should clear active");
}

#[test]
fn active_cleared_by_focus_loss() {
    let mut state = CheckboxState {
        checked: false,
        disabled: false,
        focused: true,
        active: true,
    };

    reduce_checkbox(&mut state, CheckboxEvent::FocusLoss);

    assert!(!state.active, "FocusLoss should clear active");
}

// ============================================================================
// Additional invariant tests: Activate clears active
// ============================================================================

#[test]
fn activate_clears_active() {
    let mut state = CheckboxState {
        checked: false,
        disabled: false,
        focused: true,
        active: true,
    };

    reduce_checkbox(&mut state, CheckboxEvent::Activate);

    assert!(!state.active, "Activate should clear active");
}

// ============================================================================
// Additional invariant tests: Activate does not affect focused
// ============================================================================

#[test]
fn activate_does_not_affect_focused_when_true() {
    let mut state = CheckboxState {
        checked: false,
        disabled: false,
        focused: true,
        active: false,
    };

    reduce_checkbox(&mut state, CheckboxEvent::Activate);

    assert!(state.focused, "Activate should not affect focused");
}

#[test]
fn activate_does_not_affect_focused_when_false() {
    let mut state = CheckboxState {
        checked: false,
        disabled: false,
        focused: false,
        active: false,
    };

    reduce_checkbox(&mut state, CheckboxEvent::Activate);

    assert!(!state.focused, "Activate should not affect focused");
}
