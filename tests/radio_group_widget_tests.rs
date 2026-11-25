// SPEC-HAL-130/131 Stage 2: RadioGroup Widget Tests
// Tests for the widget shell (not reducer — see radio_group_reducer_tests.rs).

use halcyon::core::event::KeyboardAuthorityMode;
use halcyon::core::layout::Constraints;
use halcyon::core::math::RectF;
use halcyon::core::rect_authority::PaintAuthorityGuard;
use halcyon::core::scene::SceneBuilder;
use halcyon::core::theme::Theme;
use halcyon::core::widget::{LayoutContext, PaintContext, Widget};
use halcyon::core::widget_tree::{OverlayHitRegion, WidgetTree};
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::utils::id::TreeWidgetId;
use halcyon::widgets::radio_group::{RadioGroupEvent, RadioId};
use halcyon::widgets::radio_group_widget::RadioGroup;
use std::collections::HashMap;

// ============================================================================
// Test 1: Initial state correct
// ============================================================================

#[test]
fn initial_state_correct() {
    let group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);

    let state = group.state();
    assert_eq!(state.selected, None, "Initial selection should be None");
    assert!(!state.disabled, "Initial disabled should be false");
    assert_eq!(state.focused, None, "Initial focused should be None");
    assert_eq!(state.active, None, "Initial active should be None");
}

#[test]
fn initial_members_stored() {
    let group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);

    assert_eq!(group.members().len(), 3, "Should store 3 members");
    assert_eq!(group.members()[0], RadioId(1));
    assert_eq!(group.members()[1], RadioId(2));
    assert_eq!(group.members()[2], RadioId(3));
}

#[test]
fn initial_state_empty_group() {
    let group = RadioGroup::new(&[]);

    assert_eq!(
        group.members().len(),
        0,
        "Empty group should have no members"
    );
    assert_eq!(group.state().selected, None);
}

#[test]
fn layout_includes_hairline_gap_between_options() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let mut tree = WidgetTree::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();
    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);

    let result = group.layout(
        &mut cx,
        Constraints {
            min_width: 0.0,
            max_width: 160.0,
            min_height: 0.0,
            max_height: 200.0,
        },
        &[],
    );

    let option_h = theme.geometry.checkbox.size;
    assert_eq!(result.size.height, option_h * 3.0 + 2.0);
}

#[test]
fn labeled_group_uses_a_finite_natural_width_when_row_measurement_is_unbounded() {
    let mut group = RadioGroup::with_labeled_options(
        halcyon::widgets::radio_group_widget::RadioGroupId(356),
        &[
            (RadioId(101), "Far unlit quad"),
            (RadioId(102), "Textured vertex-color triangle"),
            (RadioId(103), "Alpha-mask quad"),
            (RadioId(104), "Backface probe"),
        ],
    );
    let mut tree = WidgetTree::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();
    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);

    let result = group.layout(&mut cx, Constraints::unbounded(), &[]);

    assert!(
        result.size.width.is_finite() && result.size.width > theme.geometry.checkbox.size,
        "a labeled RadioGroup measured by Flex::row must retain a finite natural hit-test width"
    );
}

// ============================================================================
// Test 2: Select / Activate via on_semantic_event updates group state
// ============================================================================

#[test]
fn select_updates_group_state() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);

    group.on_semantic_event(RadioGroupEvent::Select(RadioId(2)));

    assert_eq!(
        group.state().selected,
        Some(RadioId(2)),
        "Select should update selected"
    );
}

#[test]
fn activate_updates_group_state() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);

    group.on_semantic_event(RadioGroupEvent::Activate(RadioId(1)));

    assert_eq!(
        group.state().selected,
        Some(RadioId(1)),
        "Activate should update selected"
    );
}

#[test]
fn focus_gain_updates_group_state() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);

    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(2)));

    assert_eq!(
        group.state().focused,
        Some(RadioId(2)),
        "FocusGain should update focused"
    );
}

#[test]
fn cancel_clears_active() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);

    // Set up active state via internal state manipulation for testing
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(1)));

    // Note: active is set by input mapping (not implemented here)
    // For this test, we verify Cancel behavior through the reducer
    group.on_semantic_event(RadioGroupEvent::Cancel(RadioId(1)));

    // Cancel clears active only if it matches, but active was None
    assert_eq!(group.state().active, None, "Active should remain None");
}

// ============================================================================
// Test 3: Emission only on selected delta
// ============================================================================

#[test]
fn emission_on_select_change() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);

    // First select: should emit
    let result1 = group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    assert!(result1.is_some(), "First select should emit");
    assert_eq!(result1, Some(Some(RadioId(1))));

    // Same select: should not emit
    let result2 = group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    assert!(result2.is_none(), "Same select should not emit");

    // Different select: should emit
    let result3 = group.on_semantic_event(RadioGroupEvent::Select(RadioId(2)));
    assert!(result3.is_some(), "Different select should emit");
    assert_eq!(result3, Some(Some(RadioId(2))));
}

#[test]
fn emission_on_activate_change() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);

    // First activate: should emit
    let result1 = group.on_semantic_event(RadioGroupEvent::Activate(RadioId(1)));
    assert!(result1.is_some(), "First activate should emit");

    // Same activate: should not emit
    let result2 = group.on_semantic_event(RadioGroupEvent::Activate(RadioId(1)));
    assert!(result2.is_none(), "Same activate should not emit");
}

