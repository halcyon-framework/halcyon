# SPEC-R-026: Environment and Indirect Lighting

**Author:** JC  
**Date:** 2026-10-31  
**Status:** Adopted / Binding [S1 — PbrEnvironmentIndirectV1 under HAL-370]  
**Adoption:** PERFORMED 2026-10-31 through coordinated S1 technical adoption under JC's express decision delegation; JC retains final veto.  
**Adoption record:** HAL-370-R023-R028-S1-COORDINATED-BINDING-ADOPTION-01.
**Implementation status:** NOT IMPLEMENTED / S2 NOT AUTHORIZED / S3 source/numerical/compiler/Device/backend OPEN / S4 native/performance/JC OPEN. NOT IMPLEMENTATION-READY for engineering or user-facing admission.  
**Layer:** Optional specialized 3D environment/material evaluation and source-qualified static diffuse transport  
**Design provenance:** HAL-R026-ENVIRONMENT-INDIRECT-LIGHTING-DRAFT-01, corrected by HAL-R026-EG-WORLD-FRAME-ADMISSION-RESOLUTION-01. JC retains final ownership and veto.

## Pre-Flight — historical original authoring record, 2026-10-31

This and the following Halcyon Pre-Flight retain the original inactive-draft task and single-file scope. The current header and §1 supersede their status statements only; original source/proof receipts remain historical.

- Objective: Produce one complete inactive environment/indirect contract with the architect-fixed E/G algorithms, world-frame admission, ABI, finite bounds, lifetime and proof obligations.
- Standing objective: Complete the finite R-023 through R-028 native-quality architecture without changing existing engine behavior.
- Decision ownership: JC retains product and constitutional authority and final veto. The delegated architectural review owns the exact technical choices; documentary execution is limited to inspection, transcription and consistency verification.
- Historical drafting boundary: The complete Draft and evidence require architectural review; no automatic R-028, parent amendment or implementation is authorized.
- Mandatory startup and authority entry chain completed: Current AGENTS, PRE-FLIGHT, ENGINEERING, PROJECT-ENTRY and HAL-002/002A entry completed; unchanged reading reused under the efficiency law. R-023/024/025/027 and governing HAL-355/354/357/263A/266A/295A/309 read; precise changed frame clauses and actual source seams requalified before writing. No UI-design branch is triggered by this document.
- Governing authority and status: HAL-002 Foundational and HAL-002A Permanent are supreme; named HAL parents retain their binding scopes, HAL-309 its adopted documentation authority. All referenced R-023/024/025/027 proposals remain inactive.
- Authority change, if any: NONE effective. Proposed requirements below do not activate themselves or amend a parent.
- Target layer and existing state verified: Halcyon 0.227.18-beta, HEAD `46c9d00d`; legacy three-group raster and surface-format offscreen owner. R-026 was absent. Existing dirty R-023/024/025/027 and the independently authored TagInput paint change are outside this unit.
- Owned mechanisms and files: CREATE ONLY this document; no other path or runtime mechanism is modified.
- Named invariants: Constitutional ownership/IDs/phases/immutable values/sovereignty; complete HAL-355 source/lease/front truth; HAL-266A current target; HAL-295A same-Queue publication; HAL-263A premultiplied-linear Image; unchanged proposed R-024 material, R-025 direct/shadow and R-027 output duties.
- Forbidden behavior: Hidden environment/ambient/GI, arbitrary source-frame interpretation, runtime frame transform, implicit rebake, fifth bind group, new renderer/clock/publisher, raw GPU/WGSL escape, silent quality downgrade, stale bake relabel or invented conformance.
- Existing mechanisms inspected and reuse decision: Context3D typed generations/leases, flat RenderSnapshot/ModelState, Viewport3D, existing target preparation and private offscreen/Image lifecycle; E/G extend only their separately admitted optional PBR family.
- Relevant external or dependent boundaries: Independent producer owns original HDR assets, source/world mapping, preintegration and static transport proof. R-028 separately owns general mip/LOD quality. No importer, asset download or consumer edit.
- Test ownership and placement: Documentary math/layout/source review now; future existing three_d_context/geometry/appearance/raster/backend/snapshot/viewport and custom_render_preparation owner tests, plus canonical App::run native evidence.
- Exact runnable verification commands and expected proof: Grip block `ws halcyon`, `read docs/specs/SPEC-R-026-Environment-and-Indirect-Lighting.md 1 1000 max-lines 1000`, `read docs/specs/SPEC-R-026-Environment-and-Indirect-Lighting.md hash`, `state git`; entire artifact, exact hash and only one new attributable file. No Cargo/Device/native job.
- Output or artifact completeness boundary: Original A–L decisions and all eight frame-resolution clauses integrated; full equations, byte tables, eight presence cases, paired duties, worked cases, source references and OPEN gates.
- SemVer and changelog impact: N/A; no executable or public API change.
- Git, history-rewrite, destructive, and external effects: None; preserve HEAD/index and all pre-existing bytes. No stage, commit, push, deletion, publication or cross-project mutation.
- Out of scope: Rust/API/shader implementation; R-028 or another file; active adoption; source import; runtime GI/dynamic reflections; UI/input/camera behavior.

## Halcyon Pre-Flight

- Governing SPEC/read status: HAL-002/002A supreme; HAL-355/354/357/263A/266A/295A and HAL-309 at their declared status; R-023/024/025/027 inactive dependencies.
- Widget policy or N/A: N/A, specialized rendering design only.
- Interaction added/policy: None; no keyboard, Capture, gesture or navigation work.
- Applicable FORBID rules: Sole ownership, semantic/read-phase separation, immutable source and qualified IDs; no raw GPU/partial front/2D contamination; no Image alpha repair or second publisher/preparation.
- Existing framework mechanism inspected: HAL-355 world/resource/target owners; R-024 final world normal and groups0–2; R-025 proposed group3 b0–b5; actual raster, viewport, context, appearance and CustomRenderPass seams.
- Reinvention risk: No runtime transform, new resource owner, global scene, alternate renderer, transport worker or extra phase. Producer outputs qualify to existing world directions.
- Demo/example compliance: N/A; none changed.

## 1. Purpose, profile and authority gate

**PbrEnvironmentIndirectV1** selects two independent optional contributions: **E**, source-certified distant image-based lighting, and **G**, source-baked static two-bounce DIFFUSE scene interreflection. Neither implies the other, a direct lamp, skybox, ambient coefficient, physical measurement or dynamic GI.

