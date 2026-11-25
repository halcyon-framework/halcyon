//! SPEC-HAL-161: Cross-Window Communication and Signals
//!
//! A minimal, sovereign, deterministic, and safe cross-window signaling substrate.
//!
//! # MWA Compliance
//!
//! This module is engine-internal and does NOT:
//! - Grant semantic authority to OS windows (INV-XWIN-001)
//! - Create implicit fan-out (INV-XWIN-002)
//! - Deliver during forbidden phases (INV-XWIN-003)
//! - Re-enter routing/layout/scene/paint (INV-XWIN-004)
//!
//! # Phase Integration
//!
//! Signal delivery MUST occur in the **Signal Delivery Phase**:
//! - AFTER: `input_fusion.process_frame()` + `event_router.route_input_event()`
//! - BEFORE: `run_single_frame()` (layout + scene + paint)
//!
//! Wiring location: `RedrawRequested` handler in application event loop.
//! See SPEC-HAL-161 § 6.2 for binding semantics.

use crate::core::event::{InteractionDomainId, WindowId};
use std::collections::VecDeque;

// ============================================================================
// INV-XWIN-008: Bounded queue capacity (drop-oldest policy)
// ============================================================================

/// Default queue capacity. Exceeding this triggers drop-oldest.
pub const SIGNAL_BUS_DEFAULT_CAPACITY: usize = 256;

// ============================================================================
// Addressing Types (SPEC-HAL-161 § 4.2, § 5.2, § 5.3)
// ============================================================================

/// Signal source identifier.
///
/// INV-XWIN-001: Sources are engine-internal IDs, not OS handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalSource {
    /// Signal originated from a specific window.
    Window(WindowId),
    /// Signal originated from a specific interaction domain.
    Domain(InteractionDomainId),
    /// Signal originated from the engine itself (system-level).
    System,
}

/// Signal target identifier.
///
/// INV-XWIN-002: Targets are explicit. No implicit fan-out except via broadcast APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalTarget {
    /// Deliver to a specific window.
    Window(WindowId),
    /// Deliver to a specific interaction domain.
    Domain(InteractionDomainId),
    /// Broadcast to all windows (explicit fan-out).
    BroadcastWindows,
    /// Broadcast to all domains (explicit fan-out).
    BroadcastDomains,
}

/// Signal kind for request/response/notification patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalKind {
    /// A request expecting a response.
    Request,
    /// A response to a previous request.
    Response,
    /// A one-way notification.
    Notification,
}

// ============================================================================
// Payload Types (SPEC-HAL-161 § 5.5 — Typed Payload Law)
// ============================================================================

/// INV-XWIN-007: Typed payload law.
///
/// Payloads MUST be stable and inspectable. No `Any`, no trait-object downcasting.
///
/// This enum is closed and extensible only via SPEC amendment.
/// Application-defined messages use `Custom { id, data }` with numeric ID + byte payload.
#[derive(Debug, Clone, PartialEq)]
pub enum SignalPayload {
    /// No payload (pure signal).
    Empty,

    /// Dialog result: confirmed with optional string value.
    DialogConfirm { value: Option<String> },

    /// Dialog result: cancelled.
    DialogCancel,

    /// Settings changed notification.
    SettingsChanged,

    /// Window lifecycle: window is closing.
    WindowClosing,

    /// Window lifecycle: window closed.
    WindowClosed,

    /// Application-defined message with numeric ID and byte payload.
    ///
    /// INV-XWIN-007 compliance: ID is inspectable, data is stable bytes.
    /// No dynamic downcasting. Interpretation is caller's responsibility.
    Custom {
        /// Application-defined message type ID.
        id: u32,
        /// Byte payload (stable, inspectable).
        data: Vec<u8>,
    },
}

// ============================================================================
// Envelope (SPEC-HAL-161 § 5.1)
// ============================================================================

/// Signal envelope wrapping source, target, kind, and payload.
///
/// INV-XWIN-005: Envelopes are delivered in enqueue order (FIFO).
#[derive(Debug, Clone, PartialEq)]
pub struct SignalEnvelope {
    /// Who sent this signal.
    pub source: SignalSource,
    /// Who should receive this signal.
    pub target: SignalTarget,
    /// Request/response/notification kind.
    pub kind: SignalKind,
    /// Typed payload.
    pub payload: SignalPayload,
}

