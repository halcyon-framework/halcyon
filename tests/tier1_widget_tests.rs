// SPEC-HAL-011 Tier-1 Widget Tests (v2.0 - Flex-based)

use halcyon::core::app::layout_tree;
use halcyon::core::event::{
    Event, EventPhase, KeyboardAuthorityMode, Modifiers, PointerButton, PointerEvent,
    PointerEventKind, PointerId, PointerType,
};
use halcyon::core::layout::{Alignment, Constraints, FlexParams};
use halcyon::core::math::Size;
use halcyon::core::rect_authority::LayoutAuthorityGuard;
use halcyon::core::theme::Theme;
use halcyon::core::widget::{EventContext, Widget};
use halcyon::core::widget_tree::WidgetTree;
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::widgets::button::{Button, ButtonState};
use halcyon::widgets::fixed_spacer::FixedSpacer;
use halcyon::widgets::flex::Flex;
use halcyon::widgets::flex_spacer::FlexSpacer;
use halcyon::widgets::label::Label;
use halcyon::widgets::scroll_area::ScrollArea;

// ============================================================================
// Test 1: FlexRow Spacing - Verify actual positions
// ============================================================================

#[test]
fn test_flex_row_spacing() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // SPEC-HAL-011 v2.0: Use Flex::row() with 10px spacing
    let root = tree.create_root(Box::new(Flex::row().with_spacing(10.0)), None, 0);

    // Create 3 label widgets as children
    let child1 = tree.add_child(root, Box::new(Label::new("A")), None, 0);
    let child2 = tree.add_child(root, Box::new(Label::new("B")), None, 0);
    let child3 = tree.add_child(root, Box::new(Label::new("C")), None, 0);

    let constraints = Constraints::new(0.0, 1000.0, 0.0, 100.0);

    // Use layout_tree helper to avoid borrow issues
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    // Verify children count
    assert_eq!(tree.children(root).len(), 3);

    // Verify spacing: child2.x should equal child1.x + child1.width + spacing
    let rect1 = tree.get_rect(child1).unwrap();
    let rect2 = tree.get_rect(child2).unwrap();
    let rect3 = tree.get_rect(child3).unwrap();

    // First child starts at x=0
    assert_eq!(rect1.x, 0.0, "First child should start at x=0");

    // Second child = first.x + first.width + spacing
    let expected_x2 = rect1.x + rect1.width + 10.0;
    assert!(
        (rect2.x - expected_x2).abs() < 0.1,
        "Second child x={} should be {} (first + width + spacing)",
        rect2.x,
        expected_x2
    );

    // Third child = second.x + second.width + spacing
    let expected_x3 = rect2.x + rect2.width + 10.0;
    assert!(
        (rect3.x - expected_x3).abs() < 0.1,
        "Third child x={} should be {} (second + width + spacing)",
        rect3.x,
        expected_x3
    );
}

// ============================================================================
// Test 2: FlexColumn Alignment - Verify center alignment positions
// ============================================================================

#[test]
fn test_flex_column_alignment() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // SPEC-HAL-011 v2.0: Use Flex::column() with center alignment
    let root = tree.create_root(
        Box::new(Flex::column().with_align_items(Alignment::Center)),
        None,
        0,
    );

    // Create 2 label widgets with different widths
    let child1 = tree.add_child(root, Box::new(Label::new("Short")), None, 0);
    let child2 = tree.add_child(root, Box::new(Label::new("Longer Text Here")), None, 0);

    let constraints = Constraints::new(0.0, 1000.0, 0.0, 1000.0);

    // Use layout_tree helper
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    // Verify children count
    assert_eq!(tree.children(root).len(), 2);

    // Verify vertical stacking: child2 should be below child1
    let rect1 = tree.get_rect(child1).unwrap();
    let rect2 = tree.get_rect(child2).unwrap();

    assert!(
        rect2.y >= rect1.y + rect1.height,
        "Second child y={} should be at or below first child bottom {}",
        rect2.y,
        rect1.y + rect1.height
    );
}

