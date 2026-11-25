# SPEC-HAL-047: Native Text Rendering Backends

DirectWrite / CoreText / FreeType

Status: Draft
Author: JC
Depends on: SPEC-HAL-006 (Text & Font System), SPEC-HAL-001 (Core Architecture), Renderer SPECs (040–041)
Planned follow-ups: SPEC-HAL-048 (Paragraph/Layout Engine), SPEC-HAL-049 (Font Fallback & Collections), SPEC-HAL-050 (HarfBuzz Integration)

---

## 1. Summary

This SPEC defines Halcyon’s **native typography backend architecture**.

Halcyon moves away from the prototype Glyphon pipeline and introduces a **stable, cross-platform text subsystem** built around a single trait:

```
NativeFontBackend
```

Backend implementations:

* **DirectWriteBackend** (Windows)
* **CoreTextBackend** (macOS)
* **FreeTypeBackend** (Linux / BSD / other)
* **GlyphonBackend** (compatibility fallback)

This SPEC also introduces:

* `BackendTextEngine<B>` → a thin wrapper implementing the existing `TextEngine` interface.
* A unified glyph pipeline (shape → rasterize → atlas).
* Strictly defined coordinate/metric rules matching Halcyon’s layout/render separation.

The renderer, layout engine, and widget system remain untouched.

---

## 2. Motivation

Halcyon's current text pipeline is built around Glyphon:

* Useful for prototyping.
* Not suitable long-term.

Problems:

* Inconsistent rasterization fidelity across platforms.
* Glyph positions differ subtly from native APIs.
* Font fallback unreliable and inconsistent.
* Metrics mismatch with OS-rendered text.
* Redundant shaping logic (Widget path vs Renderer path).
* Suboptimal atlas integration (rasterization happens deep inside renderer).

Halcyon needs a **first-class** text system that:

1. Uses native OS font and rasterization APIs.
2. Supports shaping engines (HarfBuzz) where required.
3. Integrates cleanly with GPU-based atlas & renderer.
4. Does not leak platform-specific details into the render pipeline.
5. Preserves Halcyon’s logical/physical coordinate split.

This SPEC defines that system.

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: Text shaping must produce:

    * Glyph IDs
    * Physical glyph positions
    * Logical metrics
* R2: Rasterization must produce:

    * Grayscale R8 bitmaps
    * Physical-size metrics
* R3: Backends must be fully swappable behind a single trait.
* R4: Renderer must not contain backend-specific logic.
* R5: `TextEngine` must continue to work without modification by widgets.
* R6: Metrics must be stable, DPI-correct, and backend-agnostic.

### 3.2 Platform requirements

* P1: Windows → DirectWrite for shaping + raster.
* P2: macOS → CoreText for shaping, CoreGraphics for raster.
* P3: Linux → FreeType for raster, HarfBuzz for shaping (later SPEC).
* P4: Fallback → GlyphonBackend for unsupported platforms or debugging.

### 3.3 Ordering & structural invariants

* I1: **shaping always happens before rasterization**.
* I2: Shaping is pure and deterministic for the same (text, font, size, scale).
* I3: Rasterization is deterministic for the same `GlyphKey`.
* I4: `GlyphKey` must uniquely describe a glyph across all platforms.
* I5: Renderer uses **physical** coordinates; layout uses **logical** only.
* I6: Backends must respect scale factor passed in (`logical → physical`).

### 3.4 Safety invariants

* S1: No backend may call GPU APIs.
* S2: No backend may mutate the atlas.
* S3: Backends may cache glyphs internally, but cache keys must be deterministic.
* S4: No backend may rely on `Instant::now()`; shaping must be time-invariant.

---

## 4. Current Architecture Overview

### 4.1 Key components today

* `TextEngine` trait (measure/metrics/shape/draw).
* `GlyphonTextEngine` implementing `TextEngine` using Glyphon:

    * `FontSystem`
    * `Buffer`
    * `SwashCache`
* `wgpu_renderer.rs` contains **duplicate shaping logic** using glyphon.
* `GlyphAtlas` consumes `(GlyphKey, bitmap, GlyphMetrics)`.

### 4.2 Problems

* Renderer and TextEngine both shape → duplicate logic.
* Rasterization happens ad-hoc: `text_engine.rasterize_glyph()` not in trait.
* No clean separation of logical/physical units.
* Glyphon abstractions leak into renderer.
* Not extensible to native APIs without major rewrite.

---

## 5. Target Architecture Overview

The SPEC introduces three absolutely central types:

```
NativeFontBackend
BackendTextEngine<B>
DefaultNativeFontBackend (cfg-chosen)
```

### 5.1 NativeFontBackend

Responsible for:

* **Shaping**
* **Rasterization**

Backend must return:

* `Vec<ShapedGlyph>`
* `TextMetrics`
* `GlyphBitmap` on raster requests

### 5.2 BackendTextEngine<B>

Implements existing `TextEngine` trait but delegates all work to backend.

Widgets and layout engine see no difference.

### 5.3 Renderer pipeline

Renderer now does:

1. Call backend `shape_text()`.
2. For each glyph:

    * Lookup atlas by GlyphKey.
    * On miss → ask backend to `rasterize_glyph()`.
3. Build quads using physical coordinates divided by scale.

### 5.4 No backend-specific logic in renderer

Renderer stays dumb:

* Knows GlyphAtlas.
* Knows how to draw a TextQuad.
* Knows only `NativeFontBackend`.

---

## 6. Detailed Specification: NativeFontBackend

