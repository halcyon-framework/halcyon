# SPEC-HAL-048: Paragraph & Text Layout Engine

## Scoped UI typography — accepted 2026-10-10

**Author:** JC. **Decision:** delegated architectural acceptance of the
scoped UI typography contract. **Implementation:** Pending source proof and
independent review. Document-wide status and unrelated Draft/Proposed authority
remain unchanged.

### Pre-Flight

- Objective: explicit resolved family/weight and nonnegative shaped-cluster gaps
  for ordinary Label and both Button text paths, preserving default SystemUI.
- Mandatory startup and authority entry chain completed: live Work Doctrine,
  ENTRY, Constitution/002A; affected 011/024/048/049/051/066/092 §8A,
  092B/141/309/315/354/360, PM-000/011/013/018 and advisory Design Intelligence.
  Draft 047/065/092B/315 are evidence, not blanket implementation authority.
- Governing authority and status: supreme 002/Permanent 002A; existing text owners,
  Binding 354/360 and 092 §8A; this explicitly accepted scoped addition.
- Authority change, if any: logical text carries immutable admitted face backing
  and cluster spacing; selection uses actual ephemeral source-cluster geometry.
- Target layer and existing state verified: original master
  14ab6be4aa321b5c7d9c2d4289d74736c07b6d3c, clean 0.208.0-beta, empty index.
- Owned mechanisms and files: existing core text/shaper/paragraph/selection,
  font manager/resolver/adapters/native admission, Label/Button/projection,
  logical scene/flatten/Compile, nearest tests, affected authority, ENTRY and own
  release metadata; only mechanically necessary existing callers may migrate.
- Named invariants: exact admitted face/backing; whole-string shaping; cluster
  edges; no trailing/intra-cluster gap; logical-only spacing; complete realization;
  first-successor coherence; unchanged phases/input/clips/order.
- Forbidden behavior: widget font discovery, native handles in Send catalogs,
  paint-time registration/IO/rasterization, inserted spaces, glyph-ordinal source
  mapping, silent missing text/clamping, new assets/services or consumer edits.
- Existing mechanisms inspected and reuse decision: FontManager/FontResolver,
  TextStyle/TextShaper, existing native constructors, TextEngine/PaintSink,
  TextNode/FlatPrimitive, Compile atlas, Label/Button and retained fragments.
- Relevant external or dependent boundaries: existing applications retain their accepted fonts;
  no consumer adoption/build or native windows/input during this source window.
- Test ownership and placement: existing inline and registered internal owners;
  exact `ui_typography_` claims and compatibility commands are declared in ENTRY.
- Exact runnable verification commands and expected proof: ENTRY supplies six
  exact sequential Grip invocations; new tests execute nonzero, with real required
  adapter/Compile/atlas/encode/readback distinguished from check success.
- Output or artifact completeness boundary: complete control-to-Compile path,
  focused proof and unstaged candidate for independent review.
- SemVer and changelog impact: additive 0.209.0-beta; own manifest/lock and newest
  substantive Unreleased entry only; no dependency upgrades.
- Git, history-rewrite, destructive, and external effects: no stage/commit/push,
  reset/stash/checkout, installs/grants/deployment or foreign-work restoration.
- Out of scope: font picker/default replacement, variable axes/collections,
  hot reload, new fallback/editor/IME/role taxonomy, assets and native quality.

### Halcyon Pre-Flight

- Governing SPEC and read status: exact text/retained/realization owners above,
  under unchanged Constitution/002A, with this delegated scoped acceptance.
- Widget policy or N/A: Label display/opt-in selection and ordinary Button
  activation, focus, appearance and capture remain unchanged.
- Interaction added and policy authority: none; correct existing selection
  geometry without adding editing or changing opt-in policy.
- Applicable FORBID rules: no cross-widget references, phase mutation, stale
  fragments, glyph layouts in SceneGraph or silent surface incompleteness.
- Existing framework mechanism inspected: resolution, shaping, projection,
  logical emission, retained replacement and Compile/native rasterization.
- Reinvention risk: extend existing owners, no duplicate registry/shaper/service.
- Demo/example compliance: no demo/native action or consumer source change.

### Contract