#[test]
fn no_emission_on_focus_events() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);

    let r1 = group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(1)));
    assert!(r1.is_none(), "FocusGain should not emit");

    let r2 = group.on_semantic_event(RadioGroupEvent::FocusLoss(RadioId(1)));
    assert!(r2.is_none(), "FocusLoss should not emit");

    let r3 = group.on_semantic_event(RadioGroupEvent::Cancel(RadioId(1)));
    assert!(r3.is_none(), "Cancel should not emit");
}

// ============================================================================
// Test 4: All members repaint from same state
// ============================================================================

#[test]
fn all_members_share_state() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);

    // Select member 2
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(2)));

    // All buttons should project from the same state
    let state = group.state();
    assert_eq!(state.selected, Some(RadioId(2)));

    // Verify state is consistent (members.len() == buttons internally)
    assert_eq!(group.members().len(), 3);
}

#[test]
fn paint_uses_shared_state() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);

    // Set up state
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(2)));
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(1)));

    let tree = WidgetTree::new();
    let mut scene = SceneBuilder::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects: HashMap<TreeWidgetId, RectF> = HashMap::new();
    let mut overlay_hit_regions: Vec<OverlayHitRegion> = Vec::new();
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
    let rect = RectF::new(0.0, 0.0, 200.0, 60.0);

    // Paint should complete without panic
    let _paint_authority = PaintAuthorityGuard::enter();
    group.paint(&mut cx, rect, &[]);

    // State should be unchanged after paint
    assert_eq!(group.state().selected, Some(RadioId(2)));
    assert_eq!(group.state().focused, Some(RadioId(1)));
}

// ============================================================================
// Test 5: Paint does not mutate state
// ============================================================================

#[test]
fn paint_does_not_mutate_state() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);

    // Set up some state
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(2)));

    let state_before = group.state().clone();

    let tree = WidgetTree::new();
    let mut scene = SceneBuilder::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects: HashMap<TreeWidgetId, RectF> = HashMap::new();
    let mut overlay_hit_regions: Vec<OverlayHitRegion> = Vec::new();
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
    let rect = RectF::new(0.0, 0.0, 200.0, 60.0);

    let _paint_authority = PaintAuthorityGuard::enter();
    group.paint(&mut cx, rect, &[]);

    assert_eq!(
        group.state(),
        &state_before,
        "Paint should not mutate state"
    );
}

#[test]
fn paint_does_not_mutate_members() {
    let group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);

    let members_before = group.members().to_vec();

    let tree = WidgetTree::new();
    let mut scene = SceneBuilder::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects: HashMap<TreeWidgetId, RectF> = HashMap::new();
    let mut overlay_hit_regions: Vec<OverlayHitRegion> = Vec::new();
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
    let rect = RectF::new(0.0, 0.0, 200.0, 60.0);

    let _paint_authority = PaintAuthorityGuard::enter();
    group.paint(&mut cx, rect, &[]);

    assert_eq!(
        group.members(),
        &members_before[..],
        "Paint should not mutate members"
    );
}

// ============================================================================
// Test 6: Disabled group blocks all transitions
// ============================================================================

#[test]
fn disabled_group_blocks_select() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);
    group.set_disabled(true);

    let result = group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));

    assert!(result.is_none(), "Disabled: Select should not emit");
    assert_eq!(
        group.state().selected,
        None,
        "Disabled: selected should not change"
    );
}

#[test]
fn disabled_group_blocks_activate() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);
    group.set_disabled(true);

    let result = group.on_semantic_event(RadioGroupEvent::Activate(RadioId(1)));

    assert!(result.is_none(), "Disabled: Activate should not emit");
    assert_eq!(
        group.state().selected,
        None,
        "Disabled: selected should not change"
    );
}

#[test]
fn disabled_group_blocks_focus_gain() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);
    group.set_disabled(true);

    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(1)));

    assert_eq!(
        group.state().focused,
        None,
        "Disabled: focused should not change"
    );
}

#[test]
fn disabled_group_preserves_selection() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);

    // Select first, then disable
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    assert_eq!(group.state().selected, Some(RadioId(1)));

    group.set_disabled(true);

    // Selection preserved
    assert_eq!(
        group.state().selected,
        Some(RadioId(1)),
        "Disabled should preserve selection"
    );

    // Cannot change selection while disabled
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(2)));
    assert_eq!(
        group.state().selected,
        Some(RadioId(1)),
        "Cannot change selection while disabled"
    );
}

#[test]
fn is_disabled_reflects_state() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);

    assert!(!group.is_disabled(), "Initial: should not be disabled");

    group.set_disabled(true);
    assert!(
        group.is_disabled(),
        "After set_disabled(true): should be disabled"
    );

    group.set_disabled(false);
    assert!(
        !group.is_disabled(),
        "After set_disabled(false): should not be disabled"
    );
}

// ============================================================================
// Additional: Helper methods
// ============================================================================

#[test]
fn selected_helper() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);

    assert_eq!(group.selected(), None, "Initial: selected() should be None");

    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    assert_eq!(
        group.selected(),
        Some(RadioId(1)),
        "After select: selected() should return RadioId(1)"
    );
}

#[test]
fn set_selected_bypasses_reducer() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);

    // set_selected is for initialization, bypasses reducer
    group.set_selected(Some(RadioId(2)));

    assert_eq!(
        group.state().selected,
        Some(RadioId(2)),
        "set_selected should directly set selection"
    );
}
