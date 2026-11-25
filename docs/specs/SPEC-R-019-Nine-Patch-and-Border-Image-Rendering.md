# SPEC-R-019: Stretch Nine-Patch Surface Material

**Author:** JC
**Status:** Adopted / Binding
**Adopted:** 2026-10-06
**Implementation status:** Implemented
**Layer:** Pure rectangular surface values and existing Image primitive lowering
**Scope:** Stretch NinePatch material, its initial ordinary Button appearance profile, and required implementation proof
**Authority:** Subordinate to SPEC-HAL-002 and SPEC-HAL-002A; existing appearance, coordinate, resource, retained-scene and renderer owners retain their authority.

This in-place revision supersedes the old Deferred **Nine-Patch & Border-Image
Rendering (UI Skinning v1)** content, including its dependency list, standalone
GPU subsystem, layout claims and implementation phases. Those earlier claims
carry no authority under this revision. Adoption establishes the contract below,
not execution support.

## 1. Purpose and governing owners

Stretch NinePatch supplies a rectangular surface by stretching a bounded 3×3
partition of one Image source into an externally allocated destination rect.
Corners and edges use configured logical destination border extents; the center
uses the remaining extent. Small destinations compress those borders under §4.

This is a complete bounded material family. Repeat, Round and CSS border-image
are separate excluded capabilities, not later phases owed by this contract.
Any future justification for them requires separate exact authority.

NinePatch MUST extend the existing `SurfaceFill<Source>` material vocabulary and
lower to ordinary Image primitives. It MUST NOT introduce a renderer subsystem.
The governing owners are:

- [SPEC-HAL-002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md) and
  [002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md): phase separation,
  immutable paint/scene values and ownership.
- [SPEC-HAL-012](SPEC-HAL-012-Theming.md): typed appearance provision, component
  content/geometry boundaries, activation and replacement.
- [SPEC-HAL-024](SPEC-HAL-024-Coordinate-Model.md): canonical logical coordinates,
  traversal-owned absolute placement/clips and renderer-owned device conversion.
- [SPEC-HAL-263](SPEC-HAL-263-Image-Pipeline-and-Texture-Resource-Architecture.md),
  [263A](SPEC-HAL-263A-Image-Alpha-Contract-Correction.md) and
  [263B](SPEC-HAL-263B-Image-Registration-Lifetime-Correction.md): Image identity,
  sampling, alpha/color, admission, registration generation and lifetime.
- [SPEC-HAL-040](SPEC-HAL-040-GPU-Resource-Lifetime-Management.md): existing GPU
  resource execution and lifetime ownership.
- [SPEC-HAL-083](SPEC-HAL-083-Unified-SceneGraph-Diff-Engine-v2.md): frozen
  widget-owned visual-root identity and unchanged scene-key emission plumbing.
- [SPEC-HAL-354](SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md): exact
  retained dependencies and narrow paint/composition invalidation.
- [SPEC-HAL-359](SPEC-HAL-359-Cross-Pipeline-Z-Order-Execution.md): strict external
  Z ordering and the absence of equal-Z layering semantics.

Links do not adopt lower-status proposals or their scoped proposed amendments.
No legacy rendering dependency is adopted by the superseded dependency list.

## 2. Material value and asset boundary

The conceptual value is geometry-independent. Exact Rust spelling is
non-normative; all of these inputs and their meanings are required:

| Input | Binding meaning |
|---|---|
| Typed Image source | The same source-owner boundary as `SurfaceImage<Source>`. Execution uses the current admitted `ImageBinding` wherever the consumer profile requires it. No GPU handle or resource ownership is carried. |
| `uv_rect` | One normalized positive-area source sub-rectangle. |
| Source split insets | Left, top, right and bottom fractions relative to the width/height of this `uv_rect`, not the full texture or physical pixels. |
| Destination border insets | Left, top, right and bottom extents in logical units, used only to partition the supplied paint rect. |
| Tint | Existing straight-linear RGBA `Color` with exactly the ordinary Image modulation semantics. Tint alpha supplies opacity; there is no separate opacity field. |

The application/asset owner maps authored identity to admitted image evidence.
Authored source-pixel slices and density metadata MAY exist in that owner's data;
that owner MUST resolve them into normalized source splits and logical
destination insets before supplying the pure appearance/paint value. Appearance,
paint and renderer MUST NOT discover source dimensions, files, density or assets
to interpret NinePatch.

These insets are paint values. They provide no intrinsic size, measurement,
padding, layout or hit-test input. Source pixels, logical destination extent and
target device scale remain distinct. No NinePatch value contains DPR or performs
device conversion.

## 3. Validation and activation

Construction and appearance activation MUST validate deterministically and return
typed rejection for malformed supplied values. Validation MUST require:

1. every floating-point input to be finite;
2. `uv_rect` to lie wholly within `[0, 1]²`, with positive width and height;
3. all source split insets to be nonnegative, with `left + right < 1` and
   `top + bottom < 1`, preserving positive source center strips;
4. all destination border insets to be nonnegative and finite; and
5. tint to satisfy the existing finite unit-range straight-linear `Color`
   contract, without conversion or premultiplication by the material.

The typed outcome MUST distinguish invalid UV selection, invalid source splits,
invalid destination insets and invalid tint. It MUST NOT repair invalid values by
clamping, guessing, substituting another material or ignoring a field. There is
no resource lookup, availability polling or GPU work in pure value validation.

Consumer capability and resource admission remain separate from value validity.
An unsupported required profile or unavailable required binding MUST reject
under the existing consumer/resource admission contract. An authored reference,
registration-number shape or queued request is not current binding evidence.
Successful value validation MUST NOT fabricate resource success or select an
implicit Solid fallback. Resource availability is checked at the existing legal
admission/Compile boundaries, not by polling during resolution, layout or paint.

## 4. Destination partition and permanent small-size behavior

The existing component/layout owner supplies the destination rect in logical
space. Its coordinates and nonnegative width/height MUST be finite under that
owner's geometry contract. A zero-area destination emits nothing. NinePatch
partitions this rect without moving or enlarging it.

For each axis independently, let `L` be destination length, `a` the configured
leading inset and `b` the trailing inset. Resolve leading border `A`, trailing
border `B` and center length `C` by the following mathematical contract:

| Condition | Resolved lengths |
|---|---|
| `L >= a + b` | `A = a`, `B = b`, `C = L - a - b` |
| `L < a + b` | `A = L * a / (a + b)`, `B = L * b / (a + b)`, `C = 0` |

When both configured insets are zero, both remain zero and the center is `L`;
division by zero is never needed. At equality the configured borders are
preserved and the center is zero. Compression is the permanent material
behavior, not a fallback or a request to change layout.

For axis origin `p`, destination split positions are:

`[p, p + A, p + L - B, p + L]`.

In the compressed case the two interior splits MUST be one shared coordinate,
so rounding cannot create a gap or overlap in the collapsed center. Derived
arithmetic MUST preserve finite, ordered boundaries within the destination,
including for large finite insets; an overflowing intermediate sum is not
permission to produce invalid geometry. There is no physical-pixel rounding,
minimum border width, epsilon expansion or DPR multiplication in this contract.

No resolved cell may have negative extent, overlap another cell's interior,
extend outside the destination or alter canonical layout geometry.

## 5. Source partition and affine cell mapping

Let `uv_rect = (u, v, du, dv)` and normalized source insets be `l`, `t`, `r`, `b`.
The source split positions are:

- horizontal: `[u, u + du*l, u + du*(1-r), u + du]`;
- vertical: `[v, v + dv*t, v + dv*(1-b), v + dv]`.

Form the Cartesian 3×3 grid of these source intervals and the independently
resolved destination intervals from §4. Destination cell `(column, row)` maps
affinely to the source cell at that same `(column, row)`, stretching its selected
UV interval across its selected logical interval. This includes corners; no
native source-pixel-size assumption is made.

Emit a cell only when both its destination and corresponding source rectangle
have positive, representable area. Zero-area destination cells emit nothing;
zero-area source cells do not invent texels or a replacement region. Adjacent
cells share their exact split boundaries. No edge/center tiling, repeat/round
calculation or additional UV transform occurs.

## 6. Lowering, ordering and clipping

The existing paint/scene path SHALL lower the selected NinePatch to at most nine
ordinary Image primitives, one per emitted cell. Scene construction remains
CPU-only; Compile performs the existing Image resource/geometry validation and
preparation; Execute consumes already-prepared resources under INV-IMG-001.

Every cell MUST use the same exact source and, on an admitted path, the same
`ImageBinding` registration generation, sampling policy and straight-linear tint.
All cells retain the canonical inherited clip scope and producer-selected Z of
the material. Lowering MUST NOT register, upload, normalize or replace an Image,
look up a replacement generation, or discover asset/source dimensions.

No new SceneGraph semantic hierarchy, pipeline, pass, shader, bind-group family,
texture uploader, resource registry, batching identity or lifetime is created.
Existing Image batching MAY mechanically merge compatible cells under the
existing exact batch key; merging does not change geometry or material meaning.

Cells have disjoint interiors. Their equal local Z carries no front/back
meaning. External overlapping content, borders or focus/capture decoration
remain producer-ordered under SPEC-HAL-359 and MUST use strict Z where order
matters. Emission order, cell index, texture identity and pipeline family MUST
NOT substitute for that order.

NinePatch creates no clip. Only canonical inherited traversal/consumer clips
already admitted by the Image path apply. It MUST NOT pop or escape a clip,
introduce arbitrary path masks, or claim its own rounded-corner execution.

NinePatch lowering MUST obey existing SPEC-HAL-083 identity plumbing unchanged:
at most one primitive per widget per frame may carry a `SceneNodeKey`, and the
key identifies that widget's visual root. Multiple Image cells MUST NOT receive
distinct scene keys or sub-keys, reuse one key across cells, or introduce nested,
stacked or subtree key scopes. NinePatch MUST NOT invent or extend identity.

When the consumer supplies its one widget-owned scene key for the material's
visual root, lowering MAY attach that exact key to at most one emitted Image
primitive: the first emitted cell in fixed row-major order (top to bottom, then
left to right within each row). Every other cell MUST use `None`. Without a
supplied key all cells use `None`; if no cells are emitted, no keyed primitive
is emitted. This consumes the existing widget-wide key allowance, not another
allowance for each material. The key remains visual-root identity, not cell
identity or a new component/member/widget identity.

The chosen keyed cell may change when cells become zero-area. This is a
visual-root update under the same widget-owned key, not stable per-cell identity.
Ephemeral `None` cells remain retainable only within their owning otherwise clean
widget fragment under SPEC-HAL-354 INV-354-FRAGMENT-4; dirtying that fragment
recreates them.

## 7. Alpha, registration lifetime and retained dependencies

SPEC-HAL-263/263A/263B apply unchanged to every cell. Sampled content is
premultiplied-linear; tint remains straight-linear; ordinary Image sampling,
modulation, coverage and source-over execution apply exactly once. NinePatch
introduces neither a content-admission boundary nor a second alpha/color
conversion. INV-IMG-002/003/005/006/007 remain binding: CPU identity only, explicit
registration lifetime, final Compile validation, no intrinsic scaling policy
and no atlas.

On the admitted profile, all emitted cells consume one exact Image binding as
one material dependency. Replacement, unregistration, renderer retirement or
device loss revoke that evidence under the existing resource law; no cell may
redirect to a later same-number generation. Same-device bind-group/pipeline
revalidation preserves generation and sampling under 263B.

The retained owner MUST track actually consumed NinePatch values and binding
generation under 354. Changed source selection, splits, destination insets, tint
or appearance state at a stable allocated rect invalidate the consuming local
paint fragment and required ancestor composition. Resource-only revocation does
the same before retained reuse, without dirtying layout or unrelated siblings.
Paint does not poll mutable binding availability or acquire resource lifetime.
There is no per-cell lifecycle, observer, clock or animation demand.

Unused NinePatch introduces no allocation, dependency table, traversal,
registration, upload or other Image/GPU work for existing Solid, Gradient or
ordinary Image paths. Existing paths keep their current lowering and owners.

## 8. Appearance and initial Button admission profile

Under 012, `SurfaceFill<Source>` is intended to gain a NinePatch variant carrying
§2's complete typed value. `AppearanceSurface` uses the same material boundary
and owner-supplied Image source evidence. This adoption creates no Brush,
DecorationStack expansion, DSL, cascade or material graph.

The initial implementation profile SHALL admit Stretch NinePatch for ordinary
Button surface and underlay roles only when all of these conditions hold:

- the NinePatch value passes §3;
- its source is a current admitted SPEC-HAL-263 §5.3 `ImageBinding`;
- its inherited clips are those supported by the existing Image execution path;
  and
- the Button's own surface radius is zero, analogous to its ordinary Image
  profile.

Activation SHALL validate the required evidence for every configured used state
through the existing appearance admission path and reject unsupported radius or
unavailable evidence atomically. Pure resolution remains immutable and does not
poll resource availability. State-specific Button values MAY choose distinct
NinePatch bindings/values through the existing owner-qualified state resolution.

Button identity, content, measurement, padding, layout, input, activation,
focus/capture ownership and required truthful focus/capture projection remain
unchanged. Destination border insets affect paint partition only. Replacement
uses the existing legal appearance Update and narrow dependency invalidation;
it MUST NOT reconstruct the widget or silently turn these insets into padding.

This is authority for the named initial profile, not an implementation claim or
automatic admission of every widget. Other consumer profiles require their
exact owner contract and implementation proof.

## 9. Binding invariants and prohibitions

- **INV-R019-VALUE:** One complete typed material satisfies §2/§3; malformed
  supplied values receive deterministic typed rejection before activation.
- **INV-R019-COORD:** Destination geometry/insets remain logical; source splits
  remain normalized relative UV values; device conversion retains its owner.
- **INV-R019-PARTITION:** §4/§5 produce bounded, nonnegative, disjoint cells;
  compressed centers and zero-area cells obey their exact omission law.
- **INV-R019-LOWERING:** Selected NinePatch emits at most nine existing Image
  primitives with one source/generation, sampling, tint, clip scope and Z.
- **INV-R019-RESOURCE:** No material/cell owns GPU resources or extends binding
  lifetime; registration/revocation and final validation retain the Image owner.
- **INV-R019-ALPHA:** Ordinary Image association, modulation and coverage apply
  once; NinePatch performs no conversion.
- **INV-R019-RETAINED:** Changed consumed values or binding evidence dirty only
  the affected paint/composition dependency; paint-only change does not dirty layout.
- **INV-R019-PAY-FOR-USE:** Non-NinePatch paths acquire no new material work or
  resource dependencies.

- **FORBID-R019-NEW-RENDERER:** No new pipeline, pass, shader, upload path,
  resource registry, atlas or GPU handle in appearance/scene values.
- **FORBID-R019-ASSET-DISCOVERY:** No file lookup, decoding, source-dimension
  discovery, automatic density selection or registration during pure traversal.
- **FORBID-R019-LAYOUT-INPUT:** No intrinsic sizing, padding inference, layout
  mutation, hit testing, input or semantic identity derived from this material.
- **FORBID-R019-CLIP-ORDER:** No new clip authority, own rounded mask, clip escape
  or equal-Z semantic ordering.
- **FORBID-R019-IMPLICIT-FALLBACK:** No guessed splits, silently changed material,
  fabricated binding or replacement-generation substitution.
- **FORBID-R019-EXTENSION-BY-ANALOGY:** No excluded capability or lower-status
  proposal is admitted by NinePatch terminology, generic storage or a reference.

## 10. Required implementation proof and completion boundary

Full implementation requires owner-correct production-path proof establishing
all of the following. Use the existing declared test/rendering
surfaces at the nearest owning boundary; do not build a hypothetical mode matrix.

1. Typed rejection covers malformed normalized UV/source splits, non-finite or
   negative destination insets and invalid tint without resource work or partial
   activation.
2. An asymmetric 3×3 fixture proves exact destination and UV correspondence,
   including a source sub-rectangle whose splits are relative to that selection.
3. Small-destination fixtures prove proportional border compression on each
   axis, exact center collapse, equality and zero-inset behavior.
4. Zero-area omission and shared boundaries prove no negative, overlapping or
   out-of-bounds cells, including omitted source strips.
