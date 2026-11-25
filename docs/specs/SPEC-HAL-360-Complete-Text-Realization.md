# SPEC-HAL-360 — Complete Text Realization

**Status:** Adopted / Binding
**Author:** JC

Scoped addition (delegated architectural acceptance, 2026-10-10): HAL-048's dated
UI typography contract carries exact immutable admitted face backing and logical
cluster spacing into Compile. Compile reshapes with that face and rasterizes its
matching native backing; unchecked IDs/names or a changed shaper alone are not
admission. Invalid/nonfinite/overflowing typography input is typed unavailable,
never silent clamping or partial-success text. Existing complete-surface failure,
atlas residency and Execute rules remain. Implementation pending source proof
and independent review, with no document-wide status change.
**Owner:** Halcyon renderer
**Observation extension:** SPEC-HAL-361 (Adopted / Binding; canonical App attempt/surface receipts without changing renderer Compile identity or text realization authority)
**Authority:** Delegated architectural adoption on 2026-09-15 following the
owner/source audit. The bounded ListView compatibility correction below was
also adopted following review of its exact proposal and authority audit.

## 1. Scope and authority reconciliation

Normal Halcyon surface rendering requires complete text realization. There is
no best-effort policy in this contract. A future degraded-text policy requires
separate adoption and a concrete caller requirement.

This contract extends SPEC-HAL-029 and SPEC-HAL-066 atlas/text preparation,
SPEC-HAL-084 Compile admission, and SPEC-HAL-359 executable text ordering.
SPEC-HAL-001 remains foundational; SPEC-HAL-163 surface isolation and the
Constitution remain unchanged. SPEC-HAL-006 and SPEC-HAL-065 retain Draft
status; this contract does not activate their font-fallback proposals.

For this text contract, rasterization, atlas mutation, final UV lowering and
GPU uploads belong exclusively to Compile under binding SPEC-HAL-084 GPU
Write Isolation and SPEC-HAL-359. This supersedes SPEC-HAL-066 section 5.10's
between-frames wording and clarifies SPEC-HAL-084 D2's acquired-but-not-written
wording only for these Compile resource writes. Build remains CPU-only;
Execute consumes immutable prepared data and performs no resource writes.

SPEC-HAL-295A custom-render preparation reports admitted offscreen image
publication, not subsequent ordinary text preparation. Its API and Viewport3D
semantics are unchanged.

**Proposed relationship to SPEC-HAL-363/364 (2026-10-05):** The paired
documents remain Proposed pending joint adoption. This annotation changes no
declared status, implemented scope or substantive law in this document.

SPEC-HAL-364 historical text sources must re-realize through this complete-text
owner. They gain no persistent shaping/atlas/compiled-plan validity or late
fallback. Composition/resource unavailability remains distinct from
SurfaceTextRealization; Complete text alone does not prove output.

## 2. Surface preparation result

INV-TEXT-COMPLETE: A surface is complete only when every requested text primitive
has been shaped and every resulting glyph is either explicitly classified as
valid no-ink or realized in valid atlas storage. Empty text and surfaces without
text are complete. Shaping and measurement alone do not establish realization.

Missing glyph/notdef, unavailable shaping, raster failure, malformed raster
storage, atlas capacity refusal and lost residency are unavailable outcomes.
The result identifies the first detected failure in the fixed gather, raster
classification, atlas admission and final lowering sequence. Each stage visits
surfaces/primitives/glyphs in deterministic order and records the primitive and,
where applicable, glyph index. Failure evidence is bounded; it does not retain
source text, glyph runs, layout references or arbitrary logs.

NativeFontBackend retains its Option-returning public method. An internal
adapter classifies its result: None is unavailable, glyph zero is missing,
and a nonzero glyph with zero bitmap dimensions and empty pixels is explicit
no-ink. Native implementations produce that no-ink representation from their
glyph metrics. A positive-sized bitmap must have the exact supported storage
length and matching key/format. An all-zero positive-sized bitmap is not used
as evidence of no-ink. This does not add shaping, fallback or font selection
to raster backends.

An immutable outcome is accessible through SurfaceCompiledData and
MultiTargetDrawPlan for the exact SurfaceId and preparation frame. Complete
records requested primitive, glyph and no-ink counts. Unavailable records the
typed failure. Neither result is GPU completion, submission or presentation
evidence. Retaining a copied result does not extend the compiled plan's lawful
resource lifetime.

## 3. Atlas working set

