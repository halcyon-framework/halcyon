// SPEC-HAL-130/131 Stage 4: RadioGroup Action Emission Tests
// Tests for action emission on selection delta only.

use std::cell::RefCell;
use std::rc::Rc;

use halcyon::core::event::KeyboardAuthorityMode;
use halcyon::core::layout::Constraints;
use halcyon::core::math::RectF;
use halcyon::core::rect_authority::PaintAuthorityGuard;
use halcyon::core::scene::SceneBuilder;
use halcyon::core::theme::Theme;
use halcyon::core::widget::{LayoutContext, PaintContext, Widget};
use halcyon::core::widget_tree::WidgetTree;
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::widgets::radio_group::{RadioGroupEvent, RadioId};
use halcyon::widgets::radio_group_widget::{RadioGroup, RadioGroupId};

// ============================================================================
// Test Helpers
// ============================================================================

/// Captures emissions for testing.
struct EmissionCapture {
    emissions: Rc<RefCell<Vec<(RadioGroupId, Option<RadioId>)>>>,
}

impl EmissionCapture {
    fn new() -> Self {
        Self {
            emissions: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn callback(&self) -> impl FnMut(RadioGroupId, Option<RadioId>) + 'static {
        let emissions = self.emissions.clone();
        move |group_id, selected| {
            emissions.borrow_mut().push((group_id, selected));
        }
    }

    fn count(&self) -> usize {
        self.emissions.borrow().len()
    }

    fn last(&self) -> Option<(RadioGroupId, Option<RadioId>)> {
        self.emissions.borrow().last().copied()
    }

    fn clear(&self) {
        self.emissions.borrow_mut().clear();
    }
}

/// Run layout on a group to initialize member_height for hit testing.
fn run_layout(group: &mut RadioGroup) {
    let mut tree = WidgetTree::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    let constraints = Constraints {
        min_width: 0.0,
        max_width: 100.0,
        min_height: 0.0,
        max_height: 200.0,
    };
    let mut layout_cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
    group.layout(&mut layout_cx, constraints, &[]);
}

// ============================================================================
// Test 1: Select emits exactly one action
// ============================================================================

#[test]
fn select_emits_exactly_one_action() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(42), &[RadioId(1), RadioId(2), RadioId(3)]);
    group.set_on_selection_changed(capture.callback());

    // Initial: no emissions
    assert_eq!(capture.count(), 0);

    // Select member 1
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));

    assert_eq!(capture.count(), 1, "Select should emit exactly one action");
    assert_eq!(capture.last(), Some((RadioGroupId(42), Some(RadioId(1)))));
}

#[test]
fn select_different_member_emits_action() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2), RadioId(3)]);
    group.set_on_selection_changed(capture.callback());

    // Select member 1
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    assert_eq!(capture.count(), 1);

    // Select member 2
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(2)));
    assert_eq!(capture.count(), 2, "Selecting different member should emit");
    assert_eq!(capture.last(), Some((RadioGroupId(1), Some(RadioId(2)))));
}

// ============================================================================
// Test 2: Re-select same member emits nothing
// ============================================================================

#[test]
fn reselect_same_member_emits_nothing() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2)]);
    group.set_on_selection_changed(capture.callback());

    // Select member 1
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    assert_eq!(capture.count(), 1);

    // Re-select member 1 (no change)
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    assert_eq!(
        capture.count(),
        1,
        "Re-selecting same member should not emit"
    );
}

#[test]
fn activate_same_member_emits_nothing() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2)]);
    group.set_on_selection_changed(capture.callback());

    // Select member 1
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    assert_eq!(capture.count(), 1);

    // Activate member 1 (no change)
    group.on_semantic_event(RadioGroupEvent::Activate(RadioId(1)));
    assert_eq!(
        capture.count(),
        1,
        "Activating already-selected member should not emit"
    );
}

// ============================================================================
// Test 3: Activate emits action
// ============================================================================

#[test]
fn activate_emits_action() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(7), &[RadioId(1), RadioId(2), RadioId(3)]);
    group.set_on_selection_changed(capture.callback());

    // Activate member 2 (changes selection from None to Some(2))
    group.on_semantic_event(RadioGroupEvent::Activate(RadioId(2)));

    assert_eq!(capture.count(), 1, "Activate should emit action");
    assert_eq!(capture.last(), Some((RadioGroupId(7), Some(RadioId(2)))));
}

