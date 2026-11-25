# SPEC-HAL-050: Unified Text Shaping Architecture

Bidirectional Text, Ligatures, Clusters, Scripts, Emoji, OpenType Features

Status: Implemented
Author: JC

Note: SPEC-HAL-050 defines the high-level architecture and invariants of Halcyon's unified shaping model.
Implementation details are defined in SPEC-HAL-065 (rustybuzz integration).
SPEC-050 remains part of the active architecture stack.

Depends on:

* **SPEC-HAL-047** (Native Text Rendering Backends)
* **SPEC-HAL-048** (Paragraph & Text Layout Engine)
* **SPEC-HAL-049** (Font Collections & Fallback Rules)
* **SPEC-HAL-006** (Text & Font System)
* **SPEC-HAL-051** (Font Loading & Runtime Font Injection)
* **SPEC-HAL-065** (Complex Text Shaping — rustybuzz)

Related:

* SPEC-HAL-052 (Interactive Text Editing Model: Cursor, Selection, Graphemes)

---

## 1. Summary

This SPEC defines Halcyon's **unified shaping architecture** for all complex-script and OpenType-dependent text.

DirectWrite and CoreText provide shaping for Latin texts, but:

* They cannot produce identical results across platforms.
* They do not expose enough control for:

    * grapheme clusters
    * bidi resolution
    * ligature handling
    * variation selectors
    * advanced OpenType features (liga, calt, kern, rlig, etc.)
    * emoji composition (ZWJ sequences)

This SPEC establishes:

* A unified **rustybuzz-based shaping path** for all backends
* Backend-specific glyph rasterization only
* A platform-agnostic cluster/glyph pipeline
* Bidirectional text direction handling
* A final shaping contract for ParagraphEngine

**TextShaper** (using rustybuzz) is the **single source of truth** for shaping, even if the rasterizer is DirectWrite/CoreText/FreeType.

---

## 2. Motivation

Native APIs differ in too many ways:

* DirectWrite: good Latin support, limited OT feature control.
* CoreText: inconsistent ligature behavior across macOS versions.
* Emoji composition rules vary by OS version.

Without a unifying shaping engine:

* Multi-platform consistency is impossible.
* ParagraphEngine cannot reason about clusters.
* Fallback fonts intermixed with primary fonts produce unpredictable glyph mapping.
* Cursor navigation (SPEC-052) becomes unimplementable.

rustybuzz (pure Rust HarfBuzz port) solves all of this:

* No C dependencies
* No platform-specific bindings
* WASM-compatible
* Byte-for-byte identical shaping to HarfBuzz

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: All text shaping must go through TextShaper (rustybuzz) regardless of platform.
* R2: Native backends are only responsible for:

    * providing font metrics (from LoadedFontFace)
    * providing glyph rasterization (via platform_handle)
* R3: Shaping must support:

    * Unicode bidi rules (direction detection)
    * combining marks
    * ligatures
    * contextual alternates
    * kerning
    * ZWJ sequences
    * emoji composition
* R4: Shaping must produce:

    * glyph clusters
    * physical advances
    * physical offsets
    * glyph IDs consistent with rasterizer backend

### 3.2 Cluster invariants

* I1: A cluster is the minimal indivisible unit for:

    * line breaking
    * cursor movement
    * selection expansion
* I2: rustybuzz cluster boundaries are authoritative.
* I3: No backend may split or alter clusters.

### 3.3 Backend invariants

* B1: Glyph indices produced by TextShaper must correspond to glyph indices expected by backend rasterizer.
* B2: Font fallback must occur before shaping (SPEC-049).
* B3: Metrics must be consistently translated from design units → physical pixels.

### 3.4 Font data invariant

* F1: The same font bytes (`Arc<Vec<u8>>`) must be used by:

    * TextShaper (rustybuzz::Face)
    * Platform rasterizer (PlatformFontHandle)
* F2: FontManager (SPEC-051) is the sole source of font data.

---

## 4. Shaping Integration Model

All platforms use:

```
Paragraph → Runs → TextShaper → Vec<ShapedCluster>
```

### 4.1 TextShaper setup

Each LoadedFontFace (from SPEC-051) provides:

* `data: Arc<Vec<u8>>` — raw font bytes
* `units_per_em: u16` — font design units
* `shaper: Arc<TextShaper>` — pre-constructed rustybuzz shaper

TextShaper internally holds:

* `rustybuzz::Face<'static>` — constructed from font bytes
* `units_per_em` — for coordinate scaling

### 4.2 Per-run workflow

Given (span_text, LoadedFontFace):

1. Get TextShaper from LoadedFontFace

2. Build rustybuzz buffer:

   ```rust
   let mut buffer = UnicodeBuffer::new();
   buffer.set_direction(direction);
   buffer.push_str(text);
   ```

3. Shape:

   ```rust
   let glyph_buffer = rustybuzz::shape(&face, &[], buffer);
   ```

4. Extract and scale:

   ```rust
   let scale = (font_size * device_scale) / units_per_em;
   for (info, pos) in glyph_buffer.glyph_infos().zip(glyph_positions()) {
       // info.glyph_id, info.cluster
       // pos.x_advance * scale, pos.x_offset * scale
   }
   ```

5. Produce `Vec<ShapedCluster>`

See SPEC-HAL-065 §4 for canonical API documentation.

---

## 5. Direction Handling

### 5.1 Current implementation

Direction is detected via first-strong-character heuristic:

* Arabic (U+0600–U+06FF, etc.) → RTL
* Hebrew (U+0590–U+05FF) → RTL
* Latin, Cyrillic, Greek → LTR
* Default → LTR

