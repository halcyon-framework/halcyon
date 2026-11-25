# SPEC-HAL-358: Luminous Contour Rendering

## Pre-Flight

- Governing SPEC and read status: SPEC-HAL-002, SPEC-HAL-002A,
  SPEC-HAL-024, SPEC-HAL-083, SPEC-HAL-085A, SPEC-HAL-266, SPEC-R-003, and
  SPEC-R-004 read before writing this authority: YES
- Widget policy: N/A — renderer-owned paint data and scene lowering only
- Interaction added and policy authority: NO
- Applicable FORBID rules: FORBID-358-TEMPORAL-SEMANTICS,
  FORBID-358-INFERRED-TOPOLOGY, FORBID-358-BLOOM,
  FORBID-358-ORDINARY-STROKE-DRIFT, FORBID-358-UNBOUNDED-HALO,
  FORBID-358-FRONT-MARKER, FORBID-358-HIDDEN-PACKING, and
  FORBID-358-SCOPE-SMUGGLING
- Existing framework mechanism inspected: `PaintSink`, immutable `SceneNode`
  paint data, SPEC-R-003 constrained polyline segments, the active
  `ShapePipeline`, renderer-owned target globals/DPR, premultiplied OVER,
  deterministic batch keys, and capacity-reusing GPU shape buffers
- Reinvention risk: extends the existing analytic shape pass; does not add a
  second renderer, path framework, post-process graph, texture effect, timing
  system, or consumer-domain model
- Demo/example compliance: dedicated display-only native example; no Escape
  handling or product interaction

**Author:** JC
**Status:** Adopted / Binding
**Date:** 2026-09-01
**Layer:** 2D paint description / SceneGraph render data / analytic shape pass
**Owner:** Halcyon renderer
**Depends on:** SPEC-HAL-002, SPEC-HAL-002A, SPEC-HAL-024, SPEC-HAL-083,
SPEC-HAL-085A, SPEC-HAL-266, SPEC-R-003, SPEC-R-004
**Extends:** the SPEC-R-003 constrained line/polyline family and the
SPEC-HAL-085A shape-pipeline parameter contract
**Implementation status:** The single-owner zoned-material implementation and
automated conformance proof are present. Final appearance calibration remains
subject to the visual owner-review boundary in §10.

This SPEC authorizes one bounded rendering capability. It does not establish
implemented truth until the source and proof named below exist.

---

## 1. Problem and Authority Decision

Halcyon can render ordinary 2D lines and polylines, but it has no general
render-description for a thin luminous contour. Existing glow mechanisms are
widget- or shape-specific, while SPEC-R-004 explicitly defers arbitrary path
blur/shadow effects. A consumer would otherwise have to approximate a halo by
emitting several independently widened strokes, duplicating geometry and
turning one visual material into ordering-sensitive consumer policy.

The authority decision is:

```text
consumer-owned already-visible contour geometry
    + zero or more consumer-supplied visual front locations
    -> checked Halcyon LuminousContour paint description
    -> immutable LuminousContour SceneNode
    -> existing deterministic shape batch
    -> one analytic ShapePipeline pass
    -> premultiplied OVER target composition
```

Halcyon owns only validation, screen-space appearance, mechanical front-range
lowering, shader packing, and rendering. The consumer owns what the contour
means, which portions exist, where visual fronts are located, and when any of
that data changes.

**INV-358-AUTHORITY:** A `LuminousContour` is presentation data. It contains no
clock, progress, reveal direction, animation state, topology operation, domain
identity, semantic value, callback, or successor-state rule.

---

## 2. Admitted Public Model

### 2.1 Appearance

The paint layer owns one checked value:

```rust
pub struct LuminousContourAppearance {
    core_luminance_lift: f32,
    inner_aura_radius: f32,
    inner_aura_intensity: f32,
    halo_radius: f32,
    halo_intensity: f32,
    halo_falloff: f32,
}
```

`try_new(halo_radius, halo_intensity, halo_falloff)` remains the compatible
single-zone constructor. It sets the three new zoned fields to zero and
preserves the original source-hue core/halo evaluation exactly.

