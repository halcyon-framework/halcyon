# SPEC-HAL-262

## TreeView Widget — Surface Semantics and Parity Law

**Status:** Draft (Binding upon adoption)
**Layer:** Widget Layer (Composite over HierarchicalTabularSubstrate + Column Geometry Engine + Selection Engine)
**Depends on:** SPEC-HAL-178, 180-197, 249-259, 261
**Peer Of:** ListView
**Last synced:** 2026-07-12 owner correction

**Amended by:** SPEC-HAL-347 for ListView only. ListView exposes pre-interaction `TabularIntentPolicy` configuration; this is construction-time configuration, not runtime selection-mode switching. TreeView's surface remains unchanged.

**Also amended by:** SPEC-HAL-348. TreeView and ListView expose identical optional-header presentation configuration while retaining their owning composite structure.

---

## 1. Purpose

This SPEC defines TreeView as a first-class tabular composite widget and establishes strict parity and ownership rules relative to ListView.

TreeView is not a special case widget.
It is a hierarchical specialization of the same tabular substrate stack.

This SPEC freezes:

- Authority boundaries
- Selection integration
- Column behavior parity
- Expand/collapse surface semantics
- Rendering invariants
- Forbidden duplication

---

## 2. Architectural Position

TreeView is a composite over:

- HierarchicalTabularSubstrate (SPEC-249-259)
- Column Geometry Engine (SPEC-180-197)
- OrderedSelectionState (SPEC-178)
- Virtualization Strategy (SPEC-250, 257)
- Engine Ownership Law (SPEC-261)

TreeView does not own:

- Column geometry resolution
- Row topology
- Expand/collapse state storage
- Virtualization strategy

TreeView owns:

- Interaction orchestration (key/pointer dispatch)
- Selection state (OrderedSelectionState<u64> lives in TreeViewCore)
- Visual composition (header paint, scroll area child, footer child)
- Surface API

Note: Selection state lives in TreeViewCore (widget layer), not in HTS. HTS has zero selection methods. HTS implements `OrderedIndex<u64>` so it serves as the ordering oracle for the reducer, but selection storage is widget-owned. This is the same pattern as ListView (ListViewCore owns OrderedSelectionState, TabularSubstrate serves as oracle).

### Implementation Status

| Claim | Status |
|-------|--------|
| Composite over HierarchicalTabularSubstrate | **Implemented.** `TreeViewCore.hts: HierarchicalTabularSubstrate`. |
| Composite over Column Geometry Engine | **Implemented.** HTS wraps ColumnGeometryEngine; `resolve_geometry()`, `column_layout()`, `body_slots()`, `header_slots()` all route through HTS. |
| Composite over OrderedSelectionState | **Implemented.** `TreeViewCore.selection: OrderedSelectionState<u64>` with `SelectionMode::Multi`. TreeView dispatches actions via `reduce_ordered_selection()`. |
| Does not own column geometry resolution | **Correct.** `TreeView::layout()` calls `core.hts.resolve_geometry(w)`. No TreeView-specific geometry math. |
| Does not own row topology | **Correct.** HTS owns topology. TreeView/TreeViewBody read projections only. |
| Does not own expand/collapse state storage | **Correct.** HTS owns expansion state. |

---

## 3. Parity Law with ListView

TreeView is a structural peer of ListView.

### INV-TV-PARITY-1

TreeView MUST support:

- Column reordering (SPEC-182)
- Column visibility (SPEC-196)
- Column pinning (SPEC-197)
- Column selection (SPEC-198, where applicable)
- OrderedSelectionState (SPEC-178)
- Scroll domain separation identical to ListView
- Header behavior identical to ListView
- Optional header-band presence identical to ListView
- Optional footer renderer hook (no fixed semantics)

### INV-TV-PARITY-2

All column mechanics MUST route through the same Column Geometry Engine interfaces used by ListView.

No TreeView-specific geometry logic is allowed.

### INV-TV-PARITY-3

Selection visuals MUST match ListView rendering semantics.

TreeView does not invent a new selection model.

### Implementation Status