// ============================================================================
// Test 3: FixedSpacer - Verify exact size allocation
// ============================================================================

#[test]
fn test_fixed_spacer_size() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Create column with fixed vertical spacer
    let root = tree.create_root(Box::new(Flex::column().with_spacing(0.0)), None, 0);

    let spacer = tree.add_child(root, Box::new(FixedSpacer::vertical(100.0)), None, 0);
    let label = tree.add_child(root, Box::new(Label::new("Below Spacer")), None, 0);

    let constraints = Constraints::new(0.0, 500.0, 0.0, 500.0);

    // Use layout_tree helper
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    // Verify spacer height is exactly 100
    let spacer_rect = tree.get_rect(spacer).unwrap();
    assert!(
        (spacer_rect.height - 100.0).abs() < 0.1,
        "FixedSpacer height={} should be 100.0",
        spacer_rect.height
    );

    // Verify label is positioned after spacer
    let label_rect = tree.get_rect(label).unwrap();
    assert!(
        (label_rect.y - 100.0).abs() < 0.1,
        "Label y={} should be 100.0 (after spacer)",
        label_rect.y
    );
}

// ============================================================================
// Test 4: FlexSpacer - Verify flex distribution
// ============================================================================

#[test]
fn test_flex_spacer_absorbs_space() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Create row with label, flex spacer, label
    let root = tree.create_root(Box::new(Flex::row().with_spacing(0.0)), None, 0);

    let label1 = tree.add_child(root, Box::new(Label::new("L")), None, 0);
    let spacer = tree.add_child(root, Box::new(FlexSpacer::new()), None, 0);
    tree.set_flex(spacer, FlexParams::flexible(1.0));
    let label2 = tree.add_child(root, Box::new(Label::new("R")), None, 0);

    // Use exact width constraint
    let constraints = Constraints::tight(Size::new(500.0, 50.0));

    // Use layout_tree helper
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let rect1 = tree.get_rect(label1).unwrap();
    let spacer_rect = tree.get_rect(spacer).unwrap();
    let rect2 = tree.get_rect(label2).unwrap();

    // FlexSpacer should absorb remaining space
    // Total width = label1.width + spacer.width + label2.width = 500
    let total = rect1.width + spacer_rect.width + rect2.width;
    assert!(
        (total - 500.0).abs() < 1.0,
        "Total width={} should be ~500 (label1={} + spacer={} + label2={})",
        total,
        rect1.width,
        spacer_rect.width,
        rect2.width
    );

    // Spacer should have gotten the majority of space
    assert!(
        spacer_rect.width > rect1.width && spacer_rect.width > rect2.width,
        "FlexSpacer width={} should be larger than labels ({}x{})",
        spacer_rect.width,
        rect1.width,
        rect2.width
    );

    // Second label should be at right edge
    assert!(
        (rect2.x + rect2.width - 500.0).abs() < 1.0,
        "Right label should end at x=500, got {}",
        rect2.x + rect2.width
    );
}

// ============================================================================
// Test 5: Button State Transitions
// ============================================================================

