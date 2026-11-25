// SPEC-HAL-133: Linked and Synchronized Scrolling (Phase 1: Anchor-Based)
//
// Engine-level coordinator for propagating scroll anchors between
// linked ScrollArea instances.
//
// Key invariants:
// - INV-LINK-ORDER-1: Link processing occurs before layout_tree()
// - INV-LINK-ORDER-2: Link processing is pure scroll-state mutation
// - INV-LINK-ORDER-3: Link processing must not call anchor resolution
// - INV-LINK-LOOP-1: No derived intent may cause re-entry in same frame
// - INV-LINK-LOOP-2: Symmetric linking via token system only
// - INV-LINK-ANCHOR-1: Anchor sync only; no offset-based fallback
// - INV-LINK-ANCHOR-2: Missing anchor target = no-op for that target
//
// Forbidden patterns:
// - No layout triggers
// - No realization triggers
// - No anchor resolution (that happens after layout)
// - No widget-to-widget direct calls
// - No offset-based fallback
// - No anchor inference

use crate::core::scroll_state::{ScrollAnchor, ScrollGroupId};

// ============================================================================
// Scroll Intent Model
// ============================================================================

/// Scroll axis for linked scrolling.
///
/// SPEC-HAL-133: Anchors can be axis-specific.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
    Both,
}

/// Scroll sync mode.
///
/// SPEC-HAL-133 Phase 1: AnchorSync is the ONLY supported mode.
/// INV-LINK-ANCHOR-1: No offset-based sync, no fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollSyncMode {
    /// Sync via anchor: propagate anchor to targets, each resolves independently.
    /// INV-LINK-ANCHOR-2: If target doesn't have anchor, no-op.
    AnchorSync,
}

/// Scroll intent for linked scrolling.
///
/// SPEC-HAL-133: Represents a scroll action to be propagated to linked targets.
///
/// - `axis`: Which axis this intent affects
/// - `mode`: Sync mode (AnchorSync only in Phase 1)
/// - `anchor`: The scroll anchor to propagate
/// - `delta`: Scroll delta that triggered this intent (for context)
/// - `token`: Monotonic token for loop prevention
#[derive(Debug, Clone)]
pub struct ScrollIntent {
    /// Axis this intent affects
    pub axis: ScrollAxis,
    /// Sync mode (AnchorSync only)
    pub mode: ScrollSyncMode,
    /// The anchor to propagate (if any)
    pub anchor: Option<ScrollAnchor>,
    /// Scroll delta that triggered this intent (informational)
    pub delta: (f32, f32),
    /// Monotonic token for loop prevention
    pub token: u64,
}

impl ScrollIntent {
    /// Create a new anchor-based scroll intent.
    ///
    /// SPEC-HAL-133: AnchorSync mode only.
    pub fn anchor_sync(anchor: Option<ScrollAnchor>, delta: (f32, f32), token: u64) -> Self {
        Self {
            axis: ScrollAxis::Vertical,
            mode: ScrollSyncMode::AnchorSync,
            anchor,
            delta,
            token,
        }
    }

    /// Create a vertical anchor sync intent.
    pub fn vertical_anchor_sync(anchor: Option<ScrollAnchor>, delta_y: f32, token: u64) -> Self {
        Self {
            axis: ScrollAxis::Vertical,
            mode: ScrollSyncMode::AnchorSync,
            anchor,
            delta: (0.0, delta_y),
            token,
        }
    }

    /// Create a horizontal anchor sync intent.
    pub fn horizontal_anchor_sync(anchor: Option<ScrollAnchor>, delta_x: f32, token: u64) -> Self {
        Self {
            axis: ScrollAxis::Horizontal,
            mode: ScrollSyncMode::AnchorSync,
            anchor,
            delta: (delta_x, 0.0),
            token,
        }
    }
}

// ============================================================================
// Scroll Link Coordinator
// ============================================================================

/// Pending intent from a scroll source.
#[derive(Debug, Clone)]
struct PendingIntent {
    /// Source group
    group_id: ScrollGroupId,
    /// The scroll intent
    intent: ScrollIntent,
}