Application assembly may supply immutable owned FontAsset bytes/metadata through
App bootstrap. FontManager admits them before the first Screen build/measurement.
Omission preserves the existing SystemUI bootstrap bytes and identity. Its
immutable catalog contains no LoadedFontFace native handles or mutable manager.
Native face instances are backend-owner resources from those exact bytes.
FontResolver remains the registered-face selection policy; requested and actual
weight/italic are distinct, including closest-weight fallback. No synthetic
emboldening or new per-codepoint fallback for simple controls is admitted.

Label and standalone Button own optional paragraph TextStyle requests. Child
Button consumes the child's complete ephemeral public Widget text projection,
including size, request and spacing, without downcasting/private fields. Legacy
text_info receives an explicit default projection. Button appearance owns colors
and padding; the metrics-based baseline helper and input behavior remain.

One whole-string shaping/spacing operation serves measurement, metrics, paint
input and Compile. Logical TextNode/FlatPrimitive carry immutable actual face
identity/backing and spacing, without glyph arrays, layout or fallback-run
references. Compile reshapes ephemeral runs and rasterizes the matching native
face. Switching only a shaper without its raster backing is invalid; HAL-360's
complete-surface unavailable outcomes remain truthful.

The gap is finite, nonnegative and logical, zero by default. Shape first with
unchanged kerning/ligatures. Add one gap between consecutive visual groups of
glyphs sharing a source cluster, none within a group or after the last. Empty or
one-group text adds none. Preserve source bytes, offsets, direction, visual order
and whitespace; never sort visual groups by source offset. Invalid/nonfinite or
overflowing input produces typed input/realization failure, never clamping.
This is cluster spacing, not CSS tracking, signed compression or justification.

Selection consumes ephemeral source-cluster ranges/positions. Hits snap to actual
cluster edges, midpoint ties to the visual trailing edge. Partial source ranges
highlight the intersected whole cluster without a final added gap. Byte offsets,
clipboard and opt-in policy remain. Paragraph measurement uses the same spacing;
wrapped lines have no terminal gap, preserving the existing line breaker.

Lawful runtime typography replacement reports LayoutAndPaint and ancestor
composition before the first successor scene; equivalent values are no-ops.
Shaping/metrics/retained/primitive dependencies include exact face/configuration
and gap. Atlas bitmap identity includes actual face; spacing changes positions.
No global cache clear, polling, hot swap or font-transfer worker is admitted.
This scoped request mechanism qualifies Draft 092B's role-only wording without
adopting its taxonomy or any unrelated Draft/Proposed text.

For the motion-enabled HAL-101 §8.4.3 TextField presentation under §8.4.5 only, the text owner may provide one crate-private ephemeral glyph-to-source-range qualification helper. It accepts a complete existing GlyphRun and the current scale and optional existing TextShaper, verifies the exact bound font backing/configuration including units-per-em, and shapes the entire source through existing shape_with_gap with that run's size and binding gap. It must reproduce the supplied run's glyph count/IDs/positions/advances, width and source-cluster table exactly before associating each raw shaping glyph cluster with its actual immutable source range. Missing/mismatched shaper or evidence refuses qualification. No font discovery, substitute face, public trait/schema field, retained glyph/source-attribution buffer or different shaping/spacing policy is admitted. The result is ephemeral and only qualifies the bounded TextField presentation; SceneGraph/Compile retain their complete logical text and actual face/backing contracts. This scoped addition adopts no unrelated Draft/Proposed typography authority.

Attribution — 2026-10-29: the exact coordinated HAL-101 editing-range addition was explicitly accepted under delegated architectural review, with JC's retained final ownership/veto. This scoped addition does not alter unrelated authority status.

The existing crate-private ephemeral whole-string glyph-to-source qualification helper may additionally serve only the source-qualified composite text-entry presentation admitted by HAL-101 section 8.4.6. Its complete-string shaping, exact face/backing/configuration reproduction, actual immutable source-cluster attribution and refusal conditions remain unchanged. No retained glyph/source buffer, public text schema, substring shaping or renderer change is admitted. This permission is unavailable until the lawful editing ownership and session lifetime required by section 8.4.6 are expressly admitted.

### Scoped non-editing source-range ink presentation

