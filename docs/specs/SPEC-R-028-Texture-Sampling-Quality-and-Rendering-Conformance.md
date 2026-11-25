# SPEC-R-028: Texture Sampling Quality and Rendering Conformance

## Adopted real-time numeric proof-boundary reform — 2026-11-03 [S1]

**Adopted / Binding [S1].** The coordinated adoption record below activates the complete 2026-11-03 reform reviewed at source revision `8d449eeb` across R024/R028/HAL370.

The dated reform clauses below are effective binding replacements or scope qualifications for their named clauses. Retained original text remains binding except to the exact extent superseded/qualified by this adopted boundary. The 2026-11-01 source-domain/backend-trust adoptions and all older unrelated S1 text and historical receipts retain their original scopes; JC's reserved constitutional/product veto remains.

The adopted contract removes mandatory production proof-only per-FOV GPU-numeric prediction sweeps while preserving original source validity, actual host checks, Context/Device/custody, actual R025 work, R027 and the existing quality gates. This documentary S1 adoption grants no implementation or S2 engineering; a separately authorized complete S2 unit remains required.

**S3/S4 remain CLOSED.**

**Earlier adopted status and dated records continue below, subject only to the exact 2026-11-03 qualification.**

**Author:** JC  
**Date:** 2026-10-31  
**Status:** Adopted / Binding [S1 — PbrTextureMipSamplingF32V2 under HAL-370]  
**Adoption:** PERFORMED 2026-10-31 through the delegated architectural reviewer's coordinated S1 decision under JC's express decision delegation; JC retains final veto.  
**Current adoption record:** HAL-R024-R028-HAL370-S1-REAL-SOURCE-COORDINATED-ADOPTION-01.
**Implementation status:** Old-domain private R-024–R-028 S2/internal unlinked Material writer ACCEPTED. Expanded original-source/frame/normal-domain/packed-f32/Naga/Device/Metal proof NOT ACCEPTED and OPEN. No new engineering grant; public S3 and native/performance/JC S4 remain CLOSED.\
**Original design mandate:** HAL-R028-TEXTURE-SAMPLING-QUALITY-ARCHITECTURE-DRAFT-01; JC retains final ownership and veto.  
**Layer:** Optional typed 2D material mip content, controlled PBR sampling and stage-qualified rendering conformance

## Historical drafting scope — 2026-10-31

This section and the following architecture review retain the original inactive-draft task and one-file scope. The current header and §1 supersede only their present-status reading; original source/proof receipts are not rewritten.

- Objective: Create one complete inactive R-028 with the architect-fixed role filters, chain dimensions, sampler/gradient rules, finite leases, transaction, numeric oracles and quality/performance gates.
- Standing objective: Complete the finite R-023 through R-028 native-quality architecture without changing existing functionality.
- Review authority: JC owns product/constitutional decisions and veto; the delegated architectural reviewer owns the exact technical design and independent review. Documentary source qualification cannot select a substitute for the contract.
- Source qualification: HAL-355/263A/266A/295A/354/357/309 and R-023/024/025/026/027 boundaries and actual content/upload/sampler sources were requalified.
- Governing authority and status: Supreme HAL-002/002A, binding named HAL parents at declared scope and adopted HAL-309. R-023 through R-027 remain inactive proposals; R-026 documentary review does not activate it.
- Authority change, if any: None effective. This file proposes V2 only and amends no existing authority.
- Target layer and existing state verified: Halcyon0.227.18-beta, HEAD `46c9d00d`; current Texture3D has one2D level, current sampler has U/V and min/mag only, current upload allocates one mip and writes level0. R-028 was absent; all existing R-draft and independent ComboBox/TagInput dirt is outside this unit.
- Owned mechanisms and files: CREATE ONLY this document. No source, other SPEC, test, asset, manifest or example modification.
- Named invariants: Constitutional owner/ID/phase/immutable-source/sovereignty; HAL-355 complete typed leases/target/front/absence; HAL-263A premultiplied Image; unchanged R-024 evaluator/normal certificate, R-025 shadows, R-026 single-mip E/G and R-027 final output.
- Forbidden behavior: Silent V1 migration, sampler/role override, material Image premultiplication, hidden color decode, arbitrary shader/GPU escape, fifth group, second owner/clock/publisher, unbounded retention, implicit cube/depth/HDR mips or fabricated quality proof.
- Existing mechanisms inspected and reuse decision: Context3D typed generations/leases and bounded upload, immutable flat RenderSnapshot/ModelState, Viewport3D, proposed five-role material family and existing per-target/same-Queue offscreen/Image publication only.
- Relevant external or dependent boundaries: Independent producer owns decoding, original image/role/UV/sampler provenance and complete CPU chain generation. Khronos/WGSL are references, not Halcyon authority or pinned backend proof. No importer/download.
- Test ownership and placement: Documentary mathematics, reference and source review now; future existing three_d_context/geometry/appearance/raster/backend/snapshot/viewport and custom_render_preparation tests; canonical App::run native proof in tests/app_frame_realization_native.rs. No test file or pixel golden added.
- Exact runnable verification commands and expected proof: Grip block `ws halcyon`, `read docs/specs/SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md 1 1000 max-lines 1000`, `read docs/specs/SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md hash`, `state git`; full readback/hash, formulas/links/status and one new attributable file. No Cargo/GPU/native job.
- Output or artifact completeness boundary: All fixed A–N choices, complete source/role/dimension/sampler/ABI/cost tables, worked cases, matched invariant/prohibition pairs, parent matrix and OPEN proof gates.
- SemVer and changelog impact: N/A, no executable/public API change.
- Out of scope: Source/asset/parent/R-023 through R-027 edits; importer, UV1/transforms/atlas policy, anisotropy, new roughness model, general transparency/refraction, runtime GI and device recovery.

## Historical architecture review

- Governing SPEC/read status: HAL-002/002A supreme; named HAL owners retain their authority; all predecessor R proposals stay inactive.
- Widget policy or N/A: N/A, renderer/source design.
- Interaction added/policy: None; no input, Capture, camera-controller or widget behavior change.
- Applicable FORBID rules: Sole ownership and phases; no raw GPU/false front/unlimited budget/2D bleed; exact Image alpha, target metrics and same-Queue publisher.
- Existing framework mechanism inspected: content.rs Texture3D/Sampler3D, context.rs one-level upload and linked min/mipmap filter, R-024 material/normal ABI and actual flat raster/viewport/offscreen source.
- Reinvention risk: V2 is a future opt-in extension of the same owners, not a renderer, global cache, scheduler, GPU mip generator or public shader service.
- Demo/example compliance: N/A, none changed.

**Binding source-domain Revision02 adoption — 2026-11-01:** Independent architectural review under JC's express technical-acceptance delegation and continuing constitutional/product veto affirmatively adopts `HAL-R024-R028-HAL370-S1-REAL-SOURCE-COORDINATED-ADOPTION-01`. The reviewed Revision02 proposal is preserved outside the active source tree, SHA-256 `81b768ded17fc89bded86bf653dca51c5a6546a82e679edcf856fb69374e67f5` (75,195 bytes/493 lines). The original proposal remains historical nonbinding material; the coordinated operative clauses in R-024/R-028/HAL-370 are now S1 binding. Old private R-024–R-028 S2 and the internal unlinked Material writer are accepted ONLY for previously tested old inputs. Expanded source/packed-f32/Naga/Device/Metal proof, public S3 and external-consumer native S4 are NOT ACCEPTED; this documentary adoption grants no new engineering work.

**Binding backend-numeric trust-boundary amendment — 2026-11-01:** Independent architectural review, under JC's approval of the policy change and technical-acceptance delegation with final veto retained, affirmatively adopts `HAL-R024-R028-HAL370-BACKEND-TRUST-S1-COORDINATED-ADOPTION-01`. The selected historical source is archived outside the active source tree, SHA-256 `0269fe6a3d8ec3021aea6b16cb5b355d6e75533a14551eb26b3d80034a1c5519` (84,056 bytes/286 lines); the original candidate remains historical DRAFT/nonbinding material. Its coordinated operative contract is now **Adopted / Binding [S1]** in R024/R028/HAL370: rigorous per-candidate source/host checks, bounded available observable backend comparisons and versioned input-domain/capability-envelope acceptance, with exact evidence provenance kept separate. Earlier decisions/receipts retain their original scope; no shader, source, runtime or test changes are executed, no S2 engineering is granted, no earlier S2 acceptance is broadened and no new Device or original external-consumer success is claimed. The unchanged original three-asset external-consumer conformance fixture remains reported RED at triangle63 before N1 FP16/Image, N4 not reached; **S3/S4 remain CLOSED**. Additional comparator bounds and backend-envelope acceptance require their separate authorized evidence/decisions.

**Binding real-time numeric proof-boundary reform adoption — 2026-11-03:** Independent architectural review under HAL370 §§1/7 and JC's express reform mandate with continuing reserved constitutional/product veto affirmatively adopts `HAL-R024-R028-HAL370-REALTIME-PROOF-S1-COORDINATED-ADOPTION-01`. The accepted source is the complete `HAL-R024-R028-HAL370-REALTIME-NUMERIC-PROOF-REFORM-01` at revision `8d449eeb` (60 R024, 66 R028 and 74 HAL370 added lines). Its coordinated 2026-11-03 clauses are now **Adopted / Binding [S1]**, effective 2026-11-03. They remove mandatory production per-FOV proof-only O(triangle/vertex) CPU simulation of GPU transform/clip/determinant/reciprocal-w/helper/UV/LOD/footprint numerics and equivalent renamed frame certificates. Original source certificates and exact still-live source/Context/generation/lease reuse, actual submitted host calculations, current target/Device/capabilities/finite custody, actual R025 shadows/cascades/depth/XY/PCF, R027 FP16/Image, R028 §12 and the 2026-11-01 source/backend split remain binding. Only the expressly inconsistent proof-only per-FOV obligation is superseded; older unrelated S1 text and receipts are unchanged. This is no constitutional amendment, S2 engineering grant, implementation/GPU/native result or S3/S4 activation; **S3/S4 remain CLOSED**.

## 1. Purpose, status and version boundary

**PbrTextureMipSamplingF32V2** is a new opt-in SAMPLING profile paired with EXACT R-024 **PbrMetallicRoughnessRealtimeF32V1 material EVALUATOR**. V2 is not a new BRDF. It extends only qualified2D material content/sampling, under the exact S1 R-024/HAL-355 versioned admission coordinated by HAL-370.

