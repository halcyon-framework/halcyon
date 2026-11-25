# **SPEC-HAL-066 — Unified Glyph Raster Pipeline (CPU Raster → GPU Atlas → Text Quads)**

**SPEC-HAL-066**
**Unified Glyph Raster Pipeline**
**Author:** Halcyon Architect
**Status:** Implemented
**Version:** 1.1
**Depends On:**

* SPEC-HAL-047 Native Text Backends
* SPEC-HAL-048 Text Layout Engine
* SPEC-HAL-049 Font Resolution
* SPEC-HAL-050 HarfBuzz Complex Shaping
* SPEC-HAL-051 Font Loading / Hot-Swap
* SPEC-HAL-065 Shaping Implementation
* SPEC-HAL-092 Unified Typography Rules
* SPEC-HAL-104 Pixel-Stable Text Metrics (for subpixel bin usage)

**Implements:**
A single, unified, cross-platform glyph pipeline:

1. CPU rasterizers (DirectWrite / CoreText / FreeType)
2. Halcyon GlyphBitmap (normalized bitmap format)
3. GPU glyph atlas (multi-channel R8 or RGBA8)
4. Text vertex layout (runs → quads → instances)
5. Text fragment shader (grayscale now, RGB AA later with 064)

This is the final piece before the renderer-level SPECs (028/034/040/045).

---

## **Constitutional Purity Rule**

The raster pipeline is a pure functional stage. It MUST NOT read or mutate WidgetState or AppState. It MUST NOT hold references to shaping, layout, or paragraph data. All AtlasEntry values are immutable.

---

## Implementation Status

**Implemented**

Existing modules/files:
- `src/renderer/glyph_atlas.rs` — GlyphAtlas, GlyphKey, AtlasEntry, SkylinePacker (R8 format)
- `src/renderer/wgpu_renderer.rs` — Text vertex building, quad generation, atlas bind groups
- `src/core/glyph_quad.rs` — GlyphToQuadParams, build_quad_from_glyph
- `src/platform/native_font_backend_directwrite.rs` — DirectWrite rasterizer (Windows)
- `src/platform/native_font_backend_coretext.rs` — CoreText rasterizer (macOS)
- `src/platform/native_font_backend_freetype.rs` — FreeType rasterizer (Linux)

---

## **1. Motivation**

Now that:

* shaping works (SPEC-065)
* font resolution works (SPEC-049)
* font loading works (SPEC-051)
* native backends are present (SPEC-047)

…Halcyon needs the **unified glyph raster → atlas → quad** pipeline.

Today the renderer still uses:

* Glyphon’s R8 atlas
* Glyphon’s grayscale shader
* No subpixel renderer
* No FreeType/CText/DWrite unification
* No run-to-quad pipeline
* No atlas eviction/packing control

066 replaces that entire legacy structure.

---

## **2. Goals**

### **2.1 Primary**

* One Halcyon glyph pipeline for all OSes
* Deterministic CPU raster output
* Unified `GlyphBitmap` struct
* Unified GPU glyph atlas
* Run → positioned quad → instance generation
* Full support for:

    * LTR/RTL
    * Complex scripts
    * Ligatures
    * Combining marks
    * Arbitrary scale & DPR

### **2.2 Secondary**

* Basic grayscale rendering as foundation
* Subpixel RGB path deferred to SPEC-064
* Support for variable pixel densities
* Deterministic atlas packing
* Stable atlas keys (`font_id`, glyph_index, px_size)

---

## **3. Architecture Overview**

```
         HarfBuzz (shaping)
                 ↓
   ShapedRun { glyph_id, x, y, advance }
                 ↓
     NativeRasterizer (per platform)
                 ↓
         GlyphBitmap { w, h, bearing_x, bearing_y, data[] }
                 ↓
        GlyphAtlas.pack() → atlas_id, uv rect
                 ↓
  Renderer builds quads for every shaped glyph
                 ↓
       Text Shader (grayscale or RGB)
                 ↓
                    Frame
```

