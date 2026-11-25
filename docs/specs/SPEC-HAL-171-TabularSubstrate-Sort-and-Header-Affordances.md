# SPEC-HAL-171: TabularSubstrate — Sort and Header Affordances

**Status:** Implemented / Closed
**Layer:** Interaction Affordance / Intent Emission (Non-Mutating)  
**Audience:** Engine, Interaction, State  
**Depends on:**  
SPEC-HAL-009, SPEC-HAL-037, SPEC-HAL-079, SPEC-HAL-140, SPEC-HAL-144, SPEC-HAL-158  
**Parent:** SPEC-HAL-164  
**Consumes:** SPEC-HAL-165, SPEC-HAL-166
**Extended by:** SPEC-HAL-353 for provider-authoritative multi-column sorting in ListView and other SPEC-HAL-188-backed consumers

---

## 1. Purpose

This SPEC defines **sorting affordances exposed through the Header Domain** and the **intent-only contract** for
requesting sort changes.

It exists to prevent:

- headers mutating data
- geometry-driven sorting
- implicit sort state
- coupling sort logic to layout or rendering

Sorting is a **semantic request**, not a UI action.

---

## 2. Affordance Definition

A **Sort Affordance** is a header-local interaction that emits a sort intent for a specific column.

Affordances may include:

- click / tap
- keyboard activation
- context menu action

Affordances are optional per column.

---

## 3. Sort Targeting

**INVARIANT-1:**  
Every sort intent must reference exactly one `column_id`.

Derived or inferred targets are forbidden.

Under SPEC-HAL-353, this remains binding for a column-header activation. Host/programmatic custom-key operations,
clear-to-baseline, cancellation, and restored `SortSpec` requests are not column-header intents and use their explicit
`SortKey`, `BaselineOrderId`, or `SortRequestId` target instead.

**INVARIANT-2:**  
Hidden columns must not emit sort intents.

---

## 4. Sort Intent Model

Sort is expressed via **intent directives** only:

- `RequestSort(column_id, direction | toggle)`
- `RequestClearSort(column_id)` (optional)

No mutation occurs in the Header Domain.

Upstream resolution decides:

- final direction
- multi-column behavior
- data reordering
- persistence

SPEC-HAL-353 extends this directive set with explicit multi-column, baseline, cancellation, custom-key, and restored-spec
operations at the DataAdapter request boundary. Header widgets remain intent-only.

---

## 5. Interaction Routing

Sort affordances are handled exclusively in the Header Domain.

**INVARIANT-3:**  
Body interactions must not emit sort intents.

**INVARIANT-4:**  
Sort intents must not be emitted during resize capture.

---

## 6. Visual State Projection

Visual indicators (icons, arrows, emphasis) are projections of **external sort state**.

Allowed:

- asc/desc indicators
- priority badges
- inactive hints

Forbidden:

- storing sort state in header widgets
- computing sort order locally
- animating layout based on sort state

---

## 7. Keyboard and Accessibility

Header sort affordances must be addressable via:

- focus navigation
- activation commands

Accessibility roles are projected; no semantic authority is owned here.

---

## 8. Multi-Sort Awareness

The Header Domain may display multi-sort state but must not manage it.

**INVARIANT-5:**  
Multi-sort resolution is upstream-only.

Header code must remain stateless regarding ordering rules.

---

## 9. Forbidden Behaviors

Explicitly illegal:

- sorting data inside header handlers
- reordering rows in response to clicks
- binding sort direction to geometry
- emitting sort intents from body cells
- implicit default sorting

---

## 10. Edit Interaction

Comparator changes are **configuration**, not intents.
They do not flow through `handle_intent` or `emit_intent`.

This comparator-specific section applies only to legacy/non-SPEC-HAL-188 TabularSubstrate fulfillment. SPEC-HAL-353
supersedes comparator fulfillment for ListView and defines the active-edit boundary for provider-authoritative sorting.

During an active edit session:

- Sort order is frozen. No sort dirtiness or version change may occur.
- Comparator changes are deferred until the edit session ends.
- No path may apply a new comparator while `is_editing() == true`.

**INVARIANT-6:**
Comparator application must not occur during active edit.

See **SPEC-HAL-174A** (Deferred Comparator Application During Edit) for binding mechanics.

---

## 11. Scope Boundaries

This SPEC does **not** define:

- data model
- persistence
- multi-column priority rules
- server-side sorting
- animations

Those are layered elsewhere.

---

## 12. Relationship to Deprecated SPECs

This SPEC replaces sort-affordance intent from:

- SPEC-HAL-118H

No behavior is inherited.

---

## 13. Next SPEC

SPEC-HAL-172 will define:

**Tree Projection over TabularSubstrate**

---

## 14. Closure

**This SPEC is fully implemented and closed.**

All invariants (INVARIANT-1 through INVARIANT-6) are enforced in code with hard guards.
The sort → realization → virtualization pipeline is identity-stable and tested.
No further work remains under this SPEC.

Future changes to sort semantics, sort intent routing, or sort-related identity
guarantees require a new SPEC.

SPEC-HAL-353 is that new SPEC for ListView and other SPEC-HAL-188-backed consumers. The intent-only header invariants in
this closed SPEC remain binding; its comparator-specific fulfillment does not apply to those consumers.
