# **SPEC-R-005 — GPU Distance Field Renderer (Vector SDF Pipeline v2)**

**Author:** Halcyon Architect
**Status:** Retired — SDF functions inlined in shadow/gradient shaders per SPEC-R-013-v2. Full SDF pipeline deferred indefinitely.
**Version:** 1.0

**Depends On:**

* SPEC-R-003 GPU Path Rendering Primer
* SPEC-R-004 GPU Path Renderer (MSAA Vector Pipeline v1)
* SPEC-HAL-024 Unified Coordinate Model
* SPEC-HAL-066 Unified Glyph Raster Pipeline

---

## Implementation Status

**Not implemented**

SDF vector rendering is not implemented.

---

**Implements:**
A complete **Signed Distance Field (SDF) vector renderer** for scalable, animated, low-overhead path rendering on the GPU:

1. CPU path → SDF atlas generation
2. Multi-channel SDFs for crisp curves
3. GPU SDF sampling and analytic edge evaluation
4. Resolution-independent rendering (infinite scale)
5. Integration into Halcyon's SceneGraph
6. Optional MSAA blend for stroke/fill hybrid precision

This is Halcyon's alternative vector pipeline for situations where MSAA tessellation is too heavy.

---

## **1. Motivation**

While **SPEC-R-004 (MSAA Pipeline)** is perfect for:

* precise shapes
* UI icons
* crisp small-scale geometry

…it begins to struggle when:

* paths scale 10×–200×
* animations warp the geometry
* extremely complex shapes stress tessellation
* vector icons are reused thousands of times

SDFs solve this.

Halcyon needs a **second vector path pipeline**:

* lighter CPU load
* GPU-friendly
* scale-independent
* ideal for icons, blurs, transforms, zooming, animating

This SPEC introduces that pipeline.

---

## **2. Goals**

### **2.1 Primary**

* Multi-channel SDF (MSDF) generation
* Stable anti-aliasing at all scales
* Crisp curves under any transform
* Correct fill vs stroke via analytic SDF
* Unified GPU path shader
* GPU atlas for vector shapes
* Zero CPU tessellation at draw time

### **2.2 Secondary**

* Optional reconstruction filters
* Support for:

    * fill
    * stroke
    * outline
    * inner/outer glow (future R-series)
* Shared atlas for CPU vector icons

### **2.3 Deferred**

* Vector gradients
* SDF “distance spread” animation effects
* Dynamic SDF feedback (compute shader)

---

## **3. Architecture Overview**

```
Path Commands
    ↓
CPU MSDF Generator (3-channel)
    ↓
SDFAtlas.pack() → atlas region + metadata
    ↓
SceneGraph::PathSdf { atlas_id, params }
    ↓
WGSL SDF Shader (analytic coverage)
    ↓
Framebuffer
```

Unlike MSAA (R-004), this path:

* needs **no triangles, no tessellation**
* uses a **fullscreen-style quad per path instance**
* evaluates geometry analytically per pixel

---

## **4. CPU SDF Generation**

### **4.1 Input**

*Path command list* (`MoveTo`, `LineTo`, `QuadTo`, `CubicTo`, `Close`)
Same as R-004.

### **4.2 Output**

```
struct SdfBitmap {
    width: u32,
    height: u32,
    channels: [Vec<u8>; 3],  // MSDF
    range: f32,              // distance-to-color mapping scale
    bounds: RectF,
}
```

### **4.3 Pipeline Requirements**

* Fast MSDF (multi-channel) algorithm
* Sharp corner detection
* Correct edge-color assignment
* Minimal bleeding in zero-distance zones
* Error threshold: **< 0.5 px** at scale 1×
* Linear color space baking

### **4.4 Padding**

SDFs require padding:

```
padding_px = range * 2.0   // for stable interpolation
```

---

## **5. GPU SDF Atlas**

### **5.1 Format**

```
RGBA8Unorm  // 3 MSDF channels + alpha mask
```

### **5.2 Atlas Key**

```
(path_hash, sdf_resolution)
```

### **5.3 Atlas Structure**

```
struct SdfAtlas {
    texture: wgpu::Texture,
    allocator: ShelfPacker,
    entries: HashMap<AtlasKey, AtlasRegion>,
}
```

### **5.4 Requirements**

* Deterministic packing
* Zero-resize-at-runtime
* Per-entry metadata:

    * UV rect
    * range
    * bounds

---

## **6. Renderer Integration**

### **6.1 SceneGraph Node**

```
PathSdf {
    atlas_id: u32,
    color: Color,
    transform: mat3x2,
    sdf_range: f32,
}
```

### **6.2 Vertex Layout**

One quad per instance:

```
position: vec2<f32>  // rect corners
uv: vec2<f32>
instance_id: u32
```

### **6.3 SDF WGSL Shader**

#### **6.3.1 Fragment Stage**

```
float4 msdf = textureSample(atlas, uv);
float sd = median(msdf.r, msdf.g, msdf.b) * sdf_range;

float alpha = clamp(sd + 0.5, 0.0, 1.0);
// optional msaa-combine
```

* median preserves shape quality
* analytic AA from signed-distance evaluation
* stable under zooming/rotation/shear

---

## **7. Fill & Stroke Logic**

### **Fill**

If `sd < 0.0` → inside
Otherwise → outside with AA falloff

### **Stroke**

```
float s = abs(sd) - stroke_width;
alpha = smoothstep(0.0, 1.0, -s);
```

Zero tessellation required.

---

## **8. Clip Stack Integration**

Same clip system as R-001 / R-002:

* clip applied before SDF evaluation
* SDF quad clipped by GPU rounded mask
* Fill/stroke evaluation only inside clip region

---

## **9. Performance Characteristics**

### **CPU**

* Heavy one-time cost per path → cached
* Zero per-frame tessellation
* Good for thousands of icons

### **GPU**

* Two triangles per path
* Very small instance buffer
* One texture sample per pixel
* High-quality scaling up to 512× without jaggies

### **Memory**

Typical MSDF icon:

* 128×128 px
* RGBA8 = 64 KB
* Thousands fit in a single atlas

---

## **10. Testing Matrix**

### **10.1 Precision Tests**

* Sharp corners
* Tiny shapes (3px icons)
* Thick strokes (50px+)

### **10.2 Transform Tests**

* Rotation
* Shear
* Scale 0.01× → 500×
* Animated transformation

### **10.3 Clip Tests**

* Rounded clip
* Nested clip
* Combined clip + stroke

### **10.4 Visual Tests**

* Compare SDF vs MSAA
* Edge falloff stability
* No color fringing

---

## **11. Implementation Plan**

### **Phase 1 — CPU SDF Generator**

* Port MSDF algorithm
* Color edge assignment
* Range computation

### **Phase 2 — SDFAtlas**

* Allocation
* Upload
* Region tracking
* UV generation

### **Phase 3 — WGSL Shader**

* Median step
* Range scale
* Alpha computation
* Fill/stroke separation

### **Phase 4 — Renderer**

* Add PathSdf node
* Add quad instancer
* Add atlas bind group

### **Phase 5 — Integration**

* Clip
* Transform
* SceneGraph

### **Phase 6 — Validation**

* Compare with reference images
* Stress tests
* Animated transforms test suite

---

## **12. Implementation Notes**

* Use MSDFGEN-style triangle-based edge coloring
* Avoid GLSL-style signed-distance errors on concave shapes
* Use linear-light color for alpha reconstruction
* Prevent mipmapping (must use “nearest” sampling)
* Provide fallback to R-004 for extremely thin paths
