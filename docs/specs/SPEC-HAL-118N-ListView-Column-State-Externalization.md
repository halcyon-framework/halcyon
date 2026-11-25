# SPEC-HAL-118N: ListView Column State Externalization

**Status:** Deprecated
**Layer:** ListView / State Ownership

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---

**Depends on:**

* SPEC-HAL-118B – ListView Columnar Contract
* SPEC-HAL-118C – ListView Column Presentation
* SPEC-HAL-118F – ListView Column Resize Contract
* SPEC-HAL-118H – ListView Sort Affordance Contract
* SPEC-HAL-118K – ListView Column Pinning Preparation

---

## 1. Purpose

This SPEC defines **state ownership and externalization rules for ListView columns**.

It establishes a strict contract for:

* which column-related states are **owned externally**
* which states ListView may **observe but not own**
* how state changes are **emitted**
* what ListView is allowed to **cache vs derive**

This SPEC prevents silent divergence between UI state and application state.

---

## 2. Core Principle

**ListView is a projector, not a state owner.**

ListView MUST NOT be the authoritative owner of column state.

---

## 3. Column State Categories

The following column-related properties are considered **stateful**:

* width
* alignment
* sort direction
* sort priority
* pinned status (future)
* visibility (future)

All of these MUST follow the externalization rules defined here.

---

## 4. Authoritative Ownership

### 4.1 External Owner

The **host application** (or data/controller layer) is the sole authoritative owner of column state.

ListView MUST NOT persist or mutate authoritative state internally.

---

### 4.2 ListView Responsibilities

ListView MAY:

* cache last-applied state for rendering efficiency
* compare incoming state to previous state
* emit intents or change requests

ListView MUST NOT:

* invent state
* resolve conflicts between competing states
* silently override host-provided state

---

## 5. State Input Model

Column state MUST be provided to ListView via explicit configuration inputs.

Examples (non-exhaustive):

* column width map
* column alignment map
* sort descriptors
* pinning descriptors
* visibility flags

State input is **pull-based**: ListView renders whatever state it is given.

---

## 6. State Change Emission

When user interaction implies a state change, ListView MUST:

1. Emit a **state intent event**
2. Include sufficient metadata to allow the host to decide
3. NOT mutate internal authoritative state

Examples:

* resize drag → emit `ColumnWidthChangeIntent`
* header click → emit `SortChangeIntent`
* future pin action → emit `ColumnPinIntent`

---

## 7. Reconciliation Cycle

The correct update cycle is:

1. User interacts with ListView
2. ListView emits intent
3. Host updates authoritative state
4. Host re-provides updated state to ListView
5. ListView re-renders

ListView MUST tolerate delayed or rejected updates.

---

## 8. No Implicit Persistence

ListView MUST NOT:

* store column state across rebuilds
* assume state continuity across lifetimes
* serialize column state implicitly

If persistence is desired, it is the host’s responsibility.

---

## 9. Sorting as Precedent

SPEC-HAL-118H already follows this model:

* ListView emits sort intent
* Host owns sort order
* ListView reflects provided sort state

This SPEC generalizes that rule to **all column state**.

---

## 10. Forbidden Behavior

The following are violations:

* ListView resizing a column without emitting intent
* ListView persisting width internally
* ListView resolving conflicting column states
* ListView applying speculative state changes

---

## 11. Invariants

**INV-COL-STATE-1**
All column state is externally owned.

**INV-COL-STATE-2**
ListView emits intents, never commits state.

**INV-COL-STATE-3**
Rendered column state always reflects host-provided state.

---

## 12. Relationship to Other SPECs

* SPEC-HAL-118F defines resize mechanics, not ownership
* SPEC-HAL-118H defines sort semantics, not persistence
* SPEC-HAL-118K prepares pinning, which MUST follow this model
* SPEC-HAL-118O (if added) will extend this to visibility

---

## 13. Rationale

State leakage is subtle and catastrophic.

Once ListView starts “helpfully” owning state, the architecture collapses into:

* hidden coupling
* unsynchronized UI
* impossible debugging

This SPEC forbids that path entirely.

---

## 14. Summary

Column state lives **outside** ListView.

ListView is a pure projection of externally-owned state plus emitted intents.

No exceptions.

---

## Implementation Notes (ListView v1)

Column state is externalized to `ColumnStructureController`. ListView emits `ColumnStructureDirective` (Resize, Reorder, etc.) via `take_pending_column_action()` and `take_pending_resize_directive()`. The host applies directives to the controller, then syncs updated state back to ListView via `set_columns()`.

---

## Implementation Reference

- `ColumnStructureController` — external column state owner
- `ColumnStructureDirective` — resize/reorder intent
- `ListView::take_pending_column_action()` — emit pending actions
- `ListView::take_pending_resize_directive()` — emit resize directive
- `ListView::set_columns()` — sync state from controller
