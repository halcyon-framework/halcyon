# SPEC-HAL-049: Font Collections, Fallback Rules & Font Resolution

Status: Implemented

Scoped addition (JC, delegated technical A1 acceptance, 2026-10-10): HAL-048's
dated UI typography contract adds an immutable FontManager face-catalog view for
the existing resolver/adapters, with exact backing and actual metadata, no native
handles or mutable manager. Requested and actual style remain separate; existing
registered-face matching and truthful closest-weight fallback apply. No synthetic
weight or new per-codepoint fallback is admitted. This addition is pending source
proof and independent review, without changing the document-wide status.
Author: JC

Depends on:

* **SPEC-HAL-006** (Text & Font System)
* **SPEC-HAL-047** (Native Text Rendering Backends)
* **SPEC-HAL-048** (Paragraph & Text Layout Engine)
* **SPEC-HAL-051** (Font Loading, Runtime Injection & Asset Management)
* **SPEC-HAL-065** (TextShaper / rustybuzz)

---

## 1. Summary

This SPEC defines the **authoritative mechanism** for resolving fonts in Halcyon:

* From **logical family names** to **concrete LoadedFontFace objects**
* Across **custom fonts** and **embedded fonts**
* With **deterministic fallback rules**
* With **no dependence on platform-native fallback**
* Ensuring **the same font bytes** are used by both:
  * rustybuzz (shaping via TextShaper)
  * NativeFontBackend (rasterization)

FontManager (SPEC-051) is the **single source of truth for all fonts**.

---

## 2. Motivation

The FontManager model guarantees:

1. **Deterministic font choice** across all platforms
2. **100% consistent shaping + rasterization** using the same bytes
3. **Stable font_id** computed from font data hash
4. **Predictable fallback** independent of OS
5. **Works everywhere:** Windows, macOS, Linux, WASM

Font selection flows through:

```
FontManager → FontResolver → ParagraphLayoutEngine → TextShaper → NativeFontBackend
```

No platform-native fallback is allowed.

---

## 3. Requirements & Invariants

### 3.1 Functional Requirements

* R1: Resolve a `TextStyle` into one concrete LoadedFontFace
* R2: Support ordered fallback stacks (FontStack)
* R3: Support per-run missing-glyph fallback during shaping
* R4: Deterministic font_id from font data

### 3.2 Structural Invariants

* I1: FontResolver consults only FontManager, not OS APIs
* I2: Style-based fallback occurs before shaping
* I3: Missing-glyph fallback occurs during shaping (glyph_id == 0 detection)
* I4: Resolution does not depend on locale, OS language, or system preferences

### 3.3 Font Identity

* F1: Identical font assets → same font_id
* F2: Different assets → different font_id
* F3: font_id is computed via SHA-256 hash of font data

### 3.4 Coverage

LoadedFontFace provides:

```rust
fn supports(&self, codepoint: char) -> bool
```

CoverageMap provides:

```rust
fn covers(&self, ch: char) -> bool
```

Used for fallback font selection.

---

## 4. Data Model

### 4.1 FontFamily (Logical)

```rust
pub enum FontFamily {
    Named(String),   // "Inter", "Noto Sans"
    Serif,
    SansSerif,
    Monospace,
    SystemUI,
}
```

### 4.2 FontStack

```rust
pub struct FontStack(pub Vec<FontFamily>);
```

Ordered list of families for fallback resolution.

### 4.3 FontDescriptor

```rust
pub struct FontDescriptor {
    pub family: FontFamily,
    pub weight: FontWeight,
    pub italic: bool,
}
```

### 4.4 ResolvedFontFace

Lightweight wrapper returned by FontResolver:

```rust
pub struct ResolvedFontFace {
    pub font_id: u64,
    pub family_name: String,
    pub postscript_name: String,
    pub weight: FontWeight,
    pub italic: bool,
    pub coverage_map: CoverageMap,
}
```

**Note**: ResolvedFontFace does not carry platform handles. Use `FontManager::get_face(font_id)` to access the full LoadedFontFace.

### 4.5 LoadedFontFace (from SPEC-051)