#[test]
fn test_button_state_machine() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create root container
    let root = tree.create_root(Box::new(Label::new("root")), None, 0);

    // Create button with label child
    let label = tree.add_child(root, Box::new(Label::new("Click Me")), None, 0);
    let mut button = Button::new(label);

    // SPEC-HAL-087: Add a dummy widget to tree just to get a valid TreeWidgetId
    let dummy = tree.add_child(root, Box::new(Label::new("dummy")), None, 1);
    let dummy_tree_id = tree.node(dummy).unwrap().id();

    // Initial state: Normal
    assert_eq!(button.state, ButtonState::Normal);

    // Simulate PointerDown event
    let mut cx = EventContext::new(
        dummy_tree_id,
        std::ptr::null_mut(),
        0,
        std::ptr::null_mut(),
        None,  // INV-TEXT-GEOM-PHASE: No text engine in test
        None,  // SPEC-HAL-021 Stage 2.5: Animation capability optional
        false, // has_pointer_capture
        false, // SPEC-HAL-268: Not relevant for tests
        KeyboardAuthorityMode::Navigation,
    );
    let down_event = Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (50.0, 50.0),
        local_pos: (50.0, 50.0),
        kind: PointerEventKind::Down,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    });

    button.handle_event(&mut cx, &down_event, EventPhase::Target);
    assert_eq!(button.state, ButtonState::Pressed);
    assert!(cx.is_capture_requested().is_some());

    // Simulate PointerUp event
    let mut cx = EventContext::new(
        dummy_tree_id,
        std::ptr::null_mut(),
        0,
        std::ptr::null_mut(),
        None,  // INV-TEXT-GEOM-PHASE: No text engine in test
        None,  // SPEC-HAL-021 Stage 2.5: Animation capability optional
        false, // has_pointer_capture
        false, // SPEC-HAL-268: Not relevant for tests
        KeyboardAuthorityMode::Navigation,
    );
    let up_event = Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (50.0, 50.0),
        local_pos: (50.0, 50.0),
        kind: PointerEventKind::Up,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    });

    button.handle_event(&mut cx, &up_event, EventPhase::Target);
    assert_eq!(button.state, ButtonState::Normal);
}

// ============================================================================
// Test 6: ScrollArea Scroll Bounds Clamping
// ============================================================================

#[test]
fn test_scroll_area_bounds() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Create root container
    let root = tree.create_root(Box::new(Label::new("root")), None, 0);

    // Create content widget
    let content = tree.add_child(root, Box::new(Label::new("Content")), None, 0);
    let mut scroll_area = ScrollArea::new(content);

    // SPEC-HAL-087: Add a dummy widget to tree just to get a valid TreeWidgetId
    let dummy = tree.add_child(root, Box::new(Label::new("dummy")), None, 1);
    let _dummy_tree_id = tree.node(dummy).unwrap().id();

    // Layout with viewport smaller than content
    let constraints = Constraints::new(0.0, 100.0, 0.0, 100.0);

    // Manual layout for scroll area (it needs special handling)
    {
        use halcyon::core::widget::LayoutContext;
        let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
        scroll_area.layout(&mut cx, constraints, &[]);
    }

    // Test scroll clamping: Try to scroll beyond min (negative)
    // SPEC-HAL-131A: scroll_to_y clamps to [0, max_scroll_y]
    scroll_area.scroll_to_y(-10.0); // Below min - should be clamped to 0

    // Verify scroll was clamped to 0
    let scroll_y = scroll_area.target_scroll_y();
    assert!(
        scroll_y >= 0.0,
        "Scroll should be clamped to 0, got {}",
        scroll_y
    );
    assert_eq!(scroll_y, 0.0, "Negative scroll should clamp to 0");
}

// ============================================================================
// Test 7: FlexRow with End Alignment (bottom)
// ============================================================================

#[test]
fn test_flex_row_vertical_alignment() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Use Flex::row() with End alignment (bottom for row)
    let root = tree.create_root(
        Box::new(Flex::row().with_align_items(Alignment::End)),
        None,
        0,
    );

    let child1 = tree.add_child(root, Box::new(Label::new("Top")), None, 0);
    let child2 = tree.add_child(root, Box::new(Label::new("Center")), None, 0);

    // Use constrained height so alignment is observable
    let constraints = Constraints::tight(Size::new(500.0, 100.0));

    // Use layout_tree helper
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    // Verify children count
    assert_eq!(tree.children(root).len(), 2);

    // With End alignment, children should be at bottom (y + height = 100)
    let rect1 = tree.get_rect(child1).unwrap();
    let rect2 = tree.get_rect(child2).unwrap();

    let bottom1 = rect1.y + rect1.height;
    let bottom2 = rect2.y + rect2.height;

    assert!(
        (bottom1 - 100.0).abs() < 1.0,
        "Child1 bottom={} should be ~100 (End alignment)",
        bottom1
    );
    assert!(
        (bottom2 - 100.0).abs() < 1.0,
        "Child2 bottom={} should be ~100 (End alignment)",
        bottom2
    );
}