INV-TEXT-RESIDENCY: Gather per-surface text demand before final UV lowering.
Classify each surface's glyph demand before attempting its atlas admission.
Admission follows compositor surface order and first glyph occurrence order.
Already admitted surfaces' glyphs and the current candidate surface's existing
glyphs are protected from LRU eviction. A failed candidate releases its own
protection; it does not invalidate previously admitted surfaces.

Atlas growth/repacking may occur during working-set admission. Only after all
surface admission attempts finish may final quads copy UVs from the final atlas.
No further insertion, growth or eviction occurs for that compiled plan. LRU
retains its deterministic ordering among unprotected entries. Oversized glyphs
and a working set that cannot fit the atlas/device limit terminate with typed
capacity refusal; there is no retry loop without progress or silent glyph loss.

Demand, glyph runs and classification bookkeeping are Compile-local. Compiled
quads retain only their existing numeric rendering data. A subsequent Compile
or frame advance invalidates prior compiled resource use. Execute rejects a
plan from another renderer or a superseded preparation before target acquisition
or encoding. This guard does not introduce persistent glyph/layout authority.

## 4. Render admission and compatibility

INV-TEXT-ADMISSION: An unavailable surface produces no successful rendering
output: do not encode or acquire/present its remaining text, backgrounds or
geometry. Preserve its immutable failure result for composition callers. Other
complete surfaces remain independently eligible; no cross-surface atomic
presentation is introduced. Existing acquisition failures remain separate.

Complete surfaces retain SPEC-HAL-359 ordering, contiguous text/background runs,
clipping and scale behavior. Non-text and empty surfaces retain normal admission.

**HAL375 scoped adoption, 2026-11-07.** The [first passive PBR HUD](SPEC-HAL-375-3D-Anchored-Spatial-HUD-and-Information-Connectors.md#9-adopted-first-passive-pbr-hud-profile--2026-11-07-s1) is optional until complete group admission. Finish ordinary required working-set admission under §§2–3 first and protect those glyphs. Then attempt the bounded HUD group through those same realization and atlas owners, before final UV lowering. If the group cannot be completely realized and lowered, omit that whole candidate from the schedule with a typed HUD refusal. The ordinary surface retains its actual §4 result. Once admitted the group's complete text, backgrounds and leader commands obey HAL359 and the matching HAL375 GPU gate. This exception creates no general optional text primitive and does not relax any ordinary surface failure.

The new result API is additive; refusing previously silently degraded text is
the intentional normal-behavior correction. Release the capability in the next
Halcyon minor version with a changelog entry.

### 4.1 Default ListView settings affordance

To keep the existing default ListView header admissible under complete-required
text realization, its settings affordance MUST use one built-in
`IconId::SETTINGS`, defined once through the existing `IconBuilder` and immutable
built-in registry, then emitted through `PaintSink::push_icon` into the existing
Path pipeline. Preserve the 24x24 button rectangle, centering,
`HEADER_TEXT_LAYER`, `typography.primary` color, clipping, hit geometry and
interaction. Use a centered 16 logical-pixel icon in the existing button.

This clause authorizes only that producer correction and existing vector-symbol
mechanism. It does not adopt Proposed SPEC-HAL-264/265/265A, introduce font
fallback, or alter other icon/widget behavior. Active SPEC-HAL-266 section 3.1
continues to govern `SceneBuilder::push_icon` position snapping. Binding
SPEC-HAL-187 and SPEC-HAL-192 retain the ListView ownership and closed-composite
boundaries; Deprecated SPEC-HAL-118/118A/118B are not restored by this correction.

## 5. Required proof

Use the existing inline unit, internal renderer and public integration test
surfaces, never examples. Prove no-ink versus missing/raster failure, bounded
capacity refusal, surface identity, and non-text admission. Exercise the actual
Compile/encode GPU path with readback: atlas growth after early text, pressure
from a later surface, and eviction must preserve admitted text pixels. Include
an over-capacity candidate and a valid independent surface. Preserve the existing
z/background regression controls. Verify that superseded compiled plans cannot
encode against newer resource contents. Preparation counts alone are insufficient
proof of pixel stability.
The default ListView header must emit the settings affordance through registered
vector paths without requesting U+2699 text. Prove its retained geometry, layer,
color and visible GPU pixels while retaining the Retina ListView/ComboBox popup
border regression control.

## 6. Implementation verification record — 0.198.0-beta

Recorded on 2026-09-15 against base commit
`50994e272dbbb92c0970949ddc9df205e93bca05`, with the HAL-360 implementation
applied. Grip verification exercised the macOS/CoreText backend. The updated
DirectWrite and FreeType implementations were not executed on their platforms.

The following groups all passed in the full library run; groups overlap:

| Proof | Passed |
| --- | ---: |
| `complete_text_realization` classification, backend refusal and GPU readback | 6 |
| `renderer_z_order`, including those GPU tests, SETTINGS/Retina and z/background controls | 9 |
| `icon_builtins` and `icon_validate` | 13 |
| `platform::font_manager_text_engine` identity, shaping and native metrics | 12 |
| `renderer_text_metrics` and `renderer_text_index_width` | 2 |

Executed through Grip `x`:

```text
verify package halcyon test renderer timeout 1200 detach-after 2
verify package halcyon check test timeout 1800 detach-after 2
```

The renderer-filter run reported 246 passed and one staging-test failure.
The full check passed with zero errors and 55 warnings; its library tests
reported 2019 passed and nine failed. These aggregate test runs remain red.
The full library failure prevented subsequent integration-test execution in
that invocation. The retained review records the scope disposition.

### 6.1 Unrelated source-controlled verification debt

Git comparison with the base commit confirms that the failing fixtures and
their guard/state owners are unchanged. Clean-HEAD executable verification was
not obtained: the isolated checkout had no approved Grip workspace alias, and
the authorized disposition forbids bypassing the proxy through CLI or direct
Cargo for that purpose. Classification therefore rests on source/diff control:

- Four `core::widget_tree::tests::test_SPEC088_clip_node_*` fixtures set rects
  and compute positions without completing a layout epoch. They fail the
  unchanged `paint_node` epoch guard before widget paint.
- `listview_paints_fixed_header_chrome_above_hydrated_body` and
  `listview_scrollable_cells_remain_inside_content_clip_after_scroll` call
  `paint_tree` without `PaintPhaseGuard`. The unchanged
  `header_visual_snapshot` fails before the changed settings paint branch.
  The fixture's registry construction does execute the added SETTINGS
  definition, but that local geometry registration does not write layout or
  paint-phase authority state and neither failing guard reads it.
- `completed_plot_frame_publishes_reference_and_price_transform_together` and
  `aligned_book_repaints_when_completed_reference_publication_changes` expect
  `needs_animation()` after reference publication. The unchanged
  `AlignedOrderBook` predicate reads only `VisibleDepthScale::settled()`;
  their publication/scale path does not enter the changed font, atlas, compile
  or settings mechanisms.

These eight failures are classified as unrelated pre-existing/latent debt for
this unit with the explicit executable-baseline limitation above. Their guards
and fixtures are preserved.

### 6.2 HAL-040 proof limitation

`repeated_path_compile_with_frame_advance_bounds_in_flight_staging` passed
three consecutive isolated runs but failed in the renderer and full-library
runs. It compares allocation maxima after two unpaced 128-compile headless
bursts with uncontrolled GPU completion schedules. It never acquires a native
surface, submits/presents rendered frames or encounters acquisition
backpressure, so it does not faithfully prove SPEC-HAL-040's implementation
status requirements for production frame completion and acquisition
backpressure (lines 25–26 at the base commit).

The existing headless target is compile-only; it cannot supply that native
presentation lifecycle. Semantic references to native target registration
lead to application/runtime and example entry points, with no existing
deterministic native-surface verification harness to reuse. Creating platform
test infrastructure is outside this unit's authorized scope. The original
staging test, staging production lifecycle and HAL-040 requirements remain
unchanged; no production-envelope stabilization proof is claimed here.

## 7. Framework-owned missing-cluster containment - 2026-11-13

**Status: SELECTED / BINDING.** JC explicitly directed that this failure be
solved systemically in Halcyon, without requiring an application error handler. Under that mandate,
this section adopts the degraded-text policy previously reserved by section1.
It supersedes sections1,2,4 only for valid shaped text containing MissingGlyph;
other unavailable outcomes retain strict admission. HAL002/002A, original
source bytes, font identity, phase isolation and executable-plan custody remain
unchanged. This scoped correction retains the existing safeguard and its separate proof limits.

### 7.1 Responsibility and result

Registered-face resolution and exact layout/paint binding remain HAL049 section15
and HAL048 responsibilities. Resolution must be attempted there before the
normal renderer contains any still-missing cluster. Compile must not choose
another font, discover OS fonts, reflow text, or mutate widgets.

A missing glyph in otherwise valid shaped ordinary text must not suppress the
rest of its surface. The renderer emits an explicit geometric missing-cluster
marker at the affected cluster, renders all remaining valid text/geometry, and
reports a non-Complete result. This applies to ordinary text primitives from
every consumer, including TextBlock and application-generated text, without a
single-codepoint restriction.
No application callback, opt-in flag, per-widget policy switch, replacement
answer panel, new thread, timer, retry loop or special application code is required.

Add TextRealizationOutcome::Degraded with primitive_count, glyph_count,
no_ink_glyph_count, missing_cluster_count and first_failure. Counts retain
their requested-demand meaning; first_failure uses the actual original
TextRealizationFailure, with reason MissingGlyph and same-attempt indices.
Complete retains its original meaning. is_complete() is false for Degraded.
Add is_renderable(): only Complete and successfully prepared Degraded permit
ordinary surface output. Unavailable permits none. Observation preserves the
actual variant and actual output/submit/present flags; Degraded is never renamed
Complete. Optional HAL375 HUD groups keep their existing all-Complete rule.

### 7.2 Exact cluster and immutable source

SurfaceTextDemand::gather retains the valid shaped run and original keys instead
of breaking at its first zero glyph. Use the actual admitted face's shaper and
the HAL048 verified whole-run source-range mapping to identify every source
cluster containing a zero glyph. Replace that WHOLE cluster's visual glyph
occurrences with one marker, including any otherwise nonzero glyph in that
same cluster. Do not split a combining/ligature cluster or confuse visual order
with logical byte order. Keep the original run/source, offsets, selection,
clipboard, caret, advances and line breaks unchanged.

Only Compile-local numeric/source-range evidence is added. No source-bearing
history, cross-frame glyph map, public SceneNode/FlatPrimitive provenance schema
or glyph0 atlas entry is selected. Missing or inconsistent mapping/backing,
nonfinite geometry, invalid shaping or other corruption remains typed
Unavailable, not an invitation to guess a cluster or claim containment.

### 7.3 Lowering through the original text pipeline

Use WgpuRenderer::build_text_quads_from_surface_primitives and the existing
text_background_quad solid-color sentinel to emit marker geometry. No font,
rasterizer, atlas slot, new shader or alternate renderer is needed for a marker.
Use the original text foreground color: an outlined box with a central vertical
and horizontal bar, made from at most six solid quads. Its position follows the
verified cluster x interval and original run baseline/ascent/descent. Normal
width is the absolute cluster advance; minimum visible width/height is two
physical pixels converted using the admitted surface scale. Border thickness is
one physical pixel, bounded by half the marker dimension. A zero-advance cluster
uses the minimum marker overhang at its original origin, without changing
following glyph positions or layout. Preserve original clipping, Z/order,
selection backgrounds, fractional origin and per-surface DPR. Do not print the
source character or codepoint into a second font-dependent label.

Skip only marked-cluster keys during raster/atlas admission and final glyph
lowering. Realize all remaining original keys under existing HAL360 residency.
Backgrounds, markers and healthy glyphs stay in the original contiguous text
command and HAL359 schedule. Execute remains read-only. If another failure
occurs, including atlas/raster/resource failure, the surface is Unavailable;
Degraded is not a blanket success path. Marker bookkeeping and geometry are
bounded by current demand, at most one marker per missing source cluster.

### 7.4 Guard, recovery and proof

INV-TEXT-LOCAL-MISSING: a contained missing cluster cannot prevent a correctly
prepared ordinary surface from progressing. INV-TEXT-FAULT-NOT-COMPLETE:
Degraded output cannot claim Complete text. INV-TEXT-SOURCE-UNCHANGED: markers
never modify application/source/clipboard bytes. An uncontained Unavailable
surface still triggers the existing no-false-output poison pill on attempted
output/submission. Invalid internal claims panic; unsupported content alone does
not. HAL361's bounded fault remains unresolved after Degraded presentation and
clears only on same-surface Complete plus actual submit/present, or retirement.

Extend existing complete_text_realization, renderer_z_order and frame_realization
tests. Prove genuine missing-glyph source yields Degraded and visible marker
pixels at 1x/2x while healthy text/controls on the SAME surface still render;
source/copy/selection and baseline/advance remain unchanged. Include a whole
combining cluster, mixed supported/missing runs, clipping/Z, repeated failures,
actual recovery and independent surfaces. A genuinely invalid backing or atlas
failure must still refuse. Existing no-ink, residency and optional HUD semantics
remain covered. Update tests whose earlier all-surface MissingGlyph expectation
this explicit adoption supersedes, retaining separate strict-failure controls.

Join this framework proof to the registered-covering-face regression and the
ordinary three-turn conversational-application acceptance scenario. A separate
real Halcyon native fixture must prove continuing input and visible frame changes
with deliberately uncovered text on the same surface, without a consumer error
hook. No repeated model job is needed merely to exercise the deterministic
unsupported-cluster control. No font files are redistributed. Classification
remains the already selected additive zero-major API unit and release boundary.
