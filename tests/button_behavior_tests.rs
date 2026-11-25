// SPEC-HAL-038: Button Behavior Tests - Pointer Button Filtering
//
// Tests that verify buttons only respond to Primary (left) mouse button,
// ignoring Secondary (right) and other buttons.

use halcyon::core::event::{
    Event, EventPhase, KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers, PointerButton,
    PointerEvent, PointerEventKind, PointerId, PointerType,
};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::utils::id::WidgetInternalId;
use halcyon::widgets::button::{Button, ButtonState};
use halcyon::widgets::label::Label;
use std::cell::RefCell;
use std::rc::Rc;

// ============================================================================
// Minimal container for tree structure
// ============================================================================

struct TestContainer {
    id: WidgetInternalId,
}

impl TestContainer {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for TestContainer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TestContainer {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(400.0, 200.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }
}

// ============================================================================
// Helper: Create button and EventContext
// ============================================================================

fn setup_button_test(click_count: Rc<RefCell<u32>>) -> (Button, WidgetTree) {
    let mut tree = WidgetTree::new();

    // Create root container
    let root = tree.create_root(Box::new(TestContainer::new()), None, 0);

    // Create a label as the button's child
    let label = tree.add_child(root, Box::new(Label::new("Test")), None, 0);

    // Create button with the label as child
    let count = click_count.clone();
    let button = Button::new(label).with_padding(10.0).on_click(move || {
        *count.borrow_mut() += 1;
    });

    (button, tree)
}

fn get_event_context(tree: &mut WidgetTree, root: NodeId) -> EventContext<'_> {
    let tree_id = tree.node(root).unwrap().id();
    // Create EventContext with valid TreeWidgetId, null tree pointer, 0 frame time, null clipboard, no animation
    // (We don't actually use the tree pointer, frame time, clipboard, or animation in these tests)
    EventContext::new(
        tree_id,
        std::ptr::null_mut(),
        0,
        std::ptr::null_mut(),
        None,  // INV-TEXT-GEOM-PHASE: No text engine in test
        None,  // SPEC-HAL-021 Stage 2.5: Animation capability optional
        false, // has_pointer_capture
        false, // SPEC-HAL-268: Not relevant for tests
        KeyboardAuthorityMode::Navigation,
    )
}

// ============================================================================
// Helper: Create pointer event
// ============================================================================

fn pointer_event(
    kind: PointerEventKind,
    button: Option<PointerButton>,
    local_pos: (f32, f32),
) -> Event {
    Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: local_pos,
        local_pos,
        kind,
        button,
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    })
}

fn key_event(key_code: KeyCode, is_down: bool) -> Event {
    Event::Key(KeyEvent {
        key_code,

        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        is_down,
        is_repeat: false,
        modifiers: Modifiers::default(),
        text: None,
    })
}

// ============================================================================
// Test 1: Right-click does not press button
// ============================================================================

#[test]
fn test_right_click_does_not_press_button() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());

    // Get root node for EventContext
    let root = tree.root().unwrap();

    // Manually set to Hovered state (simulating hover enter)
    button.state = ButtonState::Hovered;

    // Right-click down - should NOT change state to Pressed
    let right_down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Secondary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    let consumed = button.handle_event(&mut cx, &right_down, EventPhase::Target);

    // Event should NOT be consumed
    assert!(
        !consumed,
        "Right-click down should not be consumed by button"
    );

    // State should still be Hovered, NOT Pressed
    assert_eq!(
        button.state,
        ButtonState::Hovered,
        "Right-click should NOT change button to Pressed state"
    );

    // No callback should have fired
    assert_eq!(
        *click_count.borrow(),
        0,
        "Right-click should not trigger callback"
    );
}

// ============================================================================
// Test 2: Right-click does not activate
// ============================================================================

