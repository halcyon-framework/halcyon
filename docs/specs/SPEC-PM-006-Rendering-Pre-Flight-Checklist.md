# SPEC-PM-006: Rendering Pre-Flight Checklist

### Mandatory process for any widget that paints overlapping visual elements

**Author:** JC
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Status:** Permanent Process Document (Binding)
**Applies to:** Every task involving `paint()`, `paint_overlay()`, scene paint
data, or renderer ordering
**Amended:** 2026-09-13 by SPEC-HAL-359

---

# 1. Purpose

This document prevents the recurring cross-pipeline ordering class. Halcyon now
executes strictly different Z values across pipelines under SPEC-HAL-359. Paint
authors still own explicit Z allocation, equal-Z discipline, correct primitive
selection, and intentional overlay scope.

---

# 2. Current Renderer Contract

Within one compositor surface:

1. all visible ordinary batches, paths, images, and text are executed by
   ascending `ZIndex` across pipeline families;
2. a strictly larger Z value composes later and therefore above an overlapping
   smaller-Z item under its admitted blend mode;
3. `z < LAYER_OVERLAY` belongs to the main pass and
   `z >= LAYER_OVERLAY` belongs to the overlay pass for every primitive family,
   including Path and Text; and
4. equal Z values across distinct clips, batch keys, pipelines, or primitive
   families have no semantic layering order.

The deterministic equal-Z implementation tie order exists only to make output
repeatable. It is not paint authority. SPEC-PM-007 therefore remains binding:
anything that must visibly sit above another overlapping item MUST use a
strictly larger Z value.

---

# 3. Rules

### RULE 1: Use the owner-correct primitive

Choose the primitive and pipeline that owns the required geometry/material:

| Required visual | Owner-correct operation |
|-----------------|-------------------------|
| Square fill/border | `push_rect`, `push_border_rect` |
| Rounded fill/border | rounded/SDF/path operation defined by its widget authority |
| Gradient | the admitted gradient paint operation |
| Image | `push_image` or target-bound image publication |
| Text | layout-owned text engine operation |
| Text selection/caret background | `TextBackgroundSpan` within that text run |

Do not change a visual into a different pipeline merely to work around ordering.

### RULE 2: Allocate strict Z for every real overlap relation

For every pair of overlapping items, state which must be above. Give that item
a strictly larger Z value. This applies even when both items use the same
pipeline and is mandatory when clips or pipelines differ.

### RULE 3: Equal Z does not mean insertion order

Do not depend on emission order, clip hash, texture identity, pipeline enum
value, or pipeline family for equal-Z layering. If order matters, change Z.

### RULE 4: Overlay is selected by each item's own Z

All visual elements of an overlay must use overlay-range Z values. Text at an
overlay-range Z executes in the overlay pass; it no longer needs a main-pass
workaround. An overlay blocker/background must remain below its content by
strict Z.

### RULE 5: Text backgrounds remain text-native

`TextBackgroundSpan` is for selection, caret, or another background that is
intrinsically part of one text run. It renders before the glyphs of that run.
Do not use it to rasterize or impersonate an unrelated panel solely to obtain
ordering.

---

# 4. Required Checklist

Before completing rendering work, answer:

```text
□ 1. Which admitted primitive owns each visual element?
□ 2. Which pairs can overlap, and which item must be above each pair?
□ 3. Does every required above/below relation use strictly different Z values?
□ 4. Do any equal-Z items cross clip states or pipelines?
     If yes, confirm that no visual order is required; otherwise allocate Z.
□ 5. Does every popup/overlay item use z >= LAYER_OVERLAY?
□ 6. Are TextBackgroundSpan values intrinsic to their text run rather than an
     ordering workaround?
□ 7. Does behavioral proof exercise the real DrawPlan -> Compile -> Execute
     path for every newly introduced cross-pipeline overlap?
```

---

# 5. Common Mistakes and Correct Fixes

### Mistake: same-Z background and foreground

```rust
scene.push_rect(background, bg, 0, None);
scene.push_rounded_rect(indicator, fg, radius, 0, None);
```

The pipeline choice is not the defect. The missing strict layer relation is.
Keep the owner-correct primitives and assign the indicator a larger Z value.

### Mistake: main-range text inside an overlay

```rust
scene.push_rect(popup, bg, LAYER_OVERLAY + 1, None);
text_engine.draw(label, LAYER_CONTENT);
```

The label belongs to main scope and is correctly below the popup. Use an
overlay-range Z for the label and a strictly larger value than its blocker.

### Mistake: using text quads as a generic panel

Do not encode a popup or widget surface as `TextBackgroundSpan` merely because
the content contains text. Paint the panel with its owner-correct primitive and
use strict Z.

### Mistake: assuming icon fill metadata creates a cutout

An inner polygon is not an eraser unless the active primitive's tessellation
actually consumes its contour and fill-rule semantics. The MAIL regression
showed unwanted filled ink from compound contours in the simple Path fill.
Author that icon with supported simple-contour layers and test real interior
pixels; do not infer an outline from nonempty bounds or total ink alone.

### Mistake: soft-clipping an internal joined edge

Rounded-clip inward antialiasing belongs to the exterior silhouette. A selected
ComboBox clipped its trigger at the popup junction and exposed the page through
two fading pixel rows despite identical opaque materials and absent borders.
Use the existing joined outer geometry for that clip while keeping primitives
and popup content bounded. Test junction rows, not only interior color samples.

A hard-clipped Path under soft-clipped row quads can still expose a wider side
silhouette. Cover each selected fill region once under the same exterior law;
check both lateral profiles and the full browser popup before geometry acceptance.

---

# 6. Enforcement

Rendering changes are incomplete unless their review identifies primitive
owners and overlapping Z relations. Any new cross-pipeline overlap requires a
behavioral test unless an existing owner-level test already exercises the exact
route. Source-string scans do not prove visible ordering.

---

# 7. History

Before SPEC-HAL-359, Execute grouped pipelines and forced all Path and Text work
after ordinary batches. At that time distinct Z values could not express
cross-pipeline order, so this document required single-pipeline visual units and
text-pipeline popup backgrounds as workarounds. Those workarounds are retired.

The historical incidents remain useful evidence: ColorPicker, RangeSlider,
TimePicker, and popup implementations repeatedly lost indicators, fills,
borders, or text because pipeline dispatch order overrode Z. SPEC-HAL-359 fixes
that renderer defect as a class. SPEC-PM-007's strict-Z/equal-Z discipline is
not retired because equal-Z clip and batch ties still carry no visual meaning.
