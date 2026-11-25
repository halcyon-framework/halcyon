# **SPEC-HAL-040 — GPU Resource Lifetime Management**

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**Author:** JC
**Date:** 2025-11-24
**Status:** Implemented
**Scope:** Renderer, GPU memory, WGPU internals
**Purpose:** Define *strict, predictable, crash-proof* lifetimes for all GPU resources: textures, buffers, pipelines, atlases, bind groups, and shader modules. Prevent leaks, stalls, and device-lost spirals during long-running trading sessions.
**Amended by:** SPEC-HAL-295A (Adopted / Binding; bounded custom-render exception to Invariant 1)

---

## Implementation Status

**Implemented** (Stages 1-7 complete)

- Stage 1: Frame Epoch Model (`FrameEpoch` newtype, advances once per successful GPU frame)
- Stage 2: Unified GpuResourceArena (central registry for GPU resource metadata)
- Stage 3: Deferred Destruction (metadata layer with `schedule_destroy()` and `collect_garbage()`)
- Stage 4: Staging Buffer Pool (reusable pool for CPU→GPU uploads)
- Stage 5: Atlas Lifetime Rules (glyph atlas texture registration and growth tracking)
- Stage 6: Swapchain & Resize Lifetime Rules (centralized `reconfigure_surface()`, SurfaceError handling)
- Stage 7: Pipeline & BindGroup Revalidation Rules (`validate_pipelines()` for self-healing on format/atlas changes)
- Production path vertex, index, and instance transfers are renderer-owned staging-pool uploads; no path compile path may create independent per-frame transfer buffers through `Queue::write_buffer`.
- Reusable staging backing buffers are power-of-two size-classed and selected by best fit. After GPU completion, an identical repeated upload-size multiset must reuse the warm pool without increasing allocated staging capacity.
- Production frame completion calls `advance_frame()` after submission and presentation to poll submitted staging work and remaps. An unchanged repeated compile/advance workload must stabilize without relying on a test-only blocking device wait.
- Surface acquisition backpressure occurs inside the single UI-thread frame after compile. While `get_current_texture()` is blocked, the application must not begin another compile or enqueue another staging submission.

---

# **1. Problem Statement**

A trading UI runs **10–16 hours per day**, often with:

* Live charts
* Rapid redraws (120–240 FPS)
* Continuous glyph uploads
* Dynamic buffers updated per-frame
* Window resizes
* Device-lost events (Windows laptops, NVIDIA drivers)
* WGPU surface reconfigurations

Without a **formal lifetime model**, Halcyon risks:

* memory leaks,
* zombie resources that never deallocate,
* shader/pipeline invalidation on resize,
* atlas fragmentation over time,
* device-lost → permanent soft-lock,
* corrupted scene state after swapchain recreation.

Trading UI cannot tolerate that.

We need a deterministic contract for **creation**, **retention**, **invalidation**, and **recreation** of every GPU resource.

---

# **2. Design Overview**

Halcyon’s GPU system adopts **four lifecycle tiers**:

```
Tier 0: Permanent (device lifetime)
Tier 1: Surface-bound (swapchain dependent)
Tier 2: Frame-bound (transient)
Tier 3: Ephemeral (per-draw call, auto-discard)
```

Everything in the renderer **must** belong to exactly one tier.

This SPEC defines:

* which resource goes where,
* when it is created,
* when it is destroyed,
* when it must be recreated,
* what happens on device-lost,
* how to guarantee safety.

**363/364 scoped amendment:** The four ordinary tiers remain unchanged. Section 3 additionally
names the explicit SPEC-HAL-364 retained-content lifetime; “exactly one tier”
does not authorize an unlabelled extension of ordinary Tier 2.

---

# **3. Lifetime Tiers**

## **Tier 0 — Permanent (Device Lifetime)**

Created once when WGPU device is created. Destroyed only when device is dropped.

### Examples