#[test]
fn test_right_click_does_not_activate() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();

    // Start in Normal state
    assert_eq!(button.state, ButtonState::Normal);

    // Right-click down
    let right_down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Secondary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    let consumed = button.handle_event(&mut cx, &right_down, EventPhase::Target);
    assert!(!consumed, "Right-click down should not be consumed");

    // Right-click up
    let right_up = pointer_event(
        PointerEventKind::Up,
        Some(PointerButton::Secondary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    let consumed = button.handle_event(&mut cx, &right_up, EventPhase::Target);
    assert!(!consumed, "Right-click up should not be consumed");

    // Callback should NOT have fired
    assert_eq!(
        *click_count.borrow(),
        0,
        "Right-click should not trigger activation callback"
    );

    // Button should remain in Normal state
    assert_eq!(
        button.state,
        ButtonState::Normal,
        "Button should remain Normal after right-click"
    );
}

// ============================================================================
// Test 3: Right-click does not capture pointer
// ============================================================================

#[test]
fn test_right_click_does_not_capture_pointer() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();

    // Manually set to Hovered state
    button.state = ButtonState::Hovered;

    // Right-click down - should NOT capture pointer
    let right_down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Secondary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    let consumed = button.handle_event(&mut cx, &right_down, EventPhase::Target);

    // Event should NOT be consumed (no capture happened)
    assert!(
        !consumed,
        "Right-click should not consume event (no capture)"
    );

    // Capture should NOT have been requested
    assert!(
        cx.is_capture_requested().is_none(),
        "Right-click should not request pointer capture"
    );

    // State should still be Hovered (no capture transition to Pressed)
    assert_eq!(
        button.state,
        ButtonState::Hovered,
        "Right-click should not trigger capture/press transition"
    );
}

// ============================================================================
// Test 4: Left click still works normally (regression test)
// ============================================================================