### 6.1 Trait

```rust
pub trait NativeFontBackend {
    fn shape_text(
        &mut self,
        text: &str,
        font_size: f32,  // logical
        scale: f32        // logical → physical
    ) -> (Vec<ShapedGlyph>, TextMetrics);

    fn rasterize_glyph(
        &mut self,
        key: &GlyphKey,
        scale: f32
    ) -> Option<GlyphBitmap>;
}
```

### 6.2 ShapedGlyph

```
pub struct ShapedGlyph {
    pub key: GlyphKey,

    // physical pixel coordinates
    pub x_phys: f32,
    pub y_phys: f32,
    pub advance_phys: f32,
}
```

Invariants:

* Origin is at text baseline, leftmost of run.
* `x_phys` and `y_phys` match corresponding offset in rasterization.
* All values pre-multiplied by `scale`.

### 6.3 GlyphKey semantics

```
pub struct GlyphKey {
    pub font_id: u64,
    pub font_size_scaled: u32,
    pub glyph_id: u32,
    pub dpi_scaled: u16,
}
```

Rules:

* `font_id` must uniquely identify a font face.
* `font_size_scaled = (font_size * scale * 64.0) as u32`
* `dpi_scaled = (scale * 96.0) as u16`
* `glyph_id = platform glyph index (u32)`

### 6.4 GlyphBitmap

```
pub struct GlyphBitmap {
    pub key: GlyphKey,
    pub metrics: GlyphMetrics,
    pub pixels: Vec<u8>, // R8 alpha, width*height bytes
}
```

`GlyphMetrics`:

* width / height in physical px
* ascender / descender / line_height in logical px
* advance / bearings in physical px

### 6.5 TextMetrics (logical units)

```
pub struct TextMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
    pub line_height: f32,
}
```

Backends must compute these from native font metrics.

---

## 7. Backend Implementations

### 7.1 GlyphonBackend (compatibility)

* Wraps glyphon shaping/rasterization to meet trait.
* Required for immediate migration.
* Serves as fallback for unsupported platforms.

### 7.2 DirectWriteBackend (Windows)

Shaping:

* Use `IDWriteTextLayout` or `IDWriteTextAnalyzer`.
* Extract `DWRITE_GLYPH_OFFSET`, advances.
* Convert to physical px via `pixels_per_dip = scale`.

Raster:

* Use `IDWriteGlyphRunAnalysis`.
* Extract grayscale alpha texture.

Font identification:

* Use `IDWriteFontFace::GetIndex()` + `IDWriteFont` pointer hash.

### 7.3 CoreTextBackend (macOS)

Shaping:

* Create `CTLine`, obtain CTRun array.
* Extract glyph IDs, positions.

Raster:

* Use CoreGraphics to draw glyph into grayscale bitmap context.

Font identity:

* CTFontRef pointer identity.

### 7.4 FreeTypeBackend (Linux / BSD)

Shaping:

* Requires HarfBuzz (SPEC-HAL-050).
* Extract glyph positions and cluster info.

Raster:

* `FT_Load_Glyph` + `FT_Render_Glyph(FT_RENDER_MODE_NORMAL)`.

Font identity:

* `FT_Face` pointer + style tuple.

---

## 8. Renderer Integration

### 8.1 Renderer no longer shapes text

All shaping moved to backend.

### 8.2 Render flow

Per `SceneNode::Text`:

1. Call:

   ```
   let (glyphs, _) = backend.shape_text(text, font_size, scale);
   ```
2. For each glyph:

    * Check atlas.
    * On miss:

      ```
      let bitmap = backend.rasterize_glyph(&glyph.key, scale);
      atlas.insert_glyph(...)
      ```
3. Compute logical quad rects:
   `x_logical = x_phys / scale`

### 8.3 No special cases per backend

Renderer code is identical across all platforms.

---

## 9. Migration Plan

### Phase 0 — Introduce NativeFontBackend + GlyphonBackend

* Add trait.
* Move current shaping/raster code into GlyphonBackend.
* Wire renderer to use backend instead of glyphon directly.
* No visible change.

### Phase 1 — Add DirectWriteBackend (Windows)

* Implement shaping/rasterization.
* Add runtime flag to switch between Glyphon and DWrite.
* Begin platform testing.

### Phase 2 — Make DirectWrite default on Windows

* Fallback remains Glyphon.

### Phase 3 — Add CoreTextBackend (macOS)

* Implement shaping/rasterization.
* Make default.

### Phase 4 — Add FreeTypeBackend (Linux/BSD)

* Implement shaping/rasterization.
* Integrate HarfBuzz (SPEC-050).

### Phase 5 — Remove glyphon from renderer entirely

* GlyphonBackend stays for debug fallback only.

---

## 10. Testing Strategy

* Golden rendering images per backend.
* Glyph position diffing against platform-native reference.
* Metrics correctness tests:

    * ascent/descent/line_gap consistency
    * line-height logic
    * advance summation
* Visual tests: kerning pairs, ligatures, combining marks.
* Atlas consistency tests across scale factors.

---

## 11. Performance Notes

* Shaping should be cached per (text, font, font_size, scale).
* Rasterization triggered only on atlas miss.
* DWrite and CoreText automatically benefit from OS font caches.
* FreeType backend must maintain internal glyph cache.

---

## 12. Security Considerations

* Font loading APIs may load untrusted fonts.
* Shaping must handle malformed Unicode safely.
* FreeType must be built with safe config flags (no deprecated bytecode interpreter modes).
* No backend may allocate GPU resources directly.
