# SPEC-HAL-197: Column Pinning Semantics over Scroll Domains

**Status:** Draft (Binding upon adoption)
**Layer:** TabularSubstrate Column Model / Scroll Semantics
**Owner:** TabularSubstrate (state), ScrollArea (scroll domains), ListView (presentation semantics)
**Audience:** Engine, Widget, Interaction, Layout
**Scope:** ListView, TreeView projection over TabularSubstrate, Grid-like views
**Depends On:** SPEC-HAL-196 (Column Visibility), SPEC-HAL-195 (Authority), SPEC-HAL-179A
**Non-Goals:** Column grouping, multi-level headers, spanning, frozen rows, spreadsheet-style panes

---

## 0. Purpose

Define column pinning as a structural relationship between columns and horizontal scroll domains.

Pinning allows selected columns to remain visible while horizontal scrolling occurs, without duplicating geometry, breaking identity, or corrupting interaction semantics.

This SPEC defines what pinning means, where it lives, and what it is allowed to affect.

---

## 1. Terms

- **Pinned column** — A column whose visual X-position is invariant under horizontal scroll.
- **Unpinned column** — A column whose visual X-position is offset by horizontal scroll.
- **Scroll domain** — A horizontal coordinate space affected by a scroll offset.
- **Pinned domain** — Scroll domain with `scroll_x = 0`.
- **Scrollable domain** — Scroll domain with `scroll_x = horizontal_scroll_offset`.

---

## 2. Conceptual Model

Pinning is not a rendering trick. Pinning is a column-to-scroll-domain assignment.

Each column belongs to exactly one horizontal domain:

```rust
enum ColumnScrollDomain {
    PinnedLeft,
    Scrollable,
    // (future extension: PinnedRight — explicitly NOT part of this SPEC)
}
```

Only `PinnedLeft` is defined by this SPEC.

---

## 3. Binding Invariants

### Identity / Authority

### INV-PIN-IDENTITY-1 — Identity Preservation

Pinning MUST NOT change `ColumnId`.

### INV-PIN-AUTH-1 — Substrate Ownership

Pinning state is owned by the column model (TabularSubstrate-level authority), not by ListView paint logic.

### Geometry / Layout

### INV-PIN-GEOM-1 — No Scroll Extent Contribution

Pinned columns MUST NOT contribute to horizontal scroll extents.

### INV-PIN-GEOM-2 — Independent Scroll Domain Geometry

Scrollable columns MUST compute geometry as if pinned columns were not present in their scroll domain.

### INV-PIN-GEOM-3 — Contiguous Left Edge

Pinned columns MUST appear contiguously at the left edge, in logical order relative to other pinned columns.

### INV-PIN-GEOM-4 — Pinned Domain Width

The pinned domain width equals the sum of visible pinned column widths.

### Scroll Semantics

### INV-PIN-SCROLL-1 — Pinned Scroll Offset

Pinned columns are rendered with `scroll_x = 0`.

### INV-PIN-SCROLL-2 — Scrollable Scroll Offset

Scrollable columns are rendered with `scroll_x = horizontal_scroll_offset`.

### INV-PIN-SCROLL-3 — Pinned Immobility

Horizontal scrolling MUST NOT move pinned columns.

### Interaction / Hit-Testing

### INV-PIN-HITTEST-1 — Domain-Correct Hit-Testing

Hit-testing coordinates MUST be resolved in the correct scroll domain for the column.

### INV-PIN-HITTEST-2 — Domain-Bounded Separators

Separator hit zones MUST respect domain boundaries:

- separators between pinned columns live in the pinned domain
- separators between scrollable columns live in the scrollable domain
- the separator between the last pinned and first scrollable column is forbidden unless explicitly defined (see section 7)

### Resize Behavior

### INV-PIN-RESIZE-1 — No Scroll Side Effects

Resizing a pinned column MUST NOT affect scroll offsets.

### INV-PIN-RESIZE-2 — Consistent Freeze Policy

While resizing any column, horizontal scrolling MAY be frozen (policy choice, but must be consistent).

### Visibility Interaction

### INV-PIN-VIS-1 — Orthogonal to Visibility

Pinning and visibility are orthogonal.

