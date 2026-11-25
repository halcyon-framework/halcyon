# SPEC-HAL-168: TabularSubstrate — Horizontal Geometry Resolution

**Status:** Implemented / Closed
**Layer:** Layout Authority / Geometry Resolution  
**Audience:** Engine, Layout, Scroll, Virtualization  
**Depends on:**  
SPEC-HAL-008, SPEC-HAL-024, SPEC-HAL-086, SPEC-HAL-131A/B/C, SPEC-HAL-141, SPEC-HAL-145, SPEC-HAL-146, SPEC-HAL-158  
**Parent:** SPEC-HAL-164  
**Consumes:** SPEC-HAL-165, SPEC-HAL-166, SPEC-HAL-167

---

## 1. Purpose

This SPEC defines how **horizontal geometry** for TabularSubstrate is resolved, owned, and exposed.

It exists to eliminate:

- implicit width inference
- per-domain horizontal drift
- header/body desynchronization
- scroll-driven layout mutation

Horizontal geometry is **resolved once**, then consumed everywhere.

---

## 2. Resolution Authority

**INVARIANT-1:**  
Horizontal geometry resolution is owned exclusively by the **TabularSubstrate layout phase**.

**INVARIANT-2:**  
No domain (Header, Body, Footer) may resolve or adjust horizontal geometry.

**INVARIANT-3:**  
Scroll state must not influence geometry resolution.

---

## 3. Resolution Inputs

Resolution inputs are:

- available horizontal extent (viewport width)
- column model constraints (SPEC-HAL-165)
- column visibility mask

No content measurement may participate.

---

## 4. Resolution Outputs

Resolution produces a **Column Geometry Table** containing, per column:

- `column_id`
- `x_offset`
- `width`

The table is:

- frame-local
- immutable
- versioned
- shared read-only across domains

---

## 5. Single-Resolution Rule

**INVARIANT-4:**  
Horizontal geometry is resolved **exactly once per layout pass**.

**INVARIANT-5:**  
Any attempt to re-resolve geometry within the same frame is illegal.

**INVARIANT-6:**  
Any attempt to read geometry before resolution is illegal.

Violations trigger invariant failure.

---

## 6. Overflow and Clipping

If total resolved width exceeds viewport width:

- overflow is clipped
- no redistribution occurs
- scroll offset applies only to rendering and hit-testing

Geometry remains unchanged.

---

## 7. Horizontal Scroll Coupling

Horizontal scroll operates as a **pure transform**.

**INVARIANT-7:**  
Scroll offset applies after geometry resolution.

**INVARIANT-8:**  
Scroll offset must not modify column widths or offsets.

Scroll state is consumed uniformly by:

- Header Domain
- Body Domain
- Footer Domain (if present)

---

## 8. Visibility Transitions

Toggling column visibility:

- invalidates layout
- triggers full horizontal re-resolution
- preserves column identity

Partial reflow is forbidden.

---

## 9. Rendering and Hit-Testing

Rendering and hit-testing must use the same geometry table.

Forbidden:

- per-domain coordinate adjustment
- geometry caching outside layout phase
- compensating offsets in paint code

---

## 10. Forbidden Behaviors

Explicitly illegal:

- measuring text to decide column width
- resizing columns during scroll
- scroll-driven layout invalidation
- domain-local horizontal transforms
- conditional geometry based on row count

---

## 11. Scope Boundaries

This SPEC does **not** define:

- resize interaction semantics
- column pinning
- snap behavior
- footer geometry

Those are layered later.

---

## 12. Relationship to Deprecated SPECs

This SPEC replaces horizontal geometry intent from:

- SPEC-HAL-118D
- SPEC-HAL-118E

No behavior is inherited.

---

## 13. Next SPEC

SPEC-HAL-169 will define:

**Column Resize Interaction Contract**

---

## 14. Closure

**This SPEC is fully implemented and closed.** Implementation: `src/widgets/tabular_view/geometry.rs` (ColumnGeometryTable, ColumnSlot, GeometryView). All invariants (INVARIANT-1 through INVARIANT-8) enforced by single-resolution architecture and epoch-guarded read access. No further work permitted under this SPEC.
