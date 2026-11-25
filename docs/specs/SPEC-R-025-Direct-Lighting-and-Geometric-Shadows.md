# SPEC-R-025: Direct Lighting and Geometric Shadows

## Historical drafting scope

This section and the following architecture review preserve the historical 2026-10-31 draft-authoring task. Their inactive/no-adoption and single-file scope statements are task-time receipts, not current authority; the S1 header and §1 govern present status.

- Objective: Specify the complete architect-selected inactive PbrDirectLightShadowV1 direct-light/geometric-shadow profile, including the supplied six-chart Decision7 topology resolution, while preserving all existing substrate/alpha/target owners.
- Source qualification: HAL-002/002A/309, R-023/R-024/R-027, affected HAL-355/263A/295A/266A/354/357 clauses and live raster/appearance/context/viewport/pass source were inspected.
- Governing authority and status: HAL-002 Foundational supreme, HAL-002A Permanent; HAL-355/263A/295A/266A/354/357 binding and HAL-309 adopted. R-023/R-024/R-027 are inactive proposals. The explicit HAL-R025-DIRECT-LIGHT-SHADOW-ARCHITECTURE-DRAFT-01 mandate authorizes this draft only.
- Authority change: NONE effective; no parent amendment/adoption, implementation permission or runtime capability.
- Target layer and existing state verified: Halcyon0.227.18-beta, master312e25c7, wgpu25.0.2/Naga25.0.1. Initial state: tracked modified R-023, untracked R-024/R-027, zero staged; R-025 absent. Existing groups0–2, target/pass and Queue are production source; group3/light/shadow/HDR profiles are not implemented.
- Owned mechanisms and files: CREATE ONLY docs/specs/SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md. Preserve every other file, parent, sibling draft, source/test/asset, manifest, HEAD/index and unrelated change.
- Named invariants: §11 pairs for lights/units, BRDF, shadow/coverage/PCF, ABI, provenance, resources/lifetime, absence and proof; HAL-002 owner/phase/ID, HAL-355 complete leased fronts, HAL-266A target, HAL-295A same-Queue publication, HAL-263A sampled association, R-023 source-qualified shadow dependencies, R-024 response and R-027 finite certificate/output.
- Forbidden: Source/shader/GPU implementation, parent activation, different frame/sampler/lighting semantics, hidden lights/GI/ambient, decorative2D shadow substitution, partial/stale map/color fronts, GPU semantic authority/readback admission, new runtime or self-authorized successor.
- Existing mechanisms inspected and reuse decision: Reuse Context3D generational leases, flat structural/model/view tuple, private per-view participant, indexed preparation, existing Device/Queue and CustomRenderPass/Image owner only after exact future admission. No second device, queue, publisher, graph, Image shader or semantic store.
- External/dependent boundaries: Consumer owns source calibration/content/light meaning. R-024 supplies the proposed material/local-frame evaluator, R-026 independently owns environment/GI, R-027 the complete<=60000 certificate and linear-FP16 final. Neither current source nor this document proves those features.
- Test ownership and placement: Documentary formulas/ABI/source review now. Future existing three_d_geometry/context/appearance/raster/backend/viewport and tests/app_frame_realization_native.rs owners cover §13; no tests/source created or run.
- Exact runnable verification and expected proof: §14 gives Grip ws/headings/full read/hash/state commands. Verify this single inactive artifact, math/ABI, correct sibling dependencies and unchanged other paths. No Cargo, shader, GPU or native execution and no habitual mass hash inventory.
- Output/artifact completeness: One self-contained inactive contract for supplied decisions1–13 and the exact six-chart Decision7 resolution: source/units/BRDF/shadow/ABI/resources/transaction, ideal chart proof, parent obligations and OPEN implementation/backend/native gates. No global-continuity or visual-seam guarantee.
- SemVer/changelog: N/A — document only; no executable/API/release change.
- Out of scope: R-026/R-028 files, R-027 transfer changes, legacy LDR or parent edits, material/source implementation, consumer applications, native trials, camera/input/Capture and successor work.

## Historical architecture review

- Governing SPEC/read status: Supreme HAL-002/002A, binding affected HAL-355/263A/295A/266A/354/357, adopted HAL-309; R-023/R-024/R-027 remain inactive.
- Widget policy: N/A — specialized3D documentary evaluation only.
- Interaction/policy: NONE; no camera, Focus, Capture or input change.
- Applicable FORBIDs: No read-phase semantic mutation, peer IDs/references as authority, second publisher,2D Image/alpha repair, guessed source/budget/shadow or false runtime success.
- Existing mechanism: Explicit retained3D context/request/participant, controlled legacy GPU groups, indexed target preparation, same Queue/front/Image. R-024 reserves future group3 without implementing it.
- Reinvention risk: Proposed group3 belongs only to the distinct opt-in PBR family, not a public shader escape, global lighting system, ECS or duplicate rendering runtime.
- Demo compliance: N/A — no demo changed.

**Author:** JC  
**Date:** 2026-10-31  
**Status:** Adopted / Binding [S1 — PbrDirectLightShadowV1 under HAL-370]  
**Technical decision record:** HAL-R025-DIRECT-LIGHT-SHADOW-ARCHITECTURE-DRAFT-01 decisions1–13, with HAL-R025-D7-POINT-PCF-TOPOLOGY-DECISION-RESOLUTION  
**Final owner/veto:** JC  
**Point-PCF design disposition:** Six-chart deterministic face-local rule selected; ideal nondegeneracy proved in §7.3. Actual f32/cube/PCF/native quality remains OPEN.  
**Adoption:** PERFORMED 2026-10-31 through the delegated architectural reviewer's coordinated S1 decision under JC's express decision delegation; JC retains final veto.  
**Implementation / proof:** NOT IMPLEMENTED / S2 NOT AUTHORIZED / S3 source/numerical/compiler/Device/backend OPEN / S4 native/performance/JC OPEN. NOT IMPLEMENTATION-READY for engineering or user-facing admission.  
**Current adoption record:** HAL-370-R023-R028-S1-COORDINATED-BINDING-ADOPTION-01.

## 1. Scope, compatibility and bounded profile

The opt-in **PbrDirectLightShadowV1** is compatible only with **PbrMetallicRoughnessRealtimeF32V1** and **HdrSceneColorToneMapV1**. It specifies direct radiometric-RGB-proxy lighting plus geometric visibility from the exact authored scene. It is not spectral accuracy, distributed area transport, environment/GI, photorealism or physical validation.

At most one directional, four point and four spot sources are admitted; each count may be0. Every admitted direct source casts the qualified geometry shadow prescribed here. No implicit sun/lamp/ambient, point-for-spot substitution, missing-light default, unannounced shadow disable, VSM/EVSM, raytracing, SSAO, analytical UI shadow, emissive bounce or blob/contact heuristic is allowed.

When all three counts are0, no R-025 group3, shadow pool/binding/uniform/pass/job or idle demand exists. Independent R-024 emission or separately admitted R-026 work keeps its own identity/cost. No default2D or legacy shader acquires group3, resources or a clock. A nonzero source with zero RGB remains an explicit validated source; zero coefficients cannot conceal stale or missing declared resources.

Requirements below are binding at S1 in [HAL-370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md)'s exact opt-in scope and HAL-355 §17.1's acknowledgment. HAL-370 §1 records the delegated independent technical adoption; it grants no S2 engineering permission or S3/S4 proof. The §7.3 atlas resolves the original global-frame obstruction with declared face-boundary discontinuities, not a guarantee of invisible seams.

## 2. Authority, real baseline and semantic owners

