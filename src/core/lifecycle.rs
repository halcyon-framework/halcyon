// SPEC-HAL-162: Window and Domain Lifecycle and Signal Teardown
//
// This module provides the central registry for WindowId and InteractionDomainId
// lifecycle state. It is the single source of truth for identity validity.
//
// # Lifecycle States
//
// - Created: ID allocated, not yet exposed
// - Alive: Eligible for signal delivery
// - Closing: Teardown initiated, no new delivery permitted
// - Closed: Identity invalid forever (tombstone retained)
//
// # Invariants
//
// - INV-LIFE-001: Once Closed, an ID never becomes valid again
// - INV-LIFE-002: No signals delivered to Closing/Closed targets
// - INV-LIFE-003: Validity checked at delivery time, not enqueue
// - INV-LIFE-004: Dropped signals cause no side effects in release
// - INV-LIFE-005: SignalEndpoint invalid when owner enters Closing
// - INV-LIFE-006: No rerouting or compensation for dropped signals
// - INV-LIFE-007: Deterministic ordering: same sequence → same drops

use crate::core::event::{InteractionDomainId, WindowId};
use crate::core::signal_bus::SignalTarget;
use std::collections::HashMap;

/// Lifecycle state for windows and interaction domains.
///
/// State machine is one-way: Created → Alive → Closing → Closed.
/// No backward transitions permitted (INV-LIFE-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    /// ID allocated but not yet exposed for signal delivery.
    Created,
    /// Eligible for signal delivery.
    Alive,
    /// Teardown initiated; no new delivery permitted.
    Closing,
    /// Identity invalid forever. Tombstone entry retained.
    Closed,
}

impl LifecycleState {
    /// Returns true if this state permits signal delivery.
    #[inline]
    pub fn is_alive(self) -> bool {
        matches!(self, LifecycleState::Alive)
    }

    /// Returns true if this state is a terminal state.
    #[inline]
    pub fn is_terminal(self) -> bool {
        matches!(self, LifecycleState::Closed)
    }
}

/// Central registry for window and domain lifecycle states.
///
/// This is the **single source of truth** for identity validity.
/// Owned by the engine core.
#[derive(Debug, Default)]
pub struct LifecycleRegistry {
    /// Window lifecycle states. Tombstones are retained for closed windows.
    windows: HashMap<WindowId, LifecycleState>,
    /// Domain lifecycle states. Tombstones are retained for closed domains.
    domains: HashMap<InteractionDomainId, LifecycleState>,
}