5. Actual paint/scene lowering emits at most nine ordinary Image primitives with
   the exact binding generation, sampling, tint, producer Z and inherited clips;
   no new rendering family or resource preparation is used. A keyed consumer
   proves at most one emitted primitive carries its exact widget-owned key,
   selected by §6's rule, with all other cells using `None` and no duplicate or
   sub-key emission. Zero emitted cells carry no key; a changed keyed-cell choice
   preserves visual-root identity without per-cell keys. Ephemeral cell retention
   obeys INV-354-FRAGMENT-4.
6. All cells share Image generation/revocation behavior. Same-number replacement
   cannot redirect old cells; same-device revalidation preserves evidence;
   resource-only loss narrowly invalidates retained paint/composition before
   reuse while layout and unrelated siblings remain clean.
7. Solid, Gradient and ordinary Image keep their existing paths without new
   NinePatch allocation, dependency or Image/GPU work.
8. Real Button appearance activation and replacement use NinePatch surface and
   underlay values while preserving identity, content, layout/input semantics and
   required focus/capture projection; unsupported radius or unavailable evidence
   rejects atomically.
9. At least one actual production GPU/pixel or equivalent visibility proof shows
   the asymmetric corners, edges and center mapping under both stretch and a
   small destination. A CPU mapping oracle or reimplemented test shader alone
   cannot establish visible execution.

Adoption on 2026-10-06 changed authority only. Halcyon 0.203.0-beta implements the
core `SurfaceFill::NinePatch` material in `src/core/paint.rs`: immutable validated
`SurfaceNinePatch<Source>`, distinct normalized source and logical destination
inset types, and a shared non-allocating row-major partition. Both legacy and
admitted-binding PaintSink helpers lower through existing Image emission, with
the exact source/tint/Z and at most one supplied visual-root key.

The focused `surface_nine_patch_` public tests in `tests/surface_fill.rs` cover
typed value rejection, asymmetric destination/UV mapping and inherited clips,
independent border compression, equality/zero-inset behavior, large finite
insets, zero-area omission, deterministic single-key emission, allocation-free
cell partitioning, and unadmitted-source rejection at the Button boundary. The
existing Solid allocation proof is unchanged.

Halcyon 0.204.0-beta completes the initial ordinary Button surface/underlay
profile: current admitted `ImageBinding`, zero own radius, atomic capability and
resource rejection, existing admitted Image-cell lowering, and
`NinePatchImageCells` inspection. No other consumer or rounded profile is implied.

`nine_patch_button_admission_replacement_and_retained_generation_lifetime` in
`src/tests/retained_value_updates.rs` proves real Button publication, surface and
underlay cells with at most one widget-owned visual-root key, one exact binding
dependency per consumer after deduplication, and compilation of the actual Button
scene through the existing DrawPlan/renderer path. Paint-only replacement and
same-ID generation revocation preserve identity, layout, focus and owner-qualified
mouse pointer capture; only the consuming retained path is recomposed, no newer
generation is substituted, and removing the last consumer retires dependency
storage without polling or animation demand.

`nine_patch_production_image_pipeline_shows_stretched_and_collapsed_source_regions`
in `src/tests/image_alpha.rs` samples the actual lowered UV centers through the
existing production Image shader and registered-texture GPU readback helper. It
distinguishes all nine opaque source regions under asymmetric stretch and the
four surviving corners after small-destination center collapse. Adapter absence
does not count as passing this proof.

The focused `verify package halcyon test nine_patch check timeout 600` invocation
passed check with zero errors and all ten selected tests, including both proofs
above and the eight core mapping/value tests, with zero failures or ignored tests.
The named implementation contract is complete.

## 11. Explicit non-goals

This revision does not authorize:

- Repeat, Round, CSS border-image or edge/center tiling;
- UV transforms or new animation authority;
- mipmaps, texture atlases, source-dimension discovery or automatic density selection;
- asset lookup/loading/decoding, caching or generalized asset management;
- own rounded radius/mask or arbitrary path clipping;
- a new shader, generic Brush/material graph or generic skin DSL;
- intrinsic sizing, layout padding inference or layout mutation;
- hit testing, input, widget/member identity or a new scene semantic hierarchy;
- full theming migration, DecorationStack redesign or generalized consumer admission.
