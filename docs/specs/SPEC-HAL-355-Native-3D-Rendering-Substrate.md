# SPEC-HAL-355: Native 3D Rendering Substrate

## Pre-Flight

- SPEC: SPEC-HAL-001, SPEC-HAL-002, SPEC-HAL-021, SPEC-HAL-024, SPEC-HAL-040, SPEC-HAL-084, SPEC-HAL-085A, SPEC-HAL-150, SPEC-HAL-263, SPEC-HAL-263A, SPEC-HAL-266, SPEC-HAL-266A, SPEC-HAL-295, SPEC-HAL-295A, SPEC-HAL-343, and SPEC-HAL-354 read before writing/amending this task: YES
- Policy: N/A — rendering substrate, resource lifecycle, immutable render descriptions, and mechanical ray construction
- Interaction added: NO — existing routing, focus, capture, gestures, and widget policy remain authoritative; this SPEC adds no event handler or semantic action
- Forbidden behaviors checked: FORBID-355-2D-CONTAMINATION, FORBID-355-HIDDEN-CONTEXT, FORBID-355-ADMISSION-AUTH, FORBID-355-RAW-GPU-ESCAPE, FORBID-355-MIXED-PRECISION, FORBID-355-DEPTH-DRIFT, FORBID-355-UNBOUNDED-GEOMETRY, FORBID-355-ARBITRARY-SHADER, FORBID-355-FRACTIONAL-TRANSPARENCY, FORBID-355-PARTIAL-MODEL-STATE, FORBID-355-DYNAMIC-CHURN, FORBID-355-FALSE-PUBLICATION, FORBID-355-TARGET-MISMATCH, FORBID-355-SEMANTIC-GPU-AUTHORITY, FORBID-355-PRIVATE-SCHEDULER, FORBID-355-UNLIMITED-BUDGET, FORBID-355-PORTABILITY-OVERCLAIM, FORBID-355-RECOVERY-OVERCLAIM
- Existing infrastructure: retained WidgetTree layout/paint/input ownership; `CustomRenderAdmission`; `CustomRenderParticipant`; SPEC-HAL-266A post-layout preparation; `GpuResources`; `CustomRenderPass` front/back publication; Image registration and premultiplied-linear composition; shared `Arc<Device>` / `Arc<Queue>`; Plot/Showcase explicit shared-resource construction; Halcyon animation/update scheduling; WebGPU-core `Features::empty()` / `Limits::default()` device profile
- Reinvention risk: Extends the admitted offscreen producer and ordinary Image composition path; it does not create another WidgetTree, compositor, surface manager, input router, frame clock, scheduler, generic N-dimensional renderer, global GPU service, or second device/queue infrastructure

**Status:** Adopted / Binding
**Date:** 2026-08-21
**Binding Amendment:** 2026-08-23 — structural render identity, complete immutable model state, and bounded reusable dynamic storage
**Layer:** Public custom-render substrate / retained viewport integration / device-bound GPU resources
**Owner:** 3D Substrate Architecture
**Runtime scope:** Canonical `App::run` ownership topology only
**Depends on:** SPEC-HAL-001, SPEC-HAL-002, SPEC-HAL-021, SPEC-HAL-024, SPEC-HAL-040, SPEC-HAL-084, SPEC-HAL-085A, SPEC-HAL-150, SPEC-HAL-263, SPEC-HAL-263A, SPEC-HAL-266, SPEC-HAL-266A, SPEC-HAL-295, SPEC-HAL-295A, SPEC-HAL-343, SPEC-HAL-354
**Extends without amending:** SPEC-HAL-295A admitted offscreen production, SPEC-HAL-263A Image alpha, and SPEC-HAL-266A per-target preparation
**Extended without amendment by:** SPEC-HAL-356 optional application/widget 3D camera navigation policy
**Binding extension:** SPEC-HAL-357 adds optional checked one-level local-domain/content composition that lowers to this SPEC's exact flat snapshot/model boundary
**Binding opt-in visual extension (S1, 2026-10-31):** [HAL-370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md) §8 coordinates the exact R-023–R-028 feature contracts; §17.1 below limits their precedence to the separately selected new family. S2 engineering is NOT AUTHORIZED.
**Implementation status:** Not implemented by this document

This document is adopted and binding. It establishes the baseline public 3D
substrate contract while extending the named parent boundaries without amending
their semantics. Adoption does not mark the substrate implemented, mark any
backend conforming, or authorize an implementation to weaken existing binding
authority.

SPEC-HAL-357 is adopted and binding. Its optional producer-side composition
boundary must finish checked admission and deterministic lowering before this
SPEC's renderer preparation. The flat `RenderSnapshot` / complete `ModelState`
pair remains the only binding renderer input, and existing flat consumers remain
valid without a synthetic domain.

---

## 1. Purpose and Authority Decision

Halcyon requires first-class, reusable, domain-neutral 3D content embedded inside
its mature retained 2D UI. The required architecture is not a generalized scene
graph and not a second application renderer.

The authority decision is:

```text
ordinary retained Widget
    -> final logical viewport rect and ordinary Halcyon input routing
    -> SPEC-HAL-266A per-target preparation
    -> specialized participant-owned 3D offscreen render
    -> SPEC-HAL-295A submit / swap / registered-front publication
    -> ordinary ImageNode
    -> existing 2D SceneGraph / Batcher / RenderGraph / compositor / presentation
```

The 3D path owns 3D-only math, resources, validation, depth, pipelines, draw
packets, and per-view targets. It shares the baseline Device, Queue, format,
publication mechanism, retained lifecycle, target metrics, and final Image
composition because those owners are genuinely common.

This is two specialized rendering paths over one application/GPU/presentation
infrastructure. It is not duplicated renderer infrastructure.

**INV-355-AUTH-001 — Specialized Offscreen Boundary:** All 3D geometry is
rasterized into participant-owned offscreen targets. Halcyon's renderer-visible
result is only the registered single-sample front texture referenced by an
ordinary `ImageNode`.

**INV-355-AUTH-002 — 2D Sovereignty:** No 3D type, branch, traversal, resource,
depth state, pipeline state, or submission may enter the existing 2D
`SceneGraph`, `SceneNode`, `Batcher`, `DrawPlan`, `SceneCache`, 2D spatial model,
or RenderGraph hot path.

**INV-355-AUTH-003 — Domain Neutrality:** The substrate owns mechanical 3D
rendering only. It contains no consumer, chart, CAD, map, scientific,
visualization, game, document, or asset-format semantics.

---

## 2. Scope and Authority Boundaries

SPEC-HAL-355 owns:

1. the public role-based 3D math and precision contract;
2. explicit application ownership of `Context3D`;
3. device/context-bound immutable resource handles and generations;
4. immutable structural render snapshots, complete immutable flat model state,
   and draw-packet/model-state validation;
5. the canonical baseline vertex format and fixed vertex stage;
6. the bounded fragment-appearance ABI and admission rules;
7. reverse-Z depth, winding, culling, and sample-count policy;
8. per-view depth/MSAA/color target ownership;
9. requested-versus-committed publication truth;
10. finite resource, dynamic-storage, in-flight, and work budgets;
11. `Ray3` construction and pure mechanical intersection helpers;
12. the native baseline and provisional browser portability claim; and
13. the proof required before implementation can claim conformance.

It imports, without reinterpretation:

- SPEC-HAL-295A for custom-render admission, renderer privacy, offscreen-only
  resource ownership, same-Queue publication ordering, and baseline device
  lifetime;
- SPEC-HAL-263A for premultiplied-linear sampled Image content and final
  source-over composition;
- SPEC-HAL-266A for post-layout/pre-paint timing, per-target physical render
  extent, activity, invalidation, and paint-as-Image-only behavior;
- SPEC-HAL-024 for retained UI, event, logical, physical, and NDC ownership at
  the Widget/renderer boundary;
- SPEC-HAL-150 for final placement, clip, z-order, overlays, and compositor
  obedience; and
- SPEC-HAL-354 for update cadence, retained invalidation, scene reuse, and
  unchanged-frame suppression.
- SPEC-HAL-021 for one engine-owned Timeline, same-frame read-only Update
  sampling, explicit repetition, and the prohibition on renderer-owned time.

This SPEC does not reopen or amend those boundaries. `CustomRenderAdmission`
continues to be an opaque public opt-in marker only. It stores no Device, Queue,
context, epoch, or resource authority and authenticates none of them.

### 2.1 Runtime Scope

The contract applies only to canonical `App::run`, the runtime scope already
owned by SPEC-HAL-295A and SPEC-HAL-266A.

The draft `SceneSwapchain` / split UI-render topology is not covered. Supporting
that topology requires separate adopted authority for device access, request
transport, publication transport, retirement, and failure propagation.

### 2.2 Shader Authority Scope

SPEC-HAL-085A remains the binding contract for renderer-owned 2D shader
pipelines. SPEC-HAL-355 is the new-SPEC extension required by SPEC-HAL-085A §10
for the admitted, participant-owned 3D pipeline. It neither changes the 2D
`Globals` block nor reuses a renderer-private shader or bind group.

---

## 3. Normative Definitions

### 3.1 `Context3D`

`Context3D` is an explicit, public, application-owned, cloneable context for
one isolated family of 3D resources. It captures the exact `Arc<Device>`,
`Arc<Queue>`, surface-compatible format, and per-view `CustomRenderPass`
construction authority supplied by one live `GpuResources` value.

The context does not publish or unpublish Image resources. It may retain the
captured `GpuResources` only so explicit view creation can clone it into that
view's `CustomRenderPass`; the pass remains the sole owner of its texture ID,
front/back swap, registration, unregistration, and publication retirement.

It is not stored in `WgpuRenderer`, discovered from a global, installed in a
renderer registry, or synthesized by an ordinary widget. An application creates
one context and injects cloned `Arc<Context3D>` handles into the viewport
widgets that should share resources.

Multiple contexts over the same Device/Queue are legal but isolated. They do not
share resource handles or caches. Sharing means passing the same context handle.

### 3.2 3D Viewport Participant

A 3D viewport participant is an ordinary retained Widget that is also explicitly
admitted through SPEC-HAL-295A and registered for SPEC-HAL-266A preparation. It
owns only viewport-local presentation state and rebuildable GPU state. Its
application-visible geometry is the ordinary final logical rect owned by
WidgetTree layout.

### 3.3 World, Local, Render-Relative, View, Clip, and Framebuffer Spaces

These terms exist only inside the specialized offscreen producer:

- **world space:** f64 translation domain used by public view/model placement;
- **local space:** f32 geometry domain stored by a mesh;
- **render-relative space:** f32 values after subtracting an internal f64 render
  origin;
- **view space:** right-handed camera-relative space with forward along `-Z`;
- **clip/NDC space:** WebGPU clip convention with NDC `x,y` in `[-1,1]` and `z`
  in `[0,1]`; and
- **framebuffer space:** the offscreen WebGPU render target whose origin is at
  top-left and whose positive Y axis points down.

They do not add Widget layout, event, SceneGraph, or compositor coordinate
spaces. SPEC-HAL-024 remains the sole owner of those UI-boundary spaces.

### 3.4 Immutable View Descriptor

An immutable view descriptor contains a checked rigid pose and exactly one
checked projection description. It is pure presentation input. It is not a
mutable camera object, controller, input policy, animation owner, or durable
application authority.

The pose translation is the camera origin in world space. Its orientation maps
camera-local axes into world axes; the derived view transform is that rigid
pose's inverse. Callers do not supply the inverse or a raw view matrix.

### 3.5 Immutable Structural Render Snapshot and Model State

An immutable structural `RenderSnapshot` is a flat, versioned set of validated
draw packets and resource leases. It owns packet count/order/topology, geometry
and appearance generations, raster classification, stable ordinals, and the
leases that keep every referenced generation alive. It has a monotonic
non-wrapping structural revision and stable immutable identity.

A separate immutable model-state value contains exactly one final flat TRS model
placement for every packet slot in one structural snapshot, in packet order. It
has its own monotonic non-wrapping model revision and is valid only for the exact
structural identity and packet count against which it was admitted.

Structural and model revision monotonicity applies within their source lineage.
The explicit independent whole-PBR-source replacement in §11.2 selects another
already admitted immutable source domain; it does not rewrite these revisions or
turn an old value into a successor in its original domain.

`ModelPlacement3` is the reusable checked TRS render mechanism. It is not a
semantic transform target: packet order does not name a figure, a root, a group,
selection, or a scene. An application that supports transform interaction must
resolve an explicit target under its own policy, replace every affected placement
in a complete successor `ModelState`, and admit that successor normally. It may
choose a whole-model root, a group, or one placement only from application-owned
identity; the substrate never creates a `rotate_scene` primitive or infers a
target from visual equivalence, a skybox-like background, geometry, or raster
output.

`Orientation3::compose_local` is a pure checked math operation, not a transform
target or publication path. For an existing local-to-world orientation `q` and a
checked incremental local rotation `r`, it returns the normalized Hamilton
product `q * r`. It changes neither translation nor scale and never identifies,
selects, or mutates a `ModelPlacement3` or `ModelState`.

Both values are pure render intent. Neither is a retained 3D scene graph, ECS,
document, asset database, hierarchy, partial patch stream, animation owner, or
semantic model. A producer that owns a hierarchy or simulation flattens it into
complete placements before publication.