/// Scroll link coordinator for synchronized scrolling.
///
/// SPEC-HAL-133: Engine-owned subsystem that propagates scroll anchors
/// between ScrollArea instances in the same group.
///
/// LIFECYCLE: Engine-owned, lives as long as the app.
/// INVARIANT: Pure coordinator, no geometry ownership.
///
/// FORBID-LINK-SHARED-STATE: ScrollAreas never share ScrollState.
/// FORBID-LINK-WIDGET-COORDINATION: Widgets do not coordinate among themselves.
/// FORBID-LINK-OFFSET-SYNC: No offset-based synchronization (anchor only).
pub struct ScrollLinkCoordinator {
    /// Monotonically increasing token for loop prevention
    next_token: u64,
    /// Pending intents to process this frame
    pending_intents: Vec<PendingIntent>,
    /// Tokens already seen this frame (for loop detection)
    seen_tokens: Vec<u64>,
}

impl Default for ScrollLinkCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollLinkCoordinator {
    /// Create a new coordinator.
    pub const fn new() -> Self {
        Self {
            next_token: 1, // Start at 1 so 0 can mean "no token"
            pending_intents: Vec::new(),
            seen_tokens: Vec::new(),
        }
    }

    /// Begin a new frame.
    ///
    /// SPEC-HAL-133: Clears seen tokens for new cycle.
    /// INV-LINK-LOOP-1: Each frame starts with fresh loop detection.
    pub fn begin_frame(&mut self) {
        self.seen_tokens.clear();
        self.pending_intents.clear();
    }

    /// Generate a new unique token.
    ///
    /// SPEC-HAL-133: Tokens are monotonically increasing.
    /// Used for loop prevention.
    pub fn generate_token(&mut self) -> u64 {
        let token = self.next_token;
        self.next_token = self.next_token.wrapping_add(1);
        token
    }

    /// Get current token value (for reading only).
    pub fn current_token(&self) -> u64 {
        self.next_token
    }

    /// Emit a scroll intent from a source.
    ///
    /// SPEC-HAL-133: Called when a ScrollArea in a group initiates scroll.
    ///
    /// INV-LINK-LOOP-1: Token is recorded to prevent re-processing.
    /// INV-LINK-ANCHOR-1: Only anchor-based intents accepted.
    pub fn emit_intent(&mut self, group_id: ScrollGroupId, intent: ScrollIntent) {
        // INV-LINK-LOOP-1: Check if already processed this token
        if self.seen_tokens.contains(&intent.token) {
            return;
        }

        // Record token as seen
        self.seen_tokens.push(intent.token);

        // Queue intent for processing
        self.pending_intents
            .push(PendingIntent { group_id, intent });
    }

    /// Check if a token has already been seen this frame.
    ///
    /// SPEC-HAL-133: Used for external loop detection.
    pub fn has_seen_token(&self, token: u64) -> bool {
        self.seen_tokens.contains(&token)
    }

    /// Process all pending intents, returning targets and their anchors.
    ///
    /// SPEC-HAL-133: Called before layout_tree().
    ///
    /// Returns: Vec of (group_id, anchor) pairs for ScrollAreas to consume.
    ///
    /// INV-LINK-ORDER-1: Occurs before layout.
    /// INV-LINK-ORDER-2: Pure scroll-state mutation only.
    /// INV-LINK-ORDER-3: No anchor resolution here.
    /// INV-LINK-ANCHOR-2: Missing anchor target = no-op (handled by consumer).
    pub fn drain_intents(&mut self) -> Vec<(ScrollGroupId, ScrollIntent)> {
        self.pending_intents
            .drain(..)
            .map(|p| (p.group_id, p.intent))
            .collect()
    }

