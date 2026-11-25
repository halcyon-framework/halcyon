# SPEC-HAL-167: TabularSubstrate — Body Domain and Row Projection

**Status:** Implemented / Closed
**Layer:** Structural Domains / Virtualization Projection  
**Audience:** Engine, Layout, Virtualization  
**Depends on:**  
SPEC-HAL-008, SPEC-HAL-024, SPEC-HAL-037, SPEC-HAL-086, SPEC-HAL-131A/B/C, SPEC-HAL-141, SPEC-HAL-145, SPEC-HAL-146,
SPEC-HAL-158  
**Parent:** SPEC-HAL-164  
**Consumes:** SPEC-HAL-165, SPEC-HAL-166

---

## 1. Purpose

This SPEC defines the **Body Domain** and the **Row Projection** mechanism for TabularSubstrate.

It exists to enforce:

- rows as first-class structural units
- strict separation between geometry resolution and data projection
- virtualization without geometry authority bleed

---

## 2. Body Domain Definition

The Body Domain is the **scrollable structural region** containing row projections.

It owns:

- row realization lifecycle
- vertical scrolling participation
- row hit-testing routing

It does **not** own:

- column geometry
- header interaction
- selection policy
- sorting state

---

## 3. Structural Placement

**INVARIANT-1:**  
The Body Domain is a sibling of the Header Domain under TabularSubstrate.

**INVARIANT-2:**  
The Body Domain consumes resolved column geometry unchanged.

Vertical extent is governed by the scroll substrate.

---

## 4. Row Projection Model

A **Row** is a projection unit, not a widget container.

Each realized row has:

- `row_id` (stable, opaque)
- `row_index` (frame-local)
- `y_offset`
- `height`

Row height resolution may be fixed or variable but is vertically scoped only.

---

## 5. Cell Projection Rules

For each realized row:

- exactly one cell is projected per **visible column**
- cell width == resolved column width
- cell x-offset == resolved column x-offset
- cell y-offset == row y-offset

Cells may not span columns or rows.

---

## 6. Identity and Stability

Row identity derives from:

```

(TabularSubstrateId, RowId)

```

Cell identity derives from:

```

(TabularSubstrateId, RowId, ColumnId)

```

**INVARIANT-3:**  
Row and cell identity are stable across:

- scrolling
- virtualization
- column visibility changes

Index-based identity is forbidden.

---

## 7. Virtualization Contract

The Body Domain integrates with the virtualization substrate.

**INVARIANT-4:**  
Only the virtualization layer may decide which rows are realized.

**INVARIANT-5:**  
Geometry for unrealized rows must never be queried.

Row realization order must not affect geometry resolution.

---

## 8. Interaction Routing

Pointer events entering body bounds are routed to the Body Domain.

**INVARIANT-6:**  
Body interactions may emit intent directives but must not mutate:

- column geometry
- scroll state
- selection state

Those are resolved upstream.

---

## 9. Rendering Constraints

Body rendering is structurally isolated.

Forbidden:

- reading header state
- mutating column geometry
- resolving layout during paint

---

## 10. Forbidden Behaviors

Explicitly illegal:

- inferring column widths from cell content
- per-row column overrides
- row-driven layout invalidation
- embedding headers or separators as rows
- coupling row lifetime to widget tree lifetime

---

## 11. Scope Boundaries

This SPEC does **not** define:

- selection semantics
- inline editors
- row grouping
- tree indentation
- horizontal scrolling

Those are layered later.

---

## 12. Relationship to Deprecated SPECs

This SPEC replaces body/row intent from:

- SPEC-HAL-118
- SPEC-HAL-118B
- SPEC-HAL-118P

No behavior is inherited.

---

## 13. Next SPEC

SPEC-HAL-168 will define:

**Horizontal Geometry Resolution and Scroll Coupling**

---

## 14. Closure

**This SPEC is fully implemented and closed.** Implementation: `src/widgets/tabular_view/body.rs` (BodyDomain, RowProjection, CellProjection), `row.rs` (RowId, RowModel, CellId). All invariants (INVARIANT-1 through INVARIANT-6) enforced by structural domain separation, identity-based row/cell projection, and virtualization lifecycle. No further work permitted under this SPEC.

**Proposed relationship to SPEC-HAL-363/364 (2026-10-05):** The paired
documents remain Proposed pending joint adoption. This annotation changes no
declared status, implemented scope or substantive law in this document.

SPEC-HAL-363 §10 and SPEC-HAL-364 §§2–3/12 govern only a new owned immutable
paint-output handoff from an already-realized producer. They do not extend the
lifetime of RowProjection, frame-local derived data, layout borrows or a row
model, and do not reopen this closed semantic scope. Source provenance uses
existing substrate/RowId/ColumnId identities and their owner lifetime, never
indices or recycled slots. No per-row WidgetId, offscreen geometry query,
new realization policy or column/header/body/footer authority is introduced.