R-024 §8.2.3 retains V1's one-mip nearest/linear single-level sampling; V2 retains its distinct complete-chain sampler policy. BOTH use the coordinated 2026-11-01 source qualification: explicit A/B/C mapped frame or genuine-absence normal proof under R-024 §7.1, and the exact post-scale common-UV normal-domain law in §6. Original role bytes, LOCAL GS before dual L^-T/L, PBR52, Material64, controlled evaluator and r_eff=max(source_r,.045) remain unchanged. No automatic V1 migration or shader/filter/BRDF change follows.

R-025/026 group3 and R-027 HDR/tone/coverage/linear-FP16 output stay separate and unchanged. V2 does not generate mips for their depth maps, E cubes/LUT or scene/final textures. Unselected V2 adds zero resource/work/idle demand.

Every SHALL/MUST below is binding at S1 in [HAL-370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md)'s exact opt-in scope and HAL-355 §17.1 acknowledgment, including the dated Revision02 adoption above. Earlier private S2 evidence remains accepted over its tested old domain only. This source-domain adoption grants no production implementation, new engineering scope, expanded-input Device proof, public S3 or native S4.

## 2. Historical baseline and continuing sovereign owners

The following source observations preserve the original authoring baseline. They do not override the header's later accepted old-domain private implementation or imply support for the widened source domain.

| Concern | Existing fact / owner | S1 V2 boundary |
|---|---|---|
| Texture data | content.rs:58–133: Rgba8Unorm or Rgba8UnormSrgb; one width/height/texels; exact one-level byte validation | New complete typed chain admission, not a claim Texture3D already stores levels. |
| Sampler | content.rs:136–218: U/V, Nearest/Linear min/mag | Source-resolved independent MAG/within-level MIN/between-level mode; no silent LINEAR_CLAMP default. |
| GPU upload | context.rs:2590–2654: bounded upload, mip_level_count1, write level0 | Future bounded reservation/upload of every supplied level on existing Queue. |
| Sampler materialization | context.rs:2696–2706: mipmap_filter tied to min_filter | Future exact6-mode mapping/LOD proof; today's descriptor cannot represent the selected independent semantics. |
| Shader/layout | raster.rs:387–456: legacy three-group layout; R-024 PBR is S1 adopted but not implemented | Distinct controlled V2 sampling implementation, same five role textures/samplers and material evaluator. |
| Source meaning | Application/independent producer | Own decoding, encoding/roles/UV/sampler/generator provenance, complete CPU generation and tile-seam claims. |
| GPU resources/front | Context3D/per-view/HAL-266A/295A, with HAL-263A final Image | Mechanical validation, complete leases, exact target and one same-Queue publication; no semantic writeback. |

At that original baseline, enum values, mipmap_filter or mip0 uploads did not supply V2. Subsequent accepted private full-chain/gradient implementation is scoped to its tested old inputs; neither its existence nor this new law proves expanded normal-source/frame numerical safety.

HAL-002/002A retain IDs, ownership, immutable values, phase separation and sovereignty. No raw resource, application WGSL, private GPU writer, global scene or second renderer is exposed.

## 3. Qualified roles, source and immutable identity

Only R-024's PRESENT2D material roles are in scope:

| Role | Exact source/storage semantics | Generation rule | Sample/evaluator use |
|---|---|---|---|
| Base color | Straight sRGB RGB, linear numeric A; Rgba8UnormSrgb | §5 linear-light RGB; Opaque straight average or Mask alpha-weighted straight average; A independently averaged | Decode once before spatial filtering; apply R-024 factor/vertex color and Opaque/Mask policy afterward. |
| Tangent normal | Numeric signed XYZ encoded in Rgba8Unorm RGB, OpenGL+Y; unused A | §6 stored-parent component first moments and complete post-scale common-UV certificate; all-level BLUE>=128 is sufficient; no normalization/scale during generation | Numeric sampling, source X/Y normal scale then guarded normalization/world TBN under R-024. |
| Metallic-roughness | Rgba8Unorm numeric; G roughness,B metallic | §5.3 independent raw channel averages | Preserve R-024 factors/channel roles and exact r_eff policy. |
| Occlusion | Rgba8Unorm numeric; R AO | Same raw channel rule | Preserve R-024 AO scope; no GI or direct-light inference. |
| Emissive | sRGB RGB in Rgba8UnormSrgb; A unused downstream | Linear-light RGB average; numeric A average | Decode once; R-024 emission factor/use, never an implicit light. |

Each present role SHALL bind actual finite, source-qualified UV0, original image/content identity, role/format/encoding, extent, U/V addressing, MAG/MIN policy, producer/generator revision and complete level content. Base-color chain qualification explicitly identifies Opaque versus Mask generation semantics: identical level0 bytes alone do not certify compatible derived chains. Baseline fallback UV(0,0), guessed image orientation or a shared texture name is insufficient.

Normal roles additionally bind §6's exact source/chain/sampler/scale cell certificate, finite coverage/work/digest and R-024's declared original-versus-render frame interpretation. Mode C retains all original source normals/Mikk/UV/corner provenance and uses an explicitly derived shading frame; it does not modify the normal image, levels, role, scale or filter. Interpretation/certificate/packed-data changes require their owning immutable resource/material/structural/extension successors and complete leases.

UV1, texture transforms, atlas indirection, new source packing, compressed formats or extra attributes require separate authority. Material input never enters Image premultiplication/normalization; no consumer semantics follow from pixels.

Genuine optional-role absence retains exact R-024 defaults/presence flags and bounded structural costs. Present invalid, stale, unsupported or incomplete content never receives an absent-role default. Sharing requires exact compatible content/encoding/generation/UV/filter use and charged leases; role semantics remain independent.

## 4. Complete chain and exact area filter

### 4.1 Dimensions, termination and bytes

For an admitted level0 integer w0,h0>=1:

```text
w_(l+1) = max(1,floor(w_l/2))
h_(l+1) = max(1,floor(h_l/2))
stop when w_l=h_l=1
RGBA8_logical_bytes = 4*sum_l(w_l*h_l)
```

A mip-using MIN requires every level through the unique terminal1×1, in order, with exact format, dimensions and byte lengths. No missing/partial levels, duplicate terminal1×1, hidden rescale or power-of-two conversion. Authored no-mip modes may legitimately use a single level (§7), without claiming mip quality.

| Chain | Exact level dimensions / count | Texels / bytes |
|---|---|---|
|3×5 NPOT |3×5 ->1×2 ->1×1;3 levels |15+2+1=18;72 bytes |
|1024² POT |1024,512,256,128,64,32,16,8,4,2,1 on each axis;11 levels |sum_(j=0..10)4^j=(4^11-1)/3=1,398,101;5,592,404 bytes |
|1024² level0 only | One1024² level |1,048,576 texels;4,194,304 bytes |
|1×8 strip |1×8 ->1×4 ->1×2 ->1×1 |15 texels;60 bytes; a4/3 level0 estimate is insufficient |
| Five separate full1024² roles | Five independent complete chains | At least27,962,020 logical bytes, before all overhead/other resources |

All products, sums, offsets and conversions are checked finite arithmetic. Driver alignment, row padding and actual staging costs are separate charges, not hidden by logical byte counts.

### 4.2 Exact area weights, including odd dimensions

A parent's texel(i,j) is a piecewise-constant field over [i/w,(i+1)/w)×[j/h,(j+1)/h). Child(x,y) covers [x/w',(x+1)/w')×[y/h',(y+1)/h') in normalized UV. For each intersected parent texel define:

```text
dx_i = max(0,min((i+1)/w,(x+1)/w') - max(i/w,x/w'))
dy_j = max(0,min((j+1)/h,(y+1)/h') - max(j/h,y/h'))
weight_ij = dx_i*dy_j / (1/(w'*h'))
child_value = sum_ij(weight_ij*parent_value_ij)
```

Weights are nonnegative and sum exactly1 in ideal arithmetic because the parent's cells partition the child's rectangle. Apply recursively to the PREVIOUS STORED LEVEL with the role's decode/filter/encode rule, not an unquantized hidden history.

This is not a blind2×2 decimator: odd edge texels contribute. For3×5 ->1×2, every x column contributes1/3. The first child row's parent-y weights are(2/5,2/5,1/5,0,0); the second's are(0,0,1/5,2/5,2/5). Subsequent1×1 averages those two child rows. No edge is silently dropped.

U/V wrap modes govern sampling, not an invented mip edge correction. A producer claiming repeatable quality under Repeat/MirrorRepeat separately qualifies the source tile seams and generated chain; the engine does not repair a seam by guessed wrapping during generation.

## 5. Role-correct color, alpha and numeric generation

### 5.1 Linear-light sRGB color

For a parent byte b, normalized s=b/255:

```text
decode_srgb(s) = s/12.92                         if s<=.04045
                 ((s+.055)/1.055)^2.4          otherwise

encode_srgb(x) = 12.92*x                       if x<=.0031308
                 1.055*x^(1/2.4)-.055         otherwise

stored_child_byte = round_nearest_ties_even(255*encode_srgb(child_linear))
```

For all stored color/numeric channels, nearest-ties-even is explicit: for p in[0,255], n=floor(p), f=p-n; choose n for f<.5, n+1 for f>.5, and the even member of(n,n+1) for f=.5. No host-default rounding mode is assumed; actual finite arithmetic/error still needs qualification.

Base/emissive RGB decode BEFORE area averaging and encode ONCE afterward. Rgba8UnormSrgb sampling decodes once before bilinear/trilinear filtering. Never average encoded sRGB bytes, double decode or apply source gamma after filtering.

Base A is numeric/linear and independently area-averaged, stored as nearest-ties-even RGBA8. Emissive A is similarly averaged but remains unused downstream; it cannot become opacity or transport data.

Opaque base RGB uses straight linear area average even though material alpha is ignored for visibility. Factors and vertex alpha are not baked into these source levels; they remain R-024 sample/evaluation inputs.

### 5.2 Mask base and the explicit silhouette limit

For Mask base RGB, with area weights w_i, parent numeric alpha a_i and decoded linear color c_i:

```text
a_child = sum_i(w_i*a_i)
if sum_i(w_i*a_i)>0:
    c_child = sum_i(w_i*a_i*c_i)/sum_i(w_i*a_i)
otherwise:
    c_child = (0,0,0)
```

Encode c_child as STRAIGHT sRGB. Store a_child independently as linear numeric alpha. This is an explicit source filter, not stored premultiplied Image content or a new semantic alpha derived from RGB.

R-024 applies base factor/vertex alpha at sampling, preserves its original cutoff/discard and writes alpha1 for surviving samples. The filter DOES NOT guarantee exact alpha-test coverage/silhouette across LOD, particularly with varying factors/vertex alpha.

No covert alpha rescale, cutoff change, dither, alpha-to-coverage, geometry expansion or product-level coverage guarantee is admitted. A Mask silhouette claim needs source-qualified specific native evidence and JC observation. Opaque quality acceptance is independent of Mask failure/success.