* Shader modules
* Pipeline layouts
* Bind group layouts
* Static geometry buffers (quad mesh)
* Static lookup tables
* Glyph atlas metadata (not texture!)

### Rules

1. Never recreated on resize.
2. Never tied to a specific surface.
3. Safe across device-lost (recreated only when device is recreated).
4. These objects must not reference swapchain textures.

---

## **Tier 1 — Surface-Bound (Swapchain Dependent)**

Destroyed and recreated whenever:

* window is resized,
* surface is lost,
* present mode changes.

### Examples

* Swapchain texture views
* Depth/stencil buffer
* Multisampled render targets (MSAA)
* Framebuffer-sized uniform buffers
* Glyph atlas texture (optional, depending on implementation)

### Rules

1. Must be wrapped in `RendererSurfaces` struct.
2. Whenever `wgpu::Surface::configure()` is called → destroy and recreate all Tier 1.
3. No persistent GPU reference outside `Renderer`.
4. Must support "temporary null" state when the surface is unavailable.

---

## **Tier 2 — Frame-Bound (Transient Resources)**

Allocated per-frame; discarded at end of frame.

### Examples

* Staging buffers (for glyph uploads)
* CPU→GPU transfer buffers
* Per-frame uniform buffers
* Intermediate batch buffers before merge

### Rules

1. Recreated every frame.
2. Must not leak outside render loop.
3. Allocated from a per-frame allocator (arena or `Vec::clear()`-based pool).
4. Never stored in any widget or scene object.
5. A transient lease ends only after its submitted GPU work completes. Physical backing may then return to a renderer-owned pool, but repeated identical frame workloads must stabilize after warm-up rather than create new backing allocations.

---

## **Tier 3 — Ephemeral (Per-Draw Call)**

### Examples

* Temporary bind groups created inside a pass
* Push constants (if supported)
* Compute dispatch scratch buffers

### Rules

1. Exist only during a single `RenderPass`.
2. Not stored anywhere.
3. Automatically discarded after encoder submission.

## Retained Presentation Generation — explicit paired lifetime

**363/364 scoped amendment:** Retained Presentation Generation is the cross-frame content lifetime
admitted by SPEC-HAL-364. It is assigned explicitly to immutable offscreen/content
resources or pinned producer generations needed by admitted historical
occurrences. Physical backing and lease accounting are owned by the existing
renderer/resource producer, never by RenderGraph or an ordinary widget.
It is neither an unlabelled Tier 2 extension nor a new allocator/service.

Every lease SHALL name exact content generation, device compatibility,
target/surface dependence where applicable, extent/sampling/alpha interpretation,
future-use holders and submitted-work safety obligation. Content SHALL remain
immutable and unavailable for writable reuse until all readers and submitted
uses retire.

Logical retirement releases future-use rights. Physical backing returns to
its owner/pool only after safety is established. All retained and retiring bytes
remain charged under SPEC-HAL-364's finite bounds. Pooling cannot excuse
overwrite while a historical reader remains possible.

Surface-bound dependencies invalidated by resize/reconfiguration prevent further
publication/use under obsolete admission; physical destruction waits for safe
release. Target-independent immutable content may be re-admitted only when exact
metrics/coverage/device dependencies permit it. Device loss invalidates
device-bound leases separately from same-device pipeline revalidation.

This is an explicit lifetime assignment for SPEC-HAL-084. Ordinary Tier 0–3
rules remain unchanged. Sections 4–6 retain their owner boundaries; this pair
makes no recovery implementation or cross-device validity claim and grants no
persistent glyph-atlas/UV lease for historical text.

---

# **4. Device-Lost Handling Contract**

A device-lost event is not hypothetical. NVIDIA + Windows + WGPU = common.

We must guarantee:

```
device-lost → full recovery → no crash → no memory leak
```

### Requirements

* Device lost resets everything from Tier 0 to Tier 3.
* Renderer must reinitialize fully.
* App remains alive; only frame is lost.

