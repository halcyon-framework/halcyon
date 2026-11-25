# SPEC-HAL-198: Column Selection Model

**Status:** Draft (Binding upon adoption)
**Layer:** TabularSubstrate Column Model / Interaction Semantics
**Owner:** TabularSubstrate (state), ListView / TreeView / GridView (presentation)
**Audience:** Engine, Widget, Interaction
**Scope:** All tabular consumers (ListView, TreeView projection, Grid-like views)
**Depends On:** SPEC-HAL-195 (Authority), SPEC-HAL-196 (Column Visibility), SPEC-HAL-197 (Pinning)
**Non-Goals:** Row selection, cell selection, spreadsheet semantics, column groups

---

## 0. Purpose

Define a first-class, substrate-level column selection model that is:

- shared across all tabular widgets
- independent of view type
- compatible with pinning, visibility, sorting, and resizing
- interaction-driven but not interaction-owned

This SPEC defines what column selection is, where it lives, and what it must never do.

---

## 1. Non-Goals

This SPEC does not define:

- how selection is visually rendered (colors, borders, highlights)
- keyboard shortcut bindings
- context menus or commands
- column grouping or spanning
- row or cell selection

Those belong to separate SPECs.

---

## 2. Conceptual Model

Column selection is a set of ColumnIds maintained by the column model.

```rust
type ColumnSelection = Set<ColumnId>;
```

Selection is orthogonal to:

- visibility
- pinning
- sorting
- resize state

A column may be:

- selected but hidden
- selected and pinned
- selected and sorted
- selected and being resized

---

## 3. Authority and Ownership

### INV-COLSEL-AUTH-1 — Substrate Ownership

Column selection state is owned by the column model (TabularSubstrate authority).

### INV-COLSEL-AUTH-2 — Views Do Not Own Selection

Views MAY interpret user input and request selection changes, but MUST NOT own selection state.

### INV-COLSEL-AUTH-3 — Single Selection State

There is exactly one column selection state per substrate instance.

---

## 4. Selection Modes

The model supports two modes:

### 4.1 Single Selection

- At most one `ColumnId` selected.
- Selecting a new column clears the previous selection.

### 4.2 Multi Selection

- Arbitrary number of columns selected.
- Additive and subtractive operations supported.

### INV-COLSEL-MODE-1 — Explicit Mode

The active selection mode MUST be explicit and queryable.

---

## 5. Core Operations

All operations are expressed in terms of `ColumnId`.

### Required Operations

- `select(ColumnId)`
- `deselect(ColumnId)`
- `toggle(ColumnId)`
- `clear()`
- `set(Vec<ColumnId>)`

### Range Selection (Optional but Supported)

- `select_range(from: ColumnId, to: ColumnId)`

Range semantics operate on logical column order, not visual X position.

---

## 6. Ordering Semantics

### INV-COLSEL-ORDER-1 — Logical Order

Column selection ordering is logical, not visual.

Effects:

- Pinning does NOT change range semantics.
- Visibility does NOT change range semantics.
- Sorting does NOT change range semantics.

If a range includes hidden columns, they are included in selection.

---

## 7. Visibility Interaction

### INV-COLSEL-VIS-1 — Hide Preserves Selection

Hiding a selected column does NOT clear its selection.

### INV-COLSEL-VIS-2 — Unhide Restores Selection

Unhiding a column restores its selected state if it was selected before.

### INV-COLSEL-VIS-3 — Operations Valid on Hidden Columns

Selection operations MUST be valid on hidden columns.

Rationale: visibility is a presentation concern; selection is structural intent.

---

## 8. Pinning Interaction

### INV-COLSEL-PIN-1 — Pin Preserves Selection

Pinning or unpinning a column MUST NOT affect its selection state.

### INV-COLSEL-PIN-2 — Range Ignores Pin Domains

Selection ranges ignore pinning domains and operate on the full logical order.

---

## 9. Sorting Interaction

### INV-COLSEL-SORT-1 — Sort Preserves Selection

Sorting MUST NOT alter selection.

### INV-COLSEL-SORT-2 — Range Before Sort

Selection ranges are resolved before sort application.

---

## 10. Resize Interaction

### INV-COLSEL-RESIZE-1 — No Implicit Selection

Resizing a column MUST NOT implicitly select it.

### INV-COLSEL-RESIZE-2 — Coexistence

Selection MAY coexist with resize hover/active states.

---

## 11. Event Semantics

Column selection is intent-level, not raw-event-level.

Views translate input into semantic requests such as:

- `RequestSelectColumn(ColumnId)`
- `RequestToggleColumn(ColumnId)`
- `RequestSelectRange(ColumnId, ColumnId)`

The substrate validates and applies the change.

### INV-COLSEL-EVENT-1 — No Direct Mutation

Direct mutation of selection state from pointer events is forbidden.

---

## 12. Visual Semantics (Constraints Only)

This SPEC does not define visuals, but constrains them:

### INV-COLSEL-VISUAL-1 — Distinguishable

Selected columns MUST be visually distinguishable from non-selected ones.

### INV-COLSEL-VISUAL-2 — No Geometry Alteration

Selection visuals MUST NOT alter geometry.

### INV-COLSEL-VISUAL-3 — Consistent Across Regions

Selection visuals MUST be consistent across header, body, and footer.

---

## 13. Required Queries

Consumers must be able to query:

- `is_selected(ColumnId) -> bool`
- `selected_columns() -> &[ColumnId]`
- `selection_mode() -> SelectionMode`

---

## 14. Persistence

### INV-COLSEL-PERSIST-1 — Serializable

Selection state is serializable independently of view.

### INV-COLSEL-PERSIST-2 — Restore Order

Restoring selection MUST occur after column identity restoration, before layout.

---

## 15. Failure Modes Prevented

This SPEC explicitly prevents:

- view-local selection drift
- selection loss on hide/unhide
- pinning-dependent selection bugs
- accidental selection during resize
- inconsistent selection across sibling views

---

## 16. Tests and Verification

Minimum verification:

1. Select a column: selection visible in header/body/footer.
2. Hide selected column: selection preserved.
3. Unhide: selection restored.
4. Pin selected column: selection preserved.
5. Range select across pinned/unpinned columns.
6. Sort columns: selection unchanged.
7. Resize selected column: no selection change.

---

## 17. Explicitly Forbidden

- Per-view column selection state
- Implicit selection via resize or hover
- Geometry changes caused by selection
- Selection bound to pixel position
- Clearing selection on visibility or pinning changes

---

## 18. Notes for Future SPECs

Potential extensions:

- Keyboard navigation across selected columns
- Selection-driven batch operations (hide, pin, sort)
- Visual affordances for multi-selection ranges

Any extension must preserve all INV-COLSEL-* invariants unless explicitly superseded.
