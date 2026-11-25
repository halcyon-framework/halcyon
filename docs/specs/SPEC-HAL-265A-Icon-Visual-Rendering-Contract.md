# SPEC-HAL-265A — Icon Visual Rendering Contract

### *Grid Discipline, Layer Semantics, and Cross-Size Rendering Guarantees*

**Author:** JC
**Status:** Proposed
**Date:** 2026-02-28
**Depends on:** SPEC-HAL-265 (Iconography System), SPEC-HAL-095 (Iconography Rules & Pixel Grid), SPEC-HAL-264 (Vector Mesh Pipeline), SPEC-HAL-096 (Color System), SPEC-HAL-104 (Pixel-Stable Text Metrics), SPEC-HAL-092 (Unified Typography Rules)
**Companion to:** SPEC-HAL-265 (this SPEC defines the visual rendering discipline for the icon system 265 defines)
**Supersedes:** Nothing (additive)

---

# 1. Purpose

SPEC-HAL-265 defines the **structural architecture** of the icon system: identity, registry, scene integration, scaling contract.

SPEC-HAL-095 defines **placement rules**: size tokens, stroke weights, alignment formulas, DPI behavior.

Neither defines the **visual rendering contract** — the rules that govern how an icon's geometry, layers, and colors must behave to produce correct, consistent visual output across all rendering contexts.

This SPEC closes that gap. It defines:

* Grid coordinate discipline and enforcement
* Multi-layer rendering semantics and color math
* Cross-size rendering guarantees
* Optical weight calibration against typography
* Complexity limits for icon definitions
* Pixel-snapping rules at render time
* Validation requirements for icon compliance

This is the contract that ensures 13 icons or 300 icons look like they belong to the same system, at every size, on every display.

---

# 2. Scope

This SPEC governs:

* All `IconDefinition` instances registered in `IconRegistry`
* All path geometry authored for the Halcyon icon set
* The `push_icon` rendering path in `SceneBuilder` / `PaintSink`
* Layer role color resolution
* Size-dependent rendering behavior

This SPEC does NOT govern:

* Procedural icons (widget-local, per SPEC-HAL-265 §14.3)
* Raster icon fallbacks (per SPEC-HAL-265 §13)
* Tessellation or mesh caching internals (SPEC-HAL-264)
* Widget-level icon selection or placement (SPEC-HAL-095 §7)

---

# 3. Grid Coordinate Discipline

## 3.1 Canonical Viewbox

All standard Halcyon icons use a **24×24 viewbox**. This is the authoring space for all path coordinates.

The viewbox is a coordinate grid, not a pixel grid. Coordinates in viewbox space map to physical pixels through the scaling pipeline (§7).

## 3.2 Coordinate Precision Rules

**RULE-GRID-001 — Integer or Half-Grid Coordinates**

All path coordinates (MoveTo, LineTo, polygon vertices) MUST use values that are integers or exact halves (N.0 or N.5) within the 24×24 viewbox.

Allowed: `2.0`, `11.5`, `20.0`, `6.5`
Forbidden: `3.7`, `12.333`, `8.14159`, `17.25`

Rationale: At common render sizes (12px, 14px, 16px, 18px), integer and half-grid viewbox coordinates produce pixel-aligned or half-pixel coordinates after uniform scaling. Arbitrary precision introduces subpixel drift that varies unpredictably across sizes.

**RULE-GRID-002 — No Embedded Transforms**

Icon path data MUST NOT contain embedded transforms (translate, rotate, scale, skew). All geometry is expressed directly in viewbox coordinates. Transform application is the responsibility of `push_icon` at render time.

**RULE-GRID-003 — Coordinate Bounds**

All path coordinates MUST fall within `[0.0, viewbox_width] × [0.0, viewbox_height]`. No negative coordinates. No coordinates exceeding the viewbox boundary.

Exception: Stroke overshoot of up to 1.0 viewbox unit beyond boundaries is permitted for visual edge effects, but the path's fill geometry must remain within bounds.

## 3.3 Line Width Precision

Line widths specified via `IconBuilder::line()` MUST be values from the set: `{0.5, 0.6, 0.75, 1.0, 1.25, 1.5, 2.0}`.

Arbitrary line widths (e.g., `0.847`) are forbidden. This constraint ensures consistent stroke appearance after scaling.

---

# 4. Multi-Layer Rendering Model

