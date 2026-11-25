# SPEC-HAL-263A: Image Alpha Contract Correction

**Status:** Adopted / Binding
**Date:** 2026-08-21
**Amendment Type:** D — Emergency Correction
**Owner-authorized ingress extension:** 2026-10-06 (JC)
**Amends with literal replacement text:** SPEC-HAL-263 §3.2; SPEC-HAL-085A §6.4
**Parent scope read subject to this amendment:** SPEC-HAL-263 §§4.2–4.3, 6.1, 7.1, 8, and 12
**Depends on:** SPEC-HAL-024, SPEC-HAL-040, SPEC-HAL-085A, SPEC-HAL-096, SPEC-HAL-150, SPEC-HAL-263
**Extended by:** SPEC-HAL-355 (Adopted / Binding; 3D-front specialization of the Image alpha contract without changing Image semantics)

This amendment corrects the Image pipeline only; it does not change the color
contract of another pipeline.

---

## 1. Purpose

Halcyon's scene-boundary `Color` is straight (unassociated) linear RGBA, while
fragment outputs use premultiplied linear RGBA with source-over blending. At
adoption, the implemented Image path contradicted that boundary in two places:

- SPEC-HAL-263 calls `ImageNode.tint` premultiplied even though the CPU `Color`
  value forwarded by the scene and renderer is straight linear RGBA;
- `image.wgsl` treats both tint and sampled texture as premultiplied and
  multiplies sampled alpha into RGB a second time, producing `texture_alpha²`
  behavior for fractional-alpha images.

SPEC-HAL-085A §6.4 also calls the Image tint premultiplied. Its required
premultiplied fragment output is correct; its Image input association and
modulation equation are not.

This amendment establishes one conversion boundary and one shader equation so
alpha is applied exactly once.

---

## 2. Ownership of This Amendment

SPEC-HAL-263A owns:

1. `ImageNode.tint` alpha association and color space;
2. the sampled texture alpha contract for the Image pipeline;
3. normalization performed by high-level RGBA8 registration;
4. preconditions of low-level texture/view registration;
5. Image shader modulation and coverage order;
6. fractional-alpha production-pipeline evidence.

It does not own image loading, decoding, caching, texture lifetime, custom GPU
admission, target DPR, device-loss recovery, blend-mode expansion, HDR, or the
alpha contract of Rect, Text, Path, Gradient, Shadow, or Shape pipelines.

---

## 3. Normative Definitions

### 3.1 Straight Linear RGBA

A color `(r, g, b, a)` is straight linear RGBA when `r`, `g`, and `b` are
linear-light color components independent of `a`. Halcyon CPU `Color` and
`ImageNode.tint` use this representation.

### 3.2 Premultiplied-Linear Sample

A sampled texel `T` is premultiplied-linear when sampling yields:

```text
T.rgb = straight_linear_rgb * T.a
T.a   = straight_alpha
```

For an sRGB texture, storage RGB may be sRGB encoded, but the texture view's
sRGB decode must yield the premultiplied linear values above. Alpha is linear
and is not transformed by the sRGB transfer function.

### 3.3 Straight sRGB RGBA8 Input

The byte input to `register_image_rgba8_srgb()` and
`GpuResources::queue_image_rgba8_srgb()` is conventional straight sRGB RGBA8:
RGB bytes encode straight sRGB color and the alpha byte encodes linear
coverage/opacity.

### 3.4 Coverage

`coverage` is the final scalar clip/edge coverage in `[0, 1]`. It modulates all
four components of an already-premultiplied source exactly once.

---

## 4. Binding Invariants

### INV-263A-001 — Scene Tint Is Straight Linear

`ImageNode.tint` and the Image vertex color are straight linear RGBA. CPU scene
construction and `add_image_quad` forward `[r, g, b, a]` without
premultiplication.

The Image shader is the sole owner of tint-alpha premultiplication.

### INV-263A-002 — Sampled Texture Is Premultiplied Linear

Every texture view admitted to the Image sampling pipeline must yield
premultiplied-linear RGBA when sampled. Filtering therefore interpolates
premultiplied values and cannot introduce straight-alpha dark or colored halos
at transparent edges.

### INV-263A-003 — High-Level Registration Normalizes Once

`register_image_rgba8_srgb()` continues to accept straight sRGB RGBA8. Before
upload, it converts each texel to premultiplied-linear content encoded for the
sRGB texture format.

For normalized byte components `s_r`, `s_g`, `s_b`, and `a`, each RGB channel
is transformed as:

```text
linear(c) = c / 12.92                              when c <= 0.04045
            ((c + 0.055) / 1.055) ^ 2.4           otherwise

srgb(c)   = 12.92 * c                              when c <= 0.0031308
            1.055 * c ^ (1 / 2.4) - 0.055         otherwise

stored_rgb = srgb(clamp(linear(s_rgb) * a, 0, 1))
stored_a   = a
```

Stored components are rounded to the nearest representable RGBA8 value. If
`a == 0`, stored RGB must be zero. No later upload or shader stage repeats this
texture-alpha multiplication.

Normalization is confined to the upload content created for this high-level
call. Delegating the resulting texture to `register_image_texture()` does not
make that low-level method a normalizer and must not rewrite any caller-owned
texture, view, pending registration, or custom front.

For the production `Rgba8UnormSrgb` upload format, straight input
`[255, 0, 0, 128]` therefore normalizes to stored bytes
`[188, 0, 0, 128]`: `a = 128/255`, `srgb(a) * 255 =
187.84497019765465`, and the value is not a rounding tie.

### INV-263A-004 — Low-Level Registration Is an Explicit Precondition

The live public Image content-admission/publication boundaries are the following
closed set:

1. `WgpuRenderer::register_image_rgba8_srgb()` is the direct high-level
   normalizing boundary and the shared normalization/upload implementation.
2. `GpuResources::queue_image_rgba8_srgb()` is the deferred high-level
   normalizing ingress. It accepts owned decoded straight-sRGB RGBA8 pixels,
   validates dimensions, checked byte count, exact length, and the device's 2D
   texture dimension limit before queueing any work, and returns a pending
   `ImageBindingRequest` paired to that call's exact registration generation.
   Rejection is synchronous and typed. The renderer drain MUST call the direct
   implementation in item 1 and issue that entry's evidence before a later
   queued entry can replace it. Under SPEC-HAL-263 §5.3, renderer retirement
   closes the ingress, releases queued pixels, cancels pending requests, and
   rejects later calls as `RendererDestroyed` before accepting work. This ingress
   neither normalizes on the caller nor admits a premultiplied view.
3. `WgpuRenderer::register_image_texture()`,
   `WgpuRenderer::register_image_from_view()`, and
   `WgpuRenderer::register_image_from_view_nearest()` are direct low-level
   boundaries.
4. A public `PendingImageRegistration` pushed through
   `GpuResources::pending_registrations` is the deferred low-level boundary.
   `WgpuRenderer::drain_pending_image_registrations()` drains these low-level
   entries into the direct view-registration methods without conversion.
5. `CustomRenderPass::new()`, `CustomRenderPass::new_with_filter()`, and the
   `CustomRenderPass` / `CustomRenderLifecycle` publication operations
   `queue_front_registration()`, `register_front_now()`, `submit_and_swap()`,
   `resize()`, and `resize_registered()` are low-level wrappers over the direct
   or deferred boundaries above.
6. `WgpuRenderer::admit_image()` and the renderer resolution of
   `GpuResources::request_image_binding()` issue generation-specific CPU
   evidence for an already registered entry under SPEC-HAL-263 §5.3. They admit
   no new pixels, perform no upload/conversion, and certify no historical
   content lease. The entry retains the association supplied at its actual
   registration boundary; receipt issuance cannot normalize low-level content.

Every low-level boundary in items 3–5 performs no alpha conversion or content
rewrite. Its caller/producer contract is:

- the sampled view must yield premultiplied-linear RGBA;
- its format and view interpretation must be compatible with the Image
  pipeline;
- the producer must satisfy the authoritative registration/publication ordering
  before the view is sampled; SPEC-HAL-295A same-queue publication requires
  enqueue-before-publication, not a CPU wait for GPU completion;
- transparent texels must satisfy the premultiplied condition, including zero
  RGB when alpha is zero.

Ownership transfer versus view registration does not change the alpha contract.
`PendingImageRegistration::nearest` and every corresponding nearest-selection
operation change filtering only; they do not change color space or alpha
association. `Plot::register_texture()` is a compatibility no-op, and Image
unregistration/retirement operations admit no content, so neither is an
additional association boundary. A new operative public Image admission or
publication boundary requires this closed set to be amended explicitly.

### INV-263A-005 — Exact Modulation Equation

Let `T` be the premultiplied-linear sampled texel, `C` the straight-linear tint,
and `K` the final coverage. The Image fragment source is:

```text
source.rgb = T.rgb * C.rgb * C.a
source.a   = T.a   * C.a
output     = vec4(source.rgb, source.a) * K
```

This is the only conforming Image modulation equation. The result is
premultiplied linear RGBA and is composited with the existing
`One / OneMinusSrcAlpha` source-over blend state.

### INV-263A-006 — Alpha Is Applied Exactly Once

Texture alpha contributes once because it is already present in `T.rgb` and is
used once in `source.a`. Tint alpha contributes once to both source RGB and
source alpha. Coverage contributes once to all four output components.

### INV-263A-007 — Opaque Compatibility

For `T.a == 1` and `C.a == 1`, the corrected equation reduces to:

```text
source.rgb = T.rgb * C.rgb
source.a   = 1
```

Therefore opaque textures rendered with opaque tints preserve their current
color and opacity, subject only to the same sRGB storage quantization already
present. Existing live Image consumers using opaque WHITE tint remain
behaviorally unchanged.

### INV-263A-008 — One Association Per Boundary

No public API may ambiguously accept either straight or premultiplied content.
The high-level boundaries in INV-263A-004 accept straight sRGB RGBA8 and
normalize it through the same renderer-owned implementation. Every direct,
deferred, or custom-render low-level boundary in that closed set accepts
already-premultiplied sampled content. `ImageNode.tint` is always straight
linear. Fragment output is always premultiplied linear.

---

## 5. Forbidden Behaviors

The following are non-conformant:

1. documenting or treating `ImageNode.tint` as premultiplied;
2. premultiplying CPU tint before vertex upload;
3. uploading straight-alpha RGBA8 unchanged through the high-level API;
4. accepting a straight-alpha low-level texture/view without an explicit
   conversion owned by its producer;
5. multiplying `T.rgb` by `T.a` in the Image fragment shader;
6. computing `output.a` with `T.a²`, `C.a²`, or coverage squared;
7. applying clip coverage to alpha but not RGB, or RGB but not alpha;
8. performing premultiplication after texture filtering;
9. silently guessing alpha association from texture contents;
10. using an Image-only correction to redefine another shader pipeline.

---

## 6. Literal Replacements and Parent Integration

### 6.1 SPEC-HAL-263

At adoption:

- §3.2 “`tint` is premultiplied RGBA multiplier” becomes “`tint` is a
  straight, linear RGBA multiplier; the Image shader applies tint alpha exactly
  once.”

The proposal backlink anchored at SPEC-HAL-263 §3.2 then becomes this normative
parent clause:

> SPEC-HAL-263 §§4.2–4.3, 6.1, 7.1, 8, and 12 are read subject to
> SPEC-HAL-263A. That amendment owns the distinction between the
> high-level straight-sRGB normalizing boundaries and every low-level
> premultiplied-linear boundary, the Image shader equation, the fact that
> bind-group ownership does not change association, straight-linear vertex
> tint, and the binding Image alpha invariants.

No existing prose in those sections is mechanically replaced by this clause.

No texture identity, lifetime, batching, clip, or RenderGraph rule changes.

### 6.2 SPEC-HAL-085A §6.4

At adoption, “Color (premultiplied tint)” became “Color (straight linear
tint).” The former premultiplied modulation block, `INV-IMG-ALPHA-1`, and its
obsolete unwired-pipeline deferral were replaced by INV-263A-005.

SPEC-HAL-085A §7 remains unchanged: every fragment shader output must be
premultiplied. This amendment changes only how the Image pipeline reaches that
output from its two differently associated inputs.

---

## 7. Required Production-Pipeline Evidence

Adoption was conditioned on a headless GPU test path using the actual production
Image pipeline, bind-group layout, sampler, blend state, shader module, and
target format. The live high-level storage format is `Rgba8UnormSrgb`; the live
headless render target is `Bgra8UnormSrgb`. Sampling and blending occur in
linear space, and the target store applies the sRGB transfer. A cfg(test)-only
renderer helper may provide an offscreen target and readback; a reimplemented
test shader or CPU-only blend oracle is insufficient.

The target must contain a known opaque background and at least one interior quad
pixel unaffected by edge coverage. Raw headless readback is BGRA storage order;
all values below are logical RGBA after swizzle. The listed byte oracle is exact.
A backend-specific tolerance, if demonstrated necessary, may be at most one byte
per color channel, must be documented, and must not overlap a rejected result.

Required cases:

1. **Fractional texture alpha:** an exactly representable premultiplied-linear
   source, such as an `Rgba16Float` texel `T = (0.5, 0, 0, 0.5)`, with opaque
   WHITE tint over opaque blue yields linear `(0.5, 0, 0.5, 1)`. The production
   sRGB target stores logical RGBA `[188, 0, 188, 255]`. The current
   alpha-squared equation yields linear `(0.25, 0, 0.75, 1)` and stores logical
   RGBA `[137, 0, 225, 255]` (raw BGRA `[225, 0, 137, 255]`); that result must be
   rejected.
