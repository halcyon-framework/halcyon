# SPEC-HAL-096D — Scrollbar Hit Expansion Geometry

**Status:** Draft
**Layer:** Theme / Geometry Authority
**Scope:** Interaction Geometry (Hit-Testing Only)
**Related SPECs:** SPEC-HAL-096B, SPEC-HAL-096C
**Affects:** Scrollbar (V/H)
**Does NOT affect:** Layout, Paint, Visual Geometry, Scroll Math

---

## 1. Problem Statement

Scrollbars require an interaction-only geometry margin to ensure reliable pointer targeting, especially for thin visual designs.

Currently, scrollbar hit expansion is implemented as a **hardcoded literal** inside widget code. This creates:

* an unowned geometry constant
* an authority violation (literal vs theme)
* inconsistency with the rest of scrollbar geometry (thickness, radius, min thumb), which are already theme-backed

This SPEC formalizes scrollbar hit expansion as a **theme-owned interaction geometry token**, without altering any existing behavior.

---

## 2. Definitions

**Hit Expansion**
An invisible padding applied **only** to pointer hit-testing regions of the scrollbar track and thumb.

Hit expansion:

* increases hit-test rect size
* does **not** affect layout size
* does **not** affect visual rendering
* does **not** affect scroll math or content extents

---

## 3. New Theme Token

### 3.1 Theme Structure Extension

Add one field to existing geometry spec:

```
ScrollbarGeometrySpec {
    thickness: f32,
    min_thumb_size: f32,
    corner_radius: f32,
    hit_expansion: f32,
}
```

### 3.2 Default Value

```
hit_expansion = 6.0
```

This value MUST exactly match the current literal used in widget code.

---

## 4. Authority Rules

### INV-SCROLLBAR-HIT-1

Scrollbar hit expansion is **theme-authoritative**.

* Widgets MUST NOT hardcode hit expansion values
* Widgets MUST consume the value via theme-backed runtime sync

### INV-SCROLLBAR-HIT-2

Hit expansion MUST be applied **only** to pointer hit-testing.

It MUST NOT:

* affect layout constraints
* affect visual bounds
* affect scrollbar visibility decisions
* affect scroll offsets or math

---

## 5. Sync & Consumption Model

### Canonical Sync Point

* Theme → config sync MUST occur in `layout()`
* Same sync location and timing guarantees as:

    * thickness
    * min thumb size
    * corner radius

### Consumption Rules

* Event handling MAY read only from cached/config values
* Event handling MUST NOT read theme directly

---

## 6. Non-Goals (Explicitly Forbidden)

This SPEC does **not** introduce:

* density-based scaling
* adaptive hit expansion
* coupling to scrollbar thickness
* animation or hover behavior changes
* reuse of spacing or padding tokens
* automatic accessibility heuristics

Any such behavior requires a separate SPEC.

---

## 7. Backward Compatibility

* Default behavior remains **bit-identical**
* No visual or interaction regression permitted
* Existing scroll behavior and feel MUST remain unchanged

---

## 8. Migration Plan (Deferred)

Implementation will:

1. Add `hit_expansion` to `ScrollbarGeometrySpec`
2. Sync it alongside existing scrollbar geometry in layout
3. Replace the existing literal at hit-test sites

**Implementation is out of scope for this SPEC.**

---

## 9. Summary

This SPEC:

* closes the last unowned scrollbar geometry literal
* preserves all existing behavior
* maintains strict separation between visual and interaction geometry
* aligns scrollbar hit-testing with the established theme authority model

Nothing more. Nothing less.