Binding SPEC-HAL-357 authorizes one optional producer boundary: a checked
Halcyon-owned one-level domain/content value mechanically lowers to this same
exact flat snapshot/model pair while retaining committed domain association
outside the renderer. Domain-aware producers must use that admitted boundary;
flat producers continue to satisfy this contract directly. The extension adds
no domain traversal or hierarchy to renderer preparation.

### 3.6 Resource Handle and Lease

A resource handle is a typed value containing at least:

```text
ContextId
resource kind
slot
non-wrapping generation
```

A lease is shared ownership that prevents retirement while a live structural
snapshot, pipeline, view, or committed publication still references that
generation.

### 3.7 Requested Tuple and Committed Tuple

The requested tuple is the latest complete render request:

```text
request serial
target compatibility key
view revision
immutable view descriptor
structural revision and immutable RenderSnapshot identity
model revision and complete immutable model-state identity/value
sample count
```

The committed tuple is the exact tuple whose writes were submitted, whose back
became front, and whose front is the truthful published image. Requested and
committed tuples are independently observable.

### 3.8 Target Compatibility Key

The target compatibility key contains the participant identity, `SurfaceId`,
physical render extent, color format, sample count, and baseline device/context
identity required for a front to remain valid. A semantically old front may
remain truthful; a front with an incompatible target key may not be sampled.

### 3.9 Appearance Module

An appearance module is bounded application-supplied WGSL source admitted under
the fixed fragment ABI in §9. Its source form is a complete, standalone,
entry-point-free WGSL library module. It contributes one exact ordinary
appearance function plus bounded helper declarations, not a vertex, fragment,
or compute entry point and not a raw pipeline.

---

## 4. Ownership Matrix

| Concern | Sole owner | 3D substrate may | 3D substrate must not |
|---|---|---|---|
| Widget identity and lifetime | WidgetTree / SPEC-HAL-002 | retain context/resource IDs in admitted widget state | own another widget or bypass mount lifetime |
| Layout rect | WidgetTree layout | consume final logical rect through SPEC-HAL-266A | compute absolute UI geometry or physical size in layout |
| Target metrics and activity | Renderer / SPEC-HAL-266A | consume immutable supplied values | query Window/OS DPR or reinterpret conversion |
| Input target, focus, capture | EventRouter and interaction domains | receive already-routed local input | GPU-hit-test the WidgetTree or create another focus/capture authority |
| Camera/controller semantics | Application/widget policy | consume an immutable view descriptor | own orbit/fly/pan/zoom policy or durable camera state |
| Domain/model semantics | Application/session | consume an immutable structural RenderSnapshot plus complete immutable flat model state | infer hierarchy, selection, document meaning, or domain transactions |
| Transform target identity, root/group membership, and transform binding policy | Application consumer | provide checked `ModelPlacement3` and complete immutable model-state admission | infer a target from packet order, geometry, appearance, a visible scene, or GPU output |
| 3D resource identity | `Context3D` | create typed immutable generations | accept raw cross-context GPU objects |
| 3D rasterization | `Context3D` and per-view target | encode participant-owned offscreen work | access a surface, compositor, RenderGraph attachment, or 2D private pipeline |
| Front publication | SPEC-HAL-295A lifecycle | submit, swap, register a truthful front | publish before writes are enqueued or reuse a front as back |
| Image alpha | SPEC-HAL-263A | produce premultiplied-linear sampled content | ask Image composition to repair straight-alpha content |
| Placement, clip, z-order, overlays | 2D SceneGraph / compositor | emit one ordinary full-UV Image and ordinary 2D chrome | create per-geometry cross-boundary depth ordering |
| Frame/update cadence | Halcyon frame loop / SPEC-HAL-021 / SPEC-HAL-354 | render when immutable owner revisions or target state require it | own a Timeline, clock, frame loop, worker scheduler, or wakeup policy |
| Device recovery | Future SPEC-HAL-040 amendment | fail within the baseline device lifetime | claim epoch/recreation support that does not exist |

---

## 5. Retained Integration and Structural Absence

### 5.1 Ordinary Retained Participation

A conforming viewport:

1. is laid out as an ordinary Widget in logical coordinates;
2. is clipped, hidden, removed, focused, captured, and routed by existing
   retained machinery;
3. receives current target metrics only at SPEC-HAL-266A preparation;
4. emits its stable `texture_id` as a full-UV, opaque-WHITE-tinted `ImageNode`;
5. may emit labels, controls, guides, or other UI chrome only through ordinary
   2D PaintSink primitives; and
6. gains no direct compositor or presentation authority.

The image is one compositing plane. 2D primitives may be ordered before or after
that plane. They do not depth-test against geometry inside it.

### 5.2 Explicit Context Ownership and Injection

The public construction pattern is:

```text
Screen/controller obtains AppContext::gpu_resources()
    -> constructs Arc<Context3D> once with finite configuration
    -> passes clones into independent viewport widgets
```

This is the same explicit application-ownership pattern demonstrated by
Showcase's shared `PlotRenderPool` and cloned `GpuResources`. There is no hidden
context discovery.

The context mints a private `ContextId` only after capturing the exact Device and
Queue Arcs. Context identity is the identity of that private allocation; it does
not require a global allocator or registry and is not a caller-forgeable numeric
claim. Device/Queue compatibility is checked independently by identity of both
captured Arcs. `CustomRenderAdmission` is not consulted as device authentication.

`Context3D` stores no `Widget`, `Screen`, `WidgetTree`, widget-owner,
callback, or UI-tree borrow. Participant lifecycle drives view-local creation,
preparation, inactivity, and retirement explicitly; the context never discovers
or traverses retained UI state.

### 5.3 Zero-View Structural Absence

When 3D capability is compiled but the application creates no `Context3D`
and admits no 3D viewport, there is:

- no 3D service, registry, traversal, state processing, context allocation,
  worker, job, timer, feature request, GPU resource, depth/MSAA attachment,
  dynamic storage, pipeline, shader module, pass, command buffer, registration,
  or submission;
- no new 3D-specific branch in ordinary 2D scene traversal, batching, compile,
  RenderGraph execution, or presentation beyond the already-adopted generic
  SPEC-HAL-266A custom-participant seam; and
- no change to existing 2D topology, pass count, submission count, or retained
  invalidation semantics.

If an application explicitly constructs a context but has no active nonzero
viewport, the context may retain only its finite CPU configuration, captured
Arc handles, IDs, and empty registries. It creates no target, pipeline, buffer,
texture, dynamic arena, bind group, command encoder, command buffer, or
submission until an active view requires one.

**INV-355-ABSENCE-001:** Capability availability is not runtime admission.
Construction and work are demand-driven from explicit application ownership and
active registered views only.

The existing `has_custom_render_participants` O(1) branch belongs to
SPEC-HAL-266A and remains present with or without this substrate. “No 3D branch”
means SPEC-HAL-355 adds no `Context3D`/3D-specific condition or work to that
generic seam or to any ordinary 2D owner; it does not rewrite history relative to
pre-266A Halcyon.

---

## 6. Math, Precision, Projection, and Rays

### 6.1 Role-Based Precision

The public precision model is fixed by role:

| Role | Scalar |
|---|---|
| World translation for camera/view and model placement | `f64` |
| World-space `Ray3` origin, direction, and pure intersection arithmetic | `f64` |
| Local mesh positions and vectors | `f32` |
| Orientation quaternion | checked `f32` |
| Model scale | checked `f32` |
| Normals, UVs, vertex color conversion, GPU matrices, and shader data | `f32` |

Halcyon owns the public types. No public type exposes glam, nalgebra, a generic
`Real`, `Mat4<T>`, or parallel f32/f64 descriptor families.

A rigid view pose combines f64 world translation with a finite normalized f32
orientation. A model placement combines f64 world translation, finite normalized
f32 orientation, and finite positive f32 scale. Scale may be non-uniform but no
component may be zero or negative. Shear and reflection are outside baseline.
Model orientation maps local axes into world axes. A valid `Ray3` has a finite
f64 origin and a finite normalized f64 direction.

For column-vector semantics, the model linear transform is exactly:

```text
L = R(orientation) * diag(scale.x, scale.y, scale.z)
```

An application-owned local rotation `r` composes an existing placement
orientation `q` as `q' = q * r`. Thus `r` is expressed in the placement's local
axes before `q` maps those axes into world space. The reusable math operation
returns a newly checked normalized orientation; replacement placement/model-state
publication remains application-owned.

All scale components are finite and strictly positive, and the checked
quaternion produces a proper rotation. Therefore `det(L) = scale.x * scale.y *
scale.z > 0`: the transform is invertible and cannot reverse the mesh's declared
winding. Translation and render-origin subtraction do not affect winding.

The normal transform is exactly the inverse transpose:

```text
normal_to_world = transpose(inverse(L))
                = R(orientation) * diag(1/scale.x, 1/scale.y, 1/scale.z)
```

Admission rejects a placement unless the determinant, reciprocals, model matrix,
and normal matrix are finite and representable as f32. Culling therefore keeps
the canonical `FrontFace::Ccw` for every admitted model transform; reflection
requires separate authority rather than a hidden cull flip.

### 6.2 Render-Origin Conversion

Each view owns an internal f64 render origin chosen near its camera. The origin
may remain stable across camera revisions and may rebase only when required for
representable f32 relative values.

For every model translation:

```text
relative64 = model_world_translation - render_origin
relative32 = checked_f32(relative64)
```

Every component must be finite and representable under the context's precision
policy before GPU data is written. Silent truncation, saturation, or conversion
to infinity is forbidden.

Large geometry that cannot remain accurate as one local f32 mesh must be
segmented by its producer. The substrate does not accept absolute f64 vertices
or silently recenter an individual mesh contrary to its placement.

### 6.3 Coordinate Convention

The baseline convention is one non-configurable profile:

- right-handed world and view spaces;
- camera forward along view `-Z`;
- `+Y` is up in world, local, view, and NDC space;
- column-vector transform semantics and column-major GPU matrix storage;
- WebGPU NDC depth range `[0,1]`;
- reverse-Z depth;
- outward-facing mesh triangles use counter-clockwise local winding; and
- a standard projection with no extra Y reflection maps that winding to
  `wgpu::FrontFace::Ccw`.

Reverse-Z changes clip Z only. It does not change X, Y, W, or winding.

### 6.4 Projection Descriptors

The only baseline projection descriptors are:

```text
Perspective {
    vertical_fov_radians: finite f32 in (0, PI),
    near: finite f32 > 0,
    far: finite f32 > near | Infinite,
}

Orthographic {
    vertical_span: finite f32 > 0,
    near: finite f32 >= 0,
    far: finite f32 > near,
}
```

Aspect ratio is derived every preparation from the final positive physical render
extent supplied by SPEC-HAL-266A. It is not stored as durable view authority.

Perspective finite and infinite far planes share reverse-Z. Orthographic far is
always finite. Raw view matrices, raw projection matrices, caller-selected depth
conventions, and caller-selected handedness are rejected because they cannot
prove rigidity, target-aspect agreement, depth semantics, or matching ray
construction.

For a visible view-space point with positive forward distance `d = -z_view`, the
normative depth mapping is:

```text
perspective finite:   z_ndc = near * (far - d) / (d * (far - near))
perspective infinite: z_ndc = near / d
orthographic finite:  z_ndc = (far - d) / (far - near)
```

Thus the near plane maps exactly to `1`, a finite far plane maps exactly to `0`,
and infinite perspective approaches `0` as distance increases.

### 6.5 Reverse-Z Raster Contract

The baseline depth attachment and state are:

```text
format: Depth32Float
clear: 0.0
compare: Greater
write_enabled: true
depth_bias: zero
```

No stencil aspect or stencil operation is allocated or configured.

The current WebGPU viewport transform maps NDC Y through
`0.5 * (-y + 1) * height`, and its framebuffer-area definition classifies the
standard projected outward-CCW order as `FrontFace::Ccw`. Single-sided packets
use back-face culling with `FrontFace::Ccw`; double-sided packets use no culling.

Conformance still requires the actual-target winding oracle in §16. The prose
derivation cannot substitute for testing the complete matrix/shader/target path.

### 6.6 `Ray3` and Pure Picking Helpers

Ray construction uses the last committed view descriptor and its committed
target extent, never an unrendered requested view.

For a point normalized to viewport-local `u,v` in `[0,1]`:

```text
x_ndc = 2*u - 1
y_ndc = 1 - 2*v
```

Perspective ray construction is analytic from vertical FOV and aspect. It must
not unproject an infinite-far homogeneous point whose W is zero. Orthographic
ray origin is the corresponding near-plane point and its direction is committed
camera forward.

The substrate may expose pure, allocation-free intersection helpers for plane,
sphere, axis-aligned box, triangle, and checked ray-to-local transformation.
They accept explicit immutable values and return only mechanical hit data.

The substrate does not retain CPU mesh mirrors, traverse render snapshots for
hits, build a BVH, associate semantic keys, own selection, or perform GPU ID
picking. A mechanical ray or triangle hit is not an explicit transform target and
does not authorize model mutation; target resolution and selection remain a
separate application policy.

---

## 7. Context, Resource, and Generation Model

### 7.1 Device and Queue Binding