// ============================================================================
// Test 8: FixedSpacer horizontal (for FlexRow)
// ============================================================================

#[test]
fn test_fixed_spacer_horizontal() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Create row with horizontal spacer between labels
    let root = tree.create_root(Box::new(Flex::row().with_spacing(0.0)), None, 0);

    let label1 = tree.add_child(root, Box::new(Label::new("A")), None, 0);
    let spacer = tree.add_child(root, Box::new(FixedSpacer::horizontal(50.0)), None, 0);
    let label2 = tree.add_child(root, Box::new(Label::new("B")), None, 0);

    let constraints = Constraints::new(0.0, 500.0, 0.0, 50.0);

    // Use layout_tree helper
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    // Verify spacer width is exactly 50
    let spacer_rect = tree.get_rect(spacer).unwrap();
    assert!(
        (spacer_rect.width - 50.0).abs() < 0.1,
        "FixedSpacer width={} should be 50.0",
        spacer_rect.width
    );

    // Verify label2 is positioned after spacer
    let rect1 = tree.get_rect(label1).unwrap();
    let rect2 = tree.get_rect(label2).unwrap();
    let expected_x2 = rect1.width + 50.0;

    assert!(
        (rect2.x - expected_x2).abs() < 0.1,
        "Label2 x={} should be {} (label1.width + spacer.width)",
        rect2.x,
        expected_x2
    );
}

// ============================================================================
// Test 9: Cross-Axis Alignment Matrix (Row with tight constraints)
// ============================================================================

#[test]
fn test_alignment_start_row() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    let root = tree.create_root(
        Box::new(Flex::row().with_align_items(Alignment::Start)),
        None,
        0,
    );
    let child = tree.add_child(root, Box::new(Label::new("Test")), None, 0);

    let constraints = Constraints::tight(Size::new(200.0, 100.0));
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let rect = tree.get_rect(child).unwrap();
    // Start alignment: y = 0
    assert!(
        rect.y.abs() < 0.1,
        "Start alignment: child y={} should be ~0",
        rect.y
    );
}

#[test]
fn test_alignment_center_row() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    let root = tree.create_root(
        Box::new(Flex::row().with_align_items(Alignment::Center)),
        None,
        0,
    );
    let child = tree.add_child(root, Box::new(Label::new("Test")), None, 0);

    let constraints = Constraints::tight(Size::new(200.0, 100.0));
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let rect = tree.get_rect(child).unwrap();
    // Center alignment: y = (container - child) / 2
    let expected_y = (100.0 - rect.height) / 2.0;
    assert!(
        (rect.y - expected_y).abs() < 1.0,
        "Center alignment: child y={} should be ~{} (centered in 100px)",
        rect.y,
        expected_y
    );
}

#[test]
fn test_alignment_end_row() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    let root = tree.create_root(
        Box::new(Flex::row().with_align_items(Alignment::End)),
        None,
        0,
    );
    let child = tree.add_child(root, Box::new(Label::new("Test")), None, 0);

    let constraints = Constraints::tight(Size::new(200.0, 100.0));
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let rect = tree.get_rect(child).unwrap();
    // End alignment: y + height = container height
    let bottom = rect.y + rect.height;
    assert!(
        (bottom - 100.0).abs() < 1.0,
        "End alignment: child bottom={} should be ~100",
        bottom
    );
}

#[test]
fn test_alignment_stretch_row() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    let root = tree.create_root(
        Box::new(Flex::row().with_align_items(Alignment::Stretch)),
        None,
        0,
    );
    let child = tree.add_child(root, Box::new(Label::new("Test")), None, 0);

    let constraints = Constraints::tight(Size::new(200.0, 100.0));
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let rect = tree.get_rect(child).unwrap();
    // Stretch alignment positions child at y=0 and fills the cross axis.
    assert!(
        rect.y.abs() < 0.1,
        "Stretch alignment: child y={} should be ~0",
        rect.y
    );
    assert!(
        (rect.height - 100.0).abs() < 0.1,
        "Stretch alignment: child height={} should fill container height",
        rect.height
    );
}