```rust
pub struct LoadedFontFace {
    pub font_id: u64,
    pub data: Arc<Vec<u8>>,
    pub family_name: String,
    pub postscript_name: String,
    pub weight: FontWeight,
    pub italic: bool,
    pub units_per_em: u16,
    pub shaper: Arc<TextShaper>,
    pub platform_handle: PlatformFontHandle,
    pub coverage_map: CoverageMap,
}
```

### 4.6 CoverageMap

```rust
pub struct CoverageMap {
    supported: HashSet<char>,
    populated: bool,
}

impl CoverageMap {
    pub fn covers(&self, ch: char) -> bool;
    pub fn coverage_count(&self) -> usize;
    pub fn from_font_data(data: &[u8]) -> Self;
}
```

Built at font load time by sampling key Unicode ranges via `ttf_parser::Face::glyph_index()`.

---

## 5. Resolution Pipeline

Given a FontStack, weight, and italic:

### Stage 1: Primary Matching

For each family in stack order:

```rust
FontManager::find(family_name, weight, italic)
```

If found → return ResolvedFontFace.

### Stage 2: Generic Fallback

Try generic families in order:

```
SansSerif → SystemUI → Serif → Monospace
```

For each, search FontManager for faces matching the category via name heuristics.

### Stage 3: Last-Resort Fallback

Find any face in FontManager with:
* Best style match score (weight + italic proximity)
* Highest coverage count

### Stage 4: Missing-Glyph Fallback (During Shaping)

When shaping detects `glyph_id == 0`:

```rust
FontResolver::find_fallback_for_char(ch, weight, italic)
```

Searches all registered faces for one whose `coverage_map.covers(ch)` returns true. Run is split and re-shaped with fallback font.

---

## 6. Fallback Rules

### 6.1 Fallback Chain Order

1. Primary family (from stack)
2. Remaining families in stack
3. Generic families (SansSerif → SystemUI → Serif → Monospace)
4. Best-coverage face

### 6.2 Coverage Map Implementation

Coverage is built from font data by sampling Unicode ranges:

* Basic Latin (0x0020-0x007F)
* Latin Extended A/B
* Greek, Cyrillic
* Hebrew (0x0590-0x05FF)
* Arabic (0x0600-0x06FF, 0x08A0-0x08FF)
* Devanagari, Thai
* CJK (sample first 256 ideographs)
* Hiragana, Katakana
* Hangul (sample first 256)
* Emoji ranges

Uses `ttf_parser::Face::glyph_index()` for each sampled codepoint.

### 6.3 Generic Family Matching

FontResolver matches fonts to generic categories via name heuristics:

| Category | Name patterns |
|----------|--------------|
| SansSerif | "sans", "arial", "helvetica", "inter", "roboto", "noto sans" |
| Serif | "serif" (not "sans"), "times", "georgia", "palatino" |
| Monospace | "mono", "courier", "consolas", "menlo", "code" |
| SystemUI | "system", "segoe", "sf pro", "cantarell" |

### 6.4 Style Match Scoring

```rust
fn style_match_score(face, weight, italic) -> i32 {
    let mut score = 0;
    if face.italic == italic { score += 1000; }
    let weight_diff = abs(face.weight - weight);
    score += 900 - weight_diff;
    score += face.coverage_map.coverage_count() / 100;
    score
}
```

---

## 7. Integration with ParagraphLayoutEngine

ParagraphLayoutEngine provides two layout paths:

### Single-Font Path

```rust
ParagraphLayoutEngine::layout(
    paragraph: &Paragraph,
    shaper: &TextShaper,
    params: &LayoutParams,
) -> Vec<Line>
```

Uses provided shaper directly. No font resolution.

### Multi-Font Path (with Fallback)

```rust
ParagraphLayoutEngine::layout_with_fonts(
    paragraph: &Paragraph,
    font_manager: &FontManager,
    params: &LayoutParams,
) -> Vec<Line>
```

For each span:
1. Resolve font via `FontResolver::resolve()`
2. Get `LoadedFontFace` from FontManager
3. Shape via `loaded_face.shaper.shape_auto()`
4. Detect missing glyphs (glyph_id == 0)
5. Split run and re-shape with fallback font via `find_fallback_for_char()`

### Fallback Character Selection

When selecting a fallback font, `create_fallback_run()` finds the first non-whitespace, non-control character as the representative codepoint. This avoids incorrect fallback selection based on spaces or punctuation.

---

## 8. Integration with NativeFontBackend

Backends receive LoadedFontFace and must:

* Use `platform_handle` for rasterization
* Never attempt OS font lookup
* Never use DirectWrite/CoreText fallback lists

### Platform Handle Status

| Platform | Status |
|----------|--------|
| Windows (DirectWrite) | Fully implemented via dwrote |
| macOS (CoreText) | Returns `PlatformFontHandle::None` |
| Linux (FreeType) | Returns `PlatformFontHandle::None` |

---

## 9. TextEngine Integration

### FontManagerTextEngine

Primary TextEngine for UI thread:

* Uses embedded Inter font
* Computes font_id via same algorithm as FontManager
* Implements `TextEngine::shaper() -> Option<&TextShaper>`
* Does NOT hold FontManager reference (lightweight, Send-safe)

### BackendTextEngine

Wraps NativeFontBackend:

* Implements TextEngine trait
* Uses TextShaper for shaping
* Backend only used for rasterization

---

## 10. Deterministic Font ID

```rust
fn compute_font_id(
    family_name: &str,
    postscript_name: &str,
    weight: FontWeight,
    italic: bool,
    data: &[u8],
) -> u64 {
    let data_hash = SHA256(data);
    let mut hasher = DefaultHasher::new();
    family_name.hash(&mut hasher);
    postscript_name.hash(&mut hasher);
    weight.to_value().hash(&mut hasher);
    italic.hash(&mut hasher);
    data_hash.hash(&mut hasher);
    hasher.finish()
}
```

Cross-platform stable. Never reused.

---

## 11. WASM Considerations

No OS fonts available. FontManager loads fonts only from:

* Embedded assets
* Runtime injection

Fallback chain and resolution work identically to native.

---

## 12. Implementation Location

| Component | Location |
|-----------|----------|
| FontManager | `src/platform/font_manager.rs` |
| FontResolver | `src/platform/font_resolver.rs` |
| FontFamily, FontStack, FontDescriptor | `src/platform/font_manager.rs` |
| CoverageMap | `src/platform/font_manager.rs` |
| LoadedFontFace | `src/platform/font_manager.rs` |
| ResolvedFontFace | `src/platform/font_manager.rs` |
| PlatformFontHandle | `src/platform/font_manager.rs` |
| FontManagerTextEngine | `src/platform/font_manager_text_engine.rs` |
| BackendTextEngine | `src/platform/backend_text_engine.rs` |
| TextShaper | `src/core/text_shaper.rs` |
| ParagraphLayoutEngine | `src/core/paragraph.rs` |

---

## 13. Testing

* Deterministic font_id across calls
* Fallback for missing glyphs (glyph_id == 0)
* Generic family resolution via name heuristics
* Style match scoring (weight proximity)
* Coverage map correctness
* FontStack iteration order
* Font ID consistency between FontManager and FontManagerTextEngine

---

## 14. Known Limitations

1. **CoreText/FreeType platform handles**: Not yet implemented. Return `PlatformFontHandle::None`.

2. **Glyphon still present**: `text_engine_glyphon.rs` exists for potential WASM fallback but is not the primary path.

3. **No script-specific fallback**: Coverage-based only. No special handling for CJK → CJK fonts or Arabic → Naskh-style.

4. **Generic family matching is heuristic**: Based on font name substring matching, not font metadata.

## 15. Registered coverage repair for ordinary TextBlock - 2026-11-13

**Status: SELECTED / BINDING implementation authority.** JC ordered correction of
the ordinary application text-presentation failure and an early required-text
safeguard. S4 completed that safeguard and qualified a current missing-glyph
witness using the admitted .AppleSystemUIFont backing. This section selects the
remaining root correction, not another diagnostic pass. HAL002/002A remain
supreme.

### 15.1 Facts and exact scope

Current TextBlock uses the single-shaper layout path and reshapes an entire
painted line with the default face, so existing registered-font fallback is
not an effective end-to-end path for this consumer. Current CoverageMap samples
only U+4E00..U+4EFF for CJK; find_fallback_for_char treats absence from that
sample as lack of support. That cannot answer coverage for U+770B even when a
registered face genuinely contains it. A native source-backed witness already
proves the default face lacks the scalar; no atlas expansion or token-limit
change addresses either cause.

Selected owners: FontManager/CoverageMap and its immutable FontFaceCatalog;
FontResolver; ParagraphLayoutEngine; the original TextEngine adapters; and
TextBlock layout, paint and read-only hit/selection projection. Reuse these
owners. No new font service, OS fallback, alternate text renderer, Markdown
viewer, backend change, public SceneGraph/FlatPrimitive glyph storage or mutable
FontManager reference in a widget. Other widgets retain existing policy.
Existing single-shaper public entrypoints remain available.

### 15.2 Exact coverage, deterministic registered resolution

This subsection supersedes the sampled-coverage statements in sections 4.6/6.2
for eligibility decisions. Build exact supported Unicode scalar coverage from
the admitted font's Unicode cmap through the existing ttf-parser at admission.
A scalar is supported only when the actual lookup resolves a nonzero glyph.
Enumerate actual Unicode cmap records, deduplicate scalars, and validate against
the same Face::glyph_index semantics; do not brute-force arbitrary added ranges,
hard-code the observed scalar, consult OS fallback, or mistake untested values
for unsupported values. No renderer atlas is prefilled.

Keep FontResolver's existing stack/style/registered-coverage owner. Candidate
ties must be deterministic using stable font_id after existing ranking, not
HashMap iteration order. Primary-supported source clusters keep the selected
primary face; supplemental faces are considered only for genuinely missing
clusters. Coverage is an eligibility filter, not proof of successful shaping.

Shape each complete styled span through its primary admitted face to obtain
actual source-cluster boundaries. Missing glyphs require resolution of the
WHOLE affected source cluster, not one representative character for an arbitrary
missing segment. Candidates must shape that entire cluster without glyph zero.
Keep logical byte ranges separate from visual glyph order; never assume cluster
offsets increase in visual order or split a scalar/combining cluster/ligature.
Coalesce adjacent source clusters with the same chosen face and shape each
resulting complete face run with original style, direction and cluster gap.
Verify the final coalesced shaping, including its source coverage. Try candidates
only within the finite admitted catalog. No unbounded recursive fallback.

If no admitted face can shape a cluster, retain that exact original source range
and unresolved primary run for the existing HAL360 refusal. Never drop the
segment (the old Option/continue pattern is not lawful here), substitute text,
report a partially missing span as Complete, or silently draw .notdef. An absent
required registry face or mismatched backing is a typed text-input failure, not
permission to invent another face. No new per-codepoint policy is adopted for
Label/Button/TextField by this TextBlock-scoped addition.

### 15.3 Original layout-to-paint binding

Extend the existing immutable-catalog paragraph path, not a parallel layout
engine. Add a fallible catalog-based ParagraphLayoutEngine entry that receives
Paragraph, FontFaceCatalog, explicit default font_id and LayoutParams, and
returns the existing Vec<Line> with each Run.font_id identifying its actual
admitted face. Existing layout_with_fonts must share the corrected resolver/
fallback implementation, rather than retain a second inconsistent algorithm.
Preserve source bytes, hard breaks, cluster boundaries, direction, line-break
rules, logical units, existing canonical line-box policy and style gaps.
Malformed input must not be silently omitted.

The selected additive TextEngine capabilities are:
- font_face_catalog(&self) -> Option<FontFaceCatalog>;
- shape_bound_text(&mut self, text: &str, font_size: f32,
  binding: &TextBinding) -> Result<GlyphRun, TextInputError>;
- metrics_bound_text(&mut self, font_size: f32,
  binding: &TextBinding) -> Result<TextMetrics, TextInputError>.

Default implementations expose no catalog and return UnsupportedTypography for
explicit bound operations. Both original native adapters implement these by
their already-existing admitted-catalog/bound-shaping/bound-metrics owners.
They must not re-resolve the binding by family name or use the default shaper
for a different face. These operations add no font I/O and no registration.
Existing unbound/simple-control behavior is unchanged.

TextBlock retains only the same immutable catalog view needed by its laid-out
runs, never native handles or a mutable manager. With an admitted catalog it
uses the catalog paragraph entry; an old TextEngine that exposes no catalog
retains its existing single-shaper capability rather than claiming fallback.
Cache dependencies include the actual immutable catalog generation, source,
width, style/size and scale inputs. Catalog changes require legal layout
invalidation, not global cache clearing.

Paint each laid-out face run with its EXACT original source substring, binding,
size/gap, visual position and canonical-line baseline. Do not concatenate a
mixed-font line and shape it with the primary face again. Existing GlyphRun
binding -> TextNode binding -> FlatPrimitive binding -> Compile/native backing
is the mandatory transport. No public SceneGraph or FlatPrimitive schema change
is needed. Glyph runs/maps remain ephemeral layout or paint data, not retained
renderer authority. Read-only hit-testing, highlights and copy use those same
per-run geometries and original byte ranges; do not fix paint while leaving
selection on default-font whole-line measurements. Alignment, wrapping, clipping,
DPR and optional selection must remain supported.

`ParagraphLayoutEngine::shape_laid_out_run` is the public reconstruction
entry point for a final catalog-laid-out run. It uses the original paragraph's
merged span direction and the run's admitted face, reproduces the recorded
glyphs, advances and design metrics at the current engine scale, and obtains
the same bound paint metrics used for baseline alignment. A missing face or a
reproduction mismatch returns a typed error. Consumers retain the original
line and visual run geometry. This operation adds no face resolution policy.

### 15.4 Admission and regression boundary

Application assembly selects supplemental assets before the first screen
measurement through original AppConfig::with_font_assets. SystemUI remains the
default. Framework resolution consults only admitted bytes; no runtime font
discovery, download, CoreText fallback list or hidden font substitution is
authorized. The application owns the choice and availability of those assets.

Required regression evidence: real default-font missing-glyph RED; actual
registered covering-face GREEN through original TextBlock -> retained scene ->
DrawPlan/Compile -> native glyph rasterization/output, at 1x and 2x. Include the
publicly identified U+770B, additional supported characters outside the old
sampled ranges, mixed Latin/Cyrillic/CJK lines, hard breaks/wrapping, cluster-safe
selection/copy, primary-font preservation and source-byte equality. Use genuine
font bytes without adding proprietary font files to the repository. Tests must
prove that no-covering-face input still produces the original typed refusal
and S4 first-fault signal. Preserve S4's 11/11 and 7/7 existing evidence; rerun
affected owners after changes, not unrelated full suites.

The original consumer acceptance scenario also requires the ordinary application
binary with its explicit supplemental asset and three successive genuine local
requests, with visible terminal text results, at least one actually containing
U+770B, continued native input, no permanently stale pending-result frame and
unchanged resident-worker ownership. Source production completion alone is not
pixel acceptance. The retained evidence remains private and separate from build
artifacts.

This correction may add only the named defaulted TextEngine capabilities and
the catalog paragraph entry. All existing APIs remain. It selects no dependency
upgrade, font redistribution, new thread/timer, forced redraw, changed consumer
backend budget or other product redesign. The original additive framework
delivery was limited to 0.233.0-beta and its corresponding release note; it did
not change the consumer's version. Concurrent unrelated owner work is preserved.

## 16. Systemic framework responsibility - 2026-11-13

**Status: SELECTED / BINDING under JC's explicit systemic-Halcyon directive.**
Section15's exact registered coverage and bound-run repair continues unchanged
for the admitted TextBlock path. It is a framework capability, not an
application-specific exception. Consumers may supply font assets through existing
AppConfig; they must not inspect each generated answer, delete Unicode, choose
fonts per failure or install custom error panels to keep a window responsive.

Where valid ordinary shaped text still contains an uncovered cluster, HAL360
section7 and HAL361 section7 provide the common renderer-owned visible marker,
typed Degraded result and continuing same-surface presentation. This explicitly
supersedes section15's requirement that every unresolved MissingGlyph alone
must refuse the whole ordinary surface. It does not authorize silent .notdef,
Unicode replacement, an OS-selected font, or claiming full text coverage.

No special treatment of U+770B, generated text or one application is permitted.
Explicit supplemental font admission remains coverage input, not the
framework's liveness mechanism. Other widgets retain their original input,
editing, layout and font-selection policies; renderer containment applies
without those consumers adopting a new widget API.

## 17. Registered typography for native MarkdownView - 2026-11-13

**Status: ADOPTED / ACTIVE, scoped to HAL390 section14.** The new framework
MarkdownView joins TextBlock as a consumer of the exact admitted catalog,
deterministic whole-cluster resolver and immutable final face/direction
transport. This extends section15's caller scope only; it adds no OS fallback,
font service, per-scalar shaper, alternative atlas or other-widget policy.

The viewer requests explicit prose/code family names and real weight/italic
roles. Qualify primary role availability against actual admitted metadata;
a regular closest match is not evidence of bold/italic. Qualify the code
primary's actual fixed-pitch metadata rather than the generic-name heuristic.
Once that primary is qualified, unsupported clusters use existing registered
coverage fallback and actual metrics. Never manufacture a monospace width for
a fallback face. HAL360/361 Degraded containment remains the shared no-cover
behavior; structural/raster/resource failures are not hidden.

The original macOS consumer configuration admitted already-installed Arial and
Courier New regular/bold/italic/bold-italic assets, in addition to its existing
Arial Unicode asset, through the original startup FontManager and AppConfig
admission. SystemUI remained the default for other controls. This configuration
is historical consumer evidence, not a required font bundle for every
application. No font download, redistribution or discovery from the widget is
authorized.

## 18. Registered editable-text coverage and bound-run coherence

**Status: ADOPTED / ACTIVE scoped correction under JC's systemic-Halcyon
mandate.** This section owns the registered editable-text bound-run repair.
Framework implementation and consumer acceptance remain separately qualified.

### 18.1 Observed blocker and exact caller extension

The retained native consumer run established ordinary editor ingress and two
correlated text results using one resident worker. The third fixed request
reached the real TextField byte-for-byte, but the staged-window observation
reported a missing-glyph box at U+770B. No third submission occurred. This is not
evidence of another backend stall or recurrence of the former whole-surface
refusal.

Independent source inspection establishes the relevant missing connection:
TextLayoutCache::rebuild uses the single-shaper paragraph entry; ordinary
TextField paint shapes the full document with the default face. TextArea uses
the same cache and also reshapes each painted run through the default face.
Neither consumes the registered covering-face path already used by TextBlock
and MarkdownView. The admitted supplemental asset already supplies coverage;
the correction is not another asset download or a special case for one scalar.

This section expressly extends sections15-17 and HAL048's registered bound-run
caller scope to TextLayoutCache and its existing TextField/TextArea consumers,
including ordinary tree-owned TextField instances used by existing composite
editors. It supersedes only their former single-face coverage restriction.
HAL052/107/108/109 editing, input, clipping, scroll, history and policy ownership
remain unchanged. Other widgets acquire no new interaction or font policy.

### 18.2 One shared layout and shaping result

Use the existing immutable FontFaceCatalog and
ParagraphLayoutEngine::try_layout_with_catalog, with the actual current primary
binding resolved by TextEngine for the existing input typography. Retain
primary-supported clusters and the existing deterministic whole-cluster
covering-face resolution. Do not replace the default SystemUI family globally.

TextLayoutCache owns the resulting existing Lines and ClusterBoundaries plus
only the immutable catalog/binding and input identity required to reproduce
them. Build boundaries from those SAME final laid-out runs through the original
ClusterBoundary::build_from_lines. No separate per-character widths, scalar-only
cursor validation, fake glyph IDs, prefix measurement or alternate bidi engine.

Cache eligibility includes source/document version, layout width, actual input
font size, current admitted catalog generation, actual primary binding and
canonical metrics. Preserve logical paragraph scale1 and the existing native
scale/metric qualification; a scale/font/catalog change cannot reuse an
incompatible cache. Theme colors remain live paint values, not permission to
change source or rebuild unrelated widgets. Font size is synchronized before
rebuild under PM013. TextField retains its100000 logical single-line shaping
width; TextArea retains its existing wrap width, canonical line boxes and extent
rules. No font IO, discovery or admission from layout, event or paint.

Preserve public APIs and their supported callers. A crate-private fallible
cache preparation entry returning the original TextInputError is admitted for
the shared implementation and HAL383 validation. Publish new derived cache
values only after coherent preparation; no half-updated lines/boundaries.
Existing public rebuild delegates to that same preparation. An advertised
catalog with contradictory/missing required binding or invalid geometry cannot
silently fall back to the default shaper: preserve the typed cause and the
existing named UI-TYPOGRAPHY-INPUT invariant at a caller that cannot return it.
Ordinary valid Unicode without a covering face is NOT such a structural error:
retain its exact unresolved source run for normal HAL360 Degraded containment.
A TextEngine genuinely exposing no registered catalog retains its existing
single-shaper capability and cannot claim registered fallback. This capability
boundary is not an opt-in production feature or second font registry.

### 18.3 Paint, geometry and editor lifetime agree

TextField and TextArea paint each actual final face run using its exact source
range, admitted face/backing, source direction, font size and gap through
TextEngine::shape_bound_text and existing draw operations. Factor only a
crate-private bound-run operation where it avoids duplicating the existing
TextBlock mechanism. Do not concatenate mixed-face runs and reshape with the
default face. Logical scene/Compile schemas and renderer ownership are unchanged.

Keep the existing canonical control/line baseline, aligning actual face ascents
to it. Hit testing, horizontal/vertical navigation, selection highlights, caret
placement, scroll-to-caret and natural extent consume the same cache geometry.
Separate logical source order from visual run order. Preserve original source
bytes, ordered selection, cursor affinity/preferred_x, undo/redo and clipboard
values. No source normalization or selection repair in Layout/Paint.

Selection backgrounds and caret must be emitted exactly once at their existing
relative layers. Translate existing cached geometry into each run's coordinate
space without duplicating scroll, scaling or alignment. Password fields keep
the existing mask-only rendering, mask geometry and copy restrictions; never
emit or diagnose password document text as visible source. Empty text,
placeholder and the existing defensive stale-cache path must not reintroduce
default-only uncovered text: reuse the same resolved-run calculation for
ephemeral draw preparation where the prior contract already allows it, without
publishing cache, editing state or new layout authority during Paint.

Existing motion qualification remains authoritative. Preserve all previously
qualified primary-supported ASCII reveal/conceal/range transitions and Unicode
navigation. The single-bound-string transition painter may execute only when
its source/binding/metrics genuinely match the current resolved layout. A
mixed-face or otherwise unqualified content transition uses the already-adopted
canonical-text fallback at first consumption and existing lawful retirement.
Do not disable motion globally, admit new transition classes, retain shaped
history, or build mixed-font animation machinery for this correction.

IME preedit/commit and embedded/composite entry continue through their existing
owners, with the same current cache/paint binding. Availability, Capture,
keyboard routing, filters, callbacks and host operations are unchanged.

### 18.4 HAL383 qualification uses the same editor path

For this correction only, HAL383 section9.4's temporary validation cache uses
the shared registered preparation above when the actual engine exposes its
catalog, and its original no-catalog shaper otherwise. This expressly qualifies
the old S1D wording that referred only to shaper() and its old task-local
TextLayoutCache exclusion. No engine cut or publication phase changes.

The real settled theme, same100000 width, current catalog and exact final
cluster map must qualify every original cursor/anchor/head offset BEFORE
atomic Phase2 publication. A preparation error maps to existing
TextFieldHandoffError::ShapingUnavailable; an absent target boundary remains
InvalidBoundary. No clamp, snapping, synthetic sentinel proof, delayed success,
new status/API or editor-state mutation from Layout is admitted.

### 18.5 Scope, proof and return to the original application

Production scope: src/widgets/text_layout_cache.rs, text_field.rs and
text_area.rs; only exact crate-private reuse factoring in the current text
owner if required, without changing its public surface or accepted behavior.
No App/WidgetTree/router/renderer/backend/font-selection algorithm changes.
No consumer or model-runtime source edits, new fonts, dependencies, flags,
timers, workers, queues or font redistribution. The original correction retained
0.234.0-beta and added one scoped Unreleased correction note; independent owner
release work was not overwritten.

Required proof uses the existing owner tests and real registered font bytes:
1. Reproduce the original default-face failure for the exact native request in
   a real routed TextField. Keep an independent missing-primary versus covering
   supplemental face witness; do not encode only a screenshot assumption.
2. GREEN through real cache/retained scene/Compile/Metal readback at1x and2x:
   supported mixed Latin/Cyrillic/CJK/combining text has actual covering runs and
   Complete realization. Assert genuine nonzero glyphs and correct bindings,
   not merely visible marker pixels. Include RTL run-direction preservation.
3. Routed editing, reverse selection, cluster-safe movement/deletion/undo,
   caret hit geometry, horizontal scroll and HAL383 fresh-mount handoff preserve
   exact text and legal offsets. Prove TextArea's shared-cache wrapping/scroll
   and per-run paint coherence, and preserve existing password/motion paths.
4. Truly uncovered text remains source-faithful and renderable Degraded with
   healthy same-surface input. Structural/raster/resource failures are not
   relabeled coverage success. No atlas/pixel quota substitutes for actual text.
5. Obtain an owned ordinary framework-window observation of the formerly boxed
   supported input and continued routed input, with proof boundaries explicit.
   No model run is needed for the framework correction.

Use the nearest existing inline owners and one registered internal renderer
test module; extend the existing public handoff test only where necessary.
Run the affected named suites and normal package build/check sequentially.
Previously recorded unrelated failures remain distinct; do not weaken their
assertions or silently label them new regressions. Preserve foreign optical
source and authority.

After independent framework review, the original native three-request consumer
scenario remains the dependent acceptance boundary, using the measured editor
ingress route and unchanged source/backend limits. This framework delivery does
not certify those requests, physical clipboard, human gestures, exact scanout or
shutdown acknowledgement.

### 18.6 Independent source/Metal acceptance and consumer-native continuation - 2026-11-14

**Status: source and selected automated/Metal boundary ACCEPTED; ordinary-window
editor observation and the original three-request consumer result remain OPEN.**
Independent review covered the shared cache, TextField/TextArea changes, HAL383
validation, regression sources and retained original execution records. The six
owned files matched the recorded tested source identities. The cache prepares
registered Lines and original ClusterBoundaries atomically; both painters use
the final source/face/direction bindings and canonical baselines. Password
presentation remains mask-only; qualified motion and existing input owners
remain separate. No consumer backend execution was performed during this review.

The original 44-byte routed-request RED reports Degraded/MissingGlyph and the
covering-face assertion failure. Final registered_editable_ reports 7/7;
handoff 9/9, public handoff 1/1, complete-text 8/8, changed check and ordinary
framework build passed. The actual test sources use the existing retained
scene, Compile, Metal submission/readback and matching admitted fonts at 1x/2x,
not a mock font or marker treated as coverage. Four caret-suite failures retain
the exact original-source assertion values in original-failure-baseline.txt;
the TextField/TextArea suites remain failed, not relabeled green.

The original red regression, final source and handoff checks, public handoff
test, complete-text checks, both editor suites, original-failure baseline,
package check/build and native allocation records were retained privately.
Independent review inspected those original records without repeating equivalent
test runs. They are historical verification evidence, not public reproduction
artifacts linked by this document.

The prescribed asset/preset-only example did not supply a usable visual fixture:
its original composition allocated the row64x28 and field0x28. Its native
capture cannot prove the corrected field visible. This failure remains recorded;
it is not evidence that the repaired font path failed and is not a native PASS.

**Selected acceptance-route clarification:** the still-required ordinary
App::run editor/input observation in section18.5 item5 is obtained in the
original consumer application together with its already-required three-request
run. This exercises the same ordinary TextField, registered fonts and native
renderer. It replaces only the separate example prerequisite, not the
input/glyph/pixel requirement. Native success may not be inferred from the
accepted Metal test.

No further demo-layout edits, new demo, framework implementation, backend
fixture replay or font acquisition are selected. The failed example remains
unchanged and recorded. The dependent application build and native run belong
to the consumer acceptance boundary; this source/Metal review requires no
additional framework writer or parallel verification. Any new framework defect
requires its own exact observed evidence before source repair.
