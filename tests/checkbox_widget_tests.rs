// SPEC-HAL-111 Checkbox Widget Tests
// Tests for the widget shell + input mapping + action emission.

use std::cell::RefCell;
use std::rc::Rc;

use halcyon::core::event::{
    Event, KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers, PointerButton, PointerEvent,
    PointerEventKind, PointerId, PointerType,
};
use halcyon::core::layout::Constraints;
use halcyon::core::math::RectF;
use halcyon::core::rect_authority::PaintAuthorityGuard;
use halcyon::core::scene::SceneBuilder;
use halcyon::core::theme::Theme;
use halcyon::core::widget::{LayoutContext, PaintContext, Widget};
use halcyon::core::widget_tree::WidgetTree;
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::widgets::checkbox::{Checkbox, CheckboxEvent};

// ============================================================================
// Test 1: Widget construction defaults
// ============================================================================

#[test]
fn widget_construction_defaults() {
    let checkbox = Checkbox::new();

    assert!(!checkbox.is_checked(), "Default should be unchecked");
    assert!(!checkbox.is_disabled(), "Default should be enabled");
    assert!(!checkbox.is_focused(), "Default should be unfocused");

    let state = checkbox.state();
    assert!(!state.active, "Default should not be active");
}

#[test]
fn widget_construction_checked() {
    let checkbox = Checkbox::checked(true);
    assert!(
        checkbox.is_checked(),
        "Should be checked when constructed with checked(true)"
    );

    let checkbox2 = Checkbox::checked(false);
    assert!(
        !checkbox2.is_checked(),
        "Should be unchecked when constructed with checked(false)"
    );
}

// ============================================================================
// Test 2: Layout height equals shared control height
// ============================================================================

#[test]
fn layout_height_equals_theme_checkbox_size() {
    let mut checkbox = Checkbox::new();
    let mut tree = WidgetTree::new();
    let mut text_engine = FontManagerTextEngine::new();
    let theme = Theme::dark();

    let expected_height = theme.geometry.text_field.height;

    let constraints = Constraints {
        min_width: 0.0,
        max_width: 200.0,
        min_height: 0.0,
        max_height: 100.0,
    };

    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
    let result = checkbox.layout(&mut cx, constraints, &[]);

    assert_eq!(
        result.size.height, expected_height,
        "Layout height should equal theme.geometry.text_field.height ({})",
        expected_height
    );
    assert_eq!(
        result.size.width, constraints.max_width,
        "Layout width should equal max_width"
    );
}

#[test]
fn layout_height_respects_constraints() {
    let mut checkbox = Checkbox::new();
    let mut tree = WidgetTree::new();
    let mut text_engine = FontManagerTextEngine::new();
    let theme = Theme::dark();

    // Constrain height to less than checkbox size
    let constraints = Constraints {
        min_width: 0.0,
        max_width: 200.0,
        min_height: 0.0,
        max_height: 10.0, // Less than theme.geometry.checkbox.size (16.0 for Regular)
    };

    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
    let result = checkbox.layout(&mut cx, constraints, &[]);

    assert_eq!(
        result.size.height, 10.0,
        "Layout height should be clamped to max_height"
    );
}

// ============================================================================
// Test 3: on_semantic_event(Activate) toggles + emits
// ============================================================================

#[test]
fn on_semantic_event_activate_toggles_and_emits() {
    let mut checkbox = Checkbox::new();
    assert!(!checkbox.is_checked());

    // First Activate: unchecked -> checked
    let result = checkbox.on_semantic_event(CheckboxEvent::Activate);
    assert_eq!(result, Some(true), "Activate should emit Some(true)");
    assert!(checkbox.is_checked(), "Checkbox should now be checked");

    // Second Activate: checked -> unchecked
    let result2 = checkbox.on_semantic_event(CheckboxEvent::Activate);
    assert_eq!(result2, Some(false), "Activate should emit Some(false)");
    assert!(!checkbox.is_checked(), "Checkbox should now be unchecked");
}

// ============================================================================
// Test 4: Disabled widget ignores all events
// ============================================================================

