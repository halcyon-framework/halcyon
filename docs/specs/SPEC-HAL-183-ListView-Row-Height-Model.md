# SPEC-HAL-183: ListView Row Height Model

**Status:** Draft (Binding upon adoption)
**Layer:** TabularSubstrate Row System / Layout Consumer Law
**Scope:** ListView, TreeView projection over ListView, any virtualized row collection
**Parent:** SPEC-HAL-164 (TabularSubstrate — semantic engine authority)
**Depends on:** SPEC-HAL-164, SPEC-HAL-179, SPEC-HAL-167, SPEC-HAL-158, SPEC-HAL-141, SPEC-PM-001
**Non-Goals:** Text shaping, line breaking policy, per-cell auto-sizing, responsive typography

---

## 1. Purpose

This SPEC freezes the vertical sizing law for ListView rows.

Row height is the most common hidden authority leak:

* content-driven height negotiation,
* per-row height variance,
* text wrap expanding rows,
* "it's fine until scroll" virtualization instability.

This SPEC prohibits those classes of failure by defining a single lawful model.

---

## 2. Definition

A row has a vertical rect:

```
RowRect = { x, y, width, height }
```

This SPEC defines how height is determined and what is forbidden from influencing it.

---

## 3. Row Height Authority

**INV-ROW-H-AUTH-1 — TabularSubstrate Owns Row Y Geometry**

Row height is owned by TabularSubstrate (SPEC-HAL-164 semantic engine authority).

The Row System is TabularSubstrate's vertical row geometry infrastructure — analogous to how the Column System (SPEC-HAL-180) is TabularSubstrate's horizontal column geometry infrastructure. No parallel row authority exists.

Embedding components (ListView, TreeView) provide row height configuration inputs; they do not own or directly mutate authoritative row Y geometry.

* Widgets MAY NOT negotiate row height.
* Cells MAY NOT negotiate row height.
* Columns MAY NOT negotiate row height.

Row height is an input into row layout, not an output.

---

## 4. Row Height Mode (ListView)

**INV-ROW-H-MODE-1 — Fixed Height Only**

ListView rows MUST use a fixed height model.

Row height MUST be a single configuration value:

```
row_height_px: f32
```

It MUST apply uniformly to all rows in a ListView instance.

Per-row height variance is forbidden.

---

**INV-ROW-H-MODE-2 — Tree Projection Does Not Change Height**

Tree indentation, expansion state, and hierarchy MUST NOT affect row height.

TreeView projection may change:

* row contents
* indentation X
* disclosure affordances

It MUST NOT change Y sizing.

---

## 5. Content Containment Rules

**INV-ROW-H-CONTENT-1 — Content Clips or Truncates, Never Expands**

If content does not fit within the fixed row height:

* it MUST be clipped and/or truncated by the row/cell projection rules,
* it MUST NOT increase row height,
* it MUST NOT trigger reflow that changes height.

This applies to:

* text widgets (no wrap-expands-row behavior)
* embedded editors
* icons
* custom widgets

---

**INV-ROW-H-CONTENT-2 — Inline Editing Does Not Change Height**

Entering edit mode MUST NOT change row height.

Inline editing may swap bindings (SPEC-HAL-181) but the row height remains invariant.

---

## 6. Virtualization Law

**INV-ROW-H-VIRT-1 — Deterministic Y Mapping**

Given:

* `row_height_px`
* `first_row_y`
* `scroll_y_px`

The mapping from logical row index to Y position MUST be deterministic and stable:

```
row_y(i) = first_row_y + i * row_height_px - scroll_y_px
```

No per-row measurement pass is allowed.

---

**INV-ROW-H-VIRT-2 — Visibility Does Not Affect Height**

Realization / virtualization MUST NOT influence row height.

Rows being realized, destroyed, or recycled does not change `row_height_px`.

---

## 7. Layout Phase Law

**INV-ROW-H-PHASE-1 — Height Is Assigned During Layout Only**

Row height may be assigned/consumed only in the layout phase.

No post-layout mutation of row height is allowed.

This is enforced by SPEC-HAL-158 and SPEC-HAL-141.

---

## 8. Forbidden Patterns (Hard Law)

The following are architecturally illegal:

* per-row height measurement
* text wrap causing row height expansion
* cell content determining row height
* editor widgets requesting more height
* "auto row height" modes
* height changes triggered by scroll, selection, or focus
* caching row heights by visible rows

Any appearance of these patterns is an SPEC violation.

---

## 9. Acceptance Criteria

SPEC-HAL-183 is satisfied when:

* a ListView instance has exactly one `row_height_px`
* all rows use the same height regardless of content/state
* virtualization uses constant-step Y mapping
* entering edit mode does not change row height
* no code path exists where content affects row height
