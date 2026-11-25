# SPEC-HAL-118P: ListView Band Height Resolution

**Status:** Deprecated
**Layer:** ListView / Header–Footer Bands

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---

**Depends on:**

* SPEC-HAL-118B – ListView Columnar Contract
* SPEC-HAL-118C – ListView Column Presentation

---

## 1. Purpose

This SPEC defines **how header and footer band heights are resolved**, how their internal content is laid out vertically, and what clipping guarantees apply.

Its goal is to prevent silent divergence in visual behavior as ListView gains more header functionality (sorting, resizing, pinning affordances).

---

## 2. Scope

This SPEC applies exclusively to:

* Header band
* Footer band

It does **not** affect body row height or virtualization rules.

---

## 3. Band Height Ownership

Header and footer band heights are:

* **Explicitly set by the host**
* Treated as **authoritative fixed values**

ListView MUST NOT auto-size bands based on content.

---

## 4. Height Resolution Rules

For each band:

* The band height is a single scalar value:

    * `header_height`
    * `footer_height`

If a band height is zero:

* The band is considered absent
* No rendering or hit-testing occurs

---

## 5. Vertical Padding Model

Padding inside header/footer bands is:

* Column-renderer responsibility
* Not interpreted or adjusted by ListView

ListView provides the full band rect to renderers.

---

## 6. Content Overflow

If renderer content exceeds band height:

* Content MUST be clipped to band bounds
* No auto-expansion occurs
* No overflow propagation is allowed

Overflow is a renderer error, not a layout signal.

---

## 7. Text Baseline and Alignment

ListView does NOT enforce:

* text baseline alignment
* vertical centering
* typography rules

These are delegated to:

* column header renderers
* footer renderers

---

## 8. Clipping Guarantees

ListView MUST enforce:

* Header rendering clipped strictly to header band rect
* Footer rendering clipped strictly to footer band rect
* Body rendering MUST NOT overdraw bands

This clipping is structural, not optional.

---

## 9. Interaction with Hit-Testing

Hit-testing for header/footer:

* Is constrained to band rects
* Uses resolved band height as bounds

No hit-testing may escape band bounds.

---

## 10. Interaction with Horizontal Scroll

Header and footer bands:

* Participate in horizontal translation
* Do NOT participate in vertical scroll

Band height is invariant under scroll.

---

## 11. Invariants

**INV-BAND-HEIGHT-1**
Band heights are fixed and host-owned.

**INV-BAND-HEIGHT-2**
Bands never auto-size to content.

**INV-BAND-HEIGHT-3**
All band rendering is clipped to band bounds.

**INV-BAND-HEIGHT-4**
Vertical padding is renderer-defined.

---

## 12. Rationale

Without explicit height resolution rules:

* Renderers start relying on undefined overflow behavior
* Header content becomes implicitly layout-driven
* Visual consistency degrades across themes

This SPEC locks the vertical contract early.

---

## 13. Summary

Header and footer bands are:

* Fixed-height
* Host-controlled
* Strictly clipped
* Renderer-managed internally

ListView provides structure, not styling magic.

---

## Implementation Notes (ListView v1)

Band heights (`header_height`, `footer_height`) are host-provided fixed scalars. Layout computes three non-overlapping rects (header, body, footer) with strict clipping. All band rendering is clipped to band bounds via `begin_clip()`/`end_clip()`.

---

## Implementation Reference

- `ListView::header_height` / `footer_height` — fixed band heights
- `ListView::frame_header_height` / `frame_body_height` — resolved heights
- `ListViewHeaderContainer::paint()` — header clipping
- `ListViewFooterContainer::paint()` — footer clipping