---

## **4. CPU Rasterizers (Unified Interface)**

### **4.1 Trait**

```
pub trait NativeGlyphRasterizer {
    fn rasterize_glyph(
        &self,
        face: &ResolvedFontFace,
        glyph_id: u32,
        px_size: f32,
        dpr: f32,
    ) -> Option<GlyphBitmap>;
}
```

### **4.2 Raster Output Standard**

All rasterizers output **the same normalized structure**:

```
pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub advance_x: f32,
    pub data: Vec<u8>,        // R8 grayscale or RGB channels
    pub format: GlyphFormat,  // Grayscale | SubpixelRGB
}
```

### **4.3 Backends**

* Windows → DirectWrite
* macOS → CoreText
* Linux → FreeType

All produce identical `GlyphBitmap`.

---

## **5. GPU Glyph Atlas (Unified GPU Layer)**

### **5.1 Atlas Format**

Two modes:

```
R8Unorm        // grayscale
RGBA8Unorm     // subpixel RGB (SPEC-064)
```

### **5.2 Subpixel Bitmap Semantics**

`GlyphBitmap::SubpixelRgb` does **not** represent a complete visual glyph.
It represents LCD coverage only.
Final appearance depends on sampling the background color and running the platform compositing equations.

Atlas stores coverage; renderer must composite using:

* background color via sampling
* linear-light math
* per-channel coverage

This prevents developers from assuming SubpixelRgb is a normal texture.

### **5.3 Subpixel Horizontal Bins (SubpixelBinX)**

To achieve pixel-stable text rendering (per SPEC-HAL-104), glyphs MUST be cached with awareness of their horizontal subpixel phase.

#### 5.3.1 Concept

Introduce `SubpixelBinX` — a discrete horizontal subpixel phase:

```
pub enum SubpixelBinX {
    Zero,   // 0.00 - 0.125
    One,    // 0.125 - 0.375
    Two,    // 0.375 - 0.625
    Three,  // 0.625 - 0.875
    // Values >= 0.875 wrap to Zero of next integer pixel
}
```

The exact fractional thresholds are tied to the device's logical → physical mapping and MAY be adjusted, but the count (4 bins) is fixed by this SPEC.

#### 5.3.2 Bin Purpose

Different subpixel phases produce visually different rasterizations of the same glyph. By quantizing to 4 bins, Halcyon:

* Limits atlas memory usage (4 variants max per glyph/size)
* Ensures repeated glyphs at the same phase re-use identical bitmaps
* Enables stable text appearance across frames

### **5.4 Atlas Key (Extended)**

The glyph cache key MUST include the subpixel bin:

```
(font_id, glyph_id, px_size, glyph_format, subpixel_bin_x)
```

For formats that do not benefit from subpixel positioning (e.g., SDF/MSDF), `subpixel_bin_x` MUST be fixed to `Zero`.

### **5.5 Structure**

```
pub struct GlyphAtlas {
    texture: wgpu::Texture,
    allocator: ShelfPacker,
    entries: HashMap<AtlasKey, AtlasEntry>,
}
```

### **5.6 Subpixel Quantization Rules**

#### 5.6.1 Mapping Physical X to Bins

Define a deterministic function `quantize_x_to_bin(x_phys: f32)` that returns:

* `x_int_px: i32` — integer pixel position
* `bin: SubpixelBinX` — one of the four phases

```
fn quantize_x_to_bin(x_phys: f32) -> (i32, SubpixelBinX) {
    let x_int = x_phys.floor() as i32;
    let frac = x_phys - x_phys.floor();
    let bin = match frac {
        f if f < 0.125 => SubpixelBinX::Zero,
        f if f < 0.375 => SubpixelBinX::One,
        f if f < 0.625 => SubpixelBinX::Two,
        f if f < 0.875 => SubpixelBinX::Three,
        _ => SubpixelBinX::Zero,  // wraps to next pixel handled by caller
    };
    (x_int, bin)
}
```