`try_new_zoned(core_luminance_lift, inner_aura_radius,
inner_aura_intensity, halo_radius, halo_intensity, halo_falloff)` admits one
bounded luminous material with three analytic regions:

1. the existing thin core, whose source RGB may be mixed toward linear white;
2. a narrow source-hue inner aura with fixed quadratic falloff; and
3. a wider, weaker source-hue outer halo using the existing configurable
   falloff.

Both constructors admit only:

- finite `core_luminance_lift` in `0.0..=0.75`;
- finite `inner_aura_radius` in `0.0..=4.0` logical pixels;
- finite `inner_aura_intensity` in `0.0..=0.75`;
- finite `halo_radius` in `0.0..=8.0` logical pixels;
- finite `halo_intensity` in `0.0..=0.25`; and
- finite `halo_falloff` in `2.0..=8.0`.

For a zoned appearance, `inner_aura_radius <= halo_radius`; reversed zone order
is rejected. The existing `halo_*` names describe the outer zone so existing
callers and accessors retain their meaning. If all three zoned fields are zero,
evaluation uses the exact compatible single-zone branch.

The contour's existing `Color` supplies the linear source hue and alpha for all
regions. The contour's `core_width` supplies the crisp core width. Core
luminance lift is a bounded linear-RGB interpolation from source RGB toward
white; it neither changes core coverage nor alpha. No independently chosen
second hue, blend mode, texture, or unconstrained material record is admitted.

### 2.2 Front emphasis

One visual front is:

```rust
pub struct LuminousContourFront {
    distance: f32,
    half_extent: f32,
    intensity: f32,
}
```

- `distance` is finite non-negative logical arc length from the first contour
  point along the ordered supplied segments;
- `half_extent` is finite and in `2.0..=32.0` logical pixels; and
- `intensity` is finite and in `0.0..=1.0`.

`LuminousContourFronts` is a value-owned inline set containing zero through
eight fronts in stable caller order. It performs no heap allocation. More than
eight fronts is rejected; it is not silently truncated.

The complete contour validates every front against its geometry:

- `distance <= total_arc_length`;
- `half_extent >= core_width`; and
- the resolved range stays finite.

A front range has target length `2 * half_extent`. Near either contour endpoint
the range shifts inward before clamping so that it retains that length whenever
the complete contour is long enough. This makes an endpoint front a short
contour-aligned energy range rather than a circular endpoint marker.

### 2.3 Contour emission

The public paint boundary is one checked operation:

```rust
fn push_luminous_contour(
    &mut self,
    points: Arc<[Point]>,
    core_width: f32,
    color: Color,
    appearance: LuminousContourAppearance,
    fronts: LuminousContourFronts,
    z_index: ZIndex,
    key: Option<SceneNodeKey>,
) -> Result<(), LuminousContourError>;
```

The operation admits:

- `2..=16_385` ordered finite points;
- `core_width` finite and in `0.25..=4.0` logical pixels;
- finite linear RGBA channels, each in `0.0..=1.0`;
- every adjacent segment with positive finite length; and
- a positive finite total arc length.

The caller closes a contour by repeating its first point at the end. A line is
two points. A square or sampled circle is an ordinary closed polyline. This
SPEC does not add Bezier curves, an arbitrary path framework, or inferred
closure.

The node takes shared immutable point storage. Construction and SceneGraph to
batch lowering clone the `Arc`, not the point array. A consumer that changes
only front locations may reuse the same point allocation.

**INV-358-IDENTITY:** The new render primitive has
`SceneNodeKind::LuminousContour = 21`. Existing discriminants remain unchanged.
Its optional `SceneNodeKey` obeys the unchanged SPEC-HAL-083 identity rules.

---

## 3. Mechanical Coverage and Front Lowering

Halcyon resolves one ordered set of coverage regions for the whole admitted
contour. Resolution is pure and deterministic:

1. compute every front's shifted/clamped arc-length range;
2. add nine uniformly spaced arc-length knots, including both endpoints, for
   every range;