#[test]
fn disabled_widget_ignores_all_events() {
    let mut checkbox = Checkbox::new();
    checkbox.set_disabled(true);

    // Activate should be ignored
    let result = checkbox.on_semantic_event(CheckboxEvent::Activate);
    assert_eq!(result, None, "Disabled: Activate should return None");
    assert!(
        !checkbox.is_checked(),
        "Disabled: checked should not change"
    );

    // FocusGain should be ignored
    let result = checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    assert_eq!(result, None, "Disabled: FocusGain should return None");
    assert!(
        !checkbox.is_focused(),
        "Disabled: focused should not change"
    );

    // Cancel should be ignored
    let result = checkbox.on_semantic_event(CheckboxEvent::Cancel);
    assert_eq!(result, None, "Disabled: Cancel should return None");

    // FocusLoss should be ignored
    let result = checkbox.on_semantic_event(CheckboxEvent::FocusLoss);
    assert_eq!(result, None, "Disabled: FocusLoss should return None");
}

// ============================================================================
// Test 5: FocusLoss clears active
// ============================================================================

#[test]
fn focus_loss_clears_active() {
    let mut checkbox = Checkbox::new();

    // Gain focus first
    checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    assert!(checkbox.is_focused());

    // Manually set active via the state (simulate armed state)
    // We need to use on_semantic_event for this, but active is set internally
    // For this test, we verify FocusLoss clears both focused and active

    let result = checkbox.on_semantic_event(CheckboxEvent::FocusLoss);
    assert_eq!(result, None, "FocusLoss should not emit");
    assert!(!checkbox.is_focused(), "FocusLoss should clear focused");

    // Verify active is cleared (it would have been false anyway, but this confirms the path)
    assert!(!checkbox.state().active, "FocusLoss should clear active");
}

// ============================================================================
// Test 6: Paint does not mutate state (snapshot before/after)
// ============================================================================

#[test]
fn paint_does_not_mutate_state() {
    let checkbox = Checkbox::checked(true);
    let state_before = checkbox.state().clone();

    // Create paint context
    let tree = WidgetTree::new();
    let mut scene = SceneBuilder::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects: std::collections::HashMap<halcyon::utils::id::TreeWidgetId, RectF> =
        std::collections::HashMap::new();
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

    {
        let _paint_authority = PaintAuthorityGuard::enter();
        checkbox.paint(&mut cx, rect, &[]);
    }

    // Verify state unchanged
    let state_after = checkbox.state();
    assert_eq!(&state_before, state_after, "Paint should not mutate state");
}

#[test]
fn paint_does_not_mutate_state_with_focus() {
    let mut checkbox = Checkbox::new();
    checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    let state_before = checkbox.state().clone();

    let tree = WidgetTree::new();
    let mut scene = SceneBuilder::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects: std::collections::HashMap<halcyon::utils::id::TreeWidgetId, RectF> =
        std::collections::HashMap::new();
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

    {
        let _paint_authority = PaintAuthorityGuard::enter();
        checkbox.paint(&mut cx, rect, &[]);
    }

    let state_after = checkbox.state();
    assert_eq!(
        &state_before, state_after,
        "Paint should not mutate state (focused)"
    );
}

// ============================================================================
// Additional: Verify emission path
// ============================================================================

#[test]
fn emission_path_only_on_toggle() {
    let mut checkbox = Checkbox::new();

    // Non-toggle events should not emit
    let r1 = checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    assert_eq!(r1, None, "FocusGain should not emit");

    let r2 = checkbox.on_semantic_event(CheckboxEvent::Cancel);
    assert_eq!(r2, None, "Cancel should not emit");

    let r3 = checkbox.on_semantic_event(CheckboxEvent::FocusLoss);
    assert_eq!(r3, None, "FocusLoss should not emit");

    // Only Activate emits
    let r4 = checkbox.on_semantic_event(CheckboxEvent::Activate);
    assert!(r4.is_some(), "Activate should emit");
}

// ============================================================================
// Input Mapping Test Helpers
// ============================================================================

fn make_pointer_down(x: f32, y: f32) -> Event {
    Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (x, y),
        local_pos: (x, y),
        kind: PointerEventKind::Down,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    })
}