## 4.1 Layer Roles

Every `IconLayer` carries a `LayerRole` that determines its color derivation from the widget-provided base color. The current role set:

| Role | Purpose | Paint Order |
|------|---------|-------------|
| `Shadow` | Back panel, depth cue, darker tone | First (behind) |
| `Base` | Primary visible surface of the icon | Middle |
| `Highlight` | Accent surface, lighter tone, reflective edge | After Base |

Additional roles defined but reserved for future use:

| Role | Purpose |
|------|---------|
| `Outline` | Stroke border (currently unused — see §4.5) |

## 4.2 Color Resolution Math

Given a widget-provided base color `C = (r, g, b, a)` in sRGB space, layer roles resolve as follows:

**Shadow:**
```
C_shadow = (r × 0.65, g × 0.65, b × 0.65, a)
```
Multiplies each RGB channel by 0.65. Produces a darkened variant suitable for back panels and depth cues.

**Base:**
```
C_base = (r, g, b, a)
```
Pass-through. The base color is used unmodified.

**Highlight:**
```
C_highlight = lerp(C, WHITE, 0.35)
           = (r + (1.0 - r) × 0.35, g + (1.0 - g) × 0.35, b + (1.0 - b) × 0.35, a)
```
Lerps 35% toward white. Produces a lightened variant suitable for tabs, fold faces, and reflective edges.

**Outline (reserved):**
```
C_outline = (r × 0.45, g × 0.45, b × 0.45, a)
```
Darker than Shadow. Reserved for stroke borders if reintroduced.

## 4.3 Paint Order

Layers paint in array order: `layers[0]` first (backmost), `layers[N-1]` last (frontmost). All layers within a single icon share the same z-index. Visual depth is achieved through:

1. Geometric overlap (back panel extends above front face)
2. Color differentiation (Shadow darker than Base)
3. Paint order (later layers cover earlier layers where they overlap)

## 4.4 Layer Count Constraints

**RULE-LAYER-001 — Minimum 1 Layer**

Every `IconDefinition` MUST have at least 1 layer. Single-layer icons use `LayerRole::Base`.

**RULE-LAYER-002 — Maximum 5 Layers**

No icon may exceed 5 layers. If an icon requires more than 5 layers to read clearly, the geometry is too complex for the target render sizes and must be simplified.

**RULE-LAYER-003 — No Duplicate Roles (Soft)**

Within a single icon, each `LayerRole` SHOULD appear at most once. Multiple layers with the same role are permitted only when geometrically non-overlapping (e.g., two separate Shadow regions).

## 4.5 Outline Layer Policy

Filled-polygon outline layers that trace the full icon silhouette are **forbidden**. A filled outline painted last at the same z-index will completely obscure all interior layers, destroying the multi-tone depth effect.

If border/outline rendering is needed in the future, it MUST be implemented as:

* Stroke rendering (unfilled path with stroke width), OR
* A separate render pass at a different z-index, OR
* A post-process effect

It MUST NOT be a filled polygon at the same z-index as interior layers.

---

# 5. Cross-Size Rendering Contract

Icons render at multiple sizes. The icon system MUST produce acceptable visual output across the full size range.

## 5.1 Supported Size Range

Per SPEC-HAL-095 §3:

| Token | Size | Context |
|-------|------|---------|
| I-XS | 10px | Ultra-compact: dense lists, indicators |
| I-S | 12px | Primary: tree icons, list icons, small buttons |
| I-M | 14px | Standard: toolbar buttons, menu items |
| I-L | 16px | Emphasis: section headers, inspectors |
| I-XL | 20px | Large: dialogs, empty states |

Additionally, widgets may request intermediate sizes via the `(row_height × factor).clamp(min, max)` pattern (e.g., TreeView uses `(row_height × 0.7).clamp(12.0, 18.0)`).

## 5.2 Size-Dependent Behavior

**RULE-SIZE-001 — Legibility at I-XS (10px)**

At 10px (scale factor 10/24 = 0.4167), fine details collapse. Icons MUST remain identifiable at this size. This constrains geometry:

* Minimum feature separation in viewbox space: 4.0 units (→ ~1.7px at 10px render)
* Minimum enclosed area: 16 square viewbox units (→ ~2.8px² at 10px render)
* Interior detail (fold triangles, crease lines) may become invisible at I-XS — this is acceptable if the icon's primary silhouette remains recognizable

**RULE-SIZE-002 — No Aliasing Artifacts at I-L and Above (16px+)**

At 16px and above, all layers and interior detail MUST be visually distinguishable. If a layer is not visible at 16px, it serves no purpose and should be removed.

**RULE-SIZE-003 — Consistent Silhouette**

The icon's outer silhouette MUST be recognizable at every supported size. This means the primary shape (folder rectangle, file page, chevron angle) must dominate the geometry. Decorative details must not distort the silhouette.

## 5.3 Scale Factor Table

Reference table for the 24×24 viewbox:

| Render Size | Scale Factor | 1 Viewbox Unit = | Min Gap (4 VB units) = |
|-------------|-------------|-------------------|----------------------|
| 10px | 0.417 | 0.42px | 1.67px |
| 12px | 0.500 | 0.50px | 2.00px |
| 14px | 0.583 | 0.58px | 2.33px |
| 16px | 0.667 | 0.67px | 2.67px |
| 18px | 0.750 | 0.75px | 3.00px |
| 20px | 0.833 | 0.83px | 3.33px |

---

# 6. Optical Weight Calibration Against Typography

## 6.1 Problem

Icons are filled vector paths. Text is shaped and rasterized. At the same nominal size, they can appear visually mismatched — icons too heavy (thick fills dominating thin text strokes) or too light (thin icon strokes vanishing next to bold text).

## 6.2 Calibration Principle

Icon optical weight MUST be calibrated against Halcyon's primary UI font at the standard body text size (typically 13–14px per SPEC-HAL-092).

The target: an icon placed inline with body text should appear **equal in visual density** — neither heavier nor lighter than the surrounding text.

## 6.3 Calibration Method

For each icon size token, the effective visual weight is controlled by:

1. **Fill area ratio** — Ratio of filled pixels to bounding box area
2. **Stroke weight** — Per SPEC-HAL-095 §4, calibrated per size
3. **Internal whitespace** — Gaps between layers, enclosed negative space

The calibration target:

| Text Size | Icon Size | Target Fill Ratio |
|-----------|-----------|-------------------|
| 11px (caption) | I-XS (10px) | 0.30–0.40 |
| 13px (body) | I-S (12px) | 0.30–0.40 |
| 13px (body) | I-M (14px) | 0.25–0.35 |
| 14px (heading) | I-L (16px) | 0.25–0.35 |
| 16px (title) | I-XL (20px) | 0.20–0.30 |

Fill ratio decreases at larger sizes because the human eye perceives larger filled areas as heavier. This is standard optical compensation.

## 6.4 Validation

Fill ratio is measured by rasterizing the icon at target size (1x DPI) and counting filled vs total pixels within the bounding box. This is a design-time metric, not a runtime check.

Automated validation (§10) SHOULD include fill ratio measurement. Icons outside the target range require manual review — they may be acceptable if the shape inherently demands it (e.g., a solid circle indicator will always have high fill ratio).

---

# 7. Pixel Snapping

## 7.1 Snapping Responsibility

Pixel snapping is applied during `push_icon` when scaling from viewbox coordinates to logical pixel coordinates. The mesh cache (SPEC-264) operates on the snapped coordinates.

## 7.2 Position Snapping

The icon's top-left position (`icon_x`, `icon_y`) MUST be snapped to the nearest physical pixel boundary before path scaling is applied:

```
snapped_x = (icon_x × dpr).round() / dpr
snapped_y = (icon_y × dpr).round() / dpr
```

Where `dpr` is the device pixel ratio.

This ensures the icon's coordinate system aligns with the physical pixel grid, preventing subpixel offset of the entire icon.

## 7.3 Size Quantization

The rendered icon size SHOULD be quantized to produce clean scale factors from the viewbox:

| Viewbox | Render Size | Scale | Pixel-Clean? |
|---------|-------------|-------|-------------|
| 24 | 12px | 0.500 | Yes — exact halving |
| 24 | 16px | 0.667 | Acceptable — 2/3 |
| 24 | 18px | 0.750 | Yes — exact 3/4 |
| 24 | 24px | 1.000 | Yes — identity |
| 24 | 10px | 0.417 | Marginal — 5/12 |
| 24 | 14px | 0.583 | Marginal — 7/12 |