#[test]
fn test_left_click_still_works() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();

    // Start in Normal state
    assert_eq!(button.state, ButtonState::Normal);

    // Left-click down (Primary button)
    let left_down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Primary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    let consumed = button.handle_event(&mut cx, &left_down, EventPhase::Target);

    // Event should be consumed
    assert!(consumed, "Left-click down should be consumed by button");

    // Button should be Pressed
    assert_eq!(
        button.state,
        ButtonState::Pressed,
        "Left-click should change button to Pressed state"
    );

    // Capture should have been requested
    assert!(
        cx.is_capture_requested().is_some(),
        "Left-click should request pointer capture"
    );

    // Left-click up
    // Note: bounds check will use Size::zero() since we didn't run layout,
    // so we test with position (0, 0) to be "inside" the zero bounds
    let left_up = pointer_event(
        PointerEventKind::Up,
        Some(PointerButton::Primary),
        (0.0, 0.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    let consumed = button.handle_event(&mut cx, &left_up, EventPhase::Target);

    // Event should be consumed
    assert!(consumed, "Left-click up should be consumed by button");

    // Button should be Normal after release
    assert_eq!(
        button.state,
        ButtonState::Normal,
        "Button should be Normal after left-click release"
    );

    // Note: Callback might not fire because bounds = Size::zero() and (0,0) is on the edge
    // The important verification is that the state machine works for Primary button
}

// ============================================================================
// Test 5: Double-click activates button (regression test)
// ============================================================================

#[test]
fn test_double_click_activates_button() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();

    // Start in Normal state
    assert_eq!(button.state, ButtonState::Normal);

    // DoubleClick event (InputFusion emits this when second click is within threshold)
    let double_click = pointer_event(
        PointerEventKind::DoubleClick,
        Some(PointerButton::Primary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    let consumed = button.handle_event(&mut cx, &double_click, EventPhase::Target);

    // Event should be consumed
    assert!(
        consumed,
        "DoubleClick should be consumed by button (treated as Down)"
    );

    // Button should be Pressed
    assert_eq!(
        button.state,
        ButtonState::Pressed,
        "DoubleClick should change button to Pressed state"
    );

    // Capture should have been requested
    assert!(
        cx.is_capture_requested().is_some(),
        "DoubleClick should request pointer capture"
    );
}

// ============================================================================
// Test 6: Triple-click activates button (regression test)
// ============================================================================

#[test]
fn test_triple_click_activates_button() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();

    // Start in Normal state
    assert_eq!(button.state, ButtonState::Normal);

    // TripleClick event
    let triple_click = pointer_event(
        PointerEventKind::TripleClick,
        Some(PointerButton::Primary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    let consumed = button.handle_event(&mut cx, &triple_click, EventPhase::Target);

    // Event should be consumed
    assert!(
        consumed,
        "TripleClick should be consumed by button (treated as Down)"
    );

    // Button should be Pressed
    assert_eq!(
        button.state,
        ButtonState::Pressed,
        "TripleClick should change button to Pressed state"
    );

    // Capture should have been requested
    assert!(
        cx.is_capture_requested().is_some(),
        "TripleClick should request pointer capture"
    );
}

// ============================================================================
// Test 7: Auxiliary (middle) click does not activate
// ============================================================================

#[test]
fn test_auxiliary_click_does_not_activate() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();

    // Auxiliary (middle mouse) click down
    let aux_down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Auxiliary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    let consumed = button.handle_event(&mut cx, &aux_down, EventPhase::Target);
    assert!(!consumed, "Auxiliary click down should not be consumed");

    // Auxiliary click up
    let aux_up = pointer_event(
        PointerEventKind::Up,
        Some(PointerButton::Auxiliary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    let consumed = button.handle_event(&mut cx, &aux_up, EventPhase::Target);
    assert!(!consumed, "Auxiliary click up should not be consumed");

    // Callback should NOT have fired
    assert_eq!(
        *click_count.borrow(),
        0,
        "Auxiliary click should not trigger callback"
    );
}

// ============================================================================
// Test 8: Keyboard activation via on_keyboard_activate
// ============================================================================

#[test]
fn test_keyboard_activation_fires_callback() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();
    let mut cx = get_event_context(&mut tree, root);

    // Initial state
    assert_eq!(*click_count.borrow(), 0);
    assert_eq!(button.state, ButtonState::Normal);

    // Simulate keyboard press (called by event router for Space/Enter)
    button.on_keyboard_activate(&mut cx);

    assert_eq!(
        *click_count.borrow(),
        0,
        "KeyDown alone must not activate the button"
    );
    assert!(
        button.keyboard_pulse_active.get(),
        "KeyDown should arm the keyboard activation cycle"
    );

    let key_up = key_event(KeyCode::Enter, false);
    let consumed = button.handle_event(&mut cx, &key_up, EventPhase::Target);
    assert!(consumed, "Activation KeyUp should be consumed");

    // Callback should fire exactly once on key release.
    assert_eq!(
        *click_count.borrow(),
        1,
        "Keyboard release should fire callback exactly once"
    );

    // State should be Normal after the release cycle.
    assert_eq!(
        button.state,
        ButtonState::Normal,
        "Keyboard release should leave button state Normal"
    );
}

#[test]
fn test_keyboard_activation_multiple_times() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();
    let mut cx = get_event_context(&mut tree, root);

    let key_up = key_event(KeyCode::Enter, false);
    for _ in 0..3 {
        button.on_keyboard_activate(&mut cx);
        button.handle_event(&mut cx, &key_up, EventPhase::Target);
    }

    // Each press/release cycle should emit exactly once.
    assert_eq!(
        *click_count.borrow(),
        3,
        "Each keyboard press/release cycle should emit once"
    );
}

// ============================================================================
// Test 9: Focus lifecycle
// ============================================================================

#[test]
fn test_focus_lifecycle() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();
    let mut cx = get_event_context(&mut tree, root);

    // Verify focusable
    assert!(button.focusable(), "Button should be focusable");

    // Gain focus (no panic = success)
    button.on_focus(&mut cx);

    // Lose focus (no panic = success)
    button.on_blur(&mut cx);

    // No callbacks fired just from focus changes
    assert_eq!(
        *click_count.borrow(),
        0,
        "Focus changes should not fire callback"
    );
}

#[test]
fn test_accepts_keyboard_activation() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (button, _tree) = setup_button_test(click_count);

    assert!(
        button.accepts_keyboard_activation(),
        "Button should accept keyboard activation"
    );
}