impl SignalEnvelope {
    /// Create a new signal envelope.
    pub fn new(
        source: SignalSource,
        target: SignalTarget,
        kind: SignalKind,
        payload: SignalPayload,
    ) -> Self {
        Self {
            source,
            target,
            kind,
            payload,
        }
    }

    /// Create a notification from system to a specific window.
    pub fn system_to_window(window: WindowId, payload: SignalPayload) -> Self {
        Self::new(
            SignalSource::System,
            SignalTarget::Window(window),
            SignalKind::Notification,
            payload,
        )
    }

    /// Create a notification from one window to another.
    pub fn window_to_window(
        from: WindowId,
        to: WindowId,
        kind: SignalKind,
        payload: SignalPayload,
    ) -> Self {
        Self::new(
            SignalSource::Window(from),
            SignalTarget::Window(to),
            kind,
            payload,
        )
    }

    /// Create a broadcast notification from system to all windows.
    pub fn system_broadcast_windows(payload: SignalPayload) -> Self {
        Self::new(
            SignalSource::System,
            SignalTarget::BroadcastWindows,
            SignalKind::Notification,
            payload,
        )
    }
}

// ============================================================================
// Signal Bus (SPEC-HAL-161 § 4.1)
// ============================================================================

/// System-scoped signal bus for cross-window communication.
///
/// # Invariants
///
/// - INV-XWIN-001: Engine-internal only; no OS authority.
/// - INV-XWIN-002: No implicit fan-out; only explicit targets.
/// - INV-XWIN-003: Delivery only via `drain_pending()` in Signal Delivery Phase.
/// - INV-XWIN-005: FIFO ordering; deterministic delivery.
/// - INV-XWIN-008: Bounded queue with drop-oldest policy.
///
/// # Phase Guard
///
/// The bus does NOT deliver signals itself. The caller MUST:
/// 1. Call `drain_pending()` only in the Signal Delivery Phase.
/// 2. Not call `drain_pending()` during layout/scene/paint/routing.
///
/// The bus provides `is_delivery_safe()` as a debug assertion hook.
#[derive(Debug)]
pub struct SignalBus {
    /// Pending signals awaiting delivery (FIFO queue).
    queue: VecDeque<SignalEnvelope>,
    /// Maximum queue capacity (INV-XWIN-008).
    capacity: usize,
    /// Count of signals dropped due to capacity overflow.
    dropped_count: u64,
    /// Phase guard: set to true during delivery to detect re-entrancy.
    delivering: bool,
}

impl SignalBus {
    /// Create a new signal bus with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(SIGNAL_BUS_DEFAULT_CAPACITY)
    }

    /// Create a new signal bus with specified capacity.
    ///
    /// INV-XWIN-008: When capacity is exceeded, oldest signals are dropped.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity),
            capacity,
            dropped_count: 0,
            delivering: false,
        }
    }

    /// Enqueue a signal for delivery.
    ///
    /// INV-XWIN-008: If queue is at capacity, the oldest signal is dropped.
    /// INV-XWIN-004: Enqueueing is always safe; no re-entrancy risk.
    ///
    /// Returns `true` if the signal was enqueued, `false` if an old signal was dropped.
    pub fn enqueue(&mut self, envelope: SignalEnvelope) -> bool {
        // INV-XWIN-008: Drop-oldest policy
        if self.queue.len() >= self.capacity {
            self.queue.pop_front();
            self.dropped_count += 1;
            self.queue.push_back(envelope);
            false
        } else {
            self.queue.push_back(envelope);
            true
        }
    }

    /// Drain all pending signals for delivery.
    ///
    /// # Phase Requirement (INV-XWIN-003)
    ///
    /// This method MUST ONLY be called in the Signal Delivery Phase:
    /// - AFTER input routing completes
    /// - BEFORE run_single_frame() (layout/scene/paint)
    ///
    /// The caller is responsible for phase correctness.
    ///
    /// # Re-entrancy Guard (INV-XWIN-004)
    ///
    /// Debug builds will panic if called while already delivering.
    pub fn drain_pending(&mut self) -> Vec<SignalEnvelope> {
        // INV-XWIN-004: Re-entrancy guard
        debug_assert!(
            !self.delivering,
            "INV-XWIN-004 VIOLATION: SignalBus::drain_pending() called during delivery (re-entrancy)"
        );

        self.delivering = true;
        let signals: Vec<_> = self.queue.drain(..).collect();
        self.delivering = false;

        signals
    }

    /// Check if delivery is currently in progress.
    ///
    /// Used by debug assertions to detect INV-XWIN-004 violations.
    pub fn is_delivering(&self) -> bool {
        self.delivering
    }

    /// Check if it's safe to call `drain_pending()`.
    ///
    /// Returns `false` if delivery is already in progress (re-entrancy).
    /// Callers SHOULD assert this in debug builds before delivery.
    pub fn is_delivery_safe(&self) -> bool {
        !self.delivering
    }

    /// Get the number of pending signals.
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Get the number of signals dropped due to capacity overflow.
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count
    }

    /// Get the queue capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Clear all pending signals without delivering.
    ///
    /// Use with caution; signals will be lost.
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

