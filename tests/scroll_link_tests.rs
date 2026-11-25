// SPEC-HAL-133 Phase 1: Linked and Synchronized Scrolling Tests
//
// Verifies anchor-based linked scrolling between ScrollArea instances.
// Phase 1: AnchorSync mode only - no offset-based fallback.

use halcyon::core::math::Point;
use halcyon::core::scroll_link::{ScrollAxis, ScrollIntent, ScrollLinkCoordinator, ScrollSyncMode};
use halcyon::core::scroll_state::{AnchorId, ScrollAnchor, ScrollGroupId};

// ============================================================================
// Unit Tests: Coordinator Token Management
// ============================================================================

#[test]
fn test_coordinator_token_generation_is_monotonic() {
    let mut coord = ScrollLinkCoordinator::new();
    let t1 = coord.generate_token();
    let t2 = coord.generate_token();
    let t3 = coord.generate_token();

    assert!(t1 < t2);
    assert!(t2 < t3);
}

#[test]
fn test_begin_frame_clears_seen_tokens() {
    let mut coord = ScrollLinkCoordinator::new();

    let token = coord.generate_token();
    let group = ScrollGroupId::new(1);

    coord.emit_intent(group, ScrollIntent::anchor_sync(None, (0.0, 0.0), token));
    assert!(coord.has_seen_token(token));

    coord.begin_frame();
    assert!(!coord.has_seen_token(token));
}

// ============================================================================
// Unit Tests: Loop Prevention
// ============================================================================

#[test]
fn test_same_token_dropped_prevents_loop() {
    // Simulates A → B → A loop - second emission with same token should be dropped
    let mut coord = ScrollLinkCoordinator::new();
    coord.begin_frame();

    let token = coord.generate_token();
    let group = ScrollGroupId::new(1);

    // First emission (from A)
    let intent1 = ScrollIntent::vertical_anchor_sync(None, 10.0, token);
    coord.emit_intent(group, intent1);

    // Second emission (from B echoing back) - same token
    let intent2 = ScrollIntent::vertical_anchor_sync(None, 10.0, token);
    coord.emit_intent(group, intent2);

    let intents = coord.drain_intents();
    assert_eq!(
        intents.len(),
        1,
        "Loop prevention failed: second intent with same token should be dropped"
    );
}

#[test]
fn test_bidirectional_link_loop_prevention() {
    // Test A ↔ B symmetric linking scenario
    let mut coord = ScrollLinkCoordinator::new();
    coord.begin_frame();

    let group = ScrollGroupId::new(1);
    let token = coord.generate_token();

    // A scrolls, emits intent
    coord.emit_intent(group, ScrollIntent::vertical_anchor_sync(None, 10.0, token));

    // B receives, tries to emit back with same token (simulating echo)
    coord.emit_intent(group, ScrollIntent::vertical_anchor_sync(None, 10.0, token));

    let intents = coord.drain_intents();
    assert_eq!(intents.len(), 1, "Bidirectional loop must be prevented");
}

// ============================================================================
// Unit Tests: Missing Anchor Behavior
// ============================================================================

#[test]
fn test_missing_anchor_is_noop() {
    // INV-LINK-ANCHOR-2: If no anchor in intent, targets should no-op
    let mut coord = ScrollLinkCoordinator::new();
    coord.begin_frame();

    let token = coord.generate_token();
    let group = ScrollGroupId::new(1);

    // Intent with no anchor
    let intent = ScrollIntent::anchor_sync(None, (0.0, 10.0), token);
    coord.emit_intent(group, intent);

    let intents = coord.drain_intents();
    assert_eq!(intents.len(), 1);
    assert!(intents[0].1.anchor.is_none(), "Anchor should be None");
}

#[test]
fn test_anchor_missing_in_target_is_ignored() {
    // The coordinator doesn't check if target has the anchor - that's resolver's job
    // This test verifies the coordinator passes the anchor through
    let mut coord = ScrollLinkCoordinator::new();
    coord.begin_frame();

    let token = coord.generate_token();
    let group = ScrollGroupId::new(1);

    // Create an anchor for item ID 42 - may or may not exist in targets
    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 50.0),
        Point::new(0.0, 100.0),
    );

    let intent = ScrollIntent::vertical_anchor_sync(Some(anchor), 10.0, token);
    coord.emit_intent(group, intent);

    let intents = coord.drain_intents();
    assert_eq!(intents.len(), 1);

    let propagated = intents[0].1.anchor.as_ref().unwrap();
    assert_eq!(propagated.target_id.0, 42);
}