### 5.3 Metallic-roughness, AO and raw numeric channels

For each normalized numeric parent channel b/255, area-average independently and store round_nearest_ties_even(255*average). Average all four raw channels; MR still consumes G/B and AO still consumes R, with the others unused by that role. No sRGB transform applies.

An exactly qualified packed numeric source/chain may share residency between MR and AO while retaining independent role/sampler associations. Incompatible generation, role encoding or filter interpretation cannot share by byte resemblance. No implicit channel repack, color-to-data substitution, source roughness change or factor baking is allowed.

## 6. Normal chain, correlated-domain certificate and quality limits

**Adopted / Binding [S1] — 2026-11-03: qualification of §6, including §§6.3/6.5 “per-candidate” verification.** All original stored bytes, first-moment generation, source scale, complete correlated common-UV/LOD/address coverage, witnesses, positive source bounds and old sufficient routes SHALL remain unchanged and fully checked when their actual immutable dependencies change. A valid still-live source certificate may be reused with exact source/Context/generation/lease association; FOV alone SHALL NOT rescan its texels, cells or triangles.

This source-domain certificate covers its declared complete source/filter domain. It SHALL NOT acquire a new mandatory camera-specific clip, screen-determinant, helper-q or footprint certificate. Source derivation/certificate arithmetic remains rigorous; actual GPU filtering/derivative consequences use R024 §8.2.4's adopted 2026-11-03 boundary and the unchanged observable quality gates.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

### 6.1 Unchanged original bytes, first moments and source scale

Normal maps SHALL remain original numeric Rgba8Unorm RGB, signed OpenGL+Y. Alpha remains unused downstream. For stored bytes `(R,G,B)` and the original finite material normal scale `s in [−1024,1024]`, define the exact ideal scaled texel vector:

```text
q_k(s) = (s*(2R/255-1), s*(2G/255-1), 2B/255-1)
```

R-028's recursive area weights over the PREVIOUS STORED level, unnormalized signed component FIRST MOMENTS, numeric unused-alpha average and nearest-ties-even reencoding SHALL remain unchanged. No parent or child normalization, normal-scale baking, BLUE clamp, JPEG edit, role deletion, variance correction or roughness compensation is permitted. All provided levels still obey their complete source/format/dimension/derivation law. V1 remains one-level; V2 retains its explicit chain/sampler family and all six MIN policies with independent MAG/U/V and anisotropy1.

Reencode each signed component with nearest-ties-even rounding of255*(q+1)/2, and average unused alpha numerically. Every supplied stored level remains checked for exact dimensions/format/bytes/provenance. Negative BLUE alone is no longer universal rejection; the complete nonzero source certificate below is mandatory unless the sufficient all-level positive-Z route applies. No original JPEG, byte, mip recipe or sampler is repaired.

### 6.2 Complete finite shared-UV/filter domain

For the exact captured chain, sampler and original scale, the producer SHALL partition EVERY reachable ideal normal-sampling domain into finite exact common-UV cells. This covers the selected MIN mode, independent MAG, U/V address modes, admitted footprints/LOD and all boundary cases. It is not limited to a camera, observed fragments, a vertex UV list, sampled LODs or a finite test grid.

For a selected within-level linear region, the addressed neighbor indices are fixed and the interpolation coordinate on each axis is affine in the original u or v. At adjacent mip levels, use the UNION of both levels' axis breakpoints, including half-texel neighbor changes, so the same original `(u,v)` belongs to one joint cell at BOTH levels. NPOT grids need not align. Repeat and MirrorRepeat use their actual finite periodic fundamental domains and one-sided seam/reflection cells; ClampToEdge includes constant tails. Negative UVs, endpoint aliases, nearest discontinuities/ties, MAG/MIN selection, mip-nearest transitions and the last-level LOD endpoint SHALL all be covered. If an exact tie permits multiple outcomes under the qualified sampler contract, each permitted outcome must be certified. Width/height1 and clamped/aliased neighbors retain their actual lower-dimensional meaning.

This construction is finite for each captured finite chain/sampler: a finite joint axis partition and finite base/level/adjacent-level branches, with periodic/tail reductions justified by the exact address law. Additional deterministic finite subdivision may refine cells, provided the complete original domain and boundaries remain covered. The producer SHALL declare and charge work/storage limits, algorithm/revision, cell/branch counts, exact subdivision/coverage data and certificate digest. Exhausted budget, overflow, unresolved boxes or an omitted reachable region is refusal. No unbounded optimizer, preparation-time search or runtime producer is selected.

The rejected earlier predicate allowed eight contributing RAW texels to vary with arbitrary independent convex weights. Actual trilinear sampling does not have those freedoms: both levels use the SAME u,v, and only the true adjacent-level interpolation is independently variable. That raw hull can contain zero even when the reachable correlated response never does. A raw-texel separator may remain a SUFFICIENT shortcut, since it bounds every actual convex response; it SHALL NOT remain a necessary global gate.

### 6.3 Real domain-corner certificate and Bernstein proof

For a joint cell `C=[u0,u1]×[v0,v1]` and an actually adjacent level pair `(l,l+1)`, let `P_l(u,v;s)` and `P_(l+1)(u,v;s)` be the ACTUAL IDEAL bilinear responses decoded from their supplied STORED numeric bytes and post-multiplied by the original X/Y scale s. Fixed addressed neighbors and the joint partition make both bilinear in the SAME u,v. For fractional LOD `rho in [0,1]`, define:

```text
q(u,v,rho;s) = (1-rho)*P_l(u,v;s) + rho*P_(l+1)(u,v;s)
alpha = (u-u0)/(u1-u0), beta = (v-v0)/(v1-v0)
B_0(x) = 1-x; B_1(x) = x
Q_abc = q(u_a,v_b,rho_c;s),  a,b,c in {0,1}
q(u,v,rho;s)
  = sum_(a,b,c) B_a(alpha)*B_b(beta)*B_c(rho)*Q_abc
```

Here `rho_0=0,rho_1=1` for a complete adjacent-level interval; on a refined LOD subinterval the Bernstein coordinate is affinely rescaled and Q uses its ACTUAL endpoint LODs. Degenerate dimensions are reduced rather than divided by zero. The eight Q are REAL DOMAIN-CORNER RESPONSE VECTORS, not the eight arbitrary original texels. A corner at rho0 is the bilinear value at that original UV in level l, and at rho1 the value at the SAME UV in level l+1; these may already be mixtures of different stored texels. Never independently rechoose the UV of the two levels.

The ideal polynomial is separately affine in alpha/beta/fractional LOD with nonnegative Bernstein weights summing1. This is not affine GPU log-footprint or zero filter error. Complete ideal source-domain certificates remain rigorous for each candidate; actual derivative/LOD/filter consequences use R024 §8.2.4 predeclared genuine probe/output comparisons for the accepted input domain/backend envelope, not hidden-intermediate observations.

For EVERY final certified cell the producer SHALL supply a mathematical UNIT witness h_C, with its outward representation enclosure, such that:

```text
delta_Q = 1/510
lower_outward(h_C · Q_abc) >= delta_Q  for EVERY real domain corner
```

Different cells may use different witnesses. Taking the projection of the exact response identity proves:

```text
h_C · q(u,v,rho;s)
  >= delta_Q * sum_(a,b,c) B_a(alpha)*B_b(beta)*B_c(rho)
  = delta_Q
|q(u,v,rho;s)| >= delta_Q > 0
```

The captured finite coverage and inequalities, not a Boolean or digest alone, SHALL be independently checked before target writes. Record the exact source/level bytes, cell bounds, addressing/LOD branch, sampler, original scale, witness/error, outward lower bounds and work/digest. Failed certification identifies the actual level pair/cell/boundary/neighbor response and enclosure; a failed attempted separator is not automatically a proof of an actual zero. A source may be certified by a complete bounded refined partition, but an unresolved region cannot be omitted or converted to absence.

The corresponding actual corner structures for the selected modes are:

| Source-selected filter branch | Domain certificate structure |
|---|---|
| MAG Nearest; MIN Nearest (no mip) | One constant response per actual nearest-address region at level0. |
| MAG Linear; MIN Linear (no mip); V1 within-level Linear | Four actual bilinear domain-corner responses at level0; no mip manufactured. |
| NearestMipNearest | One constant response per selected level/address region; certify both sides and actual ties of level selection. |
| LinearMipNearest | Four bilinear response corners per selected-level cell, with level-selection boundaries covered. |
| NearestMipLinear | Two actual adjacent-level endpoint responses per joint nearest-region cell, with the SAME UV and every nearest jump/tie covered. |
| LinearMipLinear | Eight actual common-UV/LOD domain-corner responses as above, including refined/degenerate cells and last-level endpoints. |

Clamp aliases and1-wide/1-high levels may reduce effective corner count; they do not remove coverage obligations. V1's declared single-level nearest/linear/address law uses the corresponding one/four-corner form. V2 still requires its authored complete chain for mip MIN and its genuine no-mip semantics otherwise.

All quantities are CPU source certificate facts; none is a per-pixel GPU parameter, extra binding or sampled certificate texture. The shader still calls the original source sampler at original UV, scales sampled X/Y, normalizes q and uses the existing qualified world TBN. Ideal orthonormal mapping and double-sided reversal preserve nonzero. No shader normal repair, additional pass, fallback or sample-frequency invocation follows from this proof.

### 6.4 Preserved old positive-Z route and exact source failures

For the original all-level BLUE>=128 domain, each supplied decoded texel has `q_k.z>=1/255`, unaffected by scale. Every within-level or adjacent-level REAL DOMAIN-CORNER response is a convex combination of these texels, so choosing `h_C=+Z` gives `h_C·Q>=1/255>1/510` for every cell, address branch and boundary. The old first-moment/monotone-rounding induction preserves BLUE>=128 recursively. Thus every previously qualified chain/sampler/scale remains eligible without changing bytes, mip recipe, source generation or filtering semantics.

Any previous valid arbitrary-raw-texel separator is also a sufficient proof of this corner law: each real Q is a convex combination of those raw post-scale q_k. The converse is not required. As a LOCAL algebraic illustration, opposite raw vectors may have a hull through zero, while an actual restricted cell uses only mixing weights in `[0,1/4]` and stays in a strict half-space. The real correlated response and its entire domain, not arbitrary weights or this illustration alone, decide eligibility. The five supplied sofa cases in §6.5 are attributable source evidence of the practical distinction; the certificate still must bind their actual cells and witnesses.

