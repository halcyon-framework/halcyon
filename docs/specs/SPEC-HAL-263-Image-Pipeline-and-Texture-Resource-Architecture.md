# **SPEC-HAL-263 — Image Pipeline & Texture Resource Architecture**

**Date:** 2026-02-23
**Status:** Implemented
**Depends on:** SPEC-HAL-002, SPEC-HAL-027, SPEC-HAL-040, SPEC-HAL-045, SPEC-HAL-081A, SPEC-HAL-083, SPEC-HAL-084, SPEC-HAL-150, SPEC-HAL-158
**Replaces:** Inert image scaffolding in SceneGraph / Batcher / Shader layer
**Amended by:** SPEC-HAL-263A (Adopted / Binding; literal correction to §3.2, with §§4.2–4.3, 6.1, 7.1, 8, and 12 read subject to its Image alpha contract)
**Amended by:** SPEC-HAL-263B (Adopted / Binding; literal lifetime and revalidation correction to §§5.2, 7.1, 9.1, and 10)

---

# **1. Objective**

Introduce a fully wired Image pipeline that:

* Integrates with existing SceneGraph → Batcher → RenderGraph flow.
* Respects phase separation (Build → Compile → Execute).
* Respects deterministic batching rules.
* Preserves triple-buffer scene safety.
* Introduces no implicit behavior.

No UI semantics. No loading policy. No caching heuristics beyond explicit design.

---

# **2. Architectural Position**

Images are:

* Main-pass primitives (same pass tier as Rect and Text).
* Not overlay-only.
* Not prepass-only.
* Rendered within the existing main render pass pipeline group.

PipelineId remains:

```
PipelineId::Image = 4
```

No renumbering.

Render ordering (Main pass):

1. Rect batches
2. Image batches
3. Text batches
4. Path draws

Overlay pass follows identical relative ordering.

---

# **3. ImageNode Expansion**

## **3.1 Struct**

ImageNode is expanded to:

```rust
pub enum ImageTextureSource {
    Registered(u64),
    Admitted(ImageBindingIdentity),
    TargetBound(CustomRenderPresentationId),
}

pub struct ImageNode {
    pub rect: RectF,
    pub texture_source: ImageTextureSource,
    pub uv_rect: RectF,
    pub tint: Color,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}
```

## **3.2 Rules**

> **Adopted amendment:** SPEC-HAL-263A supplies the binding Image alpha
> correction. SPEC-HAL-263 §§4.2–4.3, 6.1, 7.1, 8, and 12 are read subject to
> SPEC-HAL-263A. That amendment owns the distinction between the high-level
> straight-sRGB normalizing boundaries and every low-level premultiplied-linear
> boundary, the Image shader equation, the fact that bind-group ownership does
> not change association, straight-linear vertex tint, and the binding Image
> alpha invariants.

> **Adopted extension:** SPEC-HAL-295A and SPEC-HAL-266A extend Image identity
> without changing the ordinary registered-image path. `Registered(texture_id)`
> is the exact legacy texture-ID form created by `SceneBuilder::push_image`; it
> retains the same registration, batching, shader, and composition behavior.
> `TargetBound(presentation_id)` is available only to an explicitly admitted
> per-target custom-render participant. The engine resolves that opaque identity
> for the current `SurfaceId` before ordinary Image batching, and omits the Image
> when no binding exists. Resolution performs no GPU or lifecycle mutation.

* `rect` is logical layout rectangle.
* `texture_source` is the exact legacy registered texture identity, the
  generation-specific CPU identity admitted under §5.3, or the admitted
  per-target extension defined above. `Admitted` contains no GPU reference.
* `uv_rect` is normalized [0..1] UV domain.
* `tint` is a straight, linear RGBA multiplier; the Image shader applies tint
  alpha exactly once.
* No intrinsic size.
* No DPI field.
* No aspect logic.

Size is always externally defined.

---

# **4. Texture Model**

## **4.1 Allocation Strategy**

Images are per-texture GPU allocations.
No atlas.

Reasoning:

* Avoids cross-frame repack hazard.
* Avoids eviction complexity.
* Avoids DPI bucket coupling.
* Maintains 1:1 texture identity.

## **4.2 Format**

Default format:

```
Rgba8UnormSrgb
```

All color images are sRGB-correct.

## **4.3 Ownership Model**