| Requirement | Status |
|-------------|--------|
| Column reordering (SPEC-182) | **Not implemented** in TreeView. (Also not implemented in ListView.) |
| Column visibility (SPEC-196) | **Not implemented** in TreeView. (Also not implemented in ListView.) |
| Column pinning (SPEC-197) | **Not implemented** in TreeView. (Also not implemented in ListView.) |
| Column selection (SPEC-198) | **Not implemented** in TreeView. (Also not implemented in ListView.) |
| OrderedSelectionState (SPEC-178) | **Implemented.** `TreeViewCore.selection: OrderedSelectionState<u64>` initialized with `SelectionMode::Multi`. Full reducer integration: click, Ctrl/Cmd+click, Shift+click, Shift+drag, range extend, toggle, select-all, clear. |
| Scroll domain separation | **Implemented.** Header painted directly by TreeView (outside ScrollArea). Body inside ScrollArea. Footer is a separate child widget outside ScrollArea. Same architecture as ListView. |
| Header behavior identical | **Implemented.** Per-slot clips, scroll-domain-aware visual_x, separator rendering with hover/active states. Matches ListView header paint pattern. |
| Footer renderer hook | **Not implemented.** TreeViewFooter exists as a structural widget rendering separators only. No `set_footer_renderer()` API. |
| INV-TV-PARITY-2 (Column Engine routing) | **Correct.** All geometry through HTS which wraps ColumnGeometryEngine. No TreeView-specific geometry. |
| INV-TV-PARITY-3 (Selection visual match) | **Implemented.** Row selection background uses `row_selected` (focused) / `row_selected_inactive` (unfocused) matching ListView. Focus border rendered with `borders.focus`. Multi-select, range, toggle all visually correct. |

---

## 4. Selection Integration

TreeView uses OrderedSelectionState<u64> from SPEC-178.

### INV-TV-SEL-1

Selection identity is `node_id: u64` (caller-assigned stable key), never projection index.

### INV-TV-SEL-2

TreeView supports:

- Multi-selection (SelectionMode::Multi, hardcoded at construction)
- Range selection (Shift+click, Shift+arrow extend)
- Toggle selection (Ctrl/Cmd+click, Space)
- Anchor preservation across focus moves (anchor_key reset on plain navigation)
- Drag-select (Shift+pointer-down arms, pointer-move extends, pointer-up finalizes)

### INV-TV-SEL-3

Selection state lives in TreeViewCore as `OrderedSelectionState<u64>`. HTS implements `OrderedIndex<u64>` and serves as the ordering oracle for the reducer. This mirrors ListView's architecture (ListViewCore owns selection, TabularSubstrate is oracle).

### INV-TV-SEL-4

Expand/collapse operations MUST NOT implicitly clear selection.

The `ensure_focus_visible` helper may move focus (via `FocusTo`) after structural mutations, but this does not alter `selected_keys`.

### Implementation Status