| Owner / current source | Retained boundary |
|---|---|
| HAL-002/002A | Sole retained tree/IDs, immutable semantic input, owner sovereignty and phase separation; no constitutional exception. |
| HAL-355 §§6.1–6.5/7–13 | Checked f64 world translation/f32 local/GPU precision; right-handed +Y, view forward-Z negative, column matrices, reverse-Z; typed leases, complete requests, finite per-target execution. |
| HAL-354 / HAL-357 | Update-owned complete values and optional lowering before preparation; no light semantics from a callback, view image or lowering node. |
| HAL-266A §§3–6 | Actual final physical extent/activity and indexed post-Layout/pre-Paint mechanical preparation; no inferred DPR/aspect or new phase. |
| HAL-295A INV-003–010 | Same private offscreen owner/Device/Queue/front publication and baseline lifetime, no private recovery or surface/graph authority. |
| HAL-263A §3.2, INV-002/004/005/006/008 | Final preassociated linear sampled Image, unchanged closed ingress/modulation/one alpha. Shadow maps never become public Image. |
| R-023 §7, S1 binding under HAL-370 | Complete light/caster/model/mask generation association and no2D decorative substitute. |
| R-024 §§7–8.4, S1 binding under HAL-370 | Exact conservative source/local-frame/normal/BRDF and typed PBR groups0–2; no current light access. |
| R-027 §§4–13, S1 binding under HAL-370 | Complete finite scene radiance certificate, tone each sample before averaging, separate linear-FP16 final and original-front truth. |
| HAL-309 | Source, documentary review, adoption, implementation, GPU output, native observation and JC visual acceptance are distinct. |

Source inspected for this unit: raster.rs:387–456/667–722/1556–1619, appearance.rs:1790–1902/2020–2105, context.rs:896–980/2085–2180, viewport.rs:465–525/1310–1374, custom_render_pass.rs:498–542/611–633. They establish the existing three-group ABI, model/view packing, typed resource/target and Queue/publication seams; they contain no admitted R-025 group3/light/shadow implementation.

The application owns geometry, actual physical scale, light placement/intended meaning and source completeness. Halcyon would own only mechanical qualification/evaluation/resource work under future admitted interfaces. There is no semantic light discovery, global registry, new scene hierarchy or renderer-owned physical truth.

## 3. Immutable source, units and qualification

Each complete light request SHALL bind explicit source identity/revision/order, calibration, exact counts/descriptors, complete structure/geometry/model/material/mask/view data, current target and all dependencies. Missing, stale, foreign, unsupported or invalid input refuses the candidate before work; it cannot become “absent” or successful black/unshadowed output.

**meters_per_scene_unit** is application-authored, immutable, finite and strictly positive. For source displacements, distance_m=meters_per_scene_unit*distance_scene. No asset name/glTF unit, AABB, UI pixel, view image or consumer convention supplies calibration. Its semantic truth belongs to the source producer.

Every position used together for attenuation and shadowing must be in one qualified render-relative metric domain before checked f32 GPU admission. Apply calibration consistently to source/receiver/caster positions and corresponding model/view/shadow transforms; directions remain normalized directions. Never mix a metre-valued radius/range with an unconverted scene-unit vector. This is an explicit new profile/source-packing obligation in the future parent admission; current HAL-355/R-024 matrices and legacy shader semantics do not silently change. Calibration is part of complete identity, never hidden in a reserved zero slot or inferred in the shader.

| Quantity | Exact admitted domain |
|---|---|
| Directional E | Finite channelwise nonnegative RGB irradiance proxy, W/m²; each channel<=4096 |
| Point/spot I | Finite channelwise nonnegative RGB radiant-intensity proxy, W/sr; each channel<=4096 |
| Source radius r | Metres, inclusive[0.01,0.5], strictly positive |
| Point/spot finite support R | Metres, r<R<=50 |
| Point/spot shadow near | Exactly0.01m; r>=near and R>near |
| Directional shadow_far | Finite metres, <=50 and strictly greater than calibrated actual view near n>0 |
| Direction / axis | Qualified finite unit vector, correct world/render-relative convention |
| Spot cone | Radians0<=inner<outer<π/2, so0<cos_outer<cos_inner<=1 |

Directional L is **surface-to-source**; its E is directly incident. A spot axis points **from emitter toward scene**, not toward the light. Point/spot positions share the receiver's qualified metric render-relative domain. RGB inputs/results use R-027 linear sRGB/Rec.709 primaries and are radiometric channel proxies, not verified spectral/photometric measurements.

Source calibration and f32 representability are independent conditions. Even a positive source scale must be refused if metric conversion, required matrices or guarded evaluation cannot meet the admitted precision/domain proof.

## 4. Direct material evaluation and finite support

At each surviving covered sample, use R-024's qualified mapped/back-face-correct shading normal N, view direction V and exact regularized GGX/separable-Smith/Schlick/diffuse response f_material(N,V,L). Use source geometric positions/normals for shadow projection/bias, not a decorative shading normal.

**Binding face-normal resolution, 2026-10-31:** Independent architectural review adopts the following D-present-only interface under JC's express technical delegation and retained veto, coordinated with HAL-370 §3.1/§8 and R-024 §8.4.3. For each original indexed/unindexed source triangle, promote its admitted f32 p0,p1,p2 to f64 and compute the existing winding-validated unit N_face=unit(cross(p1-p0,p2-p0)). Checked f32 conversion must retain finite components/nonzero length usable under the proper checked model normal matrix, without a guessed epsilon or fallback. In original triangle/index order, a D-only mechanical GPU expansion copies three canonical PbrVertex52 corners and supplies three IDENTICAL local N_face Float32x3 values in a second stride12 stream. Draw this expansion NONINDEXED; original semantic mesh/handles/generation and D=0 packing remain unchanged. A shared source vertex may belong to different source faces; its separate expanded corners retain each true original triangle identity. Both buffers, metadata/uploads and current/old/in-flight generations are charged before writes.

D-only vertex input location5 carries that face normal; matching VS-output/FS-input location5 vec3<f32> uses exactly `@interpolate(flat)` without a sampling modifier. Existing locations0/1/3/4 remain perspective-centroid and2 perspective-center. Compute N_geometric_world=unit(Object112.normal_to_world*N_face_local); negate ONLY for a double-sided back-facing receiver, retaining source orientation on the front and existing back-culling for single-sided geometry. Equal normal copies make provoking-vertex choice and lawful clipping preserve face identity. Geometric N serves shadow/bias terms; R-024 shading Ni/mapped N alone serves BRDF. No centroid derivative, shading-N substitution, primitive_index feature, new uniform/binding/group or source pixel-center restriction is admitted. The original separately authorized private R-025 S2 unit continues; this amendment grants no public S3/native S4 claim.

~~~text
direct_rgb = sum_j [
    f_material(N,V,L_j) * max(0, dot(N,L_j))
    * E_incident_j * V_shadow_j
]
scene_rgb = direct_rgb + emission + separately admitted R026_indirect
~~~

Products are componentwise RGB. R-024 emission is included exactly once, independently of AO/base alpha. AO attenuates only R-026's expressly qualified indirect term; it never darkens unrelated direct light or emission. Without admitted R-026 there is no implicit indirect/ambient term. Material mask discard precedes a surviving sample's color/coverage contribution.

For a directional source, E_incident=E and L is its fixed qualified surface-to-source unit vector.

For point/spot, let delta=light_pos-surface_pos in metres and d=length(delta):

~~~text
d = 0: contribution = 0, do not normalize delta
0 < d < R:
    L = delta / d
    w = (1 - (d/R)^4)^2
    E_incident = I * w / max(d*d, r*r)
d >= R: contribution = 0
~~~

The positive r removes the inverse-square singularity; for fixed admitted I,r the incident term is finite/nonnegative and no greater than I/(r*r). This individual bound is NOT a complete R-027 radiance certificate. At d=R support is exactly0. The d0 result is a declared proxy approximation, not exact finite-area transport or a hidden replacement normal.

For spot, multiply by the exact cone factor:

~~~text
q = normalize(surface_pos - light_pos)       only for d > 0
c = dot(unit_beam_axis, q)
t = clamp((c - cos_outer)/(cos_inner - cos_outer), 0, 1)
spot = t*t*(3 - 2*t)
~~~

The denominator is positive in ideal arithmetic under the strict cone rule. Packed shader cosines must also remain finite and strictly ordered after checked f32 conversion; equal packed values refuse before dispatch, without an epsilon/repair. Invalid/equal/reversed/nonfinite angles refuse, not a point-light substitution. Guard actual vector lengths, normalization/dot domains and R-024 zero/noncontributing hemisphere/half-vector paths; do not first produce NaN and rely on an LDR wrapper to sanitize it.

## 5. Geometric shadow sources and fixed pools

