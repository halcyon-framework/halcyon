# **SPEC-HAL-104: Pixel-Stable Text Metrics & Glyph Quad Normalization**

**Status:** Draft
**Author:** JC
**Depends on:**

* SPEC-HAL-006 — Text & Font System
* SPEC-HAL-024 — Coordinate Model
* SPEC-HAL-025 — Multi-line Text Layout
* SPEC-HAL-029 — Glyph Atlas Cache
* SPEC-HAL-048 — Paragraph & Text Layout Engine
* SPEC-HAL-050 — HarfBuzz Shaping Integration
* SPEC-HAL-051 — Font Loading & Asset Management
* SPEC-HAL-065 — Shaping Implementation
* SPEC-HAL-066 — Unified Glyph Raster Pipeline
* SPEC-HAL-067 — Subpixel & SDF Text Rendering v2
* SPEC-HAL-092 — Unified Typography Rules
* SPEC-HAL-103 — System UI Font Default

---

# 1. **Overview & Normative Role**

SPEC-HAL-104 is the **normative pixel-stability layer** for the Halcyon text pipeline. It sits on top of:

* **SPEC-HAL-048** — Paragraph & Text Layout Engine
* **SPEC-HAL-050 / SPEC-HAL-065** — HarfBuzz shaping
* **SPEC-HAL-066 / SPEC-HAL-067** — Glyph raster pipeline & atlas formats
* **SPEC-HAL-092** — Unified typography grid (logical)
* **SPEC-HAL-103** — System UI font defaults

This SPEC does **not** alter shaping semantics. It constrains:

* Metric normalization (vertical)
* Physical-pixel coordinate accumulation (horizontal)
* Atlas offset usage
* Glyph and block origin snapping rules

## 1.1 Guarantees

This SPEC guarantees:

* **Stable baselines** across runs and frames for identical typography tokens.
* **No weight wobble** for repeated glyphs at the same size.
* **No fractional vertical positioning** for base glyph geometry.
* **Deterministic mapping** from shaped clusters → glyph quads.
* **Pixel-identical output** for the same input text, font, size, and scale factor.

All text geometry MUST conform to these rules. Implementations that bypass this SPEC produce non-compliant output.

---

# 2. **Problem Statement**

The current text pipeline produces visible **weight wobble**, **periodic drift**, **baseline instability**, and **glyph shimmering** during layout or scrolling.

Observed issues:

1. Vertical drift across lines due to unrounded ascents/descents.
2. Inconsistent baseline per glyph because raw shaping metrics flow directly into quad placement.
3. Horizontal drift every few characters due to unconstrained fractional X accumulation.
4. Glyph quads straddling pixel boundaries, causing inconsistent sampling.
5. Rasterized bitmap offsets mismatching quad offsets, creating subpixel tension.
6. No deterministic rule for snapping glyphs to a stable pixel grid.

All of these combine into intermittent “thick → thin → thick” oscillation you see in sequences like:

```
=====!=====
```

And they appear on **both macOS and Windows**, so this is not backend-specific.

---

# 3. **Goal**

Define a pixel-stable, deterministic text metric and quad placement model that:

* Eliminates drift in long lines.
* Locks baselines to integer-aligned values.
* Produces visually identical text regardless of content distribution.
* Keeps full shaping capability (ligatures, GPOS, clusters).
* Does not degrade features defined in 048/050/066/067.
* Applies at **layout time**, not at render time.

The system becomes **predictable**, **repeatable**, and **GPU-stable**.

---

# 4. **Design Overview**

The solution introduces six layered rules:

---

## **Rule 1 — Rounded Font Metrics (Load-Time Normalization)**

When a font instance is created:

* Ascent
* Descent
* Line gap
* Pixel-scale equivalents

MUST be passed through the Coordinate Model and normalized to **pixel-aligned values**:

```
ascent_px  = round(ascent_raw * pixels_per_point)
descent_px = round(descent_raw * pixels_per_point)
line_gap_px = round(line_gap_raw * pixels_per_point)
```

These become canonical metrics for **all future layout**.

This stabilizes the vertical baseline globally.

---