This is implemented in `TextDirection::detect()` (`text_shaper.rs`).

### 5.2 Future work: Full Unicode TR #9

Full bidirectional algorithm (UAX #9) is not yet implemented:

* Embedding levels
* Explicit directional overrides (LRO, RLO, etc.)
* Paragraph-level bidi resolution
* Visual reordering of runs

When implemented, ParagraphEngine will:

* Resolve embedding levels per paragraph
* Split text into bidi runs
* Shape each run with correct direction
* Reorder runs for visual display

### 5.3 Mixed-script cases

Common examples:

* Latin + Hebrew
* Arabic + numbers
* CJK + emojis

rustybuzz handles all shaping; ParagraphEngine handles:

* run grouping
* logical → visual mapping (when TR #9 implemented)
* line breaking on visual glyph sequence

---

## 6. Interaction With Native Backends

Native backends provide **rasterization only**.

### 6.1 Glyph rasterization

Rasterization is platform-specific:

* Windows: DirectWrite via `PlatformFontHandle::DirectWrite`
* macOS: CoreText (future)
* Linux: FreeType (future)

Backends receive glyph IDs from TextShaper and render bitmaps.

### 6.2 Font data source

All backends use font data from FontManager:

* `LoadedFontFace.data` — shared `Arc<Vec<u8>>`
* `LoadedFontFace.platform_handle` — platform-specific handle created from same bytes

This guarantees glyph ID consistency between shaper and rasterizer.

### 6.3 No backend shaping

Backends must NOT:

* Perform text shaping
* Query OS for font fallback
* Use DirectWrite/CoreText shaping APIs

All shaping flows through TextShaper.

---

## 7. Shaping Output

TextShaper produces `Vec<ShapedCluster>`:

```rust
pub struct ShapedCluster {
    pub glyph_id: u32,       // Glyph ID for rasterization
    pub x_offset_phys: f32,  // X offset from pen position (physical px)
    pub y_offset_phys: f32,  // Y offset from baseline (physical px)
    pub advance_phys: f32,   // Horizontal advance (physical px)
    pub cluster: u32,        // Byte offset in source text
}
```

Coordinate conversion:

```
scale = (font_size * device_scale) / units_per_em
x_offset_phys = pos.x_offset * scale
y_offset_phys = pos.y_offset * scale
advance_phys = pos.x_advance * scale
```

Line breaking uses:

* glyph order (visual)
* cluster boundaries from rustybuzz
* cluster byte offsets for text slicing

---

## 8. Advanced OpenType Features

rustybuzz applies default OpenType features:

Defaults ON (via rustybuzz):

* liga (standard ligatures)
* clig (contextual ligatures)
* calt (contextual alternates)
* kern (kerning)
* mark / mkmk (combining marks)
* rlig (required ligatures)

Defaults OFF:

* stylistic sets (ss01–ss20)
* character variants (cv01–cv99)
* contextual swashes
* historical forms

### Future work: Configurable features

A future SPEC will allow per-run feature control:

```rust
TextStyle { features: Vec<OpenTypeFeature> }
```

Current implementation passes empty features slice to rustybuzz.

---

## 9. Fallback Interaction

Font fallback (SPEC-049) occurs **before shaping**.

### 9.1 Primary resolution

FontResolver resolves style → LoadedFontFace before shaping begins.

### 9.2 Missing glyph fallback

During shaping, if `glyph_id == 0` is detected:

1. ParagraphLayoutEngine identifies missing glyph segment
2. Calls `FontResolver::find_fallback_for_char()`
3. Splits run at segment boundary
4. Re-shapes segment with fallback font

### 9.3 Pipeline

```rust
for span in spans:
    resolved_face = resolver.resolve(span.style)
    clusters = resolved_face.shaper.shape_auto(text, size, scale)
    if has_missing_glyphs(clusters):
        // Split and re-shape with fallback
        runs.extend(shape_with_fallback(...))
```

See SPEC-HAL-049 §7 for implementation details.

---

## 10. Testing Strategy

* Direction detection for Arabic/Hebrew
* Emoji composition sequences (ZWJ)
* Indic scripts (Devanagari basic smoke tests)
* CJK shaping sanity checks
* OT feature tests (ligatures, kerning)
* Mixed-script text
* Line breaking across ligatures
* Cross-platform rendering consistency

Validation harness: `examples/shaping.rs`

---

## 11. Performance Notes

* rustybuzz shaping is extremely fast (pure Rust, no FFI)
* Shape cache per (text, font_size) in TextEngine implementations
* Rasterization remains the bottleneck (unchanged)
* TextShaper reused per LoadedFontFace minimizes overhead

---

## 12. Security Considerations

* Font data validated at load time (SPEC-051)
* rustybuzz is memory-safe (no raw pointers)
* Avoid exponential fallback loops: max fallback depth=3
* Reject invalid UTF sequences before shaping

---

## 13. Relationship to SPEC-HAL-065

SPEC-HAL-065 provides implementation details for rustybuzz-based shaping:

* Defines `TextShaper` and `ShapedCluster` data structures
* Documents rustybuzz integration details
* Specifies coordinate system and scaling
* Provides validation test suite

SPEC-HAL-050 defines the architecture and invariants that all implementations follow:

* High-level architectural context and motivation
* Cluster invariants for cursor/selection (SPEC-052 dependency)
* Backend integration rules (rasterization only, no shaping)
* Future roadmap (TR #9 bidi, OpenType feature API)

Both SPECs are part of the active architecture stack.