BLUE<128 alone is neither success nor refusal under this binding law. A constant stored `(255,128,127)` at scale1 gives `q=(1,1/255,−1/255)` and h=+X certifies every cell without a byte change. Conversely, scale0 and an ACTUALLY REACHABLE bilinear/trilinear equal mixture of BLUE127 and BLUE128 yields `q=(0,0,0)`, which no positive certificate can admit. Merely listing those two texels in an unconstrained hull does not prove the equal mixture is reachable under shared UV. Record the exact UV/LOD/branch when asserting a true zero; otherwise report only the failed proof. The engine SHALL NOT change source, scale, sampler, normal role or mip content to obtain a certificate.

The original recursive sufficient-route induction remains:

```text
parent_blue>=128
 -> parent_qz>=2*128/255-1=1/255
 -> sum_i(w_i*parent_qz)>=1/255
 -> 255*(averaged_qz+1)/2>=128
 -> nearest_ties_even(encoded_blue)>=128
```

Nearest rounding is monotone; integer128 is exactly representable. Recursion preserves the bound at every level. Numeric within-level filtering and between-level linear interpolation are convex combinations of valid samples, so ideal sampled q.z>=1/255 and q!=0.

### 6.5 Attributable furniture feasibility and separate numeric gate

The independently supplied external-consumer source revision `d4408a1`, `src/tests/pbr_source.rs` SHA-256 `0609229f57e33db247e5b69d692dc1b5dc94d5594a0aa650b815cd403a3bc327`, reports4 original normal roles with correct complete11-level first-moment chains. Five sofa supports fail the arbitrary raw-eight-texel hull but certify under ACTUAL shared-UV MAG Linear/MIN LinearMipLinear, Repeat/Repeat and scale1, with outward norm lower bounds [.10392156862745085,.10980392156862731,.14509803921568606,.0030084210845814116,.17843137254901936], no unresolved boxes. Other supports retain +Z or sufficient raw-hull success. Original sofa BLUE126 at(755,42) and chair-legs BLUE124 at(79,28) remain unchanged.

The smallest supplied bound exceeds1/510. These are attributed SOURCE numerical findings, not an execution by this adoption, accepted certificate ingestion, a completed C buffer or new shader/Metal proof. Actual cell/level/sampler/scale/witness/coverage records must accompany future admission; a reported norm bound alone is not the chosen cell's separator record. The source SHALL disclose any intentionally derived C shading interpretation separately from original smooth/Mikk data.

R024 §8.2.4 separates rigorous packed/source-corner validation from representative bounded observable N1/N4 conformance for the declared input domain/backend envelope. Ideal1/510 or old1/255 is not GPU precision proof. R028 .03/.05 apply only to their defined sampling/temporal metrics, never TBN/normal/GS; an essential additional numerical comparator stays OPEN until the delegated architectural reviewer selects its bound before separately authorized S2 execution. Compare only available genuine probe outputs, final FP16/Image or coverage/Device results; hidden normalization/TBN/helper values are not directly measured. Complete source cells/scale/sampler/certificates remain checked per candidate; in-domain changes require no per-frame S3 replay. No hint, fallback, byte repair or role erasure follows.

This FIRST-MOMENT filter does not preserve normal variance or certify specular antialiasing. No Toksvig, roughness adjustment, alternate lobe or new BRDF follows. r_eff=max(source_r,.045) remains EXACT V1 material policy even when source normals are mipped.

## 7. Explicit source-selected sampler contract

Each role owns U/V ClampToEdge, Repeat or MirrorRepeat and independent MAG Nearest or Linear. Source producer resolves omitted original sampler/defaults under the actual authoritative input format; Halcyon never invents one, substitutes LINEAR_CLAMP or silently upgrades Nearest.

Within-level MIN and between-level selection are distinct from MAG:

| Source MIN | Within-level minification | Between-level behavior |
|---|---|---|
| Nearest | Nearest | No mip sampling; authored base-level semantics |
| Linear | Linear | No mip sampling; authored base-level semantics |
| NearestMipNearest | Nearest | Nearest mip |
| LinearMipNearest | Linear | Nearest mip |
| NearestMipLinear | Nearest | Linear between adjacent mips |
| LinearMipLinear | Linear | Linear between adjacent mips |

No-mip source semantics are deliberate, not a fallback for a mip-requesting sampler. A single-level chain legitimately supports those two modes. A mip MIN requires the full chain and access through the last valid level L; max LOD is L, no out-of-chain sample or forced LOD0 replacing the authored mip behavior. The concrete pinned lowering must also preserve no-mip semantics if resident content has additional levels; a sampler label or linked min/mipmap fields is insufficient.

Anisotropy is fixed1. No anisotropic quality, hidden LOD bias, arbitrary sampler override or authored wrap substitution is admitted. Actual capability and selected min/mag/LOD consequences require controlled source/Device checks and R024 §8.2.4 representative observable tests for the declared domain/envelope; neither enum labels, unsupplied filter bounds nor frozen fixture/driver metadata are runtime admission authority.

## 8. Controlled derivatives and per-role explicit sampling

**Adopted / Binding [S1] — 2026-11-03: replacement of §8's production qualification obligation.** The controlled V2 program SHALL still compute center-UV dpdx/dpdy in derivative-valid fragment control flow before nonuniform role branches/Mask discard and use each present role's original explicit-gradient sampling, sampler and extent. Source UVs/roles/formats/extents/certificates remain valid immutable inputs. Check actual submitted host matrices/camera data, current target, Context/Device/capabilities and finite resources at their owning changed-input boundaries.

GPU clipping, interpolation, reciprocal-w/helper behavior, derivatives, LOD and filtering SHALL be qualified for the declared versioned S3 source/backend envelope through controlled program/IR validation, actual Device support and representative predeclared observable N1/N4 conformance. Required near/helper-sensitive, partial/clipped, UV/LOD, Mask, A/B/C and shadow output cases use honest authorized comparisons. They are not per-frame readback or direct measurement of hidden GPU arithmetic.

No production FOV candidate SHALL require walking every vertex/triangle solely for modeled packed object/projection dots, outward clip/half-space propagation, screen determinant/conditioning, a two-pixel helper reciprocal-w lower bound, UV-plane construction or footprint bounds. This applies to the main color pass and UV-only requalification for each active R025 shadow face. It also excludes a renamed/frame-local certificate that performs the same prediction. Current implementations such as `qualify_uv_domain`, repeated `qualify_material_uvs` and proof-only `qualify_packet_projection` describe the affected work, not a newly authorized source edit.

Actual R025 source/caster/receiver geometry, cascade fitting, depth/XY/PCF coverage, Mask source semantics and current map rendering remain required under R025. Original source validity, truly invalid/nonfinite host data or arithmetic, stale ownership, exhausted limits, invalid controlled derivative flow, unsupported capabilities and actual observed failures retain their real refusal/acceptance consequences. A failed or inconclusive GPU model alone is not malformed-source proof or mandatory runtime work. No guessed gradient/epsilon, role loss, normal repair, sampler/shader change or unqualified backend success follows.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

The DISTINCT controlled V2 fragment implementation SHALL compute dFdx(uv0),dFdy(uv0) in derivative-valid fragment control flow BEFORE nonuniform role-presence branches or Mask discard. Here dFdx/dFdy denote the corresponding WGSL dpdx/dpdy derivative operations. Those derivatives feed explicit textureSampleGrad-equivalent sampling for each present role with its own qualified sampler and actual role extent.

Each candidate must pass source UV/sampler/format/extent/host-representability checks, derivative-valid IR and actual capability checks, with lawful immutable checked-capture reuse. Exact host packed-clip intervals prove only their modeled host predicates, not GPU vertex/clip precision. Known-invalid data/control flow or unsupported capability refuses; an inconclusive modeled GPU enclosure is not itself invalid source. Separately authorized S2 predeclares representative N1/N4 helper-sensitive gradient/LOD/large-footprint/clipping fixtures, predictions and available observable comparisons. S3 accepts the declared input domain/backend envelope from these and the original three-asset external-consumer conformance fixture; valid dynamic source/view/target changes need no per-frame replay. Final pixels do not directly expose helper execution or hidden arithmetic; missing discrimination is UNVERIFIED and essential new comparator bounds await selection by the delegated architectural reviewer before execution. No guessed derivative, forced mip/color, role drop, CPU fragment error, readback repair or original-oracle input alteration is admitted. Actual failures remain failures.

sRGB decode precedes spatial filtering for base/emissive RGB; numeric normal/MR/AO NEVER color-decode. The controlled program preserves all R-024 factor/mask/material operations after sampling. No application-supplied shader, extra binding group, raw GPU entry or implicit texture transform is introduced.

This section specifies required semantics and proof, not a current Naga module or API. No support is claimed from newer WGSL documentation or the existence of textureSampleGrad alone.

## 9. Complete-chain leases, ABI, finite cost and source-to-target transaction

**Adopted / Binding [S1] — 2026-11-03: qualification of §9's complete validation and cost.** Original complete source/chain/certificate checks SHALL run on real dependency changes and be reused only with exact still-live captured source/Context/generations/leases. Warm FOV does not repeat immutable scans. Each changed frame still checks actual submitted host data and required actual host work, target/sample/format/capabilities, budgets and complete requested/committed association.

The existing source→Update→target preparation→necessary upload/render→same-Queue FP16/Image commit sequence, actual retained/old/in-flight charges and no-use rules remain. “Complete validation” SHALL NOT require the proof-only GPU-prediction sweeps retired by the §8 adopted reform, nor charge a mandatory replacement frame certificate. Source validity and actual R025 preparation remain necessary work; unchanged source uploads and unchanged complete frames remain zero-work as already required.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

One typed Context3D identity/generation/lease binds one COMPLETE qualified level chain and its immutable source/format/content/generator provenance. It is not a collection of independently mutable mip fronts. Present role/sampler/chain changes create actual structural content successors; no partial replacement or relabel.

All level bytes, row padding, aligned uploads/staging, CPU retained data, actual views/samplers/bind groups, cached pipeline variants, source generations, current and still-leased older/pending/in-flight fronts are counted and reserved before publication or candidate writes. Logical RGBA8 cost is exactly4*sum(w_l*h_l), not level0 bytes or an assumed4/3 approximation for arbitrary NPOT content.

For1024², full-chain overhead is5,592,404-4,194,304=1,398,100 bytes per distinct role; five separate chains cost at least27,962,020 logical bytes. Sharing qualified immutable content may avoid duplicate residency, not erase actual leases or other role ownership. Device dimension/format/layer/offset/binding constraints and finite effective HAL-355 budgets remain controlling; no guessed aggregate cap.

Group2 remains exactly five textures/five samplers plus Material64, with View80/Object112 and the52-byte PBR stream unchanged. Profile identity distinguishes V2 while BRDF/evaluator stays V1. R-025/026 group3 and R-027 output identities are unmodified; no fifth group, new Device/Queue, offscreen publisher or 2D hot-path branch.