### Steps

1. Release all GPU resources.
2. Recreate device + queue.
3. Recreate all Tier 0 items (shaders, layouts).
4. Recreate all Tier 1 items (swapchain-bound textures).
5. Resume normal rendering.

UI state must be **fully decoupled** from GPU state, so app widgets survive device resets.

---

# **5. Resize Handling Contract**

A resize triggers:

```
surface.configure()
→ destroy Tier 1
→ rebuild Tier 1
→ preserve Tier 0
```

### Correct sequence

1. Stop rendering.
2. Drop swapchain-dependent resources.
3. Recreate surface + textures + views.
4. Restart rendering.

This prevents:

* validation errors,
* stale texture views,
* mismatched formats,
* GPU memory fragmentation.

---

# **6. Atlas & Cache Lifetimes**

### Glyph Atlas (texture)

Tier 1 — surface-dependent.

* On resize: atlas texture recreated
* Cache metadata (glyph → UV) persists
* Old GPU texture is discarded

### Glyph Pipeline Objects

Tier 0 — device-dependent.

* Only recreated on device reset.

---

# **7. Renderer Data Structures**

Renderer must maintain a strict struct layout:

```
Renderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    tier0: RendererDeviceGlobals,
    tier1: RendererSurfaces,
    frame: RendererFrameState,
}
```

### `RendererDeviceGlobals` (Tier 0)

```
shaders
pipeline_layouts
pipelines
bind_group_layouts
quad_mesh
```

### `RendererSurfaces` (Tier 1)

```
swapchain_format
framebuffer_view
depth_view
msaa_target
glyph_atlas_texture
```

### `RendererFrameState` (Tier 2)

```
staging_buffers
per_frame_uniforms
batch_scratch
encoder
```

---

# **8. Required Invariants**

### Invariant 1

> **Adopted amendment:** SPEC-HAL-295A supplies the binding replacement text for
> Invariant 1 defining the sole bounded exception for an explicitly admitted
> custom-render participant.

No GPU resource is ever stored in a `SceneNode` or ordinary Widget state. The
sole bounded exception is a SPEC-HAL-295 custom-render participant, which may be
widget-associated and may retain participant-owned offscreen resources and
rebuildable GPU caches under SPEC-HAL-295A. That exception never grants
semantic authority or renderer-private resource access.

### Invariant 2

Renderer never leaks a GPU object across frames unless documented.

### Invariant 3

Window resize must never crash or freeze renderer.

### Invariant 4

Device-lost must not cascade into undefined state.

### Invariant 5

Timed expiration (for old batches / glyphs) must be explicit and safe.

**363/364 scoped amendment:** Invariant 1 still forbids GPU resources in SceneNodes and ordinary
Widget state. The SPEC-HAL-295A participant exception is not widened. Invariants
2–5 additionally require Section 3's explicit generation leases and safe,
accounted retirement for the admitted historical lifetime.

---

# **9. Implementation Checklist**

### Device-Dependent (Tier 0)

* [ ] Create `RendererDeviceGlobals`
* [ ] Move all shader creation there
* [ ] Move pipeline layouts & pipelines there
* [ ] Move static geometry buffers there
* [ ] Recreate on device-lost

### Surface-Bound (Tier 1)

* [ ] Add `RendererSurfaces`
* [ ] Recreate on resize or present mode change
* [ ] Provide `recreate_surfaces(new_size, present_mode)` API

### Frame-Bound (Tier 2)

* [ ] Implement `RendererFrameState`
* [ ] Reset pools every frame
* [ ] Ensure no persistent leak

### Ephemeral (Tier 3)

* [ ] Define per-draw scratch behavior
* [ ] Disallow storing these resources

### Device-Lost Handling

* [ ] Implement `try_render_frame()` wrapper
* [ ] Catch device-lost → rebuild renderer

---

# **10. End**