Widget-level icon size computation (e.g., `(row_height × 0.7).floor().clamp(...)`) already produces integer sizes, which is sufficient. No additional quantization is required at the icon system level.

## 7.4 Vertex Snapping (Deferred)

Per-vertex snapping (rounding each scaled path vertex to the nearest pixel) is a potential future optimization for 1x DPI displays. It is NOT required for initial implementation because:

* Integer/half-grid viewbox coordinates (§3.2) combined with position snapping (§7.2) produce acceptable results at common sizes
* Per-vertex snapping can distort geometry at small scales (angles change, proportions shift)
* Halcyon targets Retina/HiDPI as the primary display class, where subpixel precision is less visible

If 1x DPI rendering proves visually inadequate after deployment, per-vertex snapping will be specified as a future amendment.

---

# 8. Complexity Limits

## 8.1 Path Segment Limits

**RULE-COMPLEXITY-001 — Maximum Segments Per Layer**

No single layer path may exceed **24 path segments** (MoveTo, LineTo, QuadTo, CubicTo, Close combined). This keeps tessellation cost bounded and ensures icons remain simple geometric shapes.

**RULE-COMPLEXITY-002 — No Micro-Segments**

No path segment may span less than 1.0 viewbox unit in both X and Y. Segments smaller than this are invisible at most render sizes and waste tessellation budget.

Exception: Close segments and final LineTo segments returning to the start point are exempt.

**RULE-COMPLEXITY-003 — Minimal Bézier Usage**

Quadratic and cubic Bézier curves SHOULD be used only for genuinely curved shapes (circles, arcs, organic forms). Straight-edged geometric icons (folders, files, arrows, chevrons) MUST use only MoveTo, LineTo, and Close.

**RULE-COMPLEXITY-004 — No Inverted Winding**

All filled polygons MUST use consistent winding order (clockwise). Mixed winding within a single layer produces fill-rule artifacts.

## 8.2 Total Icon Budget

**RULE-COMPLEXITY-005 — Maximum Total Segments Per Icon**

The sum of path segments across all layers of a single icon MUST NOT exceed **72 segments** (3 layers × 24 max per layer). Icons exceeding this budget are too complex for the icon system and should be rendered through a different mechanism (e.g., raster fallback per SPEC-265 §13).

---

# 9. Allowed Primitives

## 9.1 Path Commands

The icon system supports:

| Command | Usage |
|---------|-------|
| `MoveTo(x, y)` | Start a new subpath |
| `LineTo(x, y)` | Straight line segment |
| `QuadTo(cx, cy, x, y)` | Quadratic Bézier (per §8.1 restrictions) |
| `CubicTo(c1x, c1y, c2x, c2y, x, y)` | Cubic Bézier (per §8.1 restrictions) |
| `Close` | Close current subpath |

## 9.2 Builder Primitives

`IconBuilder` provides convenience methods that emit the above commands:

| Method | Emits |
|--------|-------|
| `.polygon(&[(x,y)...])` | MoveTo + LineTo chain + Close |
| `.line(x1, y1, x2, y2, width)` | Rect approximation of a stroke segment |
| `.rect(x, y, w, h)` | MoveTo + 3×LineTo + Close |
| `.circle(cx, cy, r)` | 4×CubicTo approximation |

## 9.3 Forbidden Constructs

* **Gradient fills** — Icons are single-color per layer. Gradients are a renderer concern, not an icon geometry concern.
* **Clip paths** — Icon geometry must be self-contained. No clipping dependencies.
* **Text paths** — Icons must not embed text outlines. Use the text pipeline for text.
* **Image fills** — Icons are vector-only. Raster content uses the raster fallback (SPEC-265 §13).
* **Filters/effects** — No blur, drop shadow, or post-process effects in icon definitions. Effects are applied at the renderer level if needed.

---

# 10. Validation and Enforcement

## 10.1 Compile-Time Validation

An `IconDefinition` validator function MUST be provided:

```rust
pub fn validate_icon(def: &IconDefinition) -> Result<(), Vec<IconViolation>>;
```

Checks:

| Check | Rule |
|-------|------|
| Coordinate bounds | RULE-GRID-003 |
| Coordinate precision | RULE-GRID-001 |
| No embedded transforms | RULE-GRID-002 |
| Layer count range | RULE-LAYER-001, RULE-LAYER-002 |
| Segment count per layer | RULE-COMPLEXITY-001 |
| Total segment count | RULE-COMPLEXITY-005 |
| No micro-segments | RULE-COMPLEXITY-002 |
| Viewbox is 24×24 (for standard set) | §3.1 |