3. visit source segments in input order and split them at contained knots;
4. at every emitted endpoint compute the maximum of all admitted front bell
   samples, where one sample is `intensity * smoothstep(0, 1, triangular)` and
   `triangular` is one at the resolved range centre and zero at its endpoints;
5. emit only positive-length finite subsegments; and
6. interpolate adjacent endpoint samples in the shader with
   `smoothstep(0, 1, segment_t)` so value and first derivative remain continuous
   across every emitted boundary.

The implementation uses one shared visitor for count calculation and GPU
vertex generation. Its bounded scratch storage is inline; it must not allocate
a second point list or allow batch counts and emitted geometry to use different
lowering algorithms.

Front overlap combines by `max` before rendering, never by drawing another
layer. Halcyon does not infer that two locations describe one event; `max` is
only the bounded visual-energy rule for this paint primitive.

At each non-reversing source joint, adjacent analytic segment regions use one
shared ownership plane through the supplied point. The plane normal is the
normalized sum of the incoming and outgoing unit directions. The preceding
region owns the non-positive half and the following region owns the positive
half. Internal front-knot splits use the segment direction as the ownership
normal. Open contour endpoints keep the ordinary round capsule cap. Thus an
ordinary straight, curved, or corner joint contributes coverage exactly once.

An exact retrace or non-adjacent self-intersection is repeated caller geometry,
not a joint. It retains ordinary premultiplied OVER composition. Eliminating
that overlap would require path-union/topology authority deliberately excluded
by this SPEC.

---

## 4. Shape-Pipeline Rendering Contract

### 4.1 Existing pipeline only

`LuminousContour` uses `ShapeType::LuminousContour` (`shape_type = 4`) in the
existing `ShapePipeline`. It introduces no shader module, entry point, bind
group, texture, sampler, render target, or post-process pass. Existing shape IDs
remain unchanged.

Each resolved coverage region emits one expanded candidate quad. The fragment
shader clips that candidate to its documented start/end ownership half-planes
before evaluating coverage. Shape indices are `u32` so aggregate multi-surface
vertex bases cannot wrap at `u16::MAX`.

### 4.2 Stable parameter packing

For a luminous segment, `ShapeVertex::params1` means:

```text
x = core half-width in logical pixels
y = halo radius beyond the core edge in logical pixels
z = base halo intensity in [0, 0.25]
w = halo falloff exponent in [2, 8]
```

`ShapeVertex::params3` means:

```text
x = base core luminance lift in [0, 0.75]
y = inner aura radius beyond the core edge in logical pixels, in [0, 4]
z = base inner aura intensity in [0, 0.75]
w = reserved zero
```

`ShapeVertex::params2` contains the two finite ownership-plane normals:

```text
xy = start ownership normal, or (0, 0) for an open start cap
zw = end ownership normal, or (0, 0) for an open end cap
```

`ShapeVertex::contour_gains` contains the front gain at the region start and
end. All four vertices of one candidate quad carry the same two values. The
shader derives longitudinal `segment_t` analytically from `params0`; no vertex
interpolation or hidden flag determines ownership.

Ordinary existing analytic shape test data retains
`params1 = [half_width, 0.0, 0.0, 0.0]`, zero `params2`, and zero
`params3` and `contour_gains`. No flag is hidden in float bits, signs, NaNs,
colors, or unused padding.

The quad expands by:

```text
core_half_width + halo_radius + one logical AA pad
```

### 4.3 Fragment coverage

Let `sd` be signed distance from the fragment to the core stroke boundary and
`aa` the existing target-derived logical AA radius.

```text
core = 1 - smoothstep(-aa, aa, sd)
outside = max(sd, 0)
inner_t = clamp(1 - outside / inner_aura_radius, 0, 1)
inner_edge = 1 - smoothstep(
    max(inner_aura_radius - aa, 0),
    inner_aura_radius + aa,
    outside,
)
inner = inner_aura_intensity * pow(inner_t, 2) * inner_edge
outer_t = clamp(1 - outside / halo_radius, 0, 1)
outer_edge = 1 - smoothstep(max(halo_radius - aa, 0), halo_radius + aa, outside)
outer = halo_intensity * pow(outer_t, halo_falloff) * outer_edge
aura = max(inner, outer)
coverage = core + (1 - core) * aura
```

