# SPEC-HAL-118M: ListView Header Interaction Routing

**Status:** Deprecated
**Layer:** ListView / Header Interaction Routing

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---

**Depends on:**

* SPEC-HAL-118B – ListView Columnar Contract
* SPEC-HAL-118L – ListView Column Hit-Zone Contract
* SPEC-HAL-118F – ListView Column Resize Contract
* SPEC-HAL-118H – ListView Sort Affordance Contract
* SPEC-HAL-118J – ListView Column Separator Model
* SPEC-HAL-038 – Focus Model & Input Routing
* SPEC-HAL-128 – Pointer Capture & Gesture Arbitration
* SPEC-HAL-009 – Event Routing & Hit-Testing

---

## 1. Purpose

This SPEC defines **how pointer and gesture events are routed inside the ListView HeaderBand**.

It establishes a strict, deterministic routing pipeline that prevents interaction conflicts between:

* column resize
* sort affordance
* separators
* inert header regions
* List body selection

This SPEC does **not** define geometry.
Hit geometry is defined by SPEC-HAL-118L.

This SPEC defines **event flow, capture, propagation, and cancellation**.

---

## 2. Scope

Applies exclusively to:

* ListView HeaderBand
* Pointer-based interactions (mouse, touch, pen)

Explicitly excludes:

* List body interactions
* Keyboard navigation
* Scrolling logic
* Accessibility semantics

---

## 3. Header Interaction Ownership

The HeaderBand is owned by **ListView**, not by List.

ListView is the sole authority for:

* header pointer routing
* capture decisions
* suppression of body interactions

List MUST NOT receive header-originated pointer events.

---

## 4. Event Routing Pipeline (MANDATORY)

All pointer events targeting the HeaderBand MUST follow this pipeline:

1. Hit-test against header hit-zones (SPEC-HAL-118L)
2. Resolve interaction mode
3. Optionally acquire pointer capture
4. Dispatch interaction intent
5. Suppress propagation to List body

No shortcuts. No fallthrough.

---

## 5. Interaction Mode Resolution

Exactly **one** interaction mode may be active per pointer sequence.

Resolution order is fixed and MUST match SPEC-HAL-118L priority:

1. Resize
2. Separator (alias of resize)
3. Sort
4. Inert header background

Once resolved, the mode is locked until termination.

---

## 6. Pointer Down Handling

### 6.1 Resize Mode

If PointerDown occurs in a resize gutter:

* Resize mode MUST activate immediately
* Pointer MUST be captured by ListView
* No other header or body interaction may start

---

### 6.2 Sort Mode

If PointerDown occurs in a sort hit-zone:

* Sort intent is armed
* No pointer capture is taken
* Actual sort commit occurs on PointerUp inside same zone

---

### 6.3 Inert Header Mode

If PointerDown occurs in inert header area:

* Event MUST be consumed
* No state change occurs
* No propagation allowed

---

## 7. Pointer Move Handling

### 7.1 Resize Mode

* PointerMove MUST update resize delta
* Movement outside header bounds MUST NOT cancel resize
* Resize continues until explicit termination

---

### 7.2 Sort Mode

* PointerMove MAY cancel sort arming if pointer exits sort hit-zone
* No capture is taken
* Visual feedback MAY update (out of scope)

---

### 7.3 Inert Mode

* PointerMove is ignored

---

## 8. Pointer Up Handling

### 8.1 Resize Mode

Resize MUST terminate on PointerUp:

* Capture is released
* Resize commit occurs (SPEC-HAL-118F)

---

### 8.2 Sort Mode

Sort MUST commit only if:

* PointerUp occurs inside original sort hit-zone
* No cancellation occurred

Otherwise, sort is aborted.

---

### 8.3 Inert Mode

No action.

---

## 9. Cancellation Rules

Header interactions MUST cancel on:

* PointerCancel
* Escape key
* Loss of pointer capture (resize only)

Cancellation MUST:

* Reset header interaction state
* Release capture (if held)
* Produce no side effects

---

## 10. Propagation Rules (CRITICAL)

Header-originated pointer events MUST NEVER:

* reach List body
* start selection
* affect scrolling
* affect focus outside header

This is an absolute invariant.

---

## 11. Forbidden Behavior

The following are architectural violations:

* Header click selecting list rows
* Resize gesture triggering sort
* Sort triggering during active resize
* Pointer capture taken for sort
* Multiple simultaneous header modes
* Header events bubbling into List

Violations produce undefined behavior.

---

## 12. Invariants

**INV-HDR-ROUTE-1**
Header interaction routing is owned exclusively by ListView.

**INV-HDR-ROUTE-2**
Exactly one header interaction mode may be active per pointer sequence.

**INV-HDR-ROUTE-3**
Resize interactions always capture pointer.

**INV-HDR-ROUTE-4**
Header interactions never propagate to List body.

---

## 13. Relationship to Other SPECs

* SPEC-HAL-118L defines *where* interactions occur
* SPEC-HAL-118F defines *what resize means*
* SPEC-HAL-118H defines *what sorting means*
* SPEC-HAL-118N will define *where resulting state lives*

---

## 14. Rationale

Header interaction bugs are not visual bugs.
They are routing bugs.

This SPEC ensures header logic is:

* isolated
* deterministic
* immune to gesture ambiguity

No heuristics.
No emergent behavior.
No accidental coupling.

---

## 15. Summary

This SPEC freezes ListView header interaction routing as a **closed system**.

Once pointer input enters the header, it **never leaks out** until resolution.

---

## Implementation Notes (ListView v1)

Header interaction routing is implemented in `ListView::handle_event()`. Resize mode captures pointer on PointerDown in resize gutter and persists until PointerUp. Header events never propagate to List body. Multi-click arity (double-click for auto-size) is tracked via `ClickTracker`.

---

## Implementation Reference

- `ListView::handle_event()` — header event routing
- `ListView::active_resize` — resize drag state
- `ClickTracker` — multi-click detection for auto-size
- `PendingColumnAction` — action queue for resize/auto-size
