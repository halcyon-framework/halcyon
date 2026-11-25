# SPEC-HAL-386: Smoked Glass Surface Material

**Status:** ADOPTED / BINDING for the first Smoked Glass profile in section 8 and its coordinated owner amendments. Sections 1-7 retain optical intent where consistent with section 8. Implementation and JC visual acceptance are not implied.
**Date:** 2026-11-13.
**Owner:** HAL-012 appearance and current direct surface renderer.
**Confirmed JC decision:** Smoked Glass is a distinct material without backdrop blur, not a degraded Frosted Glass implementation.
**Governing owners:** HAL-002/002A, HAL-012, HAL-024, HAL-085A, HAL-354 and HAL-359.
**Related inactive proposal:** HAL-384 Analytic 2.5D UI Materials already includes an opaque SmokedGlass profile; this new candidate cannot silently adopt, duplicate or supersede it.
**First user result:** A restrained dark glass-like panel whose material identity is legible on a plain background, with no halo or grain.

## 1. Purpose and user-visible meaning

Smoked Glass is an authored dark optical surface with low-frequency tonal depth, potential ordinary transparency and coherent soft reflections. It does not redistribute background pixels. The renderer must never describe it as real frosted blur, refraction, environment reflection or physical thickness. Alpha below one shows backdrop content through standard composition; it is not a blur or scattering mechanism.

## 2. Independent material response

**INV-386-001 FACE:** The proposed single-owner material basis is the finite three-stop straight-linear gradient and rounded SDF of the existing *inactive* HAL-384 AnalyticSurface SmokedGlass candidate, with independently authored opacity a in [0,1]. HAL-384 currently defines only opaque stops, so alpha capability is a new candidate requirement and cannot be assumed implemented. Alpha zero is an absent-material diagnostic and alpha one the opaque diagnostic. A plain static Gradient alone cannot count as completed Smoked Glass visual acceptance.

**INV-386-002 DISTINCT FACE REFLECTION:** A visible nonzero SmokedGlass material must have an independently observable, direction-sensitive, broad low-frequency reflected/specular face response under at least one lawful finite nonzero light basis. The response must visibly and numerically differ from an otherwise identical plain three-stop Gradient, even when surface grain, rim and glow are all disabled. Zero illumination remains a lawful explicit *baseline control*, not final visual evidence that glass optics are present. There is no mutable global light, mouse/OS inferred source, owner-inaccessible GPU state or automatic edge glow.

**INV-386-003 INDEPENDENT OPTICS:** Surface Micrograin HAL-387, Local Ambient Glow HAL-388, reflective edge HAL-389 and contour emission HAL-358 are optional *separate* appearance selections. Smoked Glass must be independently recognizable by INV-386-002 without any of them; a golden border, shadow, decorative stripe or random grain never replaces broad face reflection.

**INV-386-004 NO BACKDROP:** No backdrop source ID, capture, blur, offscreen snapshot, previous-frame image or live background sample is part of the material algorithm. With fixed style and geometry the source surface's own computed RGBA remains the same irrespective of moving background, although ordinary alpha compositing changes final visible pixels.

## 3. Owner-correct execution

The component owns its actual rect/radius, existing interaction state and submitted paint description. HAL-012 supplies coherent immutable optical values. Renderer owns the actual physical projection, analytic SDF coverage and material evaluation. HAL-354 retains clean fragments; HAL-359 maintains surface, text and focus ordering. The proposed base is one bounded ordinary direct-surface draw, not a widget-owned GPU resource, new paint engine, Image identity or independent semantic control.

**INV-386-005 ALPHA:** Style color is straight linear and final shader output is linear-premultiplied. Own SDF and inherited clip coverage are distinct and applied at their lawful boundaries, without double transfer or alpha manipulation. Ordinary text and mandatory focus/capture cues remain externally legible.

**INV-386-006 RETAINED:** Equal material/geometry/lighting/target/style generations cause no additional preparation or idle animation. Real changes dirty only the relevant retained fragment and composition. Moving a surface does not turn a static material into a moving sampled screenshot.

## 4. HAL-384 collision and reconciliation

**INV-386-007 SINGLE MATERIAL OWNER AND COORDINATION:** The design resolution is one HAL-012 typed AnalyticSurface material family with HAL-384's already-proposed SmokedGlass profile, not an alternative public SmokedGlass enum or second renderer pipeline. HAL-386 contributes only independent optical semantics, required nonzero face-reflection proof and proposed opacity range. Before any adoption, coordinate exactly one HAL-384/HAL-012/HAL-085A owning alpha and lighting contract and preserve private Button/selected-card material authority. Inactive HAL-384 has an opaque shader ABI: it cannot be claimed as an alpha-capable implementation, and neither inactive candidate activates the other.

## 5. Prohibitions

**FORBID-386-001:** No backdrop sampling, shadow marketed as blur, fake frosted pixels, stale captured Image or implicit fallback from requested true Frosted Glass.
**FORBID-386-002:** No unconditional glow, emitted edge, random noise, synthetic per-frame flicker, product-specific gold default or automatic material substitution.
**FORBID-386-003:** No changes to layout, hits, focus/capture, text, scrolling, widget ownership, ordinary Solid/Gradient/Image/NinePatch behavior or cross-pipeline Z authority.
**FORBID-386-004:** No duplicate rendering path while HAL-384's SmokedGlass owner is unresolved.

## 6. Verification and status

Future admission must prove nonzero face reflection against an otherwise identical plain Gradient, opposite-direction illumination and a separate zero-light baseline; alpha=0/1 and genuinely translucent composition over controlled backgrounds; finite linear-premultiplied alpha, target/clip/scroll, 1x/2x AA, retained clean reuse and unchanged focus/text; no backdrop read, halo or unrelated Solid/Gradient/Image/NinePatch overhead. Native comparisons with a separately admitted Frosted Glass surface are required for the final JC visual approval. This candidate claims no current renderer feature, default or implementation permission.

## 7. Document boundary

This candidate makes Smoked Glass an independently meaningful design contract but leaves HAL-384 unchanged. It neither changes code nor creates execution permission until exact owning authority and the proposed alpha range are adopted.

## 8. Adopted S1: Smoked Glass with an independent reflective rim (2026-11-13)

**Authority:** JC authorized the independent optical-effects implementation. This section and the coordinated HAL-012/HAL-085A/HAL-084 amendments are binding for this first profile and supersede earlier future-admission clauses only within that scope. HAL-384 remains an inactive historical proposal. Its shared analytic-face approach informs this profile, while its opaque-only, uniform-radius and constant rim-mix design is replaced here. This adoption does not include the proposed matte-graphite or champagne-toned metallic profiles, HAL-385 Frosted Glass or HAL-387 Micrograin. Implementation and visual acceptance require separate evidence.

### Pre-Flight