Each aura zone whose radius or effective intensity is zero is skipped before
division or `pow`. The outer zone remains bounded by `halo_radius`; the admitted
zone order therefore preserves the candidate-quad expansion in §4.2.

In the compatible single-zone branch, output remains exactly:

```text
alpha = color.a * (core + (1 - core) * outer)
output = (color.rgb * alpha, alpha)
```

In the zoned branch, the two aura zones combine by `max`, not addition or
another OVER layer. Output remains premultiplied:

```text
core_alpha = color.a * core
aura_alpha = color.a * (1 - core) * aura
lifted_core_rgb = mix(color.rgb, linear_white, effective_core_luminance_lift)
alpha = core_alpha + aura_alpha
output = (
    lifted_core_rgb * core_alpha + color.rgb * aura_alpha,
    alpha,
)
```

Thus the crisp core keeps its exact geometric width and alpha, the inner aura
provides saturated near-field color, and the wider outer halo remains weak and
source-hued. No zone emits separate geometry or a separately blended layer.

### 4.4 Front appearance

Front emphasis is part of the base region's single fragment evaluation. Let
`g0` and `g1` be the endpoint gains from §3:

```text
h = smoothstep(0, 1, segment_t)
front_gain = mix(g0, g1, h)
```

For the compatible single-zone branch:

```text
effective_halo_intensity = min(0.35, base_halo_intensity + 0.10 * front_gain)
```

For the zoned branch:

```text
effective_core_luminance_lift = min(
    0.90,
    base_core_luminance_lift + 0.20 * front_gain,
)
effective_inner_aura_intensity = min(
    1.00,
    base_inner_aura_intensity + 0.25 * front_gain,
)
```

The zoned outer halo is not front-modulated. Core width, core alpha, source hue,
both radii, and both falloff rules remain identical to the base contour. Front
emphasis therefore continuously brightens only the thin core and saturated
near-field aura along the supplied arc-length range, without a second core,
independent marker, abrupt range end, or another OVER contribution.

---

## 5. Coordinates, DPR, Clipping, and Ordering

- Points, widths, radii, distances, and extents are absolute logical 2D paint
  coordinates.
- The renderer's immutable per-target globals derive AA from the target's
  logical projection and physical surface size. Retina/high-DPI changes sample
  density, not logical contour dimensions.
- Existing scroll translation is captured during batching without copying the
  shared point array.
- Existing clip state and deterministic z/batch ordering apply unchanged.
- A luminous contour has no layout size, hit target, input policy, or focus
  behavior.

---

## 6. Ordinary Stroke Compatibility

Existing `LineNode`, `PolylineNode`, `SceneBuilder::push_line`,
`SceneBuilder::push_polyline`, and their path-stroke lowering do not change.
They never acquire luminous defaults, expanded bounds, front data, or shape
pipeline routing.

`LuminousContour` is a separate explicit primitive. Reducing its halo intensity
toward zero continuously removes halo coverage; zero halo intensity with no
fronts leaves only its analytic core. This convergence does not claim pixel
identity between two separately governed stroke implementations.

**INV-358-ORDINARY-COMPAT:** A scene containing no `LuminousContour` nodes must
produce the same ordinary line/polyline scene data, path draws, shader inputs,
and visible result as before this SPEC.

---

## 7. Allocation, Update, and Lifecycle

- Contour points are shared immutable `Arc<[Point]>` data. Scene and batch
  transitions perform reference-count increments only.
- The inline front set is copied by value. Front movement creates no front-list
  heap allocation.
- Front subsegments are visited directly into the renderer's shape vertex/index
  build; there is no intermediate subsegment vector.
- Existing shape GPU buffers grow geometrically and reuse capacity. Changed
  contours rewrite the relevant frame's packed vertices/indices through the
  normal renderer-owned staging path.
- Static keyed/retained scene fragments retain the complete immutable node under
  SPEC-HAL-354. This SPEC adds no mutable retained handle or consumer-writable
  GPU resource.
