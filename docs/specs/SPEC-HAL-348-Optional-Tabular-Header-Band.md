# SPEC-HAL-348: Optional Tabular Header Band

**Status:** Active / Binding
**Layer:** ListView and TreeView Consumer API / Composite Presentation
**Scope:** Header-band presence only
**Depends on:** SPEC-HAL-002, SPEC-HAL-179, SPEC-HAL-186, SPEC-HAL-192, SPEC-HAL-193, SPEC-HAL-262
**Narrowly amends:** SPEC-HAL-179 §3.3 and §8; SPEC-HAL-193 §3; SPEC-HAL-262 §§3 and 10

---

## 1. Purpose

ListView and TreeView remain closed tabular composites, but a header band is redundant for some single-column navigation and dense list surfaces. Consumers need a supported way to omit that presentation band without reconstructing body geometry or bypassing the owning composite.

This SPEC admits header visibility as a presentation configuration while preserving the complete composite, column engine, body viewport, scrolling, interaction, selection, focus, accessibility, copy, and natural-height authority.

---

## 2. Admitted Public API

ListView and TreeView may expose:

```rust
pub fn set_header_visible(&mut self, visible: bool);
pub fn header_visible(&self) -> bool;
```

The default is `true`.

`set_header_visible(false)` makes the header band absent. It does not remove, replace, or expose an internal widget and does not alter column configuration.

---

## 3. Layout Contract

When the header is visible, the composite uses its owner-defined header height.

When the header is hidden:

1. resolved header height is zero;
2. the body viewport begins at the composite's top edge;
3. the body receives the height previously occupied by the header;
4. the footer remains at the bottom of the composite;
5. ListView natural height excludes the header band;
6. no header paint, overlay paint, hit-testing, sorting, reordering, configuration affordance, or header-only accessibility surface is active.

Body and footer behavior remains unchanged.

---

## 4. Ownership Invariants

- **INV-348-CLOSED-1:** ListView and TreeView remain closed composites. Header visibility is configuration, not an extension point.
- **INV-348-GEOMETRY-1:** The composite alone resolves the zero-height header and body origin. Consumers must not reconstruct band geometry.
- **INV-348-COLUMN-1:** Hiding the header does not mutate column order, width, visibility, pinning, or scroll-domain state.
- **INV-348-BODY-1:** Body selection, focus, activation, expansion, scrolling, virtualization, copy, and accessibility behavior is unchanged.
- **INV-348-NATURAL-1:** Owner-authored natural-height queries use the resolved header height.
- **INV-348-PARITY-1:** ListView and TreeView expose identical header-visibility semantics.

---

## 5. Forbidden Behaviors

- Mounting body widgets without the owning ListView or TreeView to avoid header layout.
- Consumer coordinate offsets, hit-testing, or viewport reconstruction.
- Removing column metadata when the header is hidden.
- Treating an empty header title as equivalent to an absent header band.
- Adding a custom header delegate, renderer, child, or pluggable header surface.

---

## 6. Verification

Mechanical tests must prove:

1. the default header remains visible;
2. hiding the header moves the body origin to zero and grants it the full non-footer height;
3. ListView natural height excludes the hidden header;
4. TreeView and ListView retain their normal body and footer children;
5. restoring visibility restores the owner-defined header geometry.

