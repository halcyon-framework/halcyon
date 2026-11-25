# SPEC-PM-011: Overlay Text Rendering Pipeline Mismatch

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Widget, Renderer
**Applies To:** Any widget with paint_overlay that renders text
**Amended:** 2026-09-13 — SPEC-HAL-359 and the explicit `TextEngine` Z boundary remove the hidden content-layer default while preserving the overlay producer's strict-layer obligation.

---

## 0. Pathology Name

**Overlay Text Rendering Pipeline Mismatch**

Also known as:
- "Text invisible in popup"
- "Labels don't show in overlay"
- "cx.text_engine.draw() doesn't work in overlay"

---

## 1. Summary of the Failure

Historically, widgets rendering text inside `paint_overlay()` through
`cx.text_engine.draw()` produced invisible text because the engine silently
emitted every run at z=0, behind the overlay background blocker. The text was
painted but occluded.

The current `TextEngine` contract requires an explicit producer-owned
`ZIndex`. The remaining pathology is a producer choosing a main-range value or
a value that is not strictly above its overlapping overlay blocker.

This failure appeared in ColorPickerField's OK/Cancel
button labels, which were invisible despite correct
positioning and colors.

---

## 2. Root Cause

The old standard text path erased the caller's layer choice and emitted at a
hidden content value. That API defect is removed by SPEC-HAL-359. Overlay
owners still own the complete local ladder: surface, blocker, text, border,
and foreground chrome must each use explicit strict values where they overlap.

---

## 3. Prevention Rules

**RULE-PM-011-1: All overlay text MUST use an explicit overlay-range Z.**

`TextEngine::draw*` is valid inside `paint_overlay()` only with an explicit
overlay-range value selected by that overlay owner:
```rust
cx.text_engine.draw(&run, x, y, color, overlay_text_z, cx.scene);
```

Direct `PaintSink::push_text_with_backgrounds` remains valid when the owner
needs text-native backgrounds; it has the same explicit-Z obligation.

**RULE-PM-011-2: Overlay text Z MUST be strictly above every overlapping
blocker.**

There is no universal `LAYER_OVERLAY + 2` text value. The overlay owner names
its local ladder, and strictly different Z values are executable across
pipeline families under SPEC-HAL-359. Equal-Z ties remain non-semantic.

**RULE-PM-011-3: A popup blocker MUST be overlay-range and strictly below the
text it covers.**

The exact offset belongs to the popup's declared local ladder; neither call
order nor pipeline family supplies the relation.

---

## 4. Affected Systems

- ColorPickerField popup buttons
- DatePickerField popup buttons (future)
- Any future overlay popup with text labels

---

## 5. Forbidden Patterns

FORBID-PM-011-1: `cx.text_engine.draw*()` inside `paint_overlay()` with a
main-range, implicit, or backend-selected Z value.

FORBID-PM-011-2: Overlay text at a Z value equal to or below an overlapping
background blocker.
