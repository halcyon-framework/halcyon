**Status: Historical. Superseded by SPEC-HAL-085A. Not binding.**

# SPEC-HAL-085 — Halcyon Shader Suite v1

**Status:** Draft
**Layer:** Renderer / GPU Substrate
**Severity:** Structural
**Enforcement:** Mandatory
**Related SPECs:** SPEC-HAL-002, SPEC-HAL-004, SPEC-HAL-024, SPEC-HAL-028, SPEC-HAL-030, SPEC-HAL-040, SPEC-HAL-083, SPEC-HAL-083A, SPEC-HAL-084, SPEC-HAL-150, SPEC-R-020, SPEC-R-021

---

## 1. Problem Statement

Halcyon currently has shaders that exist as implementation details rather than an enforced contract:

* WGSL modules and entry points are not standardized as a suite.
* Vertex formats vary by pipeline without a single authoritative schema.
* Bind group layouts are implicit and can drift.
* Color space and premultiplied alpha rules are partially stated but not enforced.
* There is no “shader law” that constrains future renderer work (R-series, clips, images, rounded rects, debug passes).

This causes silent divergence: new visual features accrete one-off shader conventions, which then break batching, compositing correctness, and RenderGraph phase boundaries.

---

## 2. Scope

This SPEC governs:

* The canonical WGSL shader modules that Halcyon is allowed to use for UI rendering.
* Vertex formats, bind group layouts, uniforms, and blend contracts for those modules.
* Mandatory conventions shared across all pipelines (coordinates, color, premultiply).

This SPEC does NOT define:

* RenderGraph topology or pass graph structure (SPEC-HAL-084).
* Compositor behavior beyond “shader-visible” constraints (SPEC-HAL-150).
* Text shaping, paragraph layout, glyph atlas construction (text SPECs).
* High-level widget semantics (widgets produce scene primitives; this SPEC consumes them).

---

## 3. Definitions

**Shader Suite**
A fixed set of WGSL modules and contracts (entry points, vertex formats, bind groups, uniforms) that all renderer pipelines must use.

**Pipeline Family**
A renderer pipeline bound to a shader module and a contract:
Rect / Shape / Path / Text / Image / Clip / Debug.

**Globals**
Uniforms shared by all pipelines that convert Absolute Logical → NDC and carry frame invariants.

**Material Params**
Pipeline-specific uniform payload (small, structured) that modifies behavior without changing entry points.

**Premultiplied RGBA**
Color channels already multiplied by alpha: `rgb = rgb * a`. This is the only allowed blending model for UI.

---

## 4. Canonical Module Set

The shader suite consists of these WGSL modules:

* `src/renderer/shaders/common.wgsl` (required shared code)
* `src/renderer/shaders/rect.wgsl`   (solid + rounded rect + rect border)
* `src/renderer/shaders/shape.wgsl`  (generic shape fills/strokes when used)
* `src/renderer/shaders/path.wgsl`   (path renderer / tessellation output consumption)
* `src/renderer/shaders/text.wgsl`   (glyph atlas sampling; grayscale/subpixel/SDF modes)
* `src/renderer/shaders/text_macos.wgsl` (platform variant: macOS text path only)
* `src/renderer/shaders/image.wgsl`  (bitmap sampling; icons; future 9-slice)
* `src/renderer/shaders/clip.wgsl`   (clip/mask application path)
* `src/renderer/shaders/debug.wgsl`  (overlays/visualizers; never used for UI semantics)

If a pipeline is needed that is not representable by these modules, this SPEC must be amended (or a v2 introduced). No “one-off wgsl in a Rust string” is permitted.

---

## 5. Global Conventions

### 5.1 Coordinate Convention

**INV-SHADER-COORD-1**
All CPU-provided positions and rects feeding shaders are in **Absolute Logical** coordinates per SPEC-HAL-024.

**INV-SHADER-COORD-2**
Only the vertex shader is allowed to convert Absolute Logical → NDC, using the canonical `Globals` transform.

No pipeline may invent its own coordinate space or apply ad-hoc scaling.

### 5.2 Color Space Convention

**INV-SHADER-COLOR-1**
All color math inside shaders is performed in linear space.

**INV-SHADER-COLOR-2**
The swapchain target is treated as sRGB by the presentation pipeline (SPEC-HAL-030). Shader code must not apply double conversions.

If a texture is sRGB, it must be sampled as sRGB (or explicitly linearized once), and never “half-corrected”.

### 5.3 Premultiplied Alpha Convention

**INV-SHADER-ALPHA-1**
All fragment outputs MUST be premultiplied RGBA.

**INV-SHADER-ALPHA-2**
All per-vertex/per-instance colors provided to shaders MUST be premultiplied RGBA.

