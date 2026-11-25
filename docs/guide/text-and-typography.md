---
doc-id: halcyon-guide-text
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-002
  - SPEC-HAL-006
  - SPEC-HAL-029
  - SPEC-HAL-034
---

# Text and Typography

Shaping → rasterization → atlas → GPU path at revision
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`. Two-axis tags per SPEC-HAL-309
§7. SPEC-HAL-006 (Text & Font System) is `Draft` — intent, not implemented
truth [authority: draft-intent — SPEC-HAL-006:6].

## 1. Ownership Map

```text
UI thread: TextShaper (rustybuzz)  — shaping, the ONLY shaping engine
    src/core/text_shaper.rs:1-14
TextEngine trait                    — measure/metrics/shape/draw contract
    src/core/text_engine.rs:221-271
FontManagerTextEngine               — UI-thread engine, font identity
    src/platform/font_manager_text_engine.rs:1-25
Platform font backends              — rasterization ONLY
    src/platform/native_font_backend_{coretext,directwrite,freetype,glyphon}.rs
GlyphAtlas                          — GPU-resident glyph storage
    src/renderer/glyph_atlas.rs:1-40
Text pipeline                       — batched UV-mapped quads
    src/renderer/pipelines/mod.rs:7-9, src/renderer/text_vertex.rs
