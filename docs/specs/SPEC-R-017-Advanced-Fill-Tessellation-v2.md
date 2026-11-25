# **SPEC-R-017 — Advanced Fill Tessellation v2 (Curves, Self-Intersections, GPU Earcut/Monotone Hybrid)**

**SPEC-R-017**
**Advanced Fill Tessellation v2**
**Author:** Halcyon Architect
**Status:** Deferred — After gradient pipeline (R-012-v2)
**Version:** 1.0

**Depends On:**

* SPEC-R-001 Clip Stack Rework
* SPEC-R-003 GPU Path Rendering Primer
* SPEC-R-006 Path Flattening v2
* SPEC-R-010 Texture Filters & Pixel Grid Control
* SPEC-R-011 Blend Modes & Composite Operators
* SPEC-R-013 Path Boolean Ops v1
* SPEC-R-016 Stroke Tessellation v2

---

## Implementation Status

**Not implemented**

Advanced fill tessellation is not implemented.

---

**Implements:**
A robust, GPU-accelerated fill tessellator supporting:

* convex & concave shapes
* self-intersecting paths
* arbitrary Bézier curves
* holes
* multiple subpaths
* SVG-style fill rules
* high-zoom stability
* perfect interaction with stroke & clip pipelines
* fast incremental re-tessellation

This replaces the prototype CPU earcut and CPU monotone triangulators.

---

# **1. Motivation**

Halcyon’s fill pipeline is still limited:

* CPU earcut → slow, not parallel
* no stable handling for self-intersections
* non-optimal paths for complex Béziers
* no GPU pipeline capable of incremental updates
* no unified fill + stroke consistency guarantee

Modern UIs require:

* vector icons
* SVG imports
* animated vector shapes
* UI transitions based on geometry
* interactive clipping based on paths
* smooth high-DPI zoom without pixellation

This SPEC introduces a **hybrid**, **GPU-driven** fill tessellator.

---

# **2. Hybrid Pipeline Overview**

Two subsystems:

## **2.1 GPU Monotone Tessellator (Primary)**

Fast, stable, ideal for:

* most UI shapes
* rounded rectangles
* simple icons
* monotone segments

## **2.2 GPU Earcut Tessellator (Fallback)**

Used only when:

* path has holes
* path is self-intersecting
* monotonic decomposition fails
* degenerate polygons appear

Tessellator chooses the path per subpath.

---

# **3. Unified Fill Task Descriptor**

```
pub struct FillTask {
    pub path: Arc<Path>,
    pub color: Color,
    pub fill_rule: FillRule, // NonZero or EvenOdd
}
```

Path may contain:

* multiple subpaths
* holes
* self-intersections

Fill rule fully determines polygon area resolution.

---

# **4. Fill Rules**

Halcyon supports both:

```
NonZero
EvenOdd
```

Applied inside compute shader:

```
winding = compute_winding_number(point)
if fill_rule == EvenOdd:
    filled = winding % 2 != 0
else:
    filled = winding != 0
```

Earcut fallback honors fill rule for polygon splitting.

---

# **5. GPU Pipeline**

## **5.1 Stage 1 — Path Flattening (Compute Shader)**

From SPEC-R-006.

Outputs:

* polyline points
* segment normals
* Bézier approximation metrics
* bounding data

## **5.2 Stage 2 — Monotone Subdivision**

Compute shader:

* splits flattened polyline into y-monotone regions
* detects degeneracy
* if too complex → marks region for earcut fallback

Output:

```
Vec<MonotoneRegion>
Vec<ComplexRegion> // for earcut
```

## **5.3 Stage 3 — Monotone Tessellation**

For each monotone region:

* fan between left & right chains
* produce triangles
* compute per-vertex UV for future features (gradients, patterns)

## **5.4 Stage 4 — Earcut Tessellation (Fallback)**

GPU earcut implementation:

* iterative stack-based Earcut
* supports holes
* supports self-intersections (after Boolean cleanup)
* fully parallelized

## **5.5 Stage 5 — Mesh Assembly**

Merged vertex/index buffers emitted:

```
FillMesh {
    vertices: Buffer,
    indices: Buffer,
    bounds: RectF,
}
```

Can be cached and reused.

---

# **6. Handling Self-Intersections**

Uses SPEC-R-013 Boolean Ops:

1. **Self-intersection detection**
   Sweep-line algorithm in compute shader.

2. **Contour splitting**
   Subdivide path into simple polygons.

3. **Polygon classification**
   Use fill rule to assign polarity.

4. **Send simple polygons to tessellation**
   Either monotone or earcut depending on shape.

---

# **7. Animated & Dynamic Paths**

The pipeline supports:

* changing control points
* animated Bézier curves
* morphing icons
* shape-based transitions
* procedural geometry

Partial re-tessellation is possible:

* detect which segments changed
* re-flatten only affected regions
* re-tessellate only related polygons
* merge into cached mesh

---

# **8. Clip Stack Integration**

Fills participate normally:

* clipped by R-001 SDF clips
* clipped by R-002 round masks
* clipped by path clips (R-003 optional extension)

Tessellation always occurs before clipping.

---

# **9. High-Zoom Stability**

All output vertices are in **logical space**.

Fragment shader applies:

* 1-pixel AA envelope
* no “swimming edges” as geometry zooms
* no artifacts from tessellation under scale

---

# **10. Testing Matrix**

### Paths

* convex
* concave
* star shapes
* circles/ellipses
* multiple holes
* self-intersections
* degenerate shapes

### Fill Rules

* NonZero
* EvenOdd

### Rendering

* AA quality
* clip correctness
* join behavior for self-intersection correction

### Dynamic

* animated paths
* morphing icons at 1–4× scale

---

# **11. Implementation Plan**

## **Phase 1 — Path Flattening v2 Integration**

* integrate SPEC-R-006
* compute metrics
* buffer layout

## **Phase 2 — GPU Monotone Pipeline**

* region detection
* subdivision
* fan tessellation

## **Phase 3 — GPU Earcut Fallback**

* full earcut implementation
* support for holes
* integration with fill rule

## **Phase 4 — Self-Intersection Resolution**

* sweep-line
* region splitting
* Boolean op integration

## **Phase 5 — Renderer Integration**

* FillPass
* caching
* partial updates

## **Phase 6 — Optimization**

* simplify output topology
* Z-order batching for multiple fills
* reduce overdraw

---

# **12. Future Work**

* pattern/gradient fill shaders
* signed-distance MSDF fill mode
* 3D extrusion of 2D paths
* perfect SVG compatibility layer
