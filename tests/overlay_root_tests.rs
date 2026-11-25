// ============================================================================
// SPEC-HAL-127 Stage 1 & 2: OverlayRoot Integration Tests
// ============================================================================
//
// These tests verify that the OverlayRoot structure is properly exported
// and behaves correctly from an integration perspective.
//
// Stage 1 Invariants tested:
// - S1.INV4: Deterministic stack semantics (LIFO)
// - S1.INV2: Modal-only surface (all entries are Modal kind)
// - S1.INV5: Reserved fields exist
//
// Stage 2 Invariants tested:
// - S2.INV1: OverlayRoot mirrors ModalStack exactly (count parity)
// - S2.INV2: Identity stability (stable IDs, mounted_node updates)
// ============================================================================

use halcyon::core::overlay_root::{DismissPolicy, OverlayId, OverlayKind, OverlayRoot};
use halcyon::core::widget_tree::NodeId;

/// S1.INV4: Push and pop follow LIFO semantics.
#[test]
fn push_pop_is_lifo() {
    let mut root = OverlayRoot::new();

    // Push three modals
    let id1 = root.push_modal(None, DismissPolicy::modal_default());
    let id2 = root.push_modal(None, DismissPolicy::modal_default());
    let id3 = root.push_modal(None, DismissPolicy::modal_default());

    assert_eq!(root.len(), 3);
    assert!(!root.is_empty());

    // Pop in reverse order (LIFO)
    let popped3 = root.pop_top_modal().expect("should have overlay");
    assert_eq!(popped3.id, id3);

    let popped2 = root.pop_top_modal().expect("should have overlay");
    assert_eq!(popped2.id, id2);

    let popped1 = root.pop_top_modal().expect("should have overlay");
    assert_eq!(popped1.id, id1);

    // Stack is now empty
    assert!(root.is_empty());
    assert_eq!(root.len(), 0);
    assert!(root.pop_top_modal().is_none());
}

/// S1.INV4: top_id returns the most recently pushed overlay's ID.
#[test]
fn top_modal_id_is_last() {
    let mut root = OverlayRoot::new();

    // Empty stack has no top
    assert!(root.top_id().is_none());
    assert!(root.top().is_none());

    // Push first modal
    let id1 = root.push_modal(None, DismissPolicy::modal_default());
    assert_eq!(root.top_id(), Some(id1));

    // Push second modal - now top
    let id2 = root.push_modal(None, DismissPolicy::modal_default());
    assert_eq!(root.top_id(), Some(id2));

    // Push third modal - now top
    let id3 = root.push_modal(None, DismissPolicy::modal_default());
    assert_eq!(root.top_id(), Some(id3));

    // Pop - id2 is now top
    root.pop_top_modal();
    assert_eq!(root.top_id(), Some(id2));

    // Pop - id1 is now top
    root.pop_top_modal();
    assert_eq!(root.top_id(), Some(id1));

    // Pop - empty
    root.pop_top_modal();
    assert!(root.top_id().is_none());
}

/// S1.INV2: All entries in Stage 1 have Modal kind.
#[test]
fn all_entries_are_modal_kind() {
    let mut root = OverlayRoot::new();

    root.push_modal(None, DismissPolicy::modal_default());
    root.push_modal(None, DismissPolicy::modal_default());
    root.push_modal(None, DismissPolicy::modal_default());

    // All entries should be Modal kind
    for entry in root.iter() {
        assert_eq!(entry.kind, OverlayKind::Modal);
    }

    // Top entry is also Modal
    assert_eq!(root.top().unwrap().kind, OverlayKind::Modal);
}

/// S1.INV5: Reserved fields exist and have expected default values.
#[test]
fn reserved_fields_exist_with_defaults() {
    let mut root = OverlayRoot::new();
    root.push_modal(None, DismissPolicy::modal_default());

    let entry = root.top().expect("should have entry");

    // focus_restore_target: None (not populated in Stage 1)
    assert!(entry.focus_restore_target.is_none());

    // dismiss_policy: modal defaults (on_escape = true, on_outside_click = false)
    assert!(!entry.dismiss_policy.on_outside_click);
    assert!(entry.dismiss_policy.on_escape);

    // blocks_scroll: true for modals
    assert!(entry.blocks_scroll);
}

/// OverlayIds are unique and monotonically increasing.
#[test]
fn overlay_ids_are_unique_and_monotonic() {
    let mut root = OverlayRoot::new();

    let id1 = root.push_modal(None, DismissPolicy::modal_default());
    let id2 = root.push_modal(None, DismissPolicy::modal_default());
    let id3 = root.push_modal(None, DismissPolicy::modal_default());

    // IDs are unique
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);

    // IDs are monotonically increasing
    assert!(id1.raw() < id2.raw());
    assert!(id2.raw() < id3.raw());
}

