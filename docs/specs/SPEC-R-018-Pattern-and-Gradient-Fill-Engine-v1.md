# **SPEC-R-018 — Pattern & Gradient Fill Engine v1 (Linear, Radial, Angular, Image Patterns)**

**SPEC-R-018**
**Pattern & Gradient Fill Engine v1**
**Author:** Halcyon Architect
**Status:** Deferred — After gradient pipeline (R-012-v2)
**Version:** 1.0

**Depends On:**

* SPEC-R-001 Clip Stack Rework
* SPEC-R-002 GPU Clip Shaders & Rounded Masks
* SPEC-R-003 GPU Path Rendering Primer
* SPEC-R-006 Path Flattening v2
* SPEC-R-010 Texture Filters & Pixel Grid Control
* SPEC-R-011 Blend Modes & Composite Operators
* SPEC-R-016 Stroke Tessellation v2
* SPEC-R-017 Advanced Fill Tessellation v2

---

## Implementation Status

**Not implemented**

Pattern and gradient fill engine is not implemented.

---

**Implements:**
A unified pattern/gradient fill subsystem supporting:

* linear gradients
* radial gradients
* angular (conical) gradients
* repeating patterns (image-based)
* transformable shader uniforms
* style inheritance for UI elements
* integration with fills, strokes, clips

This SPEC defines the rendering model and shader-level behavior.

---

# **1. Motivation**

Flat colors aren’t enough for modern UI work:

* highlights
* soft shadows
* surface transitions
* animated gradients
* textured components
* SVG compatibility

Current renderer only knows **solid colors**.

This SPEC introduces:

* GPU-driven gradients & patterns
* unified transform model
* consistent interpolation rules
* per-path and per-stroke fill sources

---

# **2. FillSource Type**

```
pub enum FillSource {
    Solid(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    AngularGradient(AngularGradient),
    ImagePattern(ImagePattern),
}
```

Each variant contains its own descriptor.

---

# **3. Gradient Descriptors**

## **3.1 Linear Gradient**

```
pub struct LinearGradient {
    pub start: Vec2,        // in local logical coords
    pub end: Vec2,
    pub stops: Vec<ColorStop>,
    pub repeat: RepeatMode, // Clamp | Repeat | Mirror
    pub transform: Mat3x2,  // optional
}
```

## **3.2 Radial Gradient**

```
pub struct RadialGradient {
    pub center: Vec2,
    pub radius: f32,
    pub focal: Vec2,        // if focal == center → regular radial
    pub stops: Vec<ColorStop>,
    pub repeat: RepeatMode,
    pub transform: Mat3x2,
}
```

## **3.3 Angular (Conical) Gradient**

```
pub struct AngularGradient {
    pub center: Vec2,
    pub angle_start: f32,
    pub angle_end: f32,
    pub stops: Vec<ColorStop>,
    pub repeat: RepeatMode,
    pub transform: Mat3x2,
}
```

## **3.4 Color Stop**

```
pub struct ColorStop {
    pub offset: f32,   // 0.0–1.0
    pub color: Color,  // premultiplied RGBA
}
```

Stops must be sorted.

---

# **4. Image Patterns**

```
pub struct ImagePattern {
    pub texture_id: TextureId,
    pub tile_size: Vec2,         // repeat region
    pub transform: Mat3x2,       // scale/rotate/skew tile
    pub filter: ImageFilter,     // Nearest | Linear | Mipmap
    pub opacity: f32,
}
```

Supports:

* repeating
* mirroring
* rotation
* UI-scaling

Textures come from:

* asset manager
* dynamic GPU uploads
* SVG rasterization

---

# **5. Rendering Pipeline**

Patterns & gradients integrate into the fill pipeline after tessellation.

### GPU Flow:

1. Tessellate path to triangles (from R-017)
2. Per-triangle: emit vertices with **local UVs** derived from path coordinates
3. Fragment shader samples:

   ```
   let p = local_pos;
   let uv = transform * p;
   let color = sample_gradient_or_pattern(uv);
   out = blend(color, base_color);
   ```

### Fill and Stroke both support FillSource.

---

# **6. Shader Implementations**

## **6.1 Linear Gradient Shader**

```
t = dot(p - start, dir_normalized) / length;
t = apply_repeat_mode(t);
color = interpolate_stops(stops, t);
```

## **6.2 Radial Gradient Shader**

```
d = distance(p, focal);
angle = some correction;
t = d / radius;
t = apply_repeat_mode(t);
color = interpolate_stops(stops, t);
```

## **6.3 Angular Gradient Shader**

```
θ = atan2(p.y - center.y, p.x - center.x);
θ' = normalize_angle(θ);
t = (θ' - angle_start) / (angle_end - angle_start);
t = apply_repeat_mode(t);
color = interpolate_stops(stops, t);
```

## **6.4 Image Pattern Shader**

```
uv = transform * p;
uv_mod = repeat(uv, tile_size);
color = textureSample(texture_id, uv_mod);
color *= opacity;
```

---

# **7. Style Integration**

Paths and strokes can use FillSource.

In HAL-074 Style Runtime and HAL-076 StyleCache:

* FillSource becomes a styleable property
* gradients can be animated (HAL-077)
* patterns can be overridden by themes

Default styles may generate:

* disabled-state gradients
* hover-state transitions

---

# **8. Interaction With Clip Stack**

Applied in order:

1. clip mask
2. fill or stroke gradient/pattern
3. blend operator

Gradients respect clipping; patterns are clipped normally.

---

# **9. Coordinate Spaces & Transforms**

All FillSources use **local logical coordinates**.

Transform rules:

* path-local → FillSource transform → world logical → NDC
* gradients remain stable during animation
* image patterns can tile consistently at any zoom

---

# **10. Caching & Reuse**

Fill descriptor hash:

```
hash(FillSource)
```

Caches GPU uniform buffers and texture handles.

Patterns reuse their textures.
Gradients reuse stop buffers.

---

# **11. Testing Matrix**

### Gradients:

* degenerate linear (start==end)
* radial with focal outside center
* angular wrap-around
* repeat/mirror modes

### Patterns:

* nearest / linear sampling
* scale transform
* rotation transform
* tile alignment at zoom 1×–8×

### Strokes:

* stroke width impacts gradient UV
* proper join/miter behavior

### Clips:

* patterns & gradients with rounded clips
* correct blending inside clip shapes

---

# **12. Implementation Plan**

## **Phase 1 — FillSource Type**

* Add FillSource enum
* Add gradient/pattern descriptors

## **Phase 2 — Uniform Layout**

* GPU uniform buffers for gradient parameters
* FillSource → shader mapping

## **Phase 3 — Gradient Shader Library**

* linear, radial, angular gradient WGSL
* UV transform logic

## **Phase 4 — Pattern Shader**

* tile repeat logic
* filtering support
* texture binding updates

## **Phase 5 — Renderer Integration**

* FillPass becomes aware of FillSource
* strokes use same system

## **Phase 6 — Optimization**

* constant-time stop lookup
* mipmap generation for patterns
* compressed GPU buffer formats

---

# **13. Future Work**

* gradient meshes (multi-stop polygon gradients)
* SVG radial-focal distance model
* procedural textures
* noise patterns
* animated UVs (parallax, scroll effects)
