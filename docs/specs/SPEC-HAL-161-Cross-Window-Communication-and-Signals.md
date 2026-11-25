# SPEC-HAL-161 — Cross-Window Communication and Signals

**Status:** Binding
**Layer:** Core / Engine Substrate
**Scope:** Engine-internal cross-window and cross-domain signaling
**Depends on:** SPEC-HAL-002, SPEC-HAL-082A, SPEC-HAL-150, SPEC-HAL-151, SPEC-HAL-152
**Conflicts with:** Any semantic WindowManager, cross-window input routing, compositor authority

**Implementation:** `src/core/signal_bus.rs`
**Wiring:** `src/core/app.rs::deliver_signals()`
**Window Hook:** `src/platform/window.rs::Window::with_endpoint()`

---

## 1. Problem Statement

Halcyon supports multiple windows and strict interaction-domain isolation.  
What is missing is a **first-class, engine-native mechanism for cross-window communication** that:

- allows request/response (e.g. dialog → opener),
- allows targeted delivery (to one window or domain),
- allows explicit broadcast,
- optionally allows a custom channel/endpoint to be passed at window creation,

**without** introducing new authority, violating isolation, or creating re-entrancy hazards.

This SPEC defines a **pure data signaling substrate**, not an application framework, not a UI policy layer, and not an OS
abstraction.

---

## 2. Design Principles

**P1 — Engine-internal only**  
Cross-window communication is an engine concern, not an OS, compositor, or widget concern.

**P2 — No authority creation**  
Signals carry information only. They do not grant focus, capture, modality, layout, or routing authority.

**P3 — Explicit addressing**  
All delivery is explicit. No implicit fan-out, no “current window” magic.

**P4 — Phase-correct**  
Signal delivery must occur in a dedicated engine phase and must never re-enter routing, layout, scene build, or paint.

**P5 — Deterministic**  
Given identical enqueue order, delivery order is identical.

**P6 — Typed and inspectable**  
Payloads are structurally defined. Hidden dynamic downcasting is forbidden.

---

## 3. Non-Goals

This SPEC explicitly does **not** define or solve:

- a semantic `WindowManager`,
- per-window widget trees,
- modal UI policy,
- cross-window drag-and-drop,
- persistence, window taxonomy, or multi-monitor placement,
- application-level dialog frameworks.

---

## 4. Conceptual Model

### 4.1 Signal Bus

A **system-scoped signal bus** owned by Halcyon core.

- Exists once per engine instance.
- Independent of compositor, renderer, widgets, and platform glue.
- Stores messages in a bounded queue.
- Delivers messages only during a dedicated delivery phase.

### 4.2 Addressing

Signals are addressed explicitly to one of:

- a specific `WindowId`,
- a specific `InteractionDomainId`,
- a broadcast scope (`AllWindows`, `AllDomains`).

No implicit inheritance or fallback is permitted.

### 4.3 Endpoints

An **endpoint** is a pure data handle identifying a receiver.

- Endpoints do not own state.
- Endpoints do not imply authority.
- An endpoint may be optionally passed at window creation to allow private channels.

---

## 5. Message Model

### 5.1 Envelope

Every signal is wrapped in a stable envelope:

```rust
struct SignalEnvelope {
    source: SignalSource,
    target: SignalTarget,
    kind: SignalKind,
    payload: SignalPayload,
}
````

### 5.2 Source

```rust
enum SignalSource {
    Window(WindowId),
    Domain(InteractionDomainId),
    System,
}
```

### 5.3 Target

```rust
enum SignalTarget {
    Window(WindowId),
    Domain(InteractionDomainId),
    BroadcastWindows,
    BroadcastDomains,
}
```

### 5.4 Kind

```rust
enum SignalKind {
    Request,
    Response,
    Notification,
}
```

### 5.5 Payload (Typed Payload Law)

Payloads **must be closed and inspectable**.

Allowed forms:

* a closed enum defined in core,
* an application-defined numeric message id + byte payload,
* another explicitly typed, non-dynamic structure.

Forbidden:

* `Any`,
* trait-object downcasting,
* hidden dynamic dispatch.

---

## 6. Delivery Semantics

### 6.1 Queueing

* Signals are enqueued immediately.
* Enqueueing is allowed from any engine-safe context.
* No synchronous delivery is permitted.

### 6.2 Delivery Phase (BINDING)

Signals are delivered **only** during a designated engine step:

> **Signal Delivery Phase**
> Runs after input routing and before frame finalization.
> Never during layout, scene build, or paint.

**Exact Wiring Point (Binding):**

```
Location: RedrawRequested handler (application event loop)
After:    input_fusion.process_frame() + event_router.route_input_event() loop
Before:   run_single_frame() (layout + scene + paint)

Code:     src/core/app.rs::deliver_signals(&mut signal_bus, handler)
```

**Example Wiring:**

```rust
// In RedrawRequested handler:

// 1. Frame timing
let frame_time = ticker.begin_frame();

// 2. Input processing
input_fusion.process_frame(&frame_time, &mut ticker, &mut fused_events);
for fused in fused_events.drain(..) {
    event_router.route_input_event(...);
}