## 10.2 Test-Time Enforcement

A test MUST exist that runs `validate_icon` against every icon in `builtin_registry()`. This test fails the build if any built-in icon violates the visual contract.

```rust
#[test]
fn test_all_builtins_comply_with_visual_contract() {
    let reg = builtin_registry();
    for def in reg.iter() {
        let result = validate_icon(def);
        assert!(
            result.is_ok(),
            "Icon {:?} ({}) violates visual contract: {:?}",
            def.id, def.name, result.unwrap_err()
        );
    }
}
```

## 10.3 Design-Time Metrics (Advisory)

The following metrics are measured at design time and logged, but do not fail the build:

* Fill ratio at each size token (§6.3)
* Layer overlap percentage
* Minimum feature size at I-XS

These inform manual review when icons are authored or revised.

---

# 11. Invariants

**INV-VRC-001 — Grid Precision**

All path coordinates in registered `IconDefinition` instances MUST be integer or half-integer values. Violation: build test failure.

**INV-VRC-002 — No Silhouette Outline Layers**

No registered icon may contain a filled polygon layer with `LayerRole::Outline` that traces the full icon silhouette. Violation: visual occlusion of interior layers.

**INV-VRC-003 — Layer Count Bound**

No registered icon may exceed 5 layers. Violation: build test failure.

**INV-VRC-004 — Segment Count Bound**

No single layer may exceed 24 path segments. No icon may exceed 72 total segments. Violation: build test failure.

**INV-VRC-005 — Position Snapping**

`push_icon` MUST snap the icon's origin to physical pixel boundaries before applying viewbox-to-render scaling. Violation: subpixel drift of entire icon.

**INV-VRC-006 — Color Resolution Determinism**

Layer color resolution (§4.2) MUST be a pure function of the base color and layer role. No external state, no theme lookups inside color resolution. The widget provides the base color; the layer role determines the math.

**INV-VRC-007 — Cross-Size Silhouette Stability**

An icon's primary silhouette MUST remain recognizable at all sizes from I-XS (10px) through I-XL (20px). Violation: design review failure.

---

# 12. Relationship to Existing SPECs

| SPEC | Relationship |
|-----|-------------|
| SPEC-HAL-265 | Parent. This SPEC defines the visual discipline for the icon system 265 architects. |
| SPEC-HAL-095 §1–10 | Design source. Size tokens, stroke weights, alignment, color model adopted and extended. |
| SPEC-HAL-264 | Substrate. Mesh cache operates on paths that comply with this contract. |
| SPEC-HAL-096 | Color system. Base colors provided by theme; layer role resolution is this SPEC's domain. |
| SPEC-HAL-104 | Peer discipline. Text pixel stability (104) and icon pixel stability (this SPEC §7) are parallel contracts ensuring visual coherence. |
| SPEC-HAL-092 | Typography reference. Optical weight calibration (§6) targets alignment with typography rules. |

---

# 13. Implementation Milestones

### Phase 1 — Validator Function

* Implement `validate_icon()` with grid, layer, and complexity checks
* Add `test_all_builtins_comply_with_visual_contract` test
* Audit existing 13 icons against contract; fix any violations

### Phase 2 — Position Snapping

* Add DPR-aware position snapping to `push_icon` path
* Verify visual output at 1x and 2x DPI

### Phase 3 — Fill Ratio Tooling (Advisory)

* Implement offline fill ratio measurement tool
* Measure all 13 icons at each size token
* Document results; flag outliers for review

### Phase 4 — Optical Weight Pass

* Adjust icon geometry based on fill ratio results
* Ensure visual balance against primary UI font at body text size
* Visual sign-off at all 5 size tokens

---

# 14. Conclusion

This SPEC transforms icon quality from subjective visual tuning into an **enforceable engineering contract**. Every icon authored for Halcyon must pass coordinate validation, layer discipline checks, and complexity bounds. Pixel snapping ensures physical-pixel alignment. Optical weight calibration ensures icons and text coexist as a unified visual system.

The contract scales: whether the icon set contains 13 or 300 definitions, the rules are the same, the tests are the same, and the visual output is consistent.