Let D mean at least one separately admitted R-025 direct source. D/E/G have exactly eight finite presence combinations (§8). E=G=0 has no R-026 resource, bind, pass, allocation, scan, job or idle cost, whether D is present or not. Retiring previously live resources remains necessary lifetime work.

This is an S1 binding companion to PbrMetallicRoughnessRealtimeF32V1, PbrDirectLightShadowV1 and HdrSceneColorToneMapV1 within [HAL-370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md)'s exact opt-in scope and HAL-355 §17.1's acknowledgment. HAL-370 §1 records the express delegated technical adoption; no S2 engineering permission or S3/S4 proof follows. No existing baseline appearance or 2D consumer is migrated.

Distant IBL does not evaluate scene occlusion or interreflection. G requires separately proved source geometry/material/visibility transport. A plausible sampled cube or six numbers establishes neither.

## 2. Authority, actual baseline and owners

| Boundary | Current owner and evidence | R-026 S1 role |
|---|---|---|
| Semantics, physical calibration, scene/light meaning | Application/producer; HAL-355 §§3.3–4 | Consume attributable immutable facts; never infer them from rendering. |
| Widget/tree identity, phases and sovereignty | HAL-002/002A | No new UI owner, peer reference, semantic GPU callback or phase. |
| Flat geometry/model/view and typed leases | HAL-355 §§3.5–3.8,7–8 | Extend complete qualification, not a scene hierarchy. |
| Material/evaluator | R-024 §§5–8, S1 binding under HAL-370 | Preserve roles, r_eff, local-frame-first normal and BRDF; add only selected indirect approximation. |
| Direct sources/shadows | R-025 §§3–10, S1 binding under HAL-370 | Preserve b0–b5, calibrated source and shadow truth. No E/G-created direct source. |
| Target, GPU execution and publication | HAL-266A, HAL-295A, HAL-355 §§10–13 | Existing per-target participant, Device/Queue and one final Image publisher. |
| HDR bound/coverage/final transfer | R-027 §§4–9, S1 binding under HAL-370; HAL-263A binding | Supply complete bounded scene radiance, then unchanged tone-before-average and linear-FP16 final. |
| Status and proof | HAL-309 §2 | Draft, source support, compiler/GPU proof and JC quality acceptance remain distinct. |

Live source at the inspected baseline has legacy groups0–2 only: `raster.rs:387–456`. Its ObjectBlock visibility is VERTEX only; R-024's PBR fragment visibility is S1 adopted but not implemented. `appearance.rs:1811–1860` defines View80 and Object112 without E/G orientation. `content.rs:59–62,80–131` exposes only Rgba8Unorm/Rgba8UnormSrgb 2D texture content, not these float cube assets. `viewport.rs:465–525` owns existing target/depth/MSAA resources; `custom_render_pass.rs:611–634` fixes the legacy final texture to gpu.format. None establishes environment, static GI, new float asset leases or R-027 implementation.

Halcyon owns future mechanical validation/shading/resource lifetime. The independent producer owns original asset decoding, orientation, semantic units, preintegration, source patches and transport. No renderer callback discovers or repairs that source.

## 3. Complete source and the single world sampling frame

### 3.1 Immutable source qualification

A complete association SHALL identify source owner/identity/revision, original HDR radiance asset when E is present, original source basis/handedness, exact request HAL-world identity and checked mapping, producer/integrator revision, derived asset generations, material/model/geometry/region/patch/light/environment dependencies and finite error/bound evidence. Actual meters_per_scene_unit is explicit wherever transport or source calibration uses physical distance; R-025's required calibration stays unchanged.

Linear RGB uses R-027's sRGB/Rec.709 primaries and radiometric channel proxies. It is not UI sRGB storage, a photometric/spectral certificate or an Image material upload. Source values and dependencies are immutable; equal qualified identity means equal content. External source mutation requires a truthful successor, never an unchanged revision label.

Missing, stale, foreign-context, wrong-frame, unsupported, nonfinite, unbounded and source-inconsistent inputs retain distinct attributable admission causes (§10). A filename, environment label, preview, texture byte array or claimed bake cannot supply missing provenance.

### 3.2 Exactly one E/G sampling frame

The complete request's explicitly qualified right-handed HAL-355 WORLD frame is the ONLY E/G sampling frame. R-024 supplies final checked unit N_world and V_world in that frame. Signed X/Y/Z directions mean exactly its axes, not an OS/camera/room/gravity singleton or an unverified consumer physical frame.

Render-origin subtraction translates positions to the qualified render-relative f32 domain; it does not rotate unit directions. Camera rotation changes the view/query, not the meaning of E texels or G lobes. No view/model matrix is repurposed as an independent bake orientation.

Every E/G request SHALL prove its relationship to that exact world reference before immutable candidate publication. No shader-side source-space transform, orientation flag, matrix/quaternion, reserved-slot payload or new uniform is admitted.

### 3.3 Canonical G6 source admission

The six G irradiance values SHALL already represent WORLD +X,-X,+Y,-Y,+Z,-Z before publication. The producer retains original source basis/handedness, checked source-to-world mapping, source/producer revision and finite numerical/provenance proof.

For a foreign-frame bake, the producer SHALL compute these six world-direction values from the UNDERLYING QUALIFIED TWO-BOUNCE STATIC TRANSPORT FIELD. Six foreign lobes alone are generally not closed under arbitrary rotation. Copying, swapping axes, mixing coefficients or relabeling those values does not by itself certify the world result. Even an exact signed-axis permutation needs source/mapping/transport qualification; there is no automatic engine conversion.

If the underlying qualified transport/material/visibility data needed for recomputation are unavailable, a foreign-frame six-lobe-only source is unsupported/unresolved for G. It cannot become G-absent or an ambient fallback. Already canonical, current G6 retains its real values without pointless recomputation.

### 3.4 Canonical E source admission

The independent producer SHALL prove the original HDR source orientation/handedness and its mapping to the exact HAL-world directions, including any explicitly checked handedness correction. It performs necessary directional resampling and the selected preintegration BEFORE publication.

Derived diffuse and eight specular cubes SHALL correspond to the actual qualified cube sampler's world-direction/face/texel convention. The BRDF LUT is orientation-invariant angular/roughness data, is not rotated, and retains matched profile/producer/source identity. Cube handedness, face ordering and texel orientation require actual producer/backend proof.

The shader samples diffuse at N_world and specular at R_world=reflect(-V_world,N_world) directly. Unknown original orientation, absent transform proof or missing prefilter evidence refuses E. No view-matrix rotation, camera-following environment, skybox or guessed default is permitted.