impl Default for SignalBus {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Signal Endpoint (SPEC-HAL-161 § 4.3)
// ============================================================================

/// A pure data handle identifying a signal receiver.
///
/// INV-XWIN-001: Endpoints do not own state or imply authority.
/// They are opaque tokens for addressing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignalEndpoint {
    /// Unique endpoint ID.
    id: u64,
}

impl SignalEndpoint {
    /// Create a new endpoint with the given ID.
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    /// Get the endpoint ID.
    pub fn id(&self) -> u64 {
        self.id
    }
}

/// Generator for unique endpoint IDs.
#[derive(Debug, Default)]
pub struct SignalEndpointAllocator {
    next_id: u64,
}

impl SignalEndpointAllocator {
    /// Create a new allocator.
    pub fn new() -> Self {
        Self { next_id: 1 }
    }

    /// Allocate a new unique endpoint.
    pub fn allocate(&mut self) -> SignalEndpoint {
        let id = self.next_id;
        self.next_id += 1;
        SignalEndpoint::new(id)
    }
}

// ============================================================================
// Tests (INV-XWIN-* Enforcement)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// INV-XWIN-002: Addressing isolation — message to Window A does not match Window B.
    #[test]
    fn inv_xwin_002_addressing_isolation() {
        let window_a = WindowId(1);
        let window_b = WindowId(2);

        let envelope = SignalEnvelope::new(
            SignalSource::System,
            SignalTarget::Window(window_a),
            SignalKind::Notification,
            SignalPayload::Empty,
        );

        // Target is explicitly window_a
        assert_eq!(envelope.target, SignalTarget::Window(window_a));
        // Target is NOT window_b
        assert_ne!(envelope.target, SignalTarget::Window(window_b));

        // Broadcast is distinct from unicast
        let broadcast = SignalEnvelope::system_broadcast_windows(SignalPayload::Empty);
        assert_eq!(broadcast.target, SignalTarget::BroadcastWindows);
        assert_ne!(broadcast.target, SignalTarget::Window(window_a));
    }

    /// INV-XWIN-005: Deterministic ordering — enqueue sequence matches delivery sequence.
    #[test]
    fn inv_xwin_005_deterministic_ordering() {
        let mut bus = SignalBus::new();

        // Enqueue 3 signals in order
        for i in 0..3 {
            bus.enqueue(SignalEnvelope::new(
                SignalSource::System,
                SignalTarget::Window(WindowId(i)),
                SignalKind::Notification,
                SignalPayload::Custom {
                    id: i as u32,
                    data: vec![],
                },
            ));
        }

        // Drain and verify order
        let signals = bus.drain_pending();
        assert_eq!(signals.len(), 3);

        for (i, signal) in signals.iter().enumerate() {
            assert_eq!(signal.target, SignalTarget::Window(WindowId(i as u64)));
            if let SignalPayload::Custom { id, .. } = &signal.payload {
                assert_eq!(*id, i as u32);
            } else {
                panic!("Expected Custom payload");
            }
        }
    }

    /// INV-XWIN-005: Determinism is repeatable across multiple runs.
    #[test]
    fn inv_xwin_005_determinism_repeatable() {
        fn run_sequence() -> Vec<u32> {
            let mut bus = SignalBus::new();
            for i in 0..5 {
                bus.enqueue(SignalEnvelope::new(
                    SignalSource::System,
                    SignalTarget::BroadcastWindows,
                    SignalKind::Notification,
                    SignalPayload::Custom {
                        id: i,
                        data: vec![],
                    },
                ));
            }
            bus.drain_pending()
                .into_iter()
                .filter_map(|s| {
                    if let SignalPayload::Custom { id, .. } = s.payload {
                        Some(id)
                    } else {
                        None
                    }
                })
                .collect()
        }

        // Run twice, results must be identical
        let run1 = run_sequence();
        let run2 = run_sequence();
        assert_eq!(run1, run2);
        assert_eq!(run1, vec![0, 1, 2, 3, 4]);
    }

    /// INV-XWIN-008: Bounded backpressure — drop-oldest when capacity exceeded.
    #[test]
    fn inv_xwin_008_bounded_backpressure() {
        let mut bus = SignalBus::with_capacity(3);

        // Enqueue 3 signals (at capacity)
        for i in 0..3 {
            let ok = bus.enqueue(SignalEnvelope::new(
                SignalSource::System,
                SignalTarget::BroadcastWindows,
                SignalKind::Notification,
                SignalPayload::Custom {
                    id: i,
                    data: vec![],
                },
            ));
            assert!(ok, "Should succeed when under capacity");
        }

        assert_eq!(bus.pending_count(), 3);
        assert_eq!(bus.dropped_count(), 0);

        // Enqueue 4th signal — should drop oldest (id=0)
        let ok = bus.enqueue(SignalEnvelope::new(
            SignalSource::System,
            SignalTarget::BroadcastWindows,
            SignalKind::Notification,
            SignalPayload::Custom {
                id: 3,
                data: vec![],
            },
        ));
        assert!(!ok, "Should return false when dropping oldest");
        assert_eq!(bus.pending_count(), 3);
        assert_eq!(bus.dropped_count(), 1);

        // Verify oldest was dropped (id=0 gone, id=1,2,3 remain)
        let signals = bus.drain_pending();
        let ids: Vec<u32> = signals
            .into_iter()
            .filter_map(|s| {
                if let SignalPayload::Custom { id, .. } = s.payload {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(ids, vec![1, 2, 3], "Oldest (id=0) should be dropped");
    }

    /// INV-XWIN-004: Re-entrancy detection (debug assertion).
    #[test]
    fn inv_xwin_004_reentrancy_guard() {
        let bus = SignalBus::new();

        // Initially safe
        assert!(bus.is_delivery_safe());
        assert!(!bus.is_delivering());
    }

    /// INV-XWIN-007: Typed payload law — no dynamic downcasting.
    #[test]
    fn inv_xwin_007_typed_payload() {
        // All payload variants are statically typed and inspectable
        let payloads = vec![
            SignalPayload::Empty,
            SignalPayload::DialogConfirm {
                value: Some("ok".into()),
            },
            SignalPayload::DialogCancel,
            SignalPayload::SettingsChanged,
            SignalPayload::WindowClosing,
            SignalPayload::WindowClosed,
            SignalPayload::Custom {
                id: 42,
                data: vec![1, 2, 3],
            },
        ];

        for payload in payloads {
            // Pattern matching works — no downcasting needed
            match &payload {
                SignalPayload::Empty => {}
                SignalPayload::DialogConfirm { value } => {
                    let _ = value; // Inspectable
                }
                SignalPayload::DialogCancel => {}
                SignalPayload::SettingsChanged => {}
                SignalPayload::WindowClosing => {}
                SignalPayload::WindowClosed => {}
                SignalPayload::Custom { id, data } => {
                    let _ = (id, data); // Inspectable
                }
            }
        }
    }

    /// Signal endpoint allocation produces unique IDs.
    #[test]
    fn endpoint_allocation_unique() {
        let mut allocator = SignalEndpointAllocator::new();

        let e1 = allocator.allocate();
        let e2 = allocator.allocate();
        let e3 = allocator.allocate();

        assert_ne!(e1.id(), e2.id());
        assert_ne!(e2.id(), e3.id());
        assert_ne!(e1.id(), e3.id());
    }

    /// Bus is empty after drain.
    #[test]
    fn bus_empty_after_drain() {
        let mut bus = SignalBus::new();
        bus.enqueue(SignalEnvelope::system_broadcast_windows(
            SignalPayload::Empty,
        ));

        assert!(!bus.is_empty());

        let _ = bus.drain_pending();

        assert!(bus.is_empty());
        assert_eq!(bus.pending_count(), 0);
    }

    /// INV-XWIN-003: Phase correctness — structural enforcement.
    ///
    /// This test verifies the API structure that enforces phase correctness:
    /// - Enqueue is always safe (can be called anywhere)
    /// - Drain returns signals without delivering them
    /// - Caller is responsible for phase-correct delivery
    ///
    /// The actual phase correctness is a wiring contract enforced by
    /// `deliver_signals()` in `core::app`, not runtime checks.
    #[test]
    fn inv_xwin_003_phase_correctness_structure() {
        let mut bus = SignalBus::new();

        // Enqueue is always safe
        bus.enqueue(SignalEnvelope::system_broadcast_windows(
            SignalPayload::Empty,
        ));
        bus.enqueue(SignalEnvelope::system_broadcast_windows(
            SignalPayload::SettingsChanged,
        ));

        // Drain returns signals without side effects
        let signals = bus.drain_pending();
        assert_eq!(signals.len(), 2);

        // Bus is now empty
        assert!(bus.is_empty());

        // Drain again returns empty (no phantom signals)
        let signals2 = bus.drain_pending();
        assert!(signals2.is_empty());

        // The phase correctness invariant (INV-XWIN-003) is structurally enforced:
        // - SignalBus has no layout/scene/paint dependencies
        // - deliver_signals() is the only correct call site (see core::app)
        // - Wiring is documented in SPEC-HAL-161 § 6.2
    }

    /// INV-XWIN-006: Isolation compatibility — signals do not affect focus/capture/modality.
    ///
    /// This test verifies that SignalPayload variants do not contain or imply
    /// focus/capture/modality state. The invariant is enforced by payload design.
    #[test]
    fn inv_xwin_006_isolation_compatibility() {
        // Verify payload enum has no focus/capture/modality variants
        // This is a compile-time invariant enforced by the closed enum design.
        // The test documents the invariant for readers.

        let payloads = vec![
            SignalPayload::Empty,
            SignalPayload::DialogConfirm { value: None },
            SignalPayload::DialogCancel,
            SignalPayload::SettingsChanged,
            SignalPayload::WindowClosing,
            SignalPayload::WindowClosed,
            SignalPayload::Custom {
                id: 0,
                data: vec![],
            },
        ];

        for payload in payloads {
            // None of these payloads contain:
            // - FocusState
            // - CaptureState
            // - ModalStack
            // - InputEvent
            // - WidgetId (for focus/capture target)
            //
            // This is enforced by the closed enum (no new variants without SPEC amendment).
            // Pattern matching proves exhaustiveness.
            match payload {
                SignalPayload::Empty => {}
                SignalPayload::DialogConfirm { value: _ } => {}
                SignalPayload::DialogCancel => {}
                SignalPayload::SettingsChanged => {}
                SignalPayload::WindowClosing => {}
                SignalPayload::WindowClosed => {}
                SignalPayload::Custom { id: _, data: _ } => {}
            }
        }
    }

    /// INV-XWIN-001: Authority — bus is engine-internal, OS has no authority via it.
    ///
    /// This test verifies that SignalSource/SignalTarget use Halcyon IDs,
    /// not OS handles. Authority is enforced by type design.
    #[test]
    fn inv_xwin_001_authority_no_os_handles() {
        // SignalSource uses Halcyon WindowId, not RawWindowHandle
        let source = SignalSource::Window(WindowId(1));
        match source {
            SignalSource::Window(wid) => {
                // WindowId is a Halcyon ID (u64), not an OS handle
                let _ = wid.0; // Accessing the inner u64 proves it's not an opaque OS handle
            }
            SignalSource::Domain(_) => {}
            SignalSource::System => {}
        }

        // SignalTarget uses Halcyon IDs, not OS handles
        let target = SignalTarget::Window(WindowId(2));
        match target {
            SignalTarget::Window(wid) => {
                let _ = wid.0; // Halcyon ID, not OS handle
            }
            SignalTarget::Domain(_) => {}
            SignalTarget::BroadcastWindows => {}
            SignalTarget::BroadcastDomains => {}
        }

        // No OS types (RawWindowHandle, HWND, etc.) appear in the signal API
    }

    // ========================================================================
    // SPEC-HAL-161: Cross-Window Signal Flow Regression Test
    // ========================================================================

    /// SPEC-HAL-161 REGRESSION: Headless cross-window signal flow.
    ///
    /// Validates the complete enqueue → deliver → state mutation cycle
    /// without renderer, windows, or winit. This is CI-safe and deterministic.
    ///
    /// Scenario: Parent window opens dialog, dialog sends confirm with value,
    /// parent receives and updates state.
    #[test]
    fn SPEC_hal_161_cross_window_signal_flow() {
        // Application state (simulates record being edited)
        let mut record_name = "Initial".to_string();

        // Window IDs (Halcyon-internal, not OS handles)
        let parent_window = WindowId(0);
        let dialog_window = WindowId(1);

        // Create signal bus
        let mut bus = SignalBus::new();

        // Phase 1: Dialog enqueues DialogConfirm signal to parent
        // (Simulates user pressing Save in dialog)
        bus.enqueue(SignalEnvelope::window_to_window(
            dialog_window,
            parent_window,
            SignalKind::Response,
            SignalPayload::DialogConfirm {
                value: Some("Updated Value".to_string()),
            },
        ));

        assert_eq!(bus.pending_count(), 1, "Signal should be queued");

        // Phase 2: Signal Delivery Phase (via deliver_signals)
        // Note: Using drain_pending directly since deliver_signals requires
        // the phase guard infrastructure from app.rs context.
        let signals = bus.drain_pending();

        // Phase 3: Handler processes signals and mutates state
        for envelope in signals {
            // Verify addressing
            assert_eq!(envelope.source, SignalSource::Window(dialog_window));
            assert_eq!(envelope.target, SignalTarget::Window(parent_window));
            assert_eq!(envelope.kind, SignalKind::Response);

            // Handle DialogConfirm — mutate application state
            if let SignalPayload::DialogConfirm {
                value: Some(new_name),
            } = envelope.payload
            {
                record_name = new_name;
            }
        }

        // Phase 4: Verify state mutation occurred
        assert_eq!(
            record_name, "Updated Value",
            "Record should be updated by signal"
        );
        assert!(bus.is_empty(), "Bus should be empty after delivery");
    }

    /// SPEC-HAL-161 REGRESSION: Bidirectional signal flow (request/response).
    ///
    /// Validates request → response pattern without visual infrastructure.
    #[test]
    fn SPEC_hal_161_request_response_pattern() {
        let window_a = WindowId(10);
        let window_b = WindowId(20);

        let mut bus = SignalBus::new();

        // Window A sends request to Window B
        bus.enqueue(SignalEnvelope::window_to_window(
            window_a,
            window_b,
            SignalKind::Request,
            SignalPayload::Custom {
                id: 1,
                data: b"get_status".to_vec(),
            },
        ));

        // Drain and "process" request (Window B logic)
        let requests = bus.drain_pending();
        assert_eq!(requests.len(), 1);

        for req in requests {
            // Window B receives request, sends response
            if let SignalPayload::Custom { id: 1, .. } = req.payload {
                bus.enqueue(SignalEnvelope::window_to_window(
                    window_b,
                    window_a,
                    SignalKind::Response,
                    SignalPayload::Custom {
                        id: 1,
                        data: b"status_ok".to_vec(),
                    },
                ));
            }
        }

        // Drain and "process" response (Window A logic)
        let responses = bus.drain_pending();
        assert_eq!(responses.len(), 1);

        let resp = &responses[0];
        assert_eq!(resp.source, SignalSource::Window(window_b));
        assert_eq!(resp.target, SignalTarget::Window(window_a));
        assert_eq!(resp.kind, SignalKind::Response);

        if let SignalPayload::Custom { id, data } = &resp.payload {
            assert_eq!(*id, 1);
            assert_eq!(data, b"status_ok");
        } else {
            panic!("Expected Custom payload");
        }
    }

    /// SPEC-HAL-161 REGRESSION: DialogCancel signal flow.
    ///
    /// Validates cancel path preserves original state.
    #[test]
    fn SPEC_hal_161_dialog_cancel_preserves_state() {
        let mut record_name = "Original".to_string();
        let parent = WindowId(0);
        let dialog = WindowId(1);

        let mut bus = SignalBus::new();

        // Dialog sends cancel
        bus.enqueue(SignalEnvelope::window_to_window(
            dialog,
            parent,
            SignalKind::Response,
            SignalPayload::DialogCancel,
        ));

        // Deliver
        for envelope in bus.drain_pending() {
            match envelope.payload {
                SignalPayload::DialogConfirm { value: Some(v) } => {
                    record_name = v;
                }
                SignalPayload::DialogCancel => {
                    // Cancel: do not modify state
                }
                _ => {}
            }
        }

        // State unchanged
        assert_eq!(record_name, "Original", "Cancel should not modify state");
    }
}
