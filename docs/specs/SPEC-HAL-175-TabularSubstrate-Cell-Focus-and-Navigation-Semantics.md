# SPEC-HAL-175: TabularSubstrate — Cell Focus and Navigation Semantics

**Status:** Implemented / Closed
**Layer:** Focus Projection / Navigation Semantics
**Audience:** Engine, Interaction, Accessibility
**Depends on:**
SPEC-HAL-009, SPEC-HAL-037, SPEC-HAL-038, SPEC-HAL-079, SPEC-HAL-140, SPEC-HAL-158
**Parent:** SPEC-HAL-164
**Consumes:** SPEC-HAL-167, SPEC-HAL-173, SPEC-HAL-174

**Clarified by:** SPEC-HAL-347. `OrderedSelectionState::focus_key` is a row-axis navigation cursor; it is distinct from the cell `FocusState` defined here and from framework widget focus.
**Extended by:** SPEC-HAL-353 for navigation over provider-confirmed revision-scoped order windows in
SPEC-HAL-188-backed consumers

---

## 1. Binding Invariants

**INV-175-1:** Focus is a single cell: `Option<CellId>`. No multi-focus.

**INV-175-2:** Focus changes are versioned strictly monotonic (`u64`). Exactly +1 per accepted focus change.

**INV-175-3:** Navigation is a pure function of (current focus, direction, effective row order, visible columns). No hidden inputs.

**INV-175-4:** Navigation respects visibility: hidden columns and non-visible (tree-collapsed) rows are not navigable targets.

**INV-175-5:** Navigation is virtualization-safe: focus is logical (RowId+ColumnId), not tied to physical viewport realization. Navigation targets any row in the effective row order regardless of current realization state. Viewport realization catches up on next update cycle.

**INV-175-6:** Tree projection is honored: when active, navigation operates over `TreeProjection.visible_indices()` as the row set.

**INV-175-7:** Sort is honored. Legacy/materialized substrate paths may use `effective_row_order()` and
`compute_sorted_order()`. A SPEC-HAL-188-backed consumer instead navigates the provider-confirmed order revision through
`LocateRow` and bounded RowId windows; it MUST NOT apply a local sort.

**INV-175-8:** While editing (SPEC-HAL-174), any focus/navigation intent triggers `cancel_edit_for_destabilization()` first, then proceeds normally.

**INV-175-9:** No RowModel identity mutation. No ColumnModel mutation. No geometry table mutation. `resolve_navigation()` takes `&mut self` solely for tree dirty-flag housekeeping in `effective_row_order()`; no structural mutation occurs.

---

## 2. Purpose

This SPEC defines **focus projection and navigation semantics within TabularSubstrate**.

It exists to prevent:

- focus ownership leaks
- cell widgets hijacking global focus
- navigation coupled to widget trees
- virtualization-unsafe focus memory

Focus is **projected**, not owned.

---

## 3. FocusState

```rust
pub struct FocusState {
    focused: Option<CellId>,
    anchor: Option<CellId>,
    version: u64,
}
```

- `focused`: The single focused cell, or None.
- `anchor`: Reserved for range selection (future SPEC).
- `version`: Monotonic counter, incremented exactly once per accepted focus change.

---

## 4. FocusIntent

```rust
pub enum FocusIntent {
    RequestFocus { row_id: RowId, column_id: ColumnId },
    RequestNavigate { from: Option<(RowId, ColumnId)>, direction: NavigationDirection },
    RequestClearFocus,
}
```

FocusState is **only** mutated in `handle_focus_intent()`. No other code path may write to it.

---

## 5. NavigationDirection Behavior

| Direction | Behavior |
|-----------|----------|
| Up | Previous row in effective order. Clamps at first row. |
| Down | Next row in effective order. Clamps at last row. |
| Left | Previous visible column. Clamps at first column. |
| Right | Next visible column. Clamps at last column. |
| Home | First visible column in current row. |
| End | Last visible column in current row. |
| PageUp | Up by `page_size()` rows. Clamps at first row. |
| PageDown | Down by `page_size()` rows. Clamps at last row. |

`page_size()` = `floor(viewport_height / row_height)`, minimum 1. Default 10 when viewport not set.

---

## 6. Boundary Behavior

- **Top row + Up:** No movement. Focus unchanged. Version still increments (resolve returns Some).
- **Bottom row + Down:** No movement. Focus unchanged. Version still increments.
- **Leftmost column + Left:** No movement.
- **Rightmost column + Right:** No movement.
- **Home at first column:** No-op (already there).
- **End at last column:** No-op (already there).
- **PageUp past top:** Clamps to row 0.
- **PageDown past bottom:** Clamps to last row.

---

## 7. Focus When None

When `focused` is `None` and a `RequestNavigate` arrives:

- Navigation treats current position as `(0, 0)` in effective ordering.
- Direction is applied from that position.
- Result: Down from None → row 1, col 0. Right from None → row 0, col 1. Etc.
- If effective ordering is empty (no rows or no visible columns), navigation returns None (no focus change, no version increment).

---

## 8. Focused Cell Becomes Invalid

Focus is **identity-sticky** (RowId+ColumnId), not index-based.

| Invalidation | Behavior |
|--------------|----------|
| Sort reorder | Focus retains same RowId+ColumnId. Navigation finds its new position in sorted order. |
| Tree collapse (focused row hidden) | Focus retains stale CellId. Next navigation treats position as fallback (0,0). |
| Column hide (focused column hidden) | Focus retains stale CellId. Next navigation treats column position as fallback (0). |
| Scroll (virtualization) | Focus retains CellId unchanged. Logical focus is independent of viewport realization. |

Focus is never implicitly cleared by sort/tree/column/scroll changes.
It is only cleared explicitly via `RequestClearFocus` or by `cancel_edit_for_destabilization()` restoring pre-edit focus.

---

## 9. Focus and Inline Editing (SPEC-HAL-174)

INV-175-8: `handle_intent()` calls `cancel_edit_for_destabilization()` before dispatching to `handle_focus_intent()` when `is_editing()` is true.

This means:
- Navigation during edit → edit cancelled → pre-edit focus restored → navigation proceeds from restored state.
- `RequestFocus` during edit → edit cancelled → focus set to requested cell.
- `RequestClearFocus` during edit → edit cancelled → focus cleared.

Navigation must not implicitly commit edits. Commit/cancel is explicit via SPEC-HAL-174 intents.

---

## 10. Effective Row Order

For legacy/materialized substrate paths, `effective_row_order()` computes the navigation row sequence:

1. Tree active → `visible_indices()` (collapsed rows excluded)
2. Sort active on visible set → apply `compute_sorted_order()` or `sort_visible_indices()`
3. Neither → natural order `0..row_count`

Navigation never operates on raw model indices when tree or sort is active.

For a SPEC-HAL-188-backed consumer, the effective order oracle is the provider's confirmed `OrderRevision`:

1. Resolve the current `RowId` through `LocateRow` when its ordinal is not already present in a current bounded window.
2. Request only the neighboring/page window required by the navigation operation.
3. If lookup/window testimony is pending, do not accept the navigation intent or change focus/version; retry is lawful
   after provider invalidation.
4. Never construct or locally sort a complete effective-order vector.

---

## 11. Forbidden Behaviors

Explicitly illegal:

- storing focus state inside cell widgets
- index-based focus tracking (focus is by RowId+ColumnId identity)
- focus-driven layout invalidation
- preventing virtualization to preserve focus
- global shortcut registration by cell content
- multi-cell focus
- implicit focus clear on sort/tree/scroll

---

## 12. Scope Boundaries

This SPEC does **not** define:

- selection semantics
- clipboard behavior
- text navigation inside editors
- global focus traversal
- accessibility projection

Those are layered elsewhere.

---

## 13. Closure

**This SPEC is fully implemented and closed.**

Implementation: `src/widgets/tabular_view/mod.rs` (FocusState, handle_focus_intent, resolve_navigation, effective_row_order, page_size).

Enforcement:

- **INV-175-1:** `FocusState.focused: Option<CellId>` — single cell, no Vec, no multi-focus API.
- **INV-175-2:** `version` incremented exactly once per accepted change in `handle_focus_intent`. No other code path increments it (except `cancel_edit_for_destabilization` which restores pre-edit focus).
- **INV-175-3:** `resolve_navigation(&self, direction) -> Option<CellId>` — pure function of `self.focus_state.focused`, `direction`, `effective_row_order()`, `visible_columns()`. No external state read.
- **INV-175-4:** `visible_columns()` excludes hidden columns. `effective_row_order()` uses `visible_indices()` when tree active.
- **INV-175-5:** `resolve_navigation` operates on full logical row model (or tree-visible subset). Does not check viewport realization. Focus is logical.
- **INV-175-6:** `effective_row_order()` → `tree_projection.visible_indices()` when `is_active()`.
- **INV-175-7:** Legacy/materialized `effective_row_order()` may use `compute_sorted_order()` or
  `sort_visible_indices()`. SPEC-HAL-188-backed consumers use provider-confirmed revision/window order under
  SPEC-HAL-353 and perform no local sort.
- **INV-175-8:** `handle_intent` for `Focus` variant: `if self.is_editing() { self.cancel_edit_for_destabilization(); }` before dispatch.
- **INV-175-9:** `resolve_navigation` takes `&mut self` for tree dirty-flag housekeeping via `effective_row_order()`. No RowModel/ColumnModel/geometry writes occur — the `&mut` is limited to `TreeProjection::compute_visible_indices()` refresh.

Integration tests in `focus_navigation.rs` (32 tests covering all invariants).

Future changes to focus semantics, multi-focus, selection, or navigation wrapping require a new SPEC.
