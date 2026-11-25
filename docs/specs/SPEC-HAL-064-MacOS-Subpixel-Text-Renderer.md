# **SPEC-HAL-064 — macOS Subpixel Text Renderer (Complete)**

**SPEC-HAL-064**
**macOS Subpixel Text Renderer (Level 3)**
**Author:** Halcyon Architect
**Status:** Design – Not Implemented
**Version:** 1.0
**Depends On:**

* SPEC-HAL-047 Native Text Backends
* SPEC-HAL-048 Paragraph/Text Layout
* SPEC-HAL-050 HarfBuzz/Complex Shaping (future)
* SPEC-HAL-032 Hit-Test Acceleration (coordinate correctness)
* SPEC-HAL-006 Text & Font System

**Implements:**
Pixel-perfect CoreText-quality subpixel text on macOS (RGB LCD, linear-light blending, opaque-only backgrounds).

---

## Implementation Status

**Not implemented**

macOS subpixel text rendering is not implemented. The renderer uses grayscale (R8) atlas rendering on all platforms.

---

## **1. Motivation**

macOS uses **LCD subpixel antialiasing** to deliver extremely sharp stroke edges and thin, stable lines.
Grayscale AA (R8 atlas) produces:

* Heavier strokes
* Softer vertical edges
* Slight blur at medium sizes
* Lower contrast
* “Fuzziness” compared to native macOS apps

Halcyon already supports:

* DirectWrite backend on Windows
* FreeType backend on Linux
* CoreText backend (Phase 1)
* Baseline-correct glyph positioning (Phase 2)
* Gamma-correct grayscale compensation

But **subpixel rendering** is required to reach macOS-native quality.

Goal:
Match Safari/TextEdit/Xcode text pixel-for-pixel under controlled background conditions.

---

## **2. Goals**

### **2.1 Primary**

* Implement **RGB subpixel glyph rasterization** via CoreGraphics/CoreText.
* Use **RGBA8 glyph atlas** with 3 independent coverage channels.
* Perform **per-channel blending** in **linear light**.
* Snap baselines and Y positions using Halcyon’s unified renderer snapping.
* Support **devicePixelRatio-aware** positioning.
* Detect **RGB vs BGR** subpixel order.
* Match macOS stroke weight at all sizes.

### **2.2 Secondary**

* Maintain grayscale fallback for:

    * transparent backgrounds
    * zoomed layers
    * non-integer scale transforms

### **2.3 Non-Goals**

* No CoreImage dependency.
* No multilayered color-correct compositing.
* No hinting override beyond macOS defaults.

---

## **3. System Architecture**

### **3.1 Overview**

```
CoreText glyph rasterization → offscreen RGB bitmap → Halcyon RGBA8 Atlas → Linear-light blend in text shader
```

### **3.2 Rasterization Mode**

CoreText outputs LCD AA when rendering into a **24-bit RGB** CGContext, not grayscale.

* Must disable alpha-only contexts.
* Must not use `kCGImageAlphaNoneSkipLast` incorrectly (El Capitan bug pattern).
* Must set `ShouldAntialias = true` and `ShouldSmoothFonts = true`.

### **3.3 Atlas Format**

**Old:**
`R8` single-channel (grayscale coverage)

**New:**
`RGBA8`
Channels:

* R = coverage for red subpixel
* G = coverage for green
* B = coverage for blue
* A = *unused* (reserve for future distance field)

Packed as premultiplied coverage ONLY.

---

## **4. Glyph Rasterization (macOS)**

### **4.1 Context Creation**

* 8 bits per component
* RGB bitmap context
* No premultiplication
* Colorspace: `CGColorSpaceCreateDeviceRGB()`
* Alpha mode: `kCGImageAlphaNoneSkipLast`

### **4.2 Placement Logic**

Halcyon conventions (already implemented in Phase 2):

```
bearing_y = ascent_px
baseline_y = 1.0 + descent_px
height = ascent_px + descent_px + 2
```

These values drive rasterization positioning and are consistent across sizes.

### **4.3 Data Extraction**

After drawing:

* Read raw bytes: `width * height * 4`
* Extract RGB channels (ignore alpha)
* Pack into atlas tile

---

## **4b. Background Sampling Requirement**

LCD subpixel AA cannot be composed correctly using premultiplied alpha alone.
The glyph mask must be combined with the *actual background color* under the text.
Therefore, the renderer must adopt a **two-pass text pipeline**:

### **Pass 1 — Background Prepass**

Render the background under the text region into an offscreen texture:

```
bg_texture = render_background(bounds)
```

### **Pass 2 — LCD-AA Text Pass**

Sample both:

```
cov.rgb     → glyph LCD coverage (from atlas)
bg_srgb     → background color under pixel (from bg_texture)
fg_srgb     → text color
```

Convert bg_srgb and fg_srgb to linear:

```
bg = srgb_to_linear(bg_srgb)
fg = srgb_to_linear(fg_srgb)
```

Perform correct LCD compositing in linear:

```
out_linear.r = lerp(bg.r, fg.r, cov.r)
out_linear.g = lerp(bg.g, fg.g, cov.g)
out_linear.b = lerp(bg.b, fg.b, cov.b)
```

Convert back to sRGB:

```
out = linear_to_srgb(out_linear)
```

Output premultiplied:

```
alpha = max(cov.r, cov.g, cov.b)
return vec4(out, alpha)
```

### **Fallback Rule**

If the widget's background is not opaque or cannot be isolated:

```
use grayscale AA path
```

---

## **5. GPU Pipeline**

### **5.1 Shader Contract**

Inputs:

* `cov_r`, `cov_g`, `cov_b` — from atlas sample
* `fg_color` — text color
* `bg_color` — under-text framebuffer
* `opaque_background` — bool uniform

### **5.2 Linear-Light Rules**

1. Convert bg + fg from sRGB → linear
2. Blend per channel:

```
out_linear.r = lerp(bg_lin.r, fg_lin.r, cov_r)
out_linear.g = lerp(bg_lin.g, fg_lin.g, cov_g)
out_linear.b = lerp(bg_lin.b, fg_lin.b, cov_b)
```

3. Convert back to sRGB
4. Write out

### **5.3 Transparent Background Fallback**

If `opaque_background == false`:

* Use grayscale AA instead
* Use existing pow(coverage, 1.45) gamma correction

---

## **6. DevicePixelRatio & Subpixel Order**

### **6.1 Required**

* Detect hardware pixel order (RGB or BGR):

    * Read from NSScreen
    * Fallback: assume RGB
* Adjust sampling:

    * If BGR → swap cov_r ↔ cov_b

### **6.2 DPR Interaction**

Halcyon forbids fractional DPR for text (already in SPEC-048).
DPR must be integer.

---

## **7. Testing Matrix**

### **7.1 Visual Tests**

* Sizes 6–55 in 1pt increments
* Long paragraphs (cumulative drift correctness)
* Mixed ASCII/Unicode
* Tight vertical stacking
* Nested legends and panels

### **7.2 Programmatic Tests**

* Glyph metrics match CTFont exactly
* Atlas extraction roundtrip sanity
* Shader channel correctness
* Fragment output delta vs. macOS reference < 1 per channel

---

## **8. Implementation Plan**

### **8.1 Phase 1 — Rasterizer**

* Implement RGB CGContext
* Store 3-channel coverage
* Update `GlyphBitmap`

### **8.2 Phase 2 — Atlas Update**

* Switch atlas texture format to `RGBA8UnormSrgb`
* Adjust packing and upload

### **8.3 Phase 3 — Text Shader**

* Add linear-light pipeline
* Add per-channel compositing
* Add RGB/BGR swap logic
* Add grayscale fallback path

### **8.4 Phase 4 — Integration**

* Y snapping obeys Renderer rules
* Ensure baseline-driven positioning stays intact
* Ensure FreeType/Windows unaffected

---

## **9. Edge Cases**

* Emoji → fallback to grayscale
* Color glyphs → grayscale
* Transparent widgets → grayscale
* Rotation transforms → grayscale
* DPI scaling < 1.0 → grayscale

---

# **END OF SPEC-HAL-064**

---

# Historical implementation plan (December 2025)

This is the retained historical implementation record, not a new task or a
statement of current platform conformance.

## **State Summary**

* Windows: DirectWrite backend **complete**, correct, stable.
* macOS: CoreText backend **Phase 2 complete** (baseline-driven, bearing_y = ascent_px, perfect vertical alignment).
* Gamma fix applied: `coverage = pow(coverage, 1.45)` (grayscale path only).
* Rasterizer is pure; **all snapping owned by renderer**.
* Layout-level drift solved via `cumulative_y.round()`.

## **Target Objective**

Implement **SPEC-HAL-064 — macOS Subpixel Text Renderer** (LCD RGB AA).
This is Level 3.
We must match Safari/TextEdit pixel quality.

## Implementation steps

1. Implement RGB CGContext rasterization in `native_font_backend_coretext.rs`
2. Produce RGBA8 glyph bitmaps with 3 coverage channels
3. Update `glyph_atlas.rs` to upload RGBA tiles
4. Update text fragment shader:

    * linear-light compositing per channel
    * RGB/BGR order handling
    * grayscale fallback for transparency
5. Ensure no regressions on Windows or Linux

## **Files to Inspect**

```
src/platform/native_font_backend_coretext.rs
src/renderer/glyph_atlas.rs
src/renderer/wgpu_renderer.rs  (text shader embedded as string ~lines 350-400)
src/core/text_engine.rs
src/widgets/text_block.rs
Cargo.toml
```

## **Constraints**

* No placeholders
* Code must compile
* No changes to existing working paths
* All platform-specific code goes under `#[cfg(target_os = "macos")]`
* Do not modify DirectWrite or FreeType

## **Output Format**

* Explain briefly
* Then produce unified patches
* Then list next micro-tasks