    /// Get pending intent count (for testing).
    pub fn pending_count(&self) -> usize {
        self.pending_intents.len()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::math::Point;
    use crate::core::scroll_state::AnchorId;

    #[test]
    fn test_scroll_group_id_equality() {
        let g1 = ScrollGroupId::new(1);
        let g2 = ScrollGroupId::new(1);
        let g3 = ScrollGroupId::new(2);

        assert_eq!(g1, g2);
        assert_ne!(g1, g3);
    }

    #[test]
    fn test_coordinator_token_generation() {
        let mut coord = ScrollLinkCoordinator::new();
        assert_eq!(coord.current_token(), 1);

        let t1 = coord.generate_token();
        assert_eq!(t1, 1);
        assert_eq!(coord.current_token(), 2);

        let t2 = coord.generate_token();
        assert_eq!(t2, 2);
        assert_eq!(coord.current_token(), 3);
    }

    #[test]
    fn test_loop_prevention_same_token_dropped() {
        let mut coord = ScrollLinkCoordinator::new();
        coord.begin_frame();

        let token = coord.generate_token();
        let group = ScrollGroupId::new(1);

        let intent1 = ScrollIntent::vertical_anchor_sync(None, 10.0, token);
        let intent2 = ScrollIntent::vertical_anchor_sync(None, 20.0, token);

        coord.emit_intent(group, intent1);
        coord.emit_intent(group, intent2); // Same token, should be dropped

        let intents = coord.drain_intents();
        assert_eq!(
            intents.len(),
            1,
            "Second intent with same token should be dropped"
        );
    }

    #[test]
    fn test_missing_anchor_is_noop() {
        let mut coord = ScrollLinkCoordinator::new();
        coord.begin_frame();

        let token = coord.generate_token();
        let group = ScrollGroupId::new(1);

        // Intent with no anchor
        let intent = ScrollIntent::anchor_sync(None, (0.0, 10.0), token);
        coord.emit_intent(group, intent);

        let intents = coord.drain_intents();
        assert_eq!(intents.len(), 1);

        // The anchor is None - consumer should no-op
        assert!(intents[0].1.anchor.is_none());
    }

    #[test]
    fn test_anchor_propagation() {
        let mut coord = ScrollLinkCoordinator::new();
        coord.begin_frame();

        let token = coord.generate_token();
        let group = ScrollGroupId::new(1);

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 50.0),
            Point::new(0.0, 100.0),
        );

        let intent = ScrollIntent::vertical_anchor_sync(Some(anchor), 10.0, token);
        coord.emit_intent(group, intent);

        let intents = coord.drain_intents();
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].0, group);
        assert!(intents[0].1.anchor.is_some());

        let propagated = intents[0].1.anchor.as_ref().unwrap();
        assert_eq!(propagated.target_id.0, 42);
    }

    #[test]
    fn test_multi_target_deterministic() {
        let mut coord = ScrollLinkCoordinator::new();
        coord.begin_frame();

        let group1 = ScrollGroupId::new(1);
        let group2 = ScrollGroupId::new(2);

        // Emit intents for two different groups
        let token1 = coord.generate_token();
        let token2 = coord.generate_token();

        coord.emit_intent(
            group1,
            ScrollIntent::vertical_anchor_sync(None, 10.0, token1),
        );
        coord.emit_intent(
            group2,
            ScrollIntent::vertical_anchor_sync(None, 20.0, token2),
        );

        let intents = coord.drain_intents();
        assert_eq!(intents.len(), 2);

        // Order is deterministic (insertion order)
        assert_eq!(intents[0].0, group1);
        assert_eq!(intents[1].0, group2);
    }

    #[test]
    fn test_begin_frame_clears_state() {
        let mut coord = ScrollLinkCoordinator::new();

        // First frame
        let token = coord.generate_token();
        coord.emit_intent(
            ScrollGroupId::new(1),
            ScrollIntent::anchor_sync(None, (0.0, 0.0), token),
        );

        assert_eq!(coord.pending_count(), 1);
        assert!(coord.has_seen_token(token));

        // New frame clears state
        coord.begin_frame();

        assert_eq!(coord.pending_count(), 0);
        assert!(!coord.has_seen_token(token));

        // Same token can be used again
        coord.emit_intent(
            ScrollGroupId::new(1),
            ScrollIntent::anchor_sync(None, (0.0, 0.0), token),
        );
        assert_eq!(coord.pending_count(), 1);
    }

    #[test]
    fn test_bidirectional_link_loop_prevention() {
        // Test A ↔ B scenario
        let mut coord = ScrollLinkCoordinator::new();
        coord.begin_frame();

        let group = ScrollGroupId::new(1);
        let token = coord.generate_token();

        // A scrolls, emits intent
        let intent_from_a = ScrollIntent::vertical_anchor_sync(None, 10.0, token);
        coord.emit_intent(group, intent_from_a);

        // B receives intent, tries to emit back with same token (simulating echo)
        let intent_from_b = ScrollIntent::vertical_anchor_sync(None, 10.0, token);
        coord.emit_intent(group, intent_from_b);

        // Only one intent should be queued (the loop is broken)
        let intents = coord.drain_intents();
        assert_eq!(intents.len(), 1, "Bidirectional loop must be prevented");
    }
}
