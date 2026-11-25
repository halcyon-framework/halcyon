# SPEC-HAL-172: Tree Projection over TabularSubstrate

**Status:** Implemented / Closed
**Layer:** Structural Projection / Indentation Semantics  
**Audience:** Engine, Layout, Virtualization  
**Depends on:**  
SPEC-HAL-008, SPEC-HAL-024, SPEC-HAL-037, SPEC-HAL-131A/B/C, SPEC-HAL-141, SPEC-HAL-145, SPEC-HAL-146, SPEC-HAL-158  
**Parent:** SPEC-HAL-164  
**Consumes:** SPEC-HAL-165, SPEC-HAL-167, SPEC-HAL-168
**Extended by:** SPEC-HAL-353 for provider-authoritative order in SPEC-HAL-188-backed consumers; tree sibling-order
semantics remain a separate required authority

---

## 1. Purpose

This SPEC defines how **tree semantics are projected over TabularSubstrate** without modifying its core abstractions.

It exists to ensure:

- TreeView is a projection, not a substrate
- columns remain authoritative
- indentation does not affect geometry resolution
- expansion state does not leak into layout authority

TabularSubstrate remains unchanged.

---

## 2. Projection Principle

**INVARIANT-1:**  
Tree behavior is a **row-level projection** layered on top of TabularSubstrate.

**INVARIANT-2:**  
No tree logic may mutate:

- column model
- column geometry
- header behavior
- horizontal resolution rules

---

## 3. Tree Row Model

Each row may optionally carry tree metadata:

- `node_id` (stable)
- `parent_id` (optional)
- `depth` (non-negative integer)
- `expanded` (boolean)

Tree metadata is **external state**, not owned by TabularSubstrate.

---

## 4. Indentation Semantics

Indentation is applied as a **cell-local horizontal offset** within a designated column (typically the first).

Rules:

- indentation is purely visual
- indentation does not affect column width
- indentation must not alter hit-testing bounds outside the cell

**INVARIANT-3:**  
Indentation must not modify column geometry or offsets.

---

## 5. Expansion and Collapse

Expansion state affects **row realization only**.

**INVARIANT-4:**  
Expanded/collapsed state may change which rows are realized, but not how geometry is resolved.

Virtualization integrates via existing row realization rules.

---

## 6. Hit-Testing and Interaction

Tree affordances (expand/collapse toggles):

- are cell-local
- emit intent directives only
- must not mutate realization directly

Body Domain routes interaction as usual.

---

## 7. Rendering Rules

Tree visuals may include:

- disclosure triangles
- guide lines
- branch connectors

All are non-authoritative and derived from row metadata.

---

## 8. Forbidden Behaviors

Explicitly illegal:

- treating tree nodes as columns
- resolving indentation during layout
- header-level tree logic
- column width adjustment based on depth
- storing tree state in TabularSubstrate

---

## 9. Scope Boundaries

This SPEC does **not** define:

- drag-and-drop reparenting
- animated expand/collapse
- keyboard tree navigation
- selection semantics

Those are layered separately.

---

## 10. Relationship to Deprecated SPECs

This SPEC replaces tree-over-list intent from:

- SPEC-HAL-116
- SPEC-HAL-118
- SPEC-HAL-119–121 (structural aspects only)

No substrate assumptions are inherited.

---

## 11. Completion

This SPEC completes the **TabularSubstrate base series (164–172)**.

Further extensions must not modify SPEC-HAL-164 invariants.

---

## 12. Closure

**This SPEC is fully implemented and closed.**

Implementation: `src/widgets/tabular_view/tree.rs`, `tree_paint.rs`, body integration in `body.rs`.

Enforcement:

- **INVARIANT-TREE-1:** `visible_indices` is a strict subset of `[0..nodes.len()]` — no reordering.
- **INVARIANT-TREE-2/3:** Indentation modifies cell content rects only; column geometry table untouched.
- **INVARIANT-TREE-4:** Expand/collapse mark dirty flag; RowModel identity never mutated.
- **INVARIANT-TREE-5:** `set_metadata()` replaces node array wholesale; no in-place row reordering.
- **INVARIANT-TREE-GUIDE-1 through 6:** Guide masks and stems computed in O(n) three-pass algorithm from node metadata only.
- **Legacy/materialized sort composition:** `compute_sorted_indices()` consumes `visible_indices` as base set and applies
  sort within that subset only outside SPEC-HAL-353. A SPEC-HAL-188-backed tree must not use local comparator sorting;
  SPEC-HAL-353 does not define parent/child or sibling sort semantics, so those require a separate governing SPEC before
  data-backed TreeView sorting is implemented.
- **Hit-testing:** Tree control zones use indented cell rects; no new event routing.

45 integration tests across `tree_projection.rs`, `tree_intents.rs`, `sort_tree_composition.rs`.

Future changes to tree semantics, parent-child ordering, or tree-driven layout require a new SPEC.