All admitted opaque and R-024 Mask occluders use the SAME structural geometry/model generations and exact material alpha source as color. For Mask, mask_alpha=base_texture.a*base_factor.a*vertex_color.a; discard exactly when mask_alpha<the admitted cutoff. UV, sampler, texture lease, cutoff and sidedness must agree with the current material. Opaque ignores material alpha; unsupported Blend/transmission is not silently shadowed as Opaque.

Off-camera casters remain eligible. The source must contain every caster capable of occluding qualified visible receivers; a camera-only retained/culling subset cannot certify shadow completeness. The engine has no authority to infer missing physical objects, fabricate meshes or retain a semantic scene mirror.

When any light exists, reserve the entire fixed pool set before candidate writes:

| Pool | Allocation / mapping | State |
|---|---|---|
| Directional + spot depth2D array | 7 layers of1024×1024; directional0–2, spot3+s for s0..3 | Depth32Float, sample1, one mip |
| Point depthCube array | 4 cubes /24 layers of512×512; cube p, face f, layer6p+f | Same format/sample/mip |

Raster depth is reverse-Z: clear0, write enabled, compare Greater, no stencil; receiver comparison is GreaterEqual. No alpha-to-coverage, VSM/EVSM, screen-space substitute or second contact-shadow algorithm. The nine-test PCF/bias rule is §7. Controlled passes must not read a map as a sampled texture while it is their writable attachment; concrete private depth-pass binding/usage proof remains required.

Pools may contain unused reserved layers, but unused slots cannot be read or acquire hidden light meaning. Up to31 changed map-face writes correspond to active sources. Pool reservation does not authorize31 unnecessary passes every frame.

## 6. Shadow projections and coverage

### 6.1 Directional cascades

For calibrated actual view near n>0 and finite far=shadow_far, n<far<=50m:

~~~text
lambda = 0.5
split_j = 0.5*n*(far/n)^(j/3)
        + 0.5*(n + (far-n)*j/3),    j=1,2,3
~~~

In ideal arithmetic the splits increase from n and split3=far. They are view-depth slice boundaries under actual HAL-355 view geometry, not guessed radial eye distances or OS viewport metrics. A baseline orthographic descriptor allowing near0 does not meet this directional profile's n>0 prerequisite.

Each light-space orthographic slice SHALL conservatively cover its eligible receivers and every relevant shadow caster, including off-camera casters and the required PCF footprint. Fit/quantize its footprint at1024 resolution with conservative expansion, using source-qualified current geometry/model/view and matrix identity. Reverse-Z orthographic depth follows HAL-355: z=(far_light-d_light)/(far_light-near_light), correct right-handed convention and no extra Y reflection.

This profile admits only requests for which all potentially lit visible receiver samples and relevant casters are inside qualified finite cascade coverage. A source/view not meeting that coverage refuses before writes; it is not “no shadow beyond distance.” Split boundaries, conservative quantization/roundoff, cross-cascade footprint and contact/seam fidelity need exact actual source/f32/backend proof. No resolution, blend, camera/DPR or semantic eligibility workaround is selected.

### 6.2 Spot projection and near-cap qualification

Each active spot uses its exact source cone, near0.01m, far=R, finite reverse-Z perspective and layer3+s. Projection, beam axis, cutoff and color support belong to the same light generation. No expanded cone used as a lit fallback or stale projection is allowed.

All relevant receiving/casting geometry must be outside the selected light-camera near cap, with conservative proof before rendering; r>=near alone does not establish that geometric condition. A clipped relevant occluder cannot be silently treated as absent. Invalid coverage/near compatibility refuses the profile/candidate.

### 6.3 Point cube depth and face identity

Each point uses six90° faces, near0.01m, far=R and the standard cube face association (+X,-X,+Y,-Y,+Z,-Z) with qualified right-handed transforms/lookup. Every face belongs to the same light/model/source generation; source p maps to cube p, never a neighboring point's cube.

For light-to-receiver vector h and its selected cube face:

~~~text
d_major = max(abs(h.x), abs(h.y), abs(h.z))
z_receiver = near*(far-d_major) / (d_major*(far-near))
~~~

Use face-axis depth, NEVER radial length(h) in this expression. At near it maps to1, at far to0. Relevant near-cap and all tap/face coverage must be conservatively proved; no read outside map/domain is a lit fallback.

Mandatory ideal witness: h=(3,4,0)m, near=0.01, far=10 gives d_major4, radial d5, hence correct z=1/666 while radial substitution gives1/999. They are unequal even though the point is within range. Actual face-edge/corner/cross-face lookup and tap-specific reference proof remains required under §7.3's selected atlas; global continuity is not required or claimed.

## 7. Visibility, PCF and receiver bias

### 7.1 Exact common nine-test rule

PCF uses the3×3 equal-weight grid, nine comparisons, not an unannounced wider filter. Directional/spot offsets are {-1,0,1} texels in each shadow UV axis. For each admitted tap:

~~~text
z_compare = min(1,
    z_receiver + 1/65536
    + (1/4096)*max(0,1-dot(N_geometric,L)))
pass = (z_compare >= stored_reverseZ_depth)
V_shadow = count(passing nine comparisons) / 9
~~~

N_geometric and L must be actually qualified unit directions; guard dot/normal domain and proper face/source attribution. This is receiver bias only, not a new arbitrary raster depth bias. A normal valid dot in[-1,1] bounds the additive bias by33/65536 before min1. The model is a declared finite geometric-shadow approximation; it does not prove exact ray visibility or absence of acne/leaks.

Here N_geometric is EXACTLY §4's flat source-winding face normal after true normal-matrix transformation and receiving-side reversal, not a vertex shading normal or derivative of centroid position. Mandatory counterexample: N_geometric=(0,0,1), legal smooth Ni=(3/5,0,4/5), L=+Z, model I,k=1, z_receiver=1/2, stored depth=1/2+1/32768. Correct bias delta is1/65536=5/327680, below stored-minus-receiver10/327680: GreaterEqual returns0. Substituting Ni gives21/327680 and incorrectly returns1. Actual pinned shader/Device PCF observations must prove0 versus1; ideal arithmetic alone does not close the gate.

The selected count-of-nine rule requires exactly nine independent Boolean point-depth comparison observations, each returning0 or1 at LOD0, followed by their sum divided by9. At each prescribed directional/spot grid tap or §7.3 point-cube tap, one qualified comparison-sampling observation supplies one Boolean. The normalized point tap retains its explicit per-tap cube-face lookup and face-axis depth reference; no tap is relocated or re-anchored.

Both group3/b3 and group3/b5 SHALL use this SAME exact comparison-sampler state:

| Sampler property | Required value |
|---|---|
| compare | GreaterEqual: depth_ref=z_compare versus the stored reverse-Z texel |
| magFilter / minFilter / mipmapFilter | Nearest / Nearest / Nearest |
| addressModeU / addressModeV / addressModeW | ClampToEdge / ClampToEdge / ClampToEdge |
| minLOD / maxLOD (lodMinClamp / lodMaxClamp) | 0 / 0 |
| maxAnisotropy | 1 |
| Bind-group sampler category | Comparison, not arbitrary filtering or noncomparison |

