// SPEC-HAL-133 Phase 2: Engine Path Verification (Caveat Closure)
//
// Internal test that exercises the REAL `process_linked_scroll_intents()` function.
// This test MUST live in src/tests/ because process_linked_scroll_intents is private.

use crate::core::app::process_linked_scroll_intents;
use crate::core::math::Point;
use crate::core::rect_authority::LayoutAuthorityGuard;
use crate::core::scroll_link::{ScrollIntent, ScrollLinkCoordinator};
use crate::core::scroll_state::{AnchorId, ScrollAnchor, ScrollGroupId};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::prelude::ScrollArea;

#[test]
fn test_process_linked_scroll_intents_no_recursive_emission() {
    let _guard = LayoutAuthorityGuard::enter();
    // SPEC-HAL-133 Phase 2 Caveat Closure: Prove that the REAL engine path
    // `process_linked_scroll_intents()` does not cause recursive emission.
    //
    // This test exercises the actual engine function, not a manual simulation.

    // 1. Create coordinator
    let mut coord = ScrollLinkCoordinator::new();
    coord.begin_frame();

    // 2. Create widget tree with two ScrollAreas in the same group
    let mut tree = WidgetTree::new();
    let group = ScrollGroupId::new(42);

    // Create root ScrollArea
    let scroll_area_1 = ScrollArea::new(NodeId(1))
        .with_scroll_group(group)
        .enable_vertical_scrollbar(true);
    let root_id = tree.create_root(Box::new(scroll_area_1), None, 0);

    // Add second ScrollArea as child (same group)
    let scroll_area_2 = ScrollArea::new(NodeId(2))
        .with_scroll_group(group)
        .enable_vertical_scrollbar(true);
    tree.add_child(root_id, Box::new(scroll_area_2), None, 0);

    // 3. Record token counter BEFORE emitting intent
    let token_before = coord.current_token();

    // 4. Emit one valid linked anchor intent
    let token = coord.generate_token();
    let anchor = ScrollAnchor::new(
        AnchorId::new(100),
        Point::new(0.0, 50.0),
        Point::new(0.0, 100.0),
    );
    coord.emit_intent(
        group,
        ScrollIntent::vertical_anchor_sync(Some(anchor), 10.0, token),
    );

    // Verify intent is pending
    assert_eq!(
        coord.pending_count(),
        1,
        "One intent should be pending before processing"
    );

    // Record token counter AFTER emitting (before processing)
    let token_after_emit = coord.current_token();

    // 5. Call REAL engine path
    process_linked_scroll_intents(&mut coord, &mut tree);

    // =========================================================================
    // ASSERTIONS: No recursive emission from engine path
    // =========================================================================

    // A. Coordinator pending intent count == 0
    assert_eq!(
        coord.pending_count(),
        0,
        "SPEC-HAL-133: Coordinator must have zero pending intents after process_linked_scroll_intents. \
         Non-zero indicates recursive emission from engine path."
    );

    // B. Token counter unchanged after processing
    let token_after_process = coord.current_token();
    assert_eq!(
        token_after_emit, token_after_process,
        "SPEC-HAL-133: Token counter must not change during process_linked_scroll_intents. \
         Changed from {} to {} - indicates recursive emission.",
        token_after_emit, token_after_process
    );

    // C. Verify exactly one token was consumed (the initial emit)
    assert_eq!(
        token_after_process,
        token_before + 1,
        "Only one token should have been consumed total"
    );
}
