# SPEC-HAL-092A — Optical Baseline Adjustment Contract

**Status:** Binding  
**Layer:** Typography / Text Placement  
**Amends:** SPEC-HAL-092 (Unified Typography Rules)  
**Applies to:** All control text rendered inside bounded UI geometry (buttons, list items, inputs, headers)  
**Scope:** Cross-platform baseline placement correctness

---

## 1. Problem Statement

SPEC-HAL-092 defines baseline-driven text placement using font metrics (ascent, descent) and geometric centering.  
This is **necessary but not sufficient** to guarantee *visually correct* placement across platforms.

Empirical evidence shows that identical metric-based placement produces **systematic vertical bias** depending on:

- OS font rasterizer (DirectWrite, CoreText, FreeType)
- System UI font (Segoe UI, SF Pro, Noto)
- Glyph rasterization strategy (hinting, subpixel positioning)
- DPI scaling and physical pixel snapping behavior

Result:
- On some platforms (notably macOS), control text appears **visibly lower** inside the same geometry
- The offset is **larger than rounding error** (multiple pixels)
- The issue persists even when baseline math is strictly correct

This violates the *user-perceived alignment invariant* even though metric invariants are satisfied.

---

## 2. Core Insight

**Baseline correctness ≠ visual centering correctness**

Font metrics describe glyph geometry, not optical balance.  
Different platforms encode different *implicit optical biases* into their system fonts and rasterizers.

Therefore:

> A purely metric-based baseline placement is insufficient to guarantee cross-platform visual parity.

---

## 3. New Authority Introduced

This SPEC introduces a **controlled optical correction layer** that operates *after* metric-based placement.

This correction is:
- Explicit
- Bounded
- Platform-scoped
- Declarative (not heuristic-at-callsite)

---

## 4. Binding Invariants

### INV-TYPO-OPT-1 — Baseline Authority Preservation

The **baseline remains the sole semantic anchor** for text rendering.

Optical adjustment:
- MUST NOT redefine ascent, descent, or baseline meaning
- MUST be applied as a final additive offset
- MUST NOT affect text layout, hit-testing, or selection geometry

---

### INV-TYPO-OPT-2 — Platform-Scoped Optical Offset

An **optical baseline adjustment** MAY be applied per platform backend.

- The adjustment is a **small vertical offset**
- Expressed in **logical pixels**
- Applied uniformly to all control text on that platform

---

### INV-TYPO-OPT-3 — No Per-Widget or Per-Font Heuristics

Platform optical adjustment:
- MUST NOT vary per widget
- MUST NOT vary per font
- MUST NOT be computed dynamically from glyph metrics
- MUST NOT depend on content

This prevents visual drift and layout instability.

---

### INV-TYPO-OPT-4 — Explicit Declaration

All optical baseline adjustments MUST be:
- Explicitly declared
- Centralized
- Auditable

No inline “magic numbers” are permitted in rendering code.

---

## 5. Adjustment Model

Each platform backend MAY declare:

```rust
optical_baseline_adjust_y: f32
```


Semantics:
- Positive value moves text **up**
- Negative value moves text **down**
- Applied after metric-based baseline resolution

Example (conceptual):

```rust
final_baseline_y = metric_baseline_y + optical_baseline_adjust_y
```


---

### 5.1 Explicit Product Caption Placement

On 2026-10-15 JC authorized configurable Button caption placement, with no
product-specific hardcoded correction in Halcyon. This is an authored design
input, separate from the uniform platform optical correction above.

Button accepts a finite signed `text_offset_y` in logical pixels, defaulting to
zero. Negative moves its caption up; positive moves it down. The value is added
after the existing metric-based baseline and platform/theme correction. It
applies equally to standalone and child-backed captions, without changing font
metrics, shaping, layout size, surface, hit bounds, focus or Capture. Invalid
non-finite input is rejected without replacing the previous value. Products
own the declaration; Halcyon does not infer it from a font, caption or platform.

Construction uses `with_text_offset_y`; update-time replacement uses
`set_text_offset_y` and invalidates only the affected retained paint. Equal
values are a no-op. Existing callers retain the zero-offset behavior.

## 6. Expected Magnitude

This SPEC does **not** mandate a specific value.

Observed ranges (informative, non-binding):
- 0.0 px (Windows / DirectWrite)
- ~ -1.0 to -3.0 px (macOS / CoreText)
- TBD (Linux / FreeType, X11 vs Wayland)

Exact values are platform-validated, not assumed.

---

## 7. Cross-Platform Parity Rule

### INV-TYPO-OPT-5 — Visual Parity over Metric Purity

When metric correctness and visual parity conflict:

> **Visual parity inside bounded controls takes precedence.**

This applies **only** to control text, not free-flowing text layout.

---

## 8. Non-Goals

This SPEC explicitly does NOT:
- Change text layout algorithms
- Modify glyph shaping
- Affect text measurement APIs
- Introduce font-specific tuning
- Guarantee identical pixel output across platforms

The goal is *consistent perceived alignment*, not identical raster output.

---

## 9. Relationship to Existing SPECs

- **SPEC-HAL-092** remains authoritative for typography rules
- **SPEC-HAL-104** governs pixel stability, not optical balance
- **SPEC-HAL-106** consumes the result, does not define placement
- This SPEC refines, not replaces, baseline semantics

---

## 10. Compliance Checklist

A platform backend is compliant if:
- Baseline placement remains metric-driven
- Any optical adjustment is centralized and constant
- Control text appears visually centered in standard UI components
- No widget-level or content-level hacks exist

---

## 11. Status

This amendment is **binding upon adoption**.

All control text rendering paths MUST comply.
