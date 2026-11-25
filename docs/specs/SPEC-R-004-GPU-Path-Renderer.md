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