### 3.5 Frame/currentness cuts

World-axis interpretation, original mappings and canonical derived outputs belong to complete immutable source/structural provenance. Changing that interpretation invalidates both E and G. A view-only change does not reorient/rebake world-fixed E/G assets, although requested pixels/view identity change normally.

Relevant geometry, model placement, material, lights, environment, visibility or receiver-patch changes invalidate affected G until new producer evidence arrives. Furniture moving does not rotate the distant E assets. Their reuse still requires exact current source/world identity and leases. Target compatibility and last-good rules remain separate from source-frame validity.

## 4. E assets, sampling and producer proof

All three asset roles are required together for E. They are independent typed resources with complete context/generation/lease/source association, not optional hints, a binding array or implicit public GPU objects.

| E role | Exact storage / dimensions | Mips | Logical bytes |
|---|---|---:|---:|
| Diffuse cosine-hemisphere irradiance | Rgba16Float cube,6 faces of32×32 | 1 | 6×32×32×8 =49,152 |
| GGX preconvolved specular radiance | Rgba16Float cube-array,8 cubes,6 faces each128×128 | 1 per face | 8×6×128×128×8 =6,291,456 |
| Split-sum BRDF LUT A,B | Rg16Float 2D,128×128 | 1 | 128×128×4 =65,536 |
| Complete E generation | All three roles | No chain | **6,406,144** |

These payloads exclude views, sampler, leases, staging, caches, alignment, retained prior generations and in-flight use. Extra RGBA channels do not become opacity, GI, orientation or hidden coefficients; only declared RGB irradiance/radiance and LUT RG are interpreted.

One explicit clamp-to-edge filtering sampler uses min/mag linear, one mip and no mip transition or derivative-chosen LOD. The eight roughness cubes are distinct cube-array layers, NOT eight mip levels or implicit R-028 admission. No anisotropic/general mip quality is inferred.

The diffuse cube contains source-preintegrated cosine-hemisphere irradiance, not raw sky pixels. Specular cube k encodes the declared GGX prefilter approximation at:

```text
r_k = 0.045 + (1 - 0.045)*k/7,     integer k=0..7
```

The LUT stores A,B for R-024's selected Schlick/GGX/separable-Smith split-sum reference at (NdotV,r_eff). Producer evidence SHALL identify integration method/version, original HDR source and mapping, finite work/resources, numerical error and supported domain; it SHALL later prove actual prefilter and LUT accuracy against that reference. This is source-owned precomputation, not a Halcyon runtime solver.

Every E RGB irradiance/radiance texel channel is finite and in [0,4096]. LUT A,B are finite and nonnegative, A+B<=1. Admission checks the actual supplied representation, not merely an unquantized producer intention. Invalid present data refuses; it is never clamped, replaced or relabeled absent.

The stated filter/format/cube-array/sampler behavior must be executed on pinned wgpu25/Naga25 and actual claimed Devices/backends. Format names, enums, shader source or this table prove no such support. No public raw WGSL or implicit storage conversion is introduced.

## 5. G static transport and six-world-lobe projection

G selects source-owned static two-bounce DIFFUSE interreflection. Its source has qualified geometry/occlusion, materials, lights/environment, calibrated physical/oriented data, receiver patches, finite transport/integration work and an attributable producer revision.

Let T be the source-qualified bounded diffuse transport and L0 the explicitly NON-BOUNCED incident source:

```text
L1 = T(L0)
L2 = T(L1)
G transport content = L1 + L2
```

L0 is not added as G. Separately evaluated R-025 direct or E first-hit illumination must not be counted again. G-only may reference explicitly qualified bake-source illumination without enabling that runtime direct/E component; source facts are still attributable/current, never an implicit runtime lamp.

The producer must later demonstrate actual geometry/material/visibility inputs, patch partition/mapping, two-step solution validation, finite integration/resource bounds and resulting irradiance. No executed GI capability follows from choosing T or storing baked values. The engine discovers no room, region, patch, probe or solver schedule.

Each consuming draw receives six source-qualified WORLD-direction RGB irradiance lobes, ordered +X,-X,+Y,-Y,+Z,-Z, each channel finite in [0,4096] W/m² proxy. For final unit N_world=(nx,ny,nz):

```text
G_irr(N_world) =
    nx*nx * G_world(sign(nx)*X)
  + ny*ny * G_world(sign(ny)*Y)
  + nz*nz * G_world(sign(nz)*Z)
```

A zero component contributes exactly zero; selecting either signed lobe for that zero-weight term does not change the result. The nonnegative weights sum to1 for a unit N_world. This is a convex six-lobe approximation, not exact hemispherical integration or a representation closed under arbitrary basis rotation.

There is no N_G transform. Source owns spatial patch partition and mapping; a large or differing surface requires source-authored packet splits and separately qualified lobe sets. The renderer must not infer patches from geometry, picking, room identity or a screen image.

Affected source geometry/model/material/light/environment/visibility/patch changes make the bake stale. Complete dependency attribution must include relevant offscreen geometry as source data; rendering visibility is not a substitute. No delayed repair, automatic rebake, silent G removal or dynamic bounce-response claim is admitted.

## 6. Exact indirect evaluation

Use R-024's resolved linear base c, metallic m, AO and emission without reinterpretation:

```text
c_diff = (1-m)*c
f0 = (1-m)*(.04,.04,.04) + m*c
r_eff = max(resolved_source_roughness,.045)
nv = dot(N_world,V_world)
Fview = f0 + (1-f0)*(1-nv)^5
```

Only the qualified positive hemisphere nv>0 contributes indirect. N_world/V_world are checked unit directions; R-024's numerical/zero-direction/double-sided rules remain authoritative. Dot-domain qualification follows that selected guarded evaluator, not a hidden source-factor clamp. nv<=0 yields no surface indirect; independent emission remains.

With E present, sample Eenv=diffuse_cube(N_world), and R_world=reflect(-V_world,N_world). The specular roughness coordinate is:

```text
k = 7*(r_eff-.045)/.955
for 0 <= k < 7:
    lo = floor(k)
    hi = lo+1
    u = k-lo
    preconvolved_spec = (1-u)*spec_cube(R_world,lo)
                     + u*spec_cube(R_world,hi)
at r_eff=1:
    preconvolved_spec = spec_cube(R_world,7)
(A,B) = LUT(nv,r_eff)
E_spec = preconvolved_spec * (f0*A + B)
```