JC's delegated architectural decision on 2026-11-18 admits
`TextEngine::qualify_source_range` for current, fully shaped, bound non-editing
text. This is a separate public range operation. The HAL-101 private
glyph-attribution clauses above retain their existing scope. Both owners may
reuse whole-run shape verification internally. The public result exposes
only the exact requested UTF-8 range and its complete clusters in visual
order, each with its original source range and optional run-local outline
ink rectangle. A zero-ink cluster has no rectangle. Both endpoints must be
extended-grapheme and shaped-cluster boundaries. Incomplete source coverage,
unavailable face or outlines, and incompatible run metrics are typed refusal.
The geometry is unscaled, owned and scoped to the qualification that produced
it. It conveys no glyph identity or authority to change placement or input.

`TextEngine::draw_with_source_range_effects` checks disjoint
`TextSourceRangeEffect` values for the same complete run before emitting one
logical TextNode. Each value contains a source range, checked foreground and a
finite scale in `0.90..=1.15`. A non-unit scale requires ink. Its pivot is the
geometric center of the union of original selected ink rectangles. Compile
independently reproduces the binding, full source and actual clusters, then
changes only the covered glyph quads in the same text pass using the existing
oversampled atlas coverage. Advances, line extents, other glyphs, baselines,
clip, Z and input geometry are preserved. The logical scene payload carries
only the effect values and full-run provenance. It retains no glyphs, layout
data, native handles or ink boxes.

For a final registered paragraph run, `ParagraphLayoutEngine::shape_laid_out_run`
reproduces its complete source, admitted face, inherited span direction, glyph
positions, advances and design metrics from the original paragraph and current
engine. A reproduction mismatch is a typed error. Intersect a paragraph-level
range with the original per-face run source ranges and convert each covered
part to that run's byte offsets. Qualify every covered part before emitting the
effect. Keep the original line baseline, visual run order and layout metrics.
Empty effect lists use ordinary drawing. The existing HAL-360 marker policy
stays in force for unaffected ordinary text.

For catalog layout in a widget, read `TextEngine::current_shaping_scale` through
`LayoutContext::text_engine` and supply that exact value as `LayoutParams::scale`.
Reproduce the resulting runs against the same engine while the layout matches
its current shaping inputs. The read reports `UnsupportedTypography` for an
engine without this capability and `InvalidScale` for a nonfinite or nonpositive
scale. App and renderer own target scale selection and settlement.

Status: Implemented
Author: JC
Amended: 2026-09-13 — SPEC-HAL-359 requires explicit producer-owned Z on every visible draw.
Amended: 2026-09-21 — Retired the unimplemented background-bounds/prepass requirement for the current grayscale renderer.

Depends on:

* **SPEC-HAL-006** (Text & Font System)
* **SPEC-HAL-047** (Native Text Rendering Backends)
* **SPEC-HAL-001** (Core Architecture)
* **SPEC-HAL-008** (Layout Engine)

Related:

* **SPEC-HAL-049** (Font Collections & Fallback Rules)
* **SPEC-HAL-051** (FontManager & Font Resolution)
* **SPEC-HAL-065** (TextShaper)

---

## 1. Purpose

This SPEC defines Halcyon's **Paragraph Layout Engine**:

* Multi-line text layout with word/cluster line breaking
* Per-span styling (font size, weight, color)
* Runs grouped by font face
* Integration with TextEngine for shaping
* Proper baseline alignment across runs
* Text alignment (left, center, right)

The ParagraphLayoutEngine is the **canonical** multi-line text layout system. The legacy `LineBreaker` in `text_linebreaker.rs` is deprecated.

---

## 2. Architecture Overview

The paragraph layout pipeline consists of:

```
Paragraph (text + spans)
    ↓
ParagraphLayoutEngine::layout()
    ↓ (receives shaper from TextEngine)
Vec<Line> (runs with metrics)
    ↓
Widget paint() → TextEngine::shape() + draw()
    ↓
SceneGraph (push_text only)
    ↓
Renderer (glyph rasterization)
```

**Key architectural invariant**: The paragraph engine is **font-agnostic**. It receives a shaper from the TextEngine and performs layout operations without embedding, loading, or managing fonts.

---

## 3. TextEngine as the Sole Shaper Source

The **only** allowed source of a `TextShaper` is the `TextEngine`.

### TextEngine Trait

```rust
pub trait TextEngine {
    fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32);
    fn metrics(&mut self, text: &str, font_size: f32) -> TextMetrics;
    fn shape(&mut self, text: &str, font_size: f32) -> GlyphRun;
    fn draw(
        &mut self,
        run: &GlyphRun,
        x: f32,
        y: f32,
        color: Color,
        z_index: ZIndex,
        builder: &mut SceneBuilder,
    );

    /// Optional access to the underlying TextShaper.
    /// Engines that own a shaper (like FontManagerTextEngine) override this.
    fn shaper(&self) -> Option<&TextShaper> {
        None
    }
}
```

