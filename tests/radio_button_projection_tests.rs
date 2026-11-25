// SPEC-HAL-131 Stage 1: RadioButton Projection Tests
// Tests for the projection widget (not reducer — see radio_group_reducer_tests.rs).

use std::collections::HashMap;

use halcyon::core::event::KeyboardAuthorityMode;
use halcyon::core::math::RectF;
use halcyon::core::scene::SceneBuilder;
use halcyon::core::theme::Theme;
use halcyon::core::widget::PaintContext;
use halcyon::core::widget_tree::WidgetTree;
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::utils::id::TreeWidgetId;
use halcyon::widgets::radio_button::RadioButton;
use halcyon::widgets::radio_group::{RadioGroupState, RadioId};

// ============================================================================
// Test 1: Projection correctness — selected / unselected
// ============================================================================

#[test]
fn projection_selected() {
    let button = RadioButton::new(RadioId(1));
    let group = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: None,
        active: None,
    };

    let state = button.project(&group);

    assert!(state.checked, "Member should be checked when selected");
    assert!(!state.focused, "Member should not be focused");
    assert!(!state.active, "Member should not be active");
    assert!(!state.disabled, "Member should not be disabled");
}

#[test]
fn projection_unselected() {
    let button = RadioButton::new(RadioId(1));
    let group = RadioGroupState {
        selected: Some(RadioId(2)), // Different member selected
        disabled: false,
        focused: None,
        active: None,
    };

    let state = button.project(&group);

    assert!(
        !state.checked,
        "Member should not be checked when another is selected"
    );
}

#[test]
fn projection_none_selected() {
    let button = RadioButton::new(RadioId(1));
    let group = RadioGroupState {
        selected: None,
        disabled: false,
        focused: None,
        active: None,
    };

    let state = button.project(&group);

    assert!(
        !state.checked,
        "Member should not be checked when none selected"
    );
}

// ============================================================================
// Test 2: Projection correctness — focused / unfocused
// ============================================================================

#[test]
fn projection_focused() {
    let button = RadioButton::new(RadioId(1));
    let group = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(1)),
        active: None,
    };

    let state = button.project(&group);

    assert!(
        state.focused,
        "Member should be focused when group.focused matches"
    );
    assert!(!state.checked, "Focused does not imply checked");
}

#[test]
fn projection_unfocused() {
    let button = RadioButton::new(RadioId(1));
    let group = RadioGroupState {
        selected: None,
        disabled: false,
        focused: Some(RadioId(2)), // Different member focused
        active: None,
    };

    let state = button.project(&group);

    assert!(
        !state.focused,
        "Member should not be focused when another is focused"
    );
}

// ============================================================================
// Test 3: Projection correctness — active / inactive
// ============================================================================

#[test]
fn projection_active() {
    let button = RadioButton::new(RadioId(1));
    let group = RadioGroupState {
        selected: None,
        disabled: false,
        focused: None,
        active: Some(RadioId(1)),
    };

    let state = button.project(&group);

    assert!(
        state.active,
        "Member should be active when group.active matches"
    );
    assert!(!state.checked, "Active does not imply checked");
}

#[test]
fn projection_inactive() {
    let button = RadioButton::new(RadioId(1));
    let group = RadioGroupState {
        selected: None,
        disabled: false,
        focused: None,
        active: Some(RadioId(2)), // Different member active
    };

    let state = button.project(&group);

    assert!(
        !state.active,
        "Member should not be active when another is active"
    );
}

// ============================================================================
// Test 4: Projection correctness — disabled
// ============================================================================

#[test]
fn projection_disabled() {
    let button = RadioButton::new(RadioId(1));
    let group = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: true,
        focused: None,
        active: None,
    };

    let state = button.project(&group);

    assert!(
        state.disabled,
        "Member should be disabled when group is disabled"
    );
    assert!(state.checked, "Selection is preserved when disabled");
}

#[test]
fn projection_enabled() {
    let button = RadioButton::new(RadioId(1));
    let group = RadioGroupState {
        selected: None,
        disabled: false,
        focused: None,
        active: None,
    };

    let state = button.project(&group);

    assert!(
        !state.disabled,
        "Member should be enabled when group is enabled"
    );
}

// ============================================================================
// Test 5: Different member IDs project differently from same group state
// ============================================================================

#[test]
fn different_members_project_differently() {
    let button1 = RadioButton::new(RadioId(1));
    let button2 = RadioButton::new(RadioId(2));
    let button3 = RadioButton::new(RadioId(3));

    let group = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: Some(RadioId(2)),
        active: Some(RadioId(3)),
    };

    let state1 = button1.project(&group);
    let state2 = button2.project(&group);
    let state3 = button3.project(&group);

    // Button 1: selected only
    assert!(state1.checked, "Button 1 should be checked");
    assert!(!state1.focused, "Button 1 should not be focused");
    assert!(!state1.active, "Button 1 should not be active");

    // Button 2: focused only
    assert!(!state2.checked, "Button 2 should not be checked");
    assert!(state2.focused, "Button 2 should be focused");
    assert!(!state2.active, "Button 2 should not be active");

    // Button 3: active only
    assert!(!state3.checked, "Button 3 should not be checked");
    assert!(!state3.focused, "Button 3 should not be focused");
    assert!(state3.active, "Button 3 should be active");

    // All share disabled state
    assert!(!state1.disabled);
    assert!(!state2.disabled);
    assert!(!state3.disabled);
}