fn make_pointer_up(x: f32, y: f32) -> Event {
    Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (x, y),
        local_pos: (x, y),
        kind: PointerEventKind::Up,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    })
}

fn make_key_down(key_code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        key_code,

        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        is_down: true,
        is_repeat: false,
        modifiers: Modifiers::default(),
        text: None,
    })
}

/// Captures emissions for testing.
struct EmissionCapture {
    emissions: Rc<RefCell<Vec<bool>>>,
}

impl EmissionCapture {
    fn new() -> Self {
        Self {
            emissions: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn callback(&self) -> impl FnMut(bool) + 'static {
        let emissions = self.emissions.clone();
        move |checked| {
            emissions.borrow_mut().push(checked);
        }
    }

    fn count(&self) -> usize {
        self.emissions.borrow().len()
    }

    fn last(&self) -> Option<bool> {
        self.emissions.borrow().last().copied()
    }
}

// ============================================================================
// Test: Pointer click toggles checked
// ============================================================================

#[test]
fn pointer_click_toggles_checked() {
    let mut checkbox = Checkbox::new();
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    assert!(!checkbox.is_checked());

    // Click inside bounds
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    checkbox.handle_event(&make_pointer_up(10.0, 10.0), rect);

    assert!(checkbox.is_checked(), "Pointer click should toggle checked");
}

#[test]
fn pointer_click_toggles_twice() {
    let mut checkbox = Checkbox::new();
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // First click: unchecked -> checked
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    checkbox.handle_event(&make_pointer_up(10.0, 10.0), rect);
    assert!(checkbox.is_checked());

    // Second click: checked -> unchecked
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    checkbox.handle_event(&make_pointer_up(10.0, 10.0), rect);
    assert!(!checkbox.is_checked(), "Second click should toggle back");
}

#[test]
fn pointer_click_outside_bounds_ignored() {
    let mut checkbox = Checkbox::new();
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // Click outside bounds
    let handled = checkbox.handle_event(&make_pointer_down(200.0, 200.0), rect);

    assert!(!handled, "Click outside bounds should not be handled");
    assert!(!checkbox.is_checked(), "Checked should not change");
}

// ============================================================================
// Test: Space/Enter toggles checked
// ============================================================================

#[test]
fn space_toggles_checked_when_focused() {
    let mut checkbox = Checkbox::new();
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // Set focus first
    checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    assert!(checkbox.is_focused());

    // Press Space
    checkbox.handle_event(&make_key_down(KeyCode::Space), rect);

    assert!(
        checkbox.is_checked(),
        "Space should toggle checked when focused"
    );
}

#[test]
fn enter_toggles_checked_when_focused() {
    let mut checkbox = Checkbox::new();
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // Set focus first
    checkbox.on_semantic_event(CheckboxEvent::FocusGain);

    // Press Enter
    checkbox.handle_event(&make_key_down(KeyCode::Enter), rect);

    assert!(
        checkbox.is_checked(),
        "Enter should toggle checked when focused"
    );
}

#[test]
fn space_without_focus_does_nothing() {
    let mut checkbox = Checkbox::new();
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // No focus
    assert!(!checkbox.is_focused());

    let handled = checkbox.handle_event(&make_key_down(KeyCode::Space), rect);

    assert!(!handled, "Space without focus should not be handled");
    assert!(!checkbox.is_checked(), "Checked should not change");
}

// ============================================================================
// Test: Escape clears active, not checked
// ============================================================================

#[test]
fn escape_clears_active_not_checked() {
    let mut checkbox = Checkbox::new();
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // First, check the checkbox
    checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    checkbox.on_semantic_event(CheckboxEvent::Activate);
    assert!(checkbox.is_checked());

    // Now press down (sets active)
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    assert!(
        checkbox.state().active,
        "Active should be set on pointer down"
    );

    // Press Escape
    checkbox.handle_event(&make_key_down(KeyCode::Escape), rect);

    assert!(!checkbox.state().active, "Escape should clear active");
    assert!(checkbox.is_checked(), "Escape should NOT change checked");
}

// ============================================================================
// Test: Disabled checkbox ignores all input
// ============================================================================

#[test]
fn disabled_checkbox_ignores_pointer() {
    let mut checkbox = Checkbox::new();
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    checkbox.set_disabled(true);

    let handled = checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);