// ============================================================================
// REGRESSION TEST: Space and Enter parity for keyboard activation
// ============================================================================
// Context: Space was previously routed as TextInput instead of KeyCode::Space.
// After fix (commit 9bc5434), both Space and Enter route through
// on_keyboard_activate via the event router fallback.
//
// This test verifies that on_keyboard_activate arms the common activation path
// for both Space and Enter and that KeyUp emits exactly once.

#[test]
fn regression_keyboard_activation_emits_exactly_once() {
    // This test ensures one keydown/keyup cycle emits exactly once.
    // Both Space and Enter ultimately use this path via the event router.

    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();
    let mut cx = get_event_context(&mut tree, root);

    assert_eq!(*click_count.borrow(), 0);

    button.on_keyboard_activate(&mut cx);
    assert_eq!(
        *click_count.borrow(),
        0,
        "on_keyboard_activate must not emit before KeyUp"
    );

    let key_up = key_event(KeyCode::Enter, false);
    button.handle_event(&mut cx, &key_up, EventPhase::Target);

    assert_eq!(
        *click_count.borrow(),
        1,
        "keyboard activation cycle must emit exactly once"
    );
}

// ============================================================================
// Test 10: Paint does not emit callback
// ============================================================================

#[test]
fn test_paint_does_not_emit_callback() {
    use halcyon::core::rect_authority::{enter_paint_authority, exit_paint_authority};
    use halcyon::core::scene::SceneBuilder;
    use halcyon::core::theme::Theme;
    use halcyon::core::widget::PaintContext;
    use halcyon::core::widget_tree::OverlayHitRegion;
    use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
    use std::collections::HashMap;

    let click_count = Rc::new(RefCell::new(0u32));
    let (button, tree) = setup_button_test(click_count.clone());

    // Paint the button
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();
    let mut scene = SceneBuilder::new();
    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects = HashMap::new();
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
    let rect = RectF::new(0.0, 0.0, 100.0, 40.0);

    // SPEC-HAL-146: Enter paint authority before calling paint
    enter_paint_authority();
    button.paint(&mut cx, rect, &[]);
    exit_paint_authority();

    // Callback should NOT have been called
    assert_eq!(*click_count.borrow(), 0, "Paint should never emit callback");
}

// ============================================================================
// Test 11: Pointer cancel semantics (release outside)
// ============================================================================
// Note: These tests use the default bounds (Size::zero) from setup.
// The hit-test logic uses local_pos <= bounds, so (0,0) is on the edge
// and positions > 0 are outside.

#[test]
fn test_pointer_release_outside_cancels() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();

    // Press at (0,0) - on the edge of zero-size bounds
    let down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Primary),
        (0.0, 0.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &down, EventPhase::Target);
    assert_eq!(button.state, ButtonState::Pressed);

    // Release outside (any position > 0 is outside zero-size bounds)
    let up_outside = pointer_event(
        PointerEventKind::Up,
        Some(PointerButton::Primary),
        (1.0, 1.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &up_outside, EventPhase::Target);

    // Callback should NOT have fired (released outside)
    assert_eq!(
        *click_count.borrow(),
        0,
        "Release outside should cancel (not fire callback)"
    );

    // State should reset to Normal
    assert_eq!(
        button.state,
        ButtonState::Normal,
        "State should reset to Normal after cancel"
    );
}

#[test]
fn test_pointer_release_inside_fires() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();

    // Press at (0,0) - on the edge of zero-size bounds
    let down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Primary),
        (0.0, 0.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &down, EventPhase::Target);

    // Release at (0,0) - still inside (on edge)
    let up_inside = pointer_event(
        PointerEventKind::Up,
        Some(PointerButton::Primary),
        (0.0, 0.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &up_inside, EventPhase::Target);

    // Callback SHOULD have fired
    assert_eq!(
        *click_count.borrow(),
        1,
        "Release inside should fire callback"
    );
}

