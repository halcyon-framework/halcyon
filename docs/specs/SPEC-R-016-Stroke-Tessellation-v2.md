# **SPEC-R-016 — Stroke Tessellation v2 (Hybrid GPU Tessellation + Analytic SDF Fallback)**

**SPEC-R-016**
**Stroke Tessellation v2**
**Author:** Halcyon Architect
**Status:** Deferred — After core visual polish (R-013-v2, R-012-v2, R-015-v2)
**Version:** 1.0

**Depends On:**

* SPEC-R-001 Clip Stack Rework
* SPEC-R-002 GPU Clip Shaders & Rounded Masks
* SPEC-R-003 GPU Path Rendering Primer
* SPEC-R-006 Path Flattening v2
* SPEC-R-010 Texture Filters & Pixel Grid Control
* SPEC-R-011 Blend Modes & Composite Operators
* SPEC-R-015 GPU Border Effects v2

---

## Implementation Status

**Not implemented**

Stroke tessellation v2 is not implemented.

---

**Implements:**
A hybrid stroke-rendering pipeline enabling:

* analytic-SDF strokes for simple shapes
* GPU tessellated vector strokes for arbitrary paths
* high-performance joins/caps
* infinite zoom stability
* perfect consistency with fill SDFs
* future filled-stroke effects (gradients, animated strokes, textured strokes)

This replaces the previous tessellator prototype and unifies Halcyon's stroke rendering across the board.

---

# **1. Motivation**

Current Halcyon stroke rendering is:

* CPU-side
* limited to simple shapes
* incapable of high zoom stability
* incapable of arbitrary paths
* not consistent with the fill SDF pipeline
* not compatible with dash patterns from SPEC-R-015

Modern UI engines support both approaches:

1. **SDF strokes** for standard UI primitives
2. **GPU tessellated strokes** for paths, SVG-like content, icons, curves

This SPEC integrates both into one system.

---

# **2. Two Rendering Modes**

## **2.1 Analytic SDF Stroke Mode**

Used for:

* rectangles
* rounded rectangles
* circles
* ellipses
* thin strokes
* any stroke using analytic geometry

Benefits:

* pixel-perfect AA
* infinite zoom
* constant memory
* extremely fast
* ideal for UI widgets

## **2.2 Tessellated Stroke Mode**

Used for:

* arbitrary bezier paths
* non-convex geometry
* strokes with extreme variations
* dash patterns on irregular curves
* custom vector icons
* detailed illustrations
* animated strokes

Benefits:

* exact geometry
* supports all join/cap types
* supports dash patterns
* supports variable width

---

# **3. Unified Stroke Task Descriptor**

```
pub enum StrokeMode {
    Analytic,
    Tessellated,
}

pub struct StrokeTask {
    pub mode: StrokeMode,
    pub width: f32,
    pub color: Color,
    pub join: BorderJoin,
    pub cap: StrokeCap,
    pub dash: Option<DashPattern>,
    pub placement: StrokePlacement,
    pub path: Arc<Path>,      // simple or complex
}
```

Path may be:

* Rect
* RoundRect
* Circle
* Path2D (arbitrary)

---

# **4. Stroke Placement**

Inherited from SPEC-R-015:

```
Inside
Center
Outside
```

Placement is handled differently in each mode:

### Analytic

Derived directly from SDF offsets.

### Tessellated

Stroke geometry generated explicitly for each placement.

---

# **5. GPU Tessellation Pipeline**

### **5.1 GPU Compute Stage**

Path flattening → polyline
Polyline → stroke mesh

Compute shader produces:

* vertex buffer
* index buffer
* join geometry
* cap geometry
* dash cuts

### **5.2 Mesh Layout**

```
Vertex {
    position: vec2<f32>,
    normal: vec2<f32>,     // for AA falloff
    uv: vec2<f32>,         // optional for textured strokes
}
```

### **5.3 Fragment Shader**

Evaluates:

```
alpha = clamp(1.0 - abs(dot(normal, view_delta)) / aa_width)
output = color * alpha
```

---

# **6. Analytic SDF Stroke Pipeline**

Direct descendant of SPEC-R-015.

Evaluates:

```
d = sdf(shape, pos)
alpha = stroke_range(d)
```

Strokes and fills can be combined without additional geometry.

---

# **7. Mode Selection**

Automatic:

```
if shape is simple AND dash==None AND width < threshold {
    Analytic
} else {
    Tessellated
}
```

Developer override possible.

---

# **8. Dash Support**

**Analytic Mode**

Dash is applied using arc-length param from SPEC-R-003.

**Tessellated Mode**

Dash cuts are performed in compute shader:

```
split polyline segments by dash intervals
create stroke segments
```

Supports:

* perfect dashes along Bézier curves
* variable dash lengths
* animated dash offset

---

# **9. Path2D Compatibility**

Supports fully arbitrary paths:

* MoveTo
* LineTo
* QuadTo
* CubicTo
* Close

Paths can be animated and re-tessellated incrementally.

---

# **10. Infinite Zoom Stability**

Analytic mode: infinite stability via SDF.
Tessellated mode: uses **screen-space AA** ensuring:

* no aliasing
* no pixel popping
* geometry scales without loss

---

# **11. Clip Stack Integration**

Both modes compatible with:

* rounded clip masks
* path clips
* nested clips
* opacity groups

Stroke meshes clip at GPU level using R-001/002.

---

# **12. Testing Matrix**

### Shapes

* rect
* rounded rect
* circle
* arbitrary path
* concave paths
* compound paths

### Stroke Types

* thin/thick strokes
* dash
* animated dash
* all placement modes
* all cap/join types

### DPI

* 1.0 → 4.0
* fractional DPR values

### Behavior

* no discontinuities
* smooth joins
* smooth caps
* correct dash resets

---

# **13. Implementation Plan**

## **Phase 1 — Core Abstractions**

* StrokeTask
* StrokeMode detection
* Path2D integration

## **Phase 2 — Analytic Stroke Engine**

* extend SDF shader
* integrate placement logic
* integrate simple dash

## **Phase 3 — GPU Tessellation Engine**

* compute shaders for flattening and meshing
* join/cap generators
* dash cutters

## **Phase 4 — Renderer Integration**

* new StrokePass
* unify with Clip Stack

## **Phase 5 — Optimization**

* path caching
* partial re-tessellation
* instance merging for static strokes

---

# **14. Future Work**

* textured strokes (striped, patterned)
* brush simulation (calligraphy)
* SVG stroke alignment compatibility
* multi-layer stacked strokes
