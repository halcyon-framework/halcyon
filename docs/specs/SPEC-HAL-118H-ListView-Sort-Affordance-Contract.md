# SPEC-HAL-118H: ListView Sort Affordance Contract

**Status:** Deprecated
**Layer:** Interaction Affordance / UI Signaling

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked partially implemented. Sort intent signaling deferred to ListView.

---
**Scope:** Sort affordances and signaling for ListView columns
**Depends on:** SPEC-HAL-118C, SPEC-HAL-118G
**Explicitly Does NOT Depend On:** Data model, sorting execution, query logic
**Non-Goals:** Implementing sorting, multi-key sorting logic, filtering, persistence

---

## 1. Purpose

This SPEC defines how **ListView exposes sorting affordances** on columns and how it
**signals sort intent** to the outside world.

It deliberately does **not** define how sorting is performed.

Sorting is a **data-layer concern**.
ListView is a **pure presentation + interaction surface**.

---

## 2. Core Principle

**ListView exposes intent, never behavior.**

ListView MAY:

* render sort indicators
* react to user interaction on column headers
* emit declarative sort intent signals

ListView MUST NOT:

* reorder rows
* mutate List state
* infer data semantics
* cache or apply sorting internally

---

## 3. Sort Affordance States

Each column header may expose a sort affordance with one of the following states:

* Unsorted
* Ascending
* Descending

No other states are permitted at the ListView level.

---

## 4. Column Sort Capability

Sorting is **opt-in per column**.

A column MAY:

* declare itself sortable
* declare itself non-sortable

Non-sortable columns:

* MUST NOT render sort affordances
* MUST NOT emit sort signals
* MUST ignore pointer interaction related to sorting

---

## 5. User Interaction Contract

### 5.1 Primary Interaction

* Primary click on a sortable column header emits a sort intent signal
* Default toggle sequence is:

    * Unsorted → Ascending → Descending → Unsorted

ListView MUST NOT assume this sequence is applied by the data layer.

---

### 5.2 Modifier Keys

ListView MAY capture modifier keys (Shift / Ctrl / Cmd) and include them in the emitted signal.

ListView MUST NOT interpret modifier semantics itself.

Example (non-normative):

* Shift-click could mean multi-column sort to the data layer
* Ctrl-click could mean additive sorting

These meanings are **external**.

---

## 6. Sort Intent Signal

ListView emits a **pure declarative signal**:

* Column identifier
* Requested direction (Ascending / Descending / None)
* Modifier flags (if any)

No ordering, priority, or execution semantics are implied.

---

## 7. Visual Indicators

If a column is sortable, ListView MAY render:

* sort arrow
* chevron
* caret
* glyph

Visual design is intentionally unspecified.

Indicators MUST:

* reflect the last known external sort state
* be driven by externally supplied state

ListView MUST NOT infer current sort state internally.

---

## 8. External State Injection

ListView MUST allow the host to inject current sort state per column.

This enables:

* reflecting backend-driven sorting
* synchronizing multiple views
* restoring UI state after reload

ListView treats injected state as authoritative.

---

## 9. Multi-Column Sorting

ListView:

* MAY visually indicate multiple sorted columns
* MAY emit multiple sort signals over time

ListView MUST NOT:

* assign priority
* reorder precedence
* resolve conflicts

Multi-column sorting is entirely external.

---

## 10. Accessibility Contract

Sortable headers MUST:

* be focusable if keyboard navigation is enabled
* expose their sortable nature to accessibility systems
* expose current sort state where applicable

---

## 11. Non-Goals (Explicit)

This SPEC does NOT define:

* sorting algorithms
* data access patterns
* pagination
* filtering
* search
* persistence of sort state
* default sort behavior

---

## 12. Invariants

* INV-SORT-1: ListView never mutates row order
* INV-SORT-2: Sort state is externally owned
* INV-SORT-3: Sort affordance is column-scoped
* INV-SORT-4: Sort intent signaling is declarative only
* INV-SORT-5: Visual indicators reflect injected state, not internal inference

---

## 13. Summary

ListView sorting support is **UI signaling only**.

This preserves:

* strict separation of concerns
* composability with any data backend
* consistency with Halcyon’s stateless widget model

Sorting exists **around** ListView, never **inside** it.
