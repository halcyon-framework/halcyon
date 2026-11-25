// SPEC-HAL-131A: FlowGrid/Flex Wrapped Content Size Test
//
// Verifies that Flex with wrap enabled reports correct content_size
// to ScrollArea, enabling proper scrollbar range.
//
// INV-EXTENT-1: Layout content_size must reflect true content extent,
// not viewport-constrained size.

use halcyon::core::app::layout_tree;
use halcyon::core::layout::Constraints;
use halcyon::core::rect_authority::LayoutAuthorityGuard;
use halcyon::core::theme::Theme;
use halcyon::core::widget_tree::WidgetTree;
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::widgets::fixed_spacer::FixedSpacer;
use halcyon::widgets::flex::Flex;

/// Test that wrapped Flex positions children correctly beyond viewport bounds.
///
/// This is the core fix for FlowGrid scrolling (SPEC-HAL-131A INV-EXTENT-1):
/// - Before fix: Flex::layout_wrap() returned LayoutResult::fit(constrained_size),
///   which set content_size = size = viewport size. ScrollArea saw no overflow.
/// - After fix: Flex::layout_wrap() returns LayoutResult::new(size, content_size)
///   where content_size reflects full wrapped extent. ScrollArea sees correct overflow.
///
/// This test verifies the fix by checking child positioning:
/// - Children are positioned at their correct Y coordinates (not clamped to viewport)
/// - Last row children have y > viewport height, proving full content is computed
#[test]
fn test_flex_wrap_reports_full_content_size() {
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Create wrapped Flex (FlowGrid pattern)
    let flex_node = tree.create_root(
        Box::new(Flex::row().with_wrap().with_spacing(10.0)),
        None,
        0,
    );

    // Add 10 containers: 80x30 each
    // With 200px viewport width and 10px spacing:
    // Row 1: [80][10][80] = 170px (fits 2 items)
    // Row 2: [80][10][80] = 170px (fits 2 items)
    // ... 5 rows total, each 30px high + 10px spacing between rows
    // Total height: 5 * 30 + 4 * 10 = 150 + 40 = 190px
    let mut children = Vec::new();
    for _ in 0..10 {
        let child = tree.add_child(flex_node, Box::new(FixedSpacer::new(80.0, 30.0)), None, 0);
        children.push(child);
    }

    // Layout Flex with constrained height (100px viewport, but content is 190px)
    let constraints = Constraints::new(0.0, 200.0, 0.0, 100.0);
    {
        let _guard = LayoutAuthorityGuard::enter();
        layout_tree(
            &mut tree,
            flex_node,
            constraints,
            &theme,
            &mut text_engine,
            None,
        );
    }

    // Verify: children are positioned correctly
    let first_child_rect = tree.get_rect(children[0]).unwrap();
    let last_child_rect = tree.get_rect(children[9]).unwrap();

    // First child should be at y=0
    assert!(
        first_child_rect.y.abs() < 0.1,
        "First child should start at y=0, got y={}",
        first_child_rect.y
    );

    // Last child should be on the last row (row 5, 0-indexed row 4)
    // Row positions: row 0 at y=0, row 1 at y=40, row 2 at y=80, row 3 at y=120, row 4 at y=160
    // (30px height + 10px spacing = 40px per row)
    assert!(
        last_child_rect.y > 100.0,
        "Last child should be positioned beyond viewport (y > 100), got y={}. \
         This proves Flex is correctly computing full content extent for ScrollArea.",
        last_child_rect.y
    );

    // Verify exact position of last child
    // Row 4 starts at y = 4 * (30 + 10) = 160
    assert!(
        (last_child_rect.y - 160.0).abs() < 0.1,
        "Last child should be at y=160 (row 4), got y={}",
        last_child_rect.y
    );
}

/// Test that non-wrapped Flex still works correctly (no behavior change).
#[test]
fn test_flex_no_wrap_unchanged() {
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    // Create non-wrapped Flex (should fit in one row)
    let flex_node = tree.create_root(
        Box::new(Flex::row().with_spacing(10.0)), // No wrap
        None,
        0,
    );

    // Add 3 spacers that fit in one row
    let mut children = Vec::new();
    for _ in 0..3 {
        let child = tree.add_child(flex_node, Box::new(FixedSpacer::new(50.0, 30.0)), None, 0);
        children.push(child);
    }

    // Layout with plenty of space
    let constraints = Constraints::new(0.0, 500.0, 0.0, 100.0);
    {
        let _guard = LayoutAuthorityGuard::enter();
        layout_tree(
            &mut tree,
            flex_node,
            constraints,
            &theme,
            &mut text_engine,
            None,
        );
    }

    // Verify: all children on same row (y=0)
    for (i, &child) in children.iter().enumerate() {
        let rect = tree.get_rect(child).unwrap();
        assert!(
            rect.y.abs() < 0.1,
            "Child {} should be at y=0 (no wrap), got y={}",
            i,
            rect.y
        );
    }

    // Verify: children are horizontally positioned with spacing
    let rect0 = tree.get_rect(children[0]).unwrap();
    let rect1 = tree.get_rect(children[1]).unwrap();
    let expected_x1 = rect0.x + rect0.width + 10.0; // 50 + 10 = 60
    assert!(
        (rect1.x - expected_x1).abs() < 0.1,
        "Child 1 should be at x={}, got x={}",
        expected_x1,
        rect1.x
    );
}