One context uses only the Device and Queue captured at construction. It never
accepts an external `wgpu::Buffer`, `Texture`, `TextureView`, `Sampler`,
`ShaderModule`, `BindGroup`, `RenderPipeline`, `CommandEncoder`, or
`CommandBuffer` as a public resource.

Every buffer, texture, sampler, pipeline, target, and command buffer used by the
context is created from its captured Device and submitted through its captured
Queue. A second Device or Queue is non-conformant.

Public resource registration may validate and retain immutable CPU descriptors,
but GPU materialization, writes, encoding, and submission run only from
SPEC-HAL-266A preparation on behalf of an active explicitly admitted viewport.
Image publication/unpublication is then performed only by that viewport's
`CustomRenderPass` under SPEC-HAL-295A; the context is not a second publisher.
Merely constructing a context or registering CPU content grants no independent
GPU phase and creates no background work.

### 7.2 Typed Generational Handles

All public owning handles are typed. Before any wgpu call, the context validates:

1. exact `ContextId` match;
2. exact resource kind;
3. occupied slot;
4. exact live generation; and
5. required lease and usage compatibility.

Slot reuse increments generation. Generation exhaustion fails explicitly and
the slot is never reused. Context mismatch, wrong kind, stale generation, and
freed slot are typed consumer-data errors.

Updating immutable geometry, texture, sampler, or appearance content returns a
new handle/generation. It never mutates a resource visible through an older
structural snapshot or committed front.

### 7.3 Leases and Retirement

Structural snapshots clone leases for every referenced resource generation. A
model-state value contains placements only and clones no resource lease. A
generation may be removed from lookup or cache only after no live handle,
structural snapshot, pending request, pipeline, or committed tuple leases it.

Published front textures follow SPEC-HAL-295A retirement: unregistration is
queued before the final texture-owning lease is released, and the registered
front is never selected as a writable back.

Eviction is policy, not correctness. Eviction may remove only unleased cache
entries and cannot invalidate a committed tuple.

### 7.4 Baseline Device Lifetime

The context is valid only for the captured baseline Device/Queue lifetime.
Halcyon currently has no generic device recreation epoch for custom resources.

Device loss, fatal validation failure, or unrecoverable allocation failure may
make the context terminal for that device. This SPEC promises neither device
recreation nor automatic replay. Any later nonfatal recovery requires one
generic SPEC-HAL-040 amendment defining renderer recreation, device epoch,
GpuResources replacement, stale-publication rejection, and participant rebuild
ordering.

---

## 8. Geometry, Structural Render Snapshots, and Model State

### 8.1 Canonical Vertex Stream

The baseline has one immutable interleaved vertex layout:

| Offset | Semantic | wgpu format | Public source role |
|---:|---|---|---|
| 0 | local position | `Float32x3` | mandatory finite local point |
| 12 | local normal | `Float32x3` | optional finite nonzero vector |
| 24 | UV | `Float32x2` | optional finite vector |
| 32 | vertex color | `Unorm8x4` | optional linear-quantized RGBA bytes |
| | stride | `36` bytes | one vertex buffer |

Vertex-color bytes encode linear numeric channels, not sRGB channels. For every
component byte `b`, vertex fetch exposes exactly the normalized value `b / 255.0`
subject only to the format's normal representational precision. No sRGB transfer
function is applied on upload, vertex fetch, interpolation, or appearance input.
If a higher-level producer begins with linear floats, its public packing contract
must clamp to `[0,1]` and quantize to these bytes explicitly; encoded sRGB bytes
are invalid input. The alpha byte is appearance/mask data only—the controlled
fragment wrapper still owns output alpha.

Missing optional source semantics are materialized deterministically in the GPU
stream:

```text
normal = (0, 0, 1)
uv = (0, 0)
color = (255, 255, 255, 255)
```

The mesh separately records a fixed semantic-presence bitset for `Normal`, `Uv`,
and `Color`. Defaults satisfy the fixed GPU layout; they do not falsely claim
that the producer supplied the semantic.

An appearance may require only a subset of this fixed set. Missing required
semantics fail before encoding. Semantic presence does not change the vertex
layout, vertex stage, bind-group layout, or pipeline key.

No arbitrary attribute mask, custom vertex layout, tangent, skin weight, joint,
instance layout, or extra stream is baseline.

### 8.2 Indices, Topology, and Validation

The only primitive topology is `TriangleList`. Draws may be unindexed or use
immutable `u16` or `u32` indices.

Geometry admission validates with checked arithmetic:

- finite positions, normals, and UVs;
- nonzero supplied normals;
- exact byte lengths and stride;
- index range;
- vertex/index counts under effective budgets and Device limits;
- a nonempty full draw count divisible by three, using index count for indexed
  geometry and vertex count otherwise.

One packet draws its complete geometry exactly once. Base vertex, first index,
and first instance are zero; instance count is one. Subrange draws and instancing
are outside baseline rather than hidden fields on the supposedly flat packet.

The producer guarantees that vertex order for its intended outward faces is CCW.
The substrate cannot infer semantic outwardness from arbitrary triangles, so it
does not pretend to validate that meaning; it applies the declared canonical
winding through fixed culling state and the actual-target oracle.

Canonical packing normalizes each supplied finite nonzero normal. The fallback is
exactly `+Z`. The vertex stage applies `normal_to_world`; its perspective-
interpolated fragment value is not guaranteed to remain unit length. An
appearance that requires a unit normal normalizes that fixed input explicitly.

### 8.3 Flat Immutable Structural Snapshot

A structural `RenderSnapshot` contains a monotonic non-wrapping structural
revision and an immutable ordered array of draw packets. A packet contains only:

```text
geometry handle
appearance handle
single-sided | double-sided
stable ordinal
```

Arrays may share `Arc`-backed immutable storage between revisions. Equal
structural revision means the same immutable structural identity and content;
reusing a structural revision for changed packet count/order, resource
generation, raster class, or ordinal is invalid. A lower structural revision is
stale and cannot move requested or committed structural state backward.

These comparisons remain mandatory for ordinary within-source publication.
`Viewport3D::replace_pbr_scene` (§11.2) may explicitly select a different admitted
complete-source HDR identity with its own snapshot and numerical revision. A
still-live original snapshot may be selected again as part of that independent
source; no artificial structural generation or historical chain is created.

Resource and appearance leases are captured once by the structural snapshot and
remain stable across model/view-only revisions. A topology, geometry,
appearance, raster, or ordering change publishes a new structural snapshot or a
new immutable resource generation. Placement changes do not.

### 8.4 Complete Immutable Model State

One model-state value contains an immutable ordered placement array whose length
equals the structural snapshot packet count. Slot `i` is the complete final TRS
placement for structural packet `i`. Admission records the exact structural
identity and count and rejects a missing, extra, partial, or mismatched slot
before GPU work.

Equal model revision means the same immutable model-state identity and content.
Reusing a model revision for changed placement content is invalid; a lower model
revision cannot move requested or committed model state backward. Model-state
publication is atomic and complete—there is no incremental placement patch,
dirty-object command stream, hierarchy mutation, or in-place semantic update.

Explicit independent source replacement selects the complete model belonging to
that source's exact snapshot. Its numerical revision is not compared to the
previous independent source's model revision. All model/snapshot association,
packet-count, Context and source checks remain mandatory, and ordinary successor
admission retains its original model-revision checks.

The producer may compute the flat array from a retained hierarchy, skeletal
system, physics result, or procedural model, but those owners remain outside the
substrate. The renderer observes only the complete immutable value.

Binding SPEC-HAL-357 mechanical lowering must finish before this boundary and
produce one complete array with the same exact
structural identity/count and revision laws. Domain/content IDs, embeddings,
and committed association remain producer-side metadata; they cannot become
packet-slot meaning or renderer state.

### 8.5 Draw Classification and Ordering

Baseline draw packets are only:

- opaque; or
- alpha-mask, where rejected samples execute `discard` and every written sample
  has alpha `1.0`.

Fractional per-object or per-fragment transparency is rejected. Transparent
viewport clear and MSAA edge coverage remain legal and produce the
premultiplied whole-viewport alpha required by SPEC-HAL-263A.

Opaque/masked packets may be sorted deterministically by complete pipeline key,
appearance, geometry, and stable ordinal. Depth owns visibility. Coplanar depth
ties are undefined content and are not repaired by hidden ordering heuristics.

---

## 9. Fixed GPU and Appearance ABI

### 9.1 Bind Groups and Actual Uniform Sizes

The baseline uses exactly three bind groups:

```wgsl
struct ViewBlock {
    view_projection: mat4x4<f32>,       // 64 bytes
    camera_relative: vec3<f32>,         // 12 bytes
    _padding: f32,                      // 4 bytes
}                                      // total 80 bytes

struct ObjectBlock {
    model_relative: mat4x4<f32>,        // 64 bytes
    normal_to_world: mat3x3<f32>,       // 48 bytes
}                                      // total 112 bytes

struct AppearanceBlock {
    color_factor: vec4<f32>,            // 16 bytes
}                                      // total 16 bytes
```

| Group | Binding | Resource |
|---:|---:|---|
| 0 | 0 | `ViewBlock` uniform, minimum binding size 80 |
| 1 | 0 | `ObjectBlock` uniform, minimum binding size 112 |
| 2 | 0 | `AppearanceBlock` uniform, minimum binding size 16 |
| 2 | 1 | filterable `texture_2d<f32>` |
| 2 | 2 | filtering sampler |

`min_uniform_buffer_offset_alignment` constrains a bound offset and a dynamic
offset. It does not enlarge the WGSL struct or `min_binding_size`.

When a block is suballocated with dynamic offsets:

```text
stride = round_up(actual_block_size, device.min_uniform_buffer_offset_alignment)
```

The actual sizes remain 80, 112, and 16 bytes.

`ViewBlock.camera_relative` is exactly the checked f32 conversion of:

```text
camera_world_translation - render_origin
```

It exists so the controlled fragment wrapper can derive a stable normalized
world-space direction from the fragment toward the camera without exposing f64,
an inverse view matrix, or the private render origin to application WGSL.

`ObjectBlock.model_relative` contains `L` from §6.1 plus the checked
render-relative translation. `ObjectBlock.normal_to_world` is exactly the
inverse-transpose matrix defined in §6.1; it is not an independently supplied
matrix.

No generic parameter vectors exist in the baseline Appearance block. Three
arbitrary `vec4` values had no mechanically justified semantics. A source module
may contain bounded compile-time constants, which participate in its content hash
and generation; adding independently mutable material parameters requires a
later ABI amendment rather than unnamed reserved storage.

An appearance without a texture binds a context-owned 1×1 opaque white texture
and the fixed default sampler. Optional bindings, binding arrays, and a second
layout for untextured content are forbidden.

Any non-fallback appearance texture is a typed handle owned by the same context
and must provide a filterable 2D float-sample view under the baseline Device
profile. Its declared view format owns storage decoding; the appearance function
must return linear RGB regardless of that storage representation. The fallback
must sample exactly as linear `(1, 1, 1, 1)`.

### 9.1.1 Persistent Bindings and Bounded Dynamic Storage

Structural preparation materializes and retains the geometry generations,
appearance/texture/sampler generations, appearance uniform data, appearance bind
groups, validated draw classification, and pipelines required by one structural
snapshot and target pipeline class. A model- or view-only revision must not
re-admit leases or recreate any of those values.

Every active target owns or leases finite queue-safe dynamic storage for:

```text
one ViewBlock per in-flight storage slot
one ObjectBlock per admitted structural packet slot and in-flight storage slot
```

The associated GPU buffers and bind groups are persistent objects. After the
target and structural packet capacity have warmed up, a model/view-only frame
writes only the changed view/object byte ranges into already-admitted storage.
It creates no GPU buffer, bind group, pipeline, shader module, appearance
binding, resource materialization, or packet-topology object.

Appearance data is structural in the baseline ABI. The `AppearanceBlock`,
texture, sampler, and their bind group are reused until the structural snapshot,
appearance generation, or target pipeline class changes. An animation cannot
turn those bindings into unnamed per-frame material parameters.

Storage reuse must be correct under same-Queue ordering and in-flight work. The
implementation may use a ring, multiple arenas, or an equivalently safe bounded
allocator, but it must prove that a write cannot corrupt an earlier submitted
front. The number of live/in-flight slots, total dynamic bytes, object capacity,
and bytes written per preparation/frame are finite policy values.

Capacity growth is checked and transactional. All new CPU staging and GPU
storage, bindings, offsets, and accounting reservations succeed before the
candidate replaces compatible storage. Failure leaves the prior compatible
committed tuple/front/storage truthful. Old storage remains retained only until
its in-flight use is safe to release; growth and repeated revisions cannot form
an unbounded history.

CPU packing follows the same law. Scratch bytes, offsets, packet bindings, and
validated draw metadata are reused or retained at a finite high-water mark after
warm-up. Publishing immutable model/view revisions does not authorize rebuilding
per-packet structural vectors every frame.

Mutable dynamic storage is target-isolated. Context-compatible immutable
structural resources may be shared across viewports and surfaces, but one
surface's dynamic writes, capacity, failure, or lag cannot become another
surface's current model/view state.

### 9.2 Canonical Vertex Stage