Image textures are renderer-owned via `image_textures: HashMap<u64, ImageTextureEntry>`.

Each entry owns: `wgpu::Texture` + `wgpu::TextureView` + `wgpu::BindGroup`.

* High-level registration accepts decoded straight-sRGB RGBA8 through
  `WgpuRenderer::register_image_rgba8_srgb()` directly or
  `GpuResources::queue_image_rgba8_srgb()` for deferred renderer-owned work.
  Both use the same direct normalization/upload implementation.
* Low-level registration uses `register_image_texture()` or the direct/deferred
  view boundaries in SPEC-HAL-263A, accepting already-premultiplied sampled content.
* Unregistered via `unregister_image()` — immediate removal, no deferred GC.
* No external code stores GPU texture handles.

---

# **5. Texture Lifetime**

## **5.1 Rule**

Texture lifetime is explicit: alive from `register_image_*()` until `unregister_image()`.
No deferred GC. No arena-based epoch tracking for image textures.

## **5.2 Compile-Phase Handling (INV-IMG-005)**

After same-compile pipeline and bind-group revalidation, every executable Image
batch must reference a currently registered texture and bind group.

If Compile encounters an unregistered or never-registered `texture_id`:

* Debug build → panic with `INV-IMG-005` message.
* Release build → remove the batch from the executable per-surface batch list.

No placeholder rendering. Execute is not a fallback validation phase.

## **5.3 Generation-specific CPU admission evidence**

The existing renderer registration owner MAY issue an immutable `ImageBinding`
for an actual current entry. An authored asset reference, registration number,
queued registration, or pending evidence request MUST NOT be treated as that
evidence. The binding SHALL identify the exact registration number and a
non-reused registration generation. It SHALL expose the existing sampling
interpretation and CPU-only availability observation, never a GPU handle,
renderer reference, resource ownership, or resource lease.

`WgpuRenderer::admit_image()` SHALL issue only from the current registration.
`GpuResources::request_image_binding()` SHALL queue a CPU evidence request to
that same owner. The existing renderer drain SHALL resolve such requests after
queued removals and registrations; pending and rejected requests are not
bindings. A producer MUST obtain successful evidence and validate its current
availability before activating a required Image appearance. This does not
change the producer's content/alpha obligations under SPEC-HAL-263A.
The producer retains its existing obligation to schedule the registration drain
and a later legal Update that receives its result. Issuance after scene assembly
MUST NOT activate a skin or splice a replacement into that same assembled scene.
The production frame owner SHALL service requested admission even when the
retained scene is unchanged. Pending request and completed result notification
SHALL reuse its existing wake-only frame ingress; completion enables the next
legal Update, not post-layout activation. This is causal queue servicing, not
resource-availability polling or a new clock/animation owner. No pending-request
storage or GPU work is introduced when this profile is unused.

`GpuResources::queue_image_rgba8_srgb(texture_id, width, height, Vec<u8>)` SHALL
validate nonzero dimensions, checked RGBA8 byte count, exact byte length, and
the existing device's 2D texture dimension limit synchronously. Typed rejection
SHALL queue neither registration nor admission work. A valid call SHALL queue
the owned decoded straight-sRGB pixels together with the response for its
existing `ImageBindingRequest`. Each response SHALL be causally paired to that
call's actual registration. The fixed drain order is removals, existing
low-level view registrations, deferred high-level pixels in FIFO order with
each paired response, then legacy texture-number admission resolution. For each
pixel entry the drain SHALL invoke `WgpuRenderer::register_image_rgba8_srgb()`
as the sole normalization/upload implementation, then `admit_image()` and
deliver that exact fresh generation before registering the next entry. Two
same-number calls A then B within one drain therefore return distinct g1 and g2;
B revokes A's g1 as Replaced rather than redirecting A's response to g2.
Arbitrary `request_image_binding(texture_id)` requests retain their existing
current-entry semantics after registration draining. Pending pixel work and
results reuse the existing admission wake and reused-scene servicing; receipt
still enables only a later legal Update.

Renderer retirement SHALL atomically close the high-level ingress, release all
queued pixel buffers, and cancel their pending requests. Surviving GpuResources
clones SHALL reject subsequent calls synchronously as `RendererDestroyed`
before retaining pixels or creating admission work, and SHALL not schedule a
wake for rejected work. This ingress does not decode, locate, cache, or choose
asset identity. Pending pixel buffer storage and pixel GPU work are created
only for a successfully used live ingress.

