**SPEC-HAL-162 — Window and Domain Lifecycle and Signal Teardown**

**Status:** CLOSED / Binding
**Layer:** Core / Engine Substrate
**Scope:** WindowId and InteractionDomainId lifecycle, signal teardown semantics
**Depends on:** SPEC-HAL-002, SPEC-HAL-082A, SPEC-HAL-151, SPEC-HAL-161
**Conflicts with:** Semantic WindowManager, implicit signal rerouting, OS-driven lifecycle authority

---

## 1. Problem Statement

Halcyon now supports binding-grade cross-window signaling (SPEC-HAL-161).
However, without explicit lifecycle and teardown semantics, the following remain undefined:

* Signals targeting windows/domains that are closing or already closed
* Ordering between teardown and pending signal delivery
* Validity of `WindowId`, `InteractionDomainId`, and `SignalEndpoint` over time
* Whether “send-after-close” is an error, a drop, or undefined behavior

These ambiguities will surface under ListView, dialog workflows, and multi-window stress.
This SPEC defines **hard lifecycle laws** so teardown behavior is deterministic, testable, and sovereign.

---

## 2. Design Principles

1. **Lifecycle is engine-owned**
   OS notifications are advisory only. Halcyon decides validity.

2. **Invalid targets never resurrect**
   Once a window or domain is closed, no signal may reach it.

3. **Silence over magic**
   No implicit rerouting, rebinding, or fallback delivery.

4. **Determinism over convenience**
   Same sequence → same drops, every time.

5. **No new authorities**
   This SPEC adds rules, not managers.

---

## 3. Terminology

* **Alive**: A window/domain registered and valid for signal delivery
* **Closing**: Teardown initiated but not yet finalized
* **Closed**: Fully torn down; identity permanently invalid
* **Pending Signal**: Enqueued in `SignalBus` but not yet delivered

---

## 4. Lifecycle States

### 4.1 Window Lifecycle

```
Created → Alive → Closing → Closed
```

* **Created**: `WindowId` allocated, not yet exposed
* **Alive**: Eligible for signal delivery
* **Closing**: Teardown initiated; no new delivery permitted
* **Closed**: Identity invalid forever

### 4.2 Interaction Domain Lifecycle

```
Created → Alive → Closing → Closed
```

Domains follow identical semantics, independent of windows.

---

## 5. Signal Delivery Rules (Binding)

### 5.1 Target Validity at Delivery Time

Signal delivery checks validity **at delivery**, not enqueue time.

| Target State | Delivery Result |
|--------------|-----------------|
| Alive        | Delivered       |
| Closing      | Dropped         |
| Closed       | Dropped         |

Dropped signals are **silently discarded** in release builds.

---

### 5.2 Send-After-Close Semantics

* Enqueueing a signal to a **Closed** target:

    * **Debug**: assertion / diagnostic log
    * **Release**: dropped silently

* Enqueueing to **Closing** target:

    * Allowed (enqueue is always safe)
    * Guaranteed to be dropped at delivery

This preserves enqueue safety (SPEC-161) without reviving dead targets.

---

### 5.3 Broadcast Semantics

For broadcast targets:

* Each window/domain is evaluated independently
* Closed or closing recipients are skipped
* No partial failure state exists

Broadcast is **best-effort over alive targets only**.

---

## 6. SignalEndpoint Semantics

* `SignalEndpoint` validity is strictly tied to its owner
* When a window/domain enters **Closing**:

    * Its endpoint is immediately invalid for delivery
* Endpoints are never reassigned or recycled

There is no global endpoint registry accessible to user code.

---

## 7. Ordering Guarantees

1. Teardown always precedes delivery in the same frame
2. A signal enqueued **before** teardown but delivered **after** teardown is dropped
3. No signal may observe a half-torn-down target

This is enforced by ordering teardown **before** `deliver_signals()` in the frame.

---

## 8. Forbidden Behaviors (Absolute)

The engine MUST NOT:

* Reroute signals from dead targets
* Auto-rebind endpoints
* Deliver teardown notifications as implicit signals
* Allow OS callbacks to trigger signal delivery
* Delay teardown to “flush” signals

Any of the above requires a new SPEC.

---

## 9. Engine-Grade Invariants (Binding)

**INV-LIFE-001 (Identity Finality)**
Once a `WindowId` or `InteractionDomainId` enters `Closed`, it MUST NEVER become valid again.

**INV-LIFE-002 (No Post-Mortem Delivery)**
Signals MUST NOT be delivered to targets in `Closing` or `Closed` state.

**INV-LIFE-003 (Delivery-Time Validation)**
Target validity MUST be checked at delivery, not enqueue.

**INV-LIFE-004 (Silent Drop Policy)**
Dropped signals MUST NOT cause side effects in release builds.