2. **Tint alpha exactly once:** opaque red with straight WHITE tint alpha `0.5`
   contributes premultiplied red `(0.5, 0, 0, 0.5)` before source-over.
3. **Texture and tint alpha:** texture alpha `0.5` and tint alpha `0.5` produce
   source RGB and alpha factors of `0.25`, not `0.125` or `0.0625`.
4. **High-level normalization:** straight input bytes `[255, 0, 0, 128]`
   registered through `register_image_rgba8_srgb()` are uploaded as
   `Rgba8UnormSrgb` bytes `[188, 0, 0, 128]`. Sampling yields linear
   `(0.5028864580325687, 0, 0, 0.5019607843137255)`. Over opaque blue, the
   production `Bgra8UnormSrgb` target stores logical RGBA
   `[188, 0, 187, 255]` (raw BGRA `[187, 0, 188, 255]`). The equivalent
   low-level premultiplied `Rgba8UnormSrgb` texel must produce the same bytes.
5. **Low-level custom front:** a premultiplied front texture registered through
   view registration produces the same composition result.
6. **Opaque preservation:** an opaque source and opaque WHITE tint retain the
   existing output bytes.
7. **Transparent-edge filtering:** filtering between opaque color and fully
   transparent zero-RGB content does not introduce a dark or colored halo.

The evidence runs without a window or OS surface and does not claim device
recovery. It is retained in `src/tests/image_alpha.rs` through a `cfg(test)`
renderer probe that uses the production Image shader, pipeline, bind-group
layout, sampler, blend state, and `Bgra8UnormSrgb` target. The five focused
cases prove the exact fractional-alpha, tint-alpha, combined-alpha,
high-level/direct/deferred/nearest/custom-front parity, opaque-preservation, and
transparent-edge oracles above.

---

## 8. Consistency Matrix

| Law | Result |
|---|---|
| Scene-boundary color | Preserved as straight linear RGBA. |
| SPEC-HAL-085A fragment-output law | Preserved as premultiplied linear RGBA. |
| SPEC-HAL-150 composition | Preserved through the existing premultiplied source-over blend state. |
| SPEC-HAL-263 texture identity/lifetime | Unchanged. |
| SPEC-HAL-263 clip model | Preserved; coverage now has one exact application point. |
| SPEC-HAL-295 custom fronts | Clarified: low-level published views must sample as premultiplied linear. |
| Opaque live consumers | Preserved by INV-263A-007 and required evidence case 6. |

No scene phase, renderer authority, widget authority, batching key, or resource
lifetime is introduced or relaxed.

**Proposed relationship to SPEC-HAL-363/364 (2026-10-05):** The paired
documents remain Proposed pending joint adoption. This annotation changes no
declared status, implemented scope or substantive law in this document.

SPEC-HAL-364 §7 cites this contract's straight-linear tint, premultiplied-linear
sampled content and exactly-once association/coverage rules. An internal
historical lease derived from an already admitted exact generation is not a new
public upload/registration boundary and performs no content normalization.
INV-263A-004's public set stays closed: any profile needing another operative
public boundary requires its exact amendment first. No mask API or asset
loading is admitted by this relationship.

---

## 9. Adoption and Backlinks

Before adoption, backlinks in SPEC-HAL-263 and SPEC-HAL-085A identified this
document as a proposed correction with no semantic effect.

At adoption:

1. apply only the literal replacements in §6 to SPEC-HAL-263 §3.2 and
   SPEC-HAL-085A §6.4;
2. change SPEC-HAL-263's SPEC-HAL-263A proposal metadata to “Amended by” and
   promote the §3.2 anchored proposal note to the normative parent clause quoted
   in §6.1, without mechanically rewriting §§4.2–4.3, 6.1, 7.1, 8, or 12;
3. change SPEC-HAL-085A's SPEC-HAL-263A proposal metadata to “Amended by” and
   promote its §6.4 anchored proposal note to an adopted amendment note;
4. retain this document as the audit trail; and
5. require all evidence in §7 before marking the correction implemented.

---

## 10. Non-Goals

- changing the public CPU `Color` representation;
- changing non-Image shader input contracts;
- adding image decoding, loading, streaming, mipmapping, compression, or HDR;
- changing blend modes;
- changing image resource lifetime or registration ownership;
- adding device-loss recovery;
- defining custom-render admission or target sizing.