Halcyon owns the only vertex entry point. It consumes the canonical stream,
`ViewBlock`, and `ObjectBlock`; writes clip position; and supplies the fixed
location-based fragment input superset:

```text
@location(0) render-origin-relative position: vec3<f32>
@location(1) world normal: vec3<f32>
@location(2) UV: vec2<f32>
@location(3) vertex color: vec4<f32>
```

Application source cannot replace or wrap the vertex entry point.
All four user locations use WGSL's default perspective-center interpolation.
`@builtin(front_facing)` is supplied by rasterization directly to the controlled
fragment entry point; it is not a vertex output.

### 9.3 Halcyon-Owned Fragment Wrapper

Halcyon owns the only fragment entry point and all resource/interface
declarations. An admitted appearance library contributes helper declarations and
exactly one function with this complete standalone signature:

```wgsl
fn halcyon_appearance(
    position_render_relative: vec3<f32>,
    normal_world_interpolated: vec3<f32>,
    view_direction_world: vec3<f32>,
    uv: vec2<f32>,
    vertex_color_linear: vec4<f32>,
    front_facing: bool,
    color_factor_linear: vec4<f32>,
    appearance_texture: texture_2d<f32>,
    appearance_sampler: sampler,
) -> vec3<f32>
```

`view_direction_world` is the wrapper-normalized direction from
`position_render_relative` to `ViewBlock.camera_relative`.
`normal_world_interpolated` is intentionally not unit-guaranteed after
interpolation. The return value is linear RGB.

The wrapper maps any non-finite returned component to zero, clamps remaining RGB
to `[0,1]`, and returns `vec4(safe_rgb, 1.0)`. The appearance function may execute
`discard` for alpha-mask behavior. It cannot produce fractional alpha, fragment
depth, sample mask, blend state, or an out-of-range/non-finite stored color.

Built-in solid, vertex-color, textured-unlit, and textured-mask appearances use
the same vertex stage, wrapper, binding layout, and validation path.

### 9.4 Naga Admission and Controlled Composition

This subsection governs the fragment shader module. The canonical Halcyon vertex
stage is a separate renderer-owned module and is not merged with application
source.

WGSL has no textual module-import mechanism. Conformance therefore forbids raw
string concatenation, brace/token splicing, macro substitution, or name-based
capture between application source and the controlled wrapper.

Admission has two structurally separate stages:

1. **Source library:** parse the complete application string as its own Naga
   module. It must have zero entry points, zero module-scope variables/resources,
   zero overrides, exactly one `halcyon_appearance` function with the signature
   in §9.3, and only bounded module constants, plain types, and helper functions.
   Every source-defined identifier beginning with the reserved
   `__halcyon355_` prefix is rejected.
2. **Controlled IR composition:** after source inspection, append the fixed ABI
   types, resource globals, and sole fragment entry point directly to the parsed
   `naga::Module` through structured IR handles. The wrapper calls the already
   resolved appearance-function handle; it does not resolve an application name
   again. The composed IR is then fully Naga-validated and structurally inspected
   before `Device::create_shader_module`. A backend may consume that Naga IR or
   WGSL emitted from that validated IR; application text is never pasted into a
   wrapper string.

This is not a general shader linker and does not merge two independently parsed
modules. The admitted source module remains the one module. A version-specific,
Halcyon-private compiler adapter appends only the fixed wrapper nodes to that
module and exposes no Naga type or handle through the public 3D API.

Halcyon 0.267.0-beta uses upstream wgpu 30.0.2 and Naga 30.0.2. The
version-coupled adapter retains this controlled IR path and its source spans.
Split barrier variants and cooperative matrix and ray-pipeline operations
remain outside source admission. The qualified upgrade proof is recorded in §16.2.

The source-level feasibility basis at adoption is the repository's locked
`wgpu` 25.0.2 and its re-exported Naga 25.0.1. That exact public API provides:

- public `Module` fields exposing the type, constant, override, global,
  global-expression, function, and diagnostic-filter arenas plus the entry-point
  vector;
- public `Type`, `GlobalVariable`, `Function`, `FunctionArgument`,
  `FunctionResult`, `EntryPoint`, `Expression`, `Statement`, and `Block` IR
  construction;
- `UniqueArena::insert`, `Arena::append`, `Arena::get_span`, `Block::push`, and
  typed expression `Range` construction without direct access to arena storage;
- `Module::to_ctx` and `Layouter::update` for the completed type arena;
- `Validator::validate` for complete post-composition validation;
- `back::wgsl::write_string` for a validated synthesized module; and
- `wgpu::ShaderSource::Naga` behind wgpu's `naga-ir` Cargo feature for direct
  native consumption.

An external probe against those exact versions parsed and independently
validated the §9.3 function with `texture_2d<f32>` and `sampler` parameters,
appended the fixed uniform/resource globals and a handle-calling fragment entry
point using only those public APIs, checked the 80/16 uniform layouts, validated
the completed module, constructed `ShaderSource::Naga`, emitted WGSL, and
reparsed/revalidated the emitted WGSL. Thus texture and sampler values are legal
ordinary-function parameters in the supported version, and controlled
composition is implementable without private Naga internals or textual linking.

Parser-assigned source spans remain attached to the unchanged source entries in
those arenas and must be preserved exactly. Synthesized nodes use either dedicated
renderer-template spans or `Span::UNDEFINED`; they must never reuse byte ranges
from application source. Source validation reports against the application
string before composition. A final validation failure located only in synthetic
nodes is an internal compiler-adapter failure, not a fabricated application
diagnostic.

The `naga-ir` Cargo feature is compiler plumbing, not a requested WebGPU Device
feature; `Features::empty()` remains unchanged. The implementation must isolate
this version-coupled adapter and record its exact wgpu/Naga pair. Any compiler
upgrade invalidates the adapter proof until the retained composition, layout,
validation, direct-consumption, WGSL-roundtrip, span-integrity, and collision
tests pass again.

If the supported Naga version cannot implement that handle-based composition and
final validation, the implementation is blocked and requires an authority
amendment. A textual linker is not an allowed fallback.

The final controlled module must satisfy all of the following:

1. exactly one entry point: the IR-constructed Halcyon fragment entry point;
2. exact fixed fragment inputs and exact location-0 `vec4<f32>` output;
3. only group 0 binding 0 and group 2 bindings 0, 1, and 2, with the exact
   resource types and layouts above;
4. no other bound or unbound resource global;
5. no storage, workgroup, push-constant, or writable module-scope address space;
6. no `override`, binding array, storage texture, external texture, atomic,
   barrier, subgroup operation, optional WGSL extension, `frag_depth`, or output
   sample mask;
7. the controlled entry point calls the exact admitted appearance-function
   handle and owns the finite/clamp/alpha-one return path;
8. source-IR dataflow over the admitted appearance/helper bodies identifies
   whether each `Normal`, `Uv`, or `Color` function parameter is actually read;
   the declared optional-semantic requirement set includes every such read
   parameter, while the wrapper's mechanically fixed call arguments do not by
   themselves make an unused semantic required;
9. no source declaration or IR node replaces or aliases the controlled ABI types,
   globals, wrapper, or entry-point handles; and
10. finite effective budgets cover source bytes, Naga IR nodes, functions,
    nesting, registered modules, and pipeline variants both before and after
    controlled composition.

Module constants, ordinary helper functions, function-local variables, ordinary
control flow, derivatives, filtered sampling through the fixed texture/sampler
parameters, and `discard` are permitted.

Application WGSL is trusted developer content, not an untrusted shader sandbox.
That trust does not bypass structural admission. Naga validation cannot prove
artistic correctness or termination; the fixed wrapper nevertheless enforces
finite bounded stored RGB, output alpha, and the resource/stage boundary.

### 9.5 Pipeline Key

A pipeline key contains every descriptor that changes pipeline correctness:

```text
ABI version
appearance source/content hash
culling mode
color format
Depth32Float depth format
sample count 1 | 4
```

Semantic-presence requirements are validation metadata and do not enter the key
because every mesh and appearance uses the same vertex input layout.

---

## 10. Per-View Targets and Render Execution

### 10.1 Target Ownership

Every active nonzero viewport owns:

- a SPEC-HAL-295 front/back pair in the surface-compatible color format;
- one private `Depth32Float` texture matching physical render extent and sample
  count; and
- when sample count is 4, one private multisampled color texture resolving
  directly into the single-sample back texture; and
- finite target-isolated persistent view/object dynamic storage and its
  queue-safe in-flight slots.

Sample count is exactly `1` or `4`. No other count is baseline.

At sample count 1, the back texture is the color attachment. At sample count 4,
the multisample color attachment resolves into the back texture in the same
render pass. There is no full-size copy, intermediate HDR target, or mandatory
tone-map pass.

No stencil texture, stencil aspect, stencil clear, or stencil operation is
created.

The viewport's `CustomRenderPass` is the sole publication owner for that pair.
`Context3D` neither registers a shared front nor mirrors the pass's
registration/unregistration state.

### 10.2 Target Sizing and Activity

The physical render extent is consumed exactly from SPEC-HAL-266A. The viewport
does not recalculate it from logical size or DPR.

`Inactive`, an empty logical rect, or zero physical extent creates no view target
and performs no dynamic-storage allocation/growth/write, 3D encode, or
submission. A change in target compatibility key
invalidates incompatible targets and fronts before paint can sample them.

### 10.3 Color and Alpha

The color attachment is cleared to transparent zero. The appearance wrapper
writes linear RGB with alpha one for surviving samples. Direct surface-format
storage performs the format's normal encoding; Image sampling decodes to the
premultiplied-linear values required by SPEC-HAL-263A.

The color target uses replace writes with all color channels enabled. The
multisample mask enables every sample and alpha-to-coverage is disabled. Alpha
masking is performed only by `discard`; no blend state or coverage heuristic may
turn appearance alpha into fractional object transparency.

Transparent clear, alpha-mask discard, and multisample resolve therefore produce
premultiplied-linear whole-viewport content. The `ImageNode` uses straight-linear
opaque WHITE tint. Image composition performs no repair or second
premultiplication.

For sample count 4, let `N = 4` and let `k` samples survive coverage, depth, and
mask discard. Every surviving sample stores `(C_i, 1)` with finite
`C_i` in `[0,1]^3`; every untouched sample retains clear `(0,0,0,0)`. Normalized
color resolve is component-wise in the format's linear numeric domain, with the
format encoding applied to the resolved stored value. Therefore the sampled
linear front is:

```text
RGB = (sum over surviving C_i) / N
A   = k / N
```

This is the coverage-weighted premultiplied representation of the surviving
colors. For constant `C`, `RGB = C * A`. Discard writes no replacement value, so
masked samples remain transparent zero. This derivation is a binding dependency,
not sufficient evidence by itself; §16 requires the actual publication/Image
oracle for clear, full coverage, geometry-edge coverage, and mask discard on each
claimed backend and surface-compatible format.

### 10.4 Preparation and Submission Order

For one requested tuple:

```text
consume final immutable target metrics
    -> validate view, structural snapshot, complete model state, handles,
       appearances, target compatibility, limits, and budgets
    -> capture/reuse structural leases and prepared packet/material state
    -> create/grow/reuse compatible target and bounded dynamic storage
    -> pack and write only validated immutable view/object dynamic bytes
    -> encode participant-owned offscreen pass to back/MSAA target
    -> Queue::submit on captured baseline Queue
    -> submit returns
    -> swap back to front
    -> queue registered-front publication
    -> atomically commit the tuple
    -> SPEC-HAL-266A forces scene materialization/presentation
    -> Image registration drains before 2D compile/sampling submission
```

Every writer command buffer for a front must be enqueued before swap,
publication, and later Image sampling, exactly as required by
INV-295A-008. Publication does not wait for GPU completion.

All fallible validation, allocation, lease capture, dynamic-capacity growth, and
local commit-state reservation complete before dynamic writes and encoding. A
failed candidate neither partially advances dynamic storage truth nor damages
the prior compatible last-good storage/front. After the per-view `CustomRenderPass`
successfully queues registration for the swapped front, tuple/front/lease/status
commit is one non-allocating, infallible CPU state transition under exclusive
participant ownership. No validation, channel operation, or allocation may occur
between successful registration queueing and that assignment.

If registration queueing can fail in an implementation, the candidate is not
committed and the implementation must transactionally preserve or restore the
previous compatible front and its leases. A submitted/swapped candidate with a
failed publication queue may be retired, but it cannot be exposed as committed.
An implementation may instead prove from `CustomRenderPass` live-state
preconditions that its post-swap registration enqueue has no recoverable failure;
process-fatal allocation remains under the inherited device/process boundary.

This SPEC requires no generalized command-submission scheduler. A conforming
implementation may preserve independent per-view submissions or batch already
encoded command buffers if it preserves per-front ordering and adds no 2D hot
path work. Submission batching, Plot migration, and worker scheduling are not
baseline authority.

### 10.5 Dirty and Unchanged Views

A view renders only when at least one of the following changes:

- requested view revision;
- requested model revision;
- requested structural revision/identity;
- explicitly selected independent complete-PBR-source identity (§11.2);
- referenced resource or appearance generation;
- target compatibility key;
- view activity; or
- explicit loss of a compatible committed front.

Unchanged requested inputs and stable target metrics reuse all compatible
resources and the committed front. They perform no uniform write, encode,
command-buffer creation, submission, swap, or registration.