- A pinned column may be hidden.
- A hidden pinned column contributes zero width.
- Unhiding restores it to its pinned domain.

---

## 4. Semantics of Pinning

### 4.1 Pin / Unpin

Operations apply to `ColumnId`:

- `PinLeft(ColumnId)`
- `Unpin(ColumnId)`

Effects:

- `PinLeft`: column moves to `PinnedLeft` domain.
- `Unpin`: column moves to `Scrollable` domain.

No other state is implied.

### 4.2 Order Preservation

### INV-PIN-ORDER-1 — Relative Order Preservation

Pinning MUST preserve relative order within each domain.

When pinning:

- column is removed from scrollable sequence
- inserted into pinned sequence at a deterministic position

The insertion policy must be explicit:

- either "append to end of pinned set"
- or "preserve original logical order relative to other pinned columns"

One policy must be chosen and documented per consumer.

---

## 5. Geometry Composition

The total horizontal layout is composed as:

```
| pinned domain | scrollable domain |
```

- Pinned domain is clipped to its own rect.
- Scrollable domain is clipped to the remaining viewport width.
- Domains must not overlap.

This SPEC does not mandate how many scene nodes or widgets are used, only the semantic result.

---

## 6. Painting Rules

For header, body, and footer:

1. Paint pinned columns at: `x = pinned_origin + pinned_slot.x`
2. Paint scrollable columns at: `x = scrollable_origin + slot.x - scroll_x`

### INV-PIN-PAINT-1 — Shared Row Geometry

Pinned and scrollable columns MUST share identical Y-coordinates and row heights.

### INV-PIN-PAINT-2 — Deterministic Z-Order

Z-order between pinned and scrollable content MUST be deterministic (typically pinned above).

---

## 7. Separator Rules

Separators are defined between adjacent visible columns in the same domain only.

### INV-PIN-SEP-1 — No Cross-Domain Separator

No separator exists between the last pinned column and the first scrollable column by default.

Rationale:

- That boundary is structural, not a resizable boundary.
- Allowing resize there creates ambiguous behavior.

If a future SPEC allows cross-domain resize, it must explicitly override this invariant.

---

## 8. Hit-Testing and Events

Pointer coordinates must be resolved as follows:

1. Determine which domain the pointer lies in (pinned vs scrollable).
2. Convert viewport-local X to content-local X using that domain's scroll offset.
3. Perform hit-testing only against columns belonging to that domain.

### INV-PIN-EVENT-1 — No Cross-Domain Leaking

Events MUST NOT "leak" across domains.

---

## 9. Sorting, Selection, Visibility

### 9.1 Sorting

Sorting is orthogonal. Pinned columns may be sorted and may act as sort keys.

Sorting indicators are shown only if the column is visible.

### 9.2 Column Selection

Selection state persists regardless of pinning.

Pinned columns may be selected independently from scrollable columns.

### 9.3 Visibility

Visibility rules from SPEC-HAL-196 apply unchanged.

---

## 10. Required API Surface (Conceptual)

Consumers must be able to:

- Query pinned vs scrollable columns
- Pin/unpin by `ColumnId`
- Enumerate visible pinned columns (for layout)
- Enumerate visible scrollable columns (for layout)

---

## 11. Tests and Verification

Minimum required verification:

1. Pin a column: it remains fixed during horizontal scroll.
2. Unpin it: it scrolls again.
3. Resize pinned column: width changes, no scroll shift.
4. Hide pinned column: geometry updates, no ghost gaps.
5. Hit-test separators in pinned and scrollable domains correctly.
6. No separator appears between pinned and scrollable domains.
7. Single pinned column case behaves correctly.

---

## 12. Explicitly Forbidden

- Right-side pinning
- Dual-sided pinning
- Spreadsheet-style frozen panes
- Per-view ad-hoc pinning state
- Simulating pinning by duplicating columns or geometry

---

## 13. Notes for Future SPECs

Possible extensions (not covered here):

- Right-side pinning
- Cross-domain resize affordances
- Keyboard navigation rules across domains
- Animated pin/unpin transitions

Any extension must preserve INV-PIN-* invariants unless explicitly superseded.
