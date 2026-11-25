# SPEC-HAL-165: TabularSubstrate — Column Model and Geometry Resolution

**Status:** Implemented / Closed
**Layer:** Structural Geometry / Layout Authority  
**Audience:** Engine, Layout, Virtualization, Widget Authors  
**Depends on:**  
SPEC-HAL-008, SPEC-HAL-024, SPEC-HAL-141, SPEC-HAL-145, SPEC-HAL-146, SPEC-HAL-158  
**Parent:** SPEC-HAL-164

---

## 1. Purpose

This SPEC defines the **Column Model** for TabularSubstrate and establishes the **single authoritative mechanism** for column
geometry resolution.

It exists to permanently prevent the previous failure mode where:

* columns were implicit
* widths were inferred downstream
* headers and rows disagreed on geometry
* resize logic leaked across layers

---

## 2. Column Model Definition

A **Column** is a first-class structural unit with immutable identity.

Each column has:

* `column_id` (stable, opaque)
* `min_width`
* `max_width`
* `preferred_width`
* `flex` (optional)
* `visibility` (boolean)

Columns are **ordered** and **contiguous**.

No column may exist without being registered in the Column Model.

---

## 3. Column Model Ownership

**INVARIANT-1:**  
The Column Model is **owned exclusively by TabularSubstrate**.

**INVARIANT-2:**  
Projections may observe column state but must not mutate it.

**INVARIANT-3:**  
No other widget may create, destroy, or reorder columns.

Violations are fatal.

---

## 4. Geometry Resolution Phase

Column geometry is resolved during the **layout phase**, before any projection runs.

Resolution inputs:

* available horizontal extent
* column constraints
* visibility mask

Resolution outputs (per column):

* `x_offset`
* `resolved_width`

These outputs are **frame-local**, immutable, and versioned.

---

## 5. Single-Layout-Per-Frame Enforcement

**INVARIANT-4:**  
Column geometry is resolved **at most once per frame**.

**INVARIANT-5:**  
Any attempt to read column geometry before resolution is illegal.

**INVARIANT-6:**  
Any attempt to re-resolve geometry after resolution is illegal.

This is enforced by layout authority wiring.

---

## 6. Flex Resolution Rules

If flex columns exist:

1. Fixed-width columns are resolved first.
2. Remaining space is distributed by flex weights.
3. Min/max constraints are enforced strictly.
4. Overflow is clipped, not redistributed.

No projection may override these rules.

---

## 7. Visibility Semantics

Invisible columns:

* participate in model ordering
* do not consume width
* do not generate geometry
* preserve identity

Visibility toggling is a **structural mutation** and triggers layout invalidation.

---

## 8. Column Identity Guarantees

**INVARIANT-7:**  
Column identity is stable across frames, scrolling, and virtualization.

This enables:

* header ↔ cell alignment
* state externalization
* persistent user customization

Identity stability is mandatory even when columns are hidden.

---

## 9. Forbidden Behaviors

Explicitly illegal:

* computing column widths in Header or Row code
* deriving geometry from text measurement in projections
* resizing columns during paint or input dispatch
* per-row column adjustments
* treating columns as decoration

Any of the above is grounds for rejection.

---

## 10. Scope Boundaries

This SPEC does **not** define:

* resize interaction
* sorting
* pinning
* scrolling
* tree indentation

Those are layered later and must consume this model unchanged.

---

## 11. Relationship to Deprecated SPECs

This SPEC replaces all geometry-related intent from:

* SPEC-HAL-117
* SPEC-HAL-118B–118P

No logic or assumptions are inherited.

---

## 12. Next SPEC

SPEC-HAL-166 will define:

**Header / Body / Footer Domains and Their Wiring**

No interaction semantics yet.

---

## 13. Closure

**This SPEC is fully implemented and closed.** Implementation: `src/widgets/tabular_view/column.rs` (Column, ColumnId, ColumnModel). Geometry resolution via `ColumnGeometryTable` in `geometry.rs`. All invariants (INVARIANT-1 through INVARIANT-7) enforced by ownership rules and single-resolution-per-frame guards. No further work permitted under this SPEC.
