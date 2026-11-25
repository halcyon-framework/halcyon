# **SPEC-R-003: GPU Path Rendering Primer**

**Author:** JC
**Date:** 2025-11-24
**Status:** Stage 2A Implemented
**Scope:** Conceptual boundaries for future vector primitives
**Purpose:** Define what Halcyon *may* support regarding shapes, paths, and GPU vector rendering — without implementing full path rendering.
**Extended by:** SPEC-HAL-358 (Adopted / Binding; explicit bounded luminous
line/polyline appearance in the existing analytic shape pass)

---

## Implementation Status

**Stage 1 Complete** (Scene/Batcher Scaffold)

Stage 1 implements the structural foundation for GPU path rendering:

* **SceneNode variants**: `Circle`, `Ellipse`, `Line`, `Polyline` added to scene graph
* **Batcher integration**: `add_circle/ellipse/line/polyline` methods with proper clip handling
* **PrimitiveData variants**: Shape data structures ready for geometry generation
* **PipelineId::Shape**: Batch key support for shape primitives
* **Clip metadata**: All shapes carry correct `clip_rect` from batcher stack

**Stage 2A Complete** (Shader/Pipeline Contract)

Stage 2A activates the constrained analytic SDF pipeline contract:

* **`shape.wgsl` shader**: circle, ellipse, line segment, and polyline segment distance fields
* **`ShapeVertex` format**: fixed locations 0-5 for position, local position, color, params, and shape type
* **`ShapePipeline::new`**: creates the wgpu render pipeline using renderer globals at bind group 0
* **Premultiplied output**: SDF coverage modulates alpha before premultiplied color output

**Not yet implemented (Stage 2B+):**
* `WgpuRenderer` draw submission through `ShapePipeline`
* Vertex/index buffer generation for `ShapePipeline`
* Batch routing from shape primitives into the active shape pipeline

---

## 1. Problem Statement

Halcyon is a desktop UI framework.
Trading UI requires crisp geometry:

* rulers
* cursors
* markers
* arrows
* keylines
* separator lines
* simple shapes (rounded rects, circles)

But **full vector/path rendering** (Bezier curves, SVG, arbitrary strokes) is dangerous:

* huge shader complexity
* massive CPU/GPU cost
* batching fragmentation
* high R&D burden
* unnecessary for trading UIs

We must define *exactly what is allowed*, so Halcyon does not accidentally evolve into Skia, Cairo, or Pathfinder.

This SPEC establishes the "safe vector subset."

---

## 2. Goals

1. Define a **minimal vector primitive model** for Halcyon.
2. Specify future-proof GPU shape support without committing to full path rendering.
3. Enable required trading shapes (crosshairs, arrows, simple curves).
4. Avoid the Skia vortex (too big, too complex).
5. Keep batching simple and efficient.

---

## 3. Non-Goals

* No SVG support
* No bezier editor
* No arbitrary shape stroking
* No miter joins / bevel joins / stroke expansions
* No tessellators
* No path boolean operations
* No gradient meshes

Halcyon is not a vector art library.

---

## 4. Allowed Shape Primitives

Halcyon may support **ONLY these shapes**:

### 4.1 Rect

CPU: trivial
GPU: one quad
Shader: solid or gradient

### 4.2 Rounded Rect

Already defined in clipping SPEC.
Rounded corners only.
Uniform radius or per-corner config.

### 4.3 Line Segment

* 1px or n-px thick
* Antialiasing via shader
* Axis-aligned or arbitrary angle

### 4.4 Polyline (limited)

A simple sequence of segments:

* No joints except round/bevel
* No self-intersections
* No thickness > N

Used for:

* chart trendlines
* drawing tools

Implementation can be GPU quad strip, not full path tessellation.

### 4.5 Circle / Ellipse

Shader-based distance field:

* Cheap
* Scalable
* Works with batching

### 4.6 Simple Arc (no bezier)

Just a circle segment:

* For cursors
* Chart markers
* Radial elements

Arcs are drawn as masked circle fragments.

---

## 5. Forbidden Shapes (Forever)

These features are explicitly banned:

* Bézier curves (quadratic or cubic)
* Arbitrary paths
* SVG path parsing
* Path boolean ops
* Complex stroking
* Polygon fills
* Tessellation pipelines
* MSDF text
* GPU-accelerated drawing languages

If someone wants these, they must integrate Skia externally.
Halcyon remains simple.

---

## 6. Rendering Model

### 6.1 Fragment-based Distance Field Shapes

Complex shapes (circle, ellipse, arc) rendered via:

* analytic distance
* smooth step AA
* cheap shader ops

Advantages:

* No geometry expansion
* No tessellation
* One pipeline
* Fully batchable

### 6.2 Quad-based Polyline

For polylines:

* CPU produces quad strips
* GPU handles endcaps + AA
* Minimal math
* Stable performance

### 6.3 Stroke Thickness Rules

Max guaranteed performance thickness: **<= 4px**
Higher thickness allowed, but AA precision decreases.

### 6.4 Luminous Contour Extension

SPEC-HAL-358 admits one explicit luminous appearance for checked line/polyline
contours. It remains inside the constrained analytic segment model, uses the
existing shape pass, and does not authorize arbitrary path effects, blur,
bloom, tessellation, inferred topology, or consumer temporal semantics.

---

## 7. Integration With Existing Pipelines

### 7.1 Scene Graph Nodes

Introduce new node types:

* `CircleNode`
* `EllipseNode`
* `LineNode`
* `PolylineNode`

These map to existing `PrimitiveNode` categories.

### 7.2 Batching

Batch key for shapes includes only:

* pipeline_id
* texture_id (none)
* clip_hash
* shape_type_id

Vector shapes batch extremely well.

### 7.3 WGSL Additions

* Distance-field helpers
* Polyline shader
* AA smoothing

No multi-pass.

SPEC-HAL-358 front emphasis is continuous contour-local shader data inside this
same pass, not another pass or a marker primitive. Candidate segment bounds use
documented complementary ownership planes so an ordinary adjacent join has one
coverage owner rather than repeated premultiplied-OVER accumulation.

---

## 8. Performance Considerations

* All primitives are **O(1) per fragment**, zero tessellation cost.
* No CPU triangulation.
* GPU does all shape logic via analytic math.
* Perfect for data-heavy UIs where shapes update on every frame.

Worst-case cost:
20,000 polylines × 4 segments each = fine at 240 FPS on any modern GPU.

---

## 9. Use Cases for Halcyon

### Required for Trading UIs

* Candle outlines
* VWAP lines
* Trendlines
* Fib arcs
* Indicators
* Markers
* Selection rectangles

### Required for UI

* Scrollbar corners
* Rounded panels
* Focus ring
* Toggle circles
* Dial widgets
* Pie chart slices (future?)

This SPEC gives permission to support these without expanding scope.

---

## 10. Claude Code Instructions

When implementing at later stages, Claude must:

1. Introduce new vector node types (CircleNode, LineNode, etc.).
2. Add analytic shape WGSL functions.
3. Add new pipelines or merge into unified shape pipeline.
4. Add batched rendering support.
5. Add helper constructors for widgets to use.
6. Add minimal example in `/examples/shape_primitives.rs`.

Absolutely no tessellation or SVG.
No bezier stroking.