This SPEC makes Halcyon's GPU layer **predictable, leak-free, and resilient**, capable of powering long-running high-frequency trading UIs without stalls, crashes, or silent corruption.

## Scoped Frosted Glass transient backing (HAL-385 section8, 2026-11-13)

The current-backdrop implementation uses the existing WgpuRenderer as the physical owner. Shader/pipeline definitions are lazy device-lifetime objects; current prefix/snapshot/filter textures, uniforms and their immutable bindings have Tier2 uses. Backing may become eligible for reuse in the finite frost pool after an authenticated terminal observation of the exact submitted use. Source pixels expire with the frame. There is no GPU resource in a widget, SceneNode or logical graph descriptor.

HAL-385 section8.8 defines checked per-plan256MiB and total-resident768MiB budgets, including free, prepared, encoded, submitted and unconfirmed entries. Native surface textures are not part of this pool. Resize/device changes invalidate suitability and source identity; new use requires exact current format/extent/usage/device matching. The actual submission owner observes the covered use on its original queue through [HAL-385 §9.3–9.4](SPEC-HAL-385-True-Frosted-Backdrop-Glass.md#93-actual-submission-and-failure-ordering) and [HAL-295A §3.3](SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md#33-publication-boundary). WebGPU fulfillment can accompany device loss. The observation establishes settlement for pool bookkeeping only. Device health, valid contents and successful execution require independent evidence. An automatic wrapper drop or superseded CPU preparation supplies no terminal observation.

A prepared unencoded cancellation may release safely. For encoded work, only the HAL-385 section 9 sealed command owner can prove original-command disposal: it must consume/drop that exact still-unsubmitted command before cancellation. Normal submit transfers the same inseparable use to completion tracking on its authenticated queue. Public raw extraction is consuming and irrevocably marks the use Unconfirmed BEFORE the raw command escapes. A replacement command's completion/cancellation, output reconstruction or later raw-command drop cannot settle the original use. Submission uncertainty and exported backing remain quarantined and charged until safe device retirement. Budget exhaustion refuses explicitly, without unbounded allocation or UI blocking. Repeated completed normal-route workloads stabilize backing allocations after warm-up. Existing independent resource custodians and no-frost behavior are preserved.

## Coordinated S2 optical-region accounting (2026-11-14)

HAL385 section11 expands the sampled/filtered region only for explicitly authored bounded refraction or surface emission. Charge actual S/Q textures and1344-byte uniforms, retain the existing256MiB plan and768MiB resident limits, and key suitable free backing by the real new extents/format/device. Analytic uniform storage uses its revised688-byte record and aligned stride under HAL085A. No new resource owner or lifetime tier is introduced. Original command/use/queue sealing, completion-only reuse, disposal-before-cancellation and permanent raw-export quarantine remain unchanged; optical parameters never grant custody authority.
## 2026-11-18 scoped HAL-395 optical resource accounting adoption [S1 Binding]

The first selected HAL-395 material Bloom is renderer-owned GPU work inside the existing HAL-084 RenderGraph and HAL-040 physical resource and lifetime authority, never in a widget/scene node or external producer. One existing-or-new current-target accumulator A, plus Rgba16Float source S, horizontal H and vertical G, exact uniforms and all current Frosted allocations participate in ONE checked per-SurfaceId reservation under the existing HAL-385 256MiB plan and 768MiB resident ceilings and normal device budgets. The Bloom addition has no second free allowance, unbounded cache, private Device/Queue, independent submission, or stale reuse after target supersession. No-effect/absent Bloom allocates nothing and preserves ordinary rendering. Native/Metal/browser capability checks and asynchronous readiness are owned by the current renderer with HAL-385's narrowly coordinated Pending extension, not by a new service. A selected unsupported resource set returns the exact typed refusal; no radius downscale or missing effect can be represented as success. This S1 adoption is design authority only, not proof of allocation or execution.

