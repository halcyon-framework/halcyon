// ============================================================================
// SPEC-HAL-127: Overlay Stack & Input Capture Contract
// Stage 4: Centralized Dismissal Policy
// ============================================================================
//
// This module defines the **single authoritative overlay stack** for the
// Halcyon application framework. Stage 4 makes OverlayRoot the sole owner
// of dismissal policy (outside-click, Escape), preventing modals from
// deciding when to close themselves.
//
// **Reference SPECs:**
// - SPEC-HAL-127: Overlay Stack & Input Capture Contract (primary)
// - SPEC-HAL-128: Pointer Capture & Gesture Arbitration
// - SPEC-HAL-126: Focus Traversal & Tab Order Contract
// - SPEC-HAL-110: Unified Scroll Overlay & Viewport Contract
// - SPEC-HAL-018: App Framework (ScreenStack/ModalStack)
// - SPEC-HAL-038: Focus/Input Routing
// - SPEC-HAL-089: Focus Memory Policy
//
// ============================================================================
// STAGE 1 INVARIANTS — Mechanically Checkable
// ============================================================================
//
// **S1.INV1 — Single Authority Container**
//
//   `OverlayRoot` is the only owner of overlay ordering state (stack vector).
//   `ModalStack` remains storage of modal screens; it is **not** a router.
//   No other component may maintain overlay stacking order.
//
// **S1.INV2 — Modal-Only Surface**
//
//   `OverlayRoot` entries in Stage 1 can represent only existing modals.
//   No popups, menus, or tooltips yet.
//   `OverlayKind::Modal` is the only valid kind in Stage 1.
//
// **S1.INV3 — No Behavior Wiring Yet**
//
//   `OverlayRoot` is not queried by routing.
//   `OverlayRoot` does not consume or transform input.
//   `OverlayRoot` does not mutate focus or scroll.
//   All existing modal routing behavior is unchanged.
//
// **S1.INV4 — Deterministic Stack Semantics**
//
//   `push_modal(...)` appends to the stack.
//   `pop_top_modal(...)` removes from the end.
//   No mid-stack removal API in Stage 1.
//   Stack is always LIFO.
//
// **S1.INV5 — Bookkeeping Fields Reserved**
//
//   Each `OverlayEntry` contains reserved fields (unused in Stage 1):
//   - `focus_restore_target: Option<TreeWidgetId>` — for focus restoration
//   - `dismiss_policy: DismissPolicy` — for outside-click/Escape handling
//   - `blocks_scroll: bool` — for scroll isolation
//
//   These fields exist for Stage 2+; logic is not implemented yet.
//
// ============================================================================
// STAGE 2 INVARIANTS — Modal Lifecycle Wiring
// ============================================================================
//
// **S2.INV1 — OverlayRoot Mirrors ModalStack Exactly**
//
//   For every modal open, an OverlayRoot entry is pushed **exactly once**.
//   For every modal close (top), the corresponding OverlayRoot entry is
//   popped **exactly once**.
//   OverlayRoot modal count equals ModalStack modal count at all times
//   after lifecycle operations.
//
// **S2.INV1.a — Teardown Lockstep**
//
//   During `close_all_modals()`, OverlayRoot must be popped **once per
//   drained modal**, maintaining parity throughout the teardown loop.
//   No bulk pre-clear. Each pop happens in the same iteration as the
//   corresponding ModalStack entry teardown.
//
// **S2.INV2 — Identity Stability**
//
//   Each modal entry has a stable OverlayId for its lifetime.
//   OverlayRoot entry stores a stable handle to the mounted node
//   (or a stable representation) sufficient for later routing.
//
// **S2.INV3 — No Routing Authority Yet**
//
//   EventRouter and App routing do not query OverlayRoot.
//   OverlayRoot performs no hit-testing, no input consumption,
//   no focus mutation.
//
// **S2.INV4 — No "Look-Aside" Modal Order**
//
//   No other new stack/order cache is introduced.
//   If a "mounted_nodes()" list is used anywhere, it must remain derived
//   from ModalStack (until Stage 3), not duplicated.
//
// ============================================================================
// STAGE 3 INVARIANTS — OverlayRoot as Sole Routing Authority
// ============================================================================
//
// **S3.INV1 — Single Modal Shielding Path**
//
//   There is exactly one code path that decides whether pointer/scroll
//   events are blocked by the top modal.
//   The old "pop_entry → covers_point → open_entry" probing path must not
//   exist anywhere.
//
// **S3.INV2 — OverlayRoot Is Routing Authority**
//
//   App-level modal-first routing and shielding must consult **OverlayRoot**
//   to determine:
//   - whether any overlay exists
//   - which overlay is topmost
//   ModalStack may provide the **Screen storage**, but must not be used as
//   the ordering/authority source in routing logic.
//
// **S3.INV3 — Zero ModalStack Mutation During Shielding**
//
//   Shielding checks must not mutate ModalStack (no pop/reopen, no temporary
//   removal).
//
// **S3.INV4 — Scroll Isolation Uses Same Authority**
//
//   Wheel/scroll shielding uses the same OverlayRoot-based path as pointer
//   shielding.
//
// ============================================================================
// STAGE 4 INVARIANTS — Centralized Dismissal Policy
// ============================================================================
//
// **S4.INV1 — OverlayRoot Owns Dismissal Policy**
//
//   Whether an overlay dismisses on outside click or Escape is decided **only**
//   by OverlayRoot. Modal `Screen::handle_input()` must not directly call
//   `nav_close_modal()` for outside clicks or Escape key presses.
//
// **S4.INV2 — Read-Only Modal Code**
//
//   Modal screens may *observe* pointer location.
//   Modal screens may *consume* events.
//   Modal screens must not *decide dismissal*.
//
// **S4.INV3 — Behavior Preservation**
//
//   ActionPalette and existing modals must behave exactly as before:
//   - Outside click still dismisses where it used to.
//   - Escape still dismisses where it used to.
//   No new dismissals appear. No dismissals disappear.
//
// **S4.INV4 — One Dismissal Decision Per Event**
//
//   For a given input event:
//   - Either OverlayRoot dismisses the top overlay
//   - Or it does not
//   No duplicate close calls. No reentrancy.
//
// ============================================================================
// STAGE 4.2 INVARIANTS — Overlay ↔ Widget Lifecycle Coupling
// ============================================================================
//
// **S4.2.INV1 — Modal Close → Widget Unmount Ordering**
//
//   When a modal is closed (by any path):
//     1. close_modal() is called exactly once
//     2. the modal widget is scheduled for removal
//     3. WidgetTree unmount occurs
//     4. ONLY AFTER unmount does OverlayRoot.pop_top_modal() take effect
//
//   OverlayRoot must never observe a modal as removed
//   while its widget is still mounted.
//
//
// **S4.2.INV2 — on_unmount Is Single-Shot**
//
//   For each modal widget:
//     - on_mount fires exactly once
//     - on_unmount fires exactly once
//     - never duplicated
//
//   No dismissal path may cause double unmount.
//
//
// **S4.2.INV3 — Focus Restore Happens After Unmount**
//
//   Any focus restoration triggered by modal close must occur
//   strictly AFTER the modal widget has been unmounted.
//
//   Focus must never be restored to a widget that is still mounted
//   under an active overlay.
//
//
// **S4.2.INV4 — OverlayRoot Never Holds Dead NodeIds**
//
//   OverlayRoot.mounted_node:
//     - MUST be Some(NodeId) only while widget is mounted
//     - MUST be cleared during teardown before next frame
//
//   OverlayRoot must never reference a NodeId that no longer exists.
//
//
// **S4.2.INV5 — close_all_modals Preserves Per-Modal Ordering**
//
//   During close_all_modals():
//     - teardown proceeds top → bottom
//     - each modal completes full lifecycle before next begins
//     - no bulk clearing of OverlayRoot or WidgetTree
//
// ============================================================================
//
// ============================================================================
// SPEC-HAL-127 STAGE 4.3 INVARIANTS — Immediate Parity & Deferred Widget Cleanup
// ============================================================================
//
// **S4.3.INV1 — Immediate Parity**
//
//   After any close operation, `overlay_root.len() == modals.len()` must hold
//   immediately. Both stacks are updated atomically within the close call.
//
//
// **S4.3.INV2 — OverlayRoot Represents Logical Modal State**
//
//   OverlayRoot entry is removed immediately on close; it does NOT wait for
//   widget unmount. OverlayRoot reflects intent, not widget tree state.
//
//
// **S4.3.INV3 — Deferred WidgetTree Cleanup Only**
//
//   Only `WidgetTree::remove_node(NodeId)` is deferred to `rebuild_active_view()`
//   via `pending_widget_removals`. This prevents tree mutation during event
//   handling while ensuring cleanup before the next frame.
//
//
// **S4.3.INV4 — Routing Uses OverlayRoot State**
//
//   After close, routing/shielding must observe "no modal" via OverlayRoot,
//   even if the widget node still exists until cleanup. OverlayRoot is the
//   routing authority (S3.INV2), not the widget tree.
//
//
// **S4.3.INV5 — No Stale Overlay References**
//
//   OverlayRoot must not retain any entries/ids after close. Any leftover
//   mounted widget nodes are cleaned by `pending_widget_removals` in
//   `rebuild_active_view()`.
//
// ============================================================================

