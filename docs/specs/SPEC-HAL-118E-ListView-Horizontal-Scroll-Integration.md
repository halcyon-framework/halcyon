# SPEC-HAL-118E: ListView Horizontal Scroll Integration

**Status:** Deprecated
**Layer:** Scroll Integration / Viewport Wiring

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---
**Scope:** Horizontal scroll plumbing for ListView
**Depends on:** SPEC-HAL-118B, SPEC-HAL-118C, SPEC-HAL-118I, SPEC-HAL-118J, SPEC-HAL-118D
**Enables:** SPEC-HAL-118F, SPEC-HAL-118G, SPEC-HAL-118H, SPEC-HAL-118K
**Non-Goals:** Column resizing, scroll physics, gestures, snapping, momentum

---

## 1. Purpose

This SPEC defines **how horizontal scrolling is wired** for `ListView` when horizontal overflow exists.

It specifies:

* how horizontal scroll offset is applied
* how header/body/footer remain horizontally aligned
* how ListView cooperates with ScrollArea without owning scroll state

This SPEC introduces **no new behavior** — only integration.

---

## 2. Core Principle

**Horizontal scroll is applied once, at the viewport boundary.**

There must be exactly **one horizontal translation point** in the subtree.

---

## 3. Structural Model

### 3.1 Required Tree Structure

```
ListView
 ├── HeaderBand (painted by ListView)
 ├── BodyViewport (ScrollArea)
 │    └── List
 └── FooterBand (painted by ListView)
```

**INV-HX-STRUCT-1**

`ScrollArea` is the sole scroll authority for both axes.

---

## 4. Horizontal Scroll Offset Application

### 4.1 Translation Point

**INV-HX-APPLY-1**

Horizontal scroll offset MUST be applied:

* by `ScrollArea`
* before any child paint occurs
* uniformly to header, body, and footer

`ListView` MUST NOT apply or duplicate horizontal offset.

---

### 4.2 Offset Scope

The horizontal scroll offset affects:

* header column content
* body row rendering
* footer content

All three MUST remain horizontally aligned at all times.

---

## 5. ListView Responsibilities

### 5.1 Horizontal Metadata Exposure

**INV-HX-META-1**

`ListView` MUST expose:

* resolved horizontal content extent (`W_content`)
* viewport width (`W_viewport`)

This data is consumed by `ScrollArea`.

---

### 5.2 Forbidden Behavior

`ListView` MUST NOT:

* store horizontal scroll offset
* animate horizontal scroll
* clamp or normalize scroll values
* intercept wheel or gesture input

---

## 6. ScrollArea Responsibilities

### 6.1 Activation Rules

**INV-HX-SCROLL-AREA-1**

Horizontal scrolling is enabled if and only if:

* `W_content > W_viewport`
* horizontal scrolling is enabled on the ScrollArea

---

### 6.2 Scrollbar Visibility

**INV-HX-SCROLLBAR-1**

Horizontal scrollbar:

* appears only when overflow exists
* disappears immediately when overflow resolves

No hysteresis. No delayed hiding.

---

## 7. Clipping Guarantees

**INV-HX-CLIP-1**

When horizontal scrolling is active:

* clipping is applied at the ScrollArea viewport
* clipping applies equally to header, body, and footer

No band may render outside the clipped region.

---

## 8. Interaction With Vertical Scrolling

Horizontal and vertical scrolling:

* are orthogonal
* share the same ScrollArea
* must not interfere with each other

**INV-HX-VERT-1**

Vertical scrolling semantics from SPEC-HAL-118B remain unchanged.

---

## 9. Failure Modes (Explicitly Forbidden)

The following are architectural violations:

* applying horizontal offset in ListView paint()
* header scrolling independently of body
* footer lagging behind horizontal scroll
* multiple scroll offsets in the subtree
* conditional scroll application per band

Any occurrence is a hard invariant violation.

---

## 10. Acceptance Criteria

This SPEC is satisfied if:

* Horizontal scroll moves header, rows, and footer together
* Horizontal scroll disappears when columns fit
* Vertical scroll behavior remains unchanged
* Removing ListView leaves ScrollArea behavior unchanged
* No horizontal scroll state exists outside ScrollArea

---

## 11. Summary

SPEC-HAL-118E defines **how** horizontal scrolling is applied — not **when** and not **what** scrolls.

It locks down:

* one scroll authority
* one translation point
* one clipped viewport

Everything else is illegal.