- Destruction, target loss, resize, suspension, and recovery remain ordinary
  SceneGraph/renderer lifecycle. No contour-owned cleanup callback exists.

The current renderer still rebuilds transient CPU shape vectors for a changed
paint frame. This SPEC does not claim in-place persistent GPU contour mutation.
Adding such a resource API would require separate lifecycle and budget
authority.

---

## 8. Invariants and Prohibitions

**INV-358-VALIDATED:** Invalid geometry, color, appearance, front capacity, or
front association returns `LuminousContourError` before a node enters the
SceneGraph. No clamp, truncation, NaN sentinel, or partial node is permitted.

**INV-358-ONE-PASS:** Every resolved luminous coverage region uses the existing
shape render pass and premultiplied OVER target composition. Front emphasis
does not emit a second render primitive.

**INV-358-SINGLE-OWNER-JOIN:** Every admitted non-reversing adjacent joint is
partitioned by complementary ownership half-planes. It cannot accumulate
multiple core or halo contributions merely because the contour changes
direction or closes by repeating its first point.

**INV-358-LOCAL-HALO:** Halo coverage is bounded by the admitted logical radius
around each supplied segment. It cannot brighten unrelated scene content.

**INV-358-ZONED-MATERIAL:** Zoned core, inner-aura, and outer-halo coverage are
three regions of one analytic fragment evaluation. Inner and outer aura combine
by `max`; they never accumulate as independently blended geometry.

**INV-358-MULTIPLE-FRONTS:** Zero, one, and multiple front ranges are ordinary
values of the same API and rendering path.

**FORBID-358-TEMPORAL-SEMANTICS:** Halcyon must not store or infer reveal
progress, time, speed, direction, play state, event identity, or animation.

**FORBID-358-INFERRED-TOPOLOGY:** Halcyon must not infer missing segments,
closure, paths, contours, surfaces, or product topology from visual proximity.

**FORBID-358-BLOOM:** No offscreen bloom, blur pass, post-process graph,
emissive scene model, texture glow, or global light/theme state is admitted.

**FORBID-358-ORDINARY-STROKE-DRIFT:** Existing ordinary line/polyline nodes,
lowering, batching, and appearance must not change.

**FORBID-358-UNBOUNDED-HALO:** The renderer must not accept non-finite or
out-of-range halo/front values or expand geometry beyond the admitted radius.

**FORBID-358-FRONT-MARKER:** A front must not lower to an independent thick dot
or circle. It is always a positive-length range on supplied contour segments.

**FORBID-358-FRONT-OVERLAY:** A front must not emit another stroke, quad, core,
or independently blended layer. Its only renderer effect is the documented
compatible halo modulation or zoned core/inner-aura modulation.

**FORBID-358-HIDDEN-PACKING:** Shader behavior must not depend on undocumented
float bits, negative sentinels, NaNs, padding, or color-channel overloading.

**FORBID-358-SCOPE-SMUGGLING:** This unit must not add application-specific types, temporal
models, path authoring, selection, hit testing, interaction, 3D emission,
labels, global bloom, or arbitrary effects.

---

## 9. Required Example

The repository provides one native black-background example that shows:

1. ordinary and luminous strokes side by side;
2. a two-point line, an open sampled curve, a closed square, and a sampled
   closed circle;
3. restrained cyan/teal and amber colors;
4. subtle, balanced, and strong bounded zoned materials whose core widths match
   the adjacent ordinary strokes;
5. zero, one, and multiple supplied fronts; and
6. example-local moving front locations without exposing animation or time to
   the luminous API.

The example displays its current core luminance lift, inner-aura radius and
intensity, outer-halo radius/intensity/falloff, fixed core alpha, front strength,
and front half-extent. It provides `SUBTLE`, `BALANCED`, and `STRONG` live
presets plus direct bounded controls for both aura zones, core lift, and front
strength/half-extent. `BALANCED` is the startup default and must read as
unmistakably luminous beside the same ordinary stroke at normal window size:
a thin pale source, rich cyan/amber near-field, and faint wider separation from
black, without broad fog, a thicker core, marker-like white fronts, tubes, or
join hotspots.