use crate::core::widget_tree::NodeId;
use crate::utils::id::TreeWidgetId;

// ============================================================================
// OverlayId — Stable Identifier
// ============================================================================

/// Stable identifier for an overlay in the stack.
///
/// Each overlay receives a unique, monotonically increasing ID when pushed.
/// IDs are never reused within a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayId(u64);

impl OverlayId {
    /// Get the raw u64 value (for debugging/logging).
    pub fn raw(self) -> u64 {
        self.0
    }
}

// ============================================================================
// OverlayKind — Overlay Type Discriminant
// ============================================================================

/// Kind of overlay in the stack.
///
/// Stage 1 supports only `Modal`. Future stages will add:
/// - `Popup` — dropdown popups, popovers
/// - `Menu` — context menus, menu bar menus
/// - `Tooltip` — non-capturing informational overlays
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    /// Modal dialog overlay (captures input, traps focus).
    Modal,
    // Future: Popup, Menu, Tooltip
}

impl Default for OverlayKind {
    fn default() -> Self {
        OverlayKind::Modal
    }
}

// ============================================================================
// DismissPolicy — Reserved for Stage 2+
// ============================================================================

/// Policy for overlay dismissal behavior.
///
/// Reserved for Stage 2+. Currently unused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DismissPolicy {
    /// Dismiss on click outside the overlay bounds.
    pub on_outside_click: bool,
    /// Dismiss on Escape key.
    pub on_escape: bool,
}