The independent producer finishes the complete CPU chain before immutable source admission. It supplies original image/content/role/UV/sampler/generator revision and derivation evidence. Future context admission validates the complete descriptor and reserves/uploads each level on the existing Device/Queue; preparation never generates mips or calls a source decoder.

The complete association spans chain/sampler/material/geometry/RenderSnapshot/ModelState/view/profile/source/context generations, actual HAL-266A target metrics/lifetime, dependencies and final front. View/model-only changes may reuse compatible resident chains; source/mip/sampler changes require a true new structural association. Reuse never suppresses actual source invalidation.

The existing ordered path remains:

```text
producer complete chain + source proof
 -> lawful immutable owner Update/publication
 -> Context3D/admitted target preparation, complete validation and reservation
 -> checked upload of every required level on existing Queue
 -> admitted material/direct/E/G scene rendering, if separately enabled
 -> unchanged admitted R-027 tone/coverage/FP16 final
 -> same-Queue enqueue/submit, HAL-295A swap/registration
 -> atomic complete front/committed association
```

No background mip rebuild, extra Update/preparation, hidden timer, semantic GPU writeback, independent material front or Image normalization of material inputs. Compatible last-good keeps ORIGINAL chain/source/sampler/material/view/target identity; incompatible/retired target withdraws. Leases and generations retire only through actual existing owners after all use; no eviction of still-leased old content or frame-index resurrection.

Unused V2/no-context/no-view/inactive targets add no specialized chain allocation/growth/upload/pass/job/idle demand; necessary retirement still occurs. An inactive request cannot silently downres, truncate mips, lose a role or change sampler/material to pass a budget.

## 10. Typed refusal and truthful supported scope

**Adopted / Binding [S1] — 2026-11-03: replacement of the derivative/host-numeric refusal interpretation in §10.** Invalid original UV/source/certificate/provenance, nonfinite or invalid actual submitted host values/operations, stale generations/leases, exhausted limits and unsupported actual capabilities remain attributable typed refusals before writes. Invalid controlled derivative flow and an observed failing supported backend retain their exact program/backend consequences; an affected envelope cannot silently remain accepted.

A predicted clip, screen determinant, helper reciprocal-w or footprint interval that is inconclusive or negative solely in a hypothetical GPU precision model SHALL NOT itself classify the original source as invalid, require the model to run every frame, or invent a CPU fragment failure. This changes no original source-domain inequality or error for an actual host operation. Preserve the prior complete compatible last-good and withdraw incompatible/retired targets under the existing owners.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

| Intended-path cause | Required outcome |
|---|---|
| Missing/partial/wrong-dimension/duplicate-terminal chain for a mip MIN | Incomplete/invalid chain refusal, not implicit generation or shorter success. |
| Role/format/encoding/UV/sampler provenance mismatch | Attributable source/representation refusal; no color/data/Image substitution. |
| Neither all-level BLUE sufficient proof nor complete valid correlated-domain certificate; wrong/stale source/scale/sampler/role association or changed generation semantics | Attributable normal/profile-source refusal with exact cell/level/branch cause. Negative BLUE or failed arbitrary raw hull alone is not refusal; no normalize/filter/byte/role/fallback repair. |
| Known invalid derivative/control-flow, unsupported Device/format/LOD lowering, or missing/failed required observed backend conformance. | Exact typed source/host/capability refusal before candidate writes, or unqualified backend scope as appropriate to the actual cause. An inconclusive undocumented GPU error enclosure alone is not malformed-source proof. Neither uncertainty nor failure authorizes runtime NaN repair or fabricated CPU fragment authority. |
| Stale/cross-context/retired chain or sampler lease | Distinct identity/lifetime failure; no default role or current-front relabel. |
| Exhausted chain/upload/residency/in-flight/variant/work capacity | Budget refusal before writes; no truncated/downsampled/evicted-required fallback. |
| Target inactive/incompatible/retired or device lifetime ended | Existing target/front withdrawal and resource retirement; no stale sample or private recovery. |
| Mask coverage/native appearance not proved | Quality claim remains UNVERIFIED; logical source/mask semantics are not silently altered. |
| Missing GPU timer/native/perceptual evidence | Corresponding acceptance gate UNVERIFIED, not inferred from CPU timing/FPS. |

These are required cause distinctions, not invented Rust enum/API signatures. Negative checks are bounded to reachable/conflatable intended V2 ingress and preservation of actual source/front truth; they do not create an unsupported-family test program.

## 11. Worked numeric and conformance cases

**Adopted / Binding [S1] — 2026-11-03: qualification of §11 case 8.** The derivative-before-discard and helper-sensitive output distinctions remain predeclared offline/backend conformance cases. Classify genuinely invalid source and actual host data at their own boundaries; do not require a per-frame vertex/triangle GPU-precision simulation for those cases. All source-byte/filter examples, the multiscale fixture and §12 quality thresholds remain unchanged. An offline host model may explain a case but is not observed GPU execution or production admission.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

These are documentary ideal oracles, not tests executed by this documentary adoption. Source/generator/representation and actual GPU stages remain distinct.

1. **Odd3×5 area integration.** Use the same numeric row value in each of3 columns. For source rows(0,0,0,0,1), the two1×2 child values are0 and2/5 before quantization; the final1×1 is1/5. Blind2×2 dropping the last row would produce0 and is wrong. A single nonzero center-row texel contributes1/15 to each intersecting child and1/15 to the final ideal average; no boundary data disappears. Complete dimensions total18 texels/72 bytes.

2. **Constant role chains.** Any constant stored numeric byte b stays b under area mean/nearest-ties-even at every level. Constant sRGB RGB decodes/averages/reencodes to its source value under the ideal transfer, with actual implementation roundoff separately checked. No dimension-dependent darkening or extra gamma.

3. **Black/white versus numeric midpoint.** Equal-area sRGB black/white decode to0/1, linear mean=.5, encode≈.735356983 and byte188. Sampling188 decodes≈.502886458, not exactly.5 because of storage quantization. Numeric0/255 mean127.5 rounds ties-even to128. Averaging encoded sRGB to128 and then decoding≈.21586 is not the selected color filter.

4. **Alpha-weighted Mask.** Equal-area opaque red(1,0,0,A1) and transparent bright green(0,1,0,A0) give linear A=.5, stored128, and straight red RGB, not yellow/green contamination. Zero weighted alpha gives RGB0. R-024 later applies factors/vertex alpha and its unmodified cutoff; this does not guarantee the original two-texel silhouette or half coverage at another LOD.

5. **Normal boundary.** Every parent BLUE128 gives ideal qz=1/255; any area mixture of BLUE>=128 and each recursively quantized level remains BLUE>=128. Trilinear mixtures preserve ideal positive Z. Source X/Y can cancel without creating q=0 because Z remains positive. No per-input/mip normalization, normal_scale or roughness adjustment is baked.

 Under §6's direct route, constant stored(255,128,127) at scale1 is certified by +X without modifying bytes. A scale0 equal BLUE127/128 mixture is an exact zero ONLY when its common-UV/LOD/address branch is actually reachable; an arbitrary raw hull is not a reachability proof. For actual adjacent levels evaluate their bilinear responses at the SAME UV, form the real joint-cell corners and verify the Bernstein identity and1/510 outward projection at every corner. Include nearest ties, repeated/mirrored/clamped seams, negative UV, NPOT1×N and last-level LOD; all6 MIN and independent MAG/U/V remain exact. The five supplied cases in §6.5 are source feasibility, not new GPU results.

6. **Packed MR/AO isolation.** For numeric texels with independently different R/G/B, expected child R/G/B are their respective area means. MR reads G/B and AO reads R after sampling; changing R does not become metallic/roughness, and an sRGB view cannot stand in for the numeric lease.

7. **Six MIN modes and U/V addresses.** Use identifiable source levels and boundary-crossing UV footprints, independently selected MAG and each6-mode MIN with Clamp/Repeat/Mirror U/V. Mip-nearest must choose its source-selected level and mip-linear its adjacent interpolation; authored no-mip remains base-level behavior. This must be observed on actual pinned sampling, not deduced from a descriptor name. Full chains are mandatory for the four mip modes.

8. **Derivative-before-discard.** A source-qualified varying UV0 and Mask edge require derivatives before role branches/discard; compare surviving sample footprints with the predeclared derivative-valid reference. Helper-sensitive, zero/large/degenerate and nonfinite cases require deterministic source/host classification and predeclared available observable comparisons under R024 §8.2.4 for the declared domain/envelope. Do not claim helper execution directly measured from final pixels; missing discrimination or an essential unselected comparator leaves the gate OPEN. No guessed gradient, universal guarantee or fabricated CPU fragment error follows.

9. **1024² checker level3.** For an alternating black/white sRGB checker, the recursive full chain gives level1 byte188 and every later constant level, including level3, remains188. Numeric checker levels instead settle at128. The selected gradient footprint8 source texels must exercise the corresponding minification sampling; §12 fixes stage and error gates.

10. **Coherent replacement/capacity/no-use.** A new full-chain source needs all levels/reservations, including still-leased prior generations. Over-budget, stale or cross-context content refuses without mixed levels or role loss. A compatible old front stays wholly old; retired/incompatible target withdraws. No-mip, absent-role, no-view and unused-V2 paths do no unrequested chain generation/work.

11. **Nonconstant multiscale source and explicit-level witness.** The §12.2 temporal fixture SHALL be this deterministic1024×1024 Opaque RGBA8 sRGB source. For integer texel coordinates0<=x,y<1024, all three RGB bytes equal c(x,y) and A=255:

```text
signed_bit_k(t) = 2*((floor(t/2^k) mod 2))-1
c(x,y) = 128
       + 24*signed_bit_0(x) + 20*signed_bit_2(x)
       + 16*signed_bit_4(x) + 12*signed_bit_6(x) + 8*signed_bit_8(x)
       + 16*signed_bit_1(y) + 12*signed_bit_3(y)
       + 8*signed_bit_5(y) + 4*signed_bit_7(y)
```

The nine independent signed-bit terms have total magnitude120, giving the exact attainable byte range[8,248]; no source clamp is needed. Generate the COMPLETE chain by §4's recursive area filter over the preceding stored level and §5's linear-light sRGB encoding/nearest-ties-even quantization. The fixture's authored V2 sampler is MAG Linear, MIN LinearMipLinear, U/V ClampToEdge, anisotropy1. These are nine distinct spatial scales, not a constant-color source or a replacement for the unchanged checker gate.

An independent source-byte oracle follows the original pixel(101,307) through level-L coordinates(floor(101/2^L),floor(307/2^L)):