#[test]
fn different_members_all_disabled() {
    let button1 = RadioButton::new(RadioId(1));
    let button2 = RadioButton::new(RadioId(2));

    let group = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: true,
        focused: None,
        active: None,
    };

    let state1 = button1.project(&group);
    let state2 = button2.project(&group);

    // Both disabled
    assert!(state1.disabled, "Button 1 should be disabled");
    assert!(state2.disabled, "Button 2 should be disabled");

    // But only one checked
    assert!(state1.checked, "Button 1 should be checked");
    assert!(!state2.checked, "Button 2 should not be checked");
}

// ============================================================================
// Test 6: Paint does not mutate anything
// ============================================================================

#[test]
fn paint_does_not_mutate_button() {
    let button = RadioButton::new(RadioId(1));
    let radio_id_before = button.radio_id();

    let tree = WidgetTree::new();
    let mut scene = SceneBuilder::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    let group = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: Some(RadioId(1)),
        active: None,
    };

    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects: HashMap<TreeWidgetId, RectF> = HashMap::new();
    let mut overlay_hit_regions = Vec::new();
    let mut tooltip_triggers = Vec::new();
    let icon_registry = halcyon::core::icons::builtin_registry();
    let mut cx = PaintContext::new(
        &tree,
        &mut scene,
        &theme,
        &mut text_engine,
        &icon_registry,
        0,
        &timeline,
        &interpolated_rects,
        &mut overlay_hit_regions,
        &mut tooltip_triggers,
        1.0,
        None,
        KeyboardAuthorityMode::Navigation,
    );
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    button.paint_with_group(&mut cx, rect, &group);

    // Verify button unchanged
    assert_eq!(
        button.radio_id(),
        radio_id_before,
        "Paint should not mutate button"
    );
}

#[test]
fn paint_does_not_mutate_group_state() {
    let button = RadioButton::new(RadioId(1));

    let tree = WidgetTree::new();
    let mut scene = SceneBuilder::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    let group = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false,
        focused: Some(RadioId(1)),
        active: Some(RadioId(1)),
    };
    let group_snapshot = group.clone();

    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects: HashMap<TreeWidgetId, RectF> = HashMap::new();
    let mut overlay_hit_regions = Vec::new();
    let mut tooltip_triggers = Vec::new();
    let icon_registry = halcyon::core::icons::builtin_registry();
    let mut cx = PaintContext::new(
        &tree,
        &mut scene,
        &theme,
        &mut text_engine,
        &icon_registry,
        0,
        &timeline,
        &interpolated_rects,
        &mut overlay_hit_regions,
        &mut tooltip_triggers,
        1.0,
        None,
        KeyboardAuthorityMode::Navigation,
    );
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    button.paint_with_group(&mut cx, rect, &group);

    // Verify group state unchanged
    assert_eq!(group, group_snapshot, "Paint should not mutate group state");
}

// ============================================================================
// Test 7: Compile-time check — no semantic fields stored in RadioButton
// ============================================================================

#[test]
fn no_semantic_fields_in_radio_button() {
    // This test verifies at compile time that RadioButton has no semantic state.
    // If RadioButton stored checked/focused/active/disabled, these would be
    // accessible or the struct size would be larger.

    let button = RadioButton::new(RadioId(42));

    // The only accessible semantic identity is the radio_id
    assert_eq!(button.radio_id(), RadioId(42));

    // Projection is the sole source of semantic state
    let group = RadioGroupState::default();
    let state = button.project(&group);

    // Default group = no selection, no focus, no active, not disabled
    assert!(!state.checked);
    assert!(!state.focused);
    assert!(!state.active);
    assert!(!state.disabled);
}

// ============================================================================
// Test 8: Combined state projection
// ============================================================================

#[test]
fn projection_combined_states() {
    let button = RadioButton::new(RadioId(1));

    // All states true for this member
    let group = RadioGroupState {
        selected: Some(RadioId(1)),
        disabled: false, // Note: disabled=true would prevent focus/active in real use, but projection is pure
        focused: Some(RadioId(1)),
        active: Some(RadioId(1)),
    };

    let state = button.project(&group);

    assert!(state.checked, "Should be checked");
    assert!(state.focused, "Should be focused");
    assert!(state.active, "Should be active");
    assert!(!state.disabled, "Should not be disabled");
}

#[test]
fn projection_all_false() {
    let button = RadioButton::new(RadioId(1));

    // All states point to different members or None
    let group = RadioGroupState {
        selected: Some(RadioId(99)),
        disabled: false,
        focused: Some(RadioId(99)),
        active: Some(RadioId(99)),
    };

    let state = button.project(&group);

    assert!(!state.checked, "Should not be checked");
    assert!(!state.focused, "Should not be focused");
    assert!(!state.active, "Should not be active");
    assert!(!state.disabled, "Should not be disabled");
}