- Objective: A native material comparison showing actual Smoked Glass face reflections, ordinary translucency, and an independently disabled/enabled directional reflected rim.
- Standing objective: Deliver the independently requested optical effects without replacing existing working Halcyon appearances or product UI.
- Contract ownership: This SPEC defines the material contract; the existing renderer owners implement it. Final visual acceptance remains with the maintainer.
- Continuation disposition: Implement the complete material and rim together in one face shader, then independently review real CPU/GPU/native evidence.
- Mandatory startup and authority entry chain completed: Current common doctrine; Halcyon entry and unchanged HAL-002/002A; HAL-012/024/084/085A/354/359; HAL-384 proposed mechanics, HAL-386/389 candidates, PM-026 and actual paint/scene/renderer source.
- Governing authority and status: HAL-002/002A supreme; adopted appearance, shader, phase, retained and ordering owners remain binding. This is the scoped nonconstitutional adoption.
- Authority change: One checked direct analytic material and optional rim term, one explicit pipeline profile, and the exact final-composition admission below. No product default or arbitrary material graph.
- Target layer and existing state verified: SurfaceFill has only Solid/Gradient/Image/NinePatch; SurfaceAnalytic is absent; PipelineId ends at 12; SceneNodeKind ends at 22. Existing Gradient uses 512 parameter slots and ordinary dynamic uniforms; HAL-388 has implemented typed Build refusal.
- Owned mechanisms and files: Existing paint/scene/scene-diff/batcher/DrawPlanBuilder/renderer/RenderGraph dispatch and their tests, one analytic pipeline/shader, and one native example. Unrelated runtime and application work is outside scope except for required mechanical call-site integration.
- Named invariants: INV-386-001..007, INV-389-001..009, HAL-084 phase/failure laws, HAL-359 strict Z, HAL-354 retained identity, HAL-085A premultiplied output, HAL-012 complete appearance and clip ownership.
- Forbidden behavior: Backdrop reads, global lights, new phases, material graphs, feature gates, fake borders, changes to existing Shape 0..5, or silent clip/resource fallback.
- Existing mechanisms inspected and reuse decision: Reuse SurfaceGradient, Color, RoundedCorners, ordinary immutable scene/build/compile/execute and existing RoundedClipUniform arithmetic; no copied renderer or widget-owned GPU resource.
- Dependent boundaries: Applications retain styling and model/runtime ownership. This scope adds no texture registration, focus owner or physical-input authority.
- Test ownership and placement: Existing inline paint/pipeline, draw_plan_builder, renderer_z_order and retained_value_updates owners; one native App example.
- Exact runnable verification commands and expected proof: verify package halcyon check timeout 600; verify package halcyon test smoked_glass timeout 240; verify package halcyon test reflective_rim_light timeout 240; verify package halcyon test analytic_surface timeout 240; verify package halcyon test local_ambient_glow timeout 240; verify package halcyon test luminous_contour timeout 240. New filters must own real durable tests, not report success with zero selected tests.
- Output completeness: Checked public paint use, exact shader and final clip admission, retained integration, real GPU comparison, and ordinary native example. Binding text alone proves none of these.
- SemVer/changelog impact: Keep current 0.232.0-beta for this bounded source unit; add specific Unreleased capability notes, not a product-release claim.
- Destructive and external effects: None. Preserve unrelated files and source state; no external publication.
- Out of scope: Other material profiles, true frosting, grain, default control replacement, general nested-mask service and unrelated runtime diagnostics.

### 8.1 One public owner and checked input

The single geometry-independent checked value is core::paint::SurfaceAnalytic, initially constructible only through try_smoked_glass(base: SurfaceGradient, lights: [AnalyticSurfaceLight; 2], parameters: SmokedGlassParameters, rim: Option<SurfaceRimLight>) -> Result<Self, AnalyticSurfaceError>. Store validated immutable values with read-only getters. SmokedGlassParameters is an input record with exactly these f32 fields: opacity, bevel_width, bevel_slope, roughness, ambient_strength, diffuse_strength, specular_strength, max_interior_uplift. The checked value does not retain mutable caller references.

Base must be the existing linear SurfaceGradient with exactly three opaque straight-linear RGBA stops at offsets 0.0, 0.30_f32, 1.0; endpoints remain in [0,1] and their squared separation must be greater than 0.0001, matching the existing nondegenerate Gradient path. Each base RGB is in [0,1], alpha is exactly 1. Authored opacity is separate in [0,1]. Bevel_width is 0 or in [0.5,4] logical pixels; bevel_slope is in [0,0.9]; roughness is in [0.04,1]; ambient/diffuse/specular strengths are in [0,1]; maximum relative interior uplift is in [0,0.25]. Zero bevel width requires zero bevel slope. All inputs must be finite.

AnalyticSurfaceLight::try_new(direction: [f32;3], color: Color, strength: f32, center_uv: Point, radii_uv: Point, rotation_cos_sin: Point) validates positive direction.z, squared direction length within 0.0001 of 1, RGB in [0,1] with alpha exactly 1, strength in [0,1], center components in [0,1], radii in [0.02,2], and squared rotation length within 0.0001 of 1. Normalize only an already accepted near-unit direction/rotation once into stored values; out-of-range inputs are refused, never repaired. Zero strength and black source RGB are explicit zero-radiance controls, not missing inputs.

SurfaceRimLight::try_new(color: Color, strength: f32) validates finite straight-linear RGBA in [0,1] and strength in [0,0.20]. None or strength/alpha zero disables only the rim contribution. An effective nonzero rim requires positive bevel_width and bevel_slope. There is no inferred gold, pointer-driven source, hidden clock or automatic glow.

AnalyticSurfaceError has distinguishable NonFiniteInput, InvalidBaseGradient, InvalidLight, InvalidParameters, InvalidRim, InvalidGeometry and UnsupportedPaintSink variants. Invalid requests do not silently become Gradient or Solid.

### 8.2 Geometry and fallible emission

Admit PaintSink::push_analytic_surface(&mut self, rect: RectF, corners: core::paint::RoundedCorners, material: &SurfaceAnalytic, z_index: ZIndex, key: Option<SceneNodeKey>) -> Result<(),AnalyticSurfaceError>, with the concrete checked SceneBuilder implementation. Other PaintSink implementations retain a typed UnsupportedPaintSink default. Legacy push_surface_fill/push_admitted_surface_fill and the four existing SurfaceFill variants remain unchanged: their infallible API is not widened to hide failed analytic admission. This expressly replaces the inactive HAL-384 SurfaceFill::Analytic entry proposal; the one checked material family still has only one renderer path.

Validate finite origin, dimensions, endpoints and corner radii. Absolute origin/endpoints and positive extents are bounded by 1048576 logical units. Finite nonpositive width/height is an empty draw; zero opacity is an absent draw. Visible width and height must be at least 1.0; each own corner is in [0,min(width,height)/2]; bevel_width must not exceed min(width,height)/2. Copy immutable material/geometry into SceneNode::AnalyticSurface, SceneNodeKind::AnalyticSurface=23. No style chooses target, clip, widget geometry or global Z. All existing node IDs remain unchanged.

The new direct renderer family is PipelineId::AnalyticSurface=13, verified unused after current 1..12; do not reuse Shape IDs or change HAL-358/388 vertices/shaders. Use the normal scene-diff, Batcher, immutable cross-pipeline schedule and existing Main/Overlay passes. No new RenderGraph pass, sampled image, resource producer, full-screen operation or second appearance resolver.

### 8.3 Final inherited clip, target and finite-capacity admission

Local emission does not attest future ancestor clips. During final composed-scene Build, the Batcher captures the complete current inherited stack. This profile supports any number of ordinary rectangular clips and at most ONE positive-radius rounded ancestor, using its original scroll-projected rectangle and all four radii, not the already-intersected rectangle that would move its corners. Preserve that raw source rect in the existing private ClipStackEntry; do not change legacy ClipState projection or any old pipeline's behavior. The rectangular intersection remains the existing scissor. A later inner rectangular clip must not erase the selected rounded ancestor.

While scanning, validate every rounded entry for finite nonnegative radii, including entries containing only zero or invalid radii. For each positive-rounded entry validate finite positive raw bounds and radii no larger than half its shorter side. A second positive-radius rounded ancestor is not represented by a single mask and must refuse the whole Build. Add DrawPlanBuildError::UnsupportedAnalyticSurfaceClip { surface_id, node_key, rounded_clip_depth } and InvalidAnalyticSurfaceClip with the same fields; depth identifies the second positive-rounded entry or the invalid entry respectively. When no positive-rounded ancestor is present, clip_enabled=0; for one admitted rounded ancestor, clip_enabled=1 and the shader always evaluates that coverage, independent of the old diagnostic disable flag. A changed outer clip must be requalified even when the material's local Arc/Paint is retained.

Use current renderer-owned DPR; first execution domain is finite [0.25,8]. Derived scroll-projected geometry and AA-expanded endpoints must remain finite and within the declared coordinate bound. Outside that domain return DrawPlanBuildError::InvalidAnalyticSurfaceTarget { surface_id, node_key } before Compile, not a fabricated SurfaceError.

The initial total visible analytic occurrence limit is 512 per complete DrawPlan, across surfaces, reusing the existing Gradient's bounded scale without borrowing its storage. Refuse excess with DrawPlanBuildError::AnalyticSurfaceCapacityExceeded { surface_id: SurfaceId, limit: usize, requested: usize } with limit=512, before producing a partial DrawPlan. Batcher local ordinals are checked and bounded; disabled/empty materials consume no occurrence. Existing Build Result propagation remains the only failure path; ordinary no-analytic scenes and renderer Compile/SurfaceError signatures stay unchanged.

### 8.4 Shader binding and ordered parameter ownership