### Architectural Laws

1. **ParagraphLayoutEngine always receives the shaper from the TextEngine.**
2. **Widgets NEVER construct shapers.**
3. **Core NEVER includes fonts.** (Font embedding occurs in `platform/`, not `core/`.)
4. **Shaping only occurs via TextEngine.**

Implementations:
- `FontManagerTextEngine` returns `Some(&self.shaper)` from `shaper()`
- `BackendTextEngine` returns `Some(&self.shaper)` from `shaper()`

---

## 4. Paragraph Data Model

### 4.1 Paragraph

```rust
pub struct Paragraph {
    pub text: String,
    pub spans: Vec<Span>,
}
```

### 4.2 Span

```rust
pub struct Span {
    pub range: Range<usize>,  // Byte range into paragraph text
    pub style: TextStyle,
}
```

### 4.3 TextStyle

```rust
pub struct TextStyle {
    pub font_family: FontFamily,
    pub font_size: f32,
    pub weight: FontWeight,
    pub italic: bool,
    pub color: Color,
}
```

### 4.4 Run

A run is contiguous text with uniform style and a single font face.

```rust
pub struct Run {
    pub range: Range<usize>,
    pub style: TextStyle,
    pub font_id: u64,
    pub glyphs: Vec<ShapedGlyph>,  // Logical coordinates
    pub metrics: TextMetrics,
    pub width: f32,
}
```

### 4.5 Line

```rust
pub struct Line {
    pub runs: Vec<Run>,
    pub width: f32,      // Total width in logical pixels
    pub ascent: f32,     // Maximum ascent among runs
    pub descent: f32,    // Maximum descent among runs
    pub height: f32,     // ascent + descent
    pub baseline: f32,   // Offset from line top (equals ascent)
}
```

### 4.6 TextAlign

```rust
pub enum TextAlign {
    Left,    // Default for LTR text
    Center,
    Right,   // Default for RTL text
}

impl TextAlign {
    pub fn offset(&self, line_width: f32, container_width: f32) -> f32;
}
```

---

## 5. Layout Pipeline (with TextEngine::shaper)

### Widget Layout Phase

**TextBlock::layout()** executes:

1. Build `Paragraph::from_plain_text(text, size, color)`
2. Get shaper from `cx.text_engine.shaper()`
3. Call `ParagraphLayoutEngine::layout(&paragraph, shaper, &params)`
4. Store resulting `Vec<Line>` as paragraph lines
5. Compute height as sum of `line.height`

### ParagraphLayoutEngine::layout()

```rust
pub fn layout(
    paragraph: &Paragraph,
    shaper: &TextShaper,
    params: &LayoutParams,
) -> Vec<Line>
```

The layout function performs four stages:

**Stage 1 — Span Segmentation**
- Merge adjacent spans with identical style
- Ensure coverage of full paragraph

**Stage 2 — Run Creation**
- For each span:
  - Shape text using provided shaper
  - Produce `Run { glyphs, metrics, width }`

**Stage 3 — Line Breaking**
- Cluster-aware breaking (not byte/char aware)
- Hard breaks on `\n`
- Soft breaks on whitespace (space, tab)
- Forced breaks when word exceeds max_width

**Stage 4 — Line Assembly**
- Compute line metrics: `ascent = max(run.metrics.ascent)`, `descent = max(run.metrics.descent)`
- `height = ascent + descent`
- `baseline = ascent`

---

## 6. Paint Pipeline (with TextEngine::shape + draw)

**TextBlock::paint()** executes:

1. For each Line in stored lines:
   - Compute x_offset via `TextAlign::offset(line.width, container_width)`
   - Compute line position: `(rect.x + x_offset, rect.y + cumulative_y)`
2. For each Line, extract text substring via run ranges
3. Re-shape substring via `cx.text_engine.shape(&line_text, font_size)`
4. Draw via `cx.text_engine.draw(&run, x, y, color, z_index, scene)`, where the
   layout-owning widget supplies an explicit Z value strictly above every
   overlapping primitive that the text must cover (SPEC-HAL-359 / SPEC-PM-007)
5. Advance cumulative_y by `line.height`