// ============================================================================
// SPEC-HAL-021-A: Pressed Visual Phase Tests
// ============================================================================

#[test]
fn test_pressed_visual_starts_on_primary_down() {
    // Verify that pressed_t transitions toward 1.0 on primary PointerDown
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count);
    let root = tree.root().unwrap();

    // Initial state: pressed_t should be 0.0
    assert!(
        button.pressed_t.get() < 0.001,
        "pressed_t should start at 0.0"
    );

    // Primary pointer down
    let down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Primary),
        (0.0, 0.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &down, EventPhase::Target);

    // pressed_anim should have been set (even if None due to no capability)
    // The animation handle is set, but value won't be resolved without Timeline
    // This test verifies the code path executes without panic
    assert_eq!(
        button.state,
        ButtonState::Pressed,
        "State should be Pressed after down"
    );
}

#[test]
fn test_pressed_visual_clears_on_up() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count);
    let root = tree.root().unwrap();

    // Simulate press
    let down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Primary),
        (0.0, 0.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &down, EventPhase::Target);

    // Manually set pressed_t to 1.0 (simulating animation completion)
    button.pressed_t.set(1.0);

    // Release at (0,0) - inside bounds
    let up = pointer_event(
        PointerEventKind::Up,
        Some(PointerButton::Primary),
        (0.0, 0.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &up, EventPhase::Target);

    // State should be Normal after release
    assert_eq!(
        button.state,
        ButtonState::Normal,
        "State should be Normal after up"
    );

    // pressed_anim should have been set for fade-out
    // (animation target is 0.0, pressed_t will animate toward it)
}

#[test]
fn test_pressed_visual_clears_on_leave_during_press() {
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count);
    let root = tree.root().unwrap();

    // Set non-zero bounds for proper inside/outside detection
    button.layout_bounds = Size::new(100.0, 40.0);

    // Press inside
    let down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Primary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &down, EventPhase::Target);
    assert_eq!(button.state, ButtonState::Pressed);

    // Manually set pressed_t to 1.0
    button.pressed_t.set(1.0);

    // Move outside (leave bounds while pressed)
    let move_outside = pointer_event(
        PointerEventKind::Move,
        Some(PointerButton::Primary),
        (200.0, 200.0), // outside bounds
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &move_outside, EventPhase::Target);

    // State should change to Normal (pointer left during press)
    assert_eq!(
        button.state,
        ButtonState::Normal,
        "State should be Normal after leaving bounds during press"
    );

    // pressed_anim should have been set for fade-out
}

#[test]
fn test_pressed_visual_no_animation_without_capability() {
    // When animation capability is None, animation should not start
    // but the code should not panic
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count);
    let root = tree.root().unwrap();

    // EventContext with None animation capability (default in get_event_context)
    let down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Primary),
        (0.0, 0.0),
    );
    let mut cx = get_event_context(&mut tree, root);

    // This should not panic even without animation capability
    button.handle_event(&mut cx, &down, EventPhase::Target);

    // pressed_anim should be None (no capability to start animation)
    assert!(
        button.pressed_anim.get().is_none(),
        "pressed_anim should be None without animation capability"
    );

    // State machine should still work
    assert_eq!(button.state, ButtonState::Pressed);
}

#[test]
fn test_pressed_visual_does_not_affect_click_semantics() {
    // Click semantics (callback firing) should be unchanged
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();

    // Set bounds for proper hit testing
    button.layout_bounds = Size::new(100.0, 40.0);

    // Press inside
    let down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Primary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &down, EventPhase::Target);

    // Release inside
    let up = pointer_event(
        PointerEventKind::Up,
        Some(PointerButton::Primary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &up, EventPhase::Target);

    // Callback should have fired exactly once
    assert_eq!(
        *click_count.borrow(),
        1,
        "Click callback should fire normally with pressed visuals"
    );
}