With unchanged structural identity and target pipeline class, a changed
view/model revision reuses all geometry, appearances, textures, samplers, leases,
shader modules, pipelines, packet ordering, GPU buffers, and bind groups. Only
bounded view/object dynamic bytes may be packed and written; encode, submission,
swap, and publication then follow the ordinary transaction.

Camera or model animation follows SPEC-HAL-021 and SPEC-HAL-354: Widget update
samples Timeline read-only and publishes a complete immutable view/model value
and revision before layout. Same-frame custom preparation consumes that exact
revision without Timeline access. The renderer owns no time source, animation
phase, or scheduler and never calls a private clock.

---

## 11. Lifecycle, Generation, and Publication State Machine

### 11.1 States

One viewport target is in exactly one state:

```text
Uninitialized
Inactive
ReadyWithoutCommit
Committed
FailedWithLastGood
IncompatibleTarget
Retired
TerminalDeviceFailure
```

`Committed` and `FailedWithLastGood` include a committed tuple and valid front.
`ReadyWithoutCommit`, `Inactive`, `IncompatibleTarget`, `Retired`, and
`TerminalDeviceFailure` expose no sampleable front unless another explicitly
compatible target instance owns it.

### 11.2 Request Admission

Every new request receives a non-wrapping request serial. Structural, model, and
view revisions are independently monotonic and non-wrapping. Request serial
orders render attempts; the three semantic revisions identify their immutable
values. Equal structural revision requires the same structural snapshot identity
and content. Equal model revision requires the same complete model-state identity
and content for that structure. Equal view revision requires the same immutable
view descriptor. A lower semantic revision cannot replace a higher requested or
committed value in its domain.

**Binding narrow source-domain selection — the supervising technical reviewer, 2026-11-05.** Under JC's standing technical
delegation, add exactly
`Viewport3D::replace_pbr_scene(&mut self, scene:PbrScene3D, view_revision:Viewport3DViewRevision, view:ViewDescriptor3) -> PbrSourceResult3D<bool>`.
This is explicit independent whole-source selection, alongside the unchanged
within-source `request_pbr_scene`; it is not an automatic successor exception.
The replacement must be a different strongly owned source Arc with a different
producer-authored complete-scene HDR identity from the currently requested PBR
source. Same-Arc or same-HDR-domain replacement refuses. A separately admitted,
still-live immutable A may be explicitly selected after B, even when its own
structural/model/HDR revisions are numerically equal to or lower than B's.

Before requested state changes, the replacement must pass the original complete
Context/Device/source and lease validation, snapshot/model packet closure,
supported sample count and existing view-revision checks. Retired source or
Context, foreign Context, invalid association or unsupported target/source must
refuse truthfully. Equal view revision still requires identical view content;
independent source selection is not a camera change. The original successor
validators and their structural/model/HDR and independent direct/indirect
generation checks remain unchanged on `request_pbr_scene`.

The new ingress uses the existing complete requested-state, request-serial,
target preparation and commit path. It retains the same viewport and compatible
target, retires the old HAL372 occurrence without transferring progress, and
requires the new source's matching extent for a subsequent reveal. An optional
effect refusal preserves ordinary source eligibility. It adds no source registry,
history map, renderer, clock, implicit revision synthesis, semantic source owner,
or linked/domain writer grant. This selection is technical S1 authority, not
source-test, backend, native or physical-visual acceptance.

Request admission validates the complete atomic tuple. A model-state structural
identity/count mismatch fails before requested target state or GPU state changes.
Model and view values cannot be published as independent partial patches whose
interleaving is left to preparation.

A changed target compatibility key, activity transition, or loss of a compatible
front creates a new request serial and may require a rerender while retaining the
same structural, view, and model revisions. Target/DPR/extent/sample lifecycle
is not a fake structural, camera, or model change. A successful resize commit may
therefore have a newer request serial and target key with semantic revisions
equal to the preceding commit.

Request admission records requested state but does not alter committed state.
Validation completes before encode. Failed validation creates no partial GPU
mutation visible to publication. If device materialization fails after CPU
validation, every partially created object remains unpublished and is rolled
back or retired under the same context before another request can observe it.

**INV-355-G1-CHECKED-COMPATIBILITY / FORBID-355-G1-REVISION-OR-DOMAIN-REWRITE.**
An explicitly adopted HAL-354/HAL-266A mounted Update transfer may feed the
existing complete ordinary flat Viewport3D::set_request only. It MUST preserve
HAL-355's structural/model/view identities, equal-revision equality requirements,
monotonicity, atomic request admission, resource limits, target-local preparation,
requested/committed distinction and last-good/retirement truth. It MUST NOT
synthesize HAL-357 domains, fabricate request serials or GPU success, add a
camera/controller to rendering, or require another renderer resource, queue,
timing or scheduling mechanism. A transfer receipt is not a committed tuple.
The seven proposed transfer categories and bounded APP-owned receipt lifetime in
HAL-354 MUST remain separate from renderer request/prepare/commit/presentation;
known synchronous setter results MUST NOT be classified Indeterminate. Status
cloning is historical mechanical evidence, not the surviving receipt owner.

**INV-355-G1-EXCLUSIVE-ADMISSION / FORBID-355-G1-LINKED-DIRECT-WRITER.** For an
explicitly linked target/mount, only the canonical engine-qualified HAL-G1 cutoff
may change the complete request. Link activation MUST reject incompatible active
ownership. Every direct public set_request attempt while linked, even equal,
MUST synchronously return an explicit typed exclusive-writer refusal without
changing requested/committed/target state. The qualified transfer MUST pass the
same existing checked semantic/structural/view/target validation without bypass.
Old compatible fronts retain their original tuple and provenance; activation
MUST NOT reattribute them. Direct Screen-driven requests remain fully legal for
UNLINKED mounts and may resume only after lawful link retirement. This proposed
opt-in recipient condition changes no unlinked API or renderer behavior and
selects no engine-caller authentication, Rust error spelling or data structure.

**INV-355-G1-NO-USE-COST / FORBID-355-G1-2D-CONTAMINATION.** No active link means
the existing direct Screen-set and ordinary flat viewport paths retain their
behavior and structure. Zero-view/ordinary 2D operation MUST acquire no 3D branch,
link-driven renderer work, 3D allocation or hidden subsystem dependency. Unrelated
target/lifecycle work remains governed by its existing owner; an equal publication
must not renew work merely because it was repeated.

### 11.3 Successful Commit

A commit occurs only after:

1. the entire tuple is validated;
2. every structural lease and required dynamic-storage slot is captured;
3. all dynamic writes and writes to the candidate front are successfully
   enqueued on the captured
   Queue;
4. front/back ownership is swapped without aliasing; and
5. the new front publication is queued with CPU happens-before ordering.

The tuple, front, leases, and externally visible status change atomically from
the participant's perspective through the infallible non-allocating transition
defined in §10.4. Successful publication queueing can never precede a later
recoverable failure to install that local committed state.

### 11.4 Last-Good Failure Semantics

If a changed camera, model state, structural snapshot, resource, or appearance
request fails while the
committed front remains compatible with the current target and lifecycle:

- the committed tuple and front remain published;
- preparation reports no front-publication change;
- requested and committed revisions remain distinct and queryable;
- status becomes `FailedWithLastGood` with the failed request serial and typed
  error; and
- picking and any committed-view query continue to use committed state.

The old image is truthful as the last committed revision. It must not be
reported as the failed requested revision.

For explicit independent PBR source replacement, the old compatible committed
front remains attributed to its original complete source only as last-good until
a real complete replacement front commits. Admission refusal changes neither
requested nor committed state. A later preparation failure retains the failed
replacement request and the truthful old committed source; it never relabels
the old image as the newly selected source. Incompatible target/lifecycle or
terminal Device withdrawal still follows §11.5, with queue-safe ownership and
retirement under §11.6. No discarded reveal occurrence may be revived by an old
source's late completion.

Last-good state is target-local. Each `SurfaceId` independently owns its
requested/committed tuple, dynamic storage, front, status, and failure. One
surface may commit or lag while another fails; no target may overwrite, withdraw,
or report another target's model/view revision or dynamic-storage state.

### 11.5 When the Front Must Be Withdrawn

The viewport omits/unpublishes its Image when:

- no committed front exists;
- physical render extent, format, sample count, context/device identity, or
  target lifecycle is incompatible;
- the view becomes inactive or is removed; or
- the captured device lifetime is terminal.

A semantic request failure alone is not grounds to blank a compatible last-good
front.

### 11.6 Removal and Retirement

Widget removal prunes the SPEC-HAL-266A participant entry, stops further
preparation, queues front unregistration, and retains published textures until
the unregistration drain makes release safe. Resource generations retire only
after all committed, requested, and structural-snapshot leases are released.
Target dynamic storage and immutable model/view values remain only while a live
requested, committed, last-good, or in-flight owner requires them; no historical
revision chain is retained.

---

## 12. Error Contract

Consumer-controlled invalid input produces typed, non-panicking errors before
the corresponding wgpu operation. At minimum, typed categories distinguish:

- non-finite or invalid view/projection/model values;
- structural/model-state identity or packet-count mismatch;
- unrepresentable render-relative conversion;
- stale/lower/exhausted revision;
- context, resource-kind, slot, or generation mismatch;
- invalid geometry/index/semantic requirement;
- rejected WGSL/ABI/module/pipeline descriptor;
- target incompatibility or inactivity;
- hard Device/format/sample-count limit; and
- finite dynamic-storage/in-flight/upload or other policy-budget exhaustion.

Errors are stored in participant-local status keyed to request serial. The
generic SPEC-HAL-266A callback does not become a semantic error bus.

A consumer-data error must not panic the application, publish a partial front,
or silently substitute another projection, mesh, appearance, sample count, or
resource.

Portable APIs cannot prove total VRAM availability or make device loss/OOM
reliably recoverable per view. Fatal device or allocation failure inherits the
baseline SPEC-HAL-295A device-lifetime boundary and must not be misreported as a
successful local fallback.

---

## 13. Finite Budgets and Device Limits

### 13.1 Normative Budget Law

Every context has a finite, queryable effective configuration. There is no
unlimited mode.

The configuration explicitly bounds at least:

1. live views within that context and their active target pixels;
2. single target extent and total color/depth/MSAA attachment bytes;
3. single mesh bytes, vertices, and indices;
4. aggregate resident geometry bytes;
5. single texture extent/bytes and aggregate texture bytes;
6. draw packets per structural snapshot and placements per complete model state;
7. target and aggregate dynamic-storage bytes and object capacity;
8. live/in-flight dynamic-storage slots and retained CPU staging high-water;
9. total and dynamic upload bytes per preparation/frame;
10. simultaneously retained requested/committed/last-good model-state values;
11. live resource slots and retained generations;
12. appearance source bytes and Naga IR/functions/nesting;
13. registered appearances, shader modules, and pipeline variants;
14. pending requests/publications; and
15. cache residency.

All size/count products use checked arithmetic. Exceeding a limit is a typed
error and performs no partial commit.

A context cannot count or cap sibling `Context3D` instances without the
forbidden global registry. The number of contexts is therefore explicit
application policy, not a context-enforced substrate budget. Every individual
context remains finite; aggregate pressure from several contexts is still subject
to Device limits and fallible allocation, but this SPEC makes no false claim of a
global memory governor.

### 13.2 Hard Limits Versus Policy Budgets

`Device::limits()` and guaranteed format features are hard safety ceilings for
texture dimension, buffer size, uniform binding size/alignment, bind groups,
vertex buffers/attributes/stride, interstage components, indices, and sample
compatibility.

They are not a total-memory or work budget. A buffer below `max_buffer_size` may
still fail allocation, and WebGPU exposes no portable total VRAM, draw-count,
pipeline-count, shader-complexity, or upload-throughput limit.

### 13.3 Construction-Time Defaults

The implementation supplies a finite conservative default profile. Structural
defaults are clamped to live Device limits and format
features. Aggregate memory/work defaults are finite implementation constants,
not claims about available VRAM.

Applications may supply another finite profile within hard device bounds. The
context exposes the resulting effective profile. Profiling may tune default
constants without changing this normative distinction, failure behavior, or the
ban on unbounded configuration.

Numeric default values are implementation policy and do not block this SPEC's
authority. They must be documented and tested with the implementation that
chooses them.

---

## 14. Portability Profile

### 14.1 Native Baseline

The baseline is legal under the current Halcyon device request:

```text
wgpu::Features::empty()
wgpu::Limits::default()
```

It uses only WebGPU-core facilities:

- triangle lists;
- f32 WGSL;
- ordinary vertex, index, and uniform buffers;
- one sampled 2D appearance texture and filtering sampler;
- one surface-compatible color target;
- `Depth32Float`;
- sample count 1 or 4 with direct resolve;
- `u16` / `u32` indices; and
- no threads, blocking GPU future, or optional feature in preparation.

The native conformance target is the same contract across wgpu's primary Metal,
Vulkan, and DirectX 12 backends. Backend-specific semantic variants are
forbidden.

Optional capabilities—including push constants, compression, bindless arrays,
multi-draw indirect/count, shader f16, timestamp queries, polygon line/point
mode, depth-clip control, `Depth32FloatStencil8`, float32 filtering, extra sample
counts, and adapter-specific format features—are absent from baseline. Their
absence cannot affect existing 2D or baseline 3D validity.