impl LifecycleRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    // ========================================================================
    // Window Registration and Teardown
    // ========================================================================

    /// Register a new window in Created state.
    ///
    /// Returns false if the window ID was already registered (including tombstones).
    /// INV-LIFE-001: Closed IDs are never recycled.
    pub fn register_window(&mut self, id: WindowId) -> bool {
        if self.windows.contains_key(&id) {
            return false;
        }
        self.windows.insert(id, LifecycleState::Created);
        true
    }

    /// Activate a window (Created → Alive).
    ///
    /// Returns false if window is not in Created state.
    pub fn activate_window(&mut self, id: WindowId) -> bool {
        if let Some(state) = self.windows.get_mut(&id) {
            if *state == LifecycleState::Created {
                *state = LifecycleState::Alive;
                return true;
            }
        }
        false
    }

    /// Begin teardown for a window (Alive → Closing).
    ///
    /// INV-LIFE-005: From this point, the window's SignalEndpoint is invalid.
    /// Returns false if window is not in Alive state.
    pub fn begin_window_teardown(&mut self, id: WindowId) -> bool {
        if let Some(state) = self.windows.get_mut(&id) {
            if *state == LifecycleState::Alive {
                *state = LifecycleState::Closing;
                return true;
            }
        }
        false
    }

    /// Finalize teardown for a window (Closing → Closed).
    ///
    /// INV-LIFE-001: After this, the ID is permanently invalid.
    /// The tombstone entry is retained to prevent ID reuse.
    /// Returns false if window is not in Closing state.
    pub fn finalize_window_teardown(&mut self, id: WindowId) -> bool {
        if let Some(state) = self.windows.get_mut(&id) {
            if *state == LifecycleState::Closing {
                *state = LifecycleState::Closed;
                return true;
            }
        }
        false
    }

    /// Check if a window is alive (eligible for signal delivery).
    ///
    /// INV-LIFE-002: Only Alive windows can receive signals.
    #[inline]
    pub fn is_window_alive(&self, id: WindowId) -> bool {
        self.windows.get(&id).map_or(false, |s| s.is_alive())
    }

    /// Get the current state of a window.
    pub fn window_state(&self, id: WindowId) -> Option<LifecycleState> {
        self.windows.get(&id).copied()
    }

    // ========================================================================
    // Domain Registration and Teardown
    // ========================================================================

    /// Register a new domain in Created state.
    ///
    /// Returns false if the domain ID was already registered (including tombstones).
    pub fn register_domain(&mut self, id: InteractionDomainId) -> bool {
        if self.domains.contains_key(&id) {
            return false;
        }
        self.domains.insert(id, LifecycleState::Created);
        true
    }

    /// Activate a domain (Created → Alive).
    pub fn activate_domain(&mut self, id: InteractionDomainId) -> bool {
        if let Some(state) = self.domains.get_mut(&id) {
            if *state == LifecycleState::Created {
                *state = LifecycleState::Alive;
                return true;
            }
        }
        false
    }

    /// Begin teardown for a domain (Alive → Closing).
    pub fn begin_domain_teardown(&mut self, id: InteractionDomainId) -> bool {
        if let Some(state) = self.domains.get_mut(&id) {
            if *state == LifecycleState::Alive {
                *state = LifecycleState::Closing;
                return true;
            }
        }
        false
    }

    /// Finalize teardown for a domain (Closing → Closed).
    pub fn finalize_domain_teardown(&mut self, id: InteractionDomainId) -> bool {
        if let Some(state) = self.domains.get_mut(&id) {
            if *state == LifecycleState::Closing {
                *state = LifecycleState::Closed;
                return true;
            }
        }
        false
    }

    /// Check if a domain is alive.
    #[inline]
    pub fn is_domain_alive(&self, id: InteractionDomainId) -> bool {
        self.domains.get(&id).map_or(false, |s| s.is_alive())
    }

    /// Get the current state of a domain.
    pub fn domain_state(&self, id: InteractionDomainId) -> Option<LifecycleState> {
        self.domains.get(&id).copied()
    }

    // ========================================================================
    // Signal Target Validation (INV-LIFE-003)
    // ========================================================================

    /// Check if a signal target is alive (eligible for delivery).
    ///
    /// INV-LIFE-002: Returns true only for Alive targets.
    /// INV-LIFE-003: This is called at delivery time, not enqueue time.
    pub fn is_target_alive(&self, target: &SignalTarget) -> bool {
        match target {
            SignalTarget::Window(id) => self.is_window_alive(*id),
            SignalTarget::Domain(id) => self.is_domain_alive(*id),
            // Broadcasts are handled specially - each recipient checked individually
            SignalTarget::BroadcastWindows | SignalTarget::BroadcastDomains => true,
        }
    }

    /// Get all alive windows for broadcast delivery.
    pub fn alive_windows(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.windows
            .iter()
            .filter(|(_, state)| state.is_alive())
            .map(|(id, _)| *id)
    }

    /// Get all alive domains for broadcast delivery.
    pub fn alive_domains(&self) -> impl Iterator<Item = InteractionDomainId> + '_ {
        self.domains
            .iter()
            .filter(|(_, state)| state.is_alive())
            .map(|(id, _)| *id)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // INV-LIFE-001: Identity Finality
    // ========================================================================

    #[test]
    fn inv_life_001_identity_finality_window() {
        let mut registry = LifecycleRegistry::new();
        let id = WindowId(42);

        // Register and activate
        assert!(registry.register_window(id));
        assert!(registry.activate_window(id));
        assert!(registry.is_window_alive(id));

        // Teardown
        assert!(registry.begin_window_teardown(id));
        assert!(!registry.is_window_alive(id)); // INV-LIFE-005

        assert!(registry.finalize_window_teardown(id));
        assert!(!registry.is_window_alive(id));

        // INV-LIFE-001: Cannot re-register closed window
        assert!(!registry.register_window(id));

        // State remains Closed (tombstone)
        assert_eq!(registry.window_state(id), Some(LifecycleState::Closed));
    }

    #[test]
    fn inv_life_001_identity_finality_domain() {
        let mut registry = LifecycleRegistry::new();
        let id = InteractionDomainId(99);

        assert!(registry.register_domain(id));
        assert!(registry.activate_domain(id));
        assert!(registry.begin_domain_teardown(id));
        assert!(registry.finalize_domain_teardown(id));

        // INV-LIFE-001: Cannot re-register closed domain
        assert!(!registry.register_domain(id));
        assert_eq!(registry.domain_state(id), Some(LifecycleState::Closed));
    }

    // ========================================================================
    // INV-LIFE-002: No Post-Mortem Delivery
    // ========================================================================

    #[test]
    fn inv_life_002_no_post_mortem_closing() {
        let mut registry = LifecycleRegistry::new();
        let id = WindowId(1);

        registry.register_window(id);
        registry.activate_window(id);
        registry.begin_window_teardown(id);

        // Closing state: not alive
        assert!(!registry.is_window_alive(id));
        assert!(!registry.is_target_alive(&SignalTarget::Window(id)));
    }

    #[test]
    fn inv_life_002_no_post_mortem_closed() {
        let mut registry = LifecycleRegistry::new();
        let id = WindowId(2);

        registry.register_window(id);
        registry.activate_window(id);
        registry.begin_window_teardown(id);
        registry.finalize_window_teardown(id);

        // Closed state: not alive
        assert!(!registry.is_window_alive(id));
        assert!(!registry.is_target_alive(&SignalTarget::Window(id)));
    }

    // ========================================================================
    // INV-LIFE-003: Delivery-Time Validation
    // ========================================================================

    #[test]
    fn inv_life_003_delivery_time_validation() {
        let mut registry = LifecycleRegistry::new();
        let id = WindowId(3);

        // Not registered: not alive
        assert!(!registry.is_target_alive(&SignalTarget::Window(id)));

        // Created: not alive
        registry.register_window(id);
        assert!(!registry.is_target_alive(&SignalTarget::Window(id)));

        // Alive: is alive
        registry.activate_window(id);
        assert!(registry.is_target_alive(&SignalTarget::Window(id)));

        // Closing: not alive
        registry.begin_window_teardown(id);
        assert!(!registry.is_target_alive(&SignalTarget::Window(id)));

        // Closed: not alive
        registry.finalize_window_teardown(id);
        assert!(!registry.is_target_alive(&SignalTarget::Window(id)));
    }

    // ========================================================================
    // INV-LIFE-005: Endpoint Invalidation
    // ========================================================================

    #[test]
    fn inv_life_005_endpoint_invalidation() {
        let mut registry = LifecycleRegistry::new();
        let id = WindowId(5);

        registry.register_window(id);
        registry.activate_window(id);

        // Before teardown: alive
        assert!(registry.is_window_alive(id));

        // Begin teardown: immediately not alive
        registry.begin_window_teardown(id);
        assert!(!registry.is_window_alive(id));
    }

    // ========================================================================
    // INV-LIFE-007: Deterministic Ordering
    // ========================================================================

    #[test]
    fn inv_life_007_deterministic_ordering() {
        // Run the same sequence twice and verify identical results
        for _ in 0..2 {
            let mut registry = LifecycleRegistry::new();

            let w1 = WindowId(10);
            let w2 = WindowId(20);
            let w3 = WindowId(30);

            // Register all
            assert!(registry.register_window(w1));
            assert!(registry.register_window(w2));
            assert!(registry.register_window(w3));

            // Activate all
            assert!(registry.activate_window(w1));
            assert!(registry.activate_window(w2));
            assert!(registry.activate_window(w3));

            // Same sequence of teardowns
            assert!(registry.begin_window_teardown(w2));
            assert!(registry.begin_window_teardown(w1));
            assert!(registry.finalize_window_teardown(w2));

            // Verify state is deterministic
            assert!(!registry.is_window_alive(w1)); // Closing
            assert!(!registry.is_window_alive(w2)); // Closed
            assert!(registry.is_window_alive(w3)); // Still Alive

            assert_eq!(registry.window_state(w1), Some(LifecycleState::Closing));
            assert_eq!(registry.window_state(w2), Some(LifecycleState::Closed));
            assert_eq!(registry.window_state(w3), Some(LifecycleState::Alive));
        }
    }

    // ========================================================================
    // State Machine One-Way Transitions
    // ========================================================================

    #[test]
    fn state_machine_no_backward_transitions() {
        let mut registry = LifecycleRegistry::new();
        let id = WindowId(100);

        registry.register_window(id);
        registry.activate_window(id);
        registry.begin_window_teardown(id);

        // Cannot go back to Alive from Closing
        assert!(!registry.activate_window(id));
        assert_eq!(registry.window_state(id), Some(LifecycleState::Closing));

        registry.finalize_window_teardown(id);

        // Cannot go back to Closing from Closed
        assert!(!registry.begin_window_teardown(id));
        assert_eq!(registry.window_state(id), Some(LifecycleState::Closed));
    }

    // ========================================================================
    // Broadcast Alive Iteration
    // ========================================================================

    #[test]
    fn broadcast_only_alive_windows() {
        let mut registry = LifecycleRegistry::new();

        let alive1 = WindowId(1);
        let alive2 = WindowId(2);
        let closing = WindowId(3);
        let closed = WindowId(4);

        for id in [alive1, alive2, closing, closed] {
            registry.register_window(id);
            registry.activate_window(id);
        }

        registry.begin_window_teardown(closing);
        registry.begin_window_teardown(closed);
        registry.finalize_window_teardown(closed);

        let alive: Vec<_> = registry.alive_windows().collect();
        assert_eq!(alive.len(), 2);
        assert!(alive.contains(&alive1));
        assert!(alive.contains(&alive2));
        assert!(!alive.contains(&closing));
        assert!(!alive.contains(&closed));
    }

    // ========================================================================
    // INV-LIFE-004: Silent Drop Policy
    // ========================================================================

    /// INV-LIFE-004: Dropped signals cause no side effects.
    ///
    /// This test verifies that when is_target_alive returns false,
    /// the caller can skip delivery without any compensation logic.
    #[test]
    fn inv_life_004_silent_drop() {
        let mut registry = LifecycleRegistry::new();
        let id = WindowId(4);

        registry.register_window(id);
        registry.activate_window(id);
        registry.begin_window_teardown(id);

        // Target is Closing, not alive
        assert!(!registry.is_target_alive(&SignalTarget::Window(id)));

        // INV-LIFE-004: The registry provides a simple boolean check.
        // No exceptions, no error states, no compensation callbacks.
        // The caller simply skips delivery — that's the "silent drop".
        // This test verifies the API is clean and side-effect-free.

        // Finalize to Closed
        registry.finalize_window_teardown(id);
        assert!(!registry.is_target_alive(&SignalTarget::Window(id)));

        // Still no side effects — just a boolean check
        assert_eq!(registry.window_state(id), Some(LifecycleState::Closed));
    }

    // ========================================================================
    // INV-LIFE-006: No Implicit Compensation
    // ========================================================================

    /// INV-LIFE-006: No rerouting or compensation for dropped signals.
    ///
    /// This test verifies that the registry provides no "fallback target"
    /// or "dead letter queue" mechanism. When a target is not alive,
    /// is_target_alive simply returns false. Period.
    #[test]
    fn inv_life_006_no_compensation() {
        let mut registry = LifecycleRegistry::new();

        let alive_window = WindowId(1);
        let dead_window = WindowId(2);

        // Setup: one alive, one dead
        registry.register_window(alive_window);
        registry.activate_window(alive_window);

        registry.register_window(dead_window);
        registry.activate_window(dead_window);
        registry.begin_window_teardown(dead_window);
        registry.finalize_window_teardown(dead_window);

        // Verify states
        assert!(registry.is_target_alive(&SignalTarget::Window(alive_window)));
        assert!(!registry.is_target_alive(&SignalTarget::Window(dead_window)));

        // INV-LIFE-006: There is NO API to:
        // - Get a "fallback" target for a dead window
        // - Reroute signals to another window
        // - Queue signals for later delivery
        // - Get notified of dropped signals
        //
        // The only API is is_target_alive() which returns bool.
        // This is by design: signals to dead targets are simply dropped.

        // Verify the registry has no rerouting mechanism
        // (This is implicit in the API design — there's simply no such method)
        assert_eq!(
            registry.window_state(dead_window),
            Some(LifecycleState::Closed)
        );
        assert_eq!(
            registry.window_state(alive_window),
            Some(LifecycleState::Alive)
        );
    }
}