```

## 2. Shaping — rustybuzz, Single Engine

Shaping is unified on rustybuzz (`rustybuzz = "0.20"`, `Cargo.toml:19`):
"This module is the ONLY shaping engine in Halcyon. Native backends
(DirectWrite, CoreText, FreeType) perform rasterization only"
[implementation: implemented — `src/core/text_shaper.rs:1-14`, SPEC-HAL-065
module tag]. Capabilities per module doc: Unicode-correct shaping, RTL/LTR
with first-strong-character detection, ligatures/contextual forms, cluster
mapping, complex scripts, ZWJ emoji sequences; horizontal text only (no
TTB) [implementation: implemented — `src/core/text_shaper.rs:3-25`].

Note for readers of legacy material: the retired top-level overview claims
"HarfBuzz shaping" — the live engine is rustybuzz (a Rust port of
HarfBuzz's algorithm, a different dependency). Recorded for audit.

## 3. The TextEngine Contract

`TextEngine` (`src/core/text_engine.rs:221-271`)
[implementation: implemented]:

- `measure(text, font_size) -> (w, h)` — dimensions without full shaping.
- `metrics(text, font_size) -> TextMetrics` — ascent/descent/line height.
- `font_metrics(font_size)` — font-global metrics independent of content;
  "use this for layout calculations where metrics must not vary with text
  content" (defaults to `metrics("")`).
- `shape(text, font_size) -> GlyphRun` — full layout/BiDi/shaping.
- `draw(run, x, y, color, sink)` — **x, y are the top-left origin, not the
  baseline**; per the trait doc, "baseline-origin semantics will be
  introduced in the typography overhaul"
  [implementation: implemented, with declared pending change].
- `draw_with_backgrounds(...)` — SPEC-EXIT-A1: background spans render
  before glyphs so selection highlights and carets sit behind text without
  z-index hacks.
- `shaper()` — optional access to the owned `TextShaper`.

Consumers reach the engine through the renderer
(`renderer.text_engine_mut()`, e.g. `src/core/app.rs:11118`) or the prelude
export `platform::font_manager_text_engine` (`src/lib.rs:33`).

## 4. FontManagerTextEngine and Font Identity

The UI-thread engine (`src/platform/font_manager_text_engine.rs:1-25`)
[implementation: implemented]:

- SPEC-HAL-103 Stage 2: "single-font text engine with generic font
  identity"; it "uses the legacy embedded Inter font (until OS font
  discovery is implemented)" — a self-declared interim state
  [implementation: partial by its own doc].
- Font identity is computed from
  `FontIdentity { family_name, postscript_name, weight, italic }` so that
  UI-thread shaping and platform-thread rasterization produce identical
  `font_id`s — the engine "does NOT hold or interact with FontManager at
  runtime"; it only guarantees `GlyphKey` compatibility
  [implementation: implemented — `:8-10`, `:31-45`].
- Shape cache capacity: 4096 entries (`SHAPE_CACHE_CAPACITY`, `:25`).
- Multi-font with fallback goes through
  `ParagraphLayoutEngine::layout_with_fonts(&FontManager)` per the module
  doc (`:12-13`) — the paragraph path, not this engine.

Platform inventory (`src/platform/`): `font_manager`, `font_loader`,
`font_resolver`, `system_font_provider`, `backend_text_engine`, and
per-OS rasterization backends — CoreText (macOS), DirectWrite (Windows),
FreeType (Linux), glyphon (WASM fallback per `Cargo.toml:31` comment)
[implementation: implemented — file list under `src/platform/`]. Backends
are transport/rasterization; they carry no shaping or layout semantics
(§2).

## 5. Glyph Atlas

SPEC-HAL-029 declares `Implemented`, and names the live modules:
`glyph_atlas.rs` (GlyphAtlas, GlyphKey, AtlasEntry, SkylinePacker) and the
wgpu text pipeline/bind groups
[authority: SPEC-HAL-029:5, :13-17 | implementation: implemented].

Live atlas facts (`src/renderer/glyph_atlas.rs:1-40`):

- GPU-resident storage eliminating per-frame re-rasterization; text reduces
  to UV-mapped quads (SPEC-HAL-029:23-28).
- SPEC-HAL-066: dual formats — grayscale `R8` and subpixel `RGBA8` — keyed
  by `(font_id, glyph_id, px_size, format)` (`:6-7`).
- SPEC-HAL-066/104: horizontal subpixel phase quantization into 4 bins
  (`SubpixelBinX`) so different subpixel phases get separate atlas entries
  (`:33-40`).
- Upload alignment honors wgpu's 256-byte `bytes_per_row`; glyph bitmaps are
  padded (1px sides/top, 2px bottom descender headroom) against cut-off and
  bleeding (`:16-31`).
- Storage sits in the `GpuResourceArena` with `FrameEpoch`-based lifetimes
  (`:11-12`; epoch semantics in `rendering-and-compositor.md` §5).

## 6. Batched Text and Its Limits

Text batches through the atlas under the batcher's deterministic rules
(`rendering-and-compositor.md` §4). What is **not** implemented: the
SPEC-HAL-034 merging layer — its own status says "Text nodes are collected
individually in `collect_text_quads`. The advanced merging and path
optimization described in this SPEC is not implemented"
[authority: design-intent — SPEC-HAL-034:5 (`Status: Design – Not
Implemented`) | implementation: absent — SPEC-HAL-034:17-19]. Do not build
on merged-text assumptions.

## 7. Text Editing and Layout Neighbors

Core text modules beside the engine (`src/core/`): `text_linebreaker.rs`
(line breaking; prelude export `src/lib.rs:29`), `text_selection.rs`,
`text_edit_delegate.rs`, `text_edit_policy.rs`
[implementation: implemented — file list `src/core/text_*.rs`]. The
`editing` module exposes `EditorBridge` (`src/editing/mod.rs:1-3`). Text
pipeline ephemerality is constitutional: layout/shaping artifacts must not
escape their phase [authority: binding — SPEC-HAL-002 §4.8].

## 8. What This Document Does Not Claim

- No rendering-latency or shaping-throughput numbers — no measurement was
  captured at this revision (`unknown`).
- `GlyphRun` and `TextMetrics` shapes are now verified:
  `GlyphRun { glyphs: Vec<Glyph>, width, height, text, font_size, ascent,
  descent }` (`src/core/text_engine.rs:20-34`) and
  `TextMetrics { ascent, descent, line_gap, line_height }` (`:208-216`)
  [implementation: implemented]. Paragraph layout and editing-substrate
  details remain cited only at the entry points above; deeper item
  documentation belongs to Rustdoc beside the code (SPEC-HAL-309 §2.5).