### 14.2 Authority, Implementation, and Backend Status

Architecture adoption and implementation conformance are separate status axes:

- **authority status** belongs to this document and is Adopted / Binding;
- **implementation status** records whether code implements the adopted contract;
  and
- **backend conformance status** is recorded independently for Metal, Vulkan,
  DirectX 12, and the later browser vertical.

Adopting the architecture claims no implementation and no backend support. An
implementation may be conforming on Metal while Vulkan and DirectX 12 remain
unimplemented or implemented-but-unverified. Such a build may claim only the
proved backend row; it must describe overall native implementation as partial,
not complete. Lack of an available CI host does not block architectural adoption,
but it does block a conformance/support claim for that backend.

Each backend row must identify the implementation revision, adapter/backend,
proof artifact, and one of `Unimplemented`, `Implemented — Unverified`, or
`Conforming`. Only `Conforming` is a support claim. The full native implementation
may be called conforming only when every backend it claims has passed the same
§16 contract without a semantic variant.

### 14.3 Browser Status

The API and shader profile are designed to be WebGPU-core and wasm legal.
Browser execution is nevertheless **provisional and unverified**, not a
supported runtime claim.

Browser support may be claimed only after the browser vertical in §16 proves
wasm build, adapter/device creation, target formats, 4× color/depth creation,
WGSL admission/pipeline creation, DPR resize, ray alignment, Image
alpha/publication, and nonblocking error behavior.

The initial native substrate SPEC is not blocked on that later execution proof.

---

## 15. Forbidden Behaviors

**FORBID-355-2D-CONTAMINATION:** Adding 3D variants, fields, branches,
traversal, state, depth work, pipelines, or resources to the existing 2D
SceneGraph, Batcher, DrawPlan, SceneCache, spatial substrate, RenderGraph, or
ordinary 2D application path.

**FORBID-355-HIDDEN-CONTEXT:** Storing or discovering `Context3D` through
`WgpuRenderer`, a global/singleton, a hidden AppContext registry, thread-local
state, or implicit Device lookup.

**FORBID-355-ADMISSION-AUTH:** Treating `CustomRenderAdmission` as proof of
Device, Queue, context, epoch, handle, or lifecycle identity.

**FORBID-355-RAW-GPU-ESCAPE:** Publicly accepting or returning raw wgpu resource,
pipeline, encoder, command-buffer, or bind-group ownership in place of typed
context-bound handles.

**FORBID-355-MIXED-PRECISION:** Exposing generic `Real`, a third-party math type,
parallel f32/f64 APIs, absolute f64 mesh vertices, or silently narrowing world
translations.

**FORBID-355-DEPTH-DRIFT:** Caller-selectable handedness/depth convention,
conventional-Z fallback, wrong clear/compare pair, hidden Y reflection,
`FrontFace::Cw` under the canonical projection, or culling without the required
oracle.

**FORBID-355-UNBOUNDED-GEOMETRY:** Arbitrary vertex layouts, streams, semantics,
topologies, unchecked indices, or geometry admitted without finite budgets.

**FORBID-355-ARBITRARY-SHADER:** Custom vertex/compute entry points, raw render
pipelines, uninspected WGSL, textual wrapper/source splicing, name-capture
linking, undeclared bindings, storage/atomic/subgroup access, fragment depth,
sample-mask output, or validation bypass because source is trusted.

**FORBID-355-FRACTIONAL-TRANSPARENCY:** Claiming general transparent-object
correctness, enabling fractional fragment alpha, or using draw sorting as a
false solution for intersecting/self-transparent geometry.

**FORBID-355-PARTIAL-MODEL-STATE:** Sending placement patches, dirty-object
commands, hierarchy mutations, or an array not bound to the exact structural
identity/count; applying model/view pieces independently so preparation may
observe a mixed semantic tuple; or mutating committed model state in place.

**FORBID-355-DYNAMIC-CHURN:** Re-admitting structural leases or recreating
geometry/appearance materialization, textures, samplers, shader modules,
pipelines, GPU buffers, bind groups, packet metadata, or per-packet CPU vectors
for a model/view-only revision after compatible capacity is warm.

**FORBID-355-FALSE-PUBLICATION:** Reporting requested revisions as committed,
publishing before all writes are enqueued, withdrawing a compatible last-good
front merely because a newer semantic request failed, allowing registration
queueing and tuple installation to split, or picking against uncommitted state.

**FORBID-355-TARGET-MISMATCH:** Sampling or retaining as current a front whose
extent, format, sample count, context/device, or lifecycle is incompatible with
the current SPEC-HAL-266A target snapshot.

**FORBID-355-SEMANTIC-GPU-AUTHORITY:** GPU state, readback, render snapshot,
camera helper, or hit result becoming application/domain/selection authority.

**FORBID-355-INFERRED-TRANSFORM-TARGET:** Treating a packet slot, model placement,
geometry handle, appearance, ray hit, visible background, or raster result as a
semantic figure/root/group or transform target; adding a generic `rotate_scene`
operation; or binding input to model mutation without an application-owned
explicit target source and policy.

**FORBID-355-PRIVATE-SCHEDULER:** A renderer-owned clock, animation policy,
worker pool, frame loop, time-window submission scheduler, or hidden wakeup
mechanism for 3D.

**FORBID-355-UNLIMITED-BUDGET:** Any unbounded context configuration, unchecked
size product, dynamic arena, in-flight slot count, staging retention, revision
history, or reliance on Device maxima as aggregate memory/work policy.

**FORBID-355-PORTABILITY-OVERCLAIM:** Requiring an optional feature for baseline,
creating backend-specific semantics, or calling browser execution supported
before its vertical proof.

**FORBID-355-RECOVERY-OVERCLAIM:** Claiming device epoch, device recreation,
automatic replay, or nonfatal OOM/device-loss recovery under the current
baseline lifetime.

---

## 16. Contract and Proof Obligations

Adoption defines authority; implementation conformance requires retained proof
for every item below.

### 16.1 Structural and CPU Proof

1. Public math types expose no third-party dependency and enforce the role-based
   scalar model.
2. Finite/invalid pose, projection, placement, render-origin conversion, and
   revision cases have exact typed results; local-orientation composition proves
   `q * r` order and normalization; positive non-uniform TRS proves a finite
   invertible `L`, positive determinant, exact inverse-transpose normal matrix,
   and unchanged winding.
3. Perspective finite/infinite and orthographic finite projections map near/far
   under reverse-Z and agree with analytic `Ray3` construction.
4. Pure ray-plane/sphere/AABB/triangle/local-transform helpers have deterministic
   edge-case coverage without semantic selection state.
5. Geometry accepts exactly the 36-byte layout, decodes vertex-color bytes as
   linear `b/255`, records semantic presence, materializes exact defaults,
   validates whole-buffer triangle draws, and rejects a missing required semantic
   without creating a pipeline variant.
6. Context/kind/slot/generation mismatch and generation exhaustion fail before
   wgpu.
7. Structural-snapshot/resource leases prevent stale reuse and stabilize under
   repeated replacement/retirement; model/view-only revisions retain the exact
   same structural identity and leases.
8. Structural/model/view identity and revision rules reject reuse, regression,
   packet-count mismatch, and partial model state before GPU work. Requested and
   committed complete tuples remain separately observable through success,
   failure, resize, inactivity, removal, and terminal device state.
9. Mechanical ray/intersection output and flat packet order never become
   selection or transform-target authority; explicit application target changes
   publish only a complete successor model state.

### 16.2 Shader and ABI Proof

9. Rust and WGSL layouts prove exact sizes 80, 112, and 16; camera-relative and
   inverse-transpose fields equal their §6 definitions; dynamic-offset allocation
   uses live device alignment independently.
10. Untextured and textured appearances use the same bind-group layout, with the
    exact 1×1 opaque-white fallback.
11. Every rejected Naga shape in §9.4 has focused negative coverage. A retained
    adapter probe records the exact wgpu/Naga pair and compiles using only the
    public APIs named there. Tests prove source-library parsing has zero entry
    points/globals, source spans survive composition unchanged, synthetic spans
    cannot alias application text, reserved-name collision fails, controlled
    composition uses function/IR handles rather than text, `Layouter` reports the
    exact fragment-side 80/16 layouts, the final `Validator` result has the exact
    sole entry point, `ShaderSource::Naga` construction succeeds, emitted WGSL
    reparses/revalidates, texture/sampler function parameters remain legal, and
    optional-input use cannot exceed the declared semantic requirement set. Any
    wgpu/Naga upgrade must pass this probe before the implementation can claim
    conformance.

    For 0.267.0-beta, all 16 existing `tests::three_d_appearance::` tests passed
    with upstream wgpu 30.0.2 and Naga 30.0.2. The integrated source retains the
    reviewed controlled adapter, test bodies and resolved graphics dependency
    family. This proof covers the 80/112/16 ABI, admission and span integrity,
    direct Naga source construction, and emitted WGSL round-trip validation.

12. The Halcyon wrapper maps non-finite RGB to zero, clamps finite RGB, forces
    alpha one for surviving fragments, preserves mask discard, and allows no
    appearance depth or sample-mask output.
13. Pipeline keys distinguish every correctness-changing descriptor and omit the
    semantic-presence bitset.

### 16.3 Actual GPU and Publication Proof

14. A minimal actual-target raster oracle renders known outward-CCW and reversed
    triangles with `FrontFace::Ccw`/`Cw` and back culling, then samples/publishes
    the result. The canonical path must prove only outward-CCW survives under
    `FrontFace::Ccw` on each claimed native backend.
15. `Depth32Float`, clear 0, and `Greater` prove near-over-far occlusion for
    finite and infinite perspective and finite orthographic views.
16. Sample counts 1 and 4 produce compatible fronts; 4× renders with private
    multisample color/depth and resolves directly to back without a full-size
    copy.
17. The actual surface-compatible format oracle renders known constant linear
    colors through transparent clear at sample counts 1 and 4, including full
    coverage, geometry edges with unknown implementation sample positions, and
    alpha-mask discard. Sampling the published front and production Image path
    must show clear `(0,0,0,0)`, full `(C,1)`, and at every partial pixel
    `RGB = C * A` within declared format/resolve quantization tolerance, followed
    by exact SPEC-HAL-263A composition. A varying-color case separately verifies
    coverage-weighted premultiplied resolve.
18. Every candidate front's writes are submitted before swap/publication and the
    front is sampled later on the same Queue without a GPU completion wait.
    Success and injected registration failure prove that publication and the
    non-allocating tuple/front/lease commit cannot split.
19. No registered or pending front becomes writable back; resize/removal retains
    texture storage until unregistration drains.
20. A failed newer camera, model state, structural snapshot, handle, dynamic
    storage, budget, or shader request keeps a
    compatible committed image and exposes requested/committed/error truth;
    incompatible resize or lifecycle withdraws it. A changed target key also
    rerenders under a new request serial with equal semantic revisions, while
    changed content under an equal revision is rejected.
21. Two viewports sharing one context share immutable device-bound resources but
    retain private target/depth/MSAA state; cross-context handle use fails before
    wgpu.
22. DPR 1.0, 1.5, and 2.0 plus distinct `SurfaceId` snapshots use exactly the
    SPEC-HAL-266A physical render extent and invalidate only affected target
    state.

### 16.3A Dynamic-State and Animation Proof

**DYN-PROOF-355-001 — Structural/Model Split:** A fixed structural snapshot is
paired with multiple complete model-state revisions. Every revision changes
only placements, preserves exact packet-slot correspondence and structural
leases, and rejects missing/extra/mismatched placements before any write.

**DYN-PROOF-355-002 — Same-Frame Timeline Feed:** The production frame path uses
SPEC-HAL-021 `Once`, `Loop`, and `Alternate` samples during Widget update. An
event-started handle publishes exact `from`; existing handles publish the current
frame value; and custom preparation consumes the exact new model/view revision
in that frame before paint with no one-frame lag or Timeline access. Shared
timestamps produce equal semantic values at 60 Hz and 120 Hz. Revision numbers
remain monotonic within each run and identify that run's immutable values; they
need not equal a different cadence history that published additional intermediate
samples.

**DYN-PROOF-355-003 — Warm Stable Resources:** After target, structural packet
capacity, pipeline class, and dynamic storage are warm, sustained 60 Hz and
120 Hz model animation keeps structural identity, geometry/appearance leases,
geometry/texture/sampler materializations, shader modules, pipelines, target
attachments, GPU buffers, and bind groups constant. Bounded dynamic view/object
writes, encodes, submissions, swaps, and publications advance as expected.

**DYN-PROOF-355-004 — Bounded Allocation and Counters:** Instrumentation reports
separate counters/high-water values for structural admissions and
materializations; GPU buffer/bind-group creation; dynamic capacity, live/in-flight
slots, CPU staging, and bytes written; and encode/submission/publication. A long
steady run performs no post-warm-up structural or GPU-object allocation and does
not retain revision history. Current resource-usage counters that omit transient
uniform/bind-group churn are not sufficient evidence.

**DYN-PROOF-355-005 — Atomic Camera and Model:** Concurrent camera pan/zoom input
and repeating model animation publish complete view and model revisions. Every
requested/committed record is one atomic structural/model/view tuple; no frame
loses one update, mixes revisions, or prepares a partial pair.