**INV-LIFE-005 (Endpoint Invalidation)**
`SignalEndpoint` becomes invalid immediately when its owner enters `Closing`.

**INV-LIFE-006 (No Implicit Compensation)**
The engine MUST NOT reroute, duplicate, or “make up for” dropped signals.

**INV-LIFE-007 (Deterministic Teardown Ordering)**
Given identical frame ordering, signal drops vs delivery MUST be identical.

---

## 10. Integration Points (Binding)

* Teardown logic: window/domain destruction paths
* Signal delivery: `core::app::deliver_signals()` (SPEC-161)
* Validity checks: internal to `SignalBus` or its dispatcher

No new public APIs are introduced.

---

## 11. Non-Goals

This SPEC does NOT define:

* UI close confirmations
* Modal stack unwinding
* Persistence or restore
* Cross-window drag-and-drop
* Application-level policies

---

## 12. Implementation (Binding)

### 12.1 Lifecycle State Storage

| Component | Storage | Description |
|-----------|---------|-------------|
| `LifecycleRegistry` | `src/core/lifecycle.rs` | Central registry for WindowId/DomainId state |
| `LifecycleState` | `src/core/lifecycle.rs` | Enum: `Created`, `Alive`, `Closing`, `Closed` |

The registry is owned by the engine core and is the **single source of truth** for identity validity.

### 12.2 State Transitions

| Transition | Trigger | Location |
|------------|---------|----------|
| Created → Alive | `register_window()` / `register_domain()` | `src/core/lifecycle.rs` |
| Alive → Closing | `begin_teardown()` | `src/core/lifecycle.rs` |
| Closing → Closed | `finalize_teardown()` | `src/core/lifecycle.rs` |

**State machine is one-way.** No backward transitions permitted.

### 12.3 Delivery-Time Validation (INV-LIFE-003)

`deliver_signals()` checks target validity before invoking handler:

```rust
// Inside deliver_signals() loop:
for envelope in signals {
    if !lifecycle.is_alive(&envelope.target) {
        // INV-LIFE-002: No post-mortem delivery
        // INV-LIFE-004: Silent drop in release
        #[cfg(debug_assertions)]
        tracing::debug!("Signal dropped: target {:?} not alive", envelope.target);
        continue;
    }
    handler(envelope);
}
```

### 12.4 Identity Finality (INV-LIFE-001)

Once `finalize_teardown()` is called:
- The ID enters `Closed` state
- The registry retains a "tombstone" entry (ID → Closed)
- Any future validity check returns false
- IDs are **never recycled** within a session

### 12.5 Endpoint Invalidation (INV-LIFE-005)

When a window/domain enters `Closing`:
1. `begin_teardown()` marks state as `Closing`
2. `SignalEndpoint::is_valid()` queries the registry
3. Returns `false` for `Closing` or `Closed` states

### 12.6 Frame Ordering (INV-LIFE-007)

Teardown MUST occur in a specific phase:

```
Frame Pipeline:
1. Input processing (input_fusion + event_router)
2. Teardown phase ← NEW: process pending teardowns
3. Signal delivery (deliver_signals)
4. Layout + scene + paint (run_single_frame)
```

This ensures signals enqueued before teardown are dropped at delivery time (step 3),
since teardown (step 2) runs first.

**Wiring point:**

```rust
// In RedrawRequested handler:
input_fusion.process_frame(...);
for fused in fused_events.drain(..) { ... }

// 2. Teardown phase (SPEC-HAL-162)
lifecycle.process_pending_teardowns();

// 3. Signal delivery (SPEC-HAL-161)
deliver_signals(&mut signal_bus, &lifecycle, |envelope| { ... });

// 4. Layout + Paint
run_single_frame(...);
```

### 12.7 Edge Cases

| Scenario | Behavior | Invariant |
|----------|----------|-----------|
| Signal enqueued, window closes before delivery | Signal dropped at delivery | INV-LIFE-002 |
| Signal to window in `Closing` state | Dropped | INV-LIFE-002 |
| Signal to `Closed` window | Dropped (debug: log) | INV-LIFE-002, INV-LIFE-004 |
| Broadcast to mixed alive/closed targets | Alive receive, closed skip | INV-LIFE-006 |
| Enqueue during teardown phase | Allowed, dropped at delivery | INV-LIFE-003 |

### 12.8 Required Test Coverage