#[test]
fn test_keyboard_activation_does_not_start_pressed_animation() {
    // Per SPEC-HAL-021-A: keyboard activation must NOT trigger pressed visuals
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count.clone());
    let root = tree.root().unwrap();
    let mut cx = get_event_context(&mut tree, root);

    // Initial state
    assert!(button.pressed_anim.get().is_none());

    // Keyboard activation
    button.on_keyboard_activate(&mut cx);

    // pressed_anim should still be None (keyboard activation doesn't trigger it)
    assert!(
        button.pressed_anim.get().is_none(),
        "Keyboard activation should NOT start pressed animation"
    );

    let key_up = key_event(KeyCode::Enter, false);
    button.handle_event(&mut cx, &key_up, EventPhase::Target);

    // Callback should have fired on release (activation works)
    assert_eq!(*click_count.borrow(), 1);
}

// ============================================================================
// SPEC-HAL-021 Stage 7: Animation Visual State Initialization (ANIM-INV-10)
// ============================================================================

#[test]
fn test_button_on_mount_resets_animation_state() {
    // ANIM-INV-10: All animation-backed visual state MUST be initialized
    // to neutral values at widget mount time.
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, _tree) = setup_button_test(click_count);

    // Simulate some "contaminated" state that might exist from prior usage
    button.hover_t.set(0.5);
    button.pressed_t.set(0.3);
    // (hover_anim and pressed_anim can't be set without real Timeline)

    // Call on_mount - this should reset all animation state
    button.on_mount();

    // Verify all animation caches are neutral (zero)
    assert!(
        button.hover_t.get() < 0.001,
        "on_mount must reset hover_t to 0.0"
    );
    assert!(
        button.pressed_t.get() < 0.001,
        "on_mount must reset pressed_t to 0.0"
    );
    assert!(
        button.hover_anim.get().is_none(),
        "on_mount must clear hover_anim"
    );
    assert!(
        button.pressed_anim.get().is_none(),
        "on_mount must clear pressed_anim"
    );
}

#[test]
fn test_multiple_buttons_no_startup_visual_contamination() {
    // ANIM-INV-10: No widget may render non-neutral visuals before first input.
    // This test simulates a FlowGrid-like scenario with multiple buttons.
    let mut tree = WidgetTree::new();

    // Create root container
    let root = tree.create_root(Box::new(TestContainer::new()), None, 0);

    // Create multiple buttons (simulating FlowGrid stress test)
    let mut buttons: Vec<Button> = Vec::new();
    for i in 0..10 {
        let label = tree.add_child(root, Box::new(Label::new(&format!("Btn{}", i))), None, 0);
        let mut button = Button::new(label).with_padding(8.0);
        // Call on_mount as the framework would
        button.on_mount();
        buttons.push(button);
    }

    // Verify ALL buttons have neutral animation state
    for (i, button) in buttons.iter().enumerate() {
        assert!(
            button.hover_t.get() < 0.001,
            "Button {} hover_t should be 0.0 at mount",
            i
        );
        assert!(
            button.pressed_t.get() < 0.001,
            "Button {} pressed_t should be 0.0 at mount",
            i
        );
        assert!(
            button.hover_anim.get().is_none(),
            "Button {} hover_anim should be None at mount",
            i
        );
        assert!(
            button.pressed_anim.get().is_none(),
            "Button {} pressed_anim should be None at mount",
            i
        );
        // These are the alpha values used in paint() - both must be < 0.001
        // to avoid rendering overlays
        assert!(
            button.hover_t.get() < 0.001 && button.pressed_t.get() < 0.001,
            "Button {} would render visual contamination (hover/pressed overlay)",
            i
        );
    }
}

