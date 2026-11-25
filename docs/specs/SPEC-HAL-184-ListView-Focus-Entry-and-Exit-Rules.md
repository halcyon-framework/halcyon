# SPEC-HAL-184: ListView Focus Entry and Exit Rules

**Status:** Draft (Binding upon adoption)
**Layer:** Interaction / Focus Law
**Scope:** ListView, TabularView-backed row-centric views, TreeView projection
**Depends on:** SPEC-HAL-179, SPEC-HAL-175, SPEC-HAL-038, SPEC-HAL-181, SPEC-HAL-183
**Non-Goals:** Styling, visual focus indicators, accessibility policy, platform IME specifics

---

## 1. Purpose

This SPEC defines exactly how focus enters, moves within, and exits a ListView.

Focus bugs are systemic, not cosmetic.
Without explicit entry/exit law, ListView implementations drift into:

* header stealing focus,
* focus jumping on scroll,
* focus loss during virtualization,
* row vs cell ambiguity,
* keyboard navigation inconsistencies.

This SPEC eliminates those failure modes.

---

## 2. Definitions

* **Focusable widget** — a widget eligible to receive focus per SPEC-HAL-038.
* **Cell-focusable widget** — a focusable widget bound to (RowId, ColumnId) via SPEC-HAL-181.
* **Row focus** — a conceptual state; rows themselves never receive focus.
* **Focus anchor** — the (RowId, ColumnId, WidgetId) triple representing current focus target.

---

## 3. Fundamental Law

**INV-FOCUS-LV-1 — Rows and Cells Never Own Focus**

* Rows MUST NOT receive focus.
* Cells MUST NOT receive focus.
* Only widgets may receive focus.

Rows and cells participate only as routing and navigation context.

This rule is absolute.

---

## 4. Focus Entry Rules

**INV-FOCUS-ENTRY-1 — Entry Requires Explicit Interaction**

Focus may enter a ListView only via:

* pointer interaction on a focusable widget inside a cell
* keyboard navigation from an adjacent focus domain (per SPEC-HAL-038)
* explicit programmatic focus request targeting a widget inside the ListView

Scroll, hover, selection change, or realization MUST NOT cause focus entry.

---

**INV-FOCUS-ENTRY-2 — Header Is Not a Focus Entry Point**

* Column headers MUST NOT receive focus by default.
* Header interaction (resize, reorder, sort) does not grant focus.
* If header widgets are made focusable explicitly, they form a separate focus domain and MUST NOT redirect focus into the body implicitly.

This prevents header/body focus leakage.

---

**INV-FOCUS-ENTRY-3 — Default Entry Target Resolution**

When focus enters via keyboard navigation and no prior focus anchor exists:

1. Resolve the first visible row (by row order, not index cache).
2. Resolve the first focusable column in column order.
3. Focus the bound widget at (RowId, ColumnId).

If no focusable widget exists in the ListView:

* focus entry fails cleanly
* focus remains in the previous domain

---

## 5. Focus Persistence and Stability

**INV-FOCUS-STAB-1 — Identity-Based Focus Anchoring**

Focus MUST be anchored by identity:

```
(RowId, ColumnId, WidgetId)
```

Focus MUST NOT be anchored by:

* row index
* column index
* pixel position
* realization slot

This guarantees stability under reorder, resize, and virtualization.

---

**INV-FOCUS-STAB-2 — Virtualization Transparency**

If the focused widget is virtualized out of view:

* focus anchor is preserved
* no focus exit event is emitted
* when the row is realized again, focus is restored

Virtualization MUST NOT destroy focus.

---

## 6. Focus Movement Within ListView

**INV-FOCUS-NAV-1 — Keyboard Navigation Is Cell-Relative**

Keyboard navigation within ListView follows SPEC-HAL-175, with these constraints:

* Horizontal navigation moves across columns within the same row.
* Vertical navigation moves across rows, preserving column when possible.
* Navigation skips non-focusable cells.

Cells without focusable bindings are skipped, not selected.

---

**INV-FOCUS-NAV-2 — Scroll Does Not Move Focus**

Scrolling the ListView:

* does not change focus anchor
* does not emit focus events
* does not reassign focus

Focus movement and scroll movement are orthogonal.

---

## 7. Interaction with Editing

**INV-FOCUS-EDIT-1 — Edit Mode Owns Focus**

When inline editing is active (SPEC-HAL-174):

* focus is owned by the editor widget
* navigation that would leave the editor is gated by edit commit/cancel rules
* focus MUST NOT jump to another cell implicitly

Exiting edit mode restores focus to the pre-edit focus anchor unless explicitly redirected.

---

## 8. Focus Exit Rules

**INV-FOCUS-EXIT-1 — Explicit Exit Only**

Focus may exit a ListView only via:

* keyboard navigation out of bounds (e.g. Tab / Shift+Tab)
* explicit pointer focus on an external widget
* programmatic focus reassignment

Scroll, reorder, resize, selection change, or data update MUST NOT cause focus exit.

---

**INV-FOCUS-EXIT-2 — Structural Mutations Do Not Evict Focus**

The following MUST NOT evict focus:

* column reorder
* column resize
* row reorder
* sorting
* data refresh
* virtualization

If the focused widget is destroyed due to row removal:

* focus exits to the nearest lawful external domain per SPEC-HAL-038

---

## 9. Forbidden Patterns (Hard Law)

The following are architecturally illegal:

* focusing rows or cells
* focus following selection
* focus reassignment on scroll
* header-induced body focus
* index-based focus tracking
* focus loss due to virtualization
* implicit focus entry on hover

Any of these is an SPEC violation.

---

## 10. Acceptance Criteria

SPEC-HAL-184 is satisfied when:

* focus is always widget-owned
* focus survives scroll, reorder, resize, and virtualization
* entry and exit paths are explicit and deterministic
* header interaction never steals focus
* keyboard navigation is stable and identity-based

---

## 11. Strategic Note (Non-Normative)

Selection answers "what is chosen".
Focus answers "who receives input".

Conflating them is the fastest way to destroy ListView.