impl DismissPolicy {
    /// Default modal policy: dismiss on Escape only.
    pub fn modal_default() -> Self {
        Self {
            on_outside_click: false,
            on_escape: true,
        }
    }

    /// Popup policy: dismiss on outside click and Escape.
    pub fn popup_default() -> Self {
        Self {
            on_outside_click: true,
            on_escape: true,
        }
    }
}

// ============================================================================
// OverlayEntry — Single Overlay in the Stack
// ============================================================================

/// A single entry in the overlay stack.
///
/// Contains the overlay metadata and reserved bookkeeping fields for
/// Stage 2+ behavior (focus restoration, dismissal, scroll blocking).
#[derive(Debug)]
pub struct OverlayEntry {
    /// Unique identifier for this overlay.
    pub id: OverlayId,

    /// Kind of overlay (Modal, Popup, Menu, Tooltip).
    pub kind: OverlayKind,

    /// Widget tree node for this overlay's root widget.
    ///
    /// Links OverlayRoot to the existing ModalStack/ScreenEntry system.
    /// May be None if overlay is not yet mounted.
    pub mounted_node: Option<NodeId>,

    // ========================================================================
    // Reserved Fields (S1.INV5) — Unused in Stage 1
    // ========================================================================
    /// Widget that had focus before this overlay opened.
    /// Used for focus restoration on dismissal.
    ///
    /// **Stage 1:** Reserved, not populated or used.
    pub focus_restore_target: Option<TreeWidgetId>,

