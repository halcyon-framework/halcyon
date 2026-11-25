# SPEC-HAL-196: Column Visibility and Recovery Mechanisms

**Status:** Draft (Binding upon adoption)
**Layer:** TabularSubstrate Column Model / ListView Semantics
**Owner:** TabularSubstrate (state), ListView (presentation semantics), Host (UI exposure)
**Audience:** Engine, Widget, Interaction, Layout, Demo/Examples
**Scope:** ListView, TreeView projection over TabularSubstrate, Grid-like views consuming TabularSubstrate
**Depends On:** SPEC-HAL-179A, SPEC-HAL-195 (Authority), TabularSubstrate geometry/column model
**Amended by:** SPEC-HAL-353 (hidden active-sort policy for provider-authoritative consumers)
**Non-Goals:** Multi-level headers, spanning, column groups, banded headers, pivot-style projections

---

## 0. Purpose

Define column visibility as a reversible, identity-preserving, non-destructive presentation state that applies uniformly across all TabularSubstrate consumers.

Visibility must never create "dead ends." If a column can be hidden, there must be a guaranteed recovery path.

---

## 1. Terms

- **ColumnId** — Stable identity for a column.
- **Logical column order** — The ordered list of columns in the model (includes hidden columns).
- **Visible set** — Columns currently participating in layout/paint/hit-testing.
- **Hidden set** — Columns excluded from layout/paint/hit-testing, but still present in the model.
- **Recovery mechanism** — A guaranteed user- or host-accessible way to make hidden columns visible again.

---

## 2. Model

Each column has a visibility state:

```rust
enum ColumnVisibility {
    Visible,
    Hidden,
}
```

Visibility is orthogonal to:

- selection (row/column)
- sorting
- pinning
- focus

---

## 3. Binding Invariants

### Identity / Persistence

### INV-COLVIS-IDENTITY-1 — Identity Preservation

Hiding a column MUST NOT destroy, replace, or change its `ColumnId`.

### INV-COLVIS-ORDER-1 — Order Preservation

Hidden columns MUST retain their position in the logical column order.

### INV-COLVIS-CONFIG-1 — Configuration Preservation

All column configuration (title, width intent, min/max constraints, custom metadata) MUST remain intact while hidden.

### Geometry / Rendering

### INV-COLVIS-GEOM-1 — Zero Geometry Contribution

Hidden columns MUST contribute zero to:

- content width
- header slot generation
- separator slot generation
- any scroll extents derived from column geometry

### INV-COLVIS-PAINT-1 — No Paint Output

Hidden columns MUST NOT paint:

- header cell content
- body cell content
- column separators
- hover/active resize indicators for that column

### INV-COLVIS-HITTEST-1 — No Hit-Test Participation

Hidden columns MUST NOT participate in hit-testing:

- no separator hit bands
- no header hit regions
- no column-resize affordances
- no column selection hit targets

### Interaction / State Safety

### INV-COLVIS-INTERACT-1 — No Implicit Side Effects

Hiding/unhiding MUST NOT implicitly:

- change selection (rows or columns)
- change sorting
- change pinning
- change focus state

### INV-COLVIS-RESIZE-1 — No Hide During Active Resize

A column MUST NOT be hidden while it is actively being resized. If a hide request targets an actively resizing column, it MUST be rejected or deferred until resize finalization.

### INV-COLVIS-RECOVER-1 — Guaranteed Recovery

There MUST exist at least one recovery mechanism, always reachable, such that every hidden column can be made visible again.

### Authority

### INV-COLVIS-AUTH-1 — Substrate Ownership

Visibility state is owned by TabularSubstrate's column model (or an equivalent column-state authority), not by ListView paint/layout code.

ListView may:

- expose intent (hide/show/toggle)
- present UI
- interpret gestures and commands

ListView must not:

- fork geometry tables to simulate visibility
- maintain a private shadow visibility map diverging from the substrate

---

## 4. Operations and Semantics

### 4.1 Hide / Show / Toggle

Operations are defined over `ColumnId`:

- `Hide(ColumnId)`
- `Show(ColumnId)`
- `Toggle(ColumnId)`

Semantics:

1. `Hide(id)` sets visibility to Hidden.
2. `Show(id)` sets visibility to Visible.
3. `Toggle(id)` flips state.

No other state changes are implied.

### 4.2 No-Dead-End Rule (Recovery)

A view that supports hiding MUST also support recovery. Recovery may be delivered via:

- A "Columns..." menu entry (context menu, toolbar, overflow button)
- A dedicated "Column chooser" panel/sheet
- A host-provided command surface (programmatic API) if the view is embedded without chrome

This SPEC mandates existence, not UI aesthetics.

### 4.3 Minimum Visible Columns Policy

This is a policy point; the system must define behavior if the user tries to hide everything.

### INV-COLVIS-MINV-1 — Explicit Minimum Policy

If the view enforces "at least one visible column", the enforcement MUST be explicit and deterministic:

- either refuse hiding the last visible column
- or auto-show a designated fallback column (must be declared by the host)

If the view allows zero visible columns, rendering must degrade gracefully (empty header/body), and recovery must still function.

Choose one policy per consumer; do not mix.

---

## 5. Data Model Placement

Visibility lives with column metadata in the substrate-level column state (preferred), because:

- TreeView/Grid/ListView projections need identical semantics
- geometry and separator generation depend on visibility
- recovery lists must enumerate all columns, visible or not

---

## 6. Geometry Implications

When a column is hidden:

- It is removed from the visible-column sequence used to generate header slots.
- It is removed from separator computation (no trailing separator creation rules apply only to visible columns).
- Total content width is computed over visible columns only.
- Any cached geometry epochs must invalidate appropriately (same as width change).

This SPEC does not prescribe the internal geometry table representation, only outputs.

---

## 7. Selection / Sorting / Pinning Interactions

Visibility is orthogonal, but interactions must be defined.

### 7.1 Sorting

If a hidden column is the current sort key:

For ListView and every SPEC-HAL-353 consumer, the binding policy is:

- Keep the provider-confirmed sort term active.
- Render no indicator while the column is hidden.
- Restore its direction and priority indicator when the column is shown again.
- Do not emit a sort request as a side effect of hide/show.

Legacy consumers outside SPEC-HAL-353 may retain an explicitly documented alternative, but clearing sort on hide is not
lawful for provider-authoritative ListView.

### 7.2 Column Selection

Hidden columns may remain selected.

### INV-COLVIS-CSEL-1 — Hidden Selection Safety

If a hidden column is selected, selection state may persist, but it must not produce paint/hit artifacts while hidden.

### 7.3 Pinning

If pinning exists, hidden pinned columns remain pinned in state, but contribute zero geometry while hidden.

---

## 8. Required API Surface (Conceptual)

Exact signatures are implementation-defined, but these capabilities must exist:

- **Query:**
  - enumerate all columns (logical order)
  - enumerate visible columns
  - get/set visibility by `ColumnId`
- **Intents:**
  - hide/show/toggle
  - show-only set (optional convenience)
- **Recovery support:**
  - enumerate hidden columns for UI presentation

---

## 9. Tests and Verification Requirements

Minimum verification for any consumer implementing visibility:

1. Hide a column: it disappears from header, body, separators, hit zones.
2. Show it again: identity and order preserved; width/config restored.
3. Hidden columns do not affect `content_width`.
4. Recovery mechanism can re-show any hidden column.
5. Hiding during active resize is refused/deferred deterministically.
6. Single-column case: behavior matches chosen "min visible" policy.

---

## 10. Explicitly Forbidden

- Column groups or multi-level header constructs
- Spanning headers
- Hiding by viewport responsiveness
- Implicit hiding as a side effect of resizing, scrolling, sorting, or selection
- Divergent per-consumer visibility state (shadow maps)

---

## 11. Notes for Later SPECs

This SPEC intentionally limits scope. Future SPECs may build on it:

- Column pinning over scroll domains (SPEC-HAL-197)
- Column selection model (SPEC-HAL-198)
- Provider-authoritative sorting semantics (SPEC-HAL-353)
- Column reordering (future)

They must treat visibility as an orthogonal axis and preserve INV-COLVIS-* invariants.