| Invariant | Status |
|-----------|--------|
| INV-TV-SEL-1 (Identity-based selection) | **Implemented.** Selection uses `node_id: u64` throughout. No index-based selection. |
| INV-TV-SEL-2 (Selection modes) | **Implemented.** Multi-select, range (Shift+arrow/click/drag), toggle (Ctrl/Cmd+click, Space), select-all (Ctrl/Cmd+A), clear (ESC). |
| INV-TV-SEL-3 (Selection architecture) | **Implemented.** `TreeViewCore.selection: OrderedSelectionState<u64>`. All mutations go through `reduce_ordered_selection()` with `&core.hts` as oracle. |
| INV-TV-SEL-4 (Expand doesn't clear) | **Implemented.** Expand/collapse/toggle call `expand_by_id`/`collapse_by_id`/`toggle_by_id` on HTS. The only focus-related side effect is `ensure_focus_visible` which dispatches `FocusTo` (anchor reset, no selection mutation). |

---

## 5. Expand / Collapse Semantics

TreeView exposes interaction surface only.

Expand/collapse state storage lives in HierarchicalTabularSubstrate (HTS).

TreeView public API:

- `expand(row_index: usize)` — routes to `hts.expand(row_index)`
- `collapse(row_index: usize)` — routes to `hts.collapse(row_index)`
- `toggle(row_index: usize)` — routes to `hts.toggle(row_index)`

HTS additionally provides `expand_by_id(node_id)`, `collapse_by_id(node_id)`, `toggle_by_id(node_id)`. These are used internally by structural navigation (Right/Left arrows) and DoubleClick handlers but not exposed on TreeView's public surface.

### NOT IMPLEMENTED / DEFERRED

The following were originally specified but do not exist:

- `expand_all()` — not on TreeView or HTS
- `collapse_all()` — not on TreeView or HTS
- `expand_subtree(node_id)` — not implemented anywhere
- `collapse_subtree(node_id)` — not implemented anywhere
- `_by_id(node_id)` variants on TreeView's public surface (exist on HTS, not forwarded)

### INV-TV-EXP-1

All expand/collapse actions must route via HTS action API.

### INV-TV-EXP-2

TreeView must not mutate topology directly.

### Implementation Status

| Invariant | Status |
|-----------|--------|
| INV-TV-EXP-1 (Route via HTS) | **Correct.** All expand/collapse routes through HTS. Keyboard (Right/Left), DoubleClick, and DisclosureToggle all use `expand_by_id`/`collapse_by_id`/`toggle_by_id`. |
| INV-TV-EXP-2 (No direct topology mutation) | **Correct.** TreeView/TreeViewBody never mutate topology directly. |

**Note:** TreeView's public API still uses `row_index: usize` parameters for expand/collapse/toggle, which conflicts with FORBIDDEN-2 (index-based identity). The internal paths (keyboard, double-click, disclosure toggle) correctly use `_by_id` variants.

---

## 6. Virtualization Law

TreeView projection MUST comply with:

- Fixed row height virtualization (SPEC-250)
- Virtualization Strategy Layer (SPEC-257)

### INV-TV-VIRT-1

TreeView must operate only on visible projection rows.

### INV-TV-VIRT-2

Expand/collapse must invalidate projection through layout invalidation.

### Implementation Status

| Invariant | Status |
|-----------|--------|
| Fixed row height virtualization | **Implemented.** `hts.visible_rows_with_total()` returns only the visible slice based on scroll offset and viewport height. |
| INV-TV-VIRT-1 (Visible rows only) | **Implemented.** Layout operates on `visible_rows`. Paint iterates `self.visible_rows`. Child widgets (DisclosureToggle) only exist for visible rows. |
| INV-TV-VIRT-2 (Invalidation) | **Implemented.** All structural mutation paths (Right/Left keyboard, DoubleClick, resize) invalidate both TreeView and body via `cx.invalidate_layout()` + `cx.invalidate_layout_by_id(body_id)`. See INV-STRUCTURAL-INVALIDATION. |

---

## 7. Rendering Contract

TreeView must obey the same clipping and geometry invariants as ListView.

### INV-TV-RENDER-1

Per-cell clip boundaries must align with Column Engine separator coordinates.

### INV-TV-RENDER-2

Header clipping must match ListView header semantics.

### INV-TV-RENDER-3

Disclosure controls must remain within tree column clip.

### INV-TV-RENDER-4

Per-cell content must remain clipped to resolved column boundaries. Row-wide selection and hover bands may span the resolved body width, but must not escape the body clip.

### Implementation Status

| Invariant | Status |
|-----------|--------|
| INV-TV-RENDER-1 (Cell clip = separator) | **Implemented.** Body paint uses `(base_x + slot.x + slot.width - 1.0).round()` for clip right edge. Header paint uses `(visual_x + slot.width - 1.0).round()`. Both derived from Column Engine slot geometry. |
| INV-TV-RENDER-2 (Header clip parity) | **Implemented.** Per-slot `begin_clip`/`end_clip` in header paint with whole-header outer clip. Separator rendering outside per-slot clip. |
| INV-TV-RENDER-3 (Disclosure in tree column) | **Implemented.** `clip_bounds()` returns `tree_col_clip`. `eligible_ids` filter rejects toggles that don't fit within effective clip boundary. |
| INV-TV-RENDER-4 (No bleed) | **Implemented.** Selection and identity-based hover backgrounds intentionally span the full row width, matching ListView row-wide visual semantics. Per-cell text content remains clipped per column. |

---

## 8. Footer Policy

TreeView may expose:

```
set_footer_renderer(|column_id| -> Option<Widget>)
```

### INV-TV-FOOTER-1

Footer has no intrinsic semantics.

### INV-TV-FOOTER-2

Footer must not alter substrate state.

### Implementation Status

| Requirement | Status |
|-------------|--------|
| `set_footer_renderer` | **Not implemented.** TreeViewFooter is a structural completeness widget. Renders column separators aligned with header/body. No renderer hook API. |
| INV-TV-FOOTER-1 (No intrinsic semantics) | **Satisfied.** Footer is purely visual (separators + background). |
| INV-TV-FOOTER-2 (No substrate mutation) | **Satisfied.** `handle_event` returns false. Footer only reads shared state (`hovered_separator`, `active_resize_separator`). |

---

## 9. Forbidden Behaviors

### FORBIDDEN-1

Duplicating selection state inside TreeView.

### FORBIDDEN-2

Index-based identity for selection or expansion.

### FORBIDDEN-3

Column geometry logic inside TreeView.

### FORBIDDEN-4

Substrate mutation outside defined action APIs.

### FORBIDDEN-5

Bypassing Engine Invalidation Graph.

### Implementation Status

| Rule | Status |
|------|--------|
| FORBIDDEN-1 (Selection duplication) | **Clean.** Single `OrderedSelectionState<u64>` in TreeViewCore. No duplicate selection tracking. |
| FORBIDDEN-2 (Index-based identity) | **Violated for public expansion API only.** `TreeView::expand(row_index)`, `collapse(row_index)`, `toggle(row_index)` use `usize` index. All internal paths (keyboard, double-click, disclosure toggle) correctly use `_by_id(node_id)`. Selection uses `node_id: u64` throughout. |
| FORBIDDEN-3 (Geometry in widget) | **Clean.** No column geometry math in TreeView or TreeViewBody. All geometry from HTS/ColumnGeometryEngine. |
| FORBIDDEN-4 (Mutation outside action API) | **Clean.** All topology mutations go through HTS (`expand_by_id`, `collapse_by_id`, `toggle_by_id`, `set_tree_data`, `set_columns`). |
| FORBIDDEN-5 (Bypassing invalidation) | **Clean.** Layout invalidation flows through `cx.invalidate_layout()` + `cx.invalidate_layout_by_id(body_id)`. |

---

## 10. Minimal Surface API (Current)

TreeView currently exposes:

| API | Notes |
|-----|-------|
| `set_tree_data(nodes: Vec<NodeMeta>)` | Lower-level direct data injection (not adapter pattern). |
| `set_columns(model: ColumnModel)` | Column configuration. |
| `set_tree_column(column_id)` | Designates which column receives indentation/disclosure/guides. |
| `set_column_title(column_id, title)` | Header text per column. |
| `set_row_height(height)` | Fixed row height. |
| `set_footer_height(h)` | Footer height (0 = none). |
| `set_header_visible(visible)` / `header_visible()` | Configure/read header-band presence per SPEC-HAL-348. |
| `expand(row_index)` / `collapse(row_index)` / `toggle(row_index)` | Expansion control (index-based public API; see FORBIDDEN-2 note). |
| `selection_state()` / `selection_state_mut()` | Public accessors returning `Ref`/`RefMut` of `OrderedSelectionState<u64>`. |
| `clear_selection()` / `select_all()` | Convenience selection methods routing through reducer. |
| `create_body_widget()` | Creates TreeViewBody for ScrollArea composition. |
| `create_footer_widget()` | Creates TreeViewFooter. |
| `horizontal_scroll_link()` | Returns HorizontalScrollLink for ScrollArea wiring. |
| `hts()` / `hts_mut()` | Direct HTS access (immutable/mutable). |
| `revision()` | HTS revision counter accessor. |

### NOT IMPLEMENTED / DEFERRED

- `set_adapter(...)` — no adapter pattern; direct data injection via `set_tree_data`
- `set_footer_renderer(...)` — footer exists but has no renderer hook
- Selection mode configuration — hardcoded `SelectionMode::Multi`
- `expand_all()` / `collapse_all()` / `expand_subtree()` / `collapse_subtree()`
- `_by_id(node_id)` variants on TreeView public surface (exist on HTS)

---

## 11. Current Implementation Snapshot

### Files

- `src/widgets/tree_view/tree_view.rs` — Composite widget: header paint, keyboard handling, column resize, focus lifecycle.
- `src/widgets/tree_view/body.rs` — TreeViewBody: row paint, pointer handling (click/drag/double-click), disclosure toggle reconciliation. Contains `TreeViewCore` struct.
- `src/widgets/tree_view/footer.rs` — TreeViewFooter: separator rendering, no interaction.
- `src/widgets/tree_view/disclosure_toggle.rs` — DisclosureToggle child widget.
- `src/engines/hierarchical/mod.rs` — HierarchicalTabularSubstrate: topology + geometry + virtualization orchestration. Implements `OrderedIndex<u64>`.
- `src/core/ordered_selection/mod.rs` — `OrderedSelectionState<ItemKey>`, `reduce_ordered_selection()`, all selection actions.

### Selection/Focus Model

TreeView uses `OrderedSelectionState<u64>` stored in `TreeViewCore.selection`, initialized with `SelectionMode::Multi`. All selection mutations go through `reduce_ordered_selection(&mut core.selection, action, &core.hts)` where HTS serves as the `OrderedIndex<u64>` oracle.

TreeView dispatches selection actions directly to the reducer — it does NOT use ListView's `TabularIntentPolicy` / intent pipeline. This is acceptable because both widgets use the same `OrderedSelectionState` + `reduce_ordered_selection` reducer semantics. The difference is dispatch path only, not selection model.

### Expansion/Collapse Model

Expansion state is owned by HTS topology (`HierarchicalRowTopology.expanded: Vec<bool>`). TreeView mutates via `expand_by_id(node_id)` / `collapse_by_id(node_id)` / `toggle_by_id(node_id)`.

Structural navigation (Right/Left arrows) and DoubleClick in body both route through these HTS methods. DisclosureToggle callback also uses `toggle_by_id` directly.

### Focus Lifecycle

TreeView is `focusable() = true`. `on_focus` sets `core.focused = true`; `on_blur` sets `core.focused = false` and cancels any active resize (rollback). Paint uses `core.focused` to choose `row_selected` vs `row_selected_inactive` colors.

---

## 12. Binding Invariants

### INV-FOCUS-VISIBLE

**Statement:** After any structural mutation (expand/collapse/toggle) triggered by keyboard or double-click, the focused key must refer to a node present in `hts.visible_rows()`.

**Enforcement:** The free function `ensure_focus_visible(core: &mut TreeViewCore, visible_rows: &[VisibleRow])` in `tree_view.rs` repairs focus after every structural mutation:

1. If `focus_key` is `None` → no-op.
2. If `focus_key` is present in `visible_rows` → no-op.
3. Walk parent chain via topology: `topology().index_for_id(id)` → `node_meta(phys).parent_index` → `topology().node_id(parent_phys)`. If parent is in `visible_rows` → dispatch `FocusTo { key: parent_id }`.
4. If no visible ancestor found → dispatch `FocusTo` to first visible row.

`FocusTo` resets `anchor_key` and clears `range_anchor_key` but does not mutate `selected_keys`, satisfying INV-TV-SEL-4.

**Call sites:** Right arrow expand path, Left arrow collapse path, DoubleClick toggle in body.

### INV-STRUCTURAL-INVALIDATION

**Statement:** Structural mutations (expand/collapse/toggle) that change visible row composition must invalidate the body widget's layout explicitly, otherwise the change will not render until an incidental relayout trigger (window resize, scroll).

**Enforcement:** All structural mutation paths in `handle_keyboard` (Right/Left) read `core.body_widget_id` and call both `cx.invalidate_layout()` (TreeView) and `cx.invalidate_layout_by_id(body_id)` (TreeViewBody). The DoubleClick handler in body.rs calls `cx.invalidate_layout()` from within the body widget itself, which directly marks it dirty.

**Why needed:** `cx.invalidate_layout()` on TreeView marks only TreeView as needing layout. TreeViewBody is inside ScrollArea; without explicit body invalidation, ScrollArea may not re-layout the body, leaving its cached `self.visible_rows` stale.

### INV-RESIZE-CANCEL-ROLLBACK

**Statement:** Resize cancel (ESC or blur) rolls back to the initial column width captured at arm time. It does not commit the last drag position.

**Enforcement:**

- **ESC during drag:** `cancel_resize()` restores `initial_width` via `apply_column_width`, then calls `finalize_resize()` to clear all resize state.
- **Blur during drag:** `on_blur()` calls `cancel_resize()` (same rollback path).
- **Finalize (commit):** `finalize_resize()` on PointerUp clears state without width rollback — the last-applied width from the drag is kept.

State machine: `Idle → Armed → Dragging → Idle`. Cancel returns to `Idle` with width restoration. Commit returns to `Idle` without width restoration.

### INV-NO-REFCELL-PANICS

**Statement:** Within each handler, `self.core` is borrowed in a single `borrow_mut()` scope. No nested borrows. No borrow reuse across helper calls that re-borrow.

**Enforcement:** All keyboard and pointer handlers follow the pattern: `let mut core = self.core.borrow_mut(); ... drop(core);` before any framework calls that might re-enter.

### INV-SEPARATOR-DBLCLICK-NOT-CONSUMED

**Statement:** Double-click on a column separator in body must return `false`, allowing the event to bubble to TreeView for column autosize handling.

**Enforcement:** `TreeViewBody::handle_event` for `PointerEventKind::DoubleClick` checks `self.is_on_separator(local_x)` and returns `false` if hit, before attempting row toggle.

---

## 13. Input Behavior Matrix (Current Reality)

All behaviors below are implemented and tested. "Selection" refers to `selected_keys`. "Focus" refers to `focus_key`.

| Input | Effect on Selection | Effect on Focus | Effect on Expansion | Consumes? |
|-------|-------------------|-----------------|--------------------| ---------|
| **Click** (primary) | Clears selection, selects clicked row | Sets focus + anchor to clicked row | None | Yes |
| **Ctrl/Cmd+Click** | Toggles clicked row in/out of selection | Sets focus to clicked row | None | Yes |
| **Shift+Click** | Extends selection from anchor to clicked row | Sets focus to clicked row | None | Yes |
| **Shift+Drag** | Range-selects from anchor to current row | Tracks current row | None | Yes |
| **Up** | No change | Focus previous row, reset anchor | None | Yes |
| **Down** | No change | Focus next row, reset anchor | None | Yes |
| **Shift+Up** | Extends range from anchor | Focus previous row | None | Yes |
| **Shift+Down** | Extends range from anchor | Focus next row | None | Yes |
| **Ctrl/Cmd+Up** | No change | Move focus previous (no anchor reset) | None | Yes |
| **Ctrl/Cmd+Down** | No change | Move focus next (no anchor reset) | None | Yes |
| **Right** (collapsed parent) | No change | Unchanged | Expands node | Yes |
| **Right** (expanded parent) | No change (or ExtendTo if Shift) | Focus first child | None | Yes |
| **Right** (leaf) | No change | Unchanged | None | No |
| **Left** (expanded node) | No change | Unchanged (ensure_focus_visible) | Collapses node | Yes |
| **Left** (collapsed, depth > 0) | No change (or ExtendTo if Shift) | Focus parent | None | Yes |
| **Left** (collapsed root) | No change | Unchanged | None | No |
| **Shift+Right** (expanded parent) | ExtendTo first child | Focus first child | None | Yes |
| **Shift+Left** (collapsed, depth > 0) | ExtendTo parent | Focus parent | None | Yes |
| **Home** | No change | Focus first row, reset anchor | None | Yes |
| **End** | No change | Focus last row, reset anchor | None | Yes |
| **Shift+Home** | Extends to first row | Focus first row | None | Yes |
| **Shift+End** | Extends to last row | Focus last row | None | Yes |
| **PageUp/Down** | No change | Focus page delta, reset anchor | None | Yes |
| **Shift+PageUp/Down** | Extends by page | Focus page delta | None | Yes |
| **Space** | Toggles focused row selection | Unchanged | None | Yes |
| **Ctrl/Cmd+A** | Selects all visible rows | Unchanged | None | Yes |
| **ESC** | Clears all selection | Clears focus | None (cancels resize if active) | Yes |
| **Enter** | **No behavior implemented** | Unchanged | None | No |
| **DoubleClick** (parent row) | No change | Unchanged (ensure_focus_visible) | Toggles expand/collapse | Yes |
| **DoubleClick** (leaf) | No change | Unchanged | None | No |
| **DoubleClick** (separator) | No change | Unchanged | None | No (bubbles to autosize) |

---

## 14. Deferred / Not Implemented

### Selection Mode Configuration

`SelectionMode` is hardcoded to `Multi` at TreeView construction. TreeView has no API to switch between `None`, `Single`, and `Multi` modes at runtime. ListView admits pre-interaction intent-policy configuration under SPEC-HAL-347, but its first routed row interaction locks the policy-backed selection configuration; it does not admit runtime switching after interaction begins.

### Transactional Selection

`OrderedSelectionState` supports a transactional mode (`begin_transaction` / `commit_transaction` / `rollback_transaction`). TreeView does not use transactional mode. This is deferred.

### Column Features

Column reordering (SPEC-182), visibility (SPEC-196), pinning (SPEC-197), and column selection (SPEC-198) are not implemented in TreeView or ListView.

### Bulk Expansion

`expand_all()`, `collapse_all()`, `expand_subtree(node_id)`, `collapse_subtree(node_id)` do not exist on TreeView or HTS.

### Adapter Pattern

No `set_adapter(...)` exists. Data is injected directly via `set_tree_data(nodes: Vec<NodeMeta>)`.

### Footer Renderer

No `set_footer_renderer(...)` hook. TreeViewFooter is structural only.

### Activation Semantics

Enter key has no behavior. No `on_activate` callback or event exists.

---

## 15. Outcome

After adoption:

TreeView becomes a full Tabular Composite Peer.

ListView and TreeView differ only by:

- Row topology (flat vs hierarchical)
- Disclosure rendering
- Expand/collapse interaction surface
- Structural navigation (Right/Left arrows — TreeView only)

All other mechanics remain unified through shared OrderedSelectionState reducer semantics and Column Geometry Engine routing.

---

## Appendix: Audit Summary

**Reconciliation performed against commit history through `33aef5f`.**

### Fully Implemented

- Composite architecture (TreeView + ScrollArea + TreeViewBody + TreeViewFooter)
- HTS as sole topology authority
- Column geometry delegation to HTS/ColumnGeometryEngine
- Scroll domain separation (header/footer outside ScrollArea, body inside)
- Header per-slot clipping with separator alignment
- Body per-cell clipping with separator alignment
- Disclosure toggle child widget lifecycle (virtualized, identity-stable by node_id)
- Disclosure toggle tree-column clip constraint (clip_bounds + eligible_ids filter)
- Fixed row height virtualization via HTS
- Column resize (drag + auto-size via double-click)
- Layout invalidation path (body_widget_id + invalidate_layout_by_id)
- Footer separator alignment and structural rendering
- OrderedSelectionState<u64> integration (multi-select, range, toggle, drag-select)
- Selection visuals (row_selected / row_selected_inactive, focus border)
- Structural navigation (Right: expand/descend, Left: collapse/ascend)
- Double-click expand/collapse toggle
- Focus visibility repair after structural mutations (ensure_focus_visible)
- Focus lifecycle (on_focus/on_blur with inactive selection visuals)
- Resize cancel rollback on ESC and blur
- Anchor reset on plain focus navigation (FocusPrevious/Next/First/Last/Page/To)
- Identity-based row hover, including lifecycle clear when the pointer leaves `TreeViewBody`

### Partially Implemented

- Expand/collapse public API uses row_index (FORBIDDEN-2 violation); internal paths correctly use _by_id. No bulk operations.

### Not Implemented

- Selection mode runtime configuration
- Transactional selection
- set_adapter() pattern
- set_footer_renderer() hook
- Column reordering, visibility, pinning, column selection (none exist in ListView either)
- expand_all / collapse_all / expand_subtree / collapse_subtree
- _by_id variants on TreeView public surface
- Enter / activation semantics