    /// Dismissal policy for this overlay.
    ///
    /// **Stage 1:** Reserved, not checked.
    pub dismiss_policy: DismissPolicy,

    /// Whether this overlay blocks scroll events to lower layers.
    ///
    /// **Stage 1:** Reserved, not enforced.
    pub blocks_scroll: bool,
}

impl OverlayEntry {
    /// Create a new modal overlay entry.
    ///
    /// S4.INV1: DismissPolicy is provided at creation time, not decided by the modal.
    fn new_modal(
        id: OverlayId,
        mounted_node: Option<NodeId>,
        dismiss_policy: DismissPolicy,
    ) -> Self {
        Self {
            id,
            kind: OverlayKind::Modal,
            mounted_node,
            focus_restore_target: None,
            dismiss_policy,
            blocks_scroll: true,
        }
    }
}

// ============================================================================
// OverlayRoot — The Single Authority Container
// ============================================================================

/// SPEC-HAL-127: The single authoritative overlay stack for the application.
///
/// `OverlayRoot` owns the ordered stack of active overlays. It is the **only**
/// component that may determine overlay stacking order.
///
/// **Stage 1:** Structure only. No input routing, focus management, or scroll
/// blocking behavior is implemented. The existing `ModalStack` and modal-first
/// routing remain unchanged.
///
/// **Stage 2+:** Will wire input routing, focus containment, and dismissal
/// through this structure.
#[derive(Debug)]
pub struct OverlayRoot {
    /// Ordered stack of overlay entries (bottom → top).
    ///
    /// Index 0 = bottom-most overlay (opened first).
    /// Last index = top-most overlay (receives input).
    stack: Vec<OverlayEntry>,

    /// Next ID to assign.
    next_id: u64,
}