**Note**: No glyph runs are stored in the widget. Shaping occurs at paint time for each line. This accepts redundant shaping for architectural integrity.

---

## 7. Renderer Compositing Boundary

Text layout exposes no background-reconstruction bounds API. The former requirement for:

```rust
fn text_background_bounds(&self) -> Rect
```

was never implemented and is retired. Current grayscale text rendering consumes glyph coverage, color, clip, and producer-owned Z without a background prepass.

This does not change shaping or metrics. SPEC-HAL-064 and SPEC-HAL-067 remain future, not-implemented LCD/subpixel designs; any future layout-facing background-reconstruction contract requires new authority rather than being inferred from this retired API.

---

## 8. Interaction with FontResolver (High-Level Notes)

For multi-font paragraphs with fallback, use:

```rust
ParagraphLayoutEngine::layout_with_fonts(
    paragraph: &Paragraph,
    font_manager: &FontManager,
    params: &LayoutParams,
) -> Vec<Line>
```

This integrates with SPEC-049/SPEC-051:

- Font resolution uses `FontResolver` which queries `FontManager`
- Missing glyph detection (glyph_id == 0) triggers fallback
- Runs are split and re-shaped with fallback fonts
- All font lookup and glyph selection are consistent with FontManager

**ParagraphEngine relies on the TextShaper provided by the active TextEngine. All font fallback, font lookup, and glyph selection are guaranteed to be consistent with FontManager and NativeFontBackend.**

---

## 9. SceneGraph Integration (push_text Only)

### SceneGraph Contract

SceneGraph only stores **logical text nodes** via `push_text`:

```rust
pub struct TextNode {
    pub position: Point,
    pub text: String,
    pub font_size: f32,
    pub color: Color,
    pub z_index: ZIndex,
}
```

### SceneGraph Must NOT Store

- Glyph IDs
- Glyph positions
- Font fallback information
- Pre-shaped glyph data
- Per-glyph SceneNode variants

**No `PreShapedTextNode` or equivalent structures are ever permitted.**

The SceneGraph is a logical representation. All shaping and glyph-level operations occur in the Renderer.

---

## 10. Renderer Integration (Unchanged)

The Renderer receives `SceneGraph` containing `TextNode` entries.

For each TextNode:

1. Shape text using platform backend
2. Rasterize glyphs on-demand into atlas
3. Build GPU quads with atlas UV coordinates
4. Apply pixel-perfect snapping at physical coordinates

The Renderer owns the glyph atlas and performs all physical-coordinate operations.

---

## 11. Removed Legacy Mechanisms

The following are **removed** or **deprecated**:

| Removed | Replacement |
|---------|-------------|
| `LineBreaker` | `ParagraphLayoutEngine` |
| `LineModel` | `Line` struct |
| Embedded fonts in core | Fonts in `platform/` only |
| Scene-node-level shaping | Renderer-level shaping |
| Glyph-level scene nodes | `TextNode` with logical text |
| `embedded_shaper()` helpers | `TextEngine::shaper()` |
| OS fallback in layout | `FontResolver` in platform |
| fontdb/glyphon in paragraph | TextShaper abstraction |

---

## 12. Implementation Guarantees

### Positional Invariants

1. **Shaping produces absolute logical positions inside a Line**
2. **Drawing uses `(x, y)` based on widget rect**
3. **Renderer performs pixel snapping at physical coordinates**
4. **No glyph-level absolute positioning is stored anywhere upstream of Renderer**

### Alignment & Measurement Rules

1. Horizontal alignment computed via `TextAlign.offset(width, container_width)`
2. Lines carry: `width`, `height`, baseline metrics from `TextMetrics`
3. Layout remains fully in logical pixels throughout the paragraph engine

### Strict Warning Clause

> **No part of the text system may bypass TextEngine for shaping or font resolution. SceneGraph and Renderer must never contain glyph-level structures. ParagraphEngine is font-agnostic and must never embed or load fonts.**

---

## 13. Future Extensions (RichText Spans)

Future SPECs will add:

- **SPEC-HAL-050**: HarfBuzz integration for complex shaping (RTL, ligatures)
- **Rich text spans**: Mixed styles within paragraphs with proper run splitting
- **Inline elements**: Images, links, custom widgets within text flow
- **Selection/editing**: Cursor positioning and text selection APIs

The current architecture supports these extensions without breaking changes.

