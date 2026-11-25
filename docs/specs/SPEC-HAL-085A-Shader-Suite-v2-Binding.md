## **SPEC-HAL-085A — Halcyon Shader Suite v2 (Binding)**

**Status:** Binding
**Amended:** 2026-09-21 — The current grayscale text resource group excludes unused background bindings; future LCD/subpixel work requires new authority.
**Supersedes:** SPEC-HAL-085 (Historical)
**Scope:** Renderer / Shader Interface Contracts
**Audience:** Renderer, GPU, Pipeline authors
**Non-goals:** Geometry semantics, clipping policy, widget behavior
**Amended by:** SPEC-HAL-263A (Adopted / Binding; Image-only input-association and modulation correction to §6.4; §7 remains unchanged)
**Extended by:** SPEC-HAL-355 (Adopted / Binding; separate admitted 3D shader pipeline under §10 without changing the renderer-owned 2D shader suite)
**Extended by:** SPEC-HAL-358 (Adopted / Binding; bounded luminous-contour
parameter use inside the existing Shape pipeline)

---

## 1. Purpose

This SPEC defines the **authoritative, binding shader contract** for Halcyon **as implemented**, not as aspirational design.

It replaces SPEC-HAL-085, which is now historical and non-enforcing.

The goal is to:

* Codify **existing, correct shader behavior**
* Remove false global invariants
* Make per-pipeline differences explicit
* Prevent future accidental breakage

No code changes are implied or required by this SPEC.

---

## 2. Authority Boundary

This SPEC governs **shader-level contracts only**:

* WGSL module set
* Entry points
* Bind group layouts
* Vertex formats
* Coordinate space expectations
* Alpha output rules

This SPEC explicitly does **not** govern:

* Geometry generation
* Clipping semantics
* Scene graph behavior
* Widget behavior
* Visual meaning of primitives

Those are defined elsewhere (e.g. SPEC-R-020, SPEC-R-021, layout SPECs).

---

## 2.1 Enforcement Scope

* SPEC-HAL-085A is the sole binding shader contract.
* Enforcement applies to shader modules/pipelines that are wired/compiled and used by the renderer.
* Pipelines not yet wired/compiled remain deferred and require their owning admission. Image is already governed by adopted SPEC-HAL-263A/263B and §6.4; the former Image deferral and INV-IMG-ALPHA-1 reference are obsolete.

---

## 3. Canonical Shader Modules

The following WGSL modules are canonical and mandatory:

| Module     | Path                                            |
| ---------- | ----------------------------------------------- |
| common     | `src/renderer/shaders/common.wgsl`              |
| rect       | `src/renderer/shaders/rect.wgsl`                |
| path       | `src/renderer/shaders/path.wgsl`                |
| text       | `src/renderer/shaders/text.wgsl`                |
| text_macos | `src/renderer/shaders/text_macos.wgsl`          |
| image      | `src/renderer/shaders/image.wgsl`               |
| clip       | `src/renderer/shaders/clip.wgsl`                |
| debug      | `src/renderer/shaders/debug.wgsl`               |
| shape      | `src/renderer/shaders/shape.wgsl` (placeholder) |

No module may be removed or repurposed without an SPEC amendment.

---

## 4. Entry Point Contract (Binding)

All shader modules MUST expose:

```
@vertex   fn vs_main(...)
@fragment fn fs_main(...) -> @location(0) vec4<f32>
```

Naming and return location are fixed.

---

## 5. Global Uniform Contract (Binding)

### 5.1 Bind Group 0

* `@group(0) @binding(0)` is reserved **exclusively** for `Globals`
* No other bindings are permitted in group(0)

### 5.2 Globals Layout

The `Globals` uniform buffer is **byte-exact** and fixed at **80 bytes**:

| Field        | Type        | Size         |
| ------------ | ----------- | ------------ |
| view_proj    | mat4x4<f32> | 64           |
| surface_size | vec2<f32>   | 8            |
| time_seconds | f32         | 4            |
| flags        | u32         | 4            |
| **Total**    |             | **80 bytes** |

This layout MUST match Rust exactly.

---

## 6. Pipeline-Specific Contracts (Binding)

There is **no global vertex format**.
Each pipeline defines its own authoritative interface.

### 6.1 Rect Pipeline (`rect.wgsl`)

**Vertex attributes**

* `@location(0)` position (logical, absolute)
* `@location(1)` color

**Coordinate transform**

* Shader converts Absolute Logical → NDC using `Globals.view_proj`

**Bind group 1**

* Optional material / rounded-clip UBO

**Alpha**

* Output MUST be premultiplied

---

### 6.2 Path Pipeline (`path.wgsl`)

**Vertex attributes**

* Custom path vertex + instance format
* Includes transform rows, clip id, stroke flags

This format is **authoritative** and intentionally not shared.

**Coordinate transform**

* Shader converts Absolute Logical → NDC using `Globals.view_proj`

**Bind group 1**

* Storage buffers for path data and clip data (pipeline-specific)

**Alpha**

* Output MUST be premultiplied

---

### 6.3 Text Pipelines (`text.wgsl`, `text_macos.wgsl`)

**Vertex attributes**

* Position
* UV
* Color
* Screen-space UV
* Downsample / coverage control

**Coordinate transform**

* **CPU provides NDC directly**
* Shader MUST NOT apply `view_proj` to position

This is intentional for pixel-stable text placement.

**Bind group 1**

* `@binding(0)` atlas texture
* `@binding(1)` atlas sampler
* `@binding(2)` text parameter UBO

This three-binding order is the current binding contract for both text shaders. Current rendering consumes grayscale atlas coverage and has no background-reconstruction input.

SPEC-HAL-064 and SPEC-HAL-067 remain future, not-implemented LCD/subpixel designs. A future RGB-coverage or background-reconstruction path is permitted only through new binding authority or an amendment under §10; this contract does not declare that work impossible.

**Alpha**

* Output MUST be premultiplied
* Coverage modulation is permitted

---

### 6.4 Image Pipeline (`image.wgsl`)

> **Adopted amendment:** SPEC-HAL-263A supplies the binding §6.4
> input-association and modulation correction while retaining
> premultiplied-linear sampled texture input and premultiplied fragment output.

**Vertex attributes**

* Position
* UV
* Color (straight linear tint)

**Coordinate transform**

* Shader converts Absolute Logical → NDC using `Globals.view_proj`

**Bind group 1**

* texture
* sampler

**Alpha**

* Output MUST be premultiplied

**Image Modulation Formula (Binding)**

Let `T` be the premultiplied-linear sampled texel, `C` the straight-linear tint,
and `K` the final coverage. The Image fragment source is:

```
source.rgb = T.rgb * C.rgb * C.a
source.a   = T.a   * C.a
output     = vec4(source.rgb, source.a) * K
```

Texture alpha contributes once because it is already present in `T.rgb` and is
used once in `source.a`. Tint alpha contributes once to source RGB and source
alpha. Coverage contributes once to all four output components. The result is
premultiplied linear RGBA.

---

### 6.5 Clip Pipeline (`clip.wgsl`)

**Vertex attributes**

* Position
* UV
* Color

**Coordinate transform**

* Shader converts Absolute Logical → NDC using `Globals.view_proj`

**Bind group 1**

* mask texture
* sampler

**Alpha**

* Output MUST be premultiplied

---

### 6.6 Debug Pipeline (`debug.wgsl`)

Used for visualization only.

Standard rect-style semantics apply.

---

### 6.7 Shape Pipeline (`shape.wgsl`)

Active analytic SDF module for the constrained SPEC-R-003 primitive set.

* Entry points: `vs_main`, `fs_main`
* Bind group 0: renderer `Globals` uniform at binding 0
* Coordinate transform: Absolute Logical -> NDC using `Globals.view_proj`
* Vertex locations:
  * `@location(0)` position: `vec2<f32>`
  * `@location(1)` local position: `vec2<f32>`
  * `@location(2)` color: `vec4<f32>`
  * `@location(3)` params0: `vec4<f32>`
  * `@location(4)` params1: `vec4<f32>`
  * `@location(5)` params2: `vec4<f32>`
  * `@location(6)` params3: `vec4<f32>`
  * `@location(7)` contour gains: `vec2<f32>`
  * `@location(8)` shape type: `u32`
* Shape IDs:
  * `0` circle
  * `1` ellipse
  * `2` line segment
  * `3` polyline segment
  * `4` luminous contour segment
* Line/polyline `params1` lanes:
  * `x`: core half-width in logical pixels
  * `y`: luminous halo radius beyond the core edge in logical pixels
  * `z`: luminous halo intensity
  * `w`: luminous halo falloff exponent
* Existing ordinary analytic shape data keeps the exact `params1` packing
  `[half_width, 0.0, 0.0, 0.0]`, with zero `params2`, `params3`, and contour
  gains. SPEC-HAL-358 defines the bounded ownership-plane, zoned-material, and
  front-gain meanings for shape ID `4`; it does not hide a flag in float bits,
  padding, or color.
* Output MUST be premultiplied alpha after SDF coverage modulation
* Renderer primitive-buffer generation and draw routing remain separate from this shader-binding contract

---


### 6.7.1 Scoped HAL-388 S1 Local Ambient Glow (Adopted / Binding 2026-11-13)

Add `ShapeType=5 LocalAmbientGlow` inside the existing `shape.wgsl` ShapePipeline; do not add another GPU pipeline. Its `ShapeVertex` ABI, 112-byte stride and nine attributes remain unchanged, as do current shader entry points and Globals. Type-5-only packing: `color`=lobe0 RGBA, `params0`=lobe0 center xy/radii xy, `params1`=lobe1 center xy/radii xy, `params2`=lobe1 RGBA, `params3`=cos0/sin0/cos1/sin1, `contour_gains`=intensity0/intensity1. Absent lobes have zero values; type 5 dispatch occurs before the existing ordinary SDF/zoned/HAL-358 branches, and performs exactly HAL-388 §7.4's bounded commutative two-lobe output. Other shapes keep byte-equivalent ABI/semantic lanes and shader behavior. Current target/clip/scissor, RGB linear premultiplication, Z and retained ownership remain unchanged. No texture, binding, shader stage, pipeline id, time-driven glow or global activation is admitted.

### 6.8 `gradient.wgsl`: rectangular Gradient profile

This scoped amendment defines the existing linear/radial rectangular Gradient
pipeline and its current clip projection. It does not adopt the remaining
Design-status SPEC-R-012 proposals or admit arbitrary materials or clip topology.

* Entry points: `vs_main`, `fs_main`.
* Vertex location 0: absolute logical position, `vec2<f32>`; transform through
  `Globals.view_proj`. Group 0 remains the unchanged canonical Globals ABI.
* Group 1 binding 0: dynamic `GradientParams` uniform. Its ordered fields are
  `rect_origin`, `rect_size`, `p0`, `p1` (four `vec2<f32>`), then
  `corner_radius: f32`, `kind: u32`, `stop_count: u32`, `clip_enabled: u32`,
  eight 32-byte GradientStop records, and one 80-byte RoundedClip record.
  The uniform is 384 bytes in the existing 512-byte aligned slot.
* A GradientStop is premultiplied linear `vec4<f32>`, offset `f32`, and three
  padding scalars. Input `Color` is already straight linear RGBA. CPU lowering
  MUST multiply RGB by alpha exactly once and MUST NOT decode it as sRGB again.
  Interpolation and fragment output remain premultiplied linear.
* RoundedClip uses the existing Rect/Image projection: rectangle and four
  radii (`vec4<f32>` each), inner and outer min/max (`vec2<f32>` each),
  logical AA radius and three padding scalars. Compile MUST derive it from the
  primitive's current effective clip and the existing renderer AA policy.
  The parameter key MUST include that clip dependency.
* CPU scissor remains active. With the existing rounded-clip execution flag
  enabled and `clip_enabled != 0`, the shader MUST apply the existing
  Rect/Image rounded coverage law, including asymmetric corners, to all four
  premultiplied channels. Own-radius coverage and parent coverage are independent.
  The existing diagnostic flag semantics are unchanged.
* This applies in each admitted render layer/target; it does not infer support
  for additional nested clip topology beyond the existing effective projection.
* Clip storage MUST remain within Gradient-owned parameter slots. Gradient use
  MUST NOT consume the bounded shared Rect/Image clip slots, add a GPU pass, or
  add allocation/resource preparation to Solid-only paths.

---


### 6.9 Adopted analytic Smoked Glass / reflective rim S1 (2026-11-13)

HAL-386 section 8 is the complete checked mechanical contract for the first SurfaceAnalytic face; HAL-389 section 7 makes its directional rim independently optional. Add only PipelineId::AnalyticSurface=13 and SceneNodeKind::AnalyticSurface=23. Use one direct pipeline in the current Main/Overlay passes, vertex location 0 absolute logical vec2<f32>, vs_main/fs_main, unchanged group 0 Globals and one dynamic uniform at group 1 binding 0. No textures, samplers, offscreen producer, new pass or Shape 0..5 modification.

The exact AnalyticSurfaceParams layout is 384 bytes: rect 0..15; gradient endpoints 16..31; three straight-linear opaque base colors 32..79; rim RGBA 80..95; own four-corner radii 96..111; bevel-width/slope/roughness/reserved-zero 112..127; diffuse/specular/rim/ambient strengths 128..143; uplift/opacity/zero/zero 144..159; two 64-byte light records 160..223 and 224..287; clip_control u32x4 288..303; existing 80-byte RoundedClipUniform layout 304..383. Slot stride is align_up(512, actual device uniform-offset alignment); padding is zero and not shader-visible input. This replaces the inactive HAL-384 368-byte opaque-only proposal for this selected profile; it does not alter existing Gradient uniforms.

Evaluate the four-corner SDF, synthetic normal, bounded two-light face and guarded luminance/gamut arithmetic of HAL-386 section 8.5. Apply its source-direction-dependent rim weight from section 8.6, never the old constant rim-color mix. Final output is C*(opacity*own_AA*ancestor_coverage), opacity*own_AA*ancestor_coverage in linear-premultiplied RGBA. The captured single original rounded ancestor uses the existing rounded coverage equation, without silently bypassing it through the old diagnostic flag. Rectangular scissor remains separate. Quad AA padding is 2/current_DPR; no extra halo, alpha boost, sRGB decode, dither or backdrop sample.

Compile assigns checked frame-local dynamic slots to actual (SurfaceId, surface-local analytic ordinal) pairs, with the complete 512-occurrence bound already admitted at Build. Pipeline 13 uses the existing parameter-selector key lane as an ordinal only; no Image registration or hash-based semantic alias. Both new uniform and geometry storage remain renderer-owned and lazy for unused analytic scenes. No existing pipeline ABI or output is changed.

## 7. Premultiplied Alpha Rule (Binding)

All fragment shaders MUST output **premultiplied alpha**:

```
(r * a, g * a, b * a, a)
```

Pipeline-specific coverage modulation is allowed **only** if the final output remains premultiplied.

Any deviation MUST be explicitly documented in this SPEC.

---

## 8. Explicit Non-Invariants

The following are **explicitly not guaranteed**:

* Shared vertex formats across pipelines
* Uniform bind group(1) layouts
* Shader-side coordinate transforms for all pipelines
* Visual semantics of rounding, clipping, or AA
* Compatibility with historical SPEC-HAL-085 rules

---

## 9. Relationship to Other SPECs

* SPEC-HAL-085: Historical, non-binding
* SPEC-HAL-064 / SPEC-HAL-067: Future, not-implemented LCD/subpixel designs
* SPEC-R-020 / SPEC-R-021: Clipping & RoundedRect authority
* Layout / coordinate SPECs define **what data is fed**, not shader contracts

Shader behavior MUST respect those SPECs but is not defined by them.

**Proposed relationship to SPEC-HAL-363/364 (2026-10-05):** The paired
documents remain Proposed pending joint adoption. This annotation changes no
declared status, implemented scope or substantive law in this document.

SPEC-HAL-364 B-6.13 preserves §10's ABI gate: changing vertex format, bindings,
shader set/entry points or coordinate-transform expectations requires an exact
owning amendment and compatibility proof before profile execution admission.
The current text CPU-NDC and premultiplied-output rules remain unchanged.
A generic transform or mask descriptor grants no shader capability.

---

## 10. Amendment Rules

Any of the following require a new SPEC or amendment:

* Changing vertex formats
* Changing bind group layouts
* Changing coordinate transform expectations
* Introducing new shader pipelines

No silent drift is permitted.

---

## 11. Final Status

**SPEC-HAL-085A is the sole binding shader contract.**
SPEC-HAL-085 is frozen and historical.

### 6.9.1 Adopted HAL387 S1 material micrograin extension (2026-11-13)

HAL387 section7 is binding for a single extension of the existing analytic surface shader. AnalyticSurfaceParams becomes416 bytes while bytes0..383 retain the exact HAL386 meanings and offsets. Bytes384..399 are micrograin:vec4<f32>=(scale_px,amplitude_srgb,0,0); bytes400..415 are micrograin_seed:vec4<u32>=(seed,0,0,0). Both appended fields are zero for absent/zero grain. Update only this family's384-byte binding-size checks to416; the existing align_up(512,device_alignment) stride and storage capacity remain unchanged.

The exact material-local pixel-footprint integral and u32 lattice field in HAL387 section7.3 run only for positive amplitude. Section7.4 specifies the guarded reference-sRGB luminance operation on lit S before the existing optional rim mix. Preserve zero-grain shader arithmetic, opacity, own/ancestor coverage, strict Z and premultiplied output. No new texture, sampler, bind group, vertex attribute, pipeline, target, pass or per-widget GPU resource. Reference contrast units do not identify the monitor profile or certify physical output. Existing Gradient anti-banding and Shape0..5 shaders are unchanged.

## Adopted HAL-385 S1 backdrop shader profile (2026-11-13)

HAL-385 section 8 is the binding first Frosted Glass shader/graph contract. SceneNodeKind24 and PipelineId14 are checked composition-marker identities, not an ordinary direct-draw fallback. Four renderer-owned modules, frosted_glass_horizontal.wgsl, frosted_glass_vertical.wgsl, frosted_glass_replace.wgsl and frosted_glass_terminal.wgsl, use vs_main/fs_main, unchanged group0 Globals, group1 binding0 FrostedGlassParams and read-only unfilterable-float texture inputs at bindings1/2. No sampler or consumer shader source is admitted.

The exact uniform is 1040 bytes: own rect0..15, corners16..31, tint32..47, transmission/sigma_p/DPR/zero48..63, target u32x4 64..79, V80..95, S96..111, kernel/clip control112..127, optional grain128..159, existing RoundedClipUniform160..239, reserved zeros240..255 and 49 vec4 weight records256..1039. HAL-385 section8.7 defines each lane. One immutable occurrence record uses no dynamic offset. The Gaussian uses integer textureLoad, exact normalized finite taps and Rgba16Float intermediates. Replacement and final transfer disable blending; ordinary content retains existing premultiplied OVER pipelines and formats.

The only grain reuse is the pure HAL-387 footprint/colour operation applied to tint before premultiplication, never to the filtered source. A shared helper receives explicit values and preserves the old analytic shader's arithmetic and zero-grain behavior. This does not change Shape0..5, the 416-byte AnalyticSurface ABI, Gradient dither, old bindings or control defaults. All source/sink, coverage and lifetime requirements remain those of HAL-385/HAL-084/HAL-040.

## Adopted S2 configurable optical coating and connected emission ABI (2026-11-14)

HAL386 section11, HAL385 section11, HAL389 section8 and HAL388 section10 are the binding coordinated extension. There is NO new pipeline, vertex format, texture/sampler binding, scene kind, global uniform or hidden effect mode. The existing analytic and frosting shader owners assemble one shared pure glass_optics.wgsl helper and one shared pure surface_emission.wgsl helper using explicit values. Existing micrograin.wgsl remains the only grain arithmetic owner. Old Shape0..5, Gradient, text, images, Button and HAL358 shaders keep their behavior.

GlassSurfaceOpticsParams is192 bytes: shape:vec4=(bevel_width,bevel_slope,roughness,normal_reflectance) at0; response:vec4=(reflection_strength,rim_strength,rim_falloff,0) at16; reflection_tint RGBA at32; rim_tint RGBA at48; two existing64-byte analytic light records at64 and128. SurfaceEmissionParams is64 bytes as defined in HAL388 section10. Checked-absent values pack zero; evaluate neither absent helper nor its divisions.

AnalyticSurfaceParams becomes688 bytes. Retain old bytes0..415 exactly, append extensions:u32x4=(optics_present,emission_present,0,0) at416..431, optics192 at432..623, emission64 at624..687. The dynamic stride becomes align_up(688,device.min_uniform_buffer_offset_alignment), which is768 for256-byte alignment. Update parameter/binding buffer sizes, dynamic offsets, descriptor checks and actual (SurfaceId,ordinal) tests in the same unit; never keep512-byte overlap. Capacity stays512. With neither extension, the original face/grain/legacy-rim shader operations remain identical. Emission expands the quad only by its admitted halo support and existing AA margin; canonical rect/radii remain in the unchanged prefix. Apply own/ancestor coverage exactly as HAL388 defines.

FrostedGlassParams becomes1344 bytes: old1040-byte prefix; filtered domainQ at1040; opacity/refraction/zero/zero at1056; presence u32x4 at1072; optics192 at1088; emission64 at1280. Each occurrence remains a nondynamic uniform. All four frost shader modules agree. Existing textureLoad Gaussian arithmetic remains; the vertical pass writes Q, and only nonzero refraction uses manual bilinear F reconstruction. Zero refraction keeps exact integer samples. Replacement includes the new bounded four-channel coating/emission while using the original expression for the exact old default tuple. No extra pass, HDR output, sRGB re-encoding, unbounded read, stale texture or swapchain source.

HAL387 section9 changes only the public reference-amplitude validation bound; the shared WGSL field/colour algorithm and existing storage meanings stay. This amendment qualifies source layout and intended math, not implementation, GPU execution or visual acceptance.
## 2026-11-18 coordinated scoped shader and optical ABI adoption: HAL-393/394/395 [S1 Binding]

HAL-393 uses the EXISTING two checked material-light records and group0/1 binding ABI; light motion does not require a new pipeline or time source. HAL-394's accepted optional GlassSurfaceOptics thin-film surrogate appends exactly one vec4<f32> film=(strength,thickness_nm,film_ior,0) to the established material records: AnalyticSurfaceParams 688->704 bytes, FrostedGlassParams 1344->1360 bytes. Old prefixes and absent-film WGSL output remain byte/behavior equivalent; one shared glass-optics evaluator owns the three-wavelength law.

HAL-395's first independently selected Bloom material adds controlled renderer-owned BloomSource (304-byte dynamic UNIFORM and group0 Globals), BloomHorizontal/BloomVertical (848-byte non-dynamic UNIFORM plus one nonfilterable Rgba16Float texture at group1 binding1), and BloomComposite (144-byte non-dynamic UNIFORM plus the completed Rgba16Float filtered texture at group1 binding1). Four controlled vs_main/fs_main fullscreen-triangle pipelines and exact offsets, shader data, no-sampler rule, alpha/color blend and selected formats are binding as in HAL-395 section 9.4. The original 688/1344 record remains intact for the Bloom-only profile; the separate HAL-394 film record is not automatically mixed into its optical source. Source and two filters output to Rgba16Float with blend disabled; the final SDR composite uses only its declared source and target blend state; it never samples a swapchain or destination attachment. This is a narrow accepted versioned extension to sections 6/10, not an alteration of any prior pipeline, SHAPE, text, Image or old shader binding. Parent section 7's premultiplied-color rule remains: an explicitly HDR-linear radiance source may exceed alpha in RGB magnitude without changing its coverage association. Physical HDR display/3D R-027 are not admitted. These are binding implementation requirements, not claims of currently wired shader modules or backend PASS.

## Adopted HAL-385 inward Edge Feathering ABI (2026-11-25)

HAL-385 section 12 assigns FrostedGlassParams.surface_controls.z (byte 1064) to the checked edge_feather_logical value, replacing only the previously reserved zero at that lane. The record remains exactly 1360 bytes, with surface_controls=(coverage_opacity,refraction_logical,edge_feather_logical,0). The original S1/S2 arithmetic is unchanged at zero; positive values apply the section 12 inward smoothstep only to the replacement body weight. No changes to other material records, independent emission/Bloom source equations, resource bindings, graph topology or shader interfaces are admitted.