impl OverlayRoot {
    /// Create a new empty overlay root.
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            next_id: 1,
        }
    }

    /// Returns true if the overlay stack is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Returns the number of overlays in the stack.
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Returns the ID of the topmost overlay, if any.
    pub fn top_id(&self) -> Option<OverlayId> {
        self.stack.last().map(|e| e.id)
    }

    /// Returns a reference to the topmost overlay entry, if any.
    pub fn top(&self) -> Option<&OverlayEntry> {
        self.stack.last()
    }

    /// Returns a mutable reference to the topmost overlay entry, if any.
    pub fn top_mut(&mut self) -> Option<&mut OverlayEntry> {
        self.stack.last_mut()
    }

    // ========================================================================
    // Modal Operations (S1.INV4 — Deterministic Stack Semantics)
    // ========================================================================

    /// Push a new modal overlay onto the stack.
    ///
    /// Returns the assigned `OverlayId` for the new overlay.
    ///
    /// **S1.INV4:** Appends to the stack (LIFO).
    /// **S4.INV1:** DismissPolicy is set at push time, not by the modal.
    pub fn push_modal(
        &mut self,
        mounted_node: Option<NodeId>,
        dismiss_policy: DismissPolicy,
    ) -> OverlayId {
        let id = OverlayId(self.next_id);
        self.next_id += 1;
        self.stack
            .push(OverlayEntry::new_modal(id, mounted_node, dismiss_policy));
        id
    }

    /// Pop the top modal overlay from the stack.
    ///
    /// Returns the removed `OverlayEntry`, or `None` if stack is empty.
    ///
    /// **S1.INV4:** Removes from the end (LIFO).
    /// **S2.INV1.a:** Used by close_all_modals in a loop to maintain teardown lockstep.
    pub fn pop_top_modal(&mut self) -> Option<OverlayEntry> {
        self.stack.pop()
    }

    /// Update the mounted_node for the topmost overlay.
    ///
    /// Called during rebuild when the modal widget is mounted to the tree.
    ///
    /// **S2.INV2:** Maintains identity stability between OverlayRoot and tree.
    pub fn update_top_mounted_node(&mut self, node: Option<NodeId>) {
        if let Some(entry) = self.stack.last_mut() {
            entry.mounted_node = node;
        }
    }

    // ========================================================================
    // Debug Assertions (S2.INV1)
    // ========================================================================

    /// Assert that OverlayRoot is in sync with ModalStack.
    ///
    /// **S2.INV1:** OverlayRoot modal count must equal ModalStack modal count.
    ///
    /// This is a debug-only assertion; no effect in release builds.
    #[inline]
    pub fn assert_in_sync(&self, modal_stack_len: usize) {
        debug_assert_eq!(
            self.stack.len(),
            modal_stack_len,
            "S2.INV1 violation: OverlayRoot len ({}) != ModalStack len ({})",
            self.stack.len(),
            modal_stack_len
        );
    }

    // ========================================================================
    // Query Operations
    // ========================================================================

    /// Iterate over all overlay entries from bottom to top.
    pub fn iter(&self) -> impl Iterator<Item = &OverlayEntry> {
        self.stack.iter()
    }

    /// Iterate over all overlay entries mutably from bottom to top.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut OverlayEntry> {
        self.stack.iter_mut()
    }

    /// Find an overlay by its ID.
    pub fn get(&self, id: OverlayId) -> Option<&OverlayEntry> {
        self.stack.iter().find(|e| e.id == id)
    }

    /// Find an overlay by its ID (mutable).
    pub fn get_mut(&mut self, id: OverlayId) -> Option<&mut OverlayEntry> {
        self.stack.iter_mut().find(|e| e.id == id)
    }
}

impl Default for OverlayRoot {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// S1.INV4: Push/pop is LIFO.
    #[test]
    fn push_pop_is_lifo() {
        let mut root = OverlayRoot::new();

        let id1 = root.push_modal(None, DismissPolicy::modal_default());
        let id2 = root.push_modal(None, DismissPolicy::modal_default());
        let id3 = root.push_modal(None, DismissPolicy::modal_default());

        assert_eq!(root.len(), 3);

        // Pop in reverse order
        let popped3 = root.pop_top_modal().unwrap();
        assert_eq!(popped3.id, id3);

        let popped2 = root.pop_top_modal().unwrap();
        assert_eq!(popped2.id, id2);

        let popped1 = root.pop_top_modal().unwrap();
        assert_eq!(popped1.id, id1);

        assert!(root.is_empty());
        assert!(root.pop_top_modal().is_none());
    }

    /// S1.INV4: top_id returns the last pushed overlay.
    #[test]
    fn top_modal_id_is_last() {
        let mut root = OverlayRoot::new();

        assert!(root.top_id().is_none());

        let id1 = root.push_modal(None, DismissPolicy::modal_default());
        assert_eq!(root.top_id(), Some(id1));

        let id2 = root.push_modal(None, DismissPolicy::modal_default());
        assert_eq!(root.top_id(), Some(id2));

        root.pop_top_modal();
        assert_eq!(root.top_id(), Some(id1));

        root.pop_top_modal();
        assert!(root.top_id().is_none());
    }

    /// S1.INV2: All entries in Stage 1 are Modal kind.
    #[test]
    fn all_entries_are_modal_kind() {
        let mut root = OverlayRoot::new();

        root.push_modal(None, DismissPolicy::modal_default());
        root.push_modal(None, DismissPolicy::modal_default());

        for entry in root.iter() {
            assert_eq!(entry.kind, OverlayKind::Modal);
        }
    }