| L | Level coordinates | Expected encoded R=G=B byte |
|---:|---|---:|
| 0 | (101,307) | 168 |
| 1 | (50,153) | 147 |
| 2 | (25,76) | 132 |
| 3 | (12,38) | 115 |
| 4 | (6,19) | 127 |
| 5 | (3,9) | 144 |
| 6 | (1,4) | 136 |
| 7 | (0,2) | 125 |
| 8 | (0,1) | 129 |

Thus levels0..8 have nonidentical neighboring responses in this witness; levels9/10 need not be distinct. The byte sequence is a documentary generator oracle, not an executed producer or GPU result. The alternating checker in case9 becomes constant after level1: it proves linear-light generation independently, but cannot alone discriminate later LOD selection.

## 12. Exact future quality, native and performance acceptance

### 12.1 Sampling-stage numeric gate

Before execution, the test owner SHALL record the actual source/checker bytes and encoding, complete chain/generator identity, selected V2 sampler/role, target/backend/format/sample count, exact UV/projection/derivative footprint and the interior sample ROI with explicit coordinates/excluded boundary policy. No result-dependent ROI choice.

For a1024² alternating black/white source checker, the generated level3 expected color is encoded188 / sampled linear approximately.5. Actual supported GPU explicit-level3 observation AND selected gradient sampling with footprint8 SOURCE texels must each yield per-channel linear absolute error<=.03 on that predeclared interior ROI.

Observe the texture-sampling stage before material factors/lighting/tone/output transfer, or explicitly reconstruct that same source-bound sampled value with an independently qualified stage oracle. A post-tonemap pixel or coefficient is not automatically the sampled material value. Explicit-level sampling is a test oracle, not a production replacement for §8 gradient sampling.

### 12.2 Temporal sampling gate

The required source is §11 case11's NONCONSTANT multiscale1024² Opaque fixture, its exact complete role-correct chain and the authored Linear/LinearMipLinear/ClampToEdge/anisotropy1 sampler. A constant source or the post-level1 constant checker cannot substitute for this temporal LOD proof.

Record before execution the exact opaque texture/source chain, geometry, UV/camera/projection sequence, ROI, frame identities/reprojection mapping, baseline and a predeclared supersampled reference with >=16 independent area samples per evaluated footprint. Reference positions/weights, source/color/filter domain, error computation and percentile estimator must be fixed in that owner-reviewed measurement record, not chosen after output is seen.

For each frame f=0..119, the prescribed NOMINAL isotropic source-texel footprint and LOD are:

```text
lambda_f = 8*f/119
p_f = 2^lambda_f
```

This sweeps footprint1..256, crossing all eight adjacent mip boundaries and the fractional mip ranges. Before execution, the future test owner SHALL predeclare geometry/UV/camera and the actual ROI with qualified derivatives producing and observing those nominal footprints, exact reference-space UV positions/weights and observation stage. Evidence SHALL identify each frame's actual derivative footprint in that ROI; a requested camera path or nominal lambda alone is not observation. Missing/unqualified frames or absent LOD transitions fail or leave the proof UNVERIFIED, never become zero-error samples. Do not relabel lambda from requested exposure or choose a different path/ROI after observing output.

For the120-frame controlled camera/UV footprint sweep, the95th-percentile per-pixel REPROJECTED LINEAR RGB error relative to that reference SHALL be<=.05. Identify exactly the evaluated pixels/frames and valid correspondence; missing/out-of-domain observations cannot silently be counted as perfect samples.

The same fixture additionally requires actual GPU explicit-level sampling checked against independent source mip bytes at least at ALL levels0,1,2,3,4,5,6,7,8, and real gradient samples at both integer and fractional nominal LOD. Compare in the source role's correct color domain BEFORE material factors/lighting/tone/output, or use independently qualified source-bound reconstruction of that stage. Explicit-level probes remain test oracles, not replacements for §8's production gradient rule. A fixed-LOD, constant-source or no-change sweep cannot pass even if its aggregate error is low. The predeclared >=16-area-sample reference,120-frame duration, p95<=.05 criterion and separate JC native judgment remain mandatory.

This numerical sampling criterion does not certify Mask silhouette, normal variance, optical exactness, complete PBR illumination or subjective beauty. A cosmetic pixel/style golden is not the acceptance method.

### 12.3 Native source-to-output gate

Canonical App::run proof uses actual textured floors/walls and glTF-origin furniture, source-qualified channels/tangents/UVs/samplers/chains, and separately admitted R-025/026/027 features where claimed. Missing consumer data is not supplied by mip support.

Observe actual native target/output at scene sample1 AND4, target resize/inactivity/retirement/restoration, no-use, compatible original last-good, actual source/light movement and complete rendered associations. Trace sampling through scene/HDR/tone/coverage/linear-FP16 Image composition and native presentation; do not confuse source acceptance, enqueue, GPU completion, presentation call or actual visible quality.

Baseline ordinary2D and legacy/V1 behavior remain unchanged. JC independently decides native visual quality. Headless output, compiled shaders, broad FPS, uniforms, screenshots alone or a generated concept do not close this gate.

### 12.4 Fixed performance gate

On each pinned adapter/backend at1920×1080, use >=30 warm-up frames followed by >=120 measured frames. Record actual source/profile/workload, target/sample counts, queries/instrumentation, timing scope, baseline and method before evaluation.

Track CPU update, actual GPU scene/target/tone, upload and native presentation as SEPARATE stages. GPU timing must be genuine; CPU wall time or submission duration cannot masquerade as GPU time.

For the isolated texture-sampling workload, warmed all-role LinearMipLinear p95 GPU cost SHALL be<=1.5× the IDENTICAL warmed source/workload single-level variant, separately at sample1 and4. Geometry/material evaluator and unrelated admitted work must be controlled/identified so shadow, scene setup or tone costs are not relabeled texture sampling. The sampling workload/query scope and qualified baseline must be independently reviewable; a full-frame ratio does not establish the isolated criterion.

Unchanged source content requires ZERO steady-state content uploads after warm-up. Any view/model dynamic-buffer writes are identified separately rather than hidden as zero texture uploads; they cannot mask a source-chain reupload. Account complete resident/in-flight bytes and resource churn; the gate is not a license to omit source roles, lower quality or hide work outside the measurement window.

Missing timer, actual backend, pinned comparable workload, native evidence or JC observation leaves the corresponding gate UNVERIFIED. This documentary adoption executes none of them and promises no measured speedup or GPU duration.

## 13. Invariant/prohibition pairs

**Adopted / Binding [S1] — 2026-11-03: replacements/qualifications for the named §13 pairs.** All other source, normal, sampler, ABI, lease and absence pairs remain unchanged.

- **INV-R028-GRADIENT / FORBID-R028-GRADIENT:** Preserve derivative-valid controlled WGSL, derivative-before-discard and original per-role explicit sampling. Source checks occur on dependency changes; actual changed host/target/capability checks remain. GPU helper/derivative/filter numerics use the declared domain/envelope and representative observable conformance. No mandatory production per-FOV vertex/triangle clip/determinant/helper/footprint proof, guessed gradient, altered sampler/shader or unsupported backend pass.
- **INV-R028-TRANSACTION / FORBID-R028-TRANSACTION:** Before writes, require the exact live source proof, valid actual submitted host data, current target, finite reservations and actual Device eligibility. Required current R025 work and same-Queue complete FP16/Image publication remain. No proof-only sweep or replacement frame-local certificate, per-frame oracle/readback, partial commit or relabeled last-good.
- **INV-R028-PROOF / FORBID-R028-PROOF:** Original source/host mathematics, actual controlled program/capability checks, predeclared observable GPU evidence and native/quality/performance/JC decisions remain distinct. An inconclusive GPU model is not source invalidity. No universal GPU precision theorem, hidden-intermediate telemetry, borrowed tolerance, per-frame S3 replay or known failing backend acceptance.
- **INV-R028-BUDGET / FORBID-R028-BUDGET:** All actual source verification, retained metadata, required host work, GPU objects and old/in-flight generations remain finitely charged. Do not reinstate removed GPU-model work through a mandatory certificate/cache/service, unbounded storage or changed quality.
- **INV-R028-BACKEND-BOUNDARY / FORBID-R028-BACKEND-BOUNDARY:** Apply R024 §8.2.4's adopted 2026-11-03 boundary to the versioned supported source/backend class. Preserve original source constants and every §12 threshold, predeclared comparator authority and compatibility/exclusion rules. S1 adoption, API presence, compilation or a plausible image alone grants no backend or S3/S4 acceptance.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

**INV-R028-PROFILE.** V2 is explicit/versioned sampling paired with the exact V1 evaluator; V1 remains unchanged and selectable.  
**FORBID-R028-PROFILE.** No automatic V1 migration, BRDF/roughness rewrite, implementation-selected deviation from the adopted source/frame/filter certificate policy, or activation from a Draft. The coordinated 2026-11-01 source qualification is adopted explicitly; shader/sampler math is not rewritten.

**INV-R028-SOURCE.** Original role/image/format/encoding/UV/sampler/generator identity and complete immutable levels qualify each present role. Independently verify actual source/render-mode meaning and complete common-UV witness/coverage/scale/certificate association; these deterministic requirements are not replaced by backend observations.  
**FORBID-R028-SOURCE.** No guessed provenance/defaults/coordinates, present-invalid-to-absent substitution, importer, semantic image inference or source/certificate validity inferred from device acceptance.

**INV-R028-CHAIN.** Mip MIN uses the exact finite recursive dimensions and complete area-filtered chain; odd edge contributions and all bytes are retained.  
**FORBID-R028-CHAIN.** No partial/duplicate terminal levels, blind odd-edge decimation, hidden POT resize, downres or late GPU generation.

**INV-R028-COLOR.** Color RGB decodes to linear before generation/filtering and encodes once; numeric roles never color-decode.  
**FORBID-R028-COLOR.** No encoded-space averaging, double gamma, wrong view or Image material normalization.

**INV-R028-ALPHA.** Opaque/Mask source filtering is role-explicit; Mask stores alpha-weighted STRAIGHT color and independent linear alpha with unchanged discard semantics.  
**FORBID-R028-ALPHA.** No stored material premultiplication, cutoff/coverage repair, dither/A2C, geometry expansion or unsupported silhouette claim.

**INV-R028-NUMERIC.** MR/AO raw channels average independently, preserving actual packing/role/factor meaning.  
**FORBID-R028-NUMERIC.** No channel/encoding substitution, incompatible shared lease or factor/source roughness baking.

**INV-R028-NORMAL.** Every normal level preserves original stored-parent first moments and exact source scale/sampler meaning. Independently check complete common-UV source response-corner certificates and positive post-scale bounds; BLUE>=128 remains sufficient. Within the declared source input domain/backend envelope, filtering/frame/normal output consequences require bounded observable conformance under R024 §8.2.4, not direct hidden TBN/normal telemetry.  
**FORBID-R028-NORMAL.** No generator normalization, scale baking, source-byte repair, GPU tangent/fallback normal, Toksvig/variance/roughness inference or unproved specular-AA claim. No arbitrary independent raw-eight-texel predicate as necessary filter law, and no ideal1/510 or finite observation asserted as universal GPU normal precision.