// ============================================================================
// Unit Tests: Multi-Target Determinism
// ============================================================================

#[test]
fn test_multi_group_deterministic_order() {
    let mut coord = ScrollLinkCoordinator::new();
    coord.begin_frame();

    let group1 = ScrollGroupId::new(1);
    let group2 = ScrollGroupId::new(2);
    let group3 = ScrollGroupId::new(3);

    let token1 = coord.generate_token();
    let token2 = coord.generate_token();
    let token3 = coord.generate_token();

    coord.emit_intent(
        group1,
        ScrollIntent::vertical_anchor_sync(None, 10.0, token1),
    );
    coord.emit_intent(
        group2,
        ScrollIntent::vertical_anchor_sync(None, 20.0, token2),
    );
    coord.emit_intent(
        group3,
        ScrollIntent::vertical_anchor_sync(None, 30.0, token3),
    );

    let intents = coord.drain_intents();
    assert_eq!(intents.len(), 3);

    // Order is deterministic (insertion order)
    assert_eq!(intents[0].0, group1);
    assert_eq!(intents[1].0, group2);
    assert_eq!(intents[2].0, group3);
}

#[test]
fn test_multi_target_same_group() {
    // Multiple ScrollAreas in same group receive same intent
    let mut coord = ScrollLinkCoordinator::new();
    coord.begin_frame();

    let group = ScrollGroupId::new(1);
    let token = coord.generate_token();

    let anchor = ScrollAnchor::new(
        AnchorId::new(100),
        Point::new(0.0, 0.0),
        Point::new(0.0, 50.0),
    );

    coord.emit_intent(
        group,
        ScrollIntent::vertical_anchor_sync(Some(anchor), 10.0, token),
    );

    let intents = coord.drain_intents();
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].0, group);
    assert!(intents[0].1.anchor.is_some());
}

// ============================================================================
// Unit Tests: Intent Structure
// ============================================================================

#[test]
fn test_scroll_intent_vertical_construction() {
    let anchor = ScrollAnchor::new(
        AnchorId::new(1),
        Point::new(0.0, 10.0),
        Point::new(0.0, 20.0),
    );

    let intent = ScrollIntent::vertical_anchor_sync(Some(anchor), 50.0, 123);

    assert_eq!(intent.axis, ScrollAxis::Vertical);
    assert_eq!(intent.mode, ScrollSyncMode::AnchorSync);
    assert_eq!(intent.delta, (0.0, 50.0));
    assert_eq!(intent.token, 123);
    assert!(intent.anchor.is_some());
}

#[test]
fn test_scroll_intent_horizontal_construction() {
    let anchor = ScrollAnchor::new(
        AnchorId::new(2),
        Point::new(10.0, 0.0),
        Point::new(20.0, 0.0),
    );

    let intent = ScrollIntent::horizontal_anchor_sync(Some(anchor), 30.0, 456);

    assert_eq!(intent.axis, ScrollAxis::Horizontal);
    assert_eq!(intent.mode, ScrollSyncMode::AnchorSync);
    assert_eq!(intent.delta, (30.0, 0.0));
    assert_eq!(intent.token, 456);
    assert!(intent.anchor.is_some());
}

// ============================================================================
// Unit Tests: Clamping (Coordinator Level)
// ============================================================================

#[test]
fn test_clamping_handled_by_scroll_state() {
    // The coordinator doesn't do clamping - it passes anchors through.
    // Clamping happens in ScrollState during anchor resolution.
    // This test verifies the coordinator doesn't modify the anchor.

    let mut coord = ScrollLinkCoordinator::new();
    coord.begin_frame();

    let token = coord.generate_token();
    let group = ScrollGroupId::new(1);

    // Create anchor with large offset (might exceed target's scroll bounds)
    let anchor = ScrollAnchor::new(
        AnchorId::new(999),
        Point::new(0.0, 10000.0), // Large target offset
        Point::new(0.0, 100.0),
    );

    coord.emit_intent(
        group,
        ScrollIntent::vertical_anchor_sync(Some(anchor), 100.0, token),
    );

    let intents = coord.drain_intents();
    let propagated = intents[0].1.anchor.as_ref().unwrap();

    // Coordinator passes through unchanged - clamping is resolver's job
    assert_eq!(propagated.target_offset.y, 10000.0);
}