The Depth32Float2D-array and cube-array maps remain single-mip. No bilinear comparison filtering, hidden hardware multi-tap PCF, second spatial filter or sampler-driven fractional result may stand in for a Boolean. ClampToEdge is fixed sampler state, not permission to admit an out-of-map tap or fabricate lit visibility; §7.2 coverage/refusal remains authoritative. The [WebGPU §7.1.1 sampler contract](https://www.w3.org/TR/webgpu/) distinguishes comparison from filtering; descriptor names alone do not prove the actual observation. Concrete pinned Naga25/wgpu25/Device/backend evidence SHALL establish this exact point-comparison behavior before implementation conformance is claimed. No alternate comparison/filter algorithm, manual-depth-read replacement, fallback or public WGSL entry is selected.

### 7.2 Qualification and contact truth

Visibility is in[0,1] and can only attenuate a qualified direct contribution. A disabled/absent light contributes0, never an unshadowed placeholder. Unavailable/stale/unsupported maps, invalid tap addresses, bad near/far/face projection or mismatched source identity refuse; clamping/wrapping/out-of-range API behavior is not a proof of valid visibility.

All contact effects are consequences of these same current caster/receiver maps and bias/PCF. No analytical2D SDF, ambient silhouette, decorative blob or automatic “contact” darkening term is admitted. PCF/cascade/contact quality must be falsified on real geometry and native output, not named correct by documentary adoption.

### 7.3 Point-PCF: fixed six-chart face-local frame

This is the architect's HAL-R025-D7-POINT-PCF-TOPOLOGY-DECISION-RESOLUTION. It replaces the former impossible global-continuity reading, without changing the selected nine coefficients, one-texel-equivalent radius, pool sizes, visibility/bias or budget.

Let h=surface_pos-light_pos, d=length(h)>0 and q=h/d: **light-to-receiver**, the same convention as §6.3 cube lookup and §4 spot ray. L for illumination is -q. Do not substitute L for q. Normalize the finite nonzero base direction with qualified magnitude-safe arithmetic; d0 keeps §4's zero contribution and does not construct a frame.

Select exactly one owning chart from the largest absolute component of base q. Equal magnitudes choose **X before Y before Z**; sign is the sign of that selected nonzero component. This exact rule owns all edges/corners, with no epsilon, prior frame, temporal state or alternative pole exclusion.

| Owning face / signed dominant normal n | Fixed world-axis U | At center q=n: T | At center: B=T×q |
|---|---|---|---|
| +X | (0,0,-1) | (0,0,-1) | (0,-1,0) |
| -X | (0,0,+1) | (0,0,+1) | (0,-1,0) |
| +Y | (+1,0,0) | (+1,0,0) | (0,0,+1) |
| -Y | (+1,0,0) | (+1,0,0) | (0,0,-1) |
| +Z | (+1,0,0) | (+1,0,0) | (0,-1,0) |
| -Z | (-1,0,0) | (-1,0,0) | (0,-1,0) |

For that one base chart:

~~~text
v = U - dot(U,q)*q
T = v / length(v)
B = cross(T,q)                         NOT cross(q,T)
rho = one canonical cube-face texel equivalent at resolution512
(a_i,b_i) in {-1,0,1} × {-1,0,1}
q_i = normalize(q + rho*(a_i*T + b_i*B))
~~~

The canonical90° cube-face plane spans[-1,+1], so the declared one-texel-equivalent radius is its grid step2/512; at the face center this displaces the corresponding projected coordinate by one texel. This is a fixed tangent-direction kernel, not a claim of constant projected UV spacing everywhere on a face. All nine taps use the SAME base q/owning U/T/B. Ordinary cube face selection/depth lookup uses each normalized q_i; a tap may cross to another face. It MUST NOT re-anchor its frame, clamp, reflect, drop, reorient or change weight at that crossing.

Retain the receiver's radial range d for the displaced tap ray, qualify its corresponding face-axis depth d_major,i=d*max(abs(q_i.x),abs(q_i.y),abs(q_i.z)), then apply §6.3's finite reverse-Z reference/bias. Do not use radial d as face depth, carry a reference from the original face, or change the source light/cube generation. All tap addresses/near caps/coverage still require pre-write qualification; a chart switch does not create a lit fallback.

**Ideal nondegeneracy.** U is perpendicular to its signed dominant axis n. On that chart |q_n|>=|q_U| and q is unit, hence2*q_U²<=q_n²+q_U²<=1, so |q_U|<=1/√2. Therefore length(v)=sqrt(1-q_U²)>=1/√2, including exact face centers, edges and corners. T and B are finite unit tangent directions in ideal arithmetic. Because T,B are tangent orthonormal to q, the displaced vector has squared length1+rho²*(a_i²+b_i²)>=1 before normalization. No geometric pole or zero denominator exists in the admitted atlas.

Evaluating a fixed face's U gives a continuous frame on that face's closed dominant-axis region. The atlas-wide selected frame is explicitly allowed to jump when owning faces change at an inter-face tie. No implicit blend, frame carry-over or seam filter is admitted. The [Hairy Ball theorem](https://arxiv.org/abs/1902.07657) explains why global continuity was not a valid requirement; this atlas does not claim cross-face kernel invariance or zero visible seams. Such quality requires actual oracles, and any stronger invariance would need separate owner authority.

**Exact future fixtures:** all six centers above; all12 edge midpoints and8 corners with the prescribed tie rule; near-tie directions on both sides; opposite-face rays; and kernels whose taps cross one or more faces without re-anchoring. Representative exact values:

| Base q | Owned face | T | B |
|---|---|---|---|
| (1,1,0)/√2 | +X wins X/Y | (0,0,-1) | (1,-1,0)/√2 |
| (1,0,1)/√2 | +X wins X/Z | (1,0,-1)/√2 | (0,-1,0) |
| (0,1,1)/√2 | +Y wins Y/Z | (1,0,0) | (0,-1,1)/√2 |
| (1,1,1)/√3 | +X wins corner | (1,1,-2)/√6 | (1,-1,0)/√2 |
| (-1,-1,-1)/√3 | -X wins opposite corner | (-1,-1,2)/√6 | (1,-1,0)/√2 |

The ideal bounds, signs/ties and kernel topology are selected/proved here; actual source f32 normalization/classification/roundoff margins, cube sampler/face conventions, per-tap projection, numerical precision and native seams/contact quality remain unexecuted proof gates. No fallback or alternative chart is selected.

## 8. Complete requested/prepared/committed association

The immutable complete association SHALL include:

- Profile/version, meters_per_scene_unit, source identities/revisions, all light counts/order/descriptors/limits and exact metric domain.
- Structure, geometry and every model placement/generation, current view/target/sample/activity, normal/UV/base-mask material/texture/sampler/cutoff/sidedness and R-024 response identity.
- Directional splits/fits/coverage, spot/point projections/faces, every shadow matrix and map-content generation, comparison/bias/kernel identity and qualified coverage evidence.
- All scene/shadow/final resources, typed context/kind/nonwrapping generation/leases, actual capabilities/reservations, R-027 certificate/color/exposure/pipeline/final binding identity and truthful stage.

Semantic values are published in ordinary lawful Update. Existing HAL-266A preparation may consume that immutable value plus actual final target metrics and mutate rebuildable private GPU/cache state only. It cannot fetch mutable application state, discover lights, run another Update or create semantic authority from a rendered front.

Changing geometry/model placement/mask texture/cutoff/material visibility, light enable/move or relevant shadow target invalidates dependent map content. Changing view invalidates directional splits/coverage and qualified presentation; it does not automatically change identical spot/point source content. Reuse requires exact dependency identity and current leases, not a frame counter, pointer, old front or similar matrix.

Prepare/validate/reserve ALL shadow+scene+tone/final before candidate writes. Then:

~~~text
complete changed-dependency shadow-depth writes
    -> complete scene PBR direct/material evaluation
    -> R027 per-sample tone before coverage average
    -> R027 fixed linear Rgba16Float final[1]
    -> same existing Queue submit returns
    -> same CustomRenderPass swap / authorized Image registration
    -> atomic committed complete association
~~~

No independent shadow publication, partial color front or new-light/old-shadow mixture. A compatible last-good front keeps its original complete tuple; an incompatible front is withdrawn. Source acceptance/preparation/submit/commit/GPU completion/native presentation remain distinct. GPU readback is not runtime source admission or a post-publish success/failure rewrite.

## 9. Typed PBR group3 ABI

This is a DISTINCT opt-in PBR layout, not current source, a legacy layout change, public raw WGSL or a light assignment in R-024. S1-adopted PBR groups0–2 remain separately governed; the native four-group boundary admits no automatic extra R-026 group.

The D-only vertex/interstage addition is §4's two streams52+12, six vertex attributes and six user varyings/19 scalar components: original locations0–4 unchanged, supplemental input5 Float32x3 and matching output/input5 flat vec3<f32>. Actual max_vertex_buffers>=2, max_vertex_attributes>=6, max_inter_stage_shader_components>=19 and all existing buffer/format/budget limits must pass BEFORE candidate allocation/upload/writes. This adds no group3 field and does not relax any bound below. D=0 keeps original52/five/16 and zero supplemental geometry resources/cache work; legacy36 is untouched.

| Binding | Exact resource contract |
|---|---|
| group3/b0 | DirectLightBlock, read-only uniform,608-byte payload |
| group3/b1 | ShadowTransformBlock, read-only uniform,1984-byte payload |
| group3/b2 | Depth32Float2D-array view,7 layers,1024², sample1 |
| group3/b3 | Comparison sampler with exact §7.1 state: GreaterEqual, mag/min/mipmap Nearest, U/V/W ClampToEdge, LOD0/0, maxAnisotropy1; nine Boolean point-test proof required |
| group3/b4 | Depth32FloatCube-array view,4 cubes/24 layers,512², sample1 |
| group3/b5 | Comparison sampler with the SAME exact §7.1 state as b3; original point-face/near/coverage qualification and Boolean point-test proof required |

DirectLightBlock layout:

| Byte offset | Field / stride |
|---:|---|
| 0 | header vec4<u32>: directional count0..1, point0..4, spot0..4, last0 |
| 16 | cascade_splits vec4<f32>: split1,split2,split3,shadow_far |
| 32 | One Light64 directional |
| 96 | Four point Light64, stride64 |
| 352 | Four spot Light64, stride64 |
| 608 | End; alignment16, no unidentified payload |

Each Light64 is four vec4<f32> a,b,c,d at relative offsets0,16,32,48:

| Kind | a | b | c | d |
|---|---|---|---|---|
| Directional | xyz=L, w0 | xyz=E, w0 | zero | zero |
| Point | xyz=qualified render-relative source position, w=r | xyz=I, w=R | zero | zero |
| Spot | Same position/r | Same I/R | xyz=unit beam axis, w=cos_inner | x=cos_outer, yzw0 |

Used positional/radius/range values obey §3's common metric domain; unused records/components/padding are zero. When directional count is0, cascade_splits and its directional record/matrix slots are zero and never read; an absent directional source creates no shadow_far requirement. No reserved component secretly stores calibration, a light flag or semantic enum. Zero counts prohibit invalid array/matrix access; indices are source/slot-qualified before use.

ShadowTransformBlock has exactly31 column-major mat4x4<f32>, stride64, alignment16, total1984. Matrix mapping:

| Matrix index | Owner |
|---|---|
| 0..2 | Directional cascade0..2; same depth2D layers |
| 3+s, s0..3 | Spot s; depth2D layer3+s |
| 7+6*p+f, p0..3/f0..5 | Point p face f; cube p/layer6*p+f |

Unused matrix slots are zero and never indexed as valid projections. Actual stage visibility, private depth-pass face selection, mask evaluation and non-aliasing attachments must be proved by the exact admitted controlled programs/layouts; source/Device validation cannot be inferred from these byte counts. Both uniforms require actual binding-size/offset/alignment/in-flight-slot and aggregate stage-limit checks. R-026's S1-adopted environment schema must coordinate within the same four-group limit and cannot reinterpret this version's fields.

At least one light reserves the entire two-pool set and associated bindings/capacity. All-zero direct source reserves NONE: no dummy group3, map, comparison sampler or light buffer may appear on that path. The concrete program/layout must prove this absence without changing R-024 emission/indirect or the legacy shader ABI.

## 10. Finite work, lifetime and radiance certificate

Logical shadow depth payload when active is exactly:

~~~text
(3+4)*1024*1024*4 + (4*6)*512*512*4
= 29,360,128 + 25,165,824
= 54,525,952 bytes per full set
~~~

Both light uniform payloads add608+1984=2592 bytes per live qualified slot. Also charge actual alignment, views/comparison samplers, private pass/object/material/view data, pipelines/variants/commands, staging/cache residency and ALL live old/in-flight map generations. This is logical payload, not portable physical driver allocation size or an aggregate memory budget.

R-027 target resources additionally charge(12*N+16)*W*H minimum scene/depth/final payload, plus its actual overhead. No shadow allocation substitutes for current physical scene W/H/activity. Finite effective context limits and actual Device usage/layer/format/attachment/binding constraints apply before work; requests exceeding them refuse, never lower resolution, reduce lights/shadows or silently switch profile.

At most31 active map faces and9 Boolean comparisons per light per shaded sample bound the selected topology. Checked caster draw/index/primitive counts and per-face replication must fit actual finite work policy; worst-case submission draw charge includes every relevant occluder in every dependent face, not only camera-visible packets. With at most9 lights, PCF contributes at most81 declared comparisons per shaded sample, separate from material/other work. These are not measured GPU timings or automatic budget admission.

All resources use the existing Device/Queue/context, typed generations and leases. A front/map under a pending/committed/compatible-last-good/in-flight owner cannot be overwritten/recycled as a new content generation or freed early. Target/mount retirement/unmount and device terminal failure use existing withdrawal/retirement owners, no private recovery epoch, global cache or background scheduler.

**R-027 source certificate remains mandatory before dispatch.** An attributable complete proof must cover actual BRDF*cosine*every light*V_shadow, each V in[0,1], emission and any separately admitted R-026 integral, including source roughness/evaluator/domain/transform/view. Every possible covered RGB component must be finite, nonnegative and<=60000. A channel cap, per-light finite denominator or isolated BRDF bound is NOT that complete proof.

The source integration owner supplies truthful issuer/domain/combination evidence; Halcyon mechanically validates its exact tuple and admitted model. Certificate representation/source admission, actual f32 margins and real furniture applicability remain OPEN, not a fabricated Rust API. No per-pixel CPU/GPU readback, post-publish relabel or60000 clamp substitutes for proof.

## 11. Paired invariants and prohibitions

**INV-R025-LIGHTS.** Complete immutable source counts/kinds/order and every descriptor are explicit, bounded and current; all admitted lights own qualified geometry shadows.  
**FORBID-R025-LIGHTS.** No implicit light/ambient/GI, invalid-to-absent default, spot substitution, stale source or silent shadow disable.

**INV-R025-UNITS.** Positive finite authored calibration and one qualified metric calculation domain bind positions/radii/ranges/projections to the same tuple, with checked representability.  
**FORBID-R025-UNITS.** No meters from assets/pixels/images, mixed units, hidden calibration slot or physical truth inferred from GPU geometry.

**INV-R025-BRDF.** Direct uses the exact R-024 response*cosine*incident*visibility, emission once; AO only qualifies separately admitted indirect.  
**FORBID-R025-BRDF.** No alternate material/proxy diffuse, direct AO, duplicate emission, singular d0 normalize or sanitized NaN success.

**INV-R025-SHADOW.** Depth32Float/sample1/reverse-Z opaque/masked caster data and receiver comparison are source-qualified; point references use face-axis depth.  
**FORBID-R025-SHADOW.** No radial cube depth, decorative2D/SDF/contact blob, unsupported transparency, independent shadow publisher or cross-light face alias.

**INV-R025-COVERAGE.** Conservative current cascade/cone/cube/near and PCF coverage includes relevant off-camera casters and all potentially lit visible receivers before work.  
**FORBID-R025-COVERAGE.** No camera-only caster source, clipped/out-of-map lit fallback, missing far shadows or guessed target aspect/DPR.

**INV-R025-PCF.** Exactly nine independent Boolean LOD0 point comparisons/bias define visibility through the SAME §7.1 GreaterEqual/Nearest/ClampToEdge/LOD0/anisotropy1 comparison state at b3 and b5; point taps use the fixed base receiver's X/Y/Z-tie-owned six-chart frame, exact U table and B=T×q, with unchanged weights and ordinary cross-face lookup.  
**FORBID-R025-PCF.** No fractional result substituted for a Boolean, bilinear comparison filtering, extra hardware filter/kernel or second spatial filter, global sphere-continuity claim, tap re-anchoring/clamp/drop/reflection/reorientation, hidden seam blend/history, other pole exclusion or false zero-seam quality claim.

**INV-R025-ABI.** Distinct profile group3/b0–5 has exact608/1984-byte blocks, fixed source/map indices, explicit zero unused data and actual stage/Device qualification.  
**FORBID-R025-ABI.** No legacy/group2 field reuse, public WGSL/raw GPU escape, unidentified slots, unguarded indexing or automatic future fifth group.

**INV-R025-PROVENANCE.** Source/calibration/structure/model/material/mask/light/view/target/maps/matrices/color/front belong to one complete immutable requested/prepared/committed association.  
**FORBID-R025-PROVENANCE.** No fresh light/color with stale map/mask, partial commit, old-front relabel or semantic write in preparation.

**INV-R025-RESOURCES.** Complete fixed pools54,525,952 plus2592-byte uniform payload/live-slot and all actual overhead/work/in-flight charges are reserved within effective finite limits before writes.  
**FORBID-R025-RESOURCES.** No guessed aggregate capacity, omitted old history, resolution/light downgrade, hidden churn or unbounded pass/draw/tap work.

**INV-R025-LIFETIME.** Existing context/Device/Queue and private final publisher retain exact live generations through queue-safe retirement and compatible original last-good truth.  
**FORBID-R025-LIFETIME.** No new device/queue/recovery/global cache, early release, writable published front or successor generation rebinding.

**INV-R025-ABSENCE.** Zero direct counts, unused profile and inactive/zero/retired target have no R-025 allocation/pass/job/idle demand; independent material/indirect owners remain distinct.  
**FORBID-R025-ABSENCE.** No dummy group3/map/sampler/light buffer on zero-use, all-widget scan, second tick or2D/legacy migration.

**INV-R025-PROOF.** Architect-selected equations/atlas and ideal nondegeneracy, source/certificate, host/ABI/compiler/GPU/native and JC quality acceptance are separately recorded.  
**FORBID-R025-PROOF.** No adoption or runtime support from a draft, source coefficients, compiled shader, statistics, screenshot golden or ideal arithmetic.

## 12. Exact S1 parent-extension and compatibility matrix

HAL-370 §8 and HAL-355 §17.1 adopt only the exact optional deviations below; existing HAL-354/357/266A/295A/263A/309 boundaries are preserved, not amended. The first extended request version is unlinked flat-only and refuses linked G1. S2 authorization, source integration and actual proof remain required; no runtime support is established.

| Owner / clauses | S1-coordinated obligation and remaining engineering/proof |
|---|---|
| HAL-002/002A | Preserve sole ownership/IDs/immutable semantics/read phases/sovereignty. No new clock, generic registry, peer reference or constitutional exception. |
| HAL-355 §§6.1–6.5/8 | Exact calibrated render-relative light/receiver/caster/projection qualification and shadow source domain; no legacy unit/matrix/ray/depth change. |
| HAL-355 §§7/8.3/11 | New explicit typed light/profile/matrix/map identities and immutable leases in complete requests/fronts, invalidation/reuse/last-good/source failures. No existing API claimed. |
| HAL-355 §§9.1/9.3–9.5 | Distinct PBR group3 and controlled light/shadow/depth programs, exact material/mask/zero-use binding/stage/index/pipeline validation; legacy groups0–2 untouched. |
| HAL-355 §§10.1–10.4 | Fixed private depth pools, conservative geometric projection/coverage, source-qualified comparisons and ordered shadow->scene->R-027 output within existing target/preparation/Queue. |
| HAL-355 §§12–13/15/17 | Typed missing/stale/unsupported/overbudget/refused source outcomes, exact fixed pool/work/in-flight charges; explicit scoped admission of baseline lighting/shadow non-goals. No hidden fallback. |
| HAL-354 §4.1 / mounted G1 | Complete source publication before preparation; existing G1 writer/receipt/cutoff signatures remain unchanged until separate exact admission. |
| HAL-357 §§6–8 | Lower complete scene/light/caster dependencies to admitted flat representation before raster preparation; no new hierarchy or domain renderer. |
| HAL-266A §§3–6 | Actual current physical metrics/activity, indexed mechanical preparation, read-only Paint; fixed shadow resolution does not become target/DPR authority. |
| HAL-295A INV-003–010 | Same private offscreen/Device/Queue/final Image publisher, enqueue before swap/registration, no graph/surface/recovery or independent shadow publication. |
| HAL-263A §3.2, INV-002/004/005/006/008 | Only R-027 fixed linear-FP16 display-LDR final crosses existing qualified Image ingress; shadow/HDR scene resources never do. No alpha/gamma repair or new2D pass. |
| R-023 §7, S1 binding under HAL-370 | Source-qualified light/caster/mask/model coherence, real moving-object/contact proof; no R-013 decorative2D equivalence. |
| R-024 §§5/7–8.4, S1 binding under HAL-370 | Same factors/channel/mask/normal domain/regularized BRDF and S1-adopted PBR groups0–2; current source/producer/f32/backend proof still OPEN. |
| R-027 §§4–13, S1 binding under HAL-370 | Complete<=60000 source bound, sceneN1|4/no HDR resolve, per-sample tone then average, fixed linear-FP16 final[1], exact finite target/front publication. |
| [R-026](SPEC-R-026-Environment-and-Indirect-Lighting.md), separate S1 binding feature under HAL-370 | World-aligned source-qualified E and independent static two-bounce G use b6–b10; R-025 b0–b5 remain unchanged. Coordinate eight Direct/E/G cases, including zero-direct cases, within four groups; AO affects only indirect diffuse and complete source-certified radiance includes all actual terms. S2 authorization and producer/numerical/backend/native gates remain OPEN; no implicit environment/GI/ambient is supplied here. |
| HAL-309 | No automatic adoption, implementation-ready claim, test/native acceptance or successor from this document. |

The point-frame rule is now exactly the architect-selected §7.3 atlas. No inactive parent/helper name/industry convention changes it. Topology closure is not parent adoption, source implementation or f32/native proof.

## 13. Future proof oracles and open gates

No test/source/shader/GPU work is executed in this unit.

| Owner boundary | Required future evidence on the intended path |
|---|---|
| Source / CPU geometry/context | Calibration/range/RGB/r/cone boundaries, explicit invalid/missing/stale refusal, d0/R support, current source units and representability; full radiance certificate, not an isolated cap. |
| Math / appearance/raster | Exact R-024 response/direct/AO/emission partition; spot center/penumbra/outside cubic, directional split order/endpoints, guarded normalization/grazing cases; actual f32 margins. |
| Projection / raster/backend | HAL-355 winding/reverse-Z/Greater/clear0 and GreaterEqual bias, point1/666 versus radial1/999, all six frame centers/12 edge ties/8 corners/opposites, base-frame preservation and cross-face taps, atlas discontinuities, near-cap/cascade/target coverage. |
| Point comparison / raster/backend | For adjacent stored depth texels0.25 and0.75 with depth_ref0.5, §7.1 point observations SHALL return1 and0 respectively. A LINEAR compare can mix those tests to a fractional value (e.g.0.5 at the midpoint), which is not one admitted Boolean. At an exact nearest-texel edge tie either Boolean may be selected, never a fractional result. Validate both b3/b5 states, 2D/spot and cross-face point taps, and exactly nine independent0/1 observations summed/9 on the pinned compiler/Device/backend; no executed proof is claimed here. |
| Geometric face / geometry/raster/backend | Prove original triangle/index order, exact deindex copies and identical per-face f32 normal triples on adjacent shared-index triangles with different faces. Actual Naga25/wgpu25/Device validates52+12/two buffers/six attributes, matching flat location5/19 scalars and unchanged original qualifiers; executes N1/N4 clipping/partial coverage, proper positive nonuniform normal transform and double-sided back-face reversal. Run §7.1's exact bias witness through real PCF: geometric normal comparison0, deliberately incorrect shading-N probe1. The earlier hypothetical centroid-derivative quad witness is ideal only, not an observed Metal sample pattern. Preserve D=0 five/16, zero work, all R-024/R-027 regressions and independent shading N. |
| ABI / context/backend | Reflect608/1984, all fields/zero slots,31 matrix/layer/cube indices, exact sampler nine-test semantics, all-zero group3 absence and aggregate Device/stage/layer/format/limit validation in Naga25/wgpu25. |
| Shadow source / viewport | Off-camera opaque/mask occluders, material alpha cutoff/texture change, moving furniture/light invalidation, no old map with fresh source; view-only directional versus unchanged local-light reuse. |
| Resource/lifetime | Entire fixed pools,2592 per live slot plus real overhead/in-flight history,31 face/draw bounds/9 per light, refusal without downgrade, target collapse/retirement/mount/device failures and original last-good provenance. |
| Canonical native consumer | Real source->shadow->scene->R-027 FP16 Image->native frames with both main colorN1/N4, point/spot/directional presence/zero counts, contact/acne/leaks/cascade seams, no2D/legacy change and JC perceptual judgment. |

Actual GPU pixel/sample observation belongs in existing owner tests/backend fixtures; tests/app_frame_realization_native.rs is the external canonical App::run owner, not evidence of an existing light fixture. No screenshot/pixel golden or draw count proves perceptual/physical quality.

| Gate | Current status / actual missing evidence |
|---|---|
| BLOCK-R025-PARENT | Exact parent/profile/source contract S1 ADOPTED under HAL-370; OPEN — separate S2 authorization, concrete integration and actual S3 owner/Device compatibility proof. |
| BLOCK-R025-SOURCE-CALIBRATION | OPEN — truthful source units/completeness, metric conversion/geometry/mask/view/coverage and real producer representability. |
| BLOCK-R025-RADIANCE | OPEN — complete R-024/direct/R-026/emission source certificate and actual f32 response bound; no60000 clamp/readback. |
| BLOCK-R025-DEVICE-ABI | OPEN — actual Naga25/wgpu25 controlled programs, four-group/stage/layer/format/sampler/usage/limits and zero-use absence. |
| BLOCK-R025-SHADOW-NUMERIC | OPEN — six-chart ideal architecture is selected; actual cascade/cube/per-tap depth/PCF/bias/normal/f32/coverage margins, discontinuity/seam/contact behavior still need proof. |
| BLOCK-R025-RESOURCE-LIFETIME | OPEN — finite real reservation/work/in-flight generations, dependency reuse, same-Queue transaction/current target and compatible old-front integration. |
| BLOCK-R025-CONSUMER-NATIVE | OPEN — complete real consumer inputs/native frames, actual material/light/shadow/contact quality and JC visual acceptance. |

The complete selected design, including the topology resolution, is Adopted / Binding [S1] under HAL-370. S2 authorization and all implementation/source/backend/native proof gates remain OPEN; no source work or successor follows from adoption.

## 14. Documentary verification and evidence references

Current baseline references are the files/sections in §§2/12, not a mass hash receipt. R-023/R-024/R-027 were read at respective SHA256 bc3cbed9c0bfc8e3df5bdcbdfeda4f893d04b0b62c94f5f081eccb404a0c7f36,99d745f68e26690c7c695444a3126627d13ef17b2c60e488b088a2a055b36e53,d347a8686fe6e51a4e0227032fc74b904eacb1ad160ce69d845d95247e3885d8. They are protected inputs, not authority activation. Relevant source read hashes include HAL-355 edc1bcb236f9ce104d79841ba2649767e0067aa0d1bc6a979b2c3cabbe6ed585, raster192b891d7d1d399379f6dd1b28cd5a8c58bedac7b121c741499478407bccb25b and custom pass93520bad97dd74565a84828bf0087fc578e5323d1d92fb0ac6b20ac47fcb9503.

Exact Grip documentary commands:

~~~text
ws halcyon
read docs/specs/SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md headings
read docs/specs/SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md 1 800 max-lines 800
read docs/specs/SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md hash
state git
~~~

Full artifact readback/source SHA, exact new-file-only scope and the selected atlas/ideal proof must be reported honestly. No existing runtime capability, test count, native result, actual sampler/backend acceptance or parent adoption is inferred.

The [official WGSL texture comparison reference](https://gpuweb.github.io/gpuweb/wgsl/#texturesamplecompare) distinguishes a single comparison from a sampler-filtered result and supplies depth-array/cube-array forms; pinned [wgpu25.0.2 types](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-types/src/lib.rs) supplies comparison/format/limit descriptions. They are reference evidence, not acceptance by the actual Naga/Device/backend. No newer feature or arbitrary shader API is assumed.

## 15. Proposed S1 — per-occurrence complete R025 source geometry reuse

**Adopted / Active / Binding scoped incremental CPU geometry profile.**
Independent architectural review adopted
the exact §15 candidate under JC's large architectural-floor program in
`HAL-R025-INCREMENTAL-CASTER-BASIS-S1-ADOPTION-AND-S2-EXECUTION-02`.
The reviewed beforeimage is lines 493–660 at commit
`0aed295838bd3dbc2cf59ae10d0828efb4a177f5` and whole-document SHA-256
`0da71074bf2523d59992319e70086bd31616b59ab536cbbba79c23ac9821d815`.
The following proposal status record is historical. This adoption activates
only the scoped technical clauses in §§15.1–15.5. The same task separately
grants one bounded S2 implementation and proof unit. S2 evidence is recorded
independently of this adoption. S3/S4 and physical visual acceptance are
separate.

**Proposed / Inactive / Non-binding.** The delegated architectural reviewer
selected this scope for a proposal under JC's
large architectural-floor program. Independent technical adoption and a separate
S2 grant are required. The proposed route has no implementation or conformance
at the inspected revision `c80d8e072794e9bf0858dc4f6e70c13409409fbf`. Sections
1–14 retain their original text, authority and historical status records.

### 15.1 Authority and scope

The proposed clarification of §8 applies exact dependency identity to the
view-independent geometry result of one packet occurrence. The whole-source
association, invalidation of dependent maps on placement changes, and the
current view, caster, receiver and shadow obligations of §§3 and 5–11 remain
complete. In particular,
§8's exact-dependency rule at line 296 and the provenance invariant at line 405
still govern whole map and front reuse. The current implementation's exact
whole-basis guard also requires the same model and Direct captures. That guard
stays in force for whole-basis and radiance reuse. The proposed permission is
limited to a separately qualified packet-local geometry result for a model-only
successor. [Current whole-basis and radiance guards](../../src/three_d/direct_shadow.rs#L934-L1018)

HAL-355's immutable flat structure, target, lease and last-good rules and
HAL-370 §§4 and 9–10 continue to govern. Each changed dependency still receives
its required original source, host and current frame checks. HAL-357 and
HAL-266A/HAL-295A keep their existing phase, writer and publication authority.
R-023/R-024/R-026/R-027/R-028 retain their source, material, indirect and output
contracts. This section changes no other clause or SPEC, including the parent
compatibility records.

### 15.2 Exact predecessor and occurrence eligibility

- The ordinary unlinked flat R025 target owner may use only its immediately
  preceding, successfully qualified and still-live basis. The same active
  target lineage, extent, sample count, view descriptor and view revision,
  Context3D resource family, live Device/Queue, render-origin binary64 bits and
  positive finite calibration `k` binary64 bits MUST match. Inactive or absent
  direct use performs no occurrence lookup, allocation, rebuild or idle work.
- The candidate must pass ordinary successor admission with the same strong
  `Arc<MaterialSnapshot3D>` and a distinct, strictly advancing immutable
  `MaterialModelState3D` bound to that snapshot. The ordered packet and placement
  counts must agree. Only placement values and the model revision may change as
  independent inputs. The current Direct capture and HDR association use the
  lawful current model and any revision advances required by the existing
  successor rules. Authored light identity, presence, order and descriptor bits,
  HDR identity and exposure/bound bits, and independently qualified indirect
  inputs must stay the same. Changed indirect dependencies still require
  current R026 evidence. A stale G capture cannot be kept by reusing Direct
  geometry. [Snapshot and model admission](../../src/three_d/snapshot.rs#L154-L177) · [PBR successor checks](../../src/three_d/pbr.rs#L178-L208) · [View and HDR successor checks](../../src/three_d/viewport.rs#L4293-L4321)
- One immutable chunk belongs to one authenticated index in that exact snapshot's
  ordered packet array. The snapshot pins the original geometry generation and
  certified source/triangle interpretation, material and Mask leases, and raster
  semantics for the slot. Stable ordinals, repeated geometry handles, equal
  packet counts, names, digests and equal-looking packed transforms confer no
  occurrence identity. This exact-snapshot gate prevents positional cache drift
  across removal, insertion or reorder. [Immutable packet capture](../../src/three_d/snapshot.rs#L114-L120) · [PM-010 positional invalidation failure](SPEC-PM-010-Incremental-Cache-Fragility.md#3-root-cause-architectural)
- Reuse requires a completed proof for that slot, equal translation binary64
  bits and equal quaternion and positive scale binary32 bits, including signed
  zero, and the same actual checked matrix dependencies. The previously checked
  model and normal packing must apply bit-for-bit under the same source, origin,
  calibration and canonical transform derivation. Required current host and
  packing checks remain in force. A change to any placement bit rebuilds the
  whole slot using the original qualified triangle interpretation, winding-face
  and transformed-normal checks, `metric_object` packing, binary64 metric
  triangles and `metric_position_intervals` where directional is present.
  [Checked metric packing](../../src/three_d/direct_shadow.rs#L719-L758) · [Existing slot calculation](../../src/three_d/direct_shadow.rs#L1279-L1324)

### 15.3 Complete recomposition and current proof

The new basis MUST carry the current strong `DirectSource3D` capture. Its
snapshot and model MUST match the current scene's exact strong references. The
existing complete light/source and R027 radiance validation, or the existing
independently validated witness for that exact current scene, MUST qualify the
current request. A predecessor supplies only proved geometry chunks. Its
complete source, model, HDR or radiance witness, view proof, map and front confer
no current certification, including when every slot's placement bits are
unchanged. [Source provenance checks](../../src/three_d/direct_shadow.rs#L454-L495) · [Current validation and basis selection](../../src/three_d/direct_shadow.rs#L1497-L1537)

Recompose the complete ordered triangle sequence and, when directional is
present, the matching interval sequence by concatenating slots in snapshot
order and original triangles and corners in their existing order. Each current
triangle and interval has exactly the bits and position it would have after the
original full calculation. Proof consumers MUST receive those full sequences
with the same grouping across slot boundaries, operation order and first
failure. Any assembly scratch is private to the current preparation and must be
charged. A point/spot-only source keeps the current empty directional interval
sequence.

Run the current full frame calculation on that result. It checks the actual
view and packed matrices, clips all receiver triangles to the current frustum,
fits all three directional cascades when selected, and proves packed f32 depth
and all-caster XY/PCF coverage. It repeats the required point and spot near-cap,
face/cone and tap/receiver proofs for all current dependencies. The same
off-camera casters, geometric winding, Mask/Opaque raster evaluation, active
face order and changed-dependency shadow draws remain mandatory. No chunk
authorizes culling, reuse of an invalidated map, missing receiver proof or a
lit result for out-of-range coverage. [Directional calculation](../../src/three_d/direct_shadow.rs#L1586-L1858) · [Point and spot calculation](../../src/three_d/direct_shadow.rs#L1866-L2022) · [Existing caster encoding](../../src/three_d/raster.rs#L1460-L1518)

### 15.4 Reservation, custody and refusal

The existing `DirectTarget3D` and `RetainedQualifiedDirectSource3D` remain the
target-local owners. A new basis owns its complete ordered slot references and
its own current source association. Shared immutable chunks carry their own
retained CPU charge through the last actual holder. A chunk holds no link to
an entire predecessor basis or its model. Lookup is confined to the preceding
qualified basis of this target. No history table, global cache or extra
writer, worker, Queue, feature or user-facing mode is introduced.
[Existing ownership](../../src/three_d/raster.rs#L872-L906)

Plan and reserve all actual candidate costs within the existing checked
`Context3D` limits before allocation, recomposition or candidate writes. Charge
the basis and its slot table, each newly allocated chunk including its Arc
header, inline keys/proof metadata and the triangle and interval allocation
capacities and alignment, and current reconstruction/coverage scratch and work.
Charge shared payload storage once by actual ownership. Charge every separately
allocated slot reference/table and any copies or scratch that coexist with it.
Preserve the existing projection/fit reserve and the complete GPU pool,
attachment, dynamic-storage, work and in-flight charges. Live old, pending and
retiring generations remain in the ledger until their actual CPU/Queue owners
can release them under the existing rules. Chunks are immutable CPU storage.
Existing submission custody retains the actual GPU resources that it uses.
[Current charge partition and reservation](../../src/three_d/raster.rs#L1103-L1160) · [Context accounting](../../src/three_d/context.rs#L2216-L2299) · [Retirement boundary](../../src/three_d/raster.rs#L2708-L2734)

An ineligible or incomplete optimization input or an unreservable optional
representation is an optimization miss. Drop only the unused candidate
reservations, then take the exact existing full R025 calculation if the same
current request independently qualifies and its complete costs fit. A cold or
full calculation can retain the same proved per-slot results when that
representation is separately budgeted, within its one basis and the same
calculation. If the optional form cannot fit, the existing complete-array form
remains available under its original charges. A current source, transform,
coverage, capability or full-path budget failure returns its original typed
refusal. A failed candidate changes neither current publication nor compatible
last-good provenance. Incompatibility and retirement withdraw the target under
existing rules. Quality, sample count and policy caps stay unchanged.

### 15.5 Future S2 boundaries and proof

After adoption and a separate implementation grant, one source unit would
isolate the existing slot calculation in `direct_shadow.rs`, compose its
proved output under the current source, and keep the existing whole-basis
route for an exact view-only match. The existing raster target preparation
would qualify the predecessor and reserve the selected form, then hand the full
sequence to the unchanged current shadow calculation and submit/commit path.
The snapshot/model admission owner stays unchanged. The existing direct-target
reuse predicate and its GPU dependencies are separate from this CPU change.
[Whole-basis preparation](../../src/three_d/direct_shadow.rs#L1368-L1423) · [Raster predecessor and map selection](../../src/three_d/raster.rs#L4076-L4118)

| Future S2 case | Required observation |
|---|---|
| Fixed 540-occurrence floor, one moved slot | On the admitted partial route, 539 reused and 1 rebuilt. All 6,480 triangles and 19,440 directional corner interval triples remain in current order. Compare their coordinate and endpoint bits, the checked D packing, Light608/Shadow1984 bytes and active faces with the original full calculation of the same current request. |
| New model revision with identical placement bits | All 540 chunks may reuse if eligible. The current source still receives its own valid proof and all current view and shadow work. Camera-only exact-basis reuse and identical-request no-work behavior are separately retained. |
| Changed snapshot, geometry, Mask/material, added/removed/reordered slots, Context/Device, origin or k | No cross-association chunk hit. Check existing source or capacity refusal when applicable and full-calculation parity otherwise. Repeated handles and unchanged ordinals cannot make the candidate eligible. |
| Changed authored HDR/light or mixed view/model successor, stale Direct capture or stale scene snapshot | The new route is ineligible. Keep ordinary view-only reuse and legal full successors. Current Direct, model and radiance mismatches return their existing typed failures without publication. |
| Real directional, point and spot geometry, including off-camera Opaque/Mask and invalid near/far/PCF inputs | Compare the full and incremental results on the same current source and effective conditions. Require matching typed geometry/coverage refusal and compatible old front on failure, or matching current packed proof and actual shadow output on success. |
| Optional storage miss, full-path capacity refusal, no view/unlit, last-good overlap and retirement | Exercise the exact full branch or its typed refusal. Check retained and scratch charges, unchanged caps, no new work when inactive, no early release, and no retained history after the last live owner releases. |
| Native N1/N4 at physical 1920×1080 | Use the existing 540-occurrence `App::run` owner, `--run-arch-floor-native` with `--samples-one` for N1. Compare actual source-associated shadow/FP16 pixel evidence with the full calculation. Require the current model/view/serial and target, registered Image, submitted and presented-called AppFrame. Observe physical scanout separately if available. |

Record checked slot counts, actual reused/rebuilt chunks, charged live/peak
bytes by branch and elapsed CPU time for the slot comparison, rebuild and
sequence reconstruction. Keep these separate from `direct_source_validation`,
receiver clipping, cascade fit and depth/XY/PCF checks in the existing
test-harness diagnostics. Compare the same current input in the same proof
conditions and record any ordinary full-path case as such. No GPU timer feature,
FPS claim or S3/S4 acceptance follows. The floor source and its native and
custody tests are existing baseline owners. This S1 executes no tests and
claims no new conformance. [Floor source](../../tests/app_frame_realization_native/arch_floor_source.rs#L118-L239) · [Native owner](../../tests/app_frame_realization_native/arch_floor.rs#L78-L132) · [Resource oracle](../../src/tests/three_d_viewport.rs#L53-L186)