Ideal k lies in [0,7]; endpoint7 must not address layer8. Actual f32 endpoint/index/interpolation proof remains OPEN; there is no guessed epsilon, clamped invalid source or out-of-range lookup disguised as success.

The exact selected additive approximation is:

```text
diffuse_indirect = AO*c_diff*(1-Fview)*(Eenv + G_irr)/pi
specular_indirect = E_spec

scene_rgb = admitted_R025_direct
          + R024_emission
          + diffuse_indirect
          + specular_indirect
```

Eenv and E_spec are zero ONLY for explicit E absence; G_irr is zero ONLY for explicit G absence. Invalid present input is never zeroed. R-025 direct is evaluated only under its separately admitted profile; emission is added exactly once.

AO attenuates the specified indirect DIFFUSE term only: never direct, specular IBL or emission. G excludes non-bounced L0. No implicit specular occlusion, scene environment visibility, source emitter, ambient compensation or extra bounce is included.

This is bounded approximate diffuse convolution plus a split-sum REALTIME APPROXIMATION of the regularized material reference, not a change to R-024's BRDF, ideal mirror, exact glTF optical conservation or exact full hemispherical transport. Source prefilter/LUT consistency remains a separate necessary proof.

## 7. Numerical domain and complete radiance certificate

For admitted ideal source data and convex sampling/interpolation, Eenv and G_irr each lie in [0,4096] per channel. c_diff, Fview, AO and f0 lie in [0,1]. Thus:

```text
0 <= diffuse_indirect <= 8192/pi
0 <= f0*A+B <= A+B <= 1
0 <= specular_indirect <= 4096
0 <= indirect_total <= 8192/pi + 4096
                     = approximately 6703.594588 < 6710
```

The G bound follows nx²+ny²+nz²=1. The E specular bound follows convex filtering/layer interpolation and A+B<=1. These are IDEAL inequalities, before actual source/model/FP16/f32/filter error. They are not proof of a producer, shader or device.

R-027 still requires a conservative COMPLETE pre-write certificate for every covered output RGB: finite, nonnegative and <=60000. It includes all actual R-025 direct BRDF/cosine/light/shadow terms, R-024 emission and E/G, exact material/source/world/model/view identity, evaluator domain and numerical margins. Neither the indirect bound nor per-light caps prove the complete sum.

The actual certificate issuer, source-domain validator and f32/FP16/backend margins require future source/parent admission and executed evidence. Missing or unprovable certificate refuses before any candidate write. No 60000 clamp, per-pixel CPU enumeration, synchronous GPU readback error or post-publish failure relabel is selected.

## 8. Fixed ABI and eight presence variants

### 8.1 Preserved and extended bindings

R-024 groups0/1/2 remain View80, Object112 and Material64 with their exact selected bytes/stages. No frame data is hidden in them. R-025 group3 b0–b5 retain their exact Light608/ShadowTransform1984/depth-array/cube-array/comparison-sampler contract.

| Group3 binding | Exact resource / presence |
|---|---|
| b0 | R-025 DirectLightBlock608, only D |
| b1 | R-025 ShadowTransformBlock1984, only D |
| b2 | R-025 depth2D-array7, only D |
| b3 | R-025 comparison sampler, only D |
| b4 | R-025 depthCube-array4, only D |
| b5 | R-025 comparison sampler, only D |
| b6 | IndirectBlock112 read-only uniform, FRAGMENT, only E or G |
| b7 | Filterable texture_cube<f32>, diffuse irradiance, FRAGMENT, only E |
| b8 | Filterable texture_cube_array<f32>, eight specular cubes, FRAGMENT, only E |
| b9 | Filterable texture_2d<f32>, A/B LUT, FRAGMENT, only E |
| b10 | One explicit filtering sampler, FRAGMENT, only E |

| IndirectBlock byte offset | Exact field |
|---:|---|
| 0 | header vec4<u32> = (E_present,G_present,0,0), each presence exactly0 or1 |
| 16 | WORLD +X GI RGB, W=0 |
| 32 | WORLD -X GI RGB, W=0 |
| 48 | WORLD +Y GI RGB, W=0 |
| 64 | WORLD -Y GI RGB, W=0 |
| 80 | WORLD +Z GI RGB, W=0 |
| 96 | WORLD -Z GI RGB, W=0 |
| 112 | End; no unidentified payload |

Each vec4 has size/alignment16; seven slots yield size112/alignment16. With actual uniform-offset alignment a, per-slot dynamic stride is checked round_up(112,a), not assumed112. This is byte-layout arithmetic, not host/Naga/Device validation.

When E-only, all six GI vec4s are exactly zero. When G-only, b7–b10 do not exist: no dummy sampled E resources. Header flags and selected variant must agree. Each draw may have its own qualified b6 data/offset/source association; shared E/direct leases remain with their actual owners. No cross-draw alias of mutable slots or guessed packet identity is permitted.

### 8.2 Exactly eight finite profile/layout cases

The rows below identify distinct compile/pipeline-profile cases even where two rows have the same binding set. No shader references an absent binding, and no runtime branch legitimizes an unbound resource.

| D | E | G | Exact group3 binding set | R-026 charge |
|---:|---:|---:|---|---|
| 0 | 0 | 0 | Group3 absent | None |
| 1 | 0 | 0 | b0–b5 | None; direct owner only |
| 0 | 1 | 0 | b6–b10 | E assets/sampler plus112-byte live slots; GI slots zero |
| 0 | 0 | 1 | b6 |112-byte live GI slots only |
| 0 | 1 | 1 | b6–b10 | E assets/sampler plus112-byte live GI slots |
| 1 | 1 | 0 | b0–b10 | Direct + E; GI slots zero |
| 1 | 0 | 1 | b0–b6 | Direct + G, no E sampled resources |
| 1 | 1 | 1 | b0–b10 | Direct + E + G |

D=1 means at least one actual direct light; zero direct counts omit b0–b5/resources, not independent E/G. R-025's no-direct-resource rule remains intact. E=G=0 incurs no R-026 binding even if D keeps group3 alive. An unused profile does not eagerly create eight shaders/pipelines.

There are at most four bind groups, never a fifth. All-present scene-fragment accounting includes R-024's5 material sampled textures/5 samplers, R-025's2 depth textures/2 comparison samplers and E's3 textures/1 sampler:10 sampled textures and8 samplers total, with up to6 uniform blocks including View/Object/Material/direct/shadow/indirect. Actual stage visibility, minimum sizes, offsets, aggregate limits, live variants and formats must be qualified on the real admitted Device/Naga pipeline; the counts are not support proof.