**INV-SHADER-ALPHA-3**
The default blending model for non-opaque UI primitives is premultiplied source-over:

* color: `src = 1`, `dst = 1 - src.a`
* alpha: `src = 1`, `dst = 1 - src.a`

Any other blend mode requires an explicit, named contract in this SPEC (or a later amendment).

---

## 6. Canonical GPU Interface

### 6.1 Global Uniforms (All Pipelines)

**INV-SHADER-UBO-1**
All pipelines MUST use the same global uniform layout:

```wgsl
struct Globals {
    view_proj    : mat4x4<f32>, // Absolute Logical → NDC
    surface_size : vec2<f32>,   // physical pixels
    time_seconds : f32,         // SPEC-HAL-041
    flags        : u32,         // global debug toggles (must not affect semantics)
};

@group(0) @binding(0)
var<uniform> u_globals : Globals;
````

No pipeline may change `Globals` layout. Additions require a suite-wide revision.

### 6.2 Canonical Vertex Input

The suite supports two canonical data paths. Pipelines MUST choose one and may not invent a third.

**Path A — Instance Rect (UI Quads)**
Used by rect, image, text, clip application, debug overlays.

```wgsl
struct QuadVertex {
    @location(0) local_pos : vec2<f32>, // typically (0..1, 0..1) for quad corners
    @location(1) uv        : vec2<f32>, // used by image/text, ignored by rect
};