Every successful registration, including replacement under the same number,
SHALL receive a fresh generation. Replacement SHALL irrevocably revoke old
evidence as Replaced. Explicit unregistration SHALL revoke it as Unregistered;
renderer destruction and actual device loss SHALL expire it under those exact
causes. Pipeline/bind-group revalidation on the same device SHALL preserve
registration generation and evidence. Revocation is monotonic; re-registration
MUST NOT revive an old value or redirect it to another generation.

An admitted Image scene value and batch SHALL retain exact generation identity.
Batches of different generations MUST NOT merge. Compile SHALL validate both
registration and generation against the existing registry after revalidation.
Missing, revoked, cross-renderer, and replaced evidence follows INV-IMG-005:
Debug panic; Release warning and removal of that Image batch. Execute MUST NOT
look up a replacement or silently reinterpret stale evidence. Ordinary legacy
registered-number and admitted target-bound paths remain unchanged. This
explicitly extends INV-IMG-002's CPU identity vocabulary; its GPU-reference ban
and INV-IMG-003's immediate resource removal remain binding.

CPU dependency observation SHALL be event-driven and pay-for-use. The renderer
may publish resource identity/revocation observations; it MUST NOT know widget
identity or mutate WidgetTree. The existing retained scene owner SHALL map
consumed bindings to affected local fragments and their ancestor composition
under SPEC-HAL-354. Resource-only revocation MUST NOT dirty layout or unrelated
siblings. No global resource polling, registry traversal, or Image preparation
may be added to unused/non-Image consumers. Evidence neither pins GPU content
nor certifies immutable historical content under SPEC-HAL-364. Future Stretch
materials may consume the same binding identity without changing this owner.

Required proof: real registration-only issuance; deferred pending versus
resolved/rejected evidence; replacement/unregistration/renderer and device
teardown; unchanged same-device revalidation; generation-separated batches;
stale-generation Debug/Release behavior; narrow retained invalidation; and
unchanged unused/Solid allocation and resource-preparation behavior. The proof
SHALL also cover request/result wake and successful queue servicing with a
reused scene, without unrelated input or repeated scheduling after completion.

---

# **6. Clip Model**

Images use:

```
Scissor + Rounded Clip UBO
```

Same model as Rect.

## **6.1 Shader Contract**

Group layout:

* Group 0: Globals
* Group 1:
    * binding 0: `image_tex`
    * binding 1: `image_sampler`
    * binding 2: `rounded_clip_ubo`
    * Dynamic offset used per batch

Image shader implements identical rounded coverage logic as `rect.wgsl`.

No clip buffer usage.

---

# **7. Bind Group Model**

## **7.1 Ownership**

`ImageTextureEntry` owns or retains the registered texture view, its explicit
Linear/Nearest sampling policy, and the current derived bind group.

No separate bind-group cache map. `image_textures.len()` equals the number of
registered Image identities and bind groups.

Registration identity remains alive until explicit unregistration or actual
renderer/device destruction. Same-device pipeline revalidation preserves every
registration and atomically rebuilds its bind group against the current layout.

---

# **8. Vertex Generation**

`PrimitiveData::Image` emits:

* 4 vertices
* 6 indices

Vertex attributes:

```wgsl
position: vec2<f32>
uv: vec2<f32>
color: vec4<f32>
```

UV derived from `uv_rect`.
Color derived from `tint`.

---

# **9. RenderGraph Integration**

## **9.1 Compile Phase**

Responsibilities:

* Revalidate pipeline layouts and rebuild derived Image bind groups while
  preserving registered texture/view identity and sampling policy.
* Validate final executable texture existence (INV-IMG-005).
* Prepare rounded clip UBO entries for image batches using the same
  dynamic offset model as Rect pipeline.
* Upload image vertex/index data to GPU buffers.

## **9.2 Execute Phase**

Responsibilities:

* Set pipeline.
* Set scissor.
* Set bind group.
* Issue draw.

No GPU allocation.
No texture upload.

---

# **10. Device Loss and Pipeline Revalidation**

Actual device loss invalidates the complete renderer/device resource set;
recovery may require producer re-registration after renderer reconstruction.