A cube-array is one typed sampled resource with eight cube layers, not an unadmitted binding-array feature. The new controlled family cannot expose application WGSL globals, raw bind layouts or private GPU handles. Fixed source identity and variants remain structurally qualified; factor/source changes require the complete successor/lease discipline.

## 9. Finite resources, transaction and retirement

### 9.1 Complete accounting

| Cost | Required logical charge before actual overhead |
|---|---|
| E generation |6,406,144 bytes plus actual sampler/views/leases/staging/cache/in-flight generations |
| G or E indirect uniform |112 payload bytes per live draw slot, actual aligned stride and all live/in-flight slots |
| D shadow pool when active | R-025 exactly54,525,952 depth bytes plus608+1984 uniform payloads, actual views/alignment/work |
| HDR/depth/final output | R-027 (12*N+16)*W*H minimum attachment bytes, N=1 or4, plus its complete actual costs |
| Pipelines/work | Bounded actual variants, draw/slot counts, source uploads, preparation and command residency |

No guessed aggregate memory budget or timing claim follows from these identities. Every live resource, old front, pending candidate, staging/cache allocation and in-flight lease is charged under finite effective HAL-355 owner limits, with checked arithmetic before allocation/work.

E evaluation requires at most one diffuse cube lookup, two neighboring specular cube lookups and one LUT lookup per contributing sample; endpoint7 needs no out-of-range second layer. G has fixed three squared component weights/signed-lobe selections. No online integration, data-dependent ray loop or runtime bake is admitted. This bounds selected work shape, not GPU time or total scene cost.

Exhaustion refuses the complete candidate without fewer roughness cubes, reduced shadows, reflection/GI removal, resolution downgrade or hidden material substitution. Sharing identical current immutable E leases can avoid duplicate allocation but cannot erase residency or generation obligations.

### 9.2 Existing ordered transaction

The immutable complete association includes D/E/G presence; source/world/mapping/producer/certificate identities; all source geometry/model/material/light/environment/patch data; E assets/sampler and per-draw GI generations/slots; exact view/current target/context/mount lifetimes; all R-024/025/027 profile, resource and color settings.

Lawful source Update publishes complete owner values. Existing HAL-266A preparation receives actual final target metrics and validates/reserves ALL dependent source/shadow/scene/tone/final resources before any candidate writes:

```text
qualified source publication
 -> existing per-target preparation and complete validation/reservation
 -> required changed R-025 shadow writes, if D
 -> scene material/direct/E/G evaluation into R-027 scene color
 -> R-027 per-covered-sample tone BEFORE averaging
 -> R-027 linear Rgba16Float final back
 -> existing same Queue enqueue/submit
 -> same CustomRenderPass swap/Image registration
 -> atomic complete committed association
```

R-026 creates no preparation phase, integration pass, extra clock/worker, Device/Queue, shadow/GI publisher or 2D compositor pass. The final Image already satisfies HAL-263A sampled premultiplied-linear ingress; no E/G texture uses Image normalization or becomes a public front.

Failed candidate admission preserves only a compatible original last-good with its ORIGINAL complete E/G/direct/material/light/model/view/target/source tuple. Request failure remains separately attributable. It cannot combine fresh geometry/light with stale G or relabel an old front under a fresh world/source revision. Incompatible target/lifetime withdraws publication.

Inactive/zero/retired targets perform no extension allocation/growth/upload/pass/job, and unselected/no-context/no-view paths remain structurally absent. Existing unregister-before-drop and Queue-safe lease retirement still run when necessary. No generation recycling while leased, private device recovery or frame-index resurrection.

## 10. Currentness, source refusals and absence truth

| Changed or failed fact | Exact outcome |
|---|---|
| Original/world mapping unknown or changes | Reject unknown mapping; a changed world-axis interpretation invalidates both E and G derived outputs. |
| Foreign G6 only, without transport field/proof | Unsupported/unresolved G admission; no shader normal rotation, coefficient repair or false G omission. |
| E original source/prefilter/LUT mismatch | Source-inconsistent E refusal even if every numeric texel lies in bounds. |
| Model/geometry/material/light/environment/visibility/patch affects G | Stale affected bake refuses until freshly qualified producer output; no automatic online rebake. |
| View-only change | New ordinary view/output dependency; reuse unchanged world-fixed E/G only with exact source/lease proof. |
| Furniture move with unchanged distant E source | G invalidates where affected; E world alignment does not rotate with the furniture. |
| Target extent/activity/format/sample/context/mount loss | Existing target compatibility and publication withdrawal/retirement; no prior-frame target guess. |
| Invalid texel/lobe/LUT/factor, nonfinite or unbounded input | Attributable invalid-domain/source-bound refusal before candidate writes, never clamp/default. |
| Missing/stale/cross-context generation or retired source | Distinct identity/lifetime failure; neither absent nor successful last-good relabel. |
| Finite resource/capability failure | Distinct exhausted-budget versus unsupported-device/format/layout outcome; original compatible last-good only. |

These are semantic cause categories, not invented public Rust enum variants or methods. Future parent/source admission must preserve them through actual request/preparation/front outcomes. A raw provider revision or image does not certify physical-world truth.

## 11. Worked documentary cases and frame oracles

The arithmetic cases below are exact ideal evaluation fixtures CONDITIONAL on independently qualified source sample/bake inputs. They do not claim that an arbitrary tuple of sample values proves a real prefilter/LUT/transport producer. Actual f32/FP16 and source accuracy remain separate gates.

1. **E-only arithmetic.** D=0,E=1,G=0, c=.5,m=0,AO=1,nv=1,emission=0. Thus c_diff=.5,f0=Fview=.04. For qualified sampled Eenv=1, preconvolved_spec=2 and LUT(A,B)=(.5,.125), diffuse=.48/pi, specular=2*(.04*.5+.125)=.29, scene=.48/pi+.29 per channel. All G slots are zero; the E generation and b6–b10 are required. The arithmetic fixture does not itself certify those three samples came from one correct producer.

2. **G-only canonical arithmetic.** Same material/view, E=D=0; WORLD G(+X)=1,G(-Y)=2,others0, N_world=V_world=(1,0,0). Weights(1,0,0) give G_irr=1 and scene=.48/pi per channel. Only b6 exists; no E textures/sampler or hidden direct source.

3. **E+G, no double count.** Case1 plus source-qualified WORLD G(+X)=3 and the same N_world. Diffuse=.48*(1+3)/pi=1.92/pi; specular remains .29. Adding independent R-024 emission=.125 gives scene=1.92/pi+.29+.125, once. AO=.5 halves ONLY the1.92/pi term. L0 is not separately added as G.

