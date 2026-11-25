# **SPEC-R-004 — GPU Path Renderer (MSAA Vector Pipeline v1)**

**Author:** Halcyon Architect
**Status:** Implemented (Stage 20 Complete)
**Version:** 1.0

**Depends On:**

* SPEC-R-001 Clip Stack Rework
* SPEC-R-002 GPU Clip Shaders & Rounded Masks
* SPEC-R-003 GPU Path Rendering Primer
* SPEC-HAL-024 Unified Coordinate Model
* SPEC-HAL-066 Unified Glyph Raster Pipeline (for atlas reuse)

---

## Implementation Status

**Implemented** — All 20 stages complete.

---

**Implements:**
A complete GPU-accelerated vector path renderer for Halcyon:

1. CPU path flattening
2. GPU MSAA-based coverage accumulation
3. Per-geometry instance pipeline
4. GPU-side fill & stroke evaluation
5. Integrated clip stack
6. Device-scale independent vector rendering
7. Subpixel-accurate anti-aliasing

This establishes the **first stable vector graphics pipeline** in Halcyon.

---

## **1. Motivation**

Halcyon currently lacks a canonical path renderer.

We have:

* clipping stack work (R-001, R-002)
* theoretical basis (R-003)
* strong coordinate model (HAL-024)

…but no actual **GPU vector pipeline**.

This SPEC provides the missing piece:
**MSAA + analytic coverage** vector rendering that is:

* fast
* predictable
* independent of raster pixel density
* compatible with Halcyon’s layout/paint architecture

---

## **2. Goals**

### **2.1 Primary**

* Correct, resolution-independent vector rendering
* Support for:

    * MoveTo / LineTo / QuadTo / CubicTo
    * Closed paths
    * Even-odd and non-zero winding rules
    * Strokes with joins and caps
* Analytic AA (not texture tricks)
* Clip integration via R-001 / R-002
* Stable under resizing and fractional transforms

### **2.2 Secondary**

* Minimal CPU work
* Per-path GPU instancing
* Internal batching with Shape IDs
* Pre-cached stroke geometry

### **2.3 Deferred (future R-series)**

* Distance fields
* GPU tessellation shaders
* Hardware conservative rasterization
* Gradients
* Dashed strokes
* Path effects (blur, shadow)

---

## **3. Architecture Overview**

```
CPU Path
   ↓
Path Flattening (tessellation)
   ↓
Stroke Builder (optional)
   ↓
MSAA Fill Shader (analytic coverage)
   ↓
Clip Stack Apply (R-001)
   ↓
WGPU RenderPipeline (triangle list)
   ↓
Framebuffer (resolve → present)
```

---

## **4. CPU Path Flattening**

### **4.1 Input**

```
Vec<PathCommand> {
    MoveTo(x,y)
    LineTo(x,y)
    QuadTo(cx,cy,x,y)
    CubicTo(c1x,c1y,c2x,c2y,x,y)
    Close
}
```

### **4.2 Output**

A monotonic triangle mesh:

```
struct PathMesh {
    vertices: Vec<Vec2>,
    indices: Vec<u32>,
    bounds: RectF,
}
```

### **4.3 Requirements**

* Flatten quads/cubics with adaptive subdivision
* Error tolerance: **0.25 logical px**
* Monotonic subpaths
* Guaranteed CCW orientation
* Closed paths return to start
* Stable triangulation regardless of shape complexity

Halcyon uses **earcut** or monotone triangulation (choice is implementer’s for now).

---

## **5. Strokes (Optional Geometry Stage)**

### **5.1 Stroke Builder**

```
StrokeParams {
    width: f32,
    join: Miter | Round | Bevel,
    cap: Butt | Round | Square,
}
```

Output is a **second mesh**:

```
StrokeMesh { vertices, indices }
```

### **5.2 Miter Limit Default**

```
miter_limit = 4.0
```

Consistent across platforms.

---

## **6. GPU Fill Pipeline (MSAA)**

### **6.1 Vertex Format**

```
struct PathVertex {
    position: vec2<f32>
}
```

### **6.2 Instance Format**

```
struct PathInstance {
    color: vec4<f32>
    transform: mat3x2<f32>
    clip_id: u32
    stroke: u32  // 0 = fill, 1 = stroke
}
```

### **6.3 MSAA Shader Overview**

The fragment shader computes analytic coverage:

```
coverage = compute_analytic_coverage(pixel, edges);
output = color * coverage;
```

* No texture sampling
* Edge distance computed in shader
* Subpixel-accurate AA
* Stable under fractional transforms

### **6.4 Blending**

Standard non-premultiplied (subject to HAL’s color pipeline SPEC).

---

## **7. Clip Stack Integration**

R-001 and R-002 define:

* GPU clip masks
* rectangular and rounded clips
* nested clip layers

Path rendering obeys the same pipeline:

```
instance.clip_id → clip shader → mask
```

Masks are applied before coverage evaluation.

---

## **8. Renderer Integration**

### **8.1 SceneGraph Node**

```
enum SceneNode {
    ...
    Path {
        mesh_id,
        instance,
    }
}
```