struct QuadInstance {
    @location(2) rect      : vec4<f32>, // x,y,w,h in Absolute Logical
    @location(3) color     : vec4<f32>, // premultiplied RGBA
    @location(4) extra0    : vec4<f32>, // pipeline-specific (radius, border, etc.)
    @location(5) extra1    : vec4<f32>, // pipeline-specific (glyph params, etc.)
};
```

**Path B — Baked Vertex (Shape/Path Output)**
Used by shape/path pipelines when geometry is already expanded.

```wgsl
struct BakedVertex {
    @location(0) pos   : vec2<f32>, // Absolute Logical
    @location(1) uv    : vec2<f32>, // optional
    @location(2) color : vec4<f32>, // premultiplied RGBA
    @location(3) extra : vec4<f32>, // optional
};
```

**INV-SHADER-VERT-1**
All pipelines MUST use either Path A or Path B exactly as defined above.

**INV-SHADER-VERT-2**
Any additional attributes beyond `extra0/extra1/extra` are forbidden in v1.

---

## 7. Bind Group Layouts

### 7.1 Group 0 — Global

**INV-SHADER-BIND-0**
`@group(0)` is reserved for suite-wide globals only (`Globals` at binding 0). Additional bindings in group 0 are forbidden in v1.

### 7.2 Group 1 — Pipeline Resources

**INV-SHADER-BIND-1**
`@group(1)` is pipeline-specific and must follow one of these layouts:

Rect pipeline (`rect.wgsl`):

* `@binding(0)` optional material UBO (radius/border/etc.) OR unused if instance extras suffice.

Text pipeline (`text.wgsl`):

* `@binding(0)` glyph atlas texture
* `@binding(1)` sampler
* `@binding(2)` optional material UBO (gamma/subpixel flags/etc.)

Image pipeline (`image.wgsl`):

* `@binding(0)` texture
* `@binding(1)` sampler

Clip pipeline (`clip.wgsl`):

* `@binding(0)` clip mask texture (if mask-based)
* `@binding(1)` sampler
  (If using scissor-only, this pipeline may be bypassed; it is still part of the suite contract.)

Debug pipeline (`debug.wgsl`):

* May mirror rect pipeline resources; it must never require reading SceneCache or any semantic state.

---

## 8. Entry Point Contract

Each module must expose stable entry points. Names are binding.

**INV-SHADER-ENTRY-1**
Each shader module in the suite MUST provide:

* `@vertex fn vs_main(...) -> ...`
* `@fragment fn fs_main(...) -> @location(0) vec4<f32>`

**INV-SHADER-ENTRY-2**
Behavior changes (rounded vs square rect, SDF vs grayscale text) MUST be driven by material flags or instance extras, not by swapping entry point names.

Platform variants are allowed only where explicitly listed (`text_macos.wgsl`), and must still use `vs_main/fs_main`.

---

## 9. Pipeline-Specific Contracts

### 9.1 Rect Pipeline (`rect.wgsl`)

Purpose:

* Solid rectangles
* Rounded rectangles (SDF-based or analytic; v1 chooses one, but the interface stays stable)
* Optional rect border / outline in v1

**INV-SHADER-RECT-1**
Rect pipeline must support:

* solid fill
* per-instance corner radius (uniform radius in v1)
* per-instance opacity via premultiplied color alpha

**INV-SHADER-RECT-2**
Rounded behavior must not change geometry ownership. Shader only affects coverage inside the already-owned rect.

### 9.2 Text Pipeline (`text.wgsl`)

Purpose:

* Glyph atlas sampling (grayscale today; hooks for subpixel/SDF as modes)

**INV-SHADER-TEXT-1**
Text shader output must be: `premultiplied_color * coverage`.

**INV-SHADER-TEXT-2**
Mode selection (grayscale/subpixel/SDF) must be controlled by material flags. No separate pipelines for each mode in v1 unless batching requires it and it’s declared here as a PipelineKind split.

### 9.3 Image Pipeline (`image.wgsl`)

Purpose:

* Bitmap sampling for icons and images

**INV-SHADER-IMG-1**
Sampled texel must be treated as premultiplied RGBA at output time. If inputs are not premultiplied, premultiplication must occur exactly once (upload path preferred; shader fallback allowed).

### 9.4 Clip Pipeline (`clip.wgsl`)

Purpose:

* Mask application when scissor is insufficient (rounded clips, nested clips, future clip stack)

**INV-SHADER-CLIP-1**
Clip shader must only attenuate coverage. It must not modify coordinates, reorder layers, or invent geometry.

### 9.5 Debug Pipeline (`debug.wgsl`)

Purpose:

* Visual inspection only (overdraw heatmap, mask view, atlas view, etc.)

**INV-SHADER-DBG-1**
Debug rendering must be gated (feature flag or debug build) and must never be required for correct UI output.

---

## 10. Phase Separation Constraints (RenderGraph Compatibility)

RenderGraph v2 phase walls are law (SPEC-HAL-084). Shader suite must not punch holes through them.

**INV-SHADER-PHASE-1**
Shader modules and pipeline layouts are created at initialization or during an explicit renderer “pipeline build” step that is not the Execute phase.

**INV-SHADER-PHASE-2**
No shader compilation, pipeline creation, or bind group layout mutation is allowed during Execute (command encoding only).

**INV-SHADER-PHASE-3**
Shader-visible inputs must come only from the frozen scene/draw plan products of upstream phases (no direct reads of AppState/WidgetTree/SceneCache).

---

## 11. Prohibited Patterns (Explicit)

**FORBIDDEN-SHADER-1**
Inline WGSL shader strings embedded in Rust code for production pipelines.

**FORBIDDEN-SHADER-2**
Per-feature bespoke vertex formats outside the canonical schemas (Section 6.2).

**FORBIDDEN-SHADER-3**
Non-premultiplied output blending for UI primitives.

**FORBIDDEN-SHADER-4**
Shader paths that “fix” logical bugs visually (violates compositor/render truth; see SPEC-HAL-150).

**FORBIDDEN-SHADER-5**
Debug shader usage for UI semantics (e.g., relying on debug overlay for readability).

---

## 12. Enforcement Strategy

Enforcement must be mechanical, not “remembered”.

### 12.1 Compile-Time / Type Enforcement (Required)

**ENF-SHADER-1**
Introduce a `PipelineKind` enum that enumerates only the shader suite pipelines. Renderer dispatch must be exhaustive over this enum.

**ENF-SHADER-2**
Centralize pipeline creation in a single registry with no external constructors.

**ENF-SHADER-3**
Bind group layouts must be defined once per PipelineKind and reused. No ad-hoc bind group layouts per call site.

### 12.2 Tests (Required)

**ENF-SHADER-TEST-1**
A test that asserts the expected WGSL module files exist (suite completeness).

**ENF-SHADER-TEST-2**
A test that asserts vertex buffer layouts match the canonical schemas for each PipelineKind.

**ENF-SHADER-TEST-3**
A test that asserts blend states for non-opaque pipelines are premultiplied source-over unless explicitly declared otherwise.

---

## 13. Acceptance Criteria

SPEC-HAL-085 is satisfied when:

* [ ] The shader suite module set exists at the canonical paths.
* [ ] All production pipelines use `vs_main/fs_main` entry points from suite modules.
* [ ] Vertex formats conform to Section 6.2 with no deviations.
* [ ] Bind group layouts conform to Section 7 with no deviations.
* [ ] Premultiplied alpha contract is enforced by blend state and test coverage.
* [ ] Execute phase performs command encoding only (no pipeline/shader creation).
* [ ] Any new shader/pipeline addition requires an SPEC amendment (no silent additions).

---

## 14. Final Statement

The shader suite is a contract, not a convenience layer.

If a visual feature cannot be expressed under this contract, Halcyon does not “just add a shader” — it updates the law first.
