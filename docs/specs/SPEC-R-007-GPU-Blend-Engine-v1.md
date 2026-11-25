# **SPEC-R-007 — GPU Blend Engine v1 (Linear-Light, Premultiplied, Correct Compositing)**

**Author:** Halcyon Architect
**Status:** Retired — Premultiplied alpha blending sufficient for current needs. Advanced blend modes deferred indefinitely.
**Version:** 1.0

**Depends On:**

* SPEC-R-001 Clip Stack Rework
* SPEC-R-002 GPU Clip Shaders & Rounded Masks
* SPEC-R-003 GPU Path Rendering Primer
* SPEC-R-004 GPU Path Renderer (MSAA Vector Pipeline v1)
* SPEC-R-005 GPU Distance Field Renderer (SDF Pipeline v2)
* SPEC-R-006 GPU Vector Stroke Engine
* SPEC-HAL-024 Unified Coordinate Model
* SPEC-HAL-066 Unified Glyph Raster Pipeline

---

## Implementation Status

**Not implemented**

The unified GPU blend engine described in this SPEC is not implemented. Basic alpha blending exists.

---

**Implements:**
A unified **GPU blending pipeline** for Halcyon ensuring all rendering (fills, strokes, SDF, glyphs, clip masks) uses correct, deterministic compositing:

* Premultiplied alpha
* Linear-light math
* Correct Porter–Duff operations
* Optional backdrop modes
* Fully consistent across platforms

This SPEC replaces ad-hoc sRGB / non-premultiplied blending and ensures the entire renderer obeys a single physically-correct blending strategy.

---

## **1. Motivation**

Halcyon currently blends using WGPU defaults (sRGB target, premultiply inconsistencies).

Problems:

* Wrong gamma for alpha compositing
* Text halo artifacts
* Clip-mask fringes
* Incorrect color accumulation across multiple passes
* Multi-layer UI looks “muddy”
* Inconsistent color output across OSes

Correct blending requires:

1. **Linear-light** color math
2. **Premultiplied** alpha everywhere
3. Stable Porter–Duff operators

R-007 introduces a dedicated **Blend Engine** integrated into the renderer.

---

## **2. Goals**

### **2.1 Primary**

* Every draw uses the same blending formulation
* Renderer performs blending in **linear RGB**
* All textures (glyph atlas, SDF atlas, path buffers) stored in formats compatible with linear pipelines
* Text, strokes, fills all composite correctly
* No halo / fringing from sRGB misalignment

### **2.2 Secondary**

* Optional backdrop-filter modes (blur, color matrix)
* Mask compositing
* Full-screen linear light render target

### **2.3 Deferred**

* HDR pipeline
* Wide-gamut color management (P3, Rec.2020)

---

## **3. Architecture Overview**

```
          SceneGraph (all nodes premultiplied)
                     ↓
              Render Passes
                     ↓
        Linear-Light Intermediate Target
                     ↓
         WGSL Blend Engine (Porter-Duff)
                     ↓
         sRGB Frame Output Conversion
```

---

## **4. Premultiplied Color Model**

### **4.1 Always stored as:**

```
Rgb_premult = R * A, G * A, B * A
Alpha       = A
```

### **4.2 Benefits**

* Correct interpolation
* No halo around glyph edges
* Soft masks look perfect
* Correct layering regardless of z-order

### **4.3 Requirements**

Every shader must output premultiplied values:

```
out.rgb *= out.a
```

This includes:

* Text fragment shader
* SDF shader
* Path/MSAA shader
* Stroke distance shader

---

## **5. Linear-Light Rendering Target**

### **5.1 Format**

Two possible targets:

* **RGBA16Float** (preferred, perfect)
* **RGBA8Unorm-sRGB + explicit linearize** (fallback, less precise)

### **5.2 Linearization Model**

```
linear = srgb_to_linear(in_color)
out = linear_blend(...)
final = linear_to_srgb(out)
```

WGSL helper functions included in this SPEC.

---

## **6. The Blend Engine Shader**

### **6.1 Inputs**

```
src.rgb (premult)
src.a
dst.rgb (premult)
dst.a
blend_mode
```

### **6.2 Porter–Duff Operators**

#### Over (default):

```
out.rgb = src.rgb + dst.rgb * (1 - src.a)
out.a   = src.a   + dst.a   * (1 - src.a)
```

#### Additive:

```
out.rgb = src.rgb + dst.rgb
out.a   = min(1, src.a + dst.a)
```

#### Multiply:

```
out.rgb = src.rgb * dst.rgb + src.rgb * (1 - dst.a) + dst.rgb * (1 - src.a)
out.a   = max(src.a, dst.a)
```

### **6.3 Mode Table**

| Mode         | Use case               |
| ------------ | ---------------------- |
| **Over**     | Default UI composition |
| **Add**      | Glow, highlights       |
| **Multiply** | Shadows, dark surfaces |
| **Screen**   | Light effects          |

---

## **7. Clip Integration**

Clips in Halcyon already produce **coverage masks**:

```
final_alpha = src_alpha * clip_alpha
src_rgb     = src_rgb * clip_alpha
```

Blend engine consumes these directly.

Masks remain in linear space.

---

## **8. Pipeline Integration**

### **8.1 Render Graph**

1. Path pass → premultiplied linear
2. Stroke pass → premultiplied linear
3. Text pass → premultiplied linear
4. Composition pass (Blend Engine)
5. UI intermediate target
6. sRGB resolve → swapchain

### **8.2 WGPU Bindings**

* src texture view
* dst texture view
* blend mode uniform

### **8.3 Draw Flow**

```
Bind pipeline(blend)
Bind src texture
Bind dst texture
Draw full-screen quad
Swap roles or accumulate
```

---

## **9. Backdrop Filters (Optional)**

Simple extension:

* blur via gaussian
* brightness/contrast via matrix
* color grading via LUT

Blend engine routes backdrop texture as needed.

---

## **10. Testing Matrix**

### **10.1 Color correctness**

* Flat UI surfaces
* Text on bright/dark backgrounds
* Semi-transparent overlays
* Nested transparency (3+ layers)

### **10.2 Gamma correctness**

Compare:

* incorrect sRGB blend
* correct linear-light blend

Expected: no halos, no muddy areas.

### **10.3 Compositing correctness**

* Glyph → clip mask → blend
* Stroke → fill → background
* Backdrop → text → overlay

---

## **11. Implementation Plan**

### **Phase 1 — Premultiplied Conformance**

* All shaders output premultiplied
* SceneGraph stores premultiplied color

### **Phase 2 — Linear Target**

* Create RGBA16Float intermediate
* Port all render passes to linear space

### **Phase 3 — WGSL Blend Engine**

* Implement Over / Add / Multiply / Screen
* Hooks into RenderGraph

### **Phase 4 — Integration**

* Replace wgpu built-in blending
* Adopt explicit composition pass

### **Phase 5 — Validation**

* Reference images
* Freetype test pages
* Color-gradient tests

---

## **12. Implementation Notes**

* Enforcing premultiplied stops 95% of text halos
* Linear-light blending is mandatory for UI fidelity
* Must avoid double-linearization
* Must disable WGPU’s automatic sRGB target when using RGBA16F
* WGPU blend states should be disabled entirely in the new pipeline
* Composition must be explicit, not fixed-function