### **8.2 Batch Model**

* Group by:

    * pipeline
    * clip_id
    * color
    * stroke flag
* Reuse buffers for many small paths
* No more per-path pipelines

### **8.3 Draw Pass Order**

1. Background rects
2. Paths
3. Text (can overlap via clip)
4. UI overlay

---

## **9. Performance Guarantees**

### **9.1 CPU**

* Flattening: O(n) segments
* Tessellation: O(n log n) worst-case
* Stroke expansion: O(n)

### **9.2 GPU**

* All triangles, zero branching
* MSAA 4× baseline, 8× configurable
* No per-pixel loops
* No geometry shader dependency

---

## **10. Testing Matrix**

### **10.1 Geometric Correctness**

* 100+ canonical shapes
* Sharp angles
* Rounded joins
* Degenerate curves
* Holes (donut test)

### **10.2 AA Stability**

* Fractional transforms
* DPI scaling
* Animated transforms
* Rotate/scale shear combos

### **10.3 Clip Stack**

* Nested clips
* Combined rounded/rect clips
* Clip inside stroke

---

## **11. Implementation Plan**

### **Phase 1 — CPU Path Types**

* Implement PathCommand
* Implement PathBuilder
* Implement flattening

### **Phase 2 — Triangulation**

* Monotone subdivision
* Earcut fallback

### **Phase 3 — Stroke Builder**

* Join/cap logic
* Miter limits

### **Phase 4 — GPU Pipeline**

* MSAA render pipeline
* Vertex/instance buffers
* WGSL coverage shader

### **Phase 5 — Renderer Integration**

* Add SceneNode::Path
* Integrate clip stack
* Integrate batching

### **Phase 6 — Debug Tools**

* Path visualization layer
* GPU debug overlays

---

## **12. Implementation Notes**

* Avoid dynamic tesselation on GPU (unreliable across vendors)
* Avoid stencil-buffer vector fills (slow, synchronization-heavy)
* Avoid signed-distance fields in this SPEC (part of future R-series)

## 13. Compound fill correction (2026-11-25)

**Scope:** Correct the implemented Path fill mechanism under §§2.1, 4 and 10.1.
Implemented and independently accepted in 0.271.0-beta. Focused geometric,
cache, retained-presentation, diagnostic and registry checks passed, together
with native Metal and browser WebGPU pixel proof at 1×, 2× and 1.25×. This
does not establish acceptance in every external application or on every
platform. This does not adopt SPEC-HAL-264, HAL-265 or HAL-265A as a whole or
change their declared statuses. Preserve the implemented scene, cache, clip,
stroke, retained fragment and ordered draw mechanisms under their existing
authority.

### 13.1 Geometry and rule ownership

- INV-R004-COMPOUND: One filled Path is one signed set of contours. Preserve
  every MoveTo boundary and the authored winding through flattening. Fill
  closes each contour implicitly; explicit Close remains distinct for the
  existing stroke builder. Never connect the end of one contour to another
  contour's start. Repeated/zero-length edges do not add coverage. Empty or
  geometrically zero-area filled sets are legitimate empty output.
- INV-R004-RULE: The renderer-owned `path::FillRule` is the single typed rule
  with NonZero and EvenOdd, deriving Copy, Clone, Debug, Default (NonZero),
  Eq, PartialEq and Hash. Re-export that same type from `core::icon::FillRule`
  so existing icon imports and construction remain valid. A point is filled
  iff the signed winding sum is nonzero, or its crossing parity is odd,
  respectively. Do not independently reorient the source contours.
- Preserve the public `Path { commands }`, PathCommand variants, builders,
  `flatten()` and existing `to_mesh()` signature. The flat-point compatibility
  query is not a fill tessellation input. Add
  `Path::try_to_mesh_with_fill_rule(&self, rule: FillRule) -> Result<PathMesh, PathFillError>`.
  Existing `to_mesh()` uses NonZero. PathMesh remains the existing vertices,
  indices and bounds representation; its triangle ordering is not an API
  guarantee.
- INV-R004-ONE-COVERAGE: Tessellate all contours together into one triangle-list
  mesh with disjoint triangle interiors covering exactly the admitted filled
  set, including disconnected components, nested holes and islands, overlaps,
  opposite-winding cancellation, coincident edges, touching contours, and
  self-intersections. Only output triangles are normalized to the existing
  renderer's CCW convention. No per-contour alpha composition, artificial
  bridge, eraser polygon, convex hull, unconditional fan or partial fallback.
  Transparent paint blends once per covered sample, not once per contour.

### 13.2 CPU tessellation and target projection

Use the Rust registry dependency `lyon_tessellation = "=1.0.22"` solely as the
renderer-private complex fill sweep/monotone tessellator (FillTessellator,
explicit FillOptions rule, intersections enabled, u32 indices). Its path
re-export is sufficient; no second drawing engine or GPU pipeline. Qualify
the exact newly resolved dependency closure and licenses before acceptance;
do not update unrelated packages. The dependency's types do not cross the
Halcyon API. This is the scoped implementation selection permitted by §4.3.