// 3. Signal Delivery Phase (SPEC-HAL-161)
deliver_signals(&mut signal_bus, |envelope| {
    // Handle signal here
});

// 4. Layout + Paint + Render
run_single_frame(...);
```

Delivery may only:

* notify registered handlers,
* enqueue deferred mutations,
* update non-visual state.

### 6.3 No Re-entrancy

Signal delivery **must not**:

* route input events,
* perform hit-testing,
* trigger layout,
* build scene graphs,
* invoke compositor logic.

Violations are fatal invariant breaches.

---

## 7. Backpressure Policy

The signal bus is **bounded**.

Policy (binding):

* **Drop-oldest** when capacity is exceeded.

Rationale:

* Prevents unbounded memory growth.
* Preserves most recent intent.
* Deterministic and testable.

---

## 8. Window Creation Integration

If a window creation API exists:

* it **may** accept an optional `SignalEndpoint`,
* the endpoint is attached as a pure receiver handle,
* no semantic authority is implied.

If no such API exists yet, this SPEC introduces **no new window framework**.

---

## 9. Interaction-Domain Compatibility

Signals are **not input**.

* They do not affect focus, capture, modality, or routing.
* They do not cross domain boundaries implicitly.
* A domain may only observe signals explicitly addressed to it.

---

## 10. Engine-Grade Invariants (Binding)

**INV-XWIN-001 — Authority**
Cross-window communication is engine-internal and MUST NOT grant semantic authority to OS windows or platform callbacks.

**INV-XWIN-002 — No implicit fan-out**
Signals MUST NOT deliver to any target not explicitly addressed, except via explicit broadcast APIs.

**INV-XWIN-003 — Phase-correct delivery**
Signals MUST be delivered only during the Signal Delivery Phase.

**INV-XWIN-004 — No re-entrancy**
Signal delivery MUST NOT synchronously invoke routing, layout, scene build, or compositor operations.

**INV-XWIN-005 — Deterministic ordering**
Given identical enqueue order, delivery order MUST be identical.

**INV-XWIN-006 — Isolation compatibility**
Signal delivery MUST NOT alter focus, capture, modality, or cause cross-domain input effects.

**INV-XWIN-007 — Typed payload law**
Payloads MUST be stable and inspectable; dynamic downcasting is forbidden.

**INV-XWIN-008 — Bounded growth**
The bus MUST enforce a bounded queue with drop-oldest semantics.

---

## 11. Examples (Non-Normative)

* A CRUD dialog window sends a `Response` signal to its opener window.
* A settings window broadcasts a `Notification` to all windows.
* A tool window uses a private endpoint passed at spawn to communicate with its parent.

---

## 12. Open Questions

* Whether request/response correlation ids should be standardized or left to payloads.
* Whether domain-level handlers should be pull- or push-registered.

These do not block initial implementation.

---

---

## 13. Implementation (Binding)

### 13.1 Core Types

| Type | Location | Description |
|------|----------|-------------|
| `SignalBus` | `src/core/signal_bus.rs` | System-scoped message queue |
| `SignalEnvelope` | `src/core/signal_bus.rs` | Source + target + kind + payload |
| `SignalSource` | `src/core/signal_bus.rs` | Window / Domain / System |
| `SignalTarget` | `src/core/signal_bus.rs` | Window / Domain / Broadcast |
| `SignalPayload` | `src/core/signal_bus.rs` | Typed, closed enum |
| `SignalEndpoint` | `src/core/signal_bus.rs` | Pure data handle for addressing |

### 13.2 Wiring

| Function | Location | Description |
|----------|----------|-------------|
| `deliver_signals()` | `src/core/app.rs` | Phase-correct delivery helper |
| `in_frame_critical_section()` | `src/core/app.rs` | Phase guard: true during layout/scene/paint |
| `frame_critical_section_enter()` | `src/core/app.rs` | Enter forbidden zone (called by run_single_frame) |
| `frame_critical_section_exit()` | `src/core/app.rs` | Exit forbidden zone (called by run_single_frame) |
| `Window::with_endpoint()` | `src/platform/window.rs` | Window spawn with endpoint hook |

**Phase Enforcement (INV-XWIN-003):**

`deliver_signals()` asserts in debug builds that `in_frame_critical_section() == false`.
`run_single_frame()` sets the critical section flag, making signal delivery during layout/scene/paint a fatal assertion failure.

### 13.3 Invariant Enforcement Tests

| Invariant | Test | Location |
|-----------|------|----------|
| INV-XWIN-001 | `inv_xwin_001_authority_no_os_handles` | `src/core/signal_bus.rs` |
| INV-XWIN-002 | `inv_xwin_002_addressing_isolation` | `src/core/signal_bus.rs` |
| INV-XWIN-003 | `inv_xwin_003_phase_correctness_structure` | `src/core/signal_bus.rs` |
| INV-XWIN-004 | `inv_xwin_004_reentrancy_guard` | `src/core/signal_bus.rs` |
| INV-XWIN-005 | `inv_xwin_005_deterministic_ordering` | `src/core/signal_bus.rs` |
| INV-XWIN-005 | `inv_xwin_005_determinism_repeatable` | `src/core/signal_bus.rs` |
| INV-XWIN-006 | `inv_xwin_006_isolation_compatibility` | `src/core/signal_bus.rs` |
| INV-XWIN-007 | `inv_xwin_007_typed_payload` | `src/core/signal_bus.rs` |
| INV-XWIN-008 | `inv_xwin_008_bounded_backpressure` | `src/core/signal_bus.rs` |

### 13.4 Cross-Window Flow Regression Tests (Headless, CI-Safe)

| Test | Description | Location |
|------|-------------|----------|
| `SPEC_hal_161_cross_window_signal_flow` | Dialog confirm → state mutation | `src/core/signal_bus.rs` |
| `SPEC_hal_161_request_response_pattern` | Bidirectional request/response | `src/core/signal_bus.rs` |
| `SPEC_hal_161_dialog_cancel_preserves_state` | Cancel preserves original state | `src/core/signal_bus.rs` |

These tests exercise the complete enqueue → deliver → state mutation cycle without renderer, windows, or winit.

---

## 14. Summary

SPEC-HAL-161 introduces a **minimal, sovereign, deterministic, and safe** cross-window signaling substrate.

It adds **communication without authority**, **coordination without coupling**, and **flexibility without policy**,
preserving Halcyon's core sovereignty guarantees.

No UI behavior is implied. No architectural shortcuts are taken.

---

## 15. Explicit Non-Goals / Out of Scope

The following are **explicitly NOT part of SPEC-HAL-161**:

- **Window lifecycle teardown** — What happens to pending signals when a window dies is NOT defined here.
- **Signal endpoint invalidation** — How endpoints become invalid after window close is NOT defined here.
- **Dead-letter handling** — Signals addressed to closed windows are NOT handled by this SPEC.
- **Graceful shutdown sequencing** — Order of operations during application exit is NOT defined here.

These concerns are **deferred to SPEC-HAL-162** (Window and Domain Lifecycle and Signal Teardown).

SPEC-HAL-161 defines signal **delivery semantics** only. Lifecycle and teardown are separate concerns.

---

## 16. Status: CLOSED / Binding

**SPEC-HAL-161 is formally CLOSED.**

### 16.1 Enforced Invariants

| Invariant | Description | Enforcement |
|-----------|-------------|-------------|
| INV-XWIN-001 | No OS authority via signals | Type design (WindowId not RawWindowHandle) |
| INV-XWIN-002 | No implicit fan-out | Explicit SignalTarget enum |
| INV-XWIN-003 | Phase-correct delivery | `in_frame_critical_section()` debug assert |
| INV-XWIN-004 | No re-entrancy | `delivering` flag + debug assert |
| INV-XWIN-005 | Deterministic FIFO ordering | VecDeque + drain order |
| INV-XWIN-006 | Isolation compatibility | Payload enum excludes focus/capture types |
| INV-XWIN-007 | Typed payload law | Closed SignalPayload enum |
| INV-XWIN-008 | Bounded growth | Capacity + drop-oldest policy |

### 16.2 Enforcement Locations

| Mechanism | File | Function |
|-----------|------|----------|
| Phase guard flag | `src/core/app.rs` | `IN_FRAME_CRITICAL_SECTION` thread-local |
| Phase guard enter | `src/core/app.rs` | `frame_critical_section_enter()` |
| Phase guard exit | `src/core/app.rs` | `frame_critical_section_exit()` |
| Phase guard query | `src/core/app.rs` | `in_frame_critical_section()` |
| Phase assertion | `src/core/app.rs` | `deliver_signals()` |
| Re-entrancy guard | `src/core/signal_bus.rs` | `drain_pending()` |
| Capacity enforcement | `src/core/signal_bus.rs` | `enqueue()` lines 246-255 |

### 16.3 Test Coverage

**Invariant Tests (9 tests):**
- `inv_xwin_001_authority_no_os_handles`
- `inv_xwin_002_addressing_isolation`
- `inv_xwin_003_phase_correctness_structure`
- `inv_xwin_004_reentrancy_guard`
- `inv_xwin_005_deterministic_ordering`
- `inv_xwin_005_determinism_repeatable`
- `inv_xwin_006_isolation_compatibility`
- `inv_xwin_007_typed_payload`
- `inv_xwin_008_bounded_backpressure`

**Regression Tests (3 tests, headless, CI-safe):**
- `SPEC_hal_161_cross_window_signal_flow`
- `SPEC_hal_161_request_response_pattern`
- `SPEC_hal_161_dialog_cancel_preserves_state`

**Demo:**
- `examples/signal_dialog_demo.rs` — exercises window-to-window signals with visible output

### 16.4 Closure Statement

All invariants are mechanically enforced. All enforcement locations are documented.
Headless regression tests prove cross-window semantics without visual infrastructure.
SPEC-HAL-161 is **complete and closed**. Future work on lifecycle belongs to SPEC-HAL-162.