Ordinary same-device pipeline revalidation, including glyph-atlas growth,
target scale-factor change, or surface-format pipeline replacement:

* preserves every registered Image texture/view identity;
* preserves each registration's Linear/Nearest sampling policy; and
* rebuilds every Image bind group against the replacement layout before
  marking the generation validated.

No caller re-registration is required for ordinary pipeline revalidation.

---

# **11. Deterministic Batching**

DrawBatchKey includes the exact admitted generation when present:

```
z_layer
clip_hash
pipeline
texture_id
image_generation (absent for legacy/non-Image paths)
shape_type_id
```

`texture_id` ensures:

* Same texture + same admitted generation (or both legacy) + same clip → batch merge.
* Different texture → batch split.
* Different admitted generation, or legacy versus admitted identity → batch split.
* No pipeline switch required between different textures.

---

# **12. Invariants**

**INV-IMG-001 — Execute-Phase Resource Guard**

No `create_buffer`, `create_texture`, `create_bind_group`, `write_buffer`, or `write_texture` calls may occur during Execute for the Image pipeline.
Execute may only: set pipeline, set bind group, set vertex/index buffer, set scissor, draw.

* **Enforcement point:** Execute (main pass + overlay pass)
* **Debug:** `debug_assert!` / `panic!` on missing entry or out-of-range indices
* **Release:** `continue` (skip batch silently)
* **Relationship to INV-IMG-005:** INV-IMG-005 is the primary compile-phase guard; INV-IMG-001 is the last-ditch execute-phase assertion that fires only if INV-IMG-005 was somehow bypassed.

**INV-IMG-002 — Identity by Handle**

SceneGraph stores only CPU image identities: legacy `u64` registration numbers,
generation-specific `ImageBindingIdentity` (§5.3), or the admitted target-bound
identity (§3.2). It MUST NOT store GPU references. CPU admission/dependency
records outside ImageNode do not own or lease the registered GPU resource.

* **Enforcement point:** SceneGraph (structural — no GPU types in ImageNode)

**INV-IMG-003 — Explicit Lifetime**

Texture lifetime is explicit: `register_image_*()` → use → `unregister_image()`. No deferred GC; removal is immediate.

* **Enforcement point:** WgpuRenderer (register/unregister API)

**INV-IMG-004 — Deterministic Merge**

Two Image primitives merge iff their `DrawBatchKey` values are identical.

* **Enforcement point:** Batcher

**INV-IMG-005 — Compile-Phase Texture Validation**

If an Image batch references an unregistered or never-registered `texture_id`, the compile phase rejects it before any execute-phase work.

* **Enforcement point:** Compile
* **Debug:** `panic!` with diagnostic message
* **Release:** `tracing::warn!` + skip batch (no draw)

**INV-IMG-006 — No Implicit Scaling**

ImageNode never derives intrinsic size. No DPI scaling occurs in renderer.

* **Enforcement point:** SceneGraph / Renderer (structural — no size derivation code exists)

**INV-IMG-007 — No Atlas**

Image textures are not repacked or moved after creation. One texture = one GPU allocation.

* **Enforcement point:** WgpuRenderer (structural — no atlas code exists)

**INV-IMG-008 — Dedicated Geometry Streams**

Image primitives use a dedicated vertex/index buffer stream. Image batches never index into Rect geometry buffers. Per-pipeline vertex/index offsets are maintained independently.

* **Enforcement point:** Compile / Execute (separate `image_vertex_buffer` / `image_index_buffer`)

---

# **13. Files Impacted**

**Modified:**

* `scene/mod.rs`
* `batcher.rs`
* `wgpu_renderer.rs`
* `render_graph/passes/main_pass.rs`
* `gpu_arena.rs`
* `shaders/image.wgsl`

**Added:**

* None

**Removed:**

* Dead `clip.wgsl` if unused.

---

# **14. Non-Goals**

* Image loading API
* Async decode
* Texture streaming
* Mipmaps
* Compression
* Video
* Animated textures
* Image widget semantics

---

# **15. Result**

After full implementation of SPEC-HAL-263:

* ImageNode is fully wired.
* `PipelineId::Image` is live.
* Shaders execute.
* Deterministic batching preserved.
* Triple-buffer safety preserved.
* No atlas complexity introduced.
* No invariant violations introduced.

---

This is the final architectural position for images in Halcyon.