Adapt the existing command stream into closed, separately identified flattened
contours in the current logical coordinate space at the existing 0.25 logical
pixel tolerance. Use the current adaptive flattening math for admitted curves,
preserving contour boundaries and finite progress; do not apply the stroke
builder's near-point removal policy to fill winding. Exact consecutive
duplicates and duplicate closure points may be removed. Check command grammar,
finite coordinates and finite subdivision/output before unsafe arithmetic or
tessellation. Reject unrepresentable or non-progressing subdivision instead of
recursing indefinitely. Do not discard a self-intersecting contour merely
because its signed area is zero.

For cached fills, apply the existing Center/Edge/None projection to flattened
source contour points once at the actual positive finite effective DPR, before
the compound tessellation. Use the existing rounding formulas; new intersection
vertices are generated from those projected contours and are not independently
rounded afterward. This prevents post-mesh snapping from crossing or
duplicating coverage. Without snap, tessellate the original flattened contours.
Recompute bounds from the emitted mesh. Existing true stroke expansion and
explicit precomputed mesh entrypoints retain their behavior.

A checked fill result distinguishes InvalidSource, InvalidScale,
TessellationFailed and InvalidMesh using Halcyon-owned typed errors
(`PathFillError`). Validate finite vertices, valid triangle indices, winding
and nondegenerate emitted triangles. An empty admitted fill is success, not a
failure. The old infallible mesh helpers retain their signatures, returning an
empty mesh with an ordinary error diagnostic on actual refusal; never fabricate
coverage. Batcher uses the checked result, records the failure through existing
ordinary renderer diagnostics, emits no failed path and does not cache a failed
result. The old simple-polygon index helper is not a production fill fallback.

The §9.1 tessellation bound is interpreted for the simple/nonintersecting
case; a general intersecting input can itself induce quadratic output size.
Do not promise O(n log n) worst-case independent of intersection/output
complexity, or implement a quadratic all-pairs preprocessing pass on ordinary
paths. Existing renderer resource limits remain in force.

### 13.3 Scene and cache transport

- INV-R004-TRANSPORT: Add the explicit typed `fill_rule` to PathNode. Existing
  SceneBuilder::push_path and debug paths select NonZero; preserve all existing
  signatures. Add `SceneBuilder::push_path_with_fill_rule(path, rule, color,
  z_index, key)` using the existing Path node. In PaintSink expose the same
  operation returning `Result<(), PathFillError>`: a default implementation
  forwards NonZero to the existing push_path and returns UnsupportedSink for
  EvenOdd; the ordinary SceneBuilder implementation admits both. The result
  denotes scene emission, not later GPU acceptance. No silent rule substitution.
  Existing custom PaintSink implementations remain source-compatible.
- SceneBuilder::push_icon emits each layer through this same uncached PathNode
  construction with that exact IconLayer.fill_rule, existing scaled path,
  existing resolved color and position, z-index and key policy. Do not change
  icon definitions or split a layer into independent draws.
- INV-R004-CACHE: Extend the existing MeshCacheKey with the typed fill_rule and
  exact effective DPR bits in addition to existing command hash, dimensions,
  stroke/fill, quantized DPR and snap mode. The existing renderer-owned lookup,
  lifetime, eviction and color-independent geometry reuse remain authoritative.
  Equal commands with different rules or distinct exact DPRs must not share
  an entry, including when the quantized DPR is equal. Cache success only.
  Preserve supplied meshes and their existing bypass; do not use that bypass
  for ordinary fills/icons. Existing retained fragment identity and keys remain
  unchanged; the rule is immutable paint payload and follows every ordinary
  clone/recomposition, with paint invalidation on changed authored values.
  No additional registry, global cache, widget identity, scene variant or
  feature switch.

### 13.4 Required proof and compatibility

Prove the ordinary Path and actual registered CHECK route through SceneBuilder,
cache, DrawPlan, Compile and Execute. Include an independent winding/parity
oracle for safe interior samples, non-overlapping triangle interiors, exact
gap/hole and overlap samples, both winding orientations, disjoint contours,
same/opposite coincident contours, nested island, bow-tie, touching and
collinear edges, curves, degenerate empty output and rejected invalid source.
Test different fill rules over identical command content and same-quantized
distinct DPRs, warm cache hits and retained repaint/recomposition. Use
translucent fill with known background to distinguish one blend from two.
Verify real GPU output of the unchanged CHECK arms and empty wedge, both fill
rules and representative existing simple paths at 1x, 2x and fractional scale,
including native and browser WebGPU. Preserve ordinary clipping, strict Z,
existing stroke joins/caps and public examples. Reuse valid evidence; no broad
unrelated renderer changes. This is the previously recorded PM-006 compound
fill pathology, not a reason to redraw individual icons.

The additive rule API and changed public low-level PathNode/MeshCacheKey
struct-literal shape require a zero-major minor release classification.
Target version 0.271.0-beta, with explicit compatibility notes in the
changelog. Keep the initial public 0.270.3-beta section and archived history
intact. Do not claim external application acceptance from framework-only proof.