Live tuning changes retained paint state only. They must neither request a
structural screen rebuild nor restart the moving-front phase. If the framework
does request a legitimate screen rebuild, the example must mount exactly one
root child again.

The example must resize in logical coordinates, run on the normal native app
path, preserve framework Escape ownership, and expose renderer cadence evidence
under the existing `render-audit` feature.

---

## 10. Conformance Proof

Implementation is conforming only with executed proof of:

1. all appearance, geometry, color, front, and capacity validation boundaries;
2. stable front range shifting/clamping, bounded knot generation, continuous
   endpoint gain, and deterministic multi-segment lowering, including endpoint,
   overlapping, and multiple-front cases;
3. exact `params1`, `params2`, `params3`, and `contour_gains` packing;
4. candidate-quad expansion by core half-width plus halo radius plus AA pad;
5. complementary single-owner clipping at straight, corner, and explicitly
   closed joints, with no joint hotspot;
6. exact compatible single-zone output plus zoned core/inner-aura front
   modulation with unchanged core width/alpha and no front overlay geometry;
7. `u32` shape indices and correct multi-surface rebasing;
8. ordinary `LineNode`/`PolylineNode` continuing through their unchanged path
   route when no luminous node exists;
9. clip, scroll, z-order, resize, and DPR integration through the existing
   renderer contracts;
10. the dedicated native example, native visual capture, and render-audit
    cadence evidence; and
11. one package check after focused tests.

Visual owner review remains the acceptance boundary for the final intensity,
radius, and front-emphasis calibration. A green test cannot prove taste.

---

## 11. Deliberately Excluded Authority

This SPEC does not authorize:

- reveal/progress/timeline state or application-specific concepts;
- full-screen bloom, HDR, offscreen blur, or post-processing;
- arbitrary path effects, Bezier stroking, SVG, or a generic vector framework;
- topological path union or de-duplication of explicit retraces and
  non-adjacent self-intersections;
- 3D luminous geometry, emissive materials, scene lights, or depth-aware halos;
- texture-backed contours, gradients, dashes, labels, picking, or interaction;
- consumer-writable GPU buffers or a persistent contour resource API; or
- inference of topology, closure, semantics, or front locations.

## 12. Scoped contour-front opacity response (2026-11-12)

**Status:** Adopted / Binding for this additive, backward-compatible contour-material extension. JC authorized the scoped extension to the existing GPU primitive while retaining final visual acceptance. This adoption does not activate HAL-384 or change ordinary Button, theme or focus semantics.

**Motivating native case:** Section 4.4 fixes source core alpha while varying only core color lift and aura intensity. A 68×32 selected navigation control required a nearly absent dark base perimeter with a concentrated, more opaque reflective lip at the lower center. A fixed core alpha cannot express both effects through one contour. The extension therefore uses the existing contour value and shape shader; a separate line below the Button or a general surface graph is outside this contract.

**INV-358-FRONT-OPACITY-OPT-IN:** Add exactly one finite material value, `front_opacity_boost: f32`, in [0,1], to the existing checked `LuminousContourAppearance`. The existing `try_new` and `try_new_zoned` constructors MUST produce exactly zero boost. Provide the immutable, checked builder `with_front_opacity_boost(self, boost: f32) -> Result<Self, LuminousContourError>` and a read-only accessor. Non-finite, negative or greater-than-one inputs fail through a distinct typed error before node emission. Zero boost MUST preserve existing value semantics and exact renderer inputs and output; no user, scene or runtime global controls are added.

**INV-358-ABI-LANE:** The existing `ShapeVertex.params3.w`, formerly reserved zero for `ShapeType::LuminousContour=4`, carries the validated `front_opacity_boost` and is zero for all ordinary shapes and all existing contour constructors. The other three params3 lanes, params0/1/2, contour_gains and vertex stride, attribute locations, ShapeType IDs, shader module, pipeline and clip/target/retained ownership are unchanged. This explicit semantic assignment supersedes only the earlier params3.w reserved-zero statement in section 4.2 and its exact-zero assertions. No other padding or field is repurposed.