#### 5.6.2 Quantization Requirements

* Rounding thresholds MUST be stable and monotonic (no randomness).
* For CJK or other script classes where integer-only positioning is preferred, the policy MAY force `bin = Zero` (integer-only X).
* The quantization function MUST be deterministic: the same `x_phys` value MUST always produce the same `(x_int_px, bin)` pair.

#### 5.6.3 Cache Guarantee

For any given `(font_id, glyph_id, px_size, format, bin)`, the glyph's bitmap is rasterized **at most once** and reused for all occurrences with the same bin. Different bins are allowed to produce different bitmaps of the same glyph.

### **5.7 Atlas Entry with Subpixel Bin**

Each atlas entry MUST store the `subpixel_bin_x` associated with that glyph variant:

```
pub struct AtlasEntry {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub offset_px: (i32, i32),
    pub bitmap_size_px: (u32, u32),
    pub subpixel_bin_x: SubpixelBinX,
}
```

This value is part of the lookup key and MUST NOT be ignored at quad-building time.

SPEC-HAL-104 uses `subpixel_bin_x` to ensure stable mapping from glyph physical X positions into atlas variants; the combination of `x_int_px` and `subpixel_bin_x` ensures repeated glyphs re-use identical bitmaps at identical phases.

### **5.7.1 Reference Safety Rule**

AtlasEntry MUST NOT contain references into shaper output, font parser structures, or paragraph/line allocations. Only raw numeric fields and texture coordinates are allowed.

### **5.8 Insertion Algorithm**

1. Check if atlas contains key (including `subpixel_bin_x`)
2. If not → allocate shelf region
3. Upload bitmap to correct channel format
4. Store `uv_min`, `uv_max`, `offset_px`, `bitmap_size_px`, `subpixel_bin_x`

### **5.9 Max-Size Eviction**

Atlas grows until the renderer maximum texture size. At maximum size, insertion
MAY evict least-recently-used glyph entries and reuse their freed atlas
rectangles.

Eviction MUST preserve UV stability for retained glyphs. Retained entries are not
repacked during eviction; only removed entries release their rectangles. If the
incoming glyph does not fit any released rectangle, eviction continues until a
fit exists or the atlas is empty.

### **5.10 Frame Phase Placement**

As clarified by binding [SPEC-HAL-360](SPEC-HAL-360-Complete-Text-Realization.md),
ordinary text demand, rasterization, atlas admission, final UV lowering and GPU
uploads belong to renderer **Compile** under SPEC-HAL-084 GPU Write Isolation
and SPEC-HAL-359. This supersedes this section's former Scene Build /
between-frames placement. Build remains CPU-only; Execute performs no atlas or
other GPU resource writes. Surface completeness and protected atlas residency
follow SPEC-HAL-360; shaping alone is not realization evidence.

---

## **6. Run → Quad Layout**

### **6.1 Input**

`Vec<ShapedCluster>` from SPEC-065.

### **6.2 Output**

List of **positioned glyph quads**.

```
struct GlyphQuad {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub color: [f32; 4],
}
```

### **6.3 Rules**

* x/y apply shaped cluster offsets
* bearing + atlas rect drive quad origin
* advance accumulates x
* Hebrew/Arabic reverse direction already handled by shaping
* Final positions are always in **absolute logical space** (per SPEC-024)

---

## **7. Renderer Integration**

### **7.1 Vertex Format**

```
position: vec2<f32>
uv: vec2<f32>
color: vec4<f32>
```

### **7.2 Instances**

Renderer uses **instanced drawing**:

* one quad per glyph
* per-glyph instance buffer
* static quad vertex buffer (two triangles)

### **7.3 Fragment Shader (Grayscale Path)**

```
coverage = textureSample(atlas, uv).r;
out = fg_color * coverage;
```

### **7.4 Subpixel Path**

Deferred to SPEC-064 (RGB channels).

---

## **8. Integration With HarfBuzz (SPEC-065)**

066 consumes clusters exactly as 065 provides them:

* cluster order
* advances
* x/y offsets
* glyph IDs
* run direction
* run boundaries

066 does **no shaping of its own**.

---

## **9. Migration & Removal of Legacy Code**

### **Remove Entirely:**

* glyphon::FontSystem
* glyphon::Cache
* Glyphon atlas
* Glyphon shaping paths
* Glyphon text shaders
* Glyphon quad builder

### **Replace With:**

* Halcyon NativeGlyphRasterizer (platform dependent)
* Halcyon GlyphAtlas
* Halcyon run-to-quad path
* Halcyon text shader

---

## **10. Testing Matrix**

**CPU Raster Tests:**

* Latin, Cyrillic, Arabic, Hebrew, Devanagari
* All platforms produce identical bitmap boundaries
* Bearing correctness
* Advance correctness

**Atlas Tests:**

* Packing correctness
* Reuse correctness
* Multi-size correctness

**Rendering Tests:**

* Crisp grayscale AA
* No vertical drift
* No horizontal drift
* No position jitter at fractional Y

---

## **11. Implementation Plan**

### **Phase 1 — CPU Raster Unification**

* Implement `NativeGlyphRasterizer` for:

    * DirectWrite
    * CoreText
    * FreeType

### **Phase 2 — GlyphBitmap Implementation**

* Normalize outputs
* Ensure identical metrics layout
* Encode grayscale and future RGB

### **Phase 3 — GPU Atlas**

* Create texture
* Implement shelf packer
* Implement upload path
* Implement lookup path

### **Phase 4 — Run → Quad Builder**

* Transform clusters to quads
* Implement advance accumulation
* Respect bearings and offsets

### **Phase 5 — Renderer Integration**

* Add text pass to wgpu renderer
* Add instancing
* Add grayscale shader
* Hook into scene graph

### **Phase 6 — Removal of Glyphon**

* Delete glyphon references
* Delete old shaders
* Delete old atlas
* Ensure FontResolver + Shaper are used globally

---

## **12. Implementation Notes**

### **12.1 Completed (v1.1)**

#### Core Types Added

* `GlyphFormat` enum in `src/platform/native_font_backend.rs`
  - `Grayscale` (R8) — current default
  - `SubpixelRgb` — reserved for SPEC-064

* `GlyphBitmap` extended with `format: GlyphFormat` field

* `GlyphKey` extended with `format: GlyphFormat` field
  - `GlyphKey::new()` defaults to Grayscale
  - `GlyphKey::with_format()` for explicit format

#### Run→Quad Builder

* New module: `src/core/glyph_quad.rs`
  - `GlyphQuad` struct per SPEC-066 spec
  - `RunToQuadParams` for builder configuration
  - `build_quad_from_cluster()` core builder function
  - `run_advance_phys()` / `run_advance_logical()` helpers

#### Backend Updates

All native backends now set `format: GlyphFormat::Grayscale`:
* DirectWrite (`native_font_backend_directwrite.rs`)
* CoreText (`native_font_backend_coretext.rs`)
* FreeType (`native_font_backend_freetype.rs`)
* Glyphon (`native_font_backend_glyphon.rs`)

#### Renderer Pipeline

The existing text pipeline in `wgpu_renderer.rs` already implements:
* R8 glyph atlas with `SkylinePacker`
* Text shader with gamma-corrected coverage
* Glyph → quad building via `collect_text_quads()`

### **12.2 Future Work (SPEC-064)**

* Subpixel RGB rendering path
* macOS linear-light blending
* RGBA8 atlas texture support
* Platform-specific subpixel rasterizers