4. **Exact original rotated-frame counterexample and disposition.** Case2's identical b6 bytes interpreted in an alternate G source frame rotated+90 degrees about WORLD Z would give N_G=(0,-1,0), G_irr=2 and diffuse=.96/pi instead of .48/pi. Under THIS contract that alternate source is NOT ADMISSIBLE as those identical world coefficients. If qualified original transport can be reevaluated, producer supplies WORLD G(+X)=2 and appropriately different world b6 data; otherwise G refuses with its source/frame cause. No N_G shader path or hidden uniform exists.

5. **Non-axis-aligned six-lobe nonclosure.** As an algebraic representation witness, take foreign paired X lobes1 and all Y/Z lobes0, so its approximation is f(n)=nx². A45-degree world-Z basis rotation gives f_world(N)=(Nx+Ny)²/2. Its world signed-axis values are X/Y=.5,Z=0. The world-six-lobe fit from those samples is .5*Nx²+.5*Ny², which equals .5 at N=(1,1,0)/sqrt(2), while the rotated original expression equals1. The missing Nx*Ny cross term cannot be supplied by an axis permutation or any six world-axis coefficients matching all axis values. This is a representation proof, not an executed physical bake. Qualified producer recomputation from transport yields a NEW world-axis approximation with its own declared error; it does not preserve the old approximate field exactly by relabeling.

6. **Rotated E source and rotated camera are different.** For a qualified original HDR source whose bright direction maps to WORLD+X, producer-derived cubes must place that feature at actual sampler+X regardless of the original file/cube basis. Changed original orientation/mapping requires differently derived assets/identity; copying old cube faces is not proof. A camera-only yaw retains those world-fixed assets. At the same world point, unchanged N_world/V_world imply the same query; changed view/sample locations or camera position may change R_world and sampled brightness without rotating the environment. Future image/resource evidence must distinguish those cases.

7. **Object/light invalidation.** A current E+G front uses bake geometry/model/light revisions g1,m1,l1. A relevant object move m2 or light change l2 invalidates affected G1. The request cannot write fresh direct/models with G1; it must receive qualified G2 or refuse. Unchanged distant E remains world-fixed/current if its own dependencies are unchanged. An independently changed world-axis interpretation invalidates BOTH E1 and G1.

8. **Zero use.** D=E=G=0 has no group3 and no R-026 work. D=1,E=G=0 has only R-025 b0–b5 and its54,525,952-byte shadow payload; R-026 adds zero resources/work. Emission, if present under R-024, is not E/G and gains no bounce claim.

9. **Capacity and last-good.** A genuinely new E generation needs6,406,144 logical asset bytes plus all real overhead; if the effective remaining reservation cannot cover it, refuse before scene writes. No seven-layer substitute or dropped G. A compatible old front retains its old E/G/light/model/view/source association and reported request failure; an incompatible target withdraws it. Pending/current old generations remain charged until actual owner retirement.

10. **Sampled IBL is not source proof.** A finite cube and A/B table may pass simple numeric-domain checks and look plausible yet lack original orientation, matching prefilter/LUT integration or producer revision. They refuse E source admission. Likewise unknown GI source frame or six values without transport provenance cannot claim two-bounce GI. A compiled shader, source file, renderer image or native appearance proves neither physical measurement nor transport correctness.

## 12. Invariant/prohibition pairs

**INV-R026-OWNER.** Producer owns content/meaning/preintegration/bake; existing Halcyon 3D owner alone validates/mechanically shades and publishes.  
**FORBID-R026-OWNER.** No hidden scene/light/GI semantic owner, renderer source callback, physical inference, peer widget or second runtime.

**INV-R026-PRESENCE.** D/E/G are independently explicit, matching one of the eight qualified static cases; present invalid source retains its failure.  
**FORBID-R026-PRESENCE.** No implicit ambient/environment/direct light, E-as-GI, invalid-to-absent rewrite or unbound-resource shader path.

**INV-R026-WORLD.** All E queries and G6 weights use final qualified HAL N_world/V_world and source-certified world-aligned outputs.  
**FORBID-R026-WORLD.** No N_G, runtime source-frame rotation, hidden quaternion/matrix, reserved-field data, camera-following environment or automatic lobe rotation.

**INV-R026-SOURCE.** Complete immutable original/source-world/producer/derived identity and finite domain/error evidence qualify each used asset/lobe generation.  
**FORBID-R026-SOURCE.** No texture/label/thumbnail/token-only proof, guessed orientation, missing-transform acceptance or foreign six-lobe-only conversion.

**INV-R026-E.** E contains exactly the three matched single-mip roles, eight prescribed roughness cubes and one explicit sampler, within exact source bounds.  
**FORBID-R026-E.** No missing-role default, arbitrary ambient cube, ninth layer, implicit mip/LOD, source clamp or fake producer qualification.

**INV-R026-G.** G is qualified static T(L0)+T(T(L0)) diffuse transport projected into six world irradiance lobes with source-owned patch mapping.  
**FORBID-R026-G.** No L0 double count, online solver, dynamic GI claim, inferred patch, unqualified stale bake or mere foreign-lobe relabel.

**INV-R026-RESPONSE.** R-024 material/r_eff and the exact §6 approximation govern; AO affects indirect diffuse only, emission once.  
**FORBID-R026-RESPONSE.** No BRDF swap, hidden emitter/bounce, specular/direct/emission AO, noncontributing undefined normalization or exact-optics claim.

**INV-R026-ABI.** Fixed groups0–2, R-025 b0–b5 and new b6–b10 preserve exact bytes/conditional presence and source-qualified per-draw slots.  
**FORBID-R026-ABI.** No fifth group, ABI mutation, unused-slot frame payload, dummy G-only E texture, binding-array assumption or public arbitrary WGSL.

**INV-R026-BOUND.** Ideal indirect bound is separate from complete qualified R-027<=60000 source/evaluator/radiance admission before writes.  
**FORBID-R026-BOUND.** No coefficient-only completion, unbounded combination, guessed epsilon, hidden clamp, readback admission or post-publish failure rewrite.

**INV-R026-CURRENT.** World/transport/patch dependencies and E/G/source revisions are checked for the entire requested/prepared/committed association.  
**FORBID-R026-CURRENT.** No fresh light/model with stale G, view-driven world relabel, implicit rebake/removal or old-source resurrection.