---

## Appendix: Implementation Location

| Component | Location |
|-----------|----------|
| TextEngine trait | `src/core/text_engine.rs` |
| ParagraphLayoutEngine | `src/core/paragraph.rs` |
| TextShaper | `src/core/text_shaper.rs` |
| FontManagerTextEngine | `src/platform/font_manager_text_engine.rs` |
| FontResolver | `src/platform/font_resolver.rs` |
| FontManager | `src/platform/font_manager.rs` |
| TextBlock widget | `src/widgets/text_block.rs` |
| SceneGraph | `src/core/scene.rs` |

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under coordinated architectural adoption, with JC's final ownership and veto retained. The retained adoption record identifies the exact three source bindings and adopted scope. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.

### Scoped missing-glyph attribution for HAL361 section 6.2 - 2026-11-13

**Status: Selected / Binding, diagnostic reuse only.** Under JC's explicit
required-text safeguard mandate, the existing crate-private
`glyph_source_ranges` helper may also qualify the FIRST current MissingGlyph
witness for HAL361 section 6.2. This extends only the caller restriction at
lines 114-118; every exact whole-string, face/backing, units-per-em, scale,
glyph/position/advance and cluster-table equality check remains mandatory.

The backend owner supplies the actual shaper selected for that failed
GlyphRun's binding. Never substitute the default shaper for a different bound
face. An absent binding/shaper or any mismatch produces unavailable attribution,
not a guessed character or another realization outcome. The original HAL360
refusal and its exact frozen primitive/glyph indices remain authoritative.

Use the confirmed failed glyph index to select its verified source byte range;
retain at most that range, eight scalar values with explicit truncation,
mechanical font identity/configuration and same-attempt coordinates. Keep the
run, source text, complete mapping and font bytes ephemeral. Never format the
whole GlyphRun, FontFaceBinding or backing bytes into diagnostics. This is
first-fault evidence under the existing bounded owner, not a per-frame
reshaping pass, public API, retained source map, cache replacement or fallback.

A fresh current refusal establishes its own identity. It cannot backfill the
unrecorded primitive/mount/face of an earlier process. Missing historical widget
attribution does not prevent capture of the current actual cluster and face.
The helper's normal editor callers and all rendering/admission semantics stay
unchanged.

### Registered TextBlock face-run preservation - 2026-11-13

**Status: SELECTED / BINDING.** HAL049 section15 selects exact registered
coverage and cluster-safe fallback for the existing TextBlock paragraph path.
Its named catalog/bound-shaping TextEngine additions reuse the same original
native adapters and the immutable HAL051 font admission. This is an explicit
TextBlock-scoped extension of the earlier typography caller scope, not a new
simple-control or OS fallback policy.

Whole-string shaping in this scope means each complete final resolved face run:
measurement, selection, paint input and Compile use the same exact run source,
admitted face/backing, size and cluster gap. A multi-face line MUST NOT be joined
and reshaped with its primary face at paint. Preserve original logical source
ranges separately from visual order and keep clusters indivisible. A failed
fallback MUST retain the unsupported source and the original HAL360 refusal.
Canonical line height/baseline and normal wrapping remain unchanged; align each
run's actual ascent to that baseline. No renderer glyph/layout persistence,
font discovery during Layout/Paint, scene schema change or authority transfer.

The governing decisions and complete verification gate are HAL049 section15
and SPEC-SOL-034 section24. The prior S4 safeguard is preserved, not rerun as an
independent diagnostic task and not mistaken for the completed product repair.

### Normal-path missing-cluster evidence - 2026-11-13

**Status: SELECTED / BINDING.** For HAL360 section7, extend the existing
glyph_source_ranges verifier's caller scope from first-fault diagnostics to
Compile-local missing-cluster containment. The backend supplies the actual
admitted shaper for the current GlyphRun binding. Preserve every whole-run,
font-bytes, size/scale/gap, glyph-position/advance and source-range equality
check. Perform this qualification only for runs with actual missing glyphs,
once per such Compile-local run, not for healthy text or once per glyph.

Return ephemeral cluster-to-glyph ranges and original cluster geometry to the
existing text-realization owner. No mapping, source text or font bytes enter
persistent diagnostics or renderer history. Failure to qualify is typed
Unavailable. Geometry-only fault markers are not characters, font glyphs,
clipboard text or replacement application state. Layout/caret/selection
authority and the bound face for every actual glyph remain unchanged.