    /// S1.INV5: Reserved fields exist with default values.
    #[test]
    fn reserved_fields_exist() {
        let mut root = OverlayRoot::new();
        root.push_modal(None, DismissPolicy::modal_default());

        let entry = root.top().unwrap();

        // focus_restore_target is None by default
        assert!(entry.focus_restore_target.is_none());

        // dismiss_policy has modal defaults
        assert!(!entry.dismiss_policy.on_outside_click);
        assert!(entry.dismiss_policy.on_escape);

        // blocks_scroll is true for modals
        assert!(entry.blocks_scroll);
    }

    /// IDs are unique and monotonically increasing.
    #[test]
    fn ids_are_unique_and_monotonic() {
        let mut root = OverlayRoot::new();

        let id1 = root.push_modal(None, DismissPolicy::modal_default());
        let id2 = root.push_modal(None, DismissPolicy::modal_default());
        let id3 = root.push_modal(None, DismissPolicy::modal_default());

        assert!(id1.raw() < id2.raw());
        assert!(id2.raw() < id3.raw());
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    /// Can associate mounted_node with overlay.
    #[test]
    fn mounted_node_association() {
        let mut root = OverlayRoot::new();

        let node = NodeId(42);
        let id = root.push_modal(Some(node), DismissPolicy::modal_default());

        let entry = root.get(id).unwrap();
        assert_eq!(entry.mounted_node, Some(node));
    }

    // ========================================================================
    // Stage 2: Lifecycle Parity Tests
    // ========================================================================

    /// S2.INV1.a: Teardown lockstep - pop once per entry maintains parity.
    #[test]
    fn teardown_lockstep_pop_per_entry() {
        let mut root = OverlayRoot::new();

        // Simulate opening 4 modals
        root.push_modal(None, DismissPolicy::modal_default());
        root.push_modal(None, DismissPolicy::modal_default());
        root.push_modal(None, DismissPolicy::modal_default());
        root.push_modal(None, DismissPolicy::modal_default());

        let mut simulated_modal_stack_len = 4;
        root.assert_in_sync(simulated_modal_stack_len);

        // Simulate close_all_modals teardown loop
        while !root.is_empty() {
            // Pop OverlayRoot in lockstep
            root.pop_top_modal();
            simulated_modal_stack_len -= 1;

            // After each pop, parity must hold
            root.assert_in_sync(simulated_modal_stack_len);
        }

        // Both must be empty
        assert!(root.is_empty());
        assert_eq!(simulated_modal_stack_len, 0);
    }

    /// S2.INV2: update_top_mounted_node works correctly.
    #[test]
    fn update_top_mounted_node_works() {
        let mut root = OverlayRoot::new();

        // Push without mounted_node
        let id = root.push_modal(None, DismissPolicy::modal_default());
        assert!(root.get(id).unwrap().mounted_node.is_none());

        // Update mounted_node
        root.update_top_mounted_node(Some(NodeId(100)));
        assert_eq!(root.get(id).unwrap().mounted_node, Some(NodeId(100)));

        // Update to None
        root.update_top_mounted_node(None);
        assert!(root.get(id).unwrap().mounted_node.is_none());
    }

    /// S2.INV1: assert_in_sync passes when in sync.
    #[test]
    fn assert_in_sync_passes_when_synchronized() {
        let mut root = OverlayRoot::new();

        root.assert_in_sync(0); // Both empty

        root.push_modal(None, DismissPolicy::modal_default());
        root.assert_in_sync(1);

        root.push_modal(None, DismissPolicy::modal_default());
        root.push_modal(None, DismissPolicy::modal_default());
        root.assert_in_sync(3);

        root.pop_top_modal();
        root.assert_in_sync(2);
    }

    /// S2.INV1: assert_in_sync would fail when out of sync (debug only).
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "S2.INV1 violation")]
    fn assert_in_sync_fails_when_out_of_sync() {
        let mut root = OverlayRoot::new();
        root.push_modal(None, DismissPolicy::modal_default());
        root.push_modal(None, DismissPolicy::modal_default());
        // OverlayRoot has 2, but we claim ModalStack has 3
        root.assert_in_sync(3);
    }

    // ========================================================================
    // Stage 3: Routing Authority Tests
    // ========================================================================

    /// S3.INV2: is_empty() is the authority check for modal presence.
    #[test]
    fn s3_is_empty_is_routing_authority() {
        let mut root = OverlayRoot::new();

        // Routing checks is_empty() to determine if shielding applies
        assert!(root.is_empty(), "S3.INV2: empty means no shielding");

        root.push_modal(None, DismissPolicy::modal_default());
        assert!(
            !root.is_empty(),
            "S3.INV2: non-empty means shielding applies"
        );

        root.pop_top_modal();
        assert!(root.is_empty(), "S3.INV2: back to empty, no shielding");
    }

    /// S3.INV3: Read-only access methods do not mutate state.
    #[test]
    fn s3_readonly_methods_do_not_mutate() {
        let mut root = OverlayRoot::new();

        let id1 = root.push_modal(None, DismissPolicy::modal_default());
        let id2 = root.push_modal(None, DismissPolicy::modal_default());

        // Capture state
        let len_before = root.len();
        let top_before = root.top_id();

        // Call all read-only methods
        let _ = root.is_empty();
        let _ = root.len();
        let _ = root.top_id();
        let _ = root.top();
        let _ = root.get(id1);
        let _ = root.get(id2);
        let _ = root.iter().count();

        // Verify no mutation
        assert_eq!(root.len(), len_before);
        assert_eq!(root.top_id(), top_before);
    }

    // ========================================================================
    // Stage 4: Dismissal Policy Tests
    // ========================================================================

    /// S4.INV1: DismissPolicy is stored at push time.
    #[test]
    fn s4_dismiss_policy_stored_at_push() {
        let mut root = OverlayRoot::new();

        // Push with modal default (on_outside_click: false, on_escape: true)
        root.push_modal(None, DismissPolicy::modal_default());
        let entry = root.top().unwrap();
        assert!(!entry.dismiss_policy.on_outside_click);
        assert!(entry.dismiss_policy.on_escape);

        root.pop_top_modal();

        // Push with popup default (on_outside_click: true, on_escape: true)
        root.push_modal(None, DismissPolicy::popup_default());
        let entry = root.top().unwrap();
        assert!(entry.dismiss_policy.on_outside_click);
        assert!(entry.dismiss_policy.on_escape);
    }

    /// S4.INV1: Different modals can have different dismiss policies.
    #[test]
    fn s4_different_policies_per_modal() {
        let mut root = OverlayRoot::new();

        // First modal: modal default
        let id1 = root.push_modal(None, DismissPolicy::modal_default());
        // Second modal: popup default
        let id2 = root.push_modal(None, DismissPolicy::popup_default());

        // Verify each has its own policy
        let e1 = root.get(id1).unwrap();
        assert!(!e1.dismiss_policy.on_outside_click);

        let e2 = root.get(id2).unwrap();
        assert!(e2.dismiss_policy.on_outside_click);
    }

    /// S4.INV1: Top entry's dismiss policy is accessible for routing decisions.
    #[test]
    fn s4_top_dismiss_policy_accessible() {
        let mut root = OverlayRoot::new();

        // Push modal with popup policy
        root.push_modal(None, DismissPolicy::popup_default());

        // Routing code can check dismiss policy
        let should_dismiss_on_outside = root
            .top()
            .map(|e| e.dismiss_policy.on_outside_click)
            .unwrap_or(false);
        assert!(should_dismiss_on_outside);

        let should_dismiss_on_escape = root
            .top()
            .map(|e| e.dismiss_policy.on_escape)
            .unwrap_or(false);
        assert!(should_dismiss_on_escape);
    }
}