**DYN-PROOF-355-006 — Multi-Surface Isolation:** Two surfaces sharing immutable
structural resources may prepare in either order and at different cadence. Each
retains its own dynamic storage, requested/committed tuple, front, last-good, and
failure. One surface may lag or fail without changing the other's truth or
resource counters beyond legitimate shared immutable residency.

**DYN-PROOF-355-007 — Transactional Failure:** Injected dynamic-capacity,
per-frame-upload, pending-request/publication, encode/publication, and compatible
semantic failures preserve the prior target-local committed tuple, storage,
front, and leases. Incompatible target change withdraws the front; terminal
device loss follows the existing baseline without claiming recovery.

**DYN-PROOF-355-008 — Zero Work:** Equal structural/model/view/target input
performs zero dynamic writes, encodes, command-buffer creation, submissions,
swaps, or publications. Inactive or zero-area targets perform no dynamic-storage
allocation/growth/write. Removing the final 3D view releases bounded target-local
dynamic state through the declared in-flight lifetime.

### 16.4 Zero-View and 2D-Preservation Proof

23. Test-only or explicit audit-feature instrumentation is compiled out of the
    ordinary release path and is placed only inside 3D admission, construction,
    resource, encode, submit, and publication owners—not inside normal 2D
    traversal.
24. With capability compiled and no 3D context/view, every specialized counter
    is exactly zero: contexts, registered views, workers, jobs, resources,
    depth/MSAA attachments, dynamic storage/slots/writes, buffers, bind groups,
    pipelines, shader modules, passes, command buffers, registrations, and
    submissions.
25. Relative to the adopted 295A/263A/266A foundation with no `Context3D` or
    3D viewport, the existing generic custom-participant branch behavior, 2D
    RenderGraph topology hash, pass count, queue-submit and command-buffer count,
    retained-scene behavior, and present decisions remain equal for the same
    static and animated Showcase workloads.
26. Uninstrumented release A/B performance uses the same revision pair, adapter,
    OS/backend, surface/display mode, workload, build profile, and measurement
    tool. Runs alternate or randomize A/B order, use a declared warm-up stability
    rule, predeclare a sample plan with power to detect the material threshold,
    report raw samples and distribution/confidence analysis for total and stage
    CPU time plus present cadence, and distinguish noise from regression.
    Before a conformance claim, the benchmark tool/document publishes a versioned
    measured noise floor and material-regression threshold; those values are
    fixed before evaluating the candidate. Passing means no slowdown both
    statistically attributable to compiled-but-unused 3D and larger than that
    predeclared material threshold, with no present-cadence regression. Structural
    zero is the primary normative law and timing can never excuse nonzero 3D
    work.

### 16.5 Portability Proof

27. Each claimed Metal, Vulkan, and DirectX 12 backend row records and passes the
    same semantics under `Features::empty()` / `Limits::default()`. Unavailable or
    unrun rows remain `Unimplemented` or `Implemented — Unverified`; they neither
    block authority adoption nor permit a support/full-native-conformance claim.
28. No optional feature request or 3D capability absence changes 2D device
    creation, 2D pipeline validity, or baseline rendering.
29. Browser support remains unclaimed until wasm build, WebGPU device/format,
    4× targets, WGSL, DPR, ray, publication/alpha, and nonblocking error behavior
    all pass through the real browser vertical.

No proof may replace the production Image shader, custom-render publication
path, or actual target with a CPU-only or reimplemented oracle where this section
requires the live path.

---

## 17. Explicit Non-Goals and Reduction Arguments

| Excluded system | Why it is not substrate |
|---|---|
| Retained 3D scene graph or ECS | The renderer needs one immutable flat draw snapshot. A retained hierarchy would duplicate semantic ownership and mutation machinery without being required for rasterization. |
| Durable camera/controller | Rendering needs a checked immutable view descriptor. Orbit, fly, pan, zoom, constraints, target following, easing, and persistence are interaction/application policy already served by Widget update and Halcyon animation. |
| Renderer time, scheduler, or workers | A changed immutable revision is sufficient input. Existing frame/update admission owns cadence; a second clock or worker policy would add authority without changing render correctness. |
| General fractional transparency | Opaque and alpha-mask content is depth-correct and compositable now. Draw sorting cannot generally solve intersecting or self-transparent geometry; correct OIT/translucency requires separate algorithms, budgets, and proof. |
| Lighting engine, PBR, shadows, environment system | The fixed appearance function can express bounded unlit/procedural color. Light storage, shadow passes, multi-texture materials, and environment semantics are independent rendering systems, not prerequisites for geometry/depth/publication. |
| Asset formats, loaders, streaming, compression, mip generation | The substrate consumes validated immutable resources. File/network decoding and asset policy are producer concerns and would import domain/lifecycle semantics. |
| Skeletal animation, morphing, particles, physics | These systems generate changing geometry or placements. Stable topology uses the complete immutable model-state boundary for final flat placements; changed geometry publishes a new structural snapshot/resource generation. The substrate owns neither simulation nor authoring hierarchy. |
| CPU mesh mirrors, BVH, scene picking, GPU ID picking | Mechanical `Ray3` plus pure primitives is sufficient substrate. Acceleration structures, hit keys, readback, and selection meaning depend on consumer data and update policy. |
| Generic N-dimensional or unified 2D/3D renderer | Existing 2D SceneGraph/Batcher is mature and intrinsically 2D. Generalization would contaminate its hot path; 3D shares only real GPU/lifecycle/presentation owners. |
| Per-geometry 2D/3D cross-depth | An embedded viewport is one retained image plane. No current Halcyon product authority requires arbitrary 2D primitives to depth-test with internal geometry; adding it would replace the proven offscreen seam. |
| HDR, tone mapping, deferred rendering, post-processing | Baseline linear shader output can store directly in the surface-compatible target and compose without a copy. HDR/deferred/post introduces new formats, passes, color authority, and budgets. |
| Stencil | Baseline visibility and masks require only `Depth32Float` plus fragment discard. Stencil allocation and operations add state without an intended capability. |
| Extra sample counts and optional GPU features | Sample 1/4 and WebGPU-core limits cover the baseline. Optional capabilities would change device negotiation and portability without being required. |
| Device-loss recovery | No renderer-owned Device/Queue recreation epoch exists. A 3D-private epoch would be decorative and inconsistent; recovery must be solved once for all custom GPU participants. |
| Split-runtime support | The canonical runtime already owns the required preparation and publication seams. Draft `SceneSwapchain` lacks the GPU-service and lifecycle transport needed for an honest claim. |
| Browser support claim | Core API legality does not prove execution. The later browser vertical can promote support without changing native authority. |
| General custom-render submission scheduler | Correct publication requires per-front ordering, not a global batching service. Submit-cost evidence has not justified adding scheduler ownership or migrating Plot. |
| Raw vertex/compute stages or raw render pipelines | The fixed vertex stage and bounded fragment function provide programmable appearance while preserving geometry, depth, resource, and alpha invariants. Raw pipelines would make those contracts unenforceable. |
| Selection, gizmos, semantic hit keys, commands | The substrate may return mechanical rays/hits only. Meaning and irreversible actions belong to explicit application/widget policy. |

Exclusion does not mean a system can never exist. It means that system must earn
separate authority and cannot be smuggled into baseline implementation as a
helper, convenience, or speculative generalization.

### 17.1 Binding opt-in visual-profile extension — coordinated S1 adoption, 2026-10-31

The delegated technical reviewer, acting under JC's express delegation to decide technical acceptance, adopts [HAL-370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md) as this substrate's narrow opt-in extension, together with [R-023](SPEC-R-023-3D-Visual-Rendering-Extension-Architecture.md), [R-024](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md), [R-025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md), [R-026](SPEC-R-026-Environment-and-Indirect-Lighting.md), [R-027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md) and [R-028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md). JC retains constitutional supremacy and final veto; this does not assert that JC personally reviewed the technical SPECs.

ONLY for the expressly selected new opt-in family, HAL-370 §8's exact per-clause DELTAs take precedence over this SPEC's listed fixed source/ABI/format/feature restrictions. All other clauses remain controlling. The §17 exclusions still apply to the BASELINE and to every unlisted feature. Legacy 36-byte vertices, three groups and View80/Object112/Appearance16, old source/view/model packing, wrapper, surface-compatible final and the existing exclusive HAL-354 linked transfer remain unchanged. HAL-370's first extended source version refuses linked requests; no G1 widening follows.

The admitted architecture reuses the same HAL-266A current-target preparation, HAL-295A private same-Queue/front publisher and HAL-263A closed preassociated Image ingress. It does not amend those parents, create a second publisher or change generic 2D/native-surface behavior. This S1 decision binds the selected contracts and prohibitions only: source implementation is NOT PERFORMED, S2 engineering authorization is NOT GRANTED, S3 producer/f32/Naga/Device/backend proof and S4 native/performance/JC acceptance remain OPEN. No default migration or public capability claim follows.

---

### 17.2 Binding HAL381 loading-presence presentation extension — 2026-11-06 [S1]

The supervising technical reviewer, under JC's existing technical delegation and retained constitutional/product veto, adopts [HAL381](SPEC-HAL-381-Source-Qualified-Loading-Presence-and-Materialization.md) as the following exact optional presentation exception. This is S1 law; implementation, executed GPU/native proof and physical JC acceptance remain separate.

Only for that named source-associated preview, §§8.5/9.3/10.3 permit its private fractional premultiplied Rgba16Float[N] proxy surface and opaque display-linear presentation. Canonical geometry/PBR materials, Mask, reverse-Z, R027 binary coverage/alpha k/N, source certificates and canonical Image bytes are unchanged. Sections9.1/9.2 admit a distinct PRIVATE Style64 group0/b0, semantic View80 group0/b1 and semantic Object112 group1/b0 layout with HAL381's triangle-expanded36-byte p/e01/e02 input and flat transformed geometric normal. Original shaders/layouts/packing remain unchanged.

Sections10.1/10.4/11.3–11.6 admit one Viewport-local source/request/Context/mount/view/target/N/physical-size/generation-qualified eligibility slot, one private one-texture Rgba16Float[1] writer and a real registered canonical-front generation content pin that blocks back-buffer reuse. Preview is not PBR Committed; canonical pair/publication semantics and compatible last-good backup retain their original truth. Pending/transition paints exactly ONE opaque private G/T Image. Terminal paints exactly ONE viewport-only CanonicalOverNeutral primitive reading actual matching F and emitting `(F.rgb+(1-F.alpha)*B,1)` BEFORE the inherited clip is applied ONCE. No separately clipped B rectangle or second visible Image is admitted; generic Image shader and canonical F are unchanged. Thus ideal output is `c*RGB+(1-c)*U` at every stage, including fractional c; actual FP16/filter/native endpoint compatibility requires executed evidence, not a scanout-equality assertion.

Section13.1 charges additional `P*(12*N+8)` attachment payload,108 bytes per expanded proxy triangle, actual aligned uniforms/buffers/pipelines/views/bindings/upload work and every current/old/pending/in-flight lifetime to the SAME view's limits and actual aggregate. No cap increase, feature demand, sample downgrade, silent source/provenance equivalence, general compositor/renderer/clock or semantic authority is added. Missing/refused preview preserves the independently lawful ordinary PBR loader/front. Only these exact HAL381 exceptions take precedence over the cited baseline clauses; all other authority remains controlling.