## **Rule 2 — SPEC-HAL-092 Baseline Grid Compliance**

SPEC-HAL-104 MUST derive normalized metrics in a way that respects SPEC-HAL-092's 4-unit baseline grid.

### 2.1 Line Height Calculation

Define `line_height_logical` (in logical px) as:

```
line_height_logical = ascent_rounded_logical - descent_rounded_logical + line_gap_rounded_logical
```

**Requirement:** `line_height_logical` MUST be a multiple of 4 logical units as defined by SPEC-HAL-092.

### 2.2 Grid Snapping Behavior

When the raw font metrics do not align perfectly to the 4-unit grid:

1. Compute the "natural" line height from raw metrics.
2. Snap `line_height_logical` to the **nearest** 4-unit multiple.
3. Adjust `line_gap_rounded_logical` and/or baseline offset to keep the visual center consistent.

### 2.3 Typography Token Invariant

All text rows for a given typography token (e.g., T-S, T-M, etc.) MUST use the same snapped `line_height_logical` based on this rule. Mixed fonts (emoji, fallback) MUST NOT change the line-height; they can only adjust internal glyph positioning within the established line box.

See SPEC-HAL-092 Section 2 (Baseline Grid) for the logical grid definition.

---

## **Rule 3 — Physical Pixel Accumulation (X-axis Layout)**

The layout engine (SPEC-HAL-048) MUST accumulate horizontal advances in **physical pixel space**, not logical space.

### 3.1 Horizontal Cursor Model

Introduce a clear concept:

```
cursor_x_phys: f32  // Physical-pixel horizontal cursor per line/run
```

### 3.2 Accumulation Procedure

1. After shaping, each cluster has an `advance_logical` (from HarfBuzz).
2. Convert `advance_logical` into physical px **once**:
   ```
   advance_px = round(advance_logical * pixels_per_point)
   ```
3. Accumulate:
   ```
   cursor_x_phys += advance_px
   ```

### 3.3 Conversion Invariant

Implementations MUST NOT re-multiply logical coordinates by scale factors inside the inner glyph loop. The physical X position of any glyph is computed from a snapped line origin plus the accumulated `cursor_x_phys`. The renderer MUST NOT recompute cluster positions from logical coordinates and scale factors independently.

This removes floating-point conversion drift between logical/physical domains.

---

## **Rule 4 — Raster-Time Offset Baking (Atlas Consistency)**

The rasterizer (SPEC-HAL-066/067) MUST bake the following into the atlas metadata:

* bitmap origin
* baseline offset
* bearing offsets
* integer-aligned bounding boxes

### 4.1 Atlas Entry Structure

Each atlas entry MUST contain:

* `bitmap_width_px`, `bitmap_height_px` — integer bitmap dimensions
* `offset_px = (offset_x_px, offset_y_px)` — from the line origin/baseline to the bitmap's top-left
* `subpixel_bin_x` — horizontal subpixel phase reference (see SPEC-HAL-066 Section on Subpixel Quantization)

### 4.2 Single Source Authority

The atlas returns a **stable, pre-aligned** (offset, size).

Glyph quads MUST use only `NormalizedFontMetrics` and the atlas entry's `offset_px` and `bitmap_size_px` for their vertical and horizontal placement. No code is allowed to recompute bearings or baseline offsets from raw font metrics or shaping data at quad time.

**Rationale:** This prevents mixed rounding paths between raster and quad builder stages.

This locks raster origin and placement origin to one authority.

---

## **Rule 5 — Glyph Quad Snapping (Final Placement Normalization)**

Before emitting vertices:

### 5.1 Ideal Position Calculation

For each glyph, compute its "ideal" top-left in physical px:

```
ideal_x = block_origin_px.x + line_offset_px.x + glyph_x_px + atlas_offset_px.x
ideal_y = block_origin_px.y + line_offset_px.y + baseline_px - atlas_offset_px.y
```

### 5.2 Snapping Requirement

Then define:

```
snapped_x = round(ideal_x)
snapped_y = round(ideal_y)
```

Both `snapped_x` and `snapped_y` MUST be used as the final quad corner coordinates. **Partial snapping (X only or Y only) is forbidden.**

