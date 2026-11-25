# SPEC-HAL-350: Readout Typography

**Status:** Active / Binding
**Layer:** Visual Primitive / Dense Readouts
**Scope:** `StatusGrid` and `KeyValueStrip` label/value typography
**Depends on:** SPEC-HAL-002, SPEC-HAL-092, SPEC-HAL-096, SPEC-HAL-096B, SPEC-HAL-332

---

## 1. Purpose

Dense readouts need field labels that remain subordinate to their values without becoming illegible metadata. `StatusGrid` and `KeyValueStrip` are the owners of this label/value presentation. Consumers must not reconstruct their geometry or wrap each field in separately styled text widgets.

This SPEC establishes one shared Halcyon-owned typography rule for both primitives.

---

## 2. Typography Model

Readout typography derives from the active theme's base font size:

- field labels use `(base_font_size - 2).max(10).round()`;
- field values use `(base_font_size - 1).max(11).round()`.

Under Halcyon's canonical 13px compact typography, labels resolve to 11px and values resolve to 12px.

Field labels use the theme's muted typography color. Field values use the theme's primary typography color. This preserves the status/inspector hierarchy required by `SPEC-HAL-332`: labels remain visually subordinate through both size and semantic color, while values remain authoritative.

---

## 3. Invariants

- **INV-350-OWNER-1:** `StatusGrid` and `KeyValueStrip` consume the same owner-defined label/value typography rule.
- **INV-350-THEME-1:** Label and value sizes derive from the active theme typography baseline; consumers do not provide raw sizes.
- **INV-350-HIERARCHY-1:** Labels remain smaller than values and use muted typography; values use primary typography.
- **INV-350-SELECTION-1:** Paint and read-only text-selection hit regions use the same resolved font sizes.
- **INV-350-GEOMETRY-1:** Typography correction does not transfer readout geometry, hit-testing, or paint ownership to consumers.
- **INV-350-CLIP-1:** Readout label clip regions must be at least the active label font metrics line height, and any owner-side label clip adjustment must preserve alignment between paint and read-only text-selection hit regions.

---

## 4. Forbidden Behaviors

- Adding per-consumer raw font-size or color knobs to readout items.
- Reconstructing `StatusGrid` or `KeyValueStrip` field geometry in a consumer to restyle labels.
- Using disabled or placeholder state to encode ordinary field-label hierarchy.
- Resolving paint and selection text with different font sizes.
- Making label typography application-domain-specific.

---

## 5. Verification

Mechanical tests must prove:

1. `StatusGrid` and `KeyValueStrip` paint field labels with the same theme-derived size and muted color;
2. both primitives paint values with the same theme-derived size and primary color;
3. label size remains below value size;
4. `StatusGrid` label clip regions cover the active label font metrics line height;
5. existing finite-layout, tooltip, tone, overflow, and read-only selection behavior remains intact.