/// Can look up entries by ID.
#[test]
fn get_by_id() {
    let mut root = OverlayRoot::new();

    let id1 = root.push_modal(Some(NodeId(100)), DismissPolicy::modal_default());
    let id2 = root.push_modal(Some(NodeId(200)), DismissPolicy::modal_default());

    // Can find both entries
    let entry1 = root.get(id1).expect("should find id1");
    assert_eq!(entry1.mounted_node, Some(NodeId(100)));

    let entry2 = root.get(id2).expect("should find id2");
    assert_eq!(entry2.mounted_node, Some(NodeId(200)));

    // Non-existent ID returns None
    let fake_id = OverlayId::from_test_raw(999);
    assert!(root.get(fake_id).is_none());
}

/// Can mutate entries via get_mut.
#[test]
fn get_mut_by_id() {
    let mut root = OverlayRoot::new();

    let id = root.push_modal(None, DismissPolicy::modal_default());

    // Mutate the entry
    {
        let entry = root.get_mut(id).expect("should find entry");
        entry.blocks_scroll = false;
    }

    // Verify mutation persisted
    let entry = root.get(id).expect("should find entry");
    assert!(!entry.blocks_scroll);
}

/// Can associate mounted_node with overlay.
#[test]
fn mounted_node_association() {
    let mut root = OverlayRoot::new();

    let node = NodeId(42);
    let id = root.push_modal(Some(node), DismissPolicy::modal_default());

    let entry = root.get(id).expect("should find entry");
    assert_eq!(entry.mounted_node, Some(node));

    // Can push without mounted_node
    let id2 = root.push_modal(None, DismissPolicy::modal_default());
    let entry2 = root.get(id2).expect("should find entry");
    assert!(entry2.mounted_node.is_none());
}

/// Iteration order is bottom-to-top.
#[test]
fn iteration_order_is_bottom_to_top() {
    let mut root = OverlayRoot::new();

    let id1 = root.push_modal(Some(NodeId(1)), DismissPolicy::modal_default());
    let id2 = root.push_modal(Some(NodeId(2)), DismissPolicy::modal_default());
    let id3 = root.push_modal(Some(NodeId(3)), DismissPolicy::modal_default());

    let ids: Vec<OverlayId> = root.iter().map(|e| e.id).collect();

    assert_eq!(ids, vec![id1, id2, id3]);
}

// ============================================================================
// Stage 2: Lifecycle Parity Tests
// ============================================================================

/// S2.INV1: Opening 3 modals results in OverlayRoot len == 3.
#[test]
fn s2_open_three_modals_len_is_three() {
    let mut root = OverlayRoot::new();

    // Simulate opening 3 modals
    root.push_modal(None, DismissPolicy::modal_default());
    root.push_modal(None, DismissPolicy::modal_default());
    root.push_modal(None, DismissPolicy::modal_default());

    assert_eq!(root.len(), 3);

    // Assert parity with hypothetical ModalStack of len 3
    root.assert_in_sync(3);
}

/// S2.INV1: Closing top twice decrements len correctly.
#[test]
fn s2_close_top_twice_decrements_correctly() {
    let mut root = OverlayRoot::new();

    // Open 3 modals
    root.push_modal(None, DismissPolicy::modal_default());
    root.push_modal(None, DismissPolicy::modal_default());
    root.push_modal(None, DismissPolicy::modal_default());
    assert_eq!(root.len(), 3);

    // Close top once
    root.pop_top_modal();
    assert_eq!(root.len(), 2);
    root.assert_in_sync(2);

    // Close top again
    root.pop_top_modal();
    assert_eq!(root.len(), 1);
    root.assert_in_sync(1);
}

/// S2.INV1.a: Teardown lockstep - pop once per entry maintains parity throughout.
#[test]
fn s2_teardown_lockstep_pop_per_entry() {
    let mut root = OverlayRoot::new();

    // Open several modals
    root.push_modal(None, DismissPolicy::modal_default());
    root.push_modal(None, DismissPolicy::modal_default());
    root.push_modal(None, DismissPolicy::modal_default());
    root.push_modal(None, DismissPolicy::modal_default());

    let mut simulated_modal_stack_len = 4;
    root.assert_in_sync(simulated_modal_stack_len);

    // Simulate close_all_modals teardown loop: pop once per entry
    while !root.is_empty() {
        root.pop_top_modal();
        simulated_modal_stack_len -= 1;
        // Parity must hold after each iteration
        root.assert_in_sync(simulated_modal_stack_len);
    }

    assert!(root.is_empty());
    assert_eq!(simulated_modal_stack_len, 0);
}