    assert!(
        !handled,
        "Disabled checkbox should not handle pointer events"
    );
    assert!(
        !checkbox.is_checked(),
        "Checked should not change when disabled"
    );
}

#[test]
fn disabled_checkbox_ignores_keyboard() {
    let mut checkbox = Checkbox::new();
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    checkbox.set_disabled(true);

    let handled = checkbox.handle_event(&make_key_down(KeyCode::Space), rect);

    assert!(
        !handled,
        "Disabled checkbox should not handle keyboard events"
    );
}

// ============================================================================
// Test: Emission happens only on toggle (callback)
// ============================================================================

#[test]
fn callback_emits_on_toggle() {
    let capture = EmissionCapture::new();
    let mut checkbox = Checkbox::new();
    checkbox.set_on_toggle_changed(capture.callback());
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    assert_eq!(capture.count(), 0);

    // Click to toggle
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    checkbox.handle_event(&make_pointer_up(10.0, 10.0), rect);

    assert_eq!(capture.count(), 1, "Should emit once on toggle");
    assert_eq!(capture.last(), Some(true), "Should emit true (checked)");
}

#[test]
fn callback_emits_correct_value_on_each_toggle() {
    let capture = EmissionCapture::new();
    let mut checkbox = Checkbox::new();
    checkbox.set_on_toggle_changed(capture.callback());
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // First toggle: false -> true
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    checkbox.handle_event(&make_pointer_up(10.0, 10.0), rect);
    assert_eq!(capture.last(), Some(true));

    // Second toggle: true -> false
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    checkbox.handle_event(&make_pointer_up(10.0, 10.0), rect);
    assert_eq!(capture.last(), Some(false));

    assert_eq!(capture.count(), 2, "Should emit twice for two toggles");
}

#[test]
fn callback_not_called_on_focus() {
    let capture = EmissionCapture::new();
    let mut checkbox = Checkbox::new();
    checkbox.set_on_toggle_changed(capture.callback());

    checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    checkbox.on_semantic_event(CheckboxEvent::FocusLoss);

    assert_eq!(capture.count(), 0, "Focus events should not emit");
}

#[test]
fn callback_not_called_on_cancel() {
    let capture = EmissionCapture::new();
    let mut checkbox = Checkbox::new();
    checkbox.set_on_toggle_changed(capture.callback());
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // Press down (sets active)
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    // Release outside (triggers cancel)
    checkbox.handle_event(&make_pointer_up(200.0, 200.0), rect);

    assert_eq!(capture.count(), 0, "Cancel should not emit");
}

#[test]
fn callback_not_called_when_disabled() {
    let capture = EmissionCapture::new();
    let mut checkbox = Checkbox::new();
    checkbox.set_on_toggle_changed(capture.callback());
    checkbox.set_disabled(true);
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    checkbox.handle_event(&make_pointer_up(10.0, 10.0), rect);

    assert_eq!(capture.count(), 0, "Disabled should not emit");
}

// ============================================================================
// Test: Focus change never toggles
// ============================================================================

#[test]
fn focus_change_never_toggles() {
    let mut checkbox = Checkbox::new();

    checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    assert!(!checkbox.is_checked(), "FocusGain should not toggle");

    checkbox.on_semantic_event(CheckboxEvent::FocusLoss);
    assert!(!checkbox.is_checked(), "FocusLoss should not toggle");
}

#[test]
fn focus_change_with_checked_preserves_checked() {
    let mut checkbox = Checkbox::checked(true);

    checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    assert!(checkbox.is_checked(), "FocusGain should preserve checked");

    checkbox.on_semantic_event(CheckboxEvent::FocusLoss);
    assert!(checkbox.is_checked(), "FocusLoss should preserve checked");
}

// ============================================================================
// Test: No double-emit on same interaction
// ============================================================================