**INV-R026-RESOURCES.** All exact payloads, alignment, slots/variants, old/front/cache/staging/in-flight generations and bounded work are finitely admitted.  
**FORBID-R026-RESOURCES.** No guessed capacity, unbounded retention, hidden residency or quality downgrade on exhaustion.

**INV-R026-FRONT.** Existing target preparation, same-Queue writes and single offscreen/Image publication commit one complete tuple; compatible last-good stays original.  
**FORBID-R026-FRONT.** No independent GI/shadow front, partial/mixed commit, incompatible front sampling, semantic preparation mutation or second Image tone/alpha repair.

**INV-R026-ABSENCE.** Unselected E/G and inactive/no-context/no-view paths add no R-026 resource/work/idle demand; necessary retirement follows existing owners.  
**FORBID-R026-ABSENCE.** No empty-profile bindings, eager eight-variant churn, 2D branch, private clock/job or invisible maintenance bake.

**INV-R026-PROOF.** Documentary ideal math/layout, producer/source proof, f32/Naga/Device/GPU/target execution and JC native quality are separate recorded stages.  
**FORBID-R026-PROOF.** No compiled-shader, bounded coefficient, sampled image, AABB result or inactive Draft substituted for executed transport/backend/native acceptance.

## 13. Exact parent/dependency compatibility matrix

HAL-370 §8/HAL-355 §17.1 supply the exact S1 opt-in extensions; existing HAL-354/357/266A/295A/263A/309 boundaries are unchanged. S2 authorization and actual producer/CPU/f32/compiler/Device/native evidence remain separate.

| Owner / exact affected boundary | What stays authoritative | S1 contract and remaining implementation proof |
|---|---|---|
| HAL-002/002A | Sole ownership/IDs, immutable values, phases and sovereignty | No constitutional exception proposed; qualify source/technical decisions without semantic GPU authority. |
| HAL-355 §§3.3–4,6.1–6.3 | Checked world/local/render-relative and application/engine roles | Express canonical E/G source-world association and precision/producer qualification in the opt-in profile, not a new global coordinate owner. |
| HAL-355 §§7–8.4,11–13 | Typed nonwrapping leases, complete flat source/model, front lifecycle and finite budgets | S1 adopts new float cube/cube-array/LUT content/resource admission, per-draw G source association and complete E/G generations/limits; current Rgba8 2D Texture3D is insufficient. |
| HAL-355 §§9.1–9.5 | Exact legacy ABI, controlled shader and pipeline identity | S1 adopts the distinct R-024/R-025/R-026 controlled PBR family and exact group3 variants/112-byte block; actual format/stage/limit proof remains OPEN. Legacy three groups/wrapper remain unchanged. |
| HAL-355 §§10.1–10.5,11,13,17 | Current target/submission/front/absence; lighting/environment/HDR excluded from baseline | Exact scoped optional feature/resource/target admission; R-027 internal final-format/HDR changes remain its separate dependency. No blanket non-goal relaxation. |
| HAL-354 §4.1, mounted publication and §4.4 | One lawful Update/source publication before mechanical preparation | Coordinate any extended complete public tuple/currentness transfer explicitly; existing G1 is not silently widened or changed. |
| HAL-357 §§2,4,6–8 | Optional checked lowering to one complete flat HAL world | If composition participates, preserve complete world/source/patch association at flat boundary; no semantic domain interpretation or renderer traversal. |
| HAL-266A §§3–6 | Exact immutable current target, indexed GPU-local preparation, Image-only Paint | Qualify E/G resources/work with target/current lifetime in the existing boundary; no extra preparation, Timeline or source mutation. |
| HAL-295A §§3.2–3.4 and INV-008 | Private same-Queue writer, swap/registration and lifetime | Keep one complete publisher; R-027's separate profile-specific final-format admission and real ordered resource retirement proof. |
| HAL-263A §3.2, INV-002/004/005/006/008 | Final sampled premultiplied-linear Image, closed ingress, one alpha modulation | E/G/material textures do not enter Image normalization; R-027 final association/filter proof, no new ingress/repair selected here. |
| HAL-309 §2 and status/proof clauses | Declared status, source truth and executed evidence remain distinct | Record delegated S1 adoption separately from ungranted S2 engineering and unexecuted S3/S4 proof; no code, runtime or visual claim from adoption. |
| R-023 §§7–13, S1 binding under HAL-370 | Distinct IBL/GI, finite leases/targets, complete light/material/output seams | This file supplies selected E/G design; S2 authorization and material/direct/output/producer/native proof gates remain OPEN. |
| R-024 §§5–8.4/10, S1 binding under HAL-370 | Roles, regularized r_eff, guarded world normal, Material64/PBR stream | Prove actual selected material/source/f32 path; no new tangent/normal transfer or material default. |
| R-025 §§3,8–10, S1 binding under HAL-370 | Direct/shadow source+meters, exact b0–b5,54,525,952-byte pool | Coordinate independent direct-absent/E/G-present group3 cases; R-025 data/algorithm/budgets untouched. |
| R-027 §§4–9/11–13, S1 binding under HAL-370 | Complete<=60000 certificate, per-sample tone/coverage and fixed linear-FP16 final | Real complete source/integration certificate, actual f32/format/filter/front proof; parent scope is S1 adopted; no output algorithm change. |
| [R-028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md), separate S1 binding feature under HAL-370 | Distinct PbrTextureMipSamplingF32V2 for role-correct 2D material mip chains, complete immutable chain generations and six MIN modes; unchanged R-024 V1 material evaluator/one-mip normal contract | Parent/V2 S1 adoption is performed; S2 authorization and producer/sampled/backend/native/performance proof remain OPEN. R-026 E's eight independent one-mip cubes, diffuse cube and LUT are explicitly excluded; their existence does not establish general mip support. |

HAL-370 §8 defines the exact opt-in parent deltas acknowledged by HAL-355 §17.1; no feature name or ordinary review receipt supplies authority by itself. Current S1 adoption changes documentary authority only, not source, tests, manifest or consumer behavior.

## 14. Future proof oracles and remaining OPEN gates

All executions below are FUTURE obligations, not tests added/run here.