| Invariant | Test | Description |
|-----------|------|-------------|
| INV-LIFE-001 | `inv_life_001_identity_finality` | Closed IDs never become valid |
| INV-LIFE-002 | `inv_life_002_no_post_mortem_delivery` | Signals to Closing/Closed dropped |
| INV-LIFE-003 | `inv_life_003_delivery_time_validation` | Validity checked at delivery |
| INV-LIFE-004 | `inv_life_004_silent_drop` | No side effects on drop |
| INV-LIFE-005 | `inv_life_005_endpoint_invalidation` | Endpoint invalid when owner Closing |
| INV-LIFE-006 | `inv_life_006_no_compensation` | No rerouting on drop |
| INV-LIFE-007 | `inv_life_007_deterministic_ordering` | Same order → same drops |

---

## 13. Implementation Questions (Resolved)

### Q1: Where is lifecycle state stored?
**A**: `LifecycleRegistry` in `src/core/lifecycle.rs` — single source of truth.

### Q2: How do transitions happen?
**A**: Explicit function calls: `register_*()`, `begin_teardown()`, `finalize_teardown()`.

### Q3: How is validity checked at delivery?
**A**: `deliver_signals()` calls `lifecycle.is_alive(&target)` before each handler invocation.

### Q4: How is identity finality enforced?
**A**: Tombstone entries remain in registry. IDs never recycled. `is_alive()` returns false for Closed.

### Q5: How does endpoint invalidation work?
**A**: `SignalEndpoint::is_valid()` delegates to `lifecycle.is_alive()`. Returns false for Closing/Closed.

### Q6: Where does teardown fit in the frame?
**A**: After input processing, before signal delivery. Ensures pending signals dropped at delivery.

### Q7: What about signals in flight during close?
**A**: Enqueue always succeeds (safe). Delivery-time validation drops them. No exceptions.

---

## 14. Summary

SPEC-HAL-162 seals lifecycle behavior for windows and interaction domains.

It guarantees:

* No ghost delivery
* No resurrection
* No timing-dependent ambiguity

This closes the last structural hole before ListView-scale pressure.

---

## 15. Status: CLOSED

### 15.1 Implementation Complete

All implementation items have been completed:

- [x] `src/core/lifecycle.rs` with `LifecycleRegistry` and `LifecycleState`
- [x] `register_window()`, `activate_window()`, `register_domain()`, `activate_domain()`
- [x] `begin_window_teardown()`, `finalize_window_teardown()` (and domain equivalents)
- [x] `is_window_alive()`, `is_domain_alive()`, `is_target_alive()`
- [x] `deliver_signals()` accepts `&LifecycleRegistry` and validates targets at delivery time
- [x] All 7 invariant tests implemented and passing

### 15.2 Enforcement Points

| Enforcement | Location | Function |
|-------------|----------|----------|
| Lifecycle state storage | `src/core/lifecycle.rs` | `LifecycleRegistry` |
| State transitions | `src/core/lifecycle.rs` | `begin_*_teardown()`, `finalize_*_teardown()` |
| Delivery-time validation | `src/core/app.rs` | `deliver_signals(&mut signal_bus, &lifecycle, handler)` |
| Identity finality | `src/core/lifecycle.rs` | Tombstone retention, no ID recycling |

### 15.3 Test Coverage

All invariant tests pass (`cargo test --release --lib -- lifecycle::tests`):

| Test | Status |
|------|--------|
| `inv_life_001_identity_finality_window` | ✅ |
| `inv_life_001_identity_finality_domain` | ✅ |
| `inv_life_002_no_post_mortem_closing` | ✅ |
| `inv_life_002_no_post_mortem_closed` | ✅ |
| `inv_life_003_delivery_time_validation` | ✅ |
| `inv_life_004_silent_drop` | ✅ |
| `inv_life_005_endpoint_invalidation` | ✅ |
| `inv_life_006_no_compensation` | ✅ |
| `inv_life_007_deterministic_ordering` | ✅ |

### 15.4 Demo Status

`signal_dialog_demo.rs` is **scene-only by design**. Per-window widget UI requires surface-scoped scene partitioning (INV-BG-PREPASS), which is not implemented. The demo validates signal semantics; visual rendering is secondary.

### 15.5 Closure Confirmation

This SPEC is formally closed with the following confirmations:

1. **LifecycleRegistry is the single source of truth** for WindowId and InteractionDomainId validity.

2. **Identity finality is enforced** — once an ID enters `Closed` state, it cannot resurrect. Tombstone entries prevent ID reuse.

3. **Delivery-time validation is authoritative** — `deliver_signals()` checks `lifecycle.is_target_alive()` before every handler invocation.

4. **Silent drop policy is intentional and final** — dropped signals cause no side effects in release builds. Debug builds log the drop.

5. **No dead-letter queues, no compensation** — signals to dead targets are discarded. No rerouting, no fallback, no retry.

---

## 16. Final Statement

SPEC-HAL-162 is complete. No further lifecycle or teardown semantics are permitted outside this SPEC.

**End of SPEC-HAL-162**