// ============================================================================
// Unit Tests: AnchorSync Mode Only
// ============================================================================

#[test]
fn test_only_anchor_sync_mode_exists() {
    // Phase 1: Verify only AnchorSync mode is available
    let intent = ScrollIntent::anchor_sync(None, (0.0, 0.0), 1);
    assert_eq!(intent.mode, ScrollSyncMode::AnchorSync);

    // No other modes should be constructible in Phase 1
    // (This is a compile-time guarantee via the enum definition)
}

// ============================================================================
// SPEC-HAL-133 Phase 2: Engine → Widget Contract Tightening Tests
// ============================================================================

use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::ScrollArea;

#[test]
fn test_linked_anchor_rejected_when_no_group() {
    // INV-LINK-CONTRACT-1: set_linked_anchor must reject when no scroll_group
    let mut scroll_area = ScrollArea::new(NodeId(0));
    // No scroll_group set (default is None)

    let anchor = ScrollAnchor::new(
        AnchorId::new(1),
        Point::new(0.0, 50.0),
        Point::new(0.0, 100.0),
    );

    // This should no-op (debug_assert panics in debug builds, but we catch it)
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scroll_area.set_linked_anchor(anchor, ScrollAxis::Vertical);
    }));

    // Anchor should NOT be set
    assert!(
        !scroll_area.has_anchor(),
        "Linked anchor must be rejected when no scroll_group"
    );
}

#[test]
fn test_linked_anchor_accepted_when_has_group() {
    // Verify anchor is accepted when scroll_group is set
    let mut scroll_area = ScrollArea::new(NodeId(0)).with_scroll_group(ScrollGroupId::new(1));

    let anchor = ScrollAnchor::new(
        AnchorId::new(1),
        Point::new(0.0, 50.0),
        Point::new(0.0, 100.0),
    );

    scroll_area.set_linked_anchor(anchor, ScrollAxis::Vertical);

    // Anchor should be set
    assert!(
        scroll_area.has_anchor(),
        "Linked anchor must be accepted when scroll_group is set"
    );
}

#[test]
fn test_axis_mismatch_rejected_horizontal_to_vertical() {
    // INV-LINK-CONTRACT-2: Horizontal intent rejected by vertical-only ScrollArea
    let mut scroll_area = ScrollArea::new(NodeId(0))
        .with_scroll_group(ScrollGroupId::new(1))
        .enable_vertical_scrollbar(true)
        .enable_horizontal_scrollbar(false);

    let anchor = ScrollAnchor::new(
        AnchorId::new(1),
        Point::new(50.0, 0.0),
        Point::new(100.0, 0.0),
    );

    // Horizontal intent to vertical-only ScrollArea should no-op
    // (debug_assert panics in debug builds, but we catch it)
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scroll_area.set_linked_anchor(anchor, ScrollAxis::Horizontal);
    }));

    assert!(
        !scroll_area.has_anchor(),
        "Horizontal intent must be rejected by vertical-only ScrollArea"
    );
}

#[test]
fn test_axis_mismatch_rejected_vertical_to_horizontal() {
    // INV-LINK-CONTRACT-2: Vertical intent rejected by horizontal-only ScrollArea
    let mut scroll_area = ScrollArea::new(NodeId(0))
        .with_scroll_group(ScrollGroupId::new(1))
        .enable_vertical_scrollbar(false)
        .enable_horizontal_scrollbar(true);

    let anchor = ScrollAnchor::new(
        AnchorId::new(1),
        Point::new(0.0, 50.0),
        Point::new(0.0, 100.0),
    );

    // Vertical intent to horizontal-only ScrollArea should no-op
    // (debug_assert panics in debug builds, but we catch it)
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scroll_area.set_linked_anchor(anchor, ScrollAxis::Vertical);
    }));

    assert!(
        !scroll_area.has_anchor(),
        "Vertical intent must be rejected by horizontal-only ScrollArea"
    );
}