// ============================================================================
// PRE-STAGE-8 FIX: Ghost Button Regression Test
// ============================================================================
// Context: When on_hover_leave was called WITHOUT animation capability,
// hover_t remained at its previous value (e.g., 1.0) instead of snapping to 0.
// This caused "ghost buttons" that rendered with hover overlay but didn't respond
// to clicks because they weren't actually in hovered state.
//
// Root cause: cx.start_animation() returns None when capability is absent,
// but the code only set hover_anim without updating hover_t.
//
// Fix: When animation capability is absent, snap hover_t directly to target value.

#[test]
fn regression_ghost_button_hover_leave_without_animation() {
    // REGRESSION: Ghost button caused by hover_t not resetting on hover_leave
    // when animation capability is unavailable.
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count);
    let root = tree.root().unwrap();

    // EventContext with NO animation capability (the bug trigger)
    let mut cx = get_event_context(&mut tree, root);

    // Simulate hover_t at 1.0 (button was hovered)
    button.hover_t.set(1.0);
    button.state = ButtonState::Hovered;

    // Call on_hover_leave WITHOUT animation capability
    // (This is what happens when clicking on a different widget)
    button.on_hover_leave(&mut cx);

    // CRITICAL: hover_t MUST be 0.0, not stuck at 1.0
    // Before fix: hover_t would remain at 1.0 (ghost button)
    // After fix: hover_t snaps to 0.0 immediately
    assert!(
        button.hover_t.get() < 0.001,
        "REGRESSION: hover_t must be 0.0 after on_hover_leave without animation capability. \
         Got {} - this would cause a ghost button!",
        button.hover_t.get()
    );

    // hover_anim should be None (no animation started)
    assert!(
        button.hover_anim.get().is_none(),
        "hover_anim should be None when animation capability is absent"
    );

    // State should be Normal
    assert_eq!(button.state, ButtonState::Normal);
}

#[test]
fn regression_ghost_button_hover_enter_without_animation() {
    // Complementary test: on_hover_enter without animation should also snap to 1.0
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count);
    let root = tree.root().unwrap();

    // EventContext with NO animation capability
    let mut cx = get_event_context(&mut tree, root);

    // Ensure hover_t starts at 0
    button.hover_t.set(0.0);
    button.state = ButtonState::Normal;

    // Call on_hover_enter WITHOUT animation capability
    button.on_hover_enter(&mut cx);

    // hover_t MUST be 1.0 (snapped immediately)
    assert!(
        (button.hover_t.get() - 1.0).abs() < 0.001,
        "hover_t must be 1.0 after on_hover_enter without animation capability. Got {}",
        button.hover_t.get()
    );

    // State should be Hovered
    assert_eq!(button.state, ButtonState::Hovered);
}

#[test]
fn regression_ghost_button_pressed_release_without_animation() {
    // REGRESSION: pressed_t not resetting on PointerUp without animation capability
    let click_count = Rc::new(RefCell::new(0u32));
    let (mut button, mut tree) = setup_button_test(click_count);
    let root = tree.root().unwrap();

    // Set bounds for hit testing
    button.layout_bounds = Size::new(100.0, 40.0);

    // Press button (PointerDown)
    let down = pointer_event(
        PointerEventKind::Down,
        Some(PointerButton::Primary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &down, EventPhase::Target);

    // pressed_t should be 1.0 (snapped immediately due to no animation)
    assert!(
        (button.pressed_t.get() - 1.0).abs() < 0.001,
        "pressed_t must be 1.0 after PointerDown without animation. Got {}",
        button.pressed_t.get()
    );

    // Release button (PointerUp)
    let up = pointer_event(
        PointerEventKind::Up,
        Some(PointerButton::Primary),
        (50.0, 20.0),
    );
    let mut cx = get_event_context(&mut tree, root);
    button.handle_event(&mut cx, &up, EventPhase::Target);

    // CRITICAL: pressed_t MUST be 0.0, not stuck at 1.0
    assert!(
        button.pressed_t.get() < 0.001,
        "REGRESSION: pressed_t must be 0.0 after PointerUp without animation. \
         Got {} - this would cause a stuck pressed visual!",
        button.pressed_t.get()
    );
}
