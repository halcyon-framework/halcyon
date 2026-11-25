# **SPEC-R-006 — GPU Vector Stroke Engine (Analytic Thick Strokes v1)**

**Author:** Halcyon Architect
**Status:** Retired — Stroke rendering handled by SPEC-R-004 path pipeline tessellation. Dedicated stroke engine deferred indefinitely.
**Version:** 1.0

**Depends On:**

* SPEC-R-003 GPU Path Rendering Primer
* SPEC-R-004 GPU Path Renderer (MSAA Vector Pipeline v1)
* SPEC-R-005 GPU Distance Field Renderer (SDF Pipeline v2)
* SPEC-HAL-024 Unified Coordinate Model
* SPEC-HAL-066 Unified Glyph Raster Pipeline

---

## Implementation Status

**Not implemented**

GPU vector stroke engine is not implemented.

---

**Implements:**
A GPU-based **analytic stroke rendering engine** for arbitrary stroke widths, joins, caps, and dash patterns—without full tessellation overhead and without SDF distortions at large widths.

This fills the gap between:

* **R-004 (MSAA)** — perfect for precise edges, small widths
* **R-005 (SDF)** — perfect for scalable fills and icons

…and provides a dedicated **GPU stroke pipeline** for wide strokes (1px → 200px).

---

## **1. Motivation**

Halcyon needs a stroke renderer that satisfies:

* high fidelity for thick strokes
* correct joins (miter, bevel, round)
* correct caps (butt, round, square)
* consistent visual results across all transforms
* no CPU tessellation explosion at large widths
* no SDF mushiness at extreme stroke sizes

The MSAA tessellator (R-004) cannot handle 50px–200px strokes efficiently.
The SDF pipeline (R-005) cannot maintain sharpness for strokes with variable width or extreme zoom.

R-006 introduces a hybrid analytic solution.

---

## **2. Goals**

### **2.1 Primary**

* GPU-evaluated analytic stroke function
* Clean miter/round/bevel joins
* High-quality caps
* Stable under any transform
* Minimal geometry (polyline segments only)
* Per-pixel width computation

### **2.2 Secondary**

* Dash patterns
* Variable-width strokes
* Pressure curves (Stylus input path)

### **2.3 Deferred**

* Stroke gradients
* Stroke textures
* Animated brush effects

---

## **3. Architecture Overview**

```
CPU Path (MoveTo / LineTo / QuadTo / CubicTo)
                ↓
GPU Stroke Geometry Builder (Polyline)
                ↓
StrokeInstance { segments, style, width }
                ↓
WGSL Stroke Shader (signed-distance-to-segment evaluation)
                ↓
Framebuffer
```

Key concept:
**The GPU computes coverage by evaluating the distance from the pixel to each stroke segment**, not by tessellating thick geometry.

---

## **4. CPU Stage — Polyline Segment Extraction**

### **4.1 Input**

Vector path commands as usual.

### **4.2 Output**

```
struct StrokeSegment {
    p0: vec2,
    p1: vec2,
    normal: vec2,
    length: f32,
    kind: SegmentKind, // Line | QuadraticApprox | CubicApprox
}
```

### **4.3 Curve Flattening**

Use **adaptive subdivision**:

```
error_threshold = 0.25 px
```

Produces a polyline stable at all DPIs.

---

## **5. GPU Stroke Instance**

### **5.1 Structure**

```
struct StrokeInstance {
    base_transform: mat3x2,
    width: f32,
    miter_limit: f32,
    cap: CapStyle,
    join: JoinStyle,
    color: Color,
    segment_range: Range<u32>,
}
```

Segments live in a global GPU buffer and instances reference ranges.

### **5.2 Cap Types**

* Butt
* Round
* Square

### **5.3 Join Types**

* Miter
* Bevel
* Round

---

## **6. GPU Shader — Analytic Stroke Distance**

### **6.1 Per-pixel distance computation**

For each pixel:

```
d_line = distance_to_line_segment(p, p0, p1)
d_join = join_evaluation(...)
d_cap  = cap_evaluation(...)
```

Final coverage:

```
d = min(d_line, d_join, d_cap)
alpha = smoothstep(stroke_width, stroke_width - aa_width, d)
```

Where:

```
aa_width = 1.0 px / scale_factor
```

### **6.2 Join Logic**

* **Miter**: clamp miter length
* **Bevel**: distance to triangular bevel region
* **Round**: distance to arc between segment normals

### **6.3 Cap Logic**

* **Butt**: half-plane
* **Square**: Butt + extrusion by width/2
* **Round**: radial distance test

---

## **7. Geometry**

### **7.1 What gets drawn**

Each **stroke** draws:

* one quad = the path bounding box
* GPU shader evaluates whether each pixel is inside stroke thickness

This eliminates per-segment triangles entirely.

### **7.2 Bounding box inflation**

```
bbox.inflate(width)
```

Ensures coverage.

---

## **8. Dash Patterns**

### **8.1 GPU-based pattern evaluation**

Distance field modulated:

```
t = projected_length_along_path(p)
phase = mod(t + dash_offset, dash_period)
alpha *= step(phase, dash_on_length)
```

### **8.2 CPU requirement**

Stroke segments must include accumulated length.

---

## **9. Variable-Width Strokes (Optional)**

### **9.1 Input**

```
width(t) curve
```

### **9.2 GPU**

```
stroke_width = sample(width_curve, projected_length)
```

### **9.3 Effects**

* calligraphic strokes
* pressure-sensitive brush lines

---

## **10. Integration with SceneGraph**

New node:

```
SceneNode::Stroke(StrokeInstance)
```

Painter’s order identical to fills and SDF paths.

Clips applied normally.

Coordinate model:
Absolute logical → physical via the renderer transform.

---

## **11. Performance Characteristics**

### **CPU**

* Very light (polyline extraction only)
* 20–100× lighter than tessellation

### **GPU**

* One quad per stroke
* Distance evaluation = few ops per pixel
* Quality stable under transform

### **Memory**

* Single segments buffer reused across instances
* Segment size: small (vec2 + vec2 + f32)

---

## **12. Testing Matrix**

### **12.1 Precision**

* 1px strokes
* 100px strokes
* Miter edge cases
* Thick round joins

### **12.2 Transform**

* Rotate
* Shear
* Scale (0.01×–200×)

### **12.3 Dash Patterns**

* Crisp boundaries
* Animated dash offset

### **12.4 Variable Width**

* Test width interpolation
* No warping or cusps

---

## **13. Implementation Plan**

### **Phase 1 — CPU Polyline Flattening**

* Adaptive curve splitting
* Segment buffer creation

### **Phase 2 — StrokeInstance Builder**

* Join/cap styles
* Width/miter settings
* Length accumulation

### **Phase 3 — GPU Segment Buffer**

* Storage buffer for segments
* Range indexing

### **Phase 4 — WGSL Distance Shader**

* Line distance
* Join distance
* Cap distance
* Coverage model

### **Phase 5 — SceneGraph Integration**

* Path → StrokeInstance transform
* Bounds inflation

### **Phase 6 — Visual Validation**

* Golden reference images
* LTR/RTL combine
* Complex paths

---

## **14. Implementation Notes**

* Distance evaluation done in stroke-local space
* Use linear-light space for alpha
* Avoid msdf-artifacts: this pipeline is purely analytic
* Very stable under viewport animation
* Miter edge cases require clamping logic
* Segment buffer must remain tightly packed