Vertex location 0 is absolute logical vec2<f32>; entry points vs_main/fs_main use unchanged canonical group 0 Globals. Group 1 binding 0 is one dynamic AnalyticSurfaceParams uniform of exactly 384 bytes, in slots of align_up(512, device.min_uniform_buffer_offset_alignment) bytes. For a device with 256-byte uniform-offset alignment the slot is 512 bytes; this is arithmetic, not a hardware-measurement claim. The exact ordered fields are:
- 0..15 rect_origin_size.
- 16..31 gradient_p0p1.
- 32..47, 48..63, 64..79 straight-linear opaque base_0/base_1/base_2.
- 80..95 straight-linear rim RGBA, zero when absent.
- 96..111 own corner radii in tl,tr,bl,br order.
- 112..127 shape = bevel_width, bevel_slope, roughness, reserved zero.
- 128..143 optics = diffuse_strength, specular_strength, rim_strength, ambient_strength.
- 144..159 limits = max_interior_uplift, opacity, 0, 0.
- 160..223 light0 as direction_intensity, color_RGB_zero, center_radius and rotation_cos_sin_zero_zero.
- 224..287 light1 in the same order.
- 288..303 clip_control = enabled u32, 0, 0, 0.
- 304..383 existing 80-byte RoundedClipUniform layout for the ONE admitted original ancestor. No second parser or own uniform layout for that clip.

Compile uses renderer-owned parameter storage, vertex/index buffers and existing staging lifetime. Allocate the new pipeline/buffers lazily only when a valid nonempty analytic request is compiled. No analytic allocation/upload/draw occurs on old-only scenes. The bounded 512 occurrences fit 2048 vertices and 3072 Uint16 indices without overflow. Check target resource limits before a complete executable plan is returned; genuine GPU allocation failure follows the existing renderer resource boundary, never silent truncation.

Each occurrence has a deterministic checked one-based surface-local parameter ordinal. For PipelineId13 only, the existing DrawBatchKey parameter-selector lane (texture_id, already used for non-image Gradient parameters) carries that ordinal; it is NOT an Image registration or global resource identity. Keep image_generation=None and shape_type_id=None. Compile maps (actual SurfaceId, ordinal) to its frame-local dynamic offset in compositor order; distinct occurrences never merge or alias through a hash collision. Retained semantic identity uses actual material/geometry/clip values, not that transient offset. Required front/back order remains strict Z and closed control-subtree order.

### 8.5 Exact own shape and independently lit face

Let p be current absolute logical fragment position; U=(p-rect.origin)/rect.size. Let B be the existing piecewise linear interpolation of the three opaque base RGB stops at t=clamp(dot(U-p0,p1-p0)/dot(p1-p0,p1-p0),0,1).

Set q=p-(rect.origin+rect.size/2), h=rect.size/2. In logical coordinates positive x is right and positive y is down. Select tl/tr/bl/br using q.x<0 or >=0 and q.y<0 or >=0. Let v=abs(q)-h+radius, vp=max(v,0); signed distance d=length(vp)+min(max(v.x,v.y),0)-radius. If length(vp)>0 the 2D gradient is sign(q)*vp/length(vp); otherwise it is (sign(q.x),0) when v.x>=v.y, and (0,sign(q.y)) otherwise. Define sign(0)=0. A zero gradient yields the front normal; no zero-length normalization is performed.

Let e=clamp(1-max(-d,0)/bevel_width,0,1) for positive bevel_width, otherwise e=0. For e=0 or zero gradient use N=(0,0,1); otherwise k=bevel_slope*e and N=(k*normalize(gradient),sqrt(max(1-k*k,0))). This is one synthetic normal for the actual face, not a mesh or second outline.

For each of the exactly two lights, set delta=U-center_uv and rotate to (cos*delta.x+sin*delta.y,-sin*delta.x+cos*delta.y); divide each component by the corresponding radii_uv; E=exp(-0.5*dot(ellipse,ellipse)). L is the checked stored direction; V=(0,0,1); H=normalize(L+V). Define D=clamp(dot(N,L),0,1), p_spec=4+60*(1-roughness)^2, P=pow(clamp(dot(N,H),0,1),p_spec), F=0.04+0.96*(1-clamp(dot(V,H),0,1))^5. Smoked Glass is the dielectric profile; no metalness selector is silently admitted.

R=B*(1+ambient_strength)+sum(E*light_strength*light_RGB*(B*diffuse_strength*D+vec3(F*specular_strength*P))). Define Y(X)=dot(X,(0.2126,0.7152,0.0722)), allowed=Y(B)*(1+max_interior_uplift)+0.008*e. If Y(R)=0, S=0; if 0<Y(R)<=allowed, S=R; otherwise S=R*(allowed/Y(R)), whose quotient is in [0,1]. Then S=S/max(1,max(S.r,S.g,S.b)). Do not calculate an unguarded quotient, clamp independent channels to hide overflow or invent background samples.

### 8.6 Independent directional rim and final composition

Use the same actual N, E, D and P, but a separately authored rim strength. Define g=clamp(sum(E*light_strength*Y(light_RGB)*D*P),0,1), w=e*rim_strength*rim_color.a*g. If rim is absent or w=0, C=S exactly; otherwise C=mix(S,rim_color.rgb,w). Thus zero rim does not extinguish ambient/diffuse/face-specular light, and zero source radiance does not leave a constant gold outline. The rim's additional color mix is explicitly bounded by w<=0.20, confined to the bevel; the face interior uplift bound is unchanged. This is physically inspired 2.5D reflection, not energy-conserving PBR or emitted light transport.

Compute own_aa=max(fwidth(d),0.5/current_DPR) and own_coverage=1-smoothstep(-own_aa,own_aa,d). Expand only the raster quad by 2/current_DPR logical pixels to include this finite derivative-AA support; allocated layout/hit geometry never changes. Final alpha is opacity*own_coverage*ancestor_coverage; final RGB=C*alpha. Ancestor coverage uses the existing RoundedClipUniform/Gradient geometric coverage law on the captured original rounded rect, plus normal rectangular scissor, with each coverage applied once. No post-composition filtering, grain, dither, extra color transfer or changes to gradient.wgsl. An opaque or translucent material sees backdrop only through ordinary OVER blending.

No-light/no-ambient/no-rim equals the mathematical three-stop face before the existing Gradient anti-banding correction. Do not demand byte equality with the independently dithered old shader or silently disable its dither. CPU/GPU comparisons must isolate that known distinction.

### 8.7 Completion and proof

A real ordinary native App example must compare plain Gradient, Smoked Glass with rim disabled, the same material with directional rim enabled, and translucent Smoked Glass over real changing background content, without turning all other controls into glass. Include source light reversal, zero opacity and independently disabled rim controls in the owning tests. Product palettes/layouts remain unchanged; examples supply declared fixture values only.

Proof must cover exact input rejection and 384-byte packing; finite black/near-black and extreme valid illumination, guarded quotient branches and reported GPU subnormal behavior; alpha 0/0.5/1; actual own asymmetric corners/AA and one inherited rounded mask; outer-rounded plus inner-rect preservation; two-rounded typed refusal and clip-only retained reuse/recovery; 512/513 occurrence boundary; parameter identity across at least two surfaces; strict Z with text and unchanged focus/input; no idle frame demand and correct unmount. Actual 1x/2x GPU pixels are compared with the independent CPU equations, with format-specific quantization tolerance stated in the test. Separate actual native capture from headless evidence and from JC optical acceptance.

Preserve working Local Ambient Glow, LuminousContour, Gradient/Solid/Image/NinePatch, Button material and current Compile API. Do not rerun a previously timed-out broad renderer suite without a concrete new diagnostic objective; focused affected ordering and new material proofs are sufficient for this unit. The later Frosted Glass side-by-side visual comparison remains part of programme acceptance, not a reason to fake frosting or block this independently useful material.

## 9. S1 supervisory source and execution review (2026-11-13)

Independent review matched the checked constructor, exact384-byte parameter record, directed face/rim shader, raw-ancestor capture, typed final Build and retained tests to section8 and HAL389 section7. The retained execution evidence is EVIDENCE.json (SHA256 4704b8ff6bfd88dbacd2c400adc46638a5bb0e0613a922fcbf71b628e362d1f4). The final shader is c5f80098d3d671e32695bec31e9cd6f97343f54aa1ac4649e3823bb9f7a6dacd.

Independent review additionally completed typed Grip check plus the analytic_surface filter: zero errors,118 warnings and10 passed/0 failed. The implementation's initial two RED fixture/oracle premises are retained as history; the corrected helper-lane derivative oracle keeps the two-encoded-code tolerance. Native capture provenance and A/B comparison records were inspected, not independently accepted as JC appearance or physical scanout. Two winit diagnostics and the earlier broader renderer_z_order timeout remain unqualified. Technical source/API and focused CPU/Metal acceptance does not close all optical-effects work.