**INV-R028-SAMPLER.** Independent MAG, within-level MIN, between-level mode and U/V addressing preserve all six source-selected semantics, full mip access and anisotropy1.  
**FORBID-R028-SAMPLER.** No linked min/mipmap shortcut, authored Nearest upgrade, wrap override, hidden LOD bias or forced LOD0 for a mip request.

**INV-R028-GRADIENT.** Controlled V2 computes center-UV dpdx/dpdy before nonuniform presence/discard and uses each role's explicit-gradient sampling. Rigorous source/host checks and derivative-valid IR remain mandatory for each candidate. The declared domain/envelope requires actual capabilities and predeclared observable helper-sensitive sampling/coverage results under R024 §8.2.4, without claiming direct helper-lane execution measurement.  
**FORBID-R028-GRADIENT.** No known invalid/undefined derivative treated as success, guessed gradient, application WGSL, forced mip, post-fragment CPU failure or role drop. Inconclusive undocumented GPU numerical enclosures alone are not source-invalid proof; no raster-grid skip or unqualified backend pass follows.

**INV-R028-ABI.** S1-adopted groups0–2/material bytes/five roles and independent group3/output contracts stay exact; profile identity distinguishes V2.  
**FORBID-R028-ABI.** No fifth group, raw GPU entry, ownerless sampler/binding or cube/depth/HDR/2D mip contamination.

**INV-R028-LEASES.** One complete-chain generation is retained and retired through Context3D/flat source/front owners with all old/pending/in-flight uses qualified. Certificate and interpreted-frame dependencies retain their exact immutable old/pending/current generations.\
**FORBID-R028-LEASES.** No mixed generations/levels, in-place mutation, early leased eviction, stale-front relabel or private recovery.

**INV-R028-BUDGET.** Exact full-chain bytes, aligned upload/staging/residency, variants and bounded work reserve before publication/writes. Include finite certificate verification, coverage/witness metadata and every retained generation; no unbounded optimizer or preparation-time source search.\
**FORBID-R028-BUDGET.** No base-only charge, guessed aggregate cap, unlimited growth or quality reduction on exhaustion.

**INV-R028-TRANSACTION.** Existing Update, HAL266A target and HAL295A same-Queue commit retain the complete immutable source/material/view/target association. Each candidate passes required source/certificate/host/format/capability/target checks and finite reservations before writes, with lawful reuse of checked immutable captures. S3 accepts a versioned domain/envelope from representative observable tests, not one frozen frame or a per-fragment oracle; valid dynamic successors need no per-frame S3 replay. Failed candidates never relabel old fronts; incompatible full keys withdraw.  
**FORBID-R028-TRANSACTION.** No background rebuild/clock, extra phase, second publisher, GPU semantic writeback, Image alpha repair, source/certificate bypass or conformance-test readback used as production source/admission authority.

**INV-R028-ABSENCE.** No unselected/no-role/no-view/inactive V2 work or idle cost, apart from necessary existing lifetime retirement.  
**FORBID-R028-ABSENCE.** No eager source rebuild/pipeline churn, dummy mip-quality claim or ordinary2D hot-path branch.

**INV-R028-PROOF.** Source/host mathematics and checks, compiler/ABI/capabilities, bounded observable GPU evidence, quality, native/performance and JC decisions are distinct. Each test retains exact source/camera/device/backend/driver provenance; S3 declares a versioned supported input domain/backend envelope, not that provenance as a per-frame admission key. Hidden N/T/g/arithmetic/helper values are not claimed measured from final pixels; finite tests prove no universal GPU precision.  
**FORBID-R028-PROOF.** No enum/parser/compile, coefficient, aggregate color statistic, CPU time/FPS or aesthetic golden replaces a required observable discriminating comparison. No unselected TBN/normal/GS tolerance, out-of-domain eligibility/capability class or known failing driver is silently supported; valid in-domain data changes are not rejected merely because they differ from a recorded fixture.

**INV-R028-BACKEND-BOUNDARY.** R024 §8.2.4 separates rigorous source/host checks from GPU-owned execution. S3 qualifies a versioned source input domain/backend capability envelope using representative observable N1/N4 adversarial evidence and the unchanged original three-asset external-consumer conformance fixture. Per-candidate checks remain; exact immutable test provenance is not a frozen runtime key. Declare platform/driver capability-family compatibility, regression coverage/exclusions and residual variance; all source constants/§12 thresholds remain.  
**FORBID-R028-BACKEND-BOUNDARY.** No invented deterministic vendor error contract, guessed epsilon, universal GPU trust, silent capability enablement, changed shader/source/sampler, or inference that a failed overapproximation proves a reachable hardware failure. No implementation change follows from a nonbinding candidate.

## 14. Exact parent/dependency and compatibility matrix

**Adopted / Binding [S1] — 2026-11-03: qualification of §14 cross-references.** The matrix's source/host/numerical requirements SHALL use the source-change, actual-host and GPU-conformance split adopted in §§6/8/9/13/15 and R024 §8.2.4. No matrix row reinstates unconditional proof-only vertex/triangle work on FOV. HAL355 retention, HAL002 phases, actual R025 work, R026, R027, ABI, same-Queue front and source semantics remain unchanged; this is no blanket profile or parent exception.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

HAL-370 §8/HAL-355 §17.1 adopt only the exact opt-in V2 extensions below. HAL-354/357/266A/295A/263A/309 remain unchanged; the first extended source version is unlinked flat-only and refuses linked G1. S2 authorization and actual source/Device/native proof remain required.

Source/host checks remain rigorous for each candidate. R024 §8.2.4 qualifies a declared versioned input domain/backend capability envelope using representative available observable comparisons plus the unchanged original three-asset external-consumer conformance fixture; exact evidence provenance is not a per-frame key and valid dynamic inputs do not require oracle replay. Capability-family/platform/driver compatibility uses declared regression coverage/exclusions, not metadata-only per-frame owner decisions. No hidden-intermediate measurement, essential unselected numeric tolerance or universal GPU error guarantee follows.

| Owner / affected clause | Preserved boundary | S1 contract and remaining engineering/proof |
|---|---|---|
| HAL-002/002A | Sole owner/ID, phases, immutable values and sovereignty | No constitutional exception proposed; producer facts never become semantic renderer authority. |
| HAL-355 §§7–8.4,11–13 | Typed complete resources, flat structure/models, original-front/lease/budget truth | Exact typed full-chain and independent sampler admission, complete revisions/finite bytes and bounded upload/retirement; current single-level content is insufficient. |
| HAL-355 §§9.1–9.5 | Legacy ABI/wrapper and distinct controlled shader/pipeline identity | Coordinated versioned V2 sampler/derivative/controlled PBR implementation with same material bindings and actual limits/format/IR proof, not arbitrary shader access. |
| HAL-355 §§10–13,15–17 | Actual current target, same-Queue execution, no partial front/2D bleed/unbounded work; mip generation excluded from baseline | Admit only the typed optional source chain/mechanical sampling under the exact S1 parent decision; no blanket baseline feature change or engine mip generator. |
| HAL-354 §4.1, mounted transfer and §4.4 | One source Update before mechanical preparation, complete publication | Any extended material/source tuple is explicitly coordinated; no silent G1 widening or second Update. |
| HAL-357 §§2,4,6–8 | Optional complete flat lowering and qualified associations | Preserve source-role/chain/material identity when composition is used; no shader-time domain traversal or semantic source conversion. |
| HAL-266A §§3–6 | Current immutable target metrics/indexed GPU-local preparation | Admit bounded per-target resource work, no independent DPR/target inference, Timeline or semantic writes. |
| HAL-295A §§3.2–3.4 and INV-008 | Existing Device/Queue, enqueue-before-swap, front registration/lifetime | Full chain/sampler leases and work ordered before one truthful final commit; no material publisher or second offscreen system. |
| HAL-263A §3.2 and INV-002/004/005/006/008 | Sampled premultiplied-linear FINAL Image and exact ingress/modulation | Material straight color/alpha/numeric mip data never use Image normalization; R-027 final remains independently conforming, no new ingress selected. |
| HAL-309 §2 and status/evidence rules | Intent, binding authority, source implementation and executed proof remain distinct | Record delegated coordinated S1 adoption, accepted old-domain private S2 and still-open expanded source/Device proof separately; no new engineering/public/native authority from drafting. |
| R-023 §§6/11–13, S1 binding under HAL-370 | Role-correct sampling owner, finite resource/quality and stage truth | This file supplies the S1-adopted mip contract, not implementation of the rendering family. |
| R-024 §§5–8.4/10, S1 binding under HAL-370 | Exact scalar/channels, r_eff, selected A/B/C or genuine-absence source certificate/local TBN, Material64/PBR52 and V1 single-mip sampling | Explicit V2 sampling and coordinated source-domain law; old cone/BLUE routes remain sufficient, common-UV certificates govern extension. Preserve V1 single-level versus V2 complete-chain semantics, original shader/evaluator and all independent source/currentness gates. |
| R-025 §§5–10, S1 binding under HAL-370 | Fixed depth maps/shadows/direct source and group3 | No depth-map mip/filter/visibility/resource change; real source/light/shadow proof stays separate. |
| R-026 §§3–9, S1 binding under HAL-370 | World-qualified E/G, independent eight SINGLE-MIP cubes, LUT and group3 | No E cube/LUT mip chain or GI variation from V2. Eight roughness cubes are not a general mip implementation. |
| R-027 §§4–9/11–13, S1 binding under HAL-370 | Complete<=60000 radiance certificate, tone-before-average, coverage and linear-FP16 final | New sampled material values remain part of its complete bound/source tuple; no scene/final mip or output-transform change. |

The coordinated S1 decision adopts the exact predecessor/parent exceptions, not source support. Separate S2 authorization must precede source work; real S3 capability proof and S4 native/performance/JC acceptance remain OPEN. No automatic producer implementation or source authority follows.

## 15. Proof ownership and OPEN backend/source gates

**Adopted / Binding [S1] — 2026-11-03: replacement of BLOCK-R028-NUMERIC and qualification of §15.**

**BLOCK-R028-NUMERIC:** Complete original source/frame/normal/radiance certificates and actual source derivation/packing arithmetic SHALL remain rigorous. Actual submitted camera/object/view data and necessary actual host calculations must be finite, valid and representable. No mandatory runtime per-FOV full-vertex/full-triangle simulator for GPU transform/clip/screen/helper/footprint numerics or equivalent frame certificate is required. GPU consequences require controlled derivative-valid programs, actual capabilities and independently accepted representative predeclared observable N1/N4 evidence for the versioned S3 domain/envelope. An inconclusive model alone is not source-invalid proof. Actual invalid source/host data, missing eligibility or observed failure still prevents the affected admission/acceptance; no new epsilon, fallback or shader change is selected.

