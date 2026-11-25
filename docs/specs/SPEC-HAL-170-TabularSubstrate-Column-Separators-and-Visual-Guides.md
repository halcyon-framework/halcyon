# SPEC-HAL-170: TabularSubstrate — Column Separators and Visual Guides

**Status:** Implemented / Closed
**Layer:** Structural Visualization (Non-Authoritative)  
**Audience:** Engine, Rendering, Interaction  
**Depends on:**  
SPEC-HAL-024, SPEC-HAL-141, SPEC-HAL-142, SPEC-HAL-158  
**Parent:** SPEC-HAL-164  
**Consumes:** SPEC-HAL-165, SPEC-HAL-166, SPEC-HAL-168, SPEC-HAL-169

---

## 1. Purpose

This SPEC defines **column separators and visual guides** for TabularSubstrate.

It exists to ensure separators are:

- visually consistent
- geometrically derived
- non-authoritative
- incapable of mutating layout or state

Separators are **guides**, not structure.

---

## 2. Separator Definition

A **Column Separator** is a visual guide rendered at the boundary between two adjacent visible columns.

Separators:

- have no identity
- have no state
- do not participate in layout
- do not own hit-testing authority

They are derived strictly from resolved column geometry.

---

## 3. Geometry Derivation

Separator position is derived as:

```

separator_x = column.x_offset + column.width

```

**INVARIANT-1:**  
Separator geometry must be derived from the Column Geometry Table only.

**INVARIANT-2:**  
Separators must not compute or adjust column offsets or widths.

---

## 4. Rendering Rules

Separators are rendered as part of the TabularSubstrate visual pass.

Allowed:

- lines
- dashed guides
- hover highlights
- active resize indicators

Forbidden:

- geometry mutation
- layout invalidation
- state storage

---

## 5. Interaction Relationship

Separators may be visually associated with resize hit-zones but must not own them.

**INVARIANT-3:**  
Hit-testing for resize initiation belongs to the Header Domain, not to separators.

Separators may visually respond to hover state emitted by interaction code.

---

## 6. Visibility and Styling

Separator visibility may depend on:

- column visibility
- density mode
- theme rules

Styling is:

- purely visual
- frame-local
- non-persistent

---

## 7. Scroll and Transform Behavior

Separators must respect horizontal scroll transforms.

**INVARIANT-4:**  
Scroll offset applies uniformly to:

- columns
- header cells
- separators

No compensating transforms are permitted.

---

## 8. Forbidden Behaviors

Explicitly illegal:

- storing separator geometry
- snapping resize deltas to separator visuals
- using separators as interaction sources
- conditional separator layout per row
- separator-driven column logic

---

## 9. Scope Boundaries

This SPEC does **not** define:

- resize hit-zone geometry
- snapping rules
- column pinning visuals
- footer separators

Those are layered later.

---

## 10. Relationship to Deprecated SPECs

This SPEC replaces visual separator intent from:

- SPEC-HAL-118J

No semantics are inherited.

---

## 11. Next SPEC

SPEC-HAL-171 will define:

**Sort and Header Affordances**

---

## 12. Closure

**This SPEC is fully implemented and closed.**

Implementation: `src/widgets/tabular_view/separators.rs`

Enforcement:

- **INVARIANT-1 & INVARIANT-2:** `emit_column_separators` takes `&[ResolvedColumnGeometry]` — no other geometry source is possible, no mutation path exists.
- **INVARIANT-3:** No hit-testing code in separator module. Header Domain owns resize zones (unchanged).
- **INVARIANT-4:** Separator positions are in content space (same coordinate frame as header cells and body cells). No compensating transforms.
- **Hard guard:** Paint calls `geometry_table.columns()` which panics unconditionally (all builds) if geometry is unresolved.
- **Count invariant:** `debug_assert` verifies exactly `visible_columns - 1` separators emitted.
- **No persistent state:** No fields added to TabularSubstrate. Style constants are module-local.

Future changes to separator semantics, separator interaction, or separator-driven layout require a new SPEC.