## 10. Coordinated HAL387 S1 micrograin consumer (2026-11-13)

Binding under the standing implementation mandate. HAL387 section7 adds one independent checked SurfaceMicrograin value to the existing SurfaceAnalytic owner. The original constructor and fallible paint operation remain source-compatible; with_micrograin adds or removes the optional immutable value. The texture acts on fully evaluated S before the independent rim mix, never on alpha, geometry, text or backdrop. The lighting cap continues to apply to smooth S; the additional reference-luminance texture budget is explicitly owned by HAL387, not additional illumination.

The original 384-byte record is the unchanged prefix of the new416-byte record defined by HAL387 section7.5 and HAL085A. Slot stride,512-occurrence limit, surface-local ordinals, pipeline13, node23 and all clip/target error ownership stay unchanged. None/zero grain preserves existing output; old shaders and SurfaceFill variants remain untouched. This does not implement Frosted Glass or extend grain to another unadmitted material.

## 11. Adopted S2: configurable optical surface, not a fixed glass preset (2026-11-14)

**Status: ADOPTED / BINDING design under JC's direct 2026-11-14 instruction. Implementation and visual acceptance are NOT established.** JC rejected the supplied S1 material screenshots as synthetic and required transparency, frosting, tint, surface light and edges to be configurable. HAL-002/002A remain unchanged and supreme. Historical source/GPU acceptance is preserved; it is not acceptance of the rejected appearance.


### S2 authority pre-flight

Objective: correct JC's rejected visual material as one configurable native capability, not replace old technical proof. Standing goal: independently usable glass, grain and lighting with credible user-visible appearance. Authority boundary: the governing owner selects nonconstitutional optical decisions; implementation remains separate; JC retains visual judgment. Mandatory entry completed: full Work Doctrine, PROJECT-ENTRY, HAL002/002A, PM000/026/028 and DESIGN-INTELLIGENCE; exact HAL012/024/084/085A/354/359/385-389, HAL207/210 and live material, shader, source-region, scene, clip and app-value owners were inspected. Constitution is unchanged; no new interaction policy. Display-only preview uses no input; actual standard controls keep their own policies.

Source decision: extend existing SurfaceAnalytic/FrostedGlassAppearance, Pipeline13/14, the final full-stack Build boundary and shared WGSL; do not create a second renderer or generic effect graph. Bounds, linear premultiplication, strict Z, retained equality, honest failure and original-command custody are named controlling invariants. The implementation task owns only exact affected paint/scene/batcher/material/filter sources, nearest tests, one ordinary example and proportional capability notes. Unrelated application work, text/input engines and all product consumers remain outside. Source-only authoring introduces no runtime or native PASS. Verification belongs to current paint/analytic/frost/retained/renderer tests plus the actual material lab, with new inputs proved once and old valid evidence reused. Existing version policy remains; no dependency, installation, destructive or remote effect is selected. The original observable material comparison is the closure gate, with physical/JC judgments explicitly separate.

### 11.1 Complete objective and reuse

Deliver one ordinary, live-adjustable native material study in which a thin clear surface, thin frosted surface and thicker lens-like surface are recognizably different on the SAME real background, and a luminous surface has light connected to its actual face/edge rather than disconnected ellipses underneath. All effects remain independently configurable values. Preserve the S1 examples and all old requests exactly. No automatic product skin, fixed gold/blue palette, web UI, shader graph, alternative renderer, global light manager or new frame/input authority.

The inspected S1 face limits reflected luminance relative to its dark base; Frosted Glass has no reflected front surface at all. Reuse both existing material owners and add a shared explicit optical coating, rather than silently removing the old face cap. A coating is applied after the existing Smoked Glass body/legacy rim, whose original parameters retain their meanings. Existing lit body and newly authored coating are explicit layers on the same silhouette; selecting the latter never changes old caller output by default. Its pure helper is shared with HAL385, not a second material renderer.

### 11.2 Checked optical value

Add immutable core::paint::GlassSurfaceOptics with private copied data and read-only getters. Its constructor is try_new(parameters: GlassSurfaceOpticsParameters, lights: [AnalyticSurfaceLight; 2], reflection_tint: Color, rim_tint: Color) -> Result<Self, GlassSurfaceOpticsError>. The input record has exactly seven f32 fields: bevel_width, bevel_slope, roughness, normal_reflectance, reflection_strength, rim_strength, rim_falloff.

All values are finite. Bevel width is zero or [0.25,32] logical pixels; slope is [0,0.98]; zero width requires zero slope. Roughness is [0.04,1]; normal_reflectance, reflection_strength and rim_strength are [0,1]; rim_falloff is [1,8]. A positive rim strength requires positive bevel width and slope. Tints are straight-linear RGBA in [0,1]. The two already checked AnalyticSurfaceLight values keep their current exact domains and normalization; no inferred light or mutable resource. Errors distinguish NonFiniteInput, InvalidParameters and InvalidColor. At emission, positive coating bevel width must fit within half the actual shorter side; use the existing material's typed InvalidGeometry when it does not.

Add SurfaceAnalytic::with_glass_optics(self, Option<GlassSurfaceOptics>) -> Self and glass_optics() -> Option<&GlassSurfaceOptics>. Existing try_smoked_glass initializes None. No new SurfaceFill variant, SceneNodeKind, PipelineId, light registry, paint entry or GPU pass. Add the independent SurfaceEmission value and with_surface_emission/getter under HAL388 section10; it is not implied by reflection. Old constructor arguments, legacy SurfaceRimLight, face controls, micrograin and zero-grain arithmetic remain intact.

### 11.3 Exact shared optical response

Use the actual four-corner SDF distance d and outward unit gradient g from section8.5, with zero gradient handled explicitly. Let s(x)=x*x*(3-2*x). For positive coating width b, e=s(clamp(1-max(-d,0)/b,0,1)); otherwise e=0. Let k=bevel_slope*e. With a nonzero g use N=(k*g.x,k*g.y,sqrt(max(1-k*k,0))); otherwise N=(0,0,1). This geometry is an authored optical coating on the existing silhouette, not canonical layout, physical glass thickness or a new coordinate space.

For each light i, compute the same rotated elliptical envelope E_i from its center/radii/rotation and U=(p-rect.origin)/rect.size. Let L_i be its normalized direction, H_i=normalize(L_i+(0,0,1)), exponent=4+124*(1-roughness)^2, P_i=pow(clamp(dot(N,H_i),0,1),exponent). Define a_i=E_i*light_strength_i*P_i and r_i=E_i*light_strength_i*pow(max(dot(N.xy,L_i.xy),0),2). These terms are reflection, not emitted light. Light RGB is C_i and peak_i=max(C_i). Let A=sum(a_i*peak_i), R=sum(r_i*peak_i). If A>0, reflection RGB is sum(a_i*C_i)/A multiplied componentwise by reflection_tint.rgb; otherwise reflection weight is exactly zero. Likewise use sum(r_i*C_i)/R times rim_tint.rgb when R>0. This normalization gives bounded hue without division by a small luminance and makes black/zero-strength lights exact no-contribution controls.

F=normal_reflectance+(1-normal_reflectance)*pow(1-N.z,5).
wf=reflection_strength*reflection_tint.a*F*min(A,1).
wr=rim_strength*rim_tint.a*pow(e,rim_falloff)*min(R,1).
Apply to any finite linear-premultiplied body Q:
Q1=(1-wf)*Q+wf*(reflection_RGB,1);
Q2=(1-wr)*Q1+wr*(rim_RGB,1).
Skip each operation exactly when its weight is zero; no color division is evaluated for a zero denominator. Both are convex four-channel operations and preserve 0<=RGB<=A<=1. No independent channel clamp, HDR overflow, second sRGB conversion or cap tied to body darkness. Zero rim preserves reflected face Q1 and the old body. Reversing light direction changes edge response without moving the surface. Reflection and rim do not paint beyond own silhouette/AA; emission owns halo separately.

For Smoked Glass, evaluate its unchanged S1 body, grain and legacy rim first. Pass (body_RGB,1) through this helper, then apply existing authored opacity and own coverage; integrate independently selected emission exactly under HAL388 section10. The initial S2 study uses legacy rim None so it does not stack two rim decorations accidentally. Keeping both explicitly authored is a layered appearance, not an implicit replacement. Smoked Glass NEVER gains backdrop sampling or refraction.

### 11.4 ABI, retained state and failure