### 5.3 Quad Dimensions

Width and height of the quad come from integer bitmap size in px, not recomputed floats:

```
quad.width  = atlas_entry.bitmap_width_px
quad.height = atlas_entry.bitmap_height_px
```

This ensures no glyph is ever placed straddling two pixels.

This is the core rule that fully removes the "every 5 glyphs wobble" effect.

---

## **Rule 6 — Text Block Origin Snapping**

Before emitting any glyph quads for a text node (paragraph, label, text block), the renderer MUST snap the node's origin to the nearest physical pixel.

### 6.1 Block Origin Snapping

```
origin_phys_x = round(origin_phys_x)
origin_phys_y = round(origin_phys_y)
```

### 6.2 Application Scope

* This is done **once per block/galley**.
* All glyph positions are computed relative to this snapped origin.
* This matches egui-style galley snapping but generalized for all text containers.

### 6.3 Grid Compatibility

This snapping does not violate SPEC-HAL-092's baseline grid; it aligns the logical grid to the actual framebuffer pixel grid. The baseline grid remains intact in logical space while physical rendering achieves pixel alignment.

---

# 5. **Pipeline Integration**

The normalized model modifies pipeline stages:

---

## **Stage A — Font Load (SPEC-HAL-006/051 Integration)**

Compute normalized metrics once per font+size+scale factor.

### A.1 NormalizedFontMetrics Structure

Conceptually define:

```
NormalizedFontMetrics {
    ascent_px: i32,           // Rounded physical px
    descent_px: i32,          // Rounded physical px (negative)
    line_gap_px: i32,         // Rounded physical px
    line_height_px: i32,      // Computed and grid-snapped
    baseline_offset_px: i32,  // Position of baseline within line
}
```

### A.2 Normalization Rules

1. Raw metrics come from the same font bytes that shaping uses.
2. Convert to physical px using device scale **once**.
3. Round each of ascent/descent/line_gap to nearest physical pixel.
4. Compute `line_height_px` from those values.
5. Snap `line_height_px` to the nearest multiple of `(4 * pixels_per_logical_unit)` to satisfy SPEC-HAL-092.
6. Compute `baseline_offset_px` so that the font baseline remains visually consistent.

### A.3 Post-Normalization Invariant

All later stages MUST use `NormalizedFontMetrics` as the only vertical metrics source. Using raw un-rounded font metrics after this stage is forbidden in layout or quad building.

### A.4 Identity Constraint

NormalizedFontMetrics MUST be keyed only by (font_id, size, dpi_scale). They MUST NOT contain references into any shaping, layout, or paragraph structures. No persistent cross-structure references are allowed (SPEC-HAL-002: Identity-by-ID Only).

Native text-engine adapters MUST reuse immutable metrics across frames by this
exact identity. Font size and scale use exact float-bit identity; cache retention
is bounded, and changing the active font identity or scale invalidates or rekeys
the entries. Metric reuse does not retain shaping output and must not change
metric values, glyph positions, rasterization, or draw ordering.

### A.5 Constitutional Phase Constraint

All normalization and metric computation occur in Phase 4 (Layout) and Phase 5 (Scene Build) as defined by SPEC-HAL-001. No AppState, WidgetState, or WidgetTree mutation is allowed during these phases (SPEC-HAL-002 Rule: No Mid-Frame Mutation).

---

## **Stage B — Shaping Output Adjustment (SPEC-HAL-050/065 Integration)**

Shaping still produces:

* advance
* bearing
* cluster positions

but all vertical components are **reanchored** to normalized metrics:

```
glyph.y = rounded_ascent - bearing_y
```

No per-glyph float ascent allowed.

### B.1 Ephemeral Shaping Rule

Shaping outputs (clusters, glyph runs, line breaks) are ephemeral. They MUST NOT be stored in widget state, AppState, caches, or persisted across frames. All geometry MUST be regenerated each frame from canonical text inputs and NormalizedFontMetrics.

---

## **Stage C — Multi-line Layout (SPEC-HAL-025/048 Integration)**

All line heights become:

```
line_height = rounded_ascent - rounded_descent + rounded_gap
```

