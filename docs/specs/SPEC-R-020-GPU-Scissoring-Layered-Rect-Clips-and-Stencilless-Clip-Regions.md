# **SPEC-R-020 — GPU Scissoring, Layered Rect Clips & Stencilless Clip Regions**

**SPEC-R-020**
**GPU Scissoring, Layered Rect Clips & Stencilless Clip Regions**
**Author:** Halcyon Architect
**Status:** Complete
**Version:** 1.0
**Amended:** 2026-09-21 — Removed the stale text-background attachment sizing cross-reference; current text rendering is grayscale.

**Depends On:**

* SPEC-R-001 Clip Stack Rework
* SPEC-R-002 GPU Clip Shaders & Rounded Masks
* SPEC-R-003 GPU Path Rendering Primer
* SPEC-R-010 Texture Filters & Pixel Grid Control
* SPEC-R-012 GPU Path Batching
* SPEC-R-014 Layer Ordering v1

---

## Implementation Status

**SPEC Complete** — All 13 stages implemented.

### Completed Stages

* **Stage 1-8**: Foundation — scissor clipping, clip stack, rounded clip data structures
* **Stage 9**: CPU precomputation of corner zones (inner/outer bounds)
* **Stage 10**: Global toggle for GPU rounded clipping (`set_gpu_rounded_clips_enabled`)
* **Stage 11**: CPU fallback detection for weak/buggy GPUs (Intel HD, SwiftShader, GL backend)
* **Stage 12**: Parameterized AA radius (`set_rounded_clip_aa_radius`, default 1.0)
* **Stage 13**: AA-aware inner bounds for multi-radius consistency

### Key Features

* **Stencil-free GPU rounded clipping** with per-fragment AA via smoothstep
* **CPU-precomputed bounds** for early accept/reject optimization
* **Automatic fallback** to scissor-only on problematic GPUs
* **Configurable AA radius** (0.25–4.0 logical pixels)
* **Mixed corner radii** with consistent AA band thickness

### Files Modified

* `src/renderer/wgpu_renderer.rs` — `RectGlobalsUniform`, `RoundedClipUniform`, fallback detection
* `src/renderer/shaders/rect.wgsl` — Fragment shader with AA coverage

---

# **Invariants**

## **AA-CLIP-INV-01: Unified Analytic Anti-Aliasing for Rounded Clip Edges**

**Statement:**
All rounded clip edges in `rect.wgsl` participate in analytic anti-aliasing.
A clip with `radius = 0` on any corner still receives edge-based AA for that region.
No shader path may bypass clip edge coverage computation based on whether radii are zero.

**Scope:**
This invariant applies to the **rounded clip shader** (`rect.wgsl`) only.
It does NOT apply to:
* Path pipeline (`path.wgsl`) — tessellated geometry, clip is rectangular bounds only
* Shape silhouette AA — out of scope; this invariant governs clip mask edges

**Requirements:**

1. **AA is edge-based, not radius-based**
   - Every clip edge (straight or curved) receives distance-based coverage
   - Per-corner radii (`radii_logical: vec4`) must be handled correctly (TL/TR/BL/BR)
   - No boolean `has_rounding` branches that skip coverage math

2. **Forbidden patterns:**
   ```wgsl
   // FORBIDDEN: Early return skipping AA based on radii
   if (!has_rounding) {
       return vec4<f32>(c.rgb * c.a, c.a);
   }
   ```

3. **Allowed performance shortcuts:**
   ```wgsl
   // ALLOWED: Outer reject (definitely outside clip + AA band)
   if (outside_outer_bounds) { discard; }

   // ALLOWED: Inner accept (proven coverage=1.0, >= aa from all boundaries)
   if (inside_inner_bounds) { return vec4<f32>(c.rgb * c.a, c.a); }
   ```

4. **Per-corner distance computation:**
   - Corner zones use circular distance to corner center
   - Straight edge zones use linear distance to edge
   - Coverage via `smoothstep` in both cases
   - No uniform-radius SDF that ignores per-corner radii

**Rationale:**

* **Prevents coverage discontinuities** — No visible seam between rounded corners and straight edges
* **Correct per-corner radii** — Mixed radii (e.g., TL=8, TR=0, BL=4, BR=12) render correctly
* **Eliminates AA fringe artifacts** — Empty pixels at clip edges are structurally impossible

**Path pipeline note:**
The path pipeline (`path.wgsl`) uses tessellated geometry and rectangular clip bounds.
Edge AA for path shapes comes from tessellation quality or MSAA, not fragment shader coverage.
This invariant does not govern path pipeline AA behavior.

---

## **INV-RENDER-SCISSOR-OWNERSHIP: Scissor State Non-Persistence**

**Statement:**
GPU scissor state is NOT persistent authority across render phases. Every render phase that issues draw calls MUST explicitly set scissor state before drawing. No phase may rely on scissor state set by a previous phase.

**Scope:**
This invariant governs the relationship between render phases within a single frame:
* Geometry batch phase (rect/path primitives with per-batch clip)
* Text rendering phase (text quads with per-primitive clip)
* Any future render phases added to the pipeline

**Requirements:**

1. **Explicit scissor before draw**
   - Each render phase MUST set scissor rect before issuing draw calls
   - No assumption that "scissor was reset" by previous code
   - No assumption that "last scissor is still valid"

2. **Per-primitive clip authority**
   - Primitives carry their own clip_rect (Some(RectF) or None)
   - None = fullscreen scissor (no clipping)
   - Some(rect) = scissor to that rect
   - Renderer batches by clip_rect to minimize state changes

