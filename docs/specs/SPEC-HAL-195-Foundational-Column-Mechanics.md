# SPEC-HAL-195: Foundational Column Mechanics (FCM)

**Status:** Draft (Binding upon adoption)
**Layer:** Tabular Semantics / Cross-View Foundation
**Scope:** ListView, TreeView, GridView, any tabular derivative
**Audience:** Widget, Interaction, Substrate, Layout
**Depends On:** SPEC-HAL-164, SPEC-HAL-179, SPEC-HAL-180
**Non-Goals:** Multi-row headers, column spanning, grouping, aggregation

---

## 0. Purpose

This SPEC defines the minimal, irreducible semantic mechanics of columns that must be shared across all tabular views.

It exists to prevent:

- feature-driven divergence (ListView vs TreeView vs Grid)
- UI-led semantics
- irreversible coupling between presentation and state

This SPEC defines what columns *are*, not how they look or how users interact with them.

---

## 1. Column Identity (Foundational)

### INV-COL-IDENTITY-1 — Stable Column Identity

Each column has a stable identity (`ColumnId`) independent of:

- order
- visibility
- pinning
- width
- sorting state

Column identity must never change during the lifetime of the view.

### INV-COL-IDENTITY-2 — Semantic Identity

Column identity is semantic, not visual.

Reordering or hiding a column does not affect:

- selection
- sorting
- data addressing
- pinning relationships

---

## 2. Column State Model (Canonical)

Each column has exactly these orthogonal semantic states:

```rust
ColumnState {
    visible: bool,
    pinned: PinState,
    width: WidthState,
    sort: SortState,
    selectable: bool,
}
```

No state may implicitly modify another.

---

## 3. Visibility (Foundational, Non-Optional)

### INV-COL-VIS-1 — Visibility Is Semantic

Visibility is a semantic state, not a layout optimization.

A hidden column:

- still exists
- still participates in sorting and selection (unless explicitly disabled)
- still has width, pin, and order metadata

### INV-COL-VIS-2 — Recovery Path Required

Every tabular view must provide a recovery path for hidden columns.

This does not mandate UI, but mandates capability.

Acceptable recovery mechanisms:

- programmatic API
- debug menu
- context menu
- inspector panel

Invisible columns must never become orphaned state.

---

## 4. Ordering and Layout Authority

### INV-COL-ORDER-1 — Semantic Ownership

Column ordering is owned by semantic state, not by layout.

Layout consumes column order; it does not define it.

### INV-COL-ORDER-2 — Hidden Columns Retain Order

Hidden columns retain their order slot.

Visibility toggling must not reshuffle other columns.

---

## 5. Pinning (Semantic, Not Visual)

Pinning is a constraint on horizontal scroll participation, not a positioning hack.

### INV-COL-PIN-1 — Scroll Space Removal

Pinned columns are removed from horizontal scroll space.

They remain:

- layout-participating
- width-contributing
- hit-testable

### INV-COL-PIN-2 — Canonical Pin Order

Pinned columns are ordered relative to each other, then to unpinned columns.

Canonical order:

```
[pinned-left] [unpinned-scrollable] [pinned-right]
```

No column may exist outside these domains.

### INV-COL-PIN-3 — Pin Stability

Pinning must be stable under resize, scroll, and visibility changes.

---

## 6. Width (Formalized)

### INV-COL-WIDTH-1 — Semantic Ownership

Width is owned by column state, not by layout.

Layout consumes resolved width; it does not clamp semantics.

### INV-COL-WIDTH-2 — Single Mutation Pipeline

All width mutation flows through a single semantic pipeline.

No ad-hoc clamping, no duplicated logic.

---

## 7. Selection (Column-Level)

### INV-COL-SEL-1 — Orthogonal to Row Selection

Column selection is orthogonal to row selection.

It does not imply:

- sorting
- resizing
- pinning

### INV-COL-SEL-2 — Identity-Based Selection

Multi-column selection must be representable as a set of `ColumnId`.

No positional or index-based selection is allowed.

---

## 8. Sorting (Semantic Only)

### INV-COL-SORT-1 — Descriptor, Not Executor

Sorting is a semantic descriptor, not an execution mechanism.

This SPEC defines state, not algorithms.

### INV-COL-SORT-2 — Unambiguous Multi-Column Sort

Single- and multi-column sorting must be representable without ambiguity.

```rust
SortState {
    priority: Option<u32>,
    direction: Asc | Desc,
}
```

Execution is deferred to adapters / data providers.

---

## 9. Affordances (Explicitly Non-Semantic)

An affordance is a non-semantic, culturally learned visual cue suggesting possible interaction.

Examples: resize cursor, separator highlight, drag handle glyph, hover emphasis.

### INV-AFF-1 — Affordances Must Not Define Behavior

Removing an affordance must not break semantics.

### INV-AFF-2 — Affordances Are View-Specific

Affordances may vary per view, per platform, per theme.

They are explicitly excluded from this SPEC's authority.

---

## 10. Explicit Non-Goals

This SPEC does not define:

- multi-row headers
- column spanning
- grouping
- aggregation
- tree-level column semantics
- persistence / serialization format
- UI for configuration

Those require new SPECs layered on top of this one.

---

## 11. Architectural Payoff

Adopting this SPEC guarantees:

- ListView, TreeView, GridView share column semantics
- no repaint-driven state loss
- no UI-specific hacks leaking into core
- future features compose instead of collide

---

## 12. Enforcement

Any implementation that:

- hides columns without recovery
- ties pinning to layout hacks
- duplicates width authority
- conflates affordance with behavior

violates this SPEC and must be rejected.