// ============================================================================
// Test 10: Grow/Shrink Interaction
// ============================================================================

#[test]
fn test_grow_proportional_distribution() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Three children with grow factors 1, 2, 3
    let root = tree.create_root(Box::new(Flex::row().with_spacing(0.0)), None, 0);

    let child1 = tree.add_child(root, Box::new(FlexSpacer::new()), None, 0);
    tree.set_flex(child1, FlexParams::new(1.0, 1.0, 0.0)); // grow=1

    let child2 = tree.add_child(root, Box::new(FlexSpacer::new()), None, 0);
    tree.set_flex(child2, FlexParams::new(2.0, 1.0, 0.0)); // grow=2

    let child3 = tree.add_child(root, Box::new(FlexSpacer::new()), None, 0);
    tree.set_flex(child3, FlexParams::new(3.0, 1.0, 0.0)); // grow=3

    let constraints = Constraints::tight(Size::new(600.0, 50.0));
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let rect1 = tree.get_rect(child1).unwrap();
    let rect2 = tree.get_rect(child2).unwrap();
    let rect3 = tree.get_rect(child3).unwrap();

    // Total grow = 1+2+3 = 6, so widths should be 100, 200, 300
    assert!(
        (rect1.width - 100.0).abs() < 1.0,
        "Child1 (grow=1) width={} should be ~100 (600/6)",
        rect1.width
    );
    assert!(
        (rect2.width - 200.0).abs() < 1.0,
        "Child2 (grow=2) width={} should be ~200 (600*2/6)",
        rect2.width
    );
    assert!(
        (rect3.width - 300.0).abs() < 1.0,
        "Child3 (grow=3) width={} should be ~300 (600*3/6)",
        rect3.width
    );
}

#[test]
fn test_shrink_proportional_distribution() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Three children with basis=200 each but container only 300
    // shrink factors: 0, 1, 1 - first child won't shrink
    let root = tree.create_root(Box::new(Flex::row().with_spacing(0.0)), None, 0);

    let child1 = tree.add_child(root, Box::new(Label::new("NoShrink")), None, 0);
    tree.set_flex(child1, FlexParams::new(0.0, 0.0, 100.0)); // shrink=0, basis=100

    let child2 = tree.add_child(root, Box::new(Label::new("Shrink")), None, 0);
    tree.set_flex(child2, FlexParams::new(0.0, 1.0, 100.0)); // shrink=1, basis=100

    let child3 = tree.add_child(root, Box::new(Label::new("Shrink")), None, 0);
    tree.set_flex(child3, FlexParams::new(0.0, 1.0, 100.0)); // shrink=1, basis=100

    // Total basis = 300, container = 200, deficit = 100
    let constraints = Constraints::tight(Size::new(200.0, 50.0));
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let rect1 = tree.get_rect(child1).unwrap();
    let rect2 = tree.get_rect(child2).unwrap();
    let rect3 = tree.get_rect(child3).unwrap();

    // Child1 (shrink=0) stays at basis=100
    assert!(
        (rect1.width - 100.0).abs() < 1.0,
        "Child1 (shrink=0) width={} should be ~100",
        rect1.width
    );

    // Children 2 and 3 share the 100px deficit equally (50 each)
    // So they should be 100 - 50 = 50 each
    assert!(
        (rect2.width - 50.0).abs() < 1.0,
        "Child2 (shrink=1) width={} should be ~50",
        rect2.width
    );
    assert!(
        (rect3.width - 50.0).abs() < 1.0,
        "Child3 (shrink=1) width={} should be ~50",
        rect3.width
    );
}

// ============================================================================
// Test 11: Spacer Behavior
// ============================================================================