HAL085A's S2 amendment defines AnalyticSurfaceParams=688 bytes, its preserved 416-byte prefix, coating/emission fields and new checked dynamic stride. Existing Pipeline13/SceneKind23, group0 Globals, group1 binding0, vertex layout and one ordinary draw remain. Actual (SurfaceId, ordinal) slots stay distinct; the count remains512. No analytic allocation for scenes that do not use this material.

New values participate in immutable scene equality, source validation and retained dependencies. Update publishes owned values; Paint reads its snapshot, not the control/model. Equal values, static optics and stable background create no clock or idle frame demand. Emission may expand only actual raster/culling/damage coverage; it does not change canonical rect, hit/scroll/focus/accessibility or clips. Final Build still checks the full inherited stack and one original rounded ancestor, even with a reused local Arc. Existing typed Build/Compile failure and sealed GPU-command custody remain unchanged.

### 11.5 Native material lab and visual gate

Add examples/glass_material_lab.rs, ordinary zero-argument App::run, initial size1280x820 logical. Use the existing Flex/ScrollArea/Slider/FormBinder owners, not hand-drawn input. The preview is display-only: no input handler, focus, capture, Escape interception or request_exit. Existing native close behavior remains. Real Slider controls emit through ChangeWriter; Screen::flush_pending_actions consumes FormBinder::bind / f32().take() once and stores one complete pending checked snapshot in the Screen. Screen::update uses its retained TreeWidgetId, WidgetTree::find_by_id and node_mut, verifies the current preview type, then copies the immutable snapshot through that preview's ordinary value setter. End the node/widget borrow before WidgetTree::mark_paint_dirty. Unchanged values do nothing; an absent/replaced mount is not silently targeted. The preview stores only its presentation value, never a reference to application state. No peer access from a widget, locks/borrows across UI calls, tree rebuild per drag, renderer-time mutation or new publication service. Do not generalize the unrelated Viewport3D-only G1 transfer.

The finite control panel exposes material selection, blur, transmission, body opacity, tint RGB/alpha, corner radius, coating width/slope/roughness/reflectance, reflection/rim strengths and colors, light direction/intensity and envelope, refraction displacement, grain amplitude/scale, and emission face/edge/halo strength, width/radius/direction. Remaining authored light/seed fields must be publicly usable and the complete current parameter snapshot inspectable as values. Frost-only controls are visibly inapplicable for Smoked Glass, not silently applied as fake blur. Slider scalar domains are fixed and match admitted domains. A coupled invalid setting is shown as an explicit validation result; it must not be silently clamped or claimed rendered. Normal controls keep keyboard/focus behavior; no custom interaction policy is introduced.

The comparison area contains immutable reference profiles plus the editable surface, with sharp foreground captions, a visible no-material reference and no full-opaque tint hiding the background. Use two original backgrounds: (a) large cool and warm shaded forms made with existing radial Gradient/shape draws, with at least one crisp small source feature; (b) a quiet real interface composition with lower-order text, rules and cards. Neither background is a rasterized glass screenshot or preblurred reference. Existing stripe diagnostics remain secondary tests, not the visual target. A finite ordinary Timeline may move only the background for12seconds, then genuinely settle. Knob interaction must still work after idle.

Starting fixtures are authored examples, NOT global defaults: common lights use unit direction from (-0.3,-0.4,sqrt(0.75)) at strength0.85, center(0.25,0.15), radii(1.0,0.65), rotation(1,0); secondary uses (0.3,0.4,sqrt(0.75)) at0.25, center(0.75,0.85), radii(0.8,0.8), rotation(1,0). Both are neutral-white linear lights. Reflection tint is neutral-white alpha1, rim tint=(0.8,0.9,1,1). Thin clear: sigma0, transmission0.985, tint=(0.03,0.04,0.06,0.25), opacity1, coating width1.25/slope0.9/roughness0.35/normal_reflectance0.10/reflection0.55/rim0.70/falloff1.5, displacement0. Thin frosted: same coating, sigma9/transmission0.94, displacement0. Lens-like: sigma2/transmission0.97, width16/slope0.95/roughness0.2/reflection0.7/rim0.65/falloff1.5, displacement12. These use the SAME geometry and background for a meaningful comparison; both warm and cool backgrounds must remain usable. Grain starts0.6/255 at scale1.5, seed387 and is independently disabled for isolation. Include a dark luminous action-size surface with SurfaceEmission color from sRGB(0.82,0.74,0.56,1), face0.025/edge0.20/halo0.10, edge_width2, halo_radius18, falloff2, directionality0.6 toward(0,1); not a mandatory gold preset.

JC's seven attached screenshots are the visual evidence: four S1 samples were rejected; references show thin blue glass over large forms, purple/pink thin versus thick edges, and a frosted photo panel. These are appearance references, not proof of their authoring algorithm or licensed assets in this repository. The reference images remain separate evidence; their availability must be established before relying on them. The stated visual criteria are operative: background color masses survive, the panel is not a flat grey blur rectangle, the edge follows the rounded shape and light direction rather than a uniform outline, lens displacement is confined and independently absent for thin glass, and emitted light visibly connects to the same face/edge instead of detached half-ellipses.

### 11.6 Complete proof and scope

Prove checked input/ABI/zero equivalence, exact independent CPU coating/emission equations, alpha and transparent controls, independent disabled/reversed lights and rims, actual1x/2x Metal, full inherited clip under retained reuse, changing background with unchanged glass local state, and the original sealed command/cancel/export regressions after buffer changes. Preserve the old two-encoded-code tolerance for old pixels; use independently derived new-oracle tolerances no looser than two encoded codes for ordinary controls. Expected ABI sizes/stride and grain limits change deliberately under the coordinated text, not by loosening unrelated assertions.

Run typed Grip analytic_surface, frosted_glass, local_ambient_glow and strict_z_ owner filters with check as specified by the task. Reuse unchanged full-suite and other-project evidence. Build and run the native lab; record owned-window before/after on identical backgrounds, thin/frosted/lens and connected emission, plus a real normal control change and settled state where the admitted native route can prove it. In-process routing proof is not physical input; unavailable native interaction must be reported, never fabricated or bypassed.

Inspect the actual captured pixels against the above criteria, not just logs or a screenshot hash. A successful numeric oracle alone is insufficient for visual programme closure. Return native originals and exact values for independent review and JC assessment. Source/API, CPU/Metal, actual window capture, physical input/scanout and JC aesthetic acceptance remain separate. No full scene relighting, environment map, chromatic dispersion, HDR, cross-surface source, product UI migration, new input feature or unrelated diagnostic work. Old examples and working shader behavior remain available.

## 12. S2 independent review and lab clip-ownership correction (2026-11-14)

**Disposition: NEEDS CORRECTION for the complete S2/native-lab unit.** Independent review examined the checked optical/emission values, shared shader equations, 688-byte analytic packing and aligned slots, 1344-byte frost packing, V/Q/S projection and four-channel replacement against section11 and the coordinated owners. Retained typed execution receipts establish analytic_surface19/19 with check0 errors/126 warnings, frosted_glass12/12, local_ambient_glow7/7 and strict_z_3/3 at their recorded inputs. These are independently inspected implementation receipts, not newly repeated tests. They remain valid evidence for the material mechanisms, not acceptance of the entire native consumer.

The inspected examples/glass_material_lab.rs (SHA256 ea22d4cd73bbda66f704e9b031be6da763ca2cd8623c64c4ab3893bc70bae19e) manually opens/closes per-cell clips in Background::paint at640/735 and Preview::paint at837/908. HAL024 LayerA section5 assigns ClipNode creation exclusively to traversal; HAL088 sections3/7 and HAL354 section3.2 supply the existing owner-correct route. No optical adoption grants a widget an exception. The native example therefore cannot be accepted unchanged merely because renderer tests pass. This finding does not invalidate the demonstrated Gaussian/coating/emission equations or authorize a shader rewrite.

**Selected exact correction, under the existing mandate:** keep one ordinary native lab and all current controls, fixed/editable profiles, visible parameter/validation output, geometry, source content and post-idle interaction. Represent each of the six comparison cells as a real tree-owned layout host with OverflowMode::Clip, and place separate display-only background and material/content children beneath it. Layout assigns the same relative cell rectangles; canonical traversal supplies inherited clips and scroll projection. Widgets must not emit clips, paint peer/child widgets, retain tree references, or introduce a second hierarchy. Keep foreground text strictly above the material and real source text strictly below it. Keep background and material fragments separate so motion does not require repainting unchanged glass/text.