3. **Forbidden patterns:**
   ```rust
   // FORBIDDEN: Relying on scissor from geometry phase during text phase
   // geometry_pass sets scissor to scroll_area clip
   // text_pass draws without setting scissor
   // Result: text outside scroll_area is clipped incorrectly
   ```

4. **Required pattern:**
   ```rust
   // REQUIRED: Each phase manages its own scissor
   for batch in primitives_by_clip {
       let scissor = match batch.clip_rect {
           None => fullscreen_scissor,
           Some(rect) => rect_to_scissor(rect),
       };
       render_pass.set_scissor_rect(scissor);
       render_pass.draw_indexed(batch.range);
   }
   ```

**Rationale:**

* **Prevents authority leak** — Scissor state from one phase leaking into another causes incorrect clipping. The text rendering bug (labels clipped by scroll area scissor) was caused by this leak.
* **Enables mixed-clip rendering** — Text primitives can have different clip authorities (some clipped to scroll area, some fullscreen) within the same frame.
* **Explicit over implicit** — No hidden state assumptions between render phases.

**Cross-References:**
* SPEC-HAL-020 — No hidden global state principle
* SPEC-HAL-150 — Render authority boundaries
* SPEC-HAL-081B — INV-RENDER-SHARED-ATTACHMENT (shared attachment sizing)
* SPEC-HAL-064 / SPEC-HAL-067 — future, not-implemented LCD/subpixel designs; no current text background attachment

---

**Implements:**
A high-performance, **stencil-free**, **depth-free**, **GPU scissor stack** for rectangular clips and layered cutouts.
The goal is:

* quick rectangular clipping
* no stencil buffer
* no per-primitive complex clip evaluation
* high batching retention
* unified clip pipeline for all layers (fill, stroke, textures, nine-patch, text)

This forms the foundation before more advanced clip primitives (R-021+) land.

---

# **1. Motivation**

Rectangular clips occur everywhere:

* scroll areas
* buttons
* panels
* nested lists
* code editors
* masked backgrounds
* area-select widgets
* tooltips / popovers

Stencil-based clipping is expensive and disrupts batching.
Full GPU mask evaluation (R-002) is overkill for simple rects.

We need a **fast, predictable, GPU-level scissor stack**.

---

# **2. Concepts**

## **2.1 Stencilless Clips**

All rect-only clips are resolved **on CPU** into a single **intersection rectangle** per layer pass.

This means:

* no per-clip GPU state change
* no nested GPU scissors
* no depth buffers
* one final scissor rect per draw batch

## **2.2 Layered Clip Stack**

Each layer inherits clip from its parent:

```
final_clip = intersection(parent_clip, local_clip)
```

Applied once when emitting draw commands.

---

# **3. Types**

```
pub struct ClipRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub struct ClipStack {
    stack: Vec<ClipRect>,
}
```

Operations:

```
push(clip)
pop()
current()  // returns intersection of all active clips
```

ClipRect always in **absolute logical coordinates**.

---

# **4. Renderer Integration**

## **4.1 CPU-Side Intersection**

Every time the renderer hits a Node::Clip:

1. intersect parent clip with local clip
2. push result
3. draw subtree
4. pop

## **4.2 GPU Scissor Command**

Single per-batch entry:

```
render_pass.set_scissor_rect(
    x_phys, y_phys,
    w_phys, h_phys
);
```

Coordinates in **physical** pixels, floored.

If batch contains different clips → batch split.

## **4.3 Batch Rules**

Batch is split **only when**:

* clip region changes
* render pipeline changes
* texture binding changes
* layer changes

Rect-only clips do **not** force atomic batching; they only segment where needed.

---

# **5. Interaction With Other Clip Types**

## **5.1 Rounded Clips (R-002)**

Rect clip → GPU scissor
Rounded clip → GPU mask shader (no scissor)

If both:

```
rounded_clip_rect ∧ scissor_rect → scissor applied first
```

## **5.2 Path-based Clips (R-003)**

Rect clip serves as coarse culling:

```
scissor = intersection(rect_clip, path_clip_bounds)
```

Path mask applied after.

---

# **6. Pixel Grid Rules**

Matches HAL pixel-grid spec:

* x, y floored
* w, h ceiled
* ensures coverage of all logical pixels
* prevents half-pixel artifact edges

```
phys_x = floor(logical_x * scale)
phys_y = floor(logical_y * scale)
phys_w = ceil(logical_w * scale)
phys_h = ceil(logical_h * scale)
```

Same formula used for all clips.

---

# **7. Testing Matrix**

### CPU Intersection:

* trivial overlaps
* non-overlapping rects → empty rect
* nested deep clip chains (10–100)

### GPU Scissor Behavior:

* fractional vs integer logical coords
* high-DPI (1.25×, 1.5×, 1.75×, 2×, 3×)
* animating clip rects (smooth resize)

### Batching:

* lists with hundreds of clipped children
* scroll view with moving viewport clips
* mixed rect + rounded + path clips
* transitions where clip changes mid-frame

---

# **8. Implementation Plan**

## **Phase 1 — CPU ClipStack**

* add stack
* add intersection logic
* ensure absolute logical coordinates

## **Phase 2 — Integrate into SceneGraph Traversal**

* Node::Clip updates current clip
* all children read current clip

## **Phase 3 — Renderer**

* pass clip region to batch builder
* split batches when necessary
* emit scissor rect before batch draw

## **Phase 4 — DPI & Transform**

* convert logical → physical
* ensure alignment with glyph-snap rules

## **Phase 5 — Optimization**

* small rect cache for repeated clips
* avoid redundant scissor commands

---

# **9. Future Extensions**

* Rounded-rect scissor fallback (fast-path for common rounded clips)
* Per-edge clip-repeat behavior for scrollbars
* Animated clipping masks
* Hierarchical clip-region culling for path rendering