This value MUST be grid-snapped per Rule 2. Zero drift between lines.

---

## **Stage D — Atlas Lookup (SPEC-HAL-029/066/067 Integration)**

Return stable baked offset to quad generator:

```
atlas_offset = baked_offset_px
atlas_size   = baked_bitmap_size_px
subpixel_bin = atlas_entry.subpixel_bin_x
```

No independent bearing math. See SPEC-HAL-066 for subpixel bin definitions.

---

## **Stage E — Quad Generation (TextEngine + Batcher)**

Construct quads per Rule 5:

```
ideal_x = block_origin_px.x + line_offset_px.x + glyph_x_px + atlas_offset.x
ideal_y = block_origin_px.y + line_offset_px.y + baseline_px - atlas_offset.y
```

Then snap:

```
x = round(ideal_x)
y = round(ideal_y)
```

Emit final quad with integer bitmap dimensions.

---

## **Stage F — Block Origin Snapping (Renderer Integration)**

Before Stage E, per Rule 6:

```
block_origin_px.x = round(block_origin_px.x)
block_origin_px.y = round(block_origin_px.y)
```

This is performed once per text block/galley before any glyph quads are computed.

---

# 6. **Guarantees**

The new model guarantees:

* All baselines are identical for a given font size.
* No drift when reshaping the same text block repeatedly.
* No cumulative X error in long lines.
* No Y wobble across glyphs.
* No variation in perceived stroke weight as cursor crosses fractional positions.
* Raster + layout alignment always match.

---

# 7. **Non-Goals**

* Does **not** remove ligatures.
* Does **not** remove GPOS kerning.
* Does **not** introduce fallback simplifications.
* Does **not** degrade SPEC-HAL-067 SDF pipeline.
* Does **not** alter paragraph line breaking rules.

---

# 8. **Implementation Stages**

### **Stage 1 — Normalized Metrics**

Add normalized font metrics struct and scale logic.

### **Stage 2 — Replace Raw Bearings**

Make quad builder rely on atlas baked offsets.

### **Stage 3 — Pixel-Space Cursor**

Switch layout X accumulation to pixel units.

### **Stage 4 — Pixel Snapping**

Introduce round() snapping in glyph quad builder.

### **Stage 5 — Vertical Line Height Normalization**

Use normalized metrics in line layout.

### **Stage 6 — Integration Into SPEC-HAL-092 Typography Rules**

Extend Unified Typography Rules to define Halcyon's pixel-aligned model.

### **Stage 7 — Block Origin Snapping**

Add galley/text-block origin snapping before quad generation.

---

# 9. **Expected Results**

* Text becomes completely stable.
* Repeated glyphs produce identical visual weights.
* No wobble or jitter during scrolling.
* Baseline unified across all fonts.
* Glyph rasterization and placement finally match.

---

# 10. **Cross-References**

* **SPEC-HAL-092** — Defines the logical 4-unit baseline grid that this SPEC implements in physical pixels.
* **SPEC-HAL-066** — Defines glyph raster pipeline and subpixel bin model used by Stage D.
* **SPEC-HAL-067** — Defines SDF/subpixel rendering formats compatible with this pipeline.
* **SPEC-HAL-069** — Defines animated text effects that operate on top of SPEC-HAL-104-compliant geometry.

---

# 11. **Note: Control Text Placement Invariants**

Control text placement invariants (SPEC-HAL-092 Section 8A) rely on the glyph metric
definitions and normalization guarantees defined in this SPEC.

Specifically:
* `NormalizedFontMetrics.ascent_px` and `descent_px` are the authoritative values
  for control text vertical placement.
* Controls MUST use these metrics (via `compute_control_text_baseline_y()`) rather
  than `line_height` for glyph containment within control bounds.
* The glyph containment guarantee (INV-GEOM-CTRL-TEXT-1 in SPEC-HAL-094) depends on
  the normalized metrics being accurate and stable.

See:
* SPEC-HAL-092 Section 8A — Canonical control text placement formula
* SPEC-HAL-094 Section 3A — Glyph containment invariant
* SPEC-HAL-106 Section 17A — Fidelity enforcement rule