#[test]
fn test_spacer_in_row_with_fixed_height() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Row: Label + FlexSpacer + Label with 500px width
    let root = tree.create_root(Box::new(Flex::row().with_spacing(0.0)), None, 0);

    let label1 = tree.add_child(root, Box::new(Label::new("Left")), None, 0);
    let spacer = tree.add_child(root, Box::new(FlexSpacer::new()), None, 0);
    tree.set_flex(spacer, FlexParams::flexible(1.0));
    let label2 = tree.add_child(root, Box::new(Label::new("Right")), None, 0);

    let constraints = Constraints::tight(Size::new(500.0, 50.0));
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let rect1 = tree.get_rect(label1).unwrap();
    let spacer_rect = tree.get_rect(spacer).unwrap();
    let rect2 = tree.get_rect(label2).unwrap();

    // Spacer should get the remaining space
    let expected_spacer_width = 500.0 - rect1.width - rect2.width;
    assert!(
        (spacer_rect.width - expected_spacer_width).abs() < 1.0,
        "FlexSpacer width={} should be ~{} (remaining space)",
        spacer_rect.width,
        expected_spacer_width
    );

    // Right label should be at right edge
    let right_edge = rect2.x + rect2.width;
    assert!(
        (right_edge - 500.0).abs() < 1.0,
        "Right label should end at x=500, got {}",
        right_edge
    );
}

#[test]
fn test_fixed_spacer_does_not_collapse() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Column with FixedSpacer between labels
    let root = tree.create_root(Box::new(Flex::column().with_spacing(0.0)), None, 0);

    let _label1 = tree.add_child(root, Box::new(Label::new("Top")), None, 0);
    let spacer = tree.add_child(root, Box::new(FixedSpacer::vertical(50.0)), None, 0);
    let _label2 = tree.add_child(root, Box::new(Label::new("Bottom")), None, 0);

    // Even with min-height constraint, spacer should maintain its height
    let constraints = Constraints::new(0.0, 200.0, 0.0, 200.0);
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let spacer_rect = tree.get_rect(spacer).unwrap();
    assert!(
        (spacer_rect.height - 50.0).abs() < 0.1,
        "FixedSpacer height={} should remain 50.0",
        spacer_rect.height
    );
}

// ============================================================================
// Test 12: Nested Flex
// ============================================================================

#[test]
fn test_nested_flex_no_cross_pollution() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Outer Row containing an inner Column
    let outer = tree.create_root(Box::new(Flex::row().with_spacing(0.0)), None, 0);
    let inner = tree.add_child(outer, Box::new(Flex::column().with_spacing(0.0)), None, 0);

    let label1 = tree.add_child(inner, Box::new(Label::new("Row1")), None, 0);
    let label2 = tree.add_child(inner, Box::new(Label::new("Row2")), None, 0);

    let constraints = Constraints::tight(Size::new(300.0, 200.0));
    layout_tree(
        &mut tree,
        outer,
        constraints,
        &theme,
        &mut text_engine,
        None,
    );

    // Inner column's children should be vertically stacked
    let rect1 = tree.get_rect(label1).unwrap();
    let rect2 = tree.get_rect(label2).unwrap();

    // Row2 should be below Row1
    assert!(
        rect2.y >= rect1.y + rect1.height - 0.1,
        "In nested column, label2.y={} should be >= label1.bottom={}",
        rect2.y,
        rect1.y + rect1.height
    );

    // Both should have same x (column layout)
    assert!(
        (rect1.x - rect2.x).abs() < 0.1,
        "Column children should have same x: {} vs {}",
        rect1.x,
        rect2.x
    );
}

#[test]
fn test_nested_flex_preserves_natural_height() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Outer column with inner row (no stretch)
    let outer = tree.create_root(
        Box::new(
            Flex::column()
                .with_spacing(10.0)
                .with_align_items(Alignment::Start),
        ),
        None,
        0,
    );
    let inner = tree.add_child(outer, Box::new(Flex::row().with_spacing(5.0)), None, 0);
    let label = tree.add_child(inner, Box::new(Label::new("InnerLabel")), None, 0);

    let constraints = Constraints::tight(Size::new(400.0, 300.0));
    layout_tree(
        &mut tree,
        outer,
        constraints,
        &theme,
        &mut text_engine,
        None,
    );

    let label_rect = tree.get_rect(label).unwrap();
    let inner_rect = tree.get_rect(inner).unwrap();

    // Label should have natural height (not stretched to 300)
    assert!(
        label_rect.height < 50.0,
        "Label height={} should be natural size, not stretched",
        label_rect.height
    );

    // Inner row height should match label height
    assert!(
        (inner_rect.height - label_rect.height).abs() < 1.0,
        "Inner row height={} should match label height={}",
        inner_rect.height,
        label_rect.height
    );
}