Each background child uses the same existing finite12-second engine Timeline animation, started at the same lawful Update boundary; there are exactly six such bounded owner-qualified handles, no independent clock and no additional perpetual demand. Their equal samples must be checked for a common comparison frame. The Screen retains the six background IDs, the editable material ID and status-view ID, transfers complete checked values through the existing FormBinder/flush/Update path, and invalidates only changed owners after all node borrows end. Material-only changes leave backgrounds and fixed profiles unchanged. Background selection updates the six background owners. Static header/status/action material outside the comparison cells retains ordinary traversal and separate display-only ownership. Canonical layout, hits, focus, input and ScrollArea semantics are unchanged.

Use one public integration target tests/glass_material_lab.rs that includes the actual example as a Rust module; its cfg(test) tests exercise the actual lab construction/public WidgetTree path, not a duplicate replacement lab or private framework API. Prove the six real clipping ancestors, separate background/material children, no widget-emitted clipping in their local paint, preserved mounts under a parameter update, unchanged fixed-profile state, and settling. Source guards alone are not visible-order proof. The existing public AppContext/ScreenFrameContext/WidgetTree and fresh build_scene_with_appearance path may be reused without changing production framework APIs. Native proof then reruns the original ordinary A/B/IDLE, post-idle Blur9-to18.75 and UI-backdrop scenarios after the correction, using the already qualified capture/input route and stopping input on unexpected window-identity/bounds changes.

The prior native logs and capture provenance remain evidence of the original run. Independent visual review covered only a reduced, lossy cropped derivative of the frosted panel, not the complete original native images at1:1; neither that derivative nor hashes establish visual acceptance. Corrected actual-window originals and useful1:1 crops must remain available for independent review and JC assessment. JC optical acceptance stays OPEN. No physical input/scanout, native-close acknowledgment, winit repair or broad-suite completion is inferred.

Preserve unrelated work, all material/shader/resource/custody source, old examples and existing evidence. This is correction of the original consumer under unchanged law, not a new framework capability or product skin.

## 13. S2B canonical clip-source repair and stable backdrop order (2026-11-14)

**Disposition: S2/S2A native consumer remains NOT ACCEPTED.** The preserved S2A candidate removes widget-authored clips and supplies six real Clip hosts, but its public scene test fails at zero scroll. HAL088 section 17 now grants the bounded WidgetTree/App source correction. It deliberately repairs raw child-clip ownership rather than globally reinterpreting legacy viewport metadata or inserting scroll arithmetic into widgets. All existing material, grain, emission and exact-command custody contracts remain unchanged.

A separate JC recording establishes a temporal symptom in the no-material source itself. Exact original SHA256: 99eecbf20d7b1e0d021da1f067dc02cc62929c1f520c8faa3e1e2b9335d4faf2. Independent review verified its original hash and inspected unresampled 80x80 overlap crops at source x1000/y300 from frames1/2; the blue/pink foreground changes. The retained analysis reports262 decoded frames and124 large ROI transitions. That is recording evidence, not Present frequency, a matched binary, a GPU trace or proof of post-Timeline idle. This coordination notice itself grants no source-change authority; the existing recorded JC visual-correction mandate governs this repair.

Live Background::paint emits both overlapping blue/pink radial gradients at z2. GradientParamsData hashes their changing positions; DrawBatchKey::gradient stores that hash in texture_id, and the existing comparator uses it after equal effective Z/clip/pipeline. Thus the authored scene violates HAL359 and PM006/007 regardless of whether this current source is the binary in the recording. Correct the producer; do not change renderer tie-breaking or blame Gaussian/Surface::configure without evidence.

### 13.1 Exact producer order

For all six comparison cells preserve the existing shapes, colors, motion and parameters and assign:
- background fill0, cell base1;
- blue radial form2, pink radial form3;
- the large-form vertical detail4 and horizontal detail5;
- UI source card2, rules4, source text5 and source indicator6;
- existing material10+cell_index and foreground captions100 unchanged.

Pink is explicitly in front of blue throughout the common finite Timeline. This is a fixed authoring relation, not insertion-order or hash authority. Disjoint equivalent rules may share a Z; overlapping classes requiring precedence may not. Source text stays below every material, and sharp foreground stays above it. No material/profile removal, paused-animation workaround, geometry changes or disabled controls.

### 13.2 One complete correction and evidence

Both existing-law corrections continue the preserved S2A candidate as the same finished native-lab unit. No cross-project source change is admitted.

Preserve the original S2A RED and user recording. Add source-bound temporal proof using the actual lab scene producer: sample the common Timeline, verify strict blue/pink effective order through real Build commands, and retain the old equal-Z control's measured ordering reversals if reproduced. Pair the producer proof with focused actual Metal readback at1x/2x using the same gradient parameters and selected adjacent phase values. Actual pixel overlap must match the selected foreground, not merely a sorted-source assertion. Do not assert that current-source reproduction identifies the historical video binary.

Correct the unexecuted nonzero-scroll oracle as specified by HAL088 section17; its expected raw/layout and displayed rectangles are different facts. Prove the original six clipping hosts, visible/retained behavior and unchanged FormBinder/flush/Update parameter publication, mounts and finite settling. Reuse unchanged optical/custody numerical receipts; do not repeat the41-test material set or broad renderer_z_order by ritual.

After the canonical proof succeeds, run the same ordinary zero-argument native lab and return A/B/IDLE, the real post-idle Blur9-to18.75 change and the UI backdrop. Include a bounded temporal owned-window capture across moving background and recorded Timeline completion, not just three isolated stills. Check an overlap region that remains inside both form interiors against the fixed front relation, and distinguish legitimate movement from occlusion reversal. Confirm confinement to the six cells and sharp foreground. Retain originals and useful1:1 crops; inspect actual pixels. Use only the existing qualified capture/input route, stop input on unexpected target/bounds changes, and reap only the owned child.

JC optical acceptance remains OPEN. Clip success alone does not close temporal flicker, temporal success does not certify all optics, and capture does not establish physical input, scanout or native-close acknowledgement. No speculative winit fix, new render pass, diagnostic framework, default skin, model work or consumer migration. Programme completion must not be inferred from a test count.

## 14. S2B independent review and isolated native-control completion (2026-11-14)

**Disposition: canonical clip/order source corrections ACCEPTED; complete native-lab scenario remains PARTIAL.** Independent review inspected the live WidgetTree/App sources, actual lab construction and strict blue2/pink3 producer, source-bound temporal test, final typed receipts and evidence identities. HAL088 section17.4 records the canonical-owner acceptance. The actual Lab/common-Timeline Build proof covers22 phase samples; the old equal-Z test control reproduces10 front-order reversals, while twelve Metal cases across1x/2x match the opaque pink-only foreground with worst difference0 under the unchanged two-code tolerance. This identifies and corrects the current producer defect without attributing a historical user video to an unknown binary.

Original A/B/IDLE capture hashes match their receipts. The native evidence records six identical A/B/completed phases and16 sampled overlap observations with pink in front, largest adjacent mean-channel change0.57 codes. These are bounded sampled-window facts, not continuous Present/scanout proof. Independent review verified provenance and the numerical report; it did not include viewing the full original S2B pixels. No JC appearance acceptance is inferred.

### 14.1 Exact remaining failure

The retained native log contains1893 requested-state records after the six IDLE markers. Changes span Blur, transmission, tint channels/alpha and corner radius; the controller records one authored click only. Actor and ingress are unknown. The final observed Blur14.75/transmission1.0 cannot establish the required isolated Blur9-to18.75 result, and UI-backdrop switching was not attempted.

The inspected private `native-lab.py` checks Timeline completion, binary/source, PID/parent/window, foreground and bounds before posting, but its log handler only prints requested/published values. It does not reject an unexpected parameter delta or enforce a clean initial state before the next post. This is a concrete proof-controller gap, not evidence of a Halcyon Slider, FormBinder, capture or native-input defect. Do not modify those owners or invent an actor.

### 14.2 Selected completion of the SAME S2B objective

This continuation completes the original live-adjustable lab; it introduces no new feature and does not merely repeat an unchanged run.

The only admitted ordinary-example source addition is one unconditional initial-state diagnostic from the existing `Lab::new` construction: print `HAL_OPTICS_S2 initial={:?}` using the exact initialized Request that the same Lab retains. Use the existing Debug representation and output channel; add no state owner, clock, switch, callback, remount or input behavior. Do not create a separately computed initial configuration. All library/renderer/shader/custody/manifest source and previously accepted example behavior remain untouched.

