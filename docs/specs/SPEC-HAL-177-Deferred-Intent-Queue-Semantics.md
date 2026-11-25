# SPEC-HAL-177: Deferred Intent Queue Semantics for TabularSubstrate

**Status:** Implemented / Closed (historical; partially superseded by HAL-177A)
**Scope:** TabularSubstrate intent infrastructure
**Depends on:** SPEC-HAL-174 (Edit Authority)
**Binding addendum:** [SPEC-HAL-177A](SPEC-HAL-177A-Deferred-Intent-Delivery-and-Single-Consumption.md)
reconciles admission, execution authority and single consumption. Its
superseding clauses govern over the historical rules and closure below.

---

## Summary

`TabularSubstrate` contains an internal intent queue (`pending_intents`) with a private entry point (`emit_intent`).
This queue is **intentional infrastructure**, not dead code. It exists to support deferred, phase-bound, and batch-safe
intent processing. Its presence, shape, and guards are governed by this SPEC.

Removal, public exposure, or bypass of this queue is **architecturally illegal** unless explicitly superseded by a later
SPEC.

---

## Motivation

Current production paths deliver intents directly to `handle_intent`.
The deferred path (`emit_intent → pending_intents → process_pending_intents → handle_intent`) has no production callers
yet, but exists by design to support:

* Phase-bound intent application (e.g. layout start)
* Prevention of intent accumulation during exclusive authority states (SPEC-HAL-174)
* Future batching, coalescing, or deferral without API breakage

Without formalization, this queue is indistinguishable from accidental or removable code. This SPEC fixes that.

---

## Design

### Queue Entry (`emit_intent`)

* `emit_intent` is **private**.
* It is the **sole location** where `pending_intents` may be mutated.
* It enforces **edit authority at queue entry**:

    * While an edit session is active, only `Edit` and `Resize` intents are accepted.
    * `Focus`, `Sort`, and `Tree` intents are silently dropped.
* Purpose: prevent accumulation and reordering of forbidden intents.

### Execution Authority (`handle_intent`)

* `handle_intent` is **public** and remains the **only execution authority**.
* It enforces edit authority at execution time:

    * Debug: panic on forbidden intents.
    * Release: ignore forbidden intents.
* All state mutation in response to intents occurs here.

### Queue Processing (`process_pending_intents`)

* Private.
* Drains `pending_intents` **exclusively** through `handle_intent`.
* Called at a defined phase boundary (currently layout start).
* Preserves FIFO order.

### Queue Drain (`take_intents`)

* `take_intents` is a **pure drain** of `pending_intents`.
* It performs **no filtering, validation, or state mutation**.
* It introduces **no authority semantics** of its own.
* All authority enforcement must already have occurred at:

    * Entry (`emit_intent`) or
    * Execution (`handle_intent`).

---

## Intended Uses (Future)

The queue exists to enable, without redesign:

* Layout-phase batching of intents
* Transition- or animation-gated execution
* Coalescing of redundant intents
* Internal scheduling without exposing new APIs

No commitment is made to when or if these uses will be activated.

---

## Non-Goals

* No requirement that all intents must be queued.
* No public or crate-visible queue API.
* No promise of production callers.
* No timeline for deferred processing features.

---

## Invariants

The following are **binding architectural invariants**:

1. **INVARIANT-QUEUE-1:** `pending_intents` must exist as a field on `TabularSubstrate`.
2. **INVARIANT-QUEUE-2:** `emit_intent` is the **sole** method allowed to push into `pending_intents`.
3. **INVARIANT-QUEUE-3:** `emit_intent` must remain **private**.
4. **INVARIANT-QUEUE-4:** `emit_intent` must enforce SPEC-HAL-174 edit authority at queue entry.
5. **INVARIANT-QUEUE-5:** `process_pending_intents` must drain the queue only via `handle_intent`.
6. **INVARIANT-QUEUE-6:** No direct `pending_intents.push()` may exist outside `emit_intent`.
7. **INVARIANT-QUEUE-7:** `take_intents` must remain a pure drain and must not enforce authority or mutate state.

Violation or removal of these invariants requires an explicit superseding SPEC.

---

## References

* `src/widgets/tabular_view/mod.rs`:

    * `emit_intent`
    * `handle_intent`
    * `process_pending_intents`
    * `take_intents`
* SPEC-HAL-174: Edit Authority and Exclusive Edit Semantics

---

## Closure

**This SPEC is fully implemented and closed.** Implementation: `src/widgets/tabular_view/mod.rs` (`emit_intent`, `pending_intents`, `process_pending_intents`, `take_intents`). All invariants (INVARIANT-QUEUE-1 through INVARIANT-QUEUE-7) enforced by private visibility, single-entry-point mutation, and edit authority gating at queue entry. No further work permitted under this SPEC.