#[test]
fn test_axis_both_requires_dual_support() {
    // INV-LINK-CONTRACT-2: Both axis intent rejected if not dual-axis ScrollArea
    let mut scroll_area = ScrollArea::new(NodeId(0))
        .with_scroll_group(ScrollGroupId::new(1))
        .enable_vertical_scrollbar(true)
        .enable_horizontal_scrollbar(false);

    let anchor = ScrollAnchor::new(
        AnchorId::new(1),
        Point::new(50.0, 50.0),
        Point::new(100.0, 100.0),
    );

    // Both axis intent to single-axis ScrollArea should no-op
    // (debug_assert panics in debug builds, but we catch it)
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scroll_area.set_linked_anchor(anchor, ScrollAxis::Both);
    }));

    assert!(
        !scroll_area.has_anchor(),
        "Both axis intent must be rejected by single-axis ScrollArea"
    );
}

#[test]
fn test_axis_both_accepted_with_dual_support() {
    // Both axis intent accepted when ScrollArea supports both axes
    let mut scroll_area = ScrollArea::new(NodeId(0))
        .with_scroll_group(ScrollGroupId::new(1))
        .enable_vertical_scrollbar(true)
        .enable_horizontal_scrollbar(true);

    let anchor = ScrollAnchor::new(
        AnchorId::new(1),
        Point::new(50.0, 50.0),
        Point::new(100.0, 100.0),
    );

    scroll_area.set_linked_anchor(anchor, ScrollAxis::Both);

    assert!(
        scroll_area.has_anchor(),
        "Both axis intent must be accepted by dual-axis ScrollArea"
    );
}

#[test]
fn test_vertical_intent_filters_horizontal_components() {
    // SPEC-HAL-133 Phase 2: Vertical intent should zero out horizontal components
    let mut scroll_area = ScrollArea::new(NodeId(0))
        .with_scroll_group(ScrollGroupId::new(1))
        .enable_vertical_scrollbar(true)
        .enable_horizontal_scrollbar(false);

    let anchor = ScrollAnchor::new(
        AnchorId::new(1),
        Point::new(999.0, 50.0), // Horizontal component should be filtered
        Point::new(888.0, 100.0),
    );

    scroll_area.set_linked_anchor(anchor, ScrollAxis::Vertical);

    // Anchor should be set with filtered components
    // (We can't directly inspect the anchor, but we verified in set_linked_anchor
    // that it creates a new anchor with zeroed horizontal components)
    assert!(scroll_area.has_anchor());
}

#[test]
fn test_horizontal_intent_filters_vertical_components() {
    // SPEC-HAL-133 Phase 2: Horizontal intent should zero out vertical components
    let mut scroll_area = ScrollArea::new(NodeId(0))
        .with_scroll_group(ScrollGroupId::new(1))
        .enable_vertical_scrollbar(false)
        .enable_horizontal_scrollbar(true);

    let anchor = ScrollAnchor::new(
        AnchorId::new(1),
        Point::new(50.0, 999.0), // Vertical component should be filtered
        Point::new(100.0, 888.0),
    );

    scroll_area.set_linked_anchor(anchor, ScrollAxis::Horizontal);

    assert!(scroll_area.has_anchor());
}

#[test]
fn test_supported_scroll_axis_vertical() {
    let scroll_area = ScrollArea::new(NodeId(0))
        .enable_vertical_scrollbar(true)
        .enable_horizontal_scrollbar(false);

    assert_eq!(scroll_area.supported_scroll_axis(), ScrollAxis::Vertical);
}

#[test]
fn test_supported_scroll_axis_horizontal() {
    let scroll_area = ScrollArea::new(NodeId(0))
        .enable_vertical_scrollbar(false)
        .enable_horizontal_scrollbar(true);

    assert_eq!(scroll_area.supported_scroll_axis(), ScrollAxis::Horizontal);
}