**INV-358-SINGLE-PASS-ALPHA-RESPONSE:** In the existing zoned contour fragment path, calculate the existing bounded `front_gain` from the already admitted, arc-length-sampled contour fronts. Define `source_alpha = input.color.a` and `effective_source_alpha = source_alpha + (1 - source_alpha) * front_opacity_boost * front_gain`. For the same existing core and aura coverage, replace the zoned branch's constant `source_alpha` multiplier by `effective_source_alpha` for core_alpha and aura_alpha, retaining the existing core RGB lift and source-hue aura, premultiplied-linear RGBA output and one analytic candidate-quad pass. Bounds [0,1] on all inputs imply effective_source_alpha in [source_alpha,1]. All front distances/half-extents remain consumed strictly through existing HAL-358 knot resolution. The compatible single-zone constructor path stays byte-equivalent when boost=0; older zoned materials with boost=0 MUST likewise render with unchanged alpha, RGB and coverage. The shader's `zoned` branch predicate includes positive front_opacity_boost for valid opacity-only contours and remains false for all old zero-zoned values.

**FORBID-358-ALPHA-OVERLAY:** No second border/stroke/draw, independently blended highlight, thicker core, static bottom underline, new texture, offscreen glow, temporal behavior, extra persistent resource, arbitrary BRDF graph, direct consumer shader handle or override of the existing normal focus and clip policy. This change does not authorize independent per-front hue, physically accurate metal response or fake PBR claims. The consuming application supplies both front coordinates and boost within its own selected navigation-control paint.

**Reference fixture and verification:** Use the original 29-point rounded contour on the 68×32 selected navigation control, with an authored source hue and bottom-center arc-front placement. The reflective-lip fixture MUST use low base core alpha plus positive boost concentrated at that front; without this explicit selection all other contours remain visually identical. Verify input validation and unchanged old constructors, exact `ShapeVertex.params3.w` data and shader reads, finite premultiplied-alpha bounds (including source alpha 0 and 1 and overlapping fronts), identity, strict Z, ordinary Line/Polyline invariance, and native 1×/2× where physically available. A focused native example or the original consuming application establishes the visual result. Final aesthetic approval remains separate from numerical and structural proof.

### 12.1 Exact zero-source-alpha contour eligibility - source-qualified owner correction

**Status:** Adopted / Binding addition to section 12. Governing evidence: S1 Apple M4 Max / Metal RED on a real zero-source-alpha opacity-front contour, after successful positive-alpha readbacks. All experimental production edits were restored; the live renderer is unchanged.

**INV-358-FRONT-OPACITY-ELIGIBILITY:** In the existing src/renderer/batcher.rs Batcher::add_luminous_contour and in the existing src/renderer/wgpu_renderer.rs PrimitiveData::LuminousContour shape-lowering match guard, admit that one contour iff source_color.a >= 0.001 OR appearance.front_opacity_boost() > 0.0. Equivalently, discard iff source_color.a < 0.001 AND appearance.front_opacity_boost() == 0.0. The appearance is already checked finite in [0,1]. Those are the ONLY two additional code owners in this section 12 implementation. Do not change ordinary Line/Polyline/Shape or other alpha guards, point geometry, shadow/border, input, clip, Z, or any separate GPU pass.

All legacy constructors still generate boost=0 and MUST preserve their exact old 0.001 eligibility decision and original shader output. For boost>0 at source alpha=0, eligible geometry reaches the same analytic contour shader; only a positive front_gain can create positive effective_source_alpha, while gain zero MUST produce zero output. No overlay, additional line, opacity fallback, backdoor theme or alternate paint path is permitted.

**Required RED-to-GREEN proof:** Reuse the original S1 native Metal readback and require the specific source alpha=0, boost=.8, real peak front positive premultiplied RGBA and exact zero outside the front; legacy zero-alpha boost=0 remains skipped. Preserve positive-alpha 1x baseline, 2x when supported, old shader/zero-boost compatibility, overlapping fronts, valid input rejection, bounded output and original non-luminous behavior. Do not relax or delete the failing zero-alpha test. This additional two-guard correction is necessary to finish the existing user-visible objective, not a new subsystem or optional unsupported-space test.