# SPEC-HAL-395: Selective Bloom and HDR UI Composition

**Authoring instruction:** JC, 2026-11-18: examine Bloom and HDR for bright moving colored reflections on existing Halcyon 2.5D/PBR.
**Status:** ADOPTED / BINDING [S1 first selective Bloom with internal HDR radiance and SDR presentation], 2026-11-18. Section 10 defines the operative scope. Section 11 records implementation and verification.
**Scope:** Future renderer-owned selective light spread on actual 2.5D material contributions, with internal HDR and physical HDR-display output strictly separated.
**Owners:** HAL-012 appearance, HAL-040 GPU resources, HAL-084 RenderGraph, HAL-085A shader color/ABI, HAL-150 compositor, HAL-359 Z, HAL-385 same-surface backdrop. R-027 remains a separate 3D private output contract.
**Companions:** HAL-393 specular motion; HAL-394 iridescence. Neither automatically creates Bloom or HDR.

## Pre-Flight

- Objective: Write the exact optical objective, existing source limitations and owner-correct future HDR/Bloom graph admission, without replacing the functioning SDR renderer.
- Standing objective: Intersecting reflected highlights may become bright enough for a local halo while sharp text, other surfaces and disabled effects remain correct.
- Continuation disposition: Complete document only. Later numerical, ABI, graph, capability and native acceptance require independent decisions.
- Governing authority and status: Constitution supreme; existing owner laws binding only in their scopes. This is inactive and does not amend any existing source or contract.
- Authority change, if any: NONE.
- Target layer and existing state verified: Existing analytic/glass sources and HAL-388 emission produce bounded display-linear premultiplied SDR output. HAL-388 Local Ambient Glow is finite alpha-union, not physical Bloom. R-027 private 3D HDR stores four R32Float scene channels, tone maps before publication of Rgba16Float display-LDR. It explicitly is not HDR display output.
- Owned mechanisms and files: One new SPEC only; no shader, render graph or display source changes.
- Named invariants: INV-395-001 through INV-395-011 and FORBID-395-001 through FORBID-395-007.
- Forbidden behavior: Swapchain read, hidden pass, widget-owned GPU effects, fake HDR from FP16 storage, cross-surface sampling, unbounded blur, global text bloom, changed focus/geometry.
- Existing mechanisms inspected and reuse decision: HAL-084 Build/Compile/Execute, HAL-040 resource owner, HAL-385 acyclic same-surface prefix and HAL-359 order. Reuse, never parallel compositor or custom renderer.
- Relevant external or dependent boundaries: HDR monitor presentation requires separate actual OS/device/color capability and output policy; OS is transport only. HAL-393 motion and HAL-394 color remain independent.
- Test ownership and placement: Existing renderer_z_order/RenderGraph, optical material CPU/GPU suites and ordinary native material lab after lawful adoption; distinct HDR-display hardware proof only if admitted.
- Output or artifact completeness boundary: One complete nonbinding first-consumer candidate covering optical math, appearance ingress, graph, ABI, failure/readiness and visual proof.
- SemVer and changelog impact: N/A for S0 text only.
- Out of scope: Full-screen glow default, dynamic GI, caustics, reflected illumination of unrelated materials, ray tracing, global HDR compositor and rewriting the R-027 3D renderer.

## Halcyon Pre-Flight

- Governing SPEC and read status: HAL-002/002A, HAL-040/084/085A/150/359/385/388, HAL-370/R-027 separate private scope.
- Widget policy or N/A: N/A; optical presentation does not alter behavior.
- Interaction added and policy authority: None.
- Applicable FORBID rules: HAL-002 immutable read-only GPU snapshot; HAL-084 explicit graph/no hidden GPU passes/swapchains sinks; HAL-150 no semantics; HAL-359 strict Z; HAL-040 resource lifetime.
- Existing framework mechanism inspected: SurfaceAnalytic, GlassSurfaceOptics, SurfaceEmission, LocalAmbientGlow, current Frosted graph and R-027 private HDR output.
- Reinvention risk: Unacceptable if a widget creates a second blur pass, scene reader, scheduler, Device/Queue or own render graph.
- Demo/example compliance: The existing glass lab remains the appropriate future native optical consumer. No demo-only hidden pipeline.

## 1. Three separate claims

**Internal HDR:** Finite material light/radiance can exceed nominal SDR white BEFORE tone/gamut mapping. A floating-point storage format is not proof that overbright values survived material evaluation.

**Selective Bloom:** Explicit eligible source contribution is extracted, spatially filtered and composed at the lawful source/foreground boundary. A visually soft HAL-388 halo is not automatically Bloom, and Bloom is not physical illumination of neighbor materials.

**HDR display:** A terminal output format, primaries, transfer and actual HDR-capable display are correctly negotiated and measured. Neither the R-027 FP16 front nor a wide-gamut SDR presentation proves this.

The first visual objective is two differently colored reflected sources: in isolation they remain distinct; on approach/overlap their selected combined bright region develops a smooth localized light spread; after separation the spread weakens/disappears; all ordinary text and controls remain sharp.

## 2. Source reality

The active analytic shader caps its face luminance and normalizes gamut. The coating shader blends bounded reflected colors. HAL-388 SurfaceEmission is premultiplied [0,1], and Local Ambient Glow uses bounded alpha-union. These are real present features but cannot be retroactively called overbright scene radiance. A future HDR material contribution needs its own checked immutable energy/color representation BEFORE these caps, with an explicit mapping to current display color.

R-027's distinct 3D source uses four R32Float scene attachments, exposure/tone per sample, then a linear Rgba16Float display-LDR front. Its closed private output identity, source/lease authority and no-HDR-display statement remain unchanged. Importing the format or tone function into 2.5D would not authorize a new UI HDR target.

## 3. Prospective invariants

**INV-395-001 SOURCE AUTHORITY:** Only an explicitly authored material/appearance value can mark a light contribution as Bloom-eligible or supply scene radiance. Renderer must not detect UI importance from a bright text pixel or inferred user intent.

**INV-395-002 COLOR DOMAINS:** Source-linear radiance, final material display color and target output transfer are distinct declared domains. An HDR field cannot reinterpret existing [0,1] premultiplied material RGBA as uncapped energy.

**INV-395-003 LOCAL OPT-IN VALUE:** Bloom eligibility is an ordinary explicit per-material visual selection, not a feature-gate switch or environment mode. Zero/absent contribution reproduces the original scene and creates no unnecessary pass/allocation.

**INV-395-004 GRAPH:** All renderer-owned extraction, filtering and combination are explicit acyclic HAL-084 RenderGraph nodes with declared dependencies and resource lifetimes. No hidden encoder/submission, in-place feedback or borrowed widget resource.

**INV-395-005 SOURCE PREFIX:** The first prospectively admitted scope is a completed same-SurfaceId compositor prefix or independently admitted texture with clear producer/consumer custody. No sampling another SurfaceId, an unrelated overlay, OS window, swapchain or previous-frame screenshot as current illumination.

**INV-395-006 STABLE ORDER:** Bloom cannot blur/read higher-Z foreground text/focus, modify HAL-359 strict order or alter material hit-test, scroll, accessibility, layout or capture geometry. Expanding halo coverage must respect real clipping and retained damage.

**INV-395-007 LINEAR MATH:** Finite source accumulation occurs in one declared linear working space, then one validated tone/gamut/alpha projection. No hidden second transfer, alpha premultiplication, gamma-space addition or per-channel clamp hiding invalid source energy.

**INV-395-008 INDEPENDENCE:** Reflections, HAL-394 iridescence, SurfaceEmission, LocalAmbientGlow, HAL-358 contour and HAL-385 Frosted blur are separate. None silently enables or substitutes another.

**INV-395-009 LIFETIME:** Actual physical intermediates obey HAL-040 pool/budget and HAL-084 frame graph ownership. Resize/DPR/clip/target failure/unmount prevent stale source reuse or false success. Invalid admission yields typed refusal.

**INV-395-010 QUIET IDLE:** Static material does not produce a continuous repaint. Moving sources use HAL-021 through HAL-393's proposed owning Update; Bloom never creates its own clock or GPU-driven app state.

**INV-395-011 DISPLAY TRUTH:** HDR monitor output requires separate adopted target color-space/transfer/alpha/peak policy and physical native proof. FP16, operating-system HDR mode and perceived brightness are not equivalent to verified HDR presentation.

## 4. Technical decisions reserved for any S1 adoption

Before implementation, the owning adopted profile must specify: checked radiance units and maximum per source; precise overbright extraction or authored source-mask rule; threshold/soft-knee; kernel/support and downsample/blur shape; compositing equation and source alpha; maximum outsets and resource budgets; placement relative to material/foreground and Frosted current-prefix; SDR tone/gamut policy; error and resize response; and exact shader/RenderGraph ABI. No executor may fill these with habitual Bloom defaults.

HDR display has a separate decision boundary: actual supported GPU/OS target formats, working and output primaries, transfer function, peak-luminance metadata, blending with ordinary UI, tone conversion and observed native display. A normal SDR device must remain correct; there is no hidden simulated-HDR fallback success claim.

## 5. Prohibitions

- **FORBID-395-001:** No sampling swapchain or another surface's presented pixels to fabricate current lighting.
- **FORBID-395-002:** No ad-hoc offscreen pass, second renderer/Device/Queue or widget-owned GPU memory.
- **FORBID-395-003:** No automatic global blur on text, controls, focus or every bright pixel.
- **FORBID-395-004:** No claim Bloom changes another material's physical illumination or implements GI/caustics.
- **FORBID-395-005:** No HDR-display claim from half-float output alone.
- **FORBID-395-006:** No silent redefinition of HAL-385/386/388, R-027, current material caps, alpha rules or Z semantics.
- **FORBID-395-007:** No new clock, feature gate, unbounded workload, invented source strength or default product skin.

## 6. Acceptance and proof boundary

A lawful future native scene must show dark/bright actual backgrounds, two independently colored reflections, no-Bloom baseline, Bloom at qualifying overlap, fading spread when separating and final idle. The source area remains sharp, readable text/focus above it remains unchanged and no unrelated material acquires glare. Real current-backdrop Frosted Glass must remain truthful.

Focused CPU reference must verify source bounds, exact extraction and kernel, overlap color, alpha/tone, zero equivalence and numerical finiteness. Actual 1x/2x GPU tests must show directed color and halo, clipping, strict Z, acyclic graph source dependencies, no swapchain read, removal/resize/degraded-resource refusal and retained cleanup. Native proof and GPU timing must be measured; screenshots alone cannot prove temporal motion, actual display HDR or sustained cost.

An accepted internal HDR+SDR Bloom profile does not certify HDR monitor output. The latter is independently authorized and proved on an actual capable target.

## 7. Constitution and disposition

HAL-002 retained ownership, deferred changes, stable IDs, phase order, immutable scene and read-only GPU submission remain unchanged. HAL-084/150 remain obedient orchestration/projection layers. No HAL-002A amendment is requested. This S0 does not enable any runtime feature or grant implementation.

## 8. Audited first SDR Bloom with internal HDR optical source: technical selection (2026-11-18)

**Status: S1 engineering-contract CANDIDATE / INACTIVE / NOT BINDING.** This section fills the choices intentionally left open in section 4 for ONE bounded same-surface, SDR-presented Bloom consumer. The prior S0 text remains historical where it says the decisions had not yet been made; this latest section is the complete *candidate*, not a binding amendment or verified implementation. JC's corrective mandate does not itself grant cross-owner pipeline changes or physical HDR-monitor output. Existing HAL-002/002A, HAL-040/084/085A/150/359/385/386/388, HAL-370 and R-027 remain binding in their accepted scope.

### 8.1 Exact result, target and source profile

The first admitted candidate is ONE explicit Bloom-bearing `GlassSurfaceOptics` coating occurrence per `SurfaceId`, on an otherwise real existing `SurfaceAnalytic` or `FrostedGlassAppearance` consumer. Two real `AnalyticSurfaceLight` records supply the source. No new semantic scene, light bus, widget or camera is introduced. The case is the HAL-393 two-source crossing fixture with reflection_strength1, reflection_tint=(1,1,1,1), normal_reflectance0.50 and unchanged roughness0.28; the Bloom gain below is additional source-linear optical energy, never a change to the old capped SDR surface. Existing material and text are rendered as before.

A future checked optional `SurfaceBloom` appearance value has three f32 fields: `gain` in [0,8], `sigma_logical` either 0 or in [1,16] logical pixels, and `mix` in [0,1]. It has immutable copy semantics, read-only getters and existing-owner typed NonFiniteInput/InvalidParameters errors. Any zero field canonicalizes to absent Bloom; that is an ordinary authored material value, NOT an environment feature gate. The old constructors default to None and remain byte-equivalent. A nonzero request is supported only when current surface clear alpha is exactly 1; the target is a validated linear-working sRGB color attachment with format Rgba8UnormSrgb or Bgra8UnormSrgb, current physical DPR is finite/positive, and the exact current clip can be expressed under the existing single-positive-rounded-ancestor source policy. Other targets are typed-unavailable, not silently shown as a completed Bloom. The target's 2D GPU output remains SDR; NO HDR monitor output is selected here.

### 8.2 HDR optical radiance and one extraction law

For each fragment of the real owning rounded SDF, evaluate the same checked local-UV center/radii/rotation, N, light L/H and `P_i=pow(max(dot(N,H_i),0),4+124*(1-roughness)^2)` used in `glass_optics.wgsl`. Let `e_i=exp(-0.5*dot(ellipse_i,ellipse_i))*strength_i*P_i` and let `F=normal_reflectance+(1-normal_reflectance)*pow(1-N.z,5)`. Use ONLY the explicit coating reflection selection, not the material's unrelated diffuse or emissive contour.

In unitless *scene-linear radiance-proxy units relative to SDR white*, define:
```text
Q = gain * reflection_strength * reflection_tint.a * F
L.rgb = Q * (e0*C0 + e1*C1) * reflection_tint.rgb
Y = dot(L.rgb, (0.2126,0.7152,0.0722))
T = 1.0
K = 0.25
u = clamp(Y - (T-K), 0, 2*K)
soft = u*u/(4*K)
excess = max(max(Y-T,0), soft)
E.rgb = if Y>0 then L.rgb * (excess/Y) else vec3(0)
```

All source RGB and scalar intensities are from the existing validated [0,1] records, each e_i<=1 and Q<=8; each component of L and E is finite in [0,16]. The selected soft-knee is continuously differentiable at Y=T-K and Y=T+K, identically zero below T-K, and preserves source hue by one common scalar factor. No per-channel HDR clamp, exposure guess, threshold on already capped SDR pixels or global bright-text detection is admitted. Use the original own SDF coverage and the *actual captured ancestor coverage* once in the source pass. The HDR source target stores (E.rgb*coverage,coverage) as mathematically premultiplied scene-linear light (RGB may exceed alpha because its domain is HDR), with exact zero outside source support. Material opacity is NOT reinterpreted as optical source attenuation; independent coating reflection has its own declared reflection strength/tint. The selected first profile does not combine HAL-394 film modulation: until separately adopted and coordinated, only the existing non-iridescent C0/C1 are used. A future combined profile must reuse the very same admitted per-source film modifier rather than inventing inconsistent source hue in Bloom.

### 8.3 Concrete crossing oracle

For the HAL-393 colors C0=(1,0.25,0.10) and C1=(0.08,0.35,1), strengths0.40, centers moving from (0.20,0.50)/(0.80,0.50) to the reversed endpoints over 2.4s, radii (0.20,0.25), unit directions=(0,0,1), normal N=(0,0,1), gain=8 and F=0.50, the exact reference central pixel Y is:
- p=0: approximately 0.383427, extraction E=0;
- p=0.25: approximately 0.891493, positive soft-knee excess approximately 0.020020;
- p=0.5: approximately 1.181037, excess approximately 0.185793;
- p=0.75: same as p=0.25;
- p=1: same as p=0.

The test must calculate these values from the equation above with f64 CPU reference, using the exact source ellipse, not hardcoded matching output. This proves a true overbright source at the meeting point and zero extraction at both separated endpoints. A positive halo appears around the crossing only as a consequence of that extracted source. Fixture values are NOT general framework defaults. The output Bloom is independently disabled when its authored mix is zero, without changing the original material's two-light response.

### 8.4 Exact blur, SDR projection and alpha

The renderer creates a separate `Rgba16Float` optical extraction target S. Bloom uses a separable nonnegative Gaussian with physical sigma `sigma_p=f64(sigma_logical)*f64(actual_DPR)`, integer support `R=ceil(3*sigma_p)`, and exact admitted maximum R<=192. Each tap is `w[k]=exp(-0.5*(k/sigma_p)^2)`, constructed with f64 intermediates then f32 storage, with available source-boundary taps normalized by their actual sum exactly once per axis. No silent support shortening/downsample replacement, extra alpha transfer, cropped ROI kernel, wraparound, previously presented pixels or unused border sampling is permitted.

For each bloom occurrence, V is the outward-integer-rounded admitted own physical SDF extent; the source S covers V and contains the extracted value. Blur support region B=expand(V,R) intersect current target bounds, with zero samples outside the original optical source (NOT repeated edge texels). Horizontal output H and vertical output G use distinct Rgba16Float targets and dependencies; sample precisely the support required to compute G within B. The final bloom drawing is restricted to B and the actual owning ancestor clip, not the own silhouette, so a soft halo can extend outside that silhouette without enlarging layout/hit/scroll geometry. Opaque target-clear alpha1 is retained.

Compute `bloom_rgb=mix * G.rgb/(1+G.rgb)` componentwise AFTER both linear HDR blur passes. Thus bloom_rgb is in [0,1]. Composite only through the predeclared GPU color-blend operation `dst.rgb=bloom_rgb+(1-bloom_rgb)*dst.rgb`, with the existing destination alpha unchanged. This is the precise *screen* blend, not standard OVER and not physically correct illumination of neighboring materials. The bloom full-screen-triangle or B-quad fragment outputs `vec4(bloom_rgb,1)` (an opaque premultiplied source color); its color blend factors are Src=One, Dst=OneMinusSrc, Operation=Add, and its alpha factors Src=Zero, Dst=One, Operation=Add. It samples ONLY G, never the live destination attachment. Existing Rgba8/Bgra8 sRGB attachment blending performs this in the target's linear working domain; any other target transfer is refused until qualified. This contract makes SDR output explicit; it does not publish an HDR display signal.

### 8.5 One acyclic same-surface graph schedule

The original HAL-359 per-SurfaceId immutable strict-Z command schedule remains the truth. At the ONE Bloom-bearing material command, Build reserves a renderer-owned optical sub-sequence at exactly that source's position, after the original material (and, for Frosted, only AFTER its lawful HAL-385 current-prefix B/Hx/F replacement), but before every strictly higher-Z foreground command. Its source identity is the existing material occurrence; no second widget/SceneNode/independent Z or semantic event. Every graph stage has explicit unique logical resource handles and declared dependencies:

1. Render the actual lower-Z prefix through the current normal graph to opaque accumulation A. For Frosted, complete its distinct current-prefix sample, Gaussian and replacement into A first. Do not treat the Bloom source as Frosted's own lower backdrop.
2. Render the SAME checked material/light source into separate optical HDR extraction S, without reading A or another surface; the actual material SDF, renderer target metrics and inherited clip are reused.
3. Execute `BloomHorizontal` reading S and writing H, then `BloomVertical` reading H and writing G, on their exact finite regions.
4. Execute `BloomComposite` reading G only while the current A color attachment is loaded, blending screen into A with the exact factors from section 8.4. This is not a shader sample of A. The same physical A may be reused with a NEW logical handle/version for this sequential writer after all readers of its previous version are complete, matching HAL-385's graph version discipline.
5. Resume all subsequent strictly higher-Z normal commands (including sharp text, focus and capture), then perform the current terminal sink transfer/presentation exactly once. The swapchain remains a sink; no source/scene readback or hidden encoder is added.

HAL-084 Build declares the entire topology from immutable completed source, Compile validates all target caps, clip, attachment format, budgets and source versions before allocating/encoding, Execute obeys only those declarations. Reusing HAL-385 source topology is NOT permission to read a different SourceId or apply Frosted blur to the Bloom layer. No in-place source/target simultaneous texture sampling, cross-surface content, prior-frame image, graph-cycle workaround or new compositor authority.

### 8.6 ABI, lifetime, count and explicit refusal boundary

The first optical source helper is renderer-private and controlled: one additional `BloomSourceUniform` has EXACTLY 304 bytes in this order (16-byte aligned): rect xywh 0..15; four own radii 16..31; existing 80-byte RoundedClipUniform 32..111; two existing 64-byte light records 112..175 and 176..239; shape=(bevel_width,bevel_slope,roughness,normal_reflectance) 240..255; optics=(gain,sigma_logical,mix,reflection_strength) 256..271; reflection tint RGBA 272..287; control=(clip_enabled,0,0,0) 288..303. The source pipeline uses canonical group0 Globals and one checked group1 binding0, with a dynamic slot stride aligned to actual device limits. The original analytic 688-byte and Frosted 1344-byte parameter records stay exactly intact in this HAL-395 profile. In a later coordinated HAL-394 combination, preserve its separately proposed trailing thin-film record and explicitly qualify the shared optical source instead of relying on an overlapping reserved field.

All S/H/G targets are Rgba16Float; A is the existing opaque sRGB-compatible current-target accumulator. Limit to one effective optical occurrence per SurfaceId and a total additional optical resource reservation of at most 256 MiB per target, counted together with all current graph allocations and accepted HAL-040 budgets, not as an independent free quota. Reuse the current GPU pools and exact frame/command-submission custody. The selected source geometry/blur support must fit finite physical bounds, R<=192 and the target/device Rgba16Float render/sampling limits. Fail BEFORE acquiring optical GPU resources where Build already knows an invalid source/clip; use a typed Build refusal with actual SurfaceId/source key. A target-format/budget/capability failure discovered at Compile must return a typed Compile refusal, never masquerade as `wgpu::SurfaceError` or pretend the effect rendered with a smaller blur. No partial graph, independent renderer, hidden fallback, stale source substitution, changed baseline if absent, or auto conversion of an invalid active selection to zero Bloom.

The first active input profile uses a known opaque current-surface clear. Transparent windows, unknown color transfer, unsupported ancestor clips, multiple Bloom sources on one surface, and DPR whose actual computed R exceeds 192 are explicitly unsupported until separately accepted. All ordinary scenes with no Bloom remain on their original graph, zero optical allocations, unchanged output and no idle demand. Existing source geometry, UI text, Frosted blur, 3D private R-027 pipeline and accepted non-Bloom effects remain intact.

### 8.7 Focused checks, native result and admission boundary

For a future, explicitly adopted implementation the existing `src/tests/renderer_z_order.rs`, original graph/retained tests, material analytic CPU/GPU oracle and `examples/glass_material_lab.rs`/public test must prove: exact 304-byte ABI and shader compilation; no-effect material byte identity; finite L/E bound and soft-knee continuity at Y=0.75/1.00/1.25; the five HAL-393 fixture phases; separable normalized Gaussian and zero outside source support; true 1x/2x GPU readbacks including outset AA/ancestor clips; one current Frosted prefix without recursive self-sampling; exact source Z and sharp higher-Z text; format/budget/clip refusal; retained unmount/resize/resource recovery; and original existing consumer preservation. Real native visual capture must show a smooth source-connected halo emerging at intersection and disappearing when separated, on a legitimate current background. JC appearance acceptance and any physical HDR monitor output remain separate and OPEN.
The required coordinated owner decision BEFORE such implementation is adoption of an explicit HAL-012 appearance extension, HAL-085A controlled new source shader/pipeline record, HAL-084/040/150 graph/render ownership, HAL-359 per-command sub-sequence, and HAL-385/386 first consumer compatibility. The optional R-027 3D HDR service is NOT an owning GPU attachment. These exact prospective amendments are recorded here but are not yet Binding.

The documentation-only correction is verified using the following exact Grip commands after `ws halcyon`:

```text
read docs/specs/SPEC-HAL-395-Selective-Bloom-and-HDR-UI-Composition.md 1 5000
state capsule
```

Any authorized later source work uses actual focused `verify package halcyon test glass_material_lab check timeout 600`, `verify package halcyon test frosted_glass timeout 600` and `verify package halcyon test analytic_surface timeout 600` as applicable, plus native proof with the original App. No Cargo/GPU/native result is claimed for S0.

## 9. Second control-audit REWORK: closed first-consumer integration profile (2026-11-18)

**Status: TECHNICALLY SELECTED S1 CANDIDATE / INACTIVE / NOT BINDING.** This section closes the five integration findings of the read-only control audit. For the first selected SDR Bloom profile, sections 9.1-9.7 supersede any less-specific or ambiguous statements in sections 8.1-8.7 concerning value admission, pipeline choice, region geometry, staging, error handling and readiness. Original optical equations and their numerical oracles in section 8.2-8.3 are preserved unchanged. This text does NOT amend adopted HAL-002/002A, HAL-012/040/084/085A/150/359/385/386/388, or HAL-370/R-027, does not grant engineering work, and does not claim runtime or GPU evidence.

### 9.1 Exact appearance ingress and zero-opacity refusal

The only new checked public optical value is `SurfaceBloom` with fields `gain:f32` in [0,8], `sigma_logical:f32` equal to zero or in [1,16], and `mix:f32` in [0,1]. `SurfaceBloom::try_new(gain,sigma_logical,mix)->Result<Self,SurfaceBloomError>` returns `NonFiniteInput` or `InvalidParameters`. Any zero field canonicalizes to absence.

Add one `bloom:Option<SurfaceBloom>` to the EXISTING `GlassSurfaceOptics` checked immutable value, not another light/appearance registry. `GlassSurfaceOptics::with_bloom(self,Option<SurfaceBloom>)->Self` and `bloom()->Option<SurfaceBloom>` preserve `Copy/Clone/PartialEq`, two current source lights, original `try_new` signature and equality. Existing constructors initialize `None` and preserve exactly the current pixels/cost. `SurfaceAnalytic::with_glass_optics` and `FrostedGlassAppearance::try_with_optics` remain the ONLY two existing material-consumer entrances. They carry the same coating with its optional Bloom and all ordinary base/blur/refraction/grain/emission selections intact. There is no `SurfaceFill` alternative, no new SceneNodeKind, no Widget or per-instance GPU resource.

The FIRST Bloom profile requires an effective selected coating and **body opacity greater than zero**. On a `SurfaceAnalytic` with selected nonzero Bloom and `parameters().opacity==0`, local material emission MUST return `AnalyticSurfaceError::UnsupportedBloomOpacity` BEFORE the existing `validate_geometry` could return `Ok(false)` and silently emit nothing. For Frosted with `coverage_opacity()==0`, return `FrostedGlassError::UnsupportedBloomOpacity` at its ordinary material admission. Existing zero-opacity `SurfaceEmission` remains legal when Bloom is absent; no existing accepted request changes. Section 8.2's Bloom source independence from material opacity applies to admitted POSITIVE opacity only: once positive, the source contribution is not scaled by that opacity. Zero source RGB or zero light strength is a LEGAL black Bloom source, not an invalid material. No synthetic invisible/zero-opacity material is added as a workaround.

A nonzero checked `SurfaceBloom` may be attached only where `GlassSurfaceOptics` exists. Unsupported/missing coating is typed refusal, never implicit creation. Normal component Update owns immutable material publication and narrow HAL-354 invalidation; no shader-derived state or extra clock.

### 9.2 Exact material-to-Scene-to-Compile association

The original `PaintSink` and `SceneBuilder` emit exactly the existing `AnalyticSurfaceNode` or `FrostedGlassNode`. At the FINAL composed-scene `Batcher` traversal, after original scroll/clips are known, a private `BloomOccurrence` accompanies the existing `SurfaceSceneData` in `DrawPlan`. It contains `SurfaceId`, optional original `SceneNodeKey`, existing material's original surface-local ordinal, a frozen checked `GlassSurfaceOptics` and `SurfaceBloom`, actual current scroll-projected rect/four radii, original rounded ancestor, actual rectangular scissor and effective Z. The original source node/ordinal remains the sole identity. `DrawPlanBuilder` is still the ONLY reader of SceneGraph; it performs finite geometry/full inherited clip/one-per-SurfaceId, opacity and ambiguous-Z admission BEFORE GPU acquisition. Exactly one positive rounded ancestor is supported; a second or invalid ancestor is typed Build refusal. An existing source with absent/zero Bloom yields no occurrence. Any overlapping unrelated draw at equal Z whose true order is not established by strictly different effective Z is `BloomBuildRefusal::AmbiguousOrder`, never inferred insertion order.

`WgpuRenderer::compile_multi_admitted` retains the HAL-359 final command authority. Join `BloomOccurrence` to the ACTUAL complete `SurfaceDrawCommand` by exact SurfaceId, original material ordinal and source snapshot. For direct analytic material the original command is `SurfaceDrawCommand::Batch` with `PipelineId::AnalyticSurface` and its unique ordinal from the original batch; for Frosted it is the original `SurfaceDrawCommand::FrostedGlass{ordinal,...}`. A rejected/nonexistent/duplicated/stale compiled source is `BloomCompileRefusal::MissingCompiledSource`, never a skipped successful result. The candidate does not add a public marker or new semantic Z. Source/Bloom stay at the original material position; all higher-Z text/focus/capture remain sharp.

The actual GPU topology's CPU-only Build happens at the CURRENT **Compile-owned** topology seam AFTER final executable command admission. Section 8.5's word "Build" must NOT be read as an extra UI phase or a pre-Compile prediction of command order. `DrawPlanBuilder` captures immutable source facts and refusals; the renderer constructs/validates the graph from the final command schedule, then Execute only encodes its frozen result.

### 9.3 One concrete geometry and graph

All four new GPU programs draw exactly ONE **ROI-local fullscreen triangle**, generated with `vertex_index` 0,1,2, `vs_main/fs_main`, no vertex buffer, no B-quad alternative. The target viewport is its positive-area ROI texture from (0,0) to (width,height). The fragment's ORIGINAL physical surface pixel is `P=roi_origin_px+floor(@builtin(position).xy)`. Existing `Globals` provide true current physical extent/projection; the source shader reuses the original HAL-386 SDF and one original HAL-385/386 rounded ancestor. Original rectangular scissor is projected separately at both source and final composite. There is no fullscreen HDR UI postprocess or swapchain read.

Original normal commands render into a single ordinary full-target accumulation A (exact current `Bgra8UnormSrgb` or `Rgba8UnormSrgb` format). If Frosted already uses A, **reuse the same A**, its original producer and HAL-385 resource lifetime. If no Frosted exists but Bloom does, allocate A once through the same renderer resource owner and publish through the existing one terminal target transfer. No Bloom and no Frosted means the old direct path unchanged.

At the already final `SurfaceDrawCommand::Batch` corresponding to analytic source: encode the actual ordinary material batch, split normal pass there, run four nodes `bloom_source` -> `bloom_horizontal` -> `bloom_vertical` -> `bloom_composite`, then resume the next original normal command. At the `FrostedGlass` marker: first complete its existing HAL-385 prefix snapshot, Gaussian and original replacement, THEN those four nodes and the subsequent original commands. The Bloom source itself evaluates the same original light/material SDF and **never reads A**, Frosted's backdrop or the swapchain. Its source `S` is independently rendered. All nodes have unique static names since the first profile admits at most one Bloom per SurfaceId. The original main/overlay and strict Z ordering are unchanged.

Every normal segment/composite that overwrites A creates a NEW logical `AttachmentHandle` version with a single writer; all readers of the prior version must complete first. A physical backing alias is allowed ONLY after the HAL-084 validator proves last-reader-before-overwrite and no read/write overlap. Source S, intermediate H, final G each have distinct logical/physical attachments and explicit read/write dependencies. `BloomComposite` samples G ONLY and uses attachment Load on A, never `textureLoad(A)`. No hidden encoder, alternate RenderGraph, independent publisher, image-source feedback or cross-SurfaceId sampling is admitted. Source prefixes missing from an existing Frosted operation retain HAL-385's original `IncompletePrefix` refusal. Spatial-HUD companion on that same Bloom SurfaceId is unsupported in this first profile; it is refused before allocation.

### 9.4 Complete shader and binding ABI

All programs have unchanged `@group(0) @binding(0)` `Globals`, no sampler, no storage/compute stage, and no new public shader callback. Rgba16Float source/filter textures are read only by `textureLoad` with `texture_2d<f32>`, D2 nonmultisampled, `TextureSampleType::Float{filterable:false}`. Four pipeline contracts:

* **BloomSource:** group1/binding0 one dynamic UNIFORM `BloomSourceUniform` exactly **304 bytes** with the section8.6 field order preserved: rect[0..15], four corners[16..31], original 80-byte `RoundedClipUniform`[32..111], two existing 64-byte lights[112..239], shape[240..255], optics[256..271], tint[272..287], control[288..303]. For exact ROI addressing, `control:vec4<u32>=(rounded_clip_enabled,S_origin_x_px,S_origin_y_px,0)`; old section8.6's unused zeros are superseded ONLY for this new uniform. Dynamic byte stride=`align_up(304,max(256,device.min_uniform_buffer_offset_alignment))`; min_binding_size304. Fragment uses absolute reconstructed P for four-corner SDF, original rounded ancestor and selected section8.2 radiance. It writes `S: Rgba16Float` with BLEND DISABLED as `(E.rgb*own_coverage*ancestor_coverage,own_coverage*ancestor_coverage)`. Radiance is scene-linear finite (RGB can be greater than alpha because the working domain is HDR); no old material target is reinterpreted as HDR.

* **BloomHorizontal / BloomVertical:** group1/binding0 one NONdynamic UNIFORM `BloomFilterUniform` exactly **848 bytes**: `target:vec4<u32>=(0,0,W,H)` offset0; `input_roi:vec4<u32>=(x,y,w,h)` offset16; `output_roi:vec4<u32>=(x,y,w,h)` offset32; `kernel:vec4<u32>=(axis,R,0,0)` offset48; `weights:array<vec4<f32>,49>` offsets64..847 (193 scalar weights k=0..R at `weights[k/4][k%4]`, remaining slots0). Axis0=horizontal and1=vertical; R<=192. Group1/binding1 is the single prior Rgba16Float read-only texture. Two independent frozen uniforms/bind groups, no sampler, min_binding_size848. Output H and G are Rgba16Float with BLEND DISABLED. Compute identical separable bounded Gaussian on ALL four channels; WGSL must have the matching exact CPU f64 weight reference rounded once to f32. This packing uses 49*16=784 bytes plus64, so 848 bytes total.

* **BloomComposite:** group1/binding0 NONdynamic UNIFORM `BloomCompositeUniform` exactly **144 bytes**: `output_roi:vec4<u32>`0..15, `target:vec4<u32>=(0,0,W,H)`16..31, original `RoundedClipUniform`32..111, `response:vec4<f32>=(mix,0,0,0)`112..127, `control:vec4<u32>=(rounded_clip_enabled,0,0,0)`128..143. Group1/binding1 is the read-only G texture (min_binding_size144). At reconstructed P inside the original scissor and B, the shader computes `m` from the original rounded ancestor (m=1 if absent), `bloom_rgb=m*mix*G.rgb/(1+G.rgb)` and emits **`vec4(bloom_rgb,1)`**. The existing opaque sRGB-compatible A render attachment is blended with color `src=One,dst=OneMinusSrc,op=Add`, alpha `src=Zero,dst=One,op=Add`. This is the precise section8.4 SDR screen equation with destination alpha unchanged, NOT an unqualified physical light transport. No shader reads A. No other output target format or transfer profile is admitted.

Original AnalyticSurface 688-byte and Frosted 1344-byte uniform ABI, current ShaderGroup0, and existing Shape, Image, Gradient, Text pipelines remain byte/behavior intact. New records are renderer-owned by immutable material occurrence and created only when Bloom is actually effective. New source/filter/composite ABI and any additions to HAL-085A must be explicitly accepted under that owner BEFORE implementation, not inferred from this candidate.

### 9.5 Correct bounded ROI blur and resource accounting

Let T=[0,W)x[0,H) be the ACTUAL physical target. V is the original positive-area source's outward-integer-rounded SDF rectangle plus two physical pixels of admitted AA support, intersected with T and the actual original rectangular scissor. Define `sigma_p=f64(sigma_logical)*f64(actual_DPR)>0` and `R=ceil(3*sigma_p)`, reject R>192 rather than reduce quality. S is EXACTLY V. Horizontal region `Hroi=expand_x(V,R) intersect T` with original V.y interval; vertical G and composite region `B=expand_xy(V,R) intersect T`. This fixed geometry supersedes the old unqualified "precisely required support" wording: horizontal convolution has zero source outside V.y; vertical convolution creates the full R outward y halo. No needed tap is lost at an ROI boundary.

At every output pixel P of a filter, denominator=sum over offsets k=-R..R whose sample coordinate is within physical T, using the selected f32 Gaussian w[abs(k)] calculated from f64. Numerator adds the same weight times the previous input sample IF the sample falls in its exact declared input ROI, otherwise ZERO (do not clamp-repeat or renormalize to the restricted ROI). For horizontal, sample x offsets only; for vertical, y offsets only. Both passes normalize exactly once along their current axis. Only offsets outside physical T are omitted from denominator. This produces the declared zero-padded optical source on a finite real target without seams or false bright edges. S,H,G are linear-premultiplied scene radiance; G.alpha is retained for numerical/diagnostic proof, while final screen blend uses G.rgb only. The final composite evaluates ancestor coverage AGAIN at destination rather than allowing blurred light to escape its original clip; this is a distinct destination coverage boundary, not accidental double premultiplication of one sample.

Count resources with checked u64 arithmetic: S/H/G textures each use 8 bytes per actual ROI pixel; A uses one existing or new 4-byte per full-target pixel; count exact uniform allocations/stride/bindings and the pre-existing Frosted intermediates in ONE combined target plan. The complete plan must fit the ALREADY accepted HAL-385 **256MiB plan** and **768MiB resident** ceilings and HAL-040 parent/device budgets; Bloom has NO extra allowance. Unsupported attachment usage, limits, unknown target transfer, nonopaque original clear, invalid/empty source, resource exhaustion or a need to truncate R/ROI returns typed refusal rather than downgraded output. No-Bloom scenes retain their old allocation and submission path.

### 9.6 Exact error and preparation contract, including Pending

The following are **PROPOSED** additions to existing public error envelopes, never claims of current implemented API:

```text
enum BloomBuildRefusal {
    MissingCoating, UnsupportedBodyOpacity, InvalidGeometry,
    InvalidClip { depth: usize }, UnsupportedClip { depth: usize },
    AmbiguousOrder, OccurrenceLimit { limit: usize, requested: usize },
}
DrawPlanBuildError::Bloom {
    surface_id: SurfaceId, node_key: Option<SceneNodeKey>,
    reason: BloomBuildRefusal,
}
enum BloomCompileRefusal {
    TargetUnavailable, InvalidTarget, UnsupportedFormat, NonOpaqueClear,
    MissingCompiledSource, UnsupportedCompanionGraph, UnsupportedUsage,
    KernelLimit { required: u32, limit: u32 },
    ResourceBudget { required_bytes: u64, limit_bytes: u64 },
    Graph(GraphError),
}
RendererCompileError::Bloom {
    surface_id: SurfaceId, node_key: Option<SceneNodeKey>,
    reason: BloomCompileRefusal,
}
```

Preserve all existing `DrawPlanBuildError`, `RendererCompileError::Surface` and `RendererCompileError::FrostedGlass` variants with the original semantics; the new Bloom variants and the two `UnsupportedBloomOpacity` material errors require explicit coordinated owner adoption and simultaneous actual-caller match migration. Existing `RendererCompileError::Surface(OutOfMemory)` remains the actual allocation-OOM result, never relabeled as a policy error. Any expected refusal cannot become successful NoTargets, Ok(None), silently removed source or fabricated rendered front.

The existing `Result<RendererCompileOutcome<T>,RendererCompileError>` with `Ready(T)`, `NoTargets` and `Pending(RendererPreparationIdentity)` remains the ONLY output envelope. Normal native/Metal preparation yields Ready or explicit error. The browser's asynchronous `pop_error_scope` resource qualification for Bloom (with or without Frosted) must reuse the current renderer-owned `RendererPreparationIdentity` / `on_completed` wake mechanism, never a second future, clock, timer or independent scheduler. A target with both Frosted and Bloom has ONE captured combined immutable preparation generation: Frosted Pending means Bloom cannot encode or present independently; *any* unfinished required resource scope returns that SAME Pending identity, with no partial graph. The original valid visible frame may remain while Pending; it is not presented as the new result. On completed qualification, resume exactly that generation if all current source/target/clip/device revisions still match; otherwise discard stale work and requalify current inputs. Unmount/cancel/supersession/device loss invalidate prior scopes and outputs. Genuine allocation OOM retains Surface(OutOfMemory). No busy polling, sleep, automatic retry, duplicate allocation or perpetual Pending loop.

**CRITICAL ADOPTION DEPENDENCY:** Current binding HAL-385 section8.3 permits Pending ONLY for browser Frosted allocation. Browser Bloom must NOT be claimed supported until an exact coordinated HAL-385/renderer amendment extends that condition to the same renderer-owned Bloom scopes and preserves all existing Frosted behavior. This S0 text is the complete selected future contract but is NOT that adopted amendment. Native-only qualification must still obey the old Frosted Pending behavior without claiming browser Bloom success.

### 9.7 Selected scope and real proof

First profile supports exactly one effective Bloom coating per SurfaceId, one positive-opacity actual material, one current sRGB-compatible opaque-clear target, one original rounded ancestor, no same-surface HUD companion, and the existing two checked optical lights. Transparent windows, arbitrary output transfer, multiple Bloom sources on one surface, more than one positive-rounded ancestor, full HDR display output and a spectral lighting system remain outside it. A zero-evaluable source remains a lawful zero optical contribution. No instance, default skin or ordinary old material gains Bloom without an explicit selected value.

Future implementation proof remains one coherent user-visible unit. Extend existing tests at `src/renderer/draw_plan_builder.rs`, `src/renderer/batcher.rs`, `src/renderer/compiled_draw_plan.rs`, `src/renderer/frosted_glass/graph.rs`, `src/tests/renderer_z_order.rs` and `examples/glass_material_lab.rs` only where relevant. Prove zero-effect equality, positive/zero-opacity admission in BOTH consumers, source/ordinal identity, same-Z refusal, 304/848/144 ABI, real 1x/2x Metal source/H/G/composite, exact three-region Gaussian support and 256MiB combined budget, original Frosted same-surface prefix, no read/write alias, sharp higher-Z text/focus, complete Ready/NoTargets/Pending/Err outcomes and cancellation, old no-Bloom no-work identity and the actual moving halo over a changing background. Browser Pending requires separately accepted browser allocation ownership and actual browser evidence. Tests/evidence in one stage do not prove another; JC native visual judgment remains independent.

Required authority BEFORE execution: coordinated HAL-012 coating selection; HAL-385/386 material, zero-opacity and source ownership; HAL-085A shader/ABI; HAL-084/040/150/359 graph/order/resources; the narrow HAL-385 Pending-envelope qualification and public compile-error extension. HAL-002/002A, HAL-021 and R-027 require no change. No executor may choose another shader, pass, filter, material, geometry, fallback or error model.

For this DOCUMENT-ONLY rework the exact read-only checks after `ws halcyon` are:

```text
read docs/specs/SPEC-HAL-395-Selective-Bloom-and-HDR-UI-Composition.md 1 5000
state capsule
```

S0 remains NOT BINDING. These corrections supply the selected engineering contract, not executable implementation or HDR-monitor qualification.
## 10. Binding S1 technical adoption — 2026-11-18

**Decision:** ADOPTED / BINDING for the complete first one-Bloom-per-SurfaceId GlassSurfaceOptics profile defined by the combined precise optical equations of sections 8.2-8.3 and the controlling integration corrections of sections 9.1-9.7. Under JC's directly recorded 2026-11-18 delegated technical adoption, the delegated technical reviewer accepts this scoped design and its coordinated parent amendments; JC retains constitutional/product/visual authority and veto. Historical S0, S1-candidate and Pending-adoption statements in preceding sections are superseded ONLY within this exact first profile.

The accepted execution contract uses a checked per-instance SurfaceBloom, positive body opacity (zero typed refusal), existing AnalyticSurface/Frosted material ingress, source ordinal and actual HAL-359 final schedule, one same-surface ROI triangle per selected GPU stage, source/filter/composite ABI of 304/848/144 bytes, HAL-084 acyclic graph custody, fixed source radiance/soft knee/Gaussian/screen-composite mathematics, existing HAL-040/HAL-385 combined resource budgets, and one frozen renderer-owned Pending generation for asynchronous browser resource qualification. Source/foreground Z, current-prefix Frosted sampling, no-Bloom byte identity and target-color/alpha restrictions remain strict. New prospective error variants become accepted design obligations but are NOT represented as already implemented.

Only internal bounded HDR optical computation with SDR monitor output is selected. Physical HDR/WCG scanout, cross-surface illumination, third-party GPU submission, arbitrary Bloom multiplicity and combined HAL-394 film+Bloom modulation are NOT adopted. The separately adopted HAL-394 first film profile does not automatically combine with this Bloom profile: such composition needs its own exact source-color integration and backend proof. Actual GPU/Metal/browser/native engineering and JC visual acceptance remain OPEN.

## 11. S1 implementation evidence

Halcyon 0.260.0-beta exposes `SurfaceBloom::try_new(gain, sigma_logical, mix)` and `GlassSurfaceOptics::with_bloom` in `core::paint`. Each accepted occurrence executes from its original material command. The four stages use the same accumulator, backing pool, command custody and browser preparation identity as the existing Frosted graph.

All 11 focused tests passed. The CPU crossing values and 304/848/144-byte records matched the selected contract. Actual 1× and 2× Metal readback covered all three HDR textures and SDR screen blending, including the real target edge, combined Frosted prefix and unchanged foreground order. The tests also proved zero-mix identity, exact settled pixels, stale source and overflow refusal before backing allocation, and shared Pending, cancellation and OOM handling. Package check passed with zero errors.

In the native Glass Material Lab, the two existing tracks drove both comparisons. At the captured overlap, the plain and Bloom center RGB values were (137,115,141) and (158,134,162). The last three capture pairs matched at the sampled centers after the halo faded. The original 2× captures and measurements are in `hal395-native-76vylbjf`.

The offline, locked browser probe built and bound with wasm-bindgen 0.2.116. Safari WebGPU visibly presented the combined Frosted and Bloom panel with a blurred lower backdrop, a colored center halo and sharp higher-order text. JC's appearance judgment remains open.