/// S2.INV1: No double-pop when stack is empty.
#[test]
fn s2_no_double_pop_on_empty() {
    let mut root = OverlayRoot::new();

    // Open and close one modal
    root.push_modal(None, DismissPolicy::modal_default());
    root.pop_top_modal();

    // Try to pop again - should return None, not panic
    assert!(root.pop_top_modal().is_none());
    assert_eq!(root.len(), 0);

    // Pop on empty is safe (no panic)
    assert!(root.pop_top_modal().is_none());
}

/// S2.INV1: Interleaved push/pop maintains correct count.
#[test]
fn s2_interleaved_push_pop_maintains_count() {
    let mut root = OverlayRoot::new();

    // Simulate realistic modal usage pattern
    root.push_modal(None, DismissPolicy::modal_default()); // Open modal 1
    assert_eq!(root.len(), 1);

    root.push_modal(None, DismissPolicy::modal_default()); // Open modal 2
    assert_eq!(root.len(), 2);

    root.pop_top_modal(); // Close modal 2
    assert_eq!(root.len(), 1);

    root.push_modal(None, DismissPolicy::modal_default()); // Open modal 3
    assert_eq!(root.len(), 2);

    root.push_modal(None, DismissPolicy::modal_default()); // Open modal 4
    assert_eq!(root.len(), 3);

    root.pop_top_modal(); // Close modal 4
    root.pop_top_modal(); // Close modal 3
    root.pop_top_modal(); // Close modal 1
    assert_eq!(root.len(), 0);
    assert!(root.is_empty());
}

/// S2.INV2: update_top_mounted_node updates the correct entry.
#[test]
fn s2_update_top_mounted_node() {
    let mut root = OverlayRoot::new();

    // Push modals without mounted_node
    let id1 = root.push_modal(None, DismissPolicy::modal_default());
    let id2 = root.push_modal(None, DismissPolicy::modal_default());

    // Update top (id2) mounted_node
    root.update_top_mounted_node(Some(NodeId(200)));

    // Verify id2 was updated
    assert_eq!(root.get(id2).unwrap().mounted_node, Some(NodeId(200)));

    // Verify id1 was NOT updated
    assert!(root.get(id1).unwrap().mounted_node.is_none());
}

/// S2.INV2: IDs remain stable across operations.
#[test]
fn s2_ids_remain_stable() {
    let mut root = OverlayRoot::new();

    let id1 = root.push_modal(None, DismissPolicy::modal_default());
    let id2 = root.push_modal(None, DismissPolicy::modal_default());
    let id3 = root.push_modal(None, DismissPolicy::modal_default());

    // Pop and verify IDs match
    let popped3 = root.pop_top_modal().unwrap();
    assert_eq!(popped3.id, id3);

    let popped2 = root.pop_top_modal().unwrap();
    assert_eq!(popped2.id, id2);

    let popped1 = root.pop_top_modal().unwrap();
    assert_eq!(popped1.id, id1);
}

// ============================================================================
// Stage 3: Routing Authority Tests
// ============================================================================

/// S3.INV2: OverlayRoot is_empty() matches expected modal presence.
///
/// This test verifies that routing code can use overlay_root.is_empty()
/// as the authority for modal presence checks.
#[test]
fn s3_overlay_root_is_authority_for_presence() {
    let mut root = OverlayRoot::new();

    // Empty state
    assert!(root.is_empty());

    // After push, not empty
    root.push_modal(None, DismissPolicy::modal_default());
    assert!(!root.is_empty());

    // After pop, empty again
    root.pop_top_modal();
    assert!(root.is_empty());
}

/// S3.INV2: OverlayRoot top_id() provides topmost overlay identity.
///
/// Routing code uses this to identify which overlay is topmost without
/// mutating any stack.
#[test]
fn s3_overlay_root_provides_top_identity() {
    let mut root = OverlayRoot::new();

    // No top when empty
    assert!(root.top_id().is_none());

    // Push and verify top changes
    let id1 = root.push_modal(None, DismissPolicy::modal_default());
    assert_eq!(root.top_id(), Some(id1));

    let id2 = root.push_modal(None, DismissPolicy::modal_default());
    assert_eq!(root.top_id(), Some(id2));

    // Pop and verify top updates
    root.pop_top_modal();
    assert_eq!(root.top_id(), Some(id1));
}