In a fresh private evidence directory, extend the existing qualified native controller with a bounded state machine driven by that initial record, actual requested/published records and the existing owned-window checks. Parse the complete finite Request, including all numeric fields, material/background selections and coating/emission booleans; a substring containing18.75 is never proof. Preserve original logs and attach receipt offsets plus controller monotonic observations without claiming input ingress timestamps.

Before the first scripted input, require exactly one valid initial record, no requested-state change, all six actual Timeline completions and unchanged owned window/source/binary/foreground. Capture the idle baseline. If any unsolicited state request, ownership/bounds change, refusal or malformed record occurs, stop further input immediately and report the first exact failed premise. Do not reset values, suppress real input, acquire new permissions, block the user, intercept other processes or automatically restart.

The next permitted state is the initial Request with ONLY Blur changed9-to18.75. Require that complete request and the corresponding successful ordinary publication (`preview=true`, `background=false`, `status=true`, `rebuilt=false`), then inspect an actual settled owned-window capture with sharp foreground and unchanged fixed comparisons. An unexpected field change invalidates this gate even if18.75 also appears.

Only after that gate succeeds, the next permitted state changes ONLY background Some(0)-to-Some(1), retaining Blur18.75 and every other setting. Use the existing RadioGroup and exact current visible target, not a direct binder write. Require the ordinary publication and actual changed interface backdrop with the same fixed/editable material values. Recheck exact owner, foreground and bounds immediately before every post. Preserve outbound modifier assignment/readback and distinguish posting from a received state/pixel effect.

Qualify the controller without native posting first: finite recorded-input cases must cover initial success, a non-Blur delta before input, a mixed delta after the Blur post, the exact Blur request/publication, the exact subsequent background request/publication, missing initial data and unexpected window identity. Use no live application for these parser/transition cases. They validate the proof controller only, not product input.

Then execute at most ONE ordinary zero-argument native run with the current qualified example, existing240-second lifecycle/2MiB log bounds, and the exact existing native helper/capture path. Reuse S2B moving-background/CPU/Metal receipts rather than replaying those suites or collecting another temporal series. The12-second Timeline naturally runs once to the baseline. Use the existing bounded event/selector waits, not sleep loops. Stop at the first failed premise; no automatic second run. Dispose only of this owned child through the existing supervisor, preserving accurate exit semantics.

### 14.3 Scope and closure

Allowed repository edit: only the initial-state diagnostic in `examples/glass_material_lab.rs`. Private controller, no-post fixtures and new native originals belong outside the repository in the new owned evidence directory; previous directories are immutable inputs. Run one typed package check and the narrow registered example build for the changed example, not the already-green material, clipping or broad renderer suites. No version change or capability note is implied by a diagnostic-only example addition.

Return one complete receipt distinguishing controller no-post proof, actual state transitions, native original pixels, and any missing premise. Independent numerical material and source acceptance remains valid; full native completion requires both isolated post-IDLE control and UI-backdrop observations. JC optical judgment remains OPEN. No product adoption, physical-input/scanout claim, winit diagnosis, new input policy, general trace service or unrelated source work is authorized.

### 14.4 Independent native-control acceptance; controller timeout retained (2026-11-14)

**Disposition: ACCEPTED for the two isolated native control observations required by section14.3.** This closes that functional native-lab continuation, not JC optical acceptance or the wider visual programme. The native-control evidence is retained with the source and execution record.

Independent review inspected the current `Lab::new` diagnostic, the private controller's complete-state admission and capture/review transitions, the original3220-byte native log, state/effect/capture/terminal receipts, and the recorded typed check and15-case no-post parser proof. Exactly one initial snapshot and two requested snapshots contain all59 numeric values plus material/background/coating/emission. All six finite Timelines completed before input. The first request changes only Blur9-to18.75 and publishes preview=true/background=false/status=true/rebuilt=false. The second changes only background0-to1, retaining every material setting, and publishes preview=false/background=true/status=true/rebuilt=false. Exactly two guarded authored pointer sequences are recorded; no unsolicited state request is present in this run.

Independent review verified all three original PNG identities and independently decoded their unresampled RGBA content. Across the six544x408 cell regions, baseline-to-Blur is byte-identical in the five fixed comparisons; the editable region changes346524 channel bytes, with maximum channel-code difference16. The UI-background change affects all six regions. All three stored editable crops exactly match the corresponding regions of the original captures. Independent visual review covered a544x408 lossy WebP derivative of the final editable crop at unchanged spatial extent; this is not a claim of full-original visual inspection, colourimetric accuracy or subjective optical acceptance. Implementation-side original-image reviews, including the final post-exit review, remain separately attributed.

The background image was captured at observed152.956s, after the exact second publication at148.610s and before the240s watchdog. The controller required its last image-review acknowledgment to transition from background_captured to complete; that acknowledgment did not arrive. Its exit20/watchdog, complete=false, owned child exit-15 and later separate pixel-review receipt remain unchanged. This review does not synthesize an in-run acknowledgment, native-close/caller-cleanup success, or a successful supervisor exit.

The final review was not a prerequisite to any later input: no third post was planned or performed. A saved, identity-qualified image remains reviewable after child termination. Together with the exact state/publication evidence and independent original-pixel comparison, the retained observation satisfies section14.3 without another application run. No native retry, controller rewrite, duplicate test run or new implementation follows merely to obtain a green administrative terminal flag. The earlier unexplained parameter activity and two winit diagnostics are not assigned a cause or declared repaired. Existing source/CPU/Metal and sampled temporal acceptance remain bounded as recorded; physical input, scanout, JC aesthetic acceptance and product adoption are not inferred.

## 15. Authored side-reflection and edge-glint study (2026-11-14)

**Status: ADOPTED / BINDING consumer configuration under JC's continuing visual mandate.** JC identified subtle surface reflections and localized side highlights as the missing glass cue and requested application of that observation. This unit applies the already implemented section11 coating, not another optical model. HAL002/002A, HAL385 current-prefix/custody, HAL088 child-clip ownership and HAL359 strict order remain unchanged. Technical acceptance of S2/S2B remains historical evidence; no new appearance acceptance is asserted.

### 15.1 Reuse and exact scope

The source-qualified glass_optics.wgsl already evaluates both reflected face and directed rim from the same two checked lights. AnalyticSurfaceLight already carries envelope rotation and linear RGB; the lab hardcodes these to (1,0) and white. Apply this capacity through examples/glass_material_lab.rs. Keep the six cell hosts, source geometry, original five fixed comparison values, original fixture_lights/fixed_profile outputs, connected-emission comparison, all existing controls, background choices, finite Timeline, strict order and canonical clips intact. Only the editable starting illumination is deliberately changed. Do not create another window/example, rendering path, light service, public type, shader expression, pipeline, uniform field, resource or timer.

### 15.2 Selected editable values

The starting editable request remains Frosted, Blur9, transmission0.94, signed displacement0, body opacity1, tint linear(0.03,0.04,0.06,0.25), corner18, grain0.6/255 at scale1.5/seed387, coating=true and emission=false. All unlisted scalar defaults remain unchanged.

Change the editable coating to bevel_width1.75, slope0.94, roughness0.28, normal_reflectance0.08, reflection_strength0.90, rim_strength0.65 and rim_falloff1.5. Reflection and rim tints are both neutral linear(1,1,1,1). This is an authored lab starting value, not a library or product default.

Use two explicit neutral-white illumination components on the same upper-left side. Each is passed through the existing checked AnalyticSurfaceLight constructor:
- Light0: azimuth -135 degrees, elevation75 degrees, strength0.95, center_uv(0.16,0.40), radii_uv(0.13,0.70), envelope rotation -20 degrees.
- Light1: azimuth -135 degrees, elevation25 degrees, strength0.85, center_uv(0.06,0.10), radii_uv(0.11,0.16), envelope rotation -20 degrees.

Direction remains (cos(az)*cos(el),sin(az)*cos(el),sin(el)); envelope rotation is (cos(angle),sin(angle)). Degrees convert once through the ordinary CPU value construction. The first component supplies an elongated soft face reflection; the second concentrates the edge response near the upper-left rounded corner. Both feed the SAME existing reflection/rim equations. Neither is emissive, a foreground stroke or a backdrop drawing. These values are a technically selected appearance candidate, not a claim of measured physical optics or visual success.

### 15.3 Independent controls without new input authority