#[test]
fn activate_different_member_emits_action() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2), RadioId(3)]);
    group.set_on_selection_changed(capture.callback());

    // Select member 1
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    assert_eq!(capture.count(), 1);

    // Activate member 3 (changes selection)
    group.on_semantic_event(RadioGroupEvent::Activate(RadioId(3)));
    assert_eq!(
        capture.count(),
        2,
        "Activating different member should emit"
    );
    assert_eq!(capture.last(), Some((RadioGroupId(1), Some(RadioId(3)))));
}

// ============================================================================
// Test 4: Focus/Press/Cancel emit nothing
// ============================================================================

#[test]
fn focus_gain_emits_nothing() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2)]);
    group.set_on_selection_changed(capture.callback());

    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(1)));

    assert_eq!(capture.count(), 0, "FocusGain should not emit");
}

#[test]
fn focus_loss_emits_nothing() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2)]);
    group.set_on_selection_changed(capture.callback());

    // Set focus first
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(1)));
    assert_eq!(capture.count(), 0);

    group.on_semantic_event(RadioGroupEvent::FocusLoss(RadioId(1)));

    assert_eq!(capture.count(), 0, "FocusLoss should not emit");
}

#[test]
fn press_emits_nothing() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2)]);
    group.set_on_selection_changed(capture.callback());

    group.on_semantic_event(RadioGroupEvent::Press(RadioId(1)));

    assert_eq!(capture.count(), 0, "Press should not emit");
}

#[test]
fn cancel_emits_nothing() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2)]);
    group.set_on_selection_changed(capture.callback());

    // Set active first
    group.on_semantic_event(RadioGroupEvent::Press(RadioId(1)));
    assert_eq!(capture.count(), 0);

    group.on_semantic_event(RadioGroupEvent::Cancel(RadioId(1)));

    assert_eq!(capture.count(), 0, "Cancel should not emit");
}

// ============================================================================
// Test 5: Disabled group emits nothing
// ============================================================================

#[test]
fn disabled_group_emits_nothing_on_select() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2)]);
    group.set_on_selection_changed(capture.callback());
    group.set_disabled(true);

    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));

    assert_eq!(
        capture.count(),
        0,
        "Disabled group should not emit on Select"
    );
    assert_eq!(
        group.selected(),
        None,
        "Selection should not change when disabled"
    );
}

#[test]
fn disabled_group_emits_nothing_on_activate() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2)]);
    group.set_on_selection_changed(capture.callback());
    group.set_disabled(true);

    group.on_semantic_event(RadioGroupEvent::Activate(RadioId(1)));

    assert_eq!(
        capture.count(),
        0,
        "Disabled group should not emit on Activate"
    );
}

// ============================================================================
// Test 6: Paint emits nothing
// ============================================================================

#[test]
fn paint_emits_nothing() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(1), &[RadioId(1), RadioId(2), RadioId(3)]);
    group.set_on_selection_changed(capture.callback());
    run_layout(&mut group);

    // Select something first
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(2)));
    let count_before_paint = capture.count();

    // Paint
    let tree = WidgetTree::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();
    let mut scene = SceneBuilder::new();
    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects: std::collections::HashMap<halcyon::utils::id::TreeWidgetId, RectF> =
        std::collections::HashMap::new();
    let mut overlay_hit_regions = Vec::new();
    let mut tooltip_triggers = Vec::new();
    let icon_registry = halcyon::core::icons::builtin_registry();
    let mut paint_cx = PaintContext::new(
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
        None,                              // focused_widget: SPEC-HAL-328 §12.6
        KeyboardAuthorityMode::Navigation, // keyboard_mode: SPEC-HAL-176, default
    );
    let rect = RectF::new(0.0, 0.0, 100.0, 60.0);
    let _paint_authority = PaintAuthorityGuard::enter();
    group.paint(&mut paint_cx, rect, &[]);

    assert_eq!(capture.count(), count_before_paint, "Paint should not emit");
}

// ============================================================================
// Additional: Group ID is passed correctly
// ============================================================================

#[test]
fn group_id_passed_in_callback() {
    let capture = EmissionCapture::new();
    let mut group = RadioGroup::with_id(RadioGroupId(999), &[RadioId(1)]);
    group.set_on_selection_changed(capture.callback());

    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));

    assert_eq!(
        capture.last().map(|(gid, _)| gid),
        Some(RadioGroupId(999)),
        "Group ID should be passed correctly"
    );
}

#[test]
fn no_callback_set_does_not_panic() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);

    // Should not panic even without callback
    group.on_semantic_event(RadioGroupEvent::Select(RadioId(1)));
    group.on_semantic_event(RadioGroupEvent::Activate(RadioId(2)));

    assert_eq!(group.selected(), Some(RadioId(2)));
}
