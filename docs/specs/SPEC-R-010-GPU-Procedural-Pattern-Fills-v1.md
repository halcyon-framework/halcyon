# **SPEC-R-010 — GPU Procedural Pattern Fills v1 (Checker, Stripe, Image-Tiled, Noise)**

**Author:** Halcyon Architect
**Status:** Retired — No current consumer. Deferred indefinitely.
**Version:** 1.0

**Depends On:**

* SPEC-R-001 Clip Stack Rework
* SPEC-R-002 GPU Clip Shaders & Rounded Masks
* SPEC-R-003 GPU Path Rendering Primer
* SPEC-R-004 GPU Path Renderer (MSAA Vector Pipeline v1)
* SPEC-R-005 GPU Distance Field Renderer (SDF Pipeline v2)
* SPEC-R-006 GPU Vector Stroke Engine
* SPEC-R-007 GPU Blend Engine v1
* SPEC-R-008 GPU Composition Graph v1
* SPEC-R-009 GPU Gradient Pipeline v1
* SPEC-HAL-024 Unified Coordinate Model

---

## Implementation Status

**Not implemented**

GPU procedural pattern fills are not implemented.

---

**Implements:**
A unified GPU pattern-fill system for filling arbitrary vector shapes with **procedural patterns** (not textures), allowing:

* checkerboard patterns
* stripes
* polka dots
* geometric patterns
* infinite noise (value/noise FBM)
* tileable image patterns (with UV transforms)

This SPEC forms the counterpart to R-009 (procedural gradients), enabling Halcyon to render complex vector-based UI skins, editors, and design surfaces at infinite resolution.

---

## **1. Motivation**

Without GPU pattern fills, Halcyon cannot:

* support CSS-like patterned backgrounds
* render SVG patterns dynamically
* offer unlimited resolution stripes/checkers
* provide procedural noise for UI shading
* produce editable, parameterized skins in real time

Texture-based patterns require pre-baked assets and introduce scaling artifacts.
Procedural patterns eliminate resolution limits and cost almost nothing in GPU time.

This SPEC makes patterns a first-class renderer concept, matching Flutter/Skia capabilities.

---

## **2. Goals**

### **2.1 Primary**

* Procedural pattern fills evaluated entirely in fragment shader
* Infinite resolution
* Transformable in widget/local/scene coordinates
* Full MSAA coverage integration
* Fully styleable via Style Runtime (SPEC-074)

### **2.2 Secondary**

* Pattern animation support
* Tileable image pattern support
* Linear-light color space evaluations

### **2.3 Out of Scope**

* Gradient-pattern fusion (future SPEC-R-012)
* Vector displacement mapping
* 3D procedural shaders

---

## **3. Pattern Types**

### **3.1 Checkerboard**

Parameters:

* size
* color A
* color B
* rotation
* origin

Fragment logic (modulus-based).

### **3.2 Stripes**

Parameters:

* orientation
* stripe width
* duty ratio
* color A/B

### **3.3 Polka Dots**

Parameters:

* dot radius
* spacing
* pattern rotation
* dual-color modes

### **3.4 Noise**

Two types:

* Value noise
* FBM (Fractal Brownian Motion)

Used for subtle background shading.

### **3.5 Image-Tiled Patterns**

* Sample from small 32×32 or 64×64 tiles
* Repeats infinitely
* Transforms (scale/rotation/translation) from SPEC-HAL-024

---

## **4. GPU Pipeline Architecture**

### **4.1 New Pipeline**

```
PatternFillPipeline {
    vs: PathFillVS
    fs: PatternFS
    msaa: 4x
    blend: R-007
}
```

### **4.2 Fragment Shader Responsibilities**

1. Load pattern parameters
2. Transform coordinates via SPEC-HAL-024
3. Evaluate pattern function
4. Blend with MSAA coverage
5. Apply clip mask
6. Output final color

---

## **5. Coordinate Model**

Halcyon’s unified coordinate model means pattern coordinates can be in:

* widget space
* parent space
* arbitrary transform space
* scene space

Pattern scaling is independent of device scale factor.

Pattern alignment stays stable during resize/animate operations.

---

## **6. Pattern Uniform Layout**

```
struct PatternParams {
    pattern_type: u32,
    color_a: vec4<f32>,
    color_b: vec4<f32>,
    origin: vec2<f32>,
    size: f32,
    rotation: f32,
    extra: vec4<f32>, // pattern-specific
    transform: mat3x3<f32>, // coordinate transform
}
```

Max 32 patterns per batch slice.

---

## **7. Integration With Composition Graph**

PatternFill becomes:

```
PassKind::PatternFill
```

Dependencies:

* must run after PathFill
* same MSAA target
* same clip stack

No restrictions on mixing with gradients; can be interleaved.

---

## **8. Precision Rules**

* All colors computed in linear space
* Noise FBM uses 4–6 octaves with adjustable amplitude
* Stripes/Checkers use exact floating-point modulus
* Tile patterns use filter=nearest for sharp repeat
* No gamma distortion

---

## **9. Testing Matrix**

### **9.1 Visual Tests**

* checker sharpness
* stripe aliasing
* dot uniformity
* noise smoothness
* image tiling with large scale

### **9.2 Stress Tests**

* 200 animated procedural patterns
* pattern transforms under animation
* high DPI scaling
* nested clips

### **9.3 Regression Tests**

* reordering pattern stops
* transform stability under resizing
* fractional pixel alignment

---

## **10. Implementation Plan**

### **Phase 1 — Core Types**

* Add `Pattern` struct
* Add `PatternParams` uniform

### **Phase 2 — Shaders**

* Implement checker, stripes, polka, noise
* Implement small texture tiling

### **Phase 3 — Renderer Integration**

* Add PatternFillPipeline
* Connect to CompositionGraph

### **Phase 4 — Style Integration**

* Extend SPEC-074 style structures
* Expose patterns to theming engine

### **Phase 5 — Validation**

* Full test suite
* Frame-by-frame animation tests

---

## **11. Notes**

* This SPEC completes the vector fill foundation:

    * R-009 → gradients
    * R-010 → patterns
* Combined they unlock a full design surface and advanced UI capabilities.