Append eight scalar fields to the existing K/Request schema, preserving indices0..58: Rot0, LightR0, LightG0, LightB0, Rot1, LightR1, LightG1, LightB1. COUNT becomes67. Add ordinary Slider entries in that order: rotations [-180,180] degrees, explicit step1, initial-20; each source RGB [0,1], explicit step0.01, initial1. Source alpha stays the existing required1. Build checked colors from these three linear channels and the unit cos/sin pair from each rotation field instead of hardcoded white/identity. No angle normalization outside the declared domain, hidden clamp or string-parsed optical state.

All old fields and independent reflection/rim strengths remain directly editable, including zero, as do blur, transparency, tint, geometry, refraction, grain and emission. Keep FormBinder -> flush -> one checked pending Request -> Screen::update -> qualified retained MaterialView setter -> paint dirtiness. No rebuild or peer-widget writes. Preserve explicit invalid/inapplicable feedback. StatusView must report the complete67-field snapshot and remove its obsolete assertion that both source colors/rotations are fixed. Retain the one exact initial-state diagnostic.

### 15.4 Proof and actual visible result

Extend the existing example/public tests and src/tests/renderer_z_order.rs only where needed. The actual lab fixture must prove checked67-field construction, independent rotation/color changes reaching the ordinary value path, exact no-op cleanliness, unchanged mounts/canonical geometry/background/five fixed material values, and independent reflection/rim zero controls. Existing six-cell clip, finite settling and strict source-order regressions remain intact.

Add a focused glass_material_lab-prefixed real Metal1x/2x comparison using the actual authored material values and admitted scene producer. Compare coating absent, reflected-face-only, rim-only and combined on identical sources; use the existing independent color/oracle owner and unchanged two-code tolerance. A mirrored source fixture changes direction azimuth -135 to -45, center.x to1-center.x and rotation sign, preserving all body/geometry parameters; it must follow the expected right-side response, not a fixed screen border. Record actual region/pixel differences and original images. No whole material/custody/broad-renderer suite replay when its inputs are unchanged. Run the exact focused glass_material_lab filter with a package check and the ordinary example build for this changed consumer.

The visible target is a mostly transparent/frosted body with a localized elongated reflection and a short brighter rounded-edge segment. The opposite edge must not become an equally bright outline. Foreground text stays sharp, background color masses remain visible, emission remains off and there is no outside halo. Numerical agreement alone does not establish this appearance; inspect the actual original captures on large forms and ordinary interface content.

Use the already qualified native owner/capture/input route and complete-state checks, adapted to67 fields, with no new permissions or interception. At most one ordinary zero-argument run: after six actual Timeline completions capture the lit baseline; toggle only coating off and capture; toggle only coating on and capture its recovery; then change only background0-to1 and capture the ordinary-UI case. All numeric settings and other flags remain unchanged across that sequence. Revalidate exact process/window/foreground/bounds before every post; require the exact full-state request, publication and capture before the next post, with no unsolicited changes. Use native Checkbox/RadioGroup input, never a direct binder write as native evidence. If a premise fails, stop further input and report it without resetting the human's state or automatically repeating the run.

After the last successful capture and final state drain, end the owned observation and reap only its child immediately under the existing bounded supervisor. Review all saved original pixels offline; do not keep the application waiting for a final conversational image-review acknowledgment. Preserve actual exit/cleanup facts, do not synthesize a native-close result, and never relabel an earlier watchdog. Before any later action that depends on visual review, that review must be complete. Existing240s/2MiB limits and no-sleep execution remain. Report a deficient appearance honestly; no shader or product-scope expansion is delegated.

This coherent unit owns only examples/glass_material_lab.rs, its existing public test inclusion when necessary, the focused renderer test spans and one proportional example note in CHANGELOG. All library/input/Markdown/manifest/other-project work and architectural authority remain outside. JC optical acceptance remains separate. No implementation, native result or execution-queue observation is claimed by this selection.

### 15.5 Independent review of the authored side reflection and glint (2026-11-14)

**Disposition: ACCEPTED within the prescribed consumer source, focused CPU/Metal and ordinary native OFF/ON/UI observation boundary.** This completes the bounded side-reflection and edge-glint delivery. JC optical acceptance and the wider visual programme remain OPEN; no product adoption or new implementation task follows from this record.

Independent review inspected the live 67-field schema, exact section15 defaults, checked source rotation/linear-RGB construction, unchanged publication owner and added source-control tests. All three owned source images match the implementation's final manifest. Independent extraction reproduces the recorded fixed fixture-functions hash; removing the one new renderer test reproduces the exact prior renderer-test source hash, including its existing oracle. Thus the original five fixed material profiles and previous renderer tests were preserved, not merely described as preserved. No library/shader/ABI/pass/resource-custody or input-policy change was selected.

Original retained native Grip evidence was reviewed without rerunning it. The final check and focused filter both exited0, with0 errors/127 warnings reported; the filter contains eight library and six public test executions, with six included example cases exercised in both contexts, not fourteen distinct tests. The ordinary example build exited0. The actual Lab-derived checked publication and canonical clip feed the new Metal comparison; prefix and foreground are isolated from that same scene, using the renderer's admitted text engine. All twelve cases across1x/2x retain the two-encoded-code tolerance, with maximum observed error1. Black-source output equals absent coating, foreground opaque cores remain sharp, and the directional/mirrored and no-halo assertions pass. At2x, combined left/right mean positive encoded gain is8.116645/0.099188; mirrored gain is0.103468/6.805092. These are reference image differences, not monitor measurements.

Independent review inspected the private controller and original log, complete-state/effect/capture/exit records, and the14-case no-post controller receipt. The native run has one initial67-field snapshot, six completed finite Timeline owners before input, and exactly three full-state changes: coating off, coating on, background0-to1. Every numeric value, material choice and emission flag stays unchanged. Ordinary publication, current target guards, outbound zero modifiers and actual capture effects remain distinct facts. The supervisor completes successfully in33.08s and disposes only its owned child; the child's SIGTERM/-15 result is not a native-close acknowledgment.

Independent review verified all four original PNG identities and decoded their unresampled RGBA content. All five fixed544x408 cell regions are byte-identical across initial/off/on; the editable initial/on region is also byte-identical. The coating off comparison changes181458 editable channel bytes with maximum84 codes. The UI background affects all six regions. Each of the three stored496x276 face crops equals its exact original screenshot region.

Independent visual inspection covered reduced248x138 lossy OFF/ON/UI face derivatives and an unresampled96x64 lossless ON upper-left corner crop, without the monitor profile in those review derivatives. The inspected candidate shows a localized lighter left-side surface region and a short light arc following the rounded upper-left edge, rather than a uniformly illuminated outline; the UI-source preview remains filtered behind readable foreground. This is a limited direct visual observation, not full-original-window review, colourimetric fidelity or JC's aesthetic judgment. The implementation-side separate inspection of the four original2560x1704 images and lossless crops remains separately attributed.

Existing winit diagnostics, prior failed development receipts, historical controller timeouts and unknown-actor observations retain their original dispositions. No physical-human-input, Present/scanout correlation, native-close, whole-renderer or wider-programme PASS is inferred. No new native launch, input, build or test was performed for this independent review.
## 2026-11-18 coordinated scoped analytic/coating adoption: HAL-393/394/395 [S1 Binding]

HAL-393 admits real-time parameter-only movement of the already existing two checked AnalyticSurfaceLight inputs through HAL-021/HAL-354; no shader-owned time or new source capacity. HAL-394 admits only the optional film field on the existing shared GlassSurfaceOptics with the fixed three-sample RGB surrogate and exact checked domains; the two existing material-consumer prefix records and old unchanged output when absent are preserved under HAL-085A's versioned 704/1360-byte optical profile.

HAL-395 admits one separately authored optional SurfaceBloom on this SAME GlassSurfaceOptics value for a positive-opacity SurfaceAnalytic or FrostedGlassAppearance material. The existing Smoked body, coating and emission equations stay intact; the new independent checked HDR optical source is evaluated without sampling backdrop or changing the original opacity, then passes only through HAL-084's admitted renderer-owned graph for an SDR Bloom result. A selected effective Bloom with body opacity zero MUST give typed UnsupportedBloomOpacity before current material geometry can silently yield no node; absence/zero Bloom retains previous opacity-zero and independent SurfaceEmission behavior. No default style, extra paint/render owner, physical glass/refraction claim or combined film/Bloom light-color law is implied. This is binding S1 technical design under JC's delegated decision, not source/native acceptance.