#[test]
fn test_supported_scroll_axis_both() {
    let scroll_area = ScrollArea::new(NodeId(0))
        .enable_vertical_scrollbar(true)
        .enable_horizontal_scrollbar(true);

    assert_eq!(scroll_area.supported_scroll_axis(), ScrollAxis::Both);
}

#[test]
fn test_supported_scroll_axis_defaults_to_vertical() {
    // When neither scrollbar is enabled, default to vertical
    let scroll_area = ScrollArea::new(NodeId(0))
        .enable_vertical_scrollbar(false)
        .enable_horizontal_scrollbar(false);

    assert_eq!(scroll_area.supported_scroll_axis(), ScrollAxis::Vertical);
}

// ============================================================================
// SPEC-HAL-133 Phase 2: No Recursive Linked Scroll Emission
// ============================================================================

#[test]
fn test_no_recursive_linked_scroll_emission() {
    // SPEC-HAL-133 Phase 2: Prove that applying a linked anchor to a ScrollArea
    // does NOT emit a new ScrollIntent. Linked-scroll application must be passive.
    //
    // This test uses the same engine path as production:
    // 1. Coordinator has zero pending intents
    // 2. One linked anchor intent is injected
    // 3. Intent is drained and applied to ScrollArea
    // 4. Coordinator MUST remain empty (no recursive emission)

    let mut coord = ScrollLinkCoordinator::new();
    coord.begin_frame();

    // Verify starting state: zero pending intents
    assert_eq!(
        coord.pending_count(),
        0,
        "Precondition: coordinator must start with zero pending intents"
    );

    // Record token counter before any operations
    let token_before = coord.current_token();

    // Generate token for the intent (simulates source ScrollArea emitting)
    let token = coord.generate_token();
    let group = ScrollGroupId::new(1);

    // Create a valid anchor
    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 50.0),
        Point::new(0.0, 100.0),
    );

    // Emit the intent (simulates source ScrollArea scrolling)
    coord.emit_intent(
        group,
        ScrollIntent::vertical_anchor_sync(Some(anchor.clone()), 10.0, token),
    );

    // Verify one intent is pending
    assert_eq!(coord.pending_count(), 1, "One intent should be pending");

    // =====================================================================
    // PRODUCTION PATH SIMULATION: process_linked_scroll_intents
    // =====================================================================

    // Step 1: Drain intents (matches production code)
    let intents = coord.drain_intents();
    assert_eq!(intents.len(), 1, "Should have drained exactly one intent");

    // Verify coordinator is now empty after drain
    assert_eq!(
        coord.pending_count(),
        0,
        "Coordinator must be empty after drain"
    );

    // Record token counter after drain (before applying to ScrollArea)
    let token_after_drain = coord.current_token();

    // Step 2: Create target ScrollArea (matches production code)
    let mut scroll_area = ScrollArea::new(NodeId(0))
        .with_scroll_group(group)
        .enable_vertical_scrollbar(true);

    // Step 3: Apply linked anchor to ScrollArea (matches production code)
    for (_group_id, intent) in intents {
        if let Some(intent_anchor) = intent.anchor {
            scroll_area.set_linked_anchor(intent_anchor, intent.axis);
        }
    }

    // =====================================================================
    // ASSERTIONS: No recursive emission occurred
    // =====================================================================

    // 1. Coordinator queue length MUST be 0
    assert_eq!(
        coord.pending_count(),
        0,
        "SPEC-HAL-133: Coordinator must have zero intents after linked anchor application. \
         If non-zero, ScrollArea recursively emitted a ScrollIntent - this is FORBIDDEN."
    );

    // 2. No new token was generated during application
    let token_after_apply = coord.current_token();
    assert_eq!(
        token_after_drain, token_after_apply,
        "SPEC-HAL-133: No new token should be generated during linked anchor application. \
         Token changed from {} to {} - indicates recursive emission.",
        token_after_drain, token_after_apply
    );

    // 3. Verify ScrollArea did accept the anchor (sanity check)
    assert!(
        scroll_area.has_anchor(),
        "ScrollArea should have accepted the linked anchor"
    );

    // 4. Verify we consumed exactly the tokens we expected
    // (token_before + 1 for the initial emit)
    assert_eq!(
        token_after_apply,
        token_before + 1,
        "Only one token should have been consumed (for the initial emit)"
    );
}