/// S3.INV1/S3.INV3: Shielding check pattern is read-only.
///
/// This test demonstrates the correct shielding check pattern:
/// 1. Check overlay_root.is_empty() for presence
/// 2. Access modal via read-only accessor
/// 3. No mutation occurs
///
/// Note: The actual shielding check is in AppRoot::handle_fused_input_modal_first.
/// This test validates the invariant by simulating the pattern and asserting
/// that overlay_root state is unchanged after the check.
#[test]
fn s3_shielding_check_is_readonly() {
    let mut root = OverlayRoot::new();

    // Setup: push some modals
    let id1 = root.push_modal(Some(NodeId(100)), DismissPolicy::modal_default());
    let id2 = root.push_modal(Some(NodeId(200)), DismissPolicy::modal_default());
    let id3 = root.push_modal(Some(NodeId(300)), DismissPolicy::modal_default());

    // Capture state before "shielding check"
    let len_before = root.len();
    let top_id_before = root.top_id();

    // Simulate shielding check pattern (read-only operations)
    let _has_modal = !root.is_empty();
    let _top = root.top();
    let _top_id = root.top_id();

    // Verify state unchanged after read-only check
    assert_eq!(
        root.len(),
        len_before,
        "S3.INV3: len must not change during shielding check"
    );
    assert_eq!(
        root.top_id(),
        top_id_before,
        "S3.INV3: top must not change during shielding check"
    );

    // Verify all entries still accessible in correct order
    let ids: Vec<OverlayId> = root.iter().map(|e| e.id).collect();
    assert_eq!(
        ids,
        vec![id1, id2, id3],
        "S3.INV3: order must not change during shielding check"
    );
}

/// S3.INV4: Scroll and pointer shielding use the same authority.
///
/// This is a documentation test - the actual implementation uses the same
/// code path for both. Here we verify that OverlayRoot provides the same
/// answer regardless of which event type triggered the check.
#[test]
fn s3_scroll_and_pointer_use_same_authority() {
    let mut root = OverlayRoot::new();

    // The authority check is the same for scroll and pointer:
    // overlay_root.is_empty() determines if shielding applies

    // Empty: no shielding for either event type
    assert!(root.is_empty());

    // With modal: shielding applies for both event types
    root.push_modal(None, DismissPolicy::modal_default());
    assert!(!root.is_empty());

    // The covers_point check is done via ModalStack.top_entry() in the
    // actual implementation, but OverlayRoot.is_empty() is the gate.
}

// ============================================================================
// Stage 4: Dismissal Policy Tests
// ============================================================================

/// S4.INV1: DismissPolicy is set at push time and persists.
#[test]
fn s4_dismiss_policy_set_at_push() {
    let mut root = OverlayRoot::new();

    // Modal with default policy (on_escape: true, on_outside_click: false)
    root.push_modal(None, DismissPolicy::modal_default());
    let entry = root.top().expect("should have entry");
    assert!(entry.dismiss_policy.on_escape);
    assert!(!entry.dismiss_policy.on_outside_click);
}

/// S4.INV1: Popup policy enables outside-click dismissal.
#[test]
fn s4_popup_policy_enables_outside_click() {
    let mut root = OverlayRoot::new();

    // Push with popup policy
    root.push_modal(None, DismissPolicy::popup_default());
    let entry = root.top().expect("should have entry");
    assert!(entry.dismiss_policy.on_outside_click);
    assert!(entry.dismiss_policy.on_escape);
}

/// S4.INV1: Routing can query dismiss policy from top overlay.
#[test]
fn s4_routing_can_query_policy() {
    let mut root = OverlayRoot::new();

    // Initially no policy to query
    assert!(root.top().is_none());

    // After push, policy is queryable
    root.push_modal(None, DismissPolicy::popup_default());
    let on_outside = root
        .top()
        .map(|e| e.dismiss_policy.on_outside_click)
        .unwrap_or(false);
    let on_escape = root
        .top()
        .map(|e| e.dismiss_policy.on_escape)
        .unwrap_or(false);
    assert!(on_outside);
    assert!(on_escape);

    // After pop, no policy
    root.pop_top_modal();
    assert!(root.top().is_none());
}

/// S4.INV3: Parity preserved after dismissal-related operations.
#[test]
fn s4_parity_after_dismiss_operations() {
    let mut root = OverlayRoot::new();

    // Open modals with different policies
    root.push_modal(None, DismissPolicy::modal_default());
    root.push_modal(None, DismissPolicy::popup_default());
    root.push_modal(None, DismissPolicy::modal_default());
    root.assert_in_sync(3);

    // Simulate dismissal (close one modal)
    root.pop_top_modal();
    root.assert_in_sync(2);

    // Remaining modals have correct policies
    let top = root.top().unwrap();
    assert!(top.dismiss_policy.on_outside_click); // popup_default was second
}

// ============================================================================
// Test Helpers
// ============================================================================

/// Extension trait for test-only construction of OverlayId.
trait OverlayIdTestExt {
    fn from_test_raw(raw: u64) -> OverlayId;
}

impl OverlayIdTestExt for OverlayId {
    fn from_test_raw(raw: u64) -> OverlayId {
        // This is a bit of a hack for testing - we create an OverlayId
        // by going through the normal push/pop cycle
        // For actual testing, we just compare against IDs we got from push_modal
        // This helper is only used for the "not found" case
        unsafe { std::mem::transmute(raw) }
    }
}
