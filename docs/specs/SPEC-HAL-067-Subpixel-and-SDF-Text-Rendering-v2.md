# **SPEC-HAL-067 — Subpixel & SDF Text Rendering v2**

**SPEC-HAL-067**
**Subpixel & SDF Text Rendering v2**
**Author:** Halcyon Architect
**Status:** Design – Not Implemented
**Version:** 1.0

**Depends On:**

* SPEC-HAL-047 Native Text Backends
* SPEC-HAL-048 Text Layout Engine
* SPEC-HAL-049 Font Resolution
* SPEC-HAL-050 HarfBuzz Complex Shaping
* SPEC-HAL-051 Font Loading / Hot-Swap
* SPEC-HAL-066 Unified Glyph Raster Pipeline
* SPEC-HAL-092 Unified Typography Rules
* SPEC-HAL-104 Pixel-Stable Text Metrics

---

## Implementation Status

**Not implemented**

SDF/MSDF text rendering and subpixel LCD-AA paths are not implemented. The renderer uses grayscale (R8) rasterization only.

---

**Implements:**
A dual-path text renderer:

1. **Native Subpixel LCD-AA Path**
   (DirectWrite / CoreText / FreeType LCD AA)

2. **SDF/MSDF Vector-Based Path**
   (Resolution-independent scaling & transforms)

Both paths share:

* Unified font metrics
* Unified quad builder
* Unified atlas & keying
* Deterministic behavior under fractional positioning
* Stable rendering under resize/animation

---

## **1. Motivation**

SPEC-066 completes the raster → atlas → quad pipeline.

Remaining issues:

* Native grayscale-only output limits crisp UI text
* Fractional coordinates cause mild blur without subpixel AA
* Large text and transformed text degrade rapidly
* No scaling/rotation stability
* No unified strategy for platform LCD AA
* No SDF/MSDF support for titles, editors, vector zooming
* No deterministic path selection

This SPEC defines the **Text Rendering v2** subsystem:
Subpixel AA for small/medium UI text, SDF/MSDF for large/dynamic text.

---

## **2. Goals**

### **2.1 Primary**

* Add subpixel LCD-AA text on all platforms
* Add SDF/MSDF text rendering
* Unify both in a single renderer
* Maintain identical metrics across both paths
* Deterministic per-node renderer selection
* Support fractional positioning without drift
* Stable under DPI scaling, transforms, animations

### **2.2 Secondary**

* Separate SDF & native glyph atlases
* Keep GlyphBitmap unified across formats
* Maintain identical bear/advance metrics
* Keep HarfBuzz integration unchanged
* Allow future text effects (outline, glow, shadow)

---

## **3. Architecture Overview**

```
        HarfBuzz (shaping)
               ↓
        ShapedRun { glyph_id, x, y }
               ↓
      FontRenderer (decision layer)
         ↙                     ↘
 NativeSubpixelPath        SDFPath (MSDF)
         ↓                     ↓
 GlyphBitmap (RGB)      SDFBitmap (RGB)
         ↓                     ↓
 NativeAtlas           SDFAtlas
         ↓                     ↓
  build_quads()       build_quads()
         ↓                     ↓
      Renderer → Text Pass (two pipelines)
```

---

## **4. Rendering Paths**

### **4.1 Native Grayscale Path**

* Uses grayscale AA from platform rasterizer
* Outputs `GlyphBitmap` with `GlyphFormat::Grayscale`
* MAY use `SubpixelBinX` for horizontal positioning stability, but it is optional if only vertical crispness is required
* Recommended to use `SubpixelBinX` to match egui-style behavior for consistent results

### **4.2 Native Subpixel RGB Path (LCD-AA)**

* Uses LCD-AA from platform rasterizer
* Outputs `GlyphBitmap` with `GlyphFormat::SubpixelRgb`
* Best for 10–40px UI text

#### SubpixelBinX Requirement (MANDATORY)

`SubpixelBinX` is **mandatory** for subpixel RGB glyphs:

* Subpixel RGB glyphs MUST be rasterized and cached per `SubpixelBinX` to ensure stable sampling of LCD subpixels.
* Rendering the same glyph at different bins MUST result in different bitmaps keyed by `SubpixelBinX`.
* The atlas key MUST include `subpixel_bin_x` as defined in SPEC-HAL-066 Section 5.4.

#### Supported backends:

* DirectWrite: ClearType RGB
* CoreText: LCD smoothing
* FreeType: `FT_LOAD_TARGET_LCD` + 3-channel bitmaps

#### No Feedback Loop Rule

Subpixel phase, sampling offsets, and bin selection influence rendering output only. They MUST NOT feed back into layout, shaping, or metric calculations. No render-time values may be used as layout inputs (SPEC-HAL-002).

### **4.3 SDF/MSDF Path**

* Vector-based signed distance fields
* Best for 40px+ or transformed text
* Stable under arbitrary scale/rotate
* Multi-channel SDF (MSDF) for sharp corners
* Output stored as `GlyphBitmap` with `GlyphFormat::Sdf`

#### SubpixelBinX Handling (SDF-Specific)

For SDF/MSDF formats, `SubpixelBinX = Zero` (or equivalent) MUST be used for cache keying. Subpixel phase is expressed purely as continuous sampling in the shader, not through discrete atlas variants.

SDF/MSDF is inherently phase-flexible and does not require per-bin variants in the atlas. The signed distance field allows the shader to sample at arbitrary subpixel positions without pre-rasterizing multiple variants.

### **4.4 Background Stability Constraint**

Native LCD-AA *requires* stable opaque backgrounds.
Renderer must detect:

* background opacity
* overlapping translucent widgets
* animation/transforms
* complex compositing

If the background cannot be sampled reliably:

```
RenderMode::Grayscale
```

This mirrors macOS, Windows, Chrome, Safari, and Skia behavior.

### **4.5 Decision Rule**

```
if text_size <= threshold_px && transform == identity:
    use NativeSubpixel
else:
    use SDF/MSDF
```

Threshold default: **40px**.

### **4.6 Stable-Phase Selection**

Format resolution (Grayscale / SubpixelRgb / SDF) occurs before Phase 5. It MUST NOT change dynamically during Phase 6 (Renderer), ensuring no mid-frame mutation or layout invalidation.

### **4.7 Ephemerality Constraint**

Subpixel variants and raster outputs are frame-local assets. They MUST NOT be persisted across frames as references or stateful objects. Only atlas-resident immutable bitmaps may carry over.

---

## **5. CPU Rasterizers (Extended)**

### **5.1 Trait Extension**

Existing interface from SPEC-066 extended with new format:

```
pub enum GlyphFormat {
    Grayscale,
    SubpixelRgb,
    Sdf,
}
```

### **5.2 Native Subpixel Rasterization**

All backends must produce:

* RGB horizontal AA
* bearings identical to grayscale path
* advance identical
* glyph bounding box identical

### **5.3 SDF/MSDF Generation**

Pipeline:

1. Extract FreeType outline
2. Pass outline to msdfgen (or Rust port)
3. Generate MSDF bitmap
4. Normalize channels
5. Output GlyphBitmap using format Sdf

---

## **6. GPU Atlases**

### **6.1 Two Separate Atlases**

```
NativeAtlas → RGBA8 (SubpixelRGB)
SDFAtlas    → RGBA8 (MSDF)
```

### **6.2 Keying**

Extended SPEC-066 key:

```
(font_id, glyph_id, px_size, format)
```

### **6.3 Packing**

* ShelfPacker or SkylinePacker
* Deterministic growth
* No eviction in v2

---

## **7. Quad Builder Integration**

Unchanged from SPEC-066 except:

```
GlyphQuad includes:
    render_mode: RenderMode
```

Where:

```
enum RenderMode {
    Grayscale,
    SubpixelRgb,
    Sdf,
}
```

Both paths feed the same quad builder.

---