| Proof stage / intended supported path | Required falsifiable observation | Nearest existing future owner |
|---|---|---|
| Source/world admission | §11's exact90-degree equal-bytes rejection; producer-derived different world data;45-degree nonclosure; E reorientation versus camera-only view; wrong/unknown handedness; canonical input reuse without pointless recompute | three_d_context.rs, three_d_geometry.rs, three_d_snapshot.rs; independent producer proof |
| E producer | Real original HDR plus orientation/mapping and matched32/128/128 assets; identified integrator/version and error; exact eight r_k, accurate GGX/Smith/Schlick LUT, actual cube convention and bound checks | Independent source producer; three_d_context.rs/three_d_appearance.rs for mechanical association |
| Static G producer | Real finite geometry/material/visibility/L0; two explicit transport applications, L0 exclusion, patch partition/world projection, source changes invalidating affected bakes and qualified successor/error | Independent producer; context/geometry/snapshot owner integration |
| Ideal arithmetic | E/G/both/no-use cases; G convex weights/signs; r_eff .045/1 and layer7 endpoint; Fview/AO/emission distinctions; A+B bound and the exact8192/pi+4096 indirect bound | three_d_appearance.rs, three_d_raster.rs |
| Host/ABI/layout | IndirectBlock112 offsets/zeros, header/variant agreement, all8 layouts, per-draw source slots/dynamic alignment, no unbound E/direct resources and aggregate material/direct/E limits | three_d_context.rs, three_d_appearance.rs, three_d_raster.rs |
| Actual f32/FP16 | Material/unit/dot/reflect guards, k/index/endpoints, filter and lobe normalization margins, decoded actual assets/LUT, full<=60000 certificate with numeric errors; no guessed epsilon | three_d_appearance.rs, three_d_backend.rs and source integration owner |
| Pinned compiler/Device | Naga25.0.1 controlled modules/layouts and wgpu25.0.2 actual float-cube/cube-array/Rg16Float filtering and binding/offset/stage limits for all claimed variants; R-027 N1/N4 target/output | three_d_appearance.rs, three_d_raster.rs, three_d_backend.rs |
| Transaction/retirement | Complete source/frame/material/direct/E/G versus target generations; failed reservation before writes, original last-good, zero/inactive/remount withdrawal, Queue-safe old slots/assets and zero-use | three_d_context.rs, three_d_viewport.rs, custom_render_preparation.rs |
| Native source-to-output | Canonical App::run real positive target, actual textured material/E assets, separately proved G scene, view/model/light change, source->shadow/scene->tone->FP16 Image->presentation; baseline2D/LDR unchanged | tests/app_frame_realization_native.rs, backend/viewport; JC visual review |

| Gate | Current disposition / required owner |
|---|---|
| BLOCK-R026-PARENT | Exact coordinated parent/source/material/direct/HDR contract S1 ADOPTED under HAL-370; OPEN — separate S2 authorization, concrete integration and actual S3 owner/target/publication compatibility proof. |
| BLOCK-R026-SOURCE-WORLD | Architecture SELECTED, producer proof OPEN — original-to-exact-world mapping, actual cube convention and transport-derived world G6, no six-lobe-only rotation fallback. |
| BLOCK-R026-E-PRODUCER | OPEN — identified real source/preintegration/LUT generation, bounded work/error and supported-domain evidence. |
| BLOCK-R026-G-PRODUCER | OPEN — executed finite two-bounce transport, patch/source mapping, static invalidation and world projection proof; no dynamic GI claim. |
| BLOCK-R026-NUMERIC-RADIANCE | OPEN — R-024 actual f32/source domain, R-025 full direct contribution, E/G/filter/FP16 margins and complete R-027<=60000 pre-write certificate. |
| BLOCK-R026-NAGA-DEVICE | OPEN — pinned controlled shader/ABI, all eight variants, actual Device formats/cube-array/filter/limits and R-027 sample/output proof. |
| BLOCK-R026-RESOURCE-FRONT | OPEN — real finite leases/slots/variants/cache/in-flight budgets, exact target/currentness/transaction and original last-good/retirement/no-use evidence. |
| BLOCK-R026-CONSUMER-NATIVE | OPEN — complete real consumer inputs, native stage/target output and JC perceptual acceptance; source/image/doc arithmetic is insufficient. |

The world-frame representation gap is architecturally resolved; it is not proof the producer, shader, device or real consumer can satisfy the chosen profile. Any concrete counterexample to the fixed mathematics or source/ABI must return to the architect without a silent repair.

## 15. Documentary source evidence and completion boundary

| Evidence | Exact inspected relation |
|---|---|
| HAL-355 §§3.3–4,7–13,17 | Existing coordinate/owner/resource/ABI/target/non-goal boundaries; source facts do not activate E/G. |
| R-024 §7.2:195–204; §8.4:291–345 | Final world normal, View80/Object112/Material64, controlled family and future group3 boundary; no independent bake transform. |
| R-025 §3:71–92; §9:289–333 | Explicit metric/world light source and exact direct-only group3 resources; no runtime E/G orientation. |
| R-027 §4:94–118; §8:295–315 | Complete source radiance and target association, fixed FP16 output/resource/absence obligations. |
| [appearance.rs](../../src/three_d/appearance.rs):1811–1860 | Actual View80/Object112 contents; no environment/GI frame slot. |
| [raster.rs](../../src/three_d/raster.rs):387–456 | Current legacy groups0–2 and vertex-only ObjectBlock visibility, not the S1-adopted four-group family. |
| [content.rs](../../src/three_d/content.rs):59–62,80–131; [context.rs](../../src/three_d/context.rs):896–980 | Current2D Rgba8 content and context/kind/slot/generation ownership; new float assets need admission. |
| [viewport.rs](../../src/three_d/viewport.rs):465–525,1310–1374 | Existing private target/depth and encode/same-owner swap/complete commit. |
| [custom_render_pass.rs](../../src/renderer/custom_render_pass.rs):611–634 | Current legacy gpu.format final allocation; R-027's different profile final remains unimplemented. |

Documentary verification checks whole-file readback, links,14 matched INV/FORBID pairs,8 presence cases, exact bytes/offsets/equations, original rotated-frame witness and all new world-source rules. It is not Cargo, shader compilation, GPU execution, a GI bake, native presentation or measured performance.

Explicitly excluded: runtime GI bake/solver, path/ray tracing, probe discovery/update, dynamic local-geometry reflections, import/decoder, normal mip generation, extra glTF coordinates, skybox, scene-geometry environment occlusion/shadows, general translucency, multiple scattering, changed direct model and dynamic-GI quality. General mip/LOD/filter quality stays with the separate S1-adopted R-028 V2 contract.

The original drafting unit was limited to this document and did not authorize changes to related specifications or implementation. Current disposition: **Adopted / Binding [S1] under HAL-370; NOT IMPLEMENTED / S2 NOT AUTHORIZED / S3–S4 OPEN / NOT IMPLEMENTATION-READY**. JC retains final veto.