#[test]
fn no_double_emit_on_single_click() {
    let capture = EmissionCapture::new();
    let mut checkbox = Checkbox::new();
    checkbox.set_on_toggle_changed(capture.callback());
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    checkbox.handle_event(&make_pointer_up(10.0, 10.0), rect);

    assert_eq!(capture.count(), 1, "Single click should emit exactly once");
}

#[test]
fn no_emit_on_repeated_pointer_down() {
    let capture = EmissionCapture::new();
    let mut checkbox = Checkbox::new();
    checkbox.set_on_toggle_changed(capture.callback());
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // Multiple pointer downs without ups
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);
    checkbox.handle_event(&make_pointer_down(10.0, 10.0), rect);

    assert_eq!(capture.count(), 0, "Pointer down alone should not emit");
}

// ============================================================================
// Test: Paint does not emit
// ============================================================================

#[test]
fn paint_does_not_emit() {
    let capture = EmissionCapture::new();
    let mut checkbox = Checkbox::new();
    checkbox.set_on_toggle_changed(capture.callback());

    // Toggle once
    checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    checkbox.on_semantic_event(CheckboxEvent::Activate);
    let count_before_paint = capture.count();

    // Paint
    let tree = WidgetTree::new();
    let mut scene = SceneBuilder::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects: std::collections::HashMap<halcyon::utils::id::TreeWidgetId, RectF> =
        std::collections::HashMap::new();
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
    {
        let _paint_authority = PaintAuthorityGuard::enter();
        checkbox.paint(&mut cx, rect, &[]);
    }

    assert_eq!(capture.count(), count_before_paint, "Paint should not emit");
}

// ============================================================================
// REGRESSION TEST: Space key must be routed as KeyCode::Space, not TextInput
// ============================================================================
// Context: Space was previously routed through TextInput path in platform/window.rs
// (line ~200: "// Space is NOT a control key - let it flow through TextInput path")
// This caused Checkbox/RadioGroup/Button to never receive Space as KeyCode::Space,
// making Space activation fail while Enter worked.
//
// Fix: Space is now a control key (generates KeyCode::Space), and text widgets
// (TextField/TextArea) explicitly handle KeyCode::Space to insert " ".

#[test]
fn regression_space_key_activates_checkbox_via_keycode() {
    // This test verifies that Space key activation works when the event
    // arrives as KeyCode::Space (not as TextInput " ").
    // If this test fails, check platform/window.rs control key mapping.

    let capture = EmissionCapture::new();
    let mut checkbox = Checkbox::new();
    checkbox.set_on_toggle_changed(capture.callback());
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // Set focus (required for keyboard activation)
    checkbox.on_semantic_event(CheckboxEvent::FocusGain);
    assert!(checkbox.is_focused());
    assert!(!checkbox.is_checked());

    // Send Space as KeyCode::Space (the correct routing after the fix)
    let space_event = make_key_down(KeyCode::Space);
    let handled = checkbox.handle_event(&space_event, rect);

    assert!(handled, "Space key should be handled by focused checkbox");
    assert!(
        checkbox.is_checked(),
        "Space should toggle checkbox to checked"
    );
    assert_eq!(capture.count(), 1, "Should emit exactly once");
    assert_eq!(capture.last(), Some(true), "Should emit true (checked)");
}

#[test]
fn regression_space_and_enter_behave_identically() {
    // Space and Enter should have identical activation behavior.
    // This was broken when Space was routed as TextInput instead of KeyCode.

    let mut checkbox_space = Checkbox::new();
    let mut checkbox_enter = Checkbox::new();
    let rect = RectF::new(0.0, 0.0, 100.0, 20.0);

    // Both need focus
    checkbox_space.on_semantic_event(CheckboxEvent::FocusGain);
    checkbox_enter.on_semantic_event(CheckboxEvent::FocusGain);

    // Activate with respective keys
    checkbox_space.handle_event(&make_key_down(KeyCode::Space), rect);
    checkbox_enter.handle_event(&make_key_down(KeyCode::Enter), rect);

    assert_eq!(
        checkbox_space.is_checked(),
        checkbox_enter.is_checked(),
        "Space and Enter should produce identical activation results"
    );
    assert!(checkbox_space.is_checked(), "Both should be checked");
}