// ============================================================================
// Test 13: Wrap Line Distribution
// ============================================================================

#[test]
fn test_wrap_basic_line_break() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // FlowGrid-like setup: 4 items with basis=100, container=250
    // Should wrap: line1=[100, 100], line2=[100, 100]
    let root = tree.create_root(
        Box::new(Flex::row().with_wrap().with_spacing(10.0)),
        None,
        0,
    );

    let mut children = Vec::new();
    for _ in 0..4 {
        let child = tree.add_child(root, Box::new(FlexSpacer::new()), None, 0);
        tree.set_flex(child, FlexParams::new(1.0, 1.0, 100.0)); // basis=100
        children.push(child);
    }

    let constraints = Constraints::tight(Size::new(250.0, 300.0));
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let rect0 = tree.get_rect(children[0]).unwrap();
    let rect1 = tree.get_rect(children[1]).unwrap();
    let rect2 = tree.get_rect(children[2]).unwrap();
    let rect3 = tree.get_rect(children[3]).unwrap();

    // Items 0 and 1 should be on same line (same y)
    assert!(
        (rect0.y - rect1.y).abs() < 0.1,
        "Items 0 and 1 should be on same line: y={} vs y={}",
        rect0.y,
        rect1.y
    );

    // Items 2 and 3 should be on same line (same y, different from line 1)
    assert!(
        (rect2.y - rect3.y).abs() < 0.1,
        "Items 2 and 3 should be on same line: y={} vs y={}",
        rect2.y,
        rect3.y
    );

    // Line 2 should be below line 1
    assert!(
        rect2.y > rect0.y + rect0.height - 1.0,
        "Line 2 (y={}) should be below line 1 bottom ({})",
        rect2.y,
        rect0.y + rect0.height
    );
}

#[test]
fn test_wrap_items_grow_to_fill_line() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // 3 items with basis=100, container=250, grow=1
    // Should wrap: line1=[~120, ~120] (grow to fill 250-10=240, split 2 ways + spacing)
    // line2=[250] (single item fills width)
    let root = tree.create_root(
        Box::new(Flex::row().with_wrap().with_spacing(10.0)),
        None,
        0,
    );

    let child1 = tree.add_child(root, Box::new(FlexSpacer::new()), None, 0);
    tree.set_flex(child1, FlexParams::new(1.0, 1.0, 100.0));

    let child2 = tree.add_child(root, Box::new(FlexSpacer::new()), None, 0);
    tree.set_flex(child2, FlexParams::new(1.0, 1.0, 100.0));

    let child3 = tree.add_child(root, Box::new(FlexSpacer::new()), None, 0);
    tree.set_flex(child3, FlexParams::new(1.0, 1.0, 100.0));

    let constraints = Constraints::tight(Size::new(250.0, 200.0));
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    let rect1 = tree.get_rect(child1).unwrap();
    let rect2 = tree.get_rect(child2).unwrap();
    let rect3 = tree.get_rect(child3).unwrap();

    // Items 1 and 2 on line 1 should grow to fill 250px (minus spacing)
    let line1_total = rect1.width + rect2.width + 10.0; // +spacing
    assert!(
        (line1_total - 250.0).abs() < 1.0,
        "Line 1 items should fill container: {} + {} + 10 = {} (expected ~250)",
        rect1.width,
        rect2.width,
        line1_total
    );

    // Item 3 alone on line 2 should fill entire width
    assert!(
        (rect3.width - 250.0).abs() < 1.0,
        "Single item on line 2 should fill width: {} (expected ~250)",
        rect3.width
    );
}