The prior registered TextBlock face-run repair proceeds first and is not
replaced by marker handling. Framework markers cover unresolved ordinary text
from all existing consumers, with no per-application error-handler requirement.

### Native MarkdownView bound-run reuse - 2026-11-13

**Status: ADOPTED / ACTIVE within HAL390 section14.** Extend only the
registered TextBlock face-run caller scope to the new framework-owned
MarkdownView. Its immutable MarkdownDocument carries no font/layout object.
The widget uses original Paragraph/Span, catalog-based layout, actual admitted
face/direction bindings, TextEngine bound metrics/shaping, source-cluster
hit/highlight helpers and the same logical scene/Compile transport.

Retain whole-cluster fallback, exact source ranges, actual run styles and
canonical baselines. Never concatenate mixed-face runs for default-font paint.
Underline/strike/background decorations remain separate owner-produced
geometry; no fabricated TextStyle fields, glyph data in SceneGraph, native
handles in documents, font discovery during layout/paint or new renderer.
A crate-private factoring of TextBlock's exact bound-run operation is admitted;
no new public arbitrary-run-painter interface is selected. Other controls,
editor/IME policy, HAL360/361 outcomes and existing TextBlock behavior remain
unchanged. HAL390 supplies the complete input/style/extent/acceptance contract.

### Registered editable-cache and paint coherence

**Status: ADOPTED / ACTIVE under HAL049 section18.** TextLayoutCache and its
existing TextField/TextArea consumers join the registered bound-run path. The
same resolved Lines and original source-cluster map govern editor geometry,
HAL383 validation and actual painted face runs. This is the scoped exception
to the earlier simple-control/single-shaper caller restriction, not a new
TextEngine, resolver, renderer or editor semantics.

Use the original immutable catalog, paragraph layout, exact TextBinding and
bound shaping/metrics. Preserve phase ownership, logical units, canonical
baselines, source direction and original bytes. Password presentation remains
mask-only. Existing motion must qualify against the actual layout and otherwise
use its existing canonical fallback, without global disablement or new effects.
HAL049 section18 owns the complete scope, failure rules and regression gate;
HAL052/107/108/109 and HAL383 retain their original semantic/phase authority.

### Scoped Apple source-range outline realization (2026-11-26)

**Accepted / Binding clarification under the existing HAL-048 source-range ink contract.** The observed macOS SystemUI text can have valid admitted shaping and native rasterization while `ttf_parser::Face::glyph_bounding_box` cannot supply the selected glyph outline. Such parser unavailability is not authority to substitute another font, use advance or line boxes as ink, or discard a valid native outline. Preserve the existing shared `QualifiedRun` qualification, full-run provenance, source/cluster checks, original layout, and existing ttf-parser results. On Apple targets only, when a selected non-whitespace monochrome glyph has no supported parser outline, qualify the actual vector outline via CoreText using a `CGFont` and `CTFont` constructed strictly from the same admitted immutable `FontFaceBinding` bytes at the run's logical font size, through the existing byte-bound native construction mechanism. Require a real nonempty vector path for that exact glyph ID and finite positive path bounds; no font lookup, fallback face, native shaping, raster/advance/line-box approximation or acceptance of color, SVG or bitmap glyphs. Use the existing run placement: for logical y-up outline bounds `(x_min,y_min,x_max,y_max)`, run-local rectangle is `(glyph.x+x_min, run.ascent+glyph.y-y_max, x_max-x_min, y_max-y_min)`. Keep the existing no-ink whitespace and all typed refusals, including unqualified/missing glyph, unsupported outline and invalid geometry. Construct the native outline reader lazily at most once per ephemeral qualified run, not per glyph; native handles remain local and never cross catalog, Scene or frame custody. Use that same private read path for Paint qualification/effect admission and the independent Compile reproduction through `SourceInkPaint::realize`; no new public API, scene payload, renderer-specific policy or change to non-Apple targets. Existing parser-supported paths remain byte/geometry equivalent. This is a HAL-048 defect correction, not adoption of unrelated Draft backend requirements. Qualification requires the real current macOS SystemUI heading, exact run and selected range, preservation of unselected glyph positions/coverage and ordinary text, actual native GPU readback through Compile/Execute, unchanged rejection cases and non-Apple compile. No claim of physical iOS or other-platform runtime proof without observation.