All intended-proof rows inherit that boundary. Source and immutable-admission rows validate genuine changes and exact still-live reuse; GPU rows use available observations, never hidden-intermediate claims or production readback. The original three-asset external-consumer conformance fixture at N1 and N4, R025/R027, lifecycle and unchanged §12 numeric/temporal/native/performance gates remain mandatory at their proper stages.

Only a separately authorized complete S2 unit may change runtime code/tests under HAL370 §9's adopted future S2 proof contract. Old tests pinning proof-only work/refusal require explicit contract review, not silent expectation changes. No implementation, source-profile widening or S3/S4 adoption is granted here.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

S3 accepts an explicitly declared versioned supported SOURCE INPUT DOMAIN and BACKEND CAPABILITY ENVELOPE under R024 §8.2.4, based on independently accepted representative actual tests and the mandatory unchanged original three-asset external-consumer conformance fixture. Each executed record separately preserves exact source/certificates/packed bytes, camera/target, actual Device/backend/driver/compiler and results for reproducibility. Those evidence pins are not per-frame runtime admission keys. Every candidate still passes source/host/certificate/format/capability/target/resource checks; within the accepted domain/envelope, new valid geometry/material or source values, camera motion, view and target changes require no full-oracle replay or individual S3 acceptance. Platform/driver compatibility follows its declared capability family, regression coverage and exclusions; unsupported classes, changed shader/source-eligibility classes, known failing drivers and out-of-domain inputs do not silently inherit support. Source checks, available observations, comparator authority and backend evidence remain distinct.

| Intended proof | Required evidence | Existing owning boundary |
|---|---|---|
| CPU chain/role/source | Exact odd3×5 weights/bytes, full1024² counts, stored-parent derivation/rounding, sRGB/Mask/numeric channels; original provenance; BLUE128 route or full post-scale correlated common-UV corner/coverage certificate. Independently check actual four original normal roles/all required levels, not a consumer label. | Existing three_d_context/geometry/appearance tests and independent original producer. |
| Immutable admission | Complete chain/role/sampler/scale/source identity and original A/B/C versus derived meaning; checked witnesses, finite work/storage, stale/cross-context/missing/over-budget typed refusal, exact successors and no source repair. | Existing three_d_context/snapshot/viewport tests; deterministic host validation. |
| Controlled shader/ABI/capability | Exact PBR52/Material64/five roles and source-selected V1/V2, derivatives before branches/discard, matched centroid/center interface, real formats/views/limits and selected sample/filter support. Pinned controlled Naga/wgpu validation and actual Device execution, not descriptor names alone. | Existing three_d_appearance/raster/backend tests. |
| Bounded actual GPU sampling/frame | Predeclare fixture, independent prediction and falsifiable authorized comparison for all required levels/MIN/MAG/U/V, sRGB-before-filter, scales, five correlated cases and A/B/C/absence. Include triangle63, subpixel/partial/clipping and helper-sensitive gradient/LOD cases at N1/N4. Compare actual FP16/Image, genuine available source-stage probe outputs or observable coverage/Device results. Hidden N/T/g, post-scale/normalized q, coefficients, inverseSqrt or helper execution are not directly measured from final pixels. Missing discrimination is UNVERIFIED; essential additional numeric bounds await selection by the delegated architectural reviewer before execution. | Existing three_d_raster/backend tests; R024 §8.2.4. |
| Original external-consumer execution | The unmodified three original assets and original caller of the external-consumer conformance fixture through public typed unlinked production PBR, R025/R026 when present and R027, N1 AND N4, with actual source/target/committed identity, finite FP16 front and actual registered ordinary Image observations. Internal fixtures and source-only acceptance are insufficient. | External conformance fixture `src/tests/pbr_ingress.rs::pbr_original_polyhaven_public_halcyon_metal_n1_n4`; Halcyon existing external test-harness observation owner. |
| Target/lifetime/absence | Actual metrics/full format/sample key, source-qualified current front, compatible original last-good, incompatible withdrawal, resize/inactive/restore/retire, bounded resident/old/in-flight charges, same Queue and unchanged-input zero uploads/work. | Existing three_d_context/viewport/custom_render_preparation tests. |
| Numeric quality | Unchanged §12.1 .03 source-stage checker/level/gradient metric and §12.2 original multiscale120-frame sweep, fixed ROI/reprojection, >=16-area-sample reference and p95<=.05 with original estimator/frame coverage. These thresholds are ONLY for those sampling/temporal metrics, never TBN/normal/GS comparison bounds or universal precision proof. | Existing source-stage three_d_raster/viewport/backend owners. |
| Native/performance/JC | Unchanged §12.3 real App::run N1/N4 and §12.4 genuine isolated GPU timing ratio<=1.5 at1920×1080, >=30 warm/>=120 measured, source uploads/lifetimes and independent native/JC judgment at the claimed scope. Numerical source or headless proof does not close these stages. | Existing backend/performance owners, canonical native fixture and JC. |

| Gate | Disposition and closure condition |
|---|---|
| BLOCK-R028-PARENT | Selected S1 profile/source law is binding; any backend-evidence amendment needs its own coordinated independent adoption. S2 engineering and S3/S4 decisions remain separate; a proposed document changes nothing live. |
| BLOCK-R028-PRODUCER | Close only for independently validated actual source/role/generator/UV/sampler/scale, full bytes/levels/cells and finite host verification. Prior accepted producer evidence is not GPU evidence. |
| BLOCK-R028-NUMERIC | Source/host inequalities, packing, full ideal radiance/corner certificates and modeled host representability remain rigorously checked. GPU evidence uses only available observations and predeclared discriminating comparisons for the declared domain/envelope. Host clip intervals do not prove actual GPU vertex/clip precision. An essential unselected numerical comparator or unavailable discrimination leaves its test gate OPEN; .03/.05 cannot be borrowed for frame/normal/GS. Known-invalid data or observed failure still prevents the affected acceptance. |
| BLOCK-R028-NAGA-DEVICE | Require controlled pinned modules, actual feature/format/limit/sample/filter checks and independently accepted representative observable N1/N4 conformance for the declared capability family, including original/adversarial cases. Declare platform/driver compatibility through regression coverage/exclusions; metadata-only changes are not automatically new per-frame S3 decisions. Unsupported classes and known failing drivers remain unqualified; no new optional production feature follows. |
| BLOCK-R028-RESOURCE-FRONT | Close only for exact certificate/derived-frame/chain cost, capture, complete successors/leases, full target compatibility, no-use, current/last-good and same-Queue retirement evidence; numerical trust changes no ownership guarantee. |
| BLOCK-R028-QUALITY | Existing numeric/temporal receipts retain their scope. Unchanged §12 metrics and widened-source/derived-frame/Mask claims require their own evidence; an average or percentile cannot certify source nonzero or universal GPU behavior. |
| BLOCK-R028-NATIVE-PERFORMANCE | Actual native N1/N4 consumer output, genuine timing/ratio/zero uploads and JC judgment remain independently gated; S3 sampling acceptance is not native/performance acceptance. |
| Combined original-source S3 | OPEN until independent acceptance of the declared versioned source input domain/backend envelope, rigorous per-candidate guards, representative observable N1/N4 adversarial tests, the mandatory unchanged original three-asset FP16/Image oracle, authorized numeric comparisons and required §12/lifecycle evidence. Once accepted, ordinary in-domain source/camera/view/target successors require no complete replay of the original three-asset external-consumer conformance fixture or fresh S3 decision per frame. API existence or isolated fixtures do not alone close it. |

No constant, quality tolerance, sampler approximation, generator fallback or original-oracle source/camera change is delegated to an executor. Actual failure retains its exact source/host/Device evidence; an inconclusive modeled GPU enclosure remains indeterminate. R024 §8.2.4 requires predeclared observable discrimination and keeps unselected essential numerical comparators OPEN. Source/host/certificate/capability/target checks remain per candidate; in-domain dynamic changes are not unsupported merely because evidence provenance is different. S3/S4 require separate explicit acceptance and remain CLOSED here; no GPU pass for the original three-asset external-consumer conformance fixture is asserted.

## 16. Documentary evidence, references and disposition

The following source table preserves ORIGINAL authoring observations; later accepted old private implementation is recorded in the header. Neither stage testifies to implementation of the newly widened normal/source domain.

| Source | Inspected fact / boundary |
|---|---|
| [content.rs](../../src/three_d/content.rs):58–218 | One2D RGBA8 source level; exact stored byte count; U/V address modes; coupled limited sampler fields. |
| [context.rs](../../src/three_d/context.rs):2590–2719 | Current limits/residency/upload, mip_level_count1/write level0 and min_filter-linked mipmap_filter. |
| [raster.rs](../../src/three_d/raster.rs):387–456 | Current legacy groups0–2, not an implemented V2 material sampling family. |
| R-024 §§6/8.2.3/8.4 | Exact S1 role/encoding/UV and one-mip normal restriction; preserved material ABI/evaluator. |
| R-026 §§4/8–9; R-027 §§4/8–9 | Independent single-mip E assets/group3 and complete radiance/FP16 target/front budgets; no mip authority borrowed. |

The [Khronos glTF2.0.1 sampler reference](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html) identifies the six source MIN choices and U/V semantics; it does not supply a Halcyon importer, generator or selected default. The [official WGSL textureSampleGrad reference](https://gpuweb.github.io/gpuweb/wgsl/#texturesamplegrad) describes explicit-gradient sampling; its current text does not prove pinned Naga25/backend capability. These are references, not adopted Halcyon authority or execution evidence.

Documentary verification checks whole-file readback/hash, actual links, paired INV/FORBID names, complete fixed task coverage, area/normal/sRGB/cost mathematics, six sampler modes, source/ABI/currentness matrix and all OPEN gates. No Rust build/test, CPU mip producer, shader compilation, GPU, native run, performance measurement or screenshot golden was executed by this unit.

Historical creation-unit receipt: only this file was authored by the original R-028 task; its predecessor/ComboBox/TagInput protection and no-history/index/external boundaries applied to that unit. Current disposition is **Adopted / Binding [S1], including the coordinated source-domain Revision02, 2026-11-01; old-domain private S2/internal unlinked writer ACCEPTED; expanded source/packed-f32/Naga/Device/Metal NOT ACCEPTED; S3/S4 CLOSED**. This documentary unit covered only R-024/R-028/HAL-370. No Rust/GPU/producer/asset execution or implementation grant follows. JC retains veto.
