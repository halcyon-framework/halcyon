# SPEC-HAL-186: ListView Header / Body / Footer Scroll Partition Law

**Status:** Draft (Binding upon adoption)
**Layer:** Core Widget / Layout Subsystems
**Scope:** ListView
**Depends on:** SPEC-HAL-158, SPEC-HAL-168, SPEC-HAL-179, SPEC-HAL-179A, SPEC-HAL-180
**Non-Goals:** Rendering style, visuals, interaction semantics beyond scroll and layout law

---

## 1. Purpose

This SPEC defines the structural and scrolling partition of ListView into Header, Body, and optional Footer domains.

Its purpose is to permanently eliminate ambiguity, hacks, and accidental coupling between scrolling, column geometry, and layout responsibilities.

This SPEC is foundational. Any ListView implementation that violates it is invalid by definition.

---

## 2. Scope

This SPEC governs:

* Spatial ownership of Header, Body, Footer
* Vertical and horizontal scroll participation
* Interaction with the Column System
* Interaction with the TabularSubstrate
* Forbidden architectural patterns

This SPEC does not define rendering style, visuals, or interaction semantics beyond scroll and layout law.

---

## 3. Structural Model

A ListView instance is structurally composed of three disjoint layout domains:

```
┌───────────────────────────────┐
│ Header Domain (optional)      │  ← non-scrolling (Y)
├───────────────────────────────┤
│ Body Domain (mandatory)       │  ← vertically scrolling
├───────────────────────────────┤
│ Footer Domain (optional)      │  ← non-scrolling (Y)
└───────────────────────────────┘
```

Each domain is a separate widget subtree with its own layout root.

---

## 4. Space Ownership Invariants

**INV-PART-SPACE-1 — Header/Footer Fixed Space**

Header and Footer each own fixed vertical space in the ListView layout.

**INV-PART-SPACE-2 — Body Remainder Space**

The Body domain owns all remaining vertical space after Header/Footer layout.

**INV-PART-SPACE-3 — Domain Isolation**

Header and Footer do not participate in Body vertical layout, clipping, or virtualization.

---

## 5. Vertical Scroll Law

**INV-PART-VSCROLL-1 — Body-Only Vertical Scroll**

Only the Body domain participates in vertical scrolling.

**INV-PART-VSCROLL-2 — Header/Footer Vertical Immunity**

Header and Footer MUST NOT be translated, offset, clipped, or otherwise affected by vertical scroll state.

**INV-PART-VSCROLL-3 — No Scroll State Leakage**

Vertical scroll state MUST NOT be read, inferred, or compensated for inside Header or Footer code.

---

## 6. Horizontal Scroll Law

**INV-PART-HSCROLL-1 — Shared Horizontal Axis**

Header and Body share a single horizontal axis.

**INV-PART-HSCROLL-2 — TabularSubstrate Propagation**

Horizontal scroll offset is propagated exclusively by TabularSubstrate as a post-geometry transform (SPEC-HAL-168).

**INV-PART-HSCROLL-3 — No Independent Horizontal State**

Header widgets MUST NOT maintain their own horizontal scroll state.

Footer horizontal participation is implementation-defined but must obey the same rules if enabled.

**INV-PART-HSCROLL-4 — Atomic Presented Projection**

The ScrollArea MUST publish its presented horizontal coordinate before retained composite updates. A changed coordinate
invalidates Header, Body, Footer, and their clip/hit projections as one bounded paint subtree in the same presentation
boundary. Horizontal scrolling MUST NOT require layout when column geometry and viewport dimensions are unchanged.

---

## 7. Column System Interaction

**INV-PART-COL-1 — Single Geometry Authority**

Column geometry (x-offsets, widths) is resolved once by TabularSubstrate and is authoritative.

**INV-PART-COL-2 — Shared Geometry Consumption**

Header and Body consume the same resolved column geometry table.

**INV-PART-COL-3 — No Independent Geometry**

No domain may cache, recompute, or infer column geometry independently.

This guarantees perfect column alignment without scroll hacks.

---

## 8. Interaction Semantics

**INV-PART-INTENT-1 — Header Intent Origin**

Header is the exclusive origin for column-level interaction intents:

* resize
* reorder
* sort

**INV-PART-INTENT-2 — Intent-Only Emission**

Header emits intents only. It never mutates geometry, order, or state directly.

---

## 9. Forbidden Patterns (Hard Law)

The following are explicitly forbidden:

* Vertically scrolling the Header or Footer
* Translating Header by -scroll_y
* Clipping Header to Body viewport
* Header reading Body scroll position
* Header owning or mutating column widths
* Independent horizontal scroll state per domain
* "Sticky header" implementations based on scroll compensation
* Geometry duplication between Header and Body

Violations invalidate the ListView implementation.

---

## 10. Authority Summary

| State / Concern               | Authority            |
|-------------------------------|----------------------|
| Column Model                  | TabularSubstrate     |
| Resolved Column Geometry      | TabularSubstrate     |
| Horizontal Scroll Propagation | TabularSubstrate     |
| Vertical Scroll State         | Body Domain only     |
| Header / Footer Layout        | ListView Composition |
| Column Interaction Intents    | Header Domain        |

---

## 11. Acceptance Criteria

This SPEC is satisfied when:

* Header and Footer remain visually stationary during vertical scrolling
* Column alignment remains exact under horizontal scroll
* Horizontal scrolling exposes row cells beyond the previous viewport without resize or layout invalidation
* Pinned Header/Body/Footer content and the right-edge header affordance remain fixed while the scrollable domain moves
* No header code references vertical scroll state
* No geometry duplication exists between domains
* All column interactions flow through intent emission only