The 2026-11-07 Spectral Presence revision applies [HAL381 §13](SPEC-HAL-381-Source-Qualified-Loading-Presence-and-Materialization.md#13-spectral-presence--2026-11-07-adopted-appearance-revision) to this same private path. Its fixed Style64 appearance coordinates and bounded same-proxy neighbor stencil supersede the corresponding 2026-11-06 appearance clauses. The attachment, frame, source and custody exceptions above still delimit the path.

### 17.3 Binding HAL374 first selected-object Outline — 2026-11-06 [S1]

The supervising technical reviewer, under JC's existing technical delegation and retained veto, adopts ONLY [HAL374 §8](SPEC-HAL-374-Source-Qualified-3D-Selection-Outline-and-Halo.md)'s selected first profile, including the R32Float exact-tag correction. Sections8.5/9.3 and the existing private target/publication owners admit one source-qualified opaque-PBR complete replay into separate R32Float[N] occurrence tags and Depth32Float[N], followed by true projected-edge stroke evaluation. Canonical PBR positions/culling/Mask/material/normal/source laws, reverse-Z and R027's four planes remain unchanged. Unsupported Mask/legacy/discard parity refuses Outline alone. Slots are explicit caller-authored occurrences of the exact scene/snapshot/model, never semantic identity inferred from stable ordinal or a reused geometry handle.

Only that private edge pipeline may use source-consistent interpolated reverse-Z fragment depth against its private replay attachment, Greater with writes OFF; no bias, replacement source position or canonical wrapper/fragment-depth change is admitted. Its fractional premultiplied Rgba16Float[N] stroke uses the exact static2px distance/AA, same-member neighbor, selected-union suppression and MAX rule. One separately charged Rgba16Float[1] display-space stroke-OVER-F output is eligible only against real registered generation-content-pinned matching canonical F and exact source/request/Context/mount/view/target/N/size epochs. Paint exactly ONE viewport Image with inherited clip ONCE; generic Image, canonical F alpha k/N and HAL381 presentation remain unchanged. None selection bypasses the private output and preserves original F bitwise; private C is never PBR Committed.

Charge `P*(16*N+8)` EXTRA minimum plus actual CPU geometry/selection/view work, expanded edges/staging, aligned uniforms, programs/views/bindings and every queue-safe overlap under the SAME existing single/aggregate caps. No new Device feature, fallback/sample downgrade, scene writer, picking/selection authority, clock, worker or general compositor is authorized. Wrong/retired source/target/Device/capacity removes the effect without invalidating independently lawful original PBR. S2 source, GPU/native conformance and physical JC acceptance remain separately required; only these exact named exceptions supersede cited baseline exclusions.

### 17.4 Binding HAL375 first passive PBR HUD — 2026-11-07 [S1]

Under JC's technical delegation and retained veto the supervising technical reviewer adopts [HAL375 §9](SPEC-HAL-375-3D-Anchored-Spatial-HUD-and-Information-Connectors.md#9-adopted-first-passive-pbr-hud-profile--2026-11-07-s1). Sections 9–11 and 13 permit only its exact target-local canonical sampled-depth usage, private visibility lease and ordered optional passive label/leader output. Depth, source and registered front are one matching current association. The viewport's existing target/preparation, canonical Image and compatible last-good keep their ownership. Charge the actual HUD resources and concurrent generations under existing limits. R027 §6.6, HAL370 §8.3 and the named HAL359/360/HAL150 clauses delimit the private extension. No other target, shader, UI or product authority follows.

**[HAL375 §10](SPEC-HAL-375-3D-Anchored-Spatial-HUD-and-Information-Connectors.md#10-sole-binding-presentation-companion-for-the-passive-pbr-hud), sole-binding companion [S1]:** HAL375 §9's private visibility and ordered optional presentation use the sole-binding companion and frozen occurrence receipt under HAL266A §3.4 and INV-006/007A. Sections 5.3 and 15 apply unchanged, including through generic custom-render preparation. Inactive viewports and applications without 3D do no HUD work. The normal Image, canonical N1/N4 depth and front, and compatible last-good retain their existing owners.

## 18. Consistency and Reduction Audit

| Existing authority | Result |
|---|---|
| SPEC-HAL-001 extensibility | Preserved. 3D is an embedded extension with ID ownership, phase separation, snapshots, and no WidgetTree weakening. |
| SPEC-HAL-002 Constitution | Preserved. WidgetTree remains sole widget owner; semantic state is upstream; GPU work consumes immutable inputs; no hidden global or cross-phase UI mutation is introduced. |
| SPEC-HAL-021 generic animation | Preserved. For 3D inputs, Timeline advances once and sampling occurs only during Update; 3D consumes immutable owner revisions and has no animation clock, playback mode, or Timeline access. |
| SPEC-HAL-024 coordinate model | Preserved. The viewport boundary remains ordinary logical/physical/NDC ownership. World/local/view names are private render-description spaces and never become Widget layout or event spaces. |
| SPEC-HAL-040 GPU lifetime | Preserved as narrowed by SPEC-HAL-295A. Context resources are the admitted rebuildable exception; this SPEC explicitly rejects a false device-recovery claim. |
| SPEC-HAL-084 RenderGraph | Preserved as amended by SPEC-HAL-295A. 3D commands operate only on participant-owned offscreen resources; renderer-visible work is the ordinary Image composition node. |
| SPEC-HAL-085A shader suite | Preserved. Its renderer-owned 2D modules and `Globals` stay unchanged. This document is the new SPEC required for a separate admitted pipeline. |
| SPEC-HAL-150 compositor | Preserved. The compositor receives one authorized Image plane and ordinary 2D overlays; it owns no camera, depth, picking, or semantic correction. |
| SPEC-HAL-263 / 263A Image | Imported exactly. The 3D front is already-premultiplied linear when sampled, uses WHITE straight-linear tint, and receives no hidden conversion. |
| SPEC-HAL-266 / 266A DPR/preparation | Imported exactly. Allocation follows supplied physical render extent after layout; paint only emits Image; no global scale metadata is authority. |
| SPEC-HAL-295 / 295A custom render | Specialized by a new admitted producer without broadening renderer access, admission meaning, device lifetime, or publication ordering. |
| SPEC-HAL-343 spatial reconciliation | Preserved. Its 2D spatial subsystem remains bounded and is not generalized into matrix/perspective/3D authority. |
| SPEC-HAL-354 retained frames | Preserved. Complete structural/model/view revisions enter through update, unchanged views do no work, and changed fronts use the explicit SPEC-HAL-266A presentation result. |

Audit findings:

1. No existing binding SPEC required semantic amendment for adoption.
2. SPEC-HAL-295A and SPEC-HAL-266A explicitly decline to define 3D; those
   non-goals are ownership boundaries, not bans on a later dedicated SPEC.
3. SPEC-HAL-085A §10 requires a new SPEC for a new shader pipeline; this document
   supplies that separate authority without modifying the 2D shader contract.
4. SPEC-HAL-024's absolute UI-space law is not widened. 3D world/view math stays
   inside one admitted producer after the retained viewport boundary is fixed.
5. SPEC-HAL-040's aspirational general recovery prose does not create a live
   custom-resource epoch. The later, narrower SPEC-HAL-295A baseline lifetime is
   controlling for this participant.
6. Adoption adds reciprocal extension metadata to SPEC-HAL-295A,
   SPEC-HAL-263A, and SPEC-HAL-266A plus the new-pipeline backlink required by
   SPEC-HAL-085A §10. No parent normative body or semantic status changes.
7. The O(1) `has_custom_render_participants` branch already belongs to adopted
   SPEC-HAL-266A. This SPEC adds no 3D-specific condition to that generic seam
   and does not compare against pre-266A architecture.
8. `Context3D` may construct view-local resources from captured
   `GpuResources`, but only each per-view `CustomRenderPass` owns Image
   registration, unregistration, and retirement. No second publication owner was
   introduced.
9. WGSL library extensibility remains bounded without a textual module linker:
   application source is parsed independently and the controlled wrapper is
   added by Naga IR handle. Unjustified generic appearance parameters were
   removed; the fixed block sizes are 80/112/16.
10. Structural absence remains binding; timing thresholds become versioned
    measured benchmark policy rather than invented SPEC constants. Likewise,
    authority adoption is independent from per-backend implementation evidence,
    while every support claim remains proof-gated.
11. The SPEC-HAL-263A edge-alpha dependency is now explicit: component-wise
    color resolve yields coverage-weighted premultiplied samples, and the real
    target/publication/Image oracle must prove that claim on every supported
    backend/format.
12. Finite limits are context-local. A global context-count budget would require
    the forbidden registry, so aggregate context count remains explicit
    application policy rather than a false substrate governor.
13. The locked wgpu 25.0.2 / Naga 25.0.1 source audit and external probe
    establish a public-API-only controlled-composition path: parsed-source spans
    are retained, fixed IR nodes can be appended and laid out, the full module
    validates, direct Naga consumption type-checks, emitted WGSL round-trips, and
    texture/sampler helper parameters are legal. This supports retaining the
    bounded programmable appearance ABI. It does not authorize a general linker
    or exempt future compiler upgrades from the retained adapter proof.
14. Structural resource truth and high-frequency presentation truth are now
    separate immutable domains. This preserves flat render intent and leases
    while preventing model/view animation from rebuilding persistent resources,
    GPU buffers/bind groups, or per-packet structural state each frame.

---

## 19. Non-Normative Implementation Decomposition

This section is guidance only. It follows the contract and cannot redefine it.

The recommended dependency order is:

1. **Owned checked math values:** role-based points/vectors, quaternion, TRS,
   projections, render-origin conversion, reverse-Z matrices, `Ray3`, and pure
   intersections.
2. **Context and finite policy:** explicit construction from `GpuResources`,
   ContextId, effective budgets, typed errors, resource slots, generations, and
   leases, with zero GPU allocation at context creation.
3. **Immutable content:** canonical vertex packing/validation, index buffers,
   semantic presence, appearances, flat structural render snapshots, complete
   immutable model-state values, independent monotonic revisions, exact
   structural/model identity checks, and deterministic packet ordering.
4. **Appearance admission:** standalone WGSL library parsing, a version-locked
   public-API Naga adapter with retained upgrade probe, handle-based IR
   composition, exact allowlist, 80/112/16 host layouts, fallback
   texture/sampler, pipeline keys, and negative tests.
5. **Per-view GPU target and dynamic storage:** sample-1/sample-4 color/depth
   creation, reverse-Z pipeline, fixed vertex stage, direct resolve, persistent
   structural bindings, finite queue-safe reusable view/object storage, and
   actual-target raster/depth oracles.
6. **Retained viewport:** explicit context injection, SPEC-HAL-295A admission,
   SPEC-HAL-266A preparation, requested/committed state, last-good publication,
   Image-only paint, and removal/retirement.
7. **Proof closure:** same-frame 60/120 Hz animation feed, warm stable resources
   and bounded allocation, concurrent camera/model atomicity, multi-view/context
   isolation, last-good/failure/device loss, alpha/Image proof, DPR and resize,
   zero-view structural audit, native backend matrix, and release A/B measurement.
8. **Browser vertical:** only after native authority is implemented and verified.

An internal math dependency may be considered separately during implementation,
but it cannot appear in the public API and this SPEC does not authorize adding
glam, nalgebra, or any other dependency.

Implementation may organize these owners into modules that can later be split by
amendment. It must not split authority prematurely, implement later steps around
failed earlier invariants, or treat green code as permission to change this
contract.

---

## 20. Adoption Record and Implementation Gate

Adopted on 2026-08-21 after independent source, authority, reduction, and
compiler-feasibility review ratified the fixed public precision, geometry,
appearance, depth, lifecycle, budget, and portability profiles as one coherent
baseline.

Adoption performed exactly these authority changes:

1. changed SPEC-HAL-355 from Draft to Adopted / Binding;
2. added extension metadata only to SPEC-HAL-295A, SPEC-HAL-263A, and
   SPEC-HAL-266A;
3. added the separate-pipeline backlink required by SPEC-HAL-085A §10 without
   changing the renderer-owned 2D shader suite;
4. preserved every parent normative body and ownership boundary without literal
   reinterpretation; and
5. retained implementation status and every backend row separately, leaving
   every unproved row non-conforming regardless of architectural adoption.

No Rust implementation, dependency addition, or backend support claim is part of
this adoption. Implementation must proceed as independently verified units under
this document, which remains the single baseline substrate authority until an
independently justified amendment splits a section.

### 20.1 2026-08-23 Binding Amendment

The 2026-08-23 amendment:

1. separated immutable structural `RenderSnapshot` identity/topology/resources/
   leases from complete immutable flat model-state value/revision;
2. made structural/model/view/target state one atomic requested/committed tuple;
3. required finite queue-safe reusable per-target view/object dynamic storage and
   post-warm-up reuse of GPU buffers, bind groups, materializations, pipelines,
   and packet metadata;
4. extended transactionality, last-good, multi-surface isolation, zero-work,
   finite budgets, and counters to the dynamic path;
5. added same-frame SPEC-HAL-021/354/266A and sustained 60/120 Hz proof without
   creating a 3D-specific animation system; and
6. left §16.4 item 26 unchanged and open as the predeclared release A/B
   measurement policy rather than inventing a threshold.

This amendment changes binding authority only. It contains no Rust implementation,
dependency, backend support claim, or conformance claim.

### 20.2 2026-08-28 Binding Extension Adjacency

This documentation amendment records adopted SPEC-HAL-357 as a binding optional
producer extension and states the only compatible reduction: every admitted
domain-composition path lowers before the existing flat snapshot/model renderer
boundary. The extension is compatible only under these additional binding
constraints:

- spatial co-presence, equal coordinates, equal embeddings, overlap, and
  domain/world/domain conversion imply no semantic relation or propagation;
- mechanical committed-domain currentness carries no consumer semantic revision
  and is insufficient for semantic publication without the consumer's separate
  immutable association check;
- an atomic domain frame is mechanically complete for one shared-depth front,
  not a semantic transaction or sibling-dependency claim;
- domain point/vector roles are distinct named f64 mechanical roles
  subordinate to §6.1, including preservation of the Planar2 signed-normal vector
  component rather than silent projection;
- every admitted domain composition/state/frame identity and its metadata bytes
  are charged to finite, queryable context-local aggregate budgets for the full
  requested/committed/last-good/in-flight lease lifetime; and
- ordinary 2D and zero-3D paths remain structurally untouched, while a flat
  Viewport3D may perform at most one bounded O(1) participant-owned request-kind/
  metadata-absence discrimination and zero domain lowering, allocation,
  traversal, resource, or GPU work.

Every committed domain association inherits §11.4 literally: an active
compatible `FailedWithLastGood` continues to answer from the old committed tuple
while failed requested state remains distinct. It is not a separate lifecycle
rule.

This record reciprocally reconciles SPEC-HAL-355 with SPEC-HAL-357 adoption. It
authorizes no renderer-side domain state and changes no existing runtime,
ownership, lifecycle, resource, raster, publication, or proof rule. Live
implementation and conformance remain independently evidenced.