## **8. Renderer Integration**

### **8.1 Pipelines**

Renderer maintains two pipelines:

1. **native_text_pipeline**

    * Subpixel RGB sampling
    * Linear-light blending (macOS)
    * Gamma-correct multiply

2. **sdf_text_pipeline**

    * SDF/MSDF sampling
    * Smoothstep or analytic edge
    * Transform-ready

### **8.2 Instance Layout**

Same as SPEC-066 plus a mode flag.

```
struct TextInstance {
    pos: vec2,
    uv_min: vec2,
    uv_max: vec2,
    color: vec4,
    render_mode: u32,
}
```

### **8.3 Shader Responsibilities**

**Native AA Shader**

* Sample RGB
* Convert to linear
* Multiply color
* Convert to sRGB

**SDF Shader**

* Sample MSDF
* Compute signed distance
* Run smoothstep
* Output color

### **8.4 Correct LCD-AA Compositing Contract**

Native subpixel glyphs are *not* alpha masks.
They require:

1. Sampling the background
2. Linear-light RGB compositing
3. Per-channel coverage

Explicit formula:

```
fg_lin = srgb_to_linear(fg_color)
bg_lin = srgb_to_linear(bg_color)
out_lin = bg_lin * (1 - cov.rgb) + fg_lin * cov.rgb
out = linear_to_srgb(out_lin)
alpha = max(cov.r, cov.g, cov.b)
```

---

## **9. Interaction with Text Layout (SPEC-048)**

No changes.

* Advances identical
* Baselines identical
* Metrics identical
* Only renderer path differs

---

## **10. Testing Matrix**

### **10.1 Native Subpixel**

* Fractional x positions
* Horizontal AA stability
* Resizing windows
* ClearType/Gamma correctness

### **10.2 SDF Path**

* Scaling: 0.5× … 4×
* Rotations: 0–360°
* Sharp corner fidelity
* Distance field alignment

### **10.3 Atlas Behavior**

* Mixed sizes
* Mixed formats
* Deterministic packing
* No drift

---

## **11. Implementation Plan**

### **Phase 1 — Format Layer**

* Extend GlyphFormat
* Integrate format into GlyphKey
* Extend GlyphBitmap for RGB & SDF

### **Phase 2 — Native Subpixel Rasterizers**

* DirectWrite LCD-AA
* CoreText LCD smoothing
* FreeType LCD-AA
* Ensure identical bearings/advances to grayscale

### **Phase 3 — SDF/MSDF Generator**

* Outline extraction
* msdfgen integration
* SDFAtlas creation

### **Phase 4 — Renderer Pipelines**

* native_text_pipeline
* sdf_text_pipeline
* uniform & instance updates

### **Phase 5 — FontRenderer Dispatch**

* Automatic selection rule
* Developer overrides

### **Phase 6 — Stabilization**

* Fractional AA tests
* SDF scaling tests
* Screenshot validation

---

## **12. Implementation Notes**

### **12.1 Compatibility**

* Works with existing shaping (SPEC-050)
* Works with unified raster pipeline (SPEC-066)
* No changes to ParagraphEngine

### **12.2 SPEC-HAL-104 Pipeline Integration**

SPEC-HAL-104 considers `SubpixelBinX` when mapping physical X positions to atlas entries, and relies on SPEC-HAL-067's guarantee that subpixel-aware formats have correct per-bin rasters. Without this guarantee, SPEC-HAL-104's stability guarantees for subpixel text do not hold.

Summary of format-specific `SubpixelBinX` requirements:

| Format         | SubpixelBinX Usage                        |
| -------------- | ----------------------------------------- |
| Grayscale      | Optional (recommended for consistency)    |
| SubpixelRgb    | **Mandatory** (per-bin rasterization)     |
| SDF/MSDF       | Fixed to `Zero` (shader handles phase)    |

### **12.3 Future SPECs**

* SPEC-HAL-068 — Text Compositing Effects
* SPEC-HAL-069 — Text Animations & Morphing
