# SPEC-HAL-370: Opt-In 3D Visual Profile Admission

## Adopted feature-free MRT4 linear scene amendment — 2026-11-06 [S1]

**Adopted / Active / Binding [S1 technical contract only].** Independent architectural review under JC's delegated technical authority and retained constitutional/product veto adopts `HAL-R027-HAL370-MRT4-FEATURE-FREE-S1-ADOPTION-01` jointly with [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md) §16. The exact reviewed seven-clause candidate SHA256 is `bfdc6a9a7a4ee5ce7cc75c767e1c0d3ebb37756120a17a0d14b43146638329f2`. The reviewed live HAL370 beforeimage is `783a3a1ae76576195976960ff820ec52208e93fe24e349d048691a3c40a4a90b`; all pre-existing unrelated amendment/working text is preserved.

This replaces ONLY the private single-FP16 scene with R/G/B/Coverage R32Float MRT4, its private scene/tone ABI, attachment accounting and unconditional actual-nearest premise, as specified in §§3/6/8/13 and R027 §§3–13/16. The original final Rgba16Float pair, native surface/Image ABI, source math, N1/N4, exposure after storage, same Device/Queue and all other adopted laws stay unchanged. Old private-FP16-scene descriptions are historical within this exact scope; §6.1 explicitly qualifies the retained earlier numeric paragraph. There is no Device feature request, runtime switch, new renderer or S2 implementation permission. Isolated feasibility is not full source/backend/native acceptance; S3/S4 dispositions and R024/R026 residual causes are unchanged.

## Adopted real-time numeric proof-boundary reform — 2026-11-03 [S1]

**Adopted / Binding [S1].** The coordinated adoption record below activates the complete 2026-11-03 reform reviewed at source revision `8d449eeb` across R024/R028/HAL370.

The dated reform clauses below are effective binding replacements or scope qualifications for their named clauses. Retained original text remains binding except to the exact extent superseded/qualified by this adopted boundary. The 2026-11-01 source-domain/backend-trust adoptions and all older unrelated S1 text and historical receipts retain their original scopes; JC's reserved constitutional/product veto remains.

The adopted contract removes mandatory production proof-only per-FOV GPU-numeric prediction sweeps while preserving original source validity, actual host checks, Context/Device/custody, actual R025 work, R027 and the existing quality gates. This documentary S1 adoption grants no implementation or S2 engineering; a separately authorized complete S2 unit remains required.

**S3/S4 remain CLOSED.**

**Earlier adopted status and dated records continue below, subject only to the exact 2026-11-03 qualification.**

**Author and final adoption authority:** JC; final veto retained  
**Date:** 2026-10-31  
**Status:** Adopted / Binding [S1] — narrow opt-in addition to HAL-355  
**Adoption:** PERFORMED 2026-10-31 through independent architectural review under JC's express decision delegation; JC retains final veto.  
**Implementation:** Old-domain private R-024–R-028 S2 and the internal unlinked Material writer ACCEPTED only for previously tested inputs. The coordinated Revision02 source domain is S1 binding but expanded producer/packed-f32/Naga/Device/Metal evidence is OPEN/NOT ACCEPTED; no new engineering grant. Public S3 and native/performance/JC S4 remain CLOSED.\
**Design mandate:** HAL-370-COORDINATED-3D-VISUAL-PARENT-ADMISSION-DRAFT-01; documentary resolution of B-01/B-02/B-03/B-04 only.
**Current adoption record:** HAL-R024-R028-HAL370-S1-REAL-SOURCE-COORDINATED-ADOPTION-01.

## Historical S0 contract scope — 2026-10-31

This section and the following architecture review record retain the original draft's facts and boundaries. Their inactive/no-adoption statements are historical; the current header and §1 record the subsequent coordinated S1 decision. No historical source or executed-proof receipt is upgraded.

- Objective: specify the exact proposed optional material/light/source/HDR parent deltas and proof-producing engineering stages; clarify only R-024's S2/S3 terminology.
- Standing objective: lawful, bounded native 3D visual implementation readiness; architecture selection, binding adoption, implementation and executed evidence remain separate.
- Review authority: JC retains constitutional/product/adoption authority and veto; the architectural reviewer selects the technical contract under express delegation; documentary review cannot select alternatives.
- Governing law: HAL-002/002A supreme; HAL-355/354/357/266A/295A/263A retain their binding scopes; HAL-309 is adopted. R-023–028 remain inactive proposals.
- Effective authority change: NONE. Prospective exceptions below require separate explicit JC approval and exact coordinated parent/feature adoption.
- Inspected baseline: halcyon0.227.18-beta, HEAD4fef351c; the pre-existing local R-023/R-025/R-028 corrections are protected inputs.
- Owned artifacts: this new file and only R-024 §§8.2.3–8.2.4 stage wording. No other parent, R document, code, test, example, asset, manifest or instruction change.
- Target: existing Context3D, flat Viewport3D, per-target participant and private CustomRenderPass/Image seam; no alternate GPU framework.
- Invariants: immutable complete requests, typed nonwrapping leases, actual target authority, one Update, strict phase separation, legacy ABI, same Queue, original last-good provenance and premultiplied-linear final Image.
- Forbidden: self-adoption, raw GPU/public shader escape, invented API signatures, hidden metric uniform, G1 widening, semantic preparation callbacks, second Device/Queue/publisher/scheduler, source GI ownership or false conformance.
- Reuse decision: extend the named versioned 3D family through the existing resource/request/publication owners only after their admission; producer work stays producer-owned.
- Dependencies: attributable decoded geometry/materials/tangents, source calibration, canonical E/G, complete radiance bounds and real compiler/Device/native proof remain independently required.
- Verification: documentary source/math/ABI/lifetime review, complete new-file readback/hash, narrow R-024 diff and protected local state; no Cargo, shader, GPU or native run.
- Proof placement: future existing three_d_context/geometry/appearance/snapshot/raster/backend/viewport and custom_render_preparation tests; canonical App::run observation in tests/app_frame_realization_native.rs; JC native judgment.
- Completion: all four proposed admission cuts and their exact remaining adoption/source/backend gates recorded, not a claim that they are operative or implemented.
- SemVer/changelog: N/A; inactive documentation only.
- Out of scope: source engineering, importers, runtime GI, camera/input, generic Image/2D changes, R-series algorithm changes or further SPECs.

## Historical architecture review

- Governing: HAL-002/002A; existing HAL-355/354/357/266A/295A/263A; HAL-309; separately inactive R-023–028.
- Widget policy: N/A; no widget behavior. Interaction added: NONE; no Focus/Capture/gesture/navigation change.
- FORBIDs: existing sole owner/identity/phase/sovereignty, no raw GPU/2D contamination/false front/unbounded work, closed Image ingress and same-Queue publisher remain in force.
- Mechanisms inspected: typed source/leases and flat requests; actual View80/Object112/Appearance16 layouts; mounted G1 exclusivity; current target preparation; surface-format CustomRenderPass front creation and existing Image registration.
- Reinvention check: no second rendering service, scene hierarchy, global material/light owner, new clock or phase. All new resource work remains private to the separately admitted 3D participant.
- Demo/example: N/A; none changed.

**Binding source-domain Revision02 adoption — 2026-11-01:** Independent architectural review under JC's express technical-acceptance delegation and continuing constitutional/product veto affirmatively adopts `HAL-R024-R028-HAL370-S1-REAL-SOURCE-COORDINATED-ADOPTION-01`. The reviewed Revision02 proposal is preserved outside the active source tree, SHA-256 `81b768ded17fc89bded86bf653dca51c5a6546a82e679edcf856fb69374e67f5` (75,195 bytes/493 lines). The original proposal remains historical nonbinding material; the coordinated operative clauses in R-024/R-028/HAL-370 are now S1 binding. Old private R-024–R-028 S2 and the internal unlinked Material writer are accepted ONLY for previously tested old inputs. Expanded source/packed-f32/Naga/Device/Metal proof, public S3 and external-consumer native S4 are NOT ACCEPTED; this documentary adoption grants no new engineering work.

**Binding backend-numeric trust-boundary amendment — 2026-11-01:** Independent architectural review, under JC's approval of the policy change and technical-acceptance delegation with final veto retained, affirmatively adopts `HAL-R024-R028-HAL370-BACKEND-TRUST-S1-COORDINATED-ADOPTION-01`. The selected historical source is archived outside the active source tree, SHA-256 `0269fe6a3d8ec3021aea6b16cb5b355d6e75533a14551eb26b3d80034a1c5519` (84,056 bytes/286 lines); the original candidate remains historical DRAFT/nonbinding material. Its coordinated operative contract is now **Adopted / Binding [S1]** in R024/R028/HAL370: rigorous per-candidate source/host checks, bounded available observable backend comparisons and versioned input-domain/capability-envelope acceptance, with exact evidence provenance kept separate. Earlier decisions/receipts retain their original scope; no shader, source, runtime or test changes are executed, no S2 engineering is granted, no earlier S2 acceptance is broadened and no new Device or original external-consumer success is claimed. The unchanged original three-asset external-consumer conformance fixture remains reported RED at triangle63 before N1 FP16/Image, N4 not reached; **S3/S4 remain CLOSED**. Additional comparator bounds and backend-envelope acceptance require their separate authorized evidence/decisions.

**Binding real-time numeric proof-boundary reform adoption — 2026-11-03:** Independent architectural review under HAL370 §§1/7 and JC's express reform mandate with continuing reserved constitutional/product veto affirmatively adopts `HAL-R024-R028-HAL370-REALTIME-PROOF-S1-COORDINATED-ADOPTION-01`. The accepted source is the complete `HAL-R024-R028-HAL370-REALTIME-NUMERIC-PROOF-REFORM-01` at revision `8d449eeb` (60 R024, 66 R028 and 74 HAL370 added lines). Its coordinated 2026-11-03 clauses are now **Adopted / Binding [S1]**, effective 2026-11-03. They remove mandatory production per-FOV proof-only O(triangle/vertex) CPU simulation of GPU transform/clip/determinant/reciprocal-w/helper/UV/LOD/footprint numerics and equivalent renamed frame certificates. Original source certificates and exact still-live source/Context/generation/lease reuse, actual submitted host calculations, current target/Device/capabilities/finite custody, actual R025 shadows/cascades/depth/XY/PCF, R027 FP16/Image, R028 §12 and the 2026-11-01 source/backend split remain binding. Only the expressly inconsistent proof-only per-FOV obligation is superseded; older unrelated S1 text and receipts are unchanged. This is no constitutional amendment, S2 engineering grant, implementation/GPU/native result or S3/S4 activation; **S3/S4 remain CLOSED**.

## 1. Purpose, authority and exact admission boundary

This law is Adopted / Binding at S1 for the narrow optional R-series family. Its SHALL/MUST clauses bind the selected architecture, source domains, algorithms, interfaces and prohibitions, with only the precise HAL-355 exceptions in §8 and the parent's §17.1 acknowledgment. It grants no S2 engineering work or implemented/public capability.

**Current adoption record — 2026-10-31.** Independent architectural review makes the affirmative S1 decision pursuant to JC's express delegation to decide technical acceptance. This records delegated technical adoption; it does not claim that JC personally read or reviewed the technical SPECs. JC retains final ownership, constitutional authority and veto. The exact coordinated corpus is this HAL-370, HAL-355's narrow acknowledgment, and [R-023](SPEC-R-023-3D-Visual-Rendering-Extension-Architecture.md), [R-024](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md), [R-025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md), [R-026](SPEC-R-026-Environment-and-Indirect-Lighting.md), [R-027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md), [R-028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md). Their selected feature scopes are binding only as coordinated here. No other Draft, future version or unlisted feature inherits adoption.

| Gap | Selected S1 binding resolution | Implementation prerequisite still OPEN |
|---|---|---|
| B-01 | Closed versioned material/direct/E/G/HDR/mip family, exact ABI and parent deltas; coordinated parent/feature S1 adoption PERFORMED | Separate source engineering authorization and actual source/backend proof |
| B-02 | Complete extension-source revision, unlinked flat publication and D-only metric packing | Concrete checked source/CPU/host integration, real producer and precision evidence |
| B-03 | Existing private front owner with profile-specific linear-FP16 final pair and existing low-level Image ingress; internal-format/target contract adopted at S1 | Separately authorized engineering and actual format/filter/sample/lifetime proof |
| B-04 | S2 complete private proof-producing engineering versus S3 public capability admission | Separate S2 authorization, same-scope executed S3 evidence, then the independently required S4 claim |

JC retains final adoption authority and veto; the express delegation above supplies the architectural reviewer's authority for this exact S1 decision. Quoting this law cannot adopt a later version, authorize S2, establish implementation or prove a backend. No constitutional exception is granted.

## 2. Source-qualified baseline and owners

The ORIGINAL authoring baseline had no implemented R-series family. Its source observations below remain historical, not a denial of later accepted old-domain private S2/internal writer. Existing HAL-355 remains fully supported and selectable:36-byte vertices, View80/Object112/Appearance16, groups0–2, one D2 appearance texture/sampler, controlled legacy WGSL, reverse-Z, sample1/4 and its surface-compatible offscreen front. The new Revision02 source law does not establish new implementation or expand that prior private proof.

| Existing evidence | Actual boundary, not new capability |
|---|---|
| HAL-355 §§3.5–3.8/6/8–11 | Flat immutable structure/model/view; typed context leases; checked scene-unit packing; requested/committed/target/last-good truth |
| [content.rs](../../src/three_d/content.rs):58–103 | Current texture formats are Rgba8Unorm/Rgba8UnormSrgb, one width/height/texel payload |
| [context.rs](../../src/three_d/context.rs):2622–2655 | One D2 layer, one mip, upload of level0; no admitted E assets, full mip chains or float role resources |
| [raster.rs](../../src/three_d/raster.rs):387–456 | Legacy three-group layout; Object112 VERTEX only; no material group2 or light group3 |
| [viewport.rs](../../src/three_d/viewport.rs):465–525 | Existing per-view pass, private Depth32Float/MSAA resources and target budget lease |
| [viewport.rs](../../src/three_d/viewport.rs):815–838,868–923 | Ordinary flat setter refuses an active linked writer; canonical linked transfer retains its separate exact authority |
| [custom_render_pass.rs](../../src/renderer/custom_render_pass.rs):611–634 | Current private final allocation fixes format=gpu.format; no existing FP16 final selector is claimed |
| HAL-263A §3.2/INV-004–006; HAL-295A INV-008 | Producer supplies premultiplied-linear sampled content through the closed low-level seam; same-Queue writes precede swap and publication |

The producer/application owns asset decoding, physical meaning, authored calibration, MikkTSpace execution/provenance, E preintegration and static G transport/world canonicalization. Halcyon would own only checked admission, mechanical shading and bounded resource/publication work. HAL-357 can mechanically lower qualified composition into flat geometry/placements; it is not a new scene or semantic owner. Source file presence, asset labels, dimensions, generated concepts and GPU images certify none of that meaning.

## 3. B-01: closed opt-in family and combined ABI

The S1-adopted family is closed to the following selected profiles. A selection is explicit typed immutable source content; no product-name, image, feature-presence guess or application WGSL selects a pipeline. A packet has exactly LegacyAppearance OR PbrMaterial, never both. The D-enabled restriction in §5 further rejects mixed legacy packets.

| Profile / source owner | Exact S1 contract; other original obligations retained |
|---|---|
| [R-024](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md), PbrMetallicRoughnessRealtimeF32V1 | Typed immutable material generations/role leases; local-first Gram–Schmidt and dual transforms; explicit R-024 A/B/C mapped frame or genuine-absence certificate, original normal/Mikk/corner provenance, complete normal-domain proof and positive-scale domain;52-byte stream, tangent offset36; Material64 and the dedicated controlled evaluator. Source roughness remains unchanged; r_eff=max(r,.045), GGX/Smith/Schlick/F0=.04 and all source/color/mask/default rules remain exact. |
| [R-025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md), PbrDirectLightShadowV1 | At most1 directional/4 point/4 spot; source-calibrated metric inputs and complete off-camera casters; group3/b0–5, Light608/Transform1984, seven1024² 2D depth layers/four512² depth cubes,54,525,952-byte pool. Both comparison samplers have GreaterEqual, Nearest mag/min/mipmap, ClampToEdge U/V/W, LOD0/0, anisotropy1; exactly nine Boolean observations. Original bias, near/far, reverse-Z, D7 six-chart/tie/U/B=T×q/cross-face/depth rules remain exact. |
| [R-026](SPEC-R-026-Environment-and-Indirect-Lighting.md), PbrEnvironmentIndirectV1 | Independent distant E and static source-baked two-bounce diffuse G; world-aligned prefiltered assets/GI6, group3/b6–10 and eight D/E/G layouts. E uses six32² irradiance faces, eight independent one-mip128² specular cubes and128² LUT,6,406,144 logical bytes. G remains producer T(L0)+T(T(L0)), excluding L0; no runtime bake, shader-side source-frame transform or implicit ambient. |
| [R-027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md), HdrSceneColorToneMapV1 | ONE private R/G/B/Coverage R32Float MRT4 scene at N1/N4 with common Depth32Float, one existing PBR/depth pass; no scene resolve; same-sample per-plane loads and original tone BEFORE averaging; unchanged linear Rgba16Float[1] final pair/Uniform16/coverage/EV/full<=60000 source certificate. |
| [R-028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md), PbrTextureMipSamplingF32V2 | Explicit additional 2D material full-chain/role-correct sampler/gradient version paired with the EXACT R-024 V1 evaluator. Six MIN modes, authored U/V/MAG and derivative-before-discard remain exact. V1's one-level sampling stays selectable under the coordinated R-024 source certificate; V1 does not enable V2. No E cube, shadow or HDR mip change. |

R-023 is the S1-adopted foundational ownership/quality architecture, not an additional runtime or blanket grant. This law neither duplicates the R feature algorithms nor relaxes their independently selected source, numerical, resource and native gates.

### 3.1 Fixed material-pass resources

| Group/binding | Exact S1 resource contract / significance |
|---|---|
| 0/b0 | ViewBlock80, same field types/offsets and view purpose; §5 explicitly changes D-enabled PBR numeric packing only |
| 1/b0 | ObjectBlock112, model_relative at0 and normal_to_world at64; PBR-only VERTEX\|FRAGMENT visibility; §5 D-only numeric exception |
| 2/b0 | MaterialBlock64, the exact four vec4 members and role/mode bits in R-024 §8.4.1 |
| 2/b1..5 | Five named D2 material textures: base color, normal, MR, AO, emissive |
| 2/b6..10 | Five corresponding source-qualified material samplers; V2 changes sampling version, not binding count |
| 3/b0 | DirectLightBlock608, only when D present |
| 3/b1 | ShadowTransformBlock1984, only when D present |
| 3/b2 / b3 | Seven-layer Depth32Float2D array / exact R-025 point-comparison sampler |
| 3/b4 / b5 | Four-cube Depth32Float array / SAME exact R-025 point-comparison sampler |
| 3/b6 | IndirectBlock112 when E or G: header16(E,G,0,0), six WORLD GI vec4 at16/32/48/64/80/96; W=0 |
| 3/b7 / b8 / b9 / b10 | E-only diffuse cube / eight-cube array / BRDF LUT / one clamp-to-edge linear filtering sampler, all original one-mip resources |

The full all-present fragment interface totals10 sampled/depth textures,8 samplers and6 uniform blocks: View, Object, Material, DirectLight, ShadowTransform, Indirect. Count alone proves no support: actual Device stage/binding/interstage/offset/format limits, Naga25.0.1/wgpu25.0.2 and backend programs must pass. No fifth group, application-assigned binding, public raw shader entry, new mandatory Device feature or second rendering framework is admitted. Pipeline identity includes the exact family/ABI/profile, fixed controlled shader, raster/cull/target/depth/sample and presence variant, not material identity alone.

**Binding D-only face-normal amendment, 2026-10-31:** Independent architectural review under JC's express technical delegation and retained veto adopts R-025's supplemental geometric-normal interface. D=0 retains the original PbrVertex52, five vertex attributes/user varyings and16 interstage scalar components. D present retains that primary52-byte layout/locations0–4 and adds ONLY a second stride12 Float32x3 vertex buffer at input location5 and matching VS-output/FS-input location5 vec3<f32> `@interpolate(flat)`: two buffers, six attributes/user locations,19 scalars. The D-only nonindexed GPU expansion follows original source triangle/index order, copies three primary52 corners and the SAME qualified local source-winding face normal to all three supplemental corners. Context/source/generation, both buffers/uploads/metadata and old/in-flight leases remain fully charged. Actual Device limits must admit at least2 vertex buffers,6 attributes and19 interstage components before writes. No binding, group, required feature, semantic mesh, legacy36 or D=0 change is admitted. Source geometric N, transformed by the existing true normal matrix and reversed on a double-sided back-facing receiver, serves geometric shadow terms/bias only; independent shading N serves the unchanged R-024 BRDF. Derivatives of centroid position, shading Ni substitution and reserved-slot reuse are forbidden. This narrow S1 amendment resolves the receiver-normal delivery gap; actual S2/compiler/Device proof and S3/S4 remain independent.

Genuinely absent material roles retain R-024's typed constant defaults and FALSE presence bits; a present invalid role never receives a default. Those selected-family defaults are charged resources, and absent normal sampling/TBN stays bypassed. They do not authorize dummy direct/E/G resources or any allocation for an unselected family.

### 3.2 Exactly eight direct/environment/GI presence layouts

D/E/G are explicit, validated source presence facts. D and its counts must agree: zero direct counts have no direct resources; present D requires the admitted nonzero source set. E/G presence is independent. No variant may reference an unbound resource.

| D | E | G | Exact group3 bindings | PBR position packing |
|---:|---:|---:|---|---|
| 0 | 0 | 0 | Group3 absent entirely | Ordinary HAL scene units |
| 0 | 0 | 1 | b6 only; no sampled E assets | Ordinary HAL scene units |
| 0 | 1 | 0 | b6–10; all six GI lobes zero | Ordinary HAL scene units |
| 0 | 1 | 1 | b6–10 | Ordinary HAL scene units |
| 1 | 0 | 0 | b0–5 | Common metric render-relative |
| 1 | 0 | 1 | b0–6; b7–10 absent | Common metric render-relative |
| 1 | 1 | 0 | b0–10; all six GI lobes zero | Common metric render-relative |
| 1 | 1 | 1 | b0–10 | Common metric render-relative |

G-only never binds dummy E textures; direct-absent never binds dummy b0–5. The tone pass remains a SEPARATE controlled pipeline/layout: original group0/b0 ToneMapUniform16, then b1=R,b2=G,b3=B,b4=Coverage R32Float views, all exact single- or multisampled unfiltered Float textures, no sampler. It consumes no fifth material group and changes no light/View80/Object112/Object128/Material64 input ABI; only private scene fragment outputs and tone loads change.

## 4. B-02: complete immutable input, revisions and writer admission

**Adopted / Binding [S1] — 2026-11-03: qualification of §4.** Complete source/model/material/role/texture/mip/sampler/frame/certificate/D/E/G/HDR generations and original-versus-derived meaning SHALL remain exact, immutable and leased. Full source validation occurs when its actual dependencies change; independently checked still-live captures may be reused only with authenticated source/Context/generation/lease association. A changed FOV is not a new source generation and performs no unchanged-source texel/certificate scan.

Every current request retains actual view/target/serial and writer association. This source proof does not prove a current target, GPU execution or front. It SHALL NOT require a new per-view GPU vertex/triangle numerical certificate. Unsupported source/writer/profile classes and linked exclusion are unchanged.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

### 4.1 Closed complete source association

The checked request contains the following complete immutable facts and leases, not partial setters or an independently mutable light collection. This is a content contract, not invented Rust types, methods or public signatures.

| Required association | Qualified owner / rule |
|---|---|
| Source identity, source revision, extension-source revision, selected profile/presence and tagged packets | Explicit application source plus checked 3D admission; no guessed epoch or identity from a pointer/image |
| Structural snapshot and complete ModelState, view descriptor/revision, sample1 or4 | Original HAL-355 structural/model/view identities and ordering laws remain individually authoritative |
| Material factors/classification/cutoff, exact original/derived frame mode and UV/normal/tangent/certificate evidence, role encoding, texture/sampler/scale generations and complete leases | R-024 §§4/7/8; V2 additionally captures R-028's exact complete original stored chains, authored sampling and real common-UV response-corner certificates |
| HAL WORLD axes and original-to-world source mapping; k when D present | Attributable source facts, checked qualification, §5; no hidden rotation/unit uniform |
| D counts/order/descriptors, full geometry/models/masks and shadow casters including off-camera contributors | R-025 source completeness and coverage; no camera-only caster list |
| E irradiance/specular/LUT identities and integration evidence; G world GI6, patch/transport/bake dependencies | Independent producer under R-026; no shader reorientation, foreign-G6 relabel or stale bake |
| Exposure and complete finite nonnegative<=60000 scene radiance certificate | R-027; combines actual material/direct/emission/E/G and their numerical domains, not a light cap alone |
| Context/Device/participant identity, target association, sample/format/pipeline class, capabilities and all budgets/leases | Existing resource and renderer owners; the actual final target metrics are supplied ONLY by HAL-266A |

Publication during existing Update freezes source intent. Preparation attaches/validates the actual renderer-owned target compatibility key and resources in the existing requested/prepared/committed model; it cannot let the caller invent final W/H, DPR or SurfaceId metrics, fetch new semantic content post-Update, call the source back, or use a CPU/GPU readback as source authority.

The 2026-11-01 coordinated source law adds immutable producer facts to this SAME association, not a new registry, shader interface or public source API:

- Each mapped original triangle declares exactly A (old UNIT/.75 sufficient cones), B AuthoredSmoothFrame (actual packed all9 unit-b projections>=1/16), or C SourceGeometricFrame under R-024 §7.1. C requires a source-proven original smooth singular situation and explicit source choice, not exhausted B search or automatic fallback. Genuine normal absence instead has the old normal cone or unit-d projection>=1/16 and bypasses T/TBN.
- Retain exact original local POSITION/NORMAL/UV0/indices, ALL original signed Mikk corner results, source/node/triangle/corner correspondence and mode/revision/meaning. C intentionally packs a different shading Fp/Up/H from original local POSITION+UV, with the SAME certified values at all three render corners, explicit UV-derived sign and bounded derivation/packing proof. Original semantic mesh and derived render-frame identity remain distinguishable; source-producer and inspector provenance discloses the changed shading and potential faceting. No original asset/JPEG/Mikk repair, renderer inference, topology/UV change or claim of unchanged smooth appearance is admitted.
- Retain exact original normal-role bytes/levels, source sampler/all6 MIN/independent MAG/U/V and scale. Old all-level BLUE>=128 is sufficient, not universally necessary. Otherwise capture R-028 §6's complete finite SAME-UV joint-level/address/LOD cells, REAL response corners, unit-h outward lower projections>=1/510, witnesses/coverage/work/digest and exact source/role/sampler/scale generations. Arbitrary raw-eight-texel hull success is sufficient; its failure is not an actual-domain failure.
- Each candidate must pass independent source/certificate/association and packed/host representability checks, with lawful reuse of checked immutable captures/generations/leases. GPU acceptance uses R024 §8.2.4 and §7 representative observable conformance for the declared input domain/backend envelope, not exact fixture provenance as a runtime key or hidden arithmetic telemetry. S2 may execute separately authorized proof cases; required predictions/comparators precede execution. No per-fragment hint, replacement normal, production solver/readback or new trust mechanism follows. All work/storage/expanded corners/retained generations remain charged; invalid source or exhausted budget refuses without partial publication.

R-024 owns complete frame/handedness/old-domain and numerical proofs; R-028 owns the shared-UV multi-affine Bernstein proof and unchanged generation/filter law. This parent captures those exact adopted contracts without duplicating or altering BRDF, shader, group3, shadows, E/G or HDR algorithms.

### 4.2 Independent revision laws

Structural, model and view revisions retain HAL-355's own monotone nonwrapping content-equality/admission rules. The independent extension-source revision is also immutable and nonwrapping. Changed ancillary source/profile/presence, light, E/G, calibration or exposure changes that complete extension record and requested serial; it cannot relabel an old requested/front ID or hide a change behind an unchanged captured revision. Equal captured revision must never name different extension content; stale/mismatched/exhausted identity refuses under its actual owner.

Material factor/binding/profile changes that alter structural packet content still require R-024/HAL-355 material and structural successors as well as the complete extension association. Conversely a light/exposure-only change does NOT falsely rewrite otherwise unchanged structural/model/view revisions. Model-only and view-only changes remain their real complete successors. Resource generations are context/kind-qualified and never rebound to newer slot contents through an old lease.

A changed per-triangle frame interpretation, original-to-render corner map, packed data or normal-domain certificate changes its actual owning geometry/material/structural/full extension association. Even identical output bytes do not erase a changed source meaning. Do not equate independent D/E/G/HDR revision numbers. Requalify R-025 actual shading/radiance and source association, R-026 affected G transport/currentness and complete R-027<=60000 radiance for the altered interpretation; unchanged E world assets may be reused only under their true unchanged dependencies. No automatic G rebake or old-front relabel is permitted.

Requested, prepared, committed, enqueue, GPU completion and actual display remain distinct stages. The committed record identifies the exact complete tuple and target that produced its front, including original light/material/E/G/exposure/calibration and resource generations. A compatible old front remains wholly old; no partial source update can make it current.

### 4.3 First-version writer scope: unlinked ordinary flat source only

After separate S1/S2 admission, the first version permits direct checked publication ONLY by an explicitly UNLINKED caller owning an ordinary flat Viewport3D source. The existing HAL-354 mounted G1 tuple is not widened. A linked target SHALL explicitly refuse the extended request as unsupported under its exclusive-writer boundary before modifying requested, committed or target state. It must not strip new fields, tunnel them through ModelState, impersonate a cutoff or bypass the canonical writer, including for an otherwise equal request.

Existing linked G1 and ordinary unlinked legacy clients keep their complete original functions/identities. A future mounted extended transfer needs separate exact HAL-354/HAL-266A owner admission; this S1 contract supplies none. Optional HAL-357 may produce already-qualified flat geometry/world placements before this first-version boundary; it supplies no renderer hierarchy, new semantic owner or automatic extended composition/mounted transport API.

## 5. B-02: explicit D-only metric packing exception

### 5.1 Common checked metric working space

**Adopted / Binding [S1] — 2026-11-03: qualification of §§5.1–5.3.** Preserve the exact positive-k metric packing exception, original model/normal/world meaning and ideal projection identities. Check actual host calibration/origin subtraction, matrix construction/inversion/packing, finite representability and required source associations when those inputs change. Actual R025 host geometry/cascade/depth/coverage work remains checked and current.

“Host representability” SHALL NOT impose an extra whole-source per-vertex simulation of GPU object/projection/clip operations for each FOV. GPU transform/clip/interpolation consequences use R024 §8.2.4's adopted 2026-11-03 boundary and the supported observable backend envelope. This changes no metric constant, normal/frame/evaluator, D-absent packing or shadow algorithm.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

For D present, k=meters_per_scene_unit is authored, finite and strictly positive. HAL-355's right-handed WORLD orientation/handedness and chosen f64 render_origin_world remain unchanged; world positions/origin are in source scene units. The fixed PBR family CPU preparation performs the following common conversion through checked f64 operations BEFORE checked f32 storage:

```text
L_scene = R(q) * diag(scale)
L_metric = k * R(q) * diag(scale)
t_metric = k * (t_world - render_origin_world)
normal_metric = transpose(inverse(L_metric))
camera_relative_metric = k * (camera_world - render_origin_world)
```

PBR ObjectBlock112 stores L_metric and t_metric in model_relative and the TRUE inverse-transpose normal_metric in normal_to_world at their original offsets/types. PBR ViewBlock80 stores camera_relative_metric; view_projection derives from that same camera orientation/metric translation and metric-calibrated perspective near/far or orthographic near/far AND vertical_span. Vertical FOV and aspect do not change. No independent arbitrary raw matrix, extra Y reflection or k in a reserved zero slot is admitted.

Every point/spot, caster and receiver position used together for attenuation/shadowing is in this SAME metric render-relative space. R-025 r/R, near0.01m, directional shadow limit and other metre-valued domains remain metres, not double-calibrated values. Shadow cameras/coverage use the same converted source. Cube references still use metric face-axis d_major, never radial depth. Source geometric normals/directions, E/G WORLD signed axes, winding and reverse-Z stay unchanged.

Retain original world frame/mapping and k. Check host subtraction/products, proper invertibility, transform/normal/scale, source-frame/common-UV certificates and actual packed-matrix representability before GPU work. Host clip/enclosure calculations establish only their modeled host predicates, not actual GPU multiply/clip precision. GPU consequences use R024 §8.2.4 available observable comparisons within the accepted input domain/backend envelope. Positive k alone proves neither absolute scale nor finite reciprocal/storage; actual host/capability failure refuses before writes.

### 5.2 Local-frame and projection preservation in ideal arithmetic

R-024 fragment-local interpolation and Gram–Schmidt remain BEFORE either world transform. For its locally orthogonal n,g and invertible L_metric:

```text
(L_metric^-T * n) dot (L_metric * g) = n dot g = 0
L_metric^-T * n = (1/k) * L_scene^-T * n
L_metric * g = k * L_scene * g
```

Thus positive uniform k leaves normalized world N/T and E/G query directions unchanged for EACH SELECTED source frame in ideal arithmetic. Mode A retains its .75 cones and ideal |g|>=3/32; B retains its certified packed1/16 cross and delta_F/M_N GS bound; C has ideal constant F/U and separately certified actual packed Fp/Up. The normal BLUE>=128 route retains its sufficient1/255 proof; R-028's complete common-UV certificate provides the selected1/510 alternative. Positive scale ratio<=64, original normal_scale and .045 remain exact. This does not claim C equals original smooth shading or prove f32/backend behavior.

With the camera translation and perspective near/far scaled consistently, perspective clip coordinates scale together, leaving NDC/depth unchanged ideally. Orthographic metric near/far/vertical_span cancel the common positional k. Actual projection/depth, off-camera coverage, cube lookup and normalized-vector consequences require R024 §8.2.4 representative observable evidence for the declared input domain/backend envelope; source/IR inspection or final pixels do not directly measure hidden GPU intermediate precision. Valid in-domain view/target changes require ordinary candidate checks, not a new S3 oracle per frame.

### 5.3 Strict family restriction and non-D behavior

Every color/shadow packet in a D-enabled request must use the new PbrMaterial family for positional metric shading. Mixed LegacyAppearance geometry in that request is UNSUPPORTED: refuse the complete request, never silently convert old appearance semantics or render it in a mismatched domain. This restricts only the NEW D-enabled family; no legacy client or ordinary2D path is migrated or disabled.

For D=0, keep ordinary HAL scene-unit View/Object packing. E world directions and G WORLD GI6 are sampled with R-024's final checked world N/V, with no metric transform. Physical calibration needed by a G producer remains attributable transport evidence, not a hidden GPU k. A change that invalidates G source transport needs newly qualified producer data; it does not automatically rotate distant E or rebake G.

This is a NEW opt-in numeric-packing exception to HAL-355 §§6.1/9.1, not a claim of byte-for-byte identical matrix VALUES. View80/Object112 FIELD TYPES, OFFSETS and PURPOSES remain; their D-enabled PBR values differ by the explicit convention above. R-024 §8.4.1's byte-layout/field-purpose preservation is not permission to infer that metric values already exist. The separately adopted D packing extension must be stated alongside the unchanged material ABI; baseline layout/stages, D=0 convention and legacy numeric semantics remain exact.

## 6. B-03: one private linear-FP16 final owner

The selected HDR path remains R-027, without a new Image ingress or output owner:

```text
qualified complete source + actual HAL-266A target W,H,N
  -> ONE PBR/depth pass: private R32Float R/G/B/Coverage[N] + common Depth32Float[N], N=1 or4
  -> controlled per-sample textureLoad tone pass (NO scene hardware resolve)
  -> private Rgba16Float FINAL back[1]
  -> same captured Queue writer submit(s) return
  -> existing CustomRenderPass swap
  -> existing low-level Image front registration
  -> atomic complete committed association
  -> ordinary2D composition / unchanged native surface gpu.format
```

Scene clear remains(0,0,0,0); the unchanged algorithm writes intended nonnegative finite straight RGB and alpha1 for surviving Opaque/Mask samples by REPLACE, never blending/A2C. R027 §4.1 retains a rigorous conservative FULL ideal source/evaluator-domain certificate for every possible covered component of the complete material/direct/emission/E/G/model/view request: finite, >=0 and <=60000. It is not an average, GPU-f32 precision proof or readback estimate. Actual execution consequences require R024 §8.2.4 predeclared observable FP16/Image or genuine probe comparisons in the declared envelope. EV remains[-16,+16], e=2^EV checked positive finite and ToneMapUniform16 retains e/three zeros; no autoexposure.

The two exact tone pipeline/layout classes use FOUR texture_2d<f32> inputs for N1 or FOUR texture_multisampled_2d<f32> inputs for N4, fixed b1=R,b2=G,b3=B,b4=Coverage. Reconstruct logical RGBA from .x of ALL FOUR textureLoad results at the SAME physical in-bounds pixel and sample index; N4 uses0,1,2,3 separately. R32Float unused returned components do not supply alpha. No sampler, filtering feature, hardware HDR resolve or cross-plane/sample/generation mixing. Only Coverage1 admits original RGB to the unchanged tone; average colors and binary coverage only after per-sample tone.

```text
T_e(x) = e*x/(1+e*x)
final.rgb = sum_over_covered_samples(T_e(scene_rgb_i)) / N
final.a = covered_sample_count / N
```

Tone occurs BEFORE averaging. For N4 with one covered(4,2,1), e1, final=(1/5,1/6,1/8,1/4); tone-after-resolve would be wrong. FINAL front AND back are fixed LINEAR Rgba16Float, sample1,8 bytes/texel, not surface-dependent or sRGB. Coverage alpha is exactly representable in ideal binary16; monotone nearest conversion preserves RGB<=alpha and convex filtering preserves ideal premultiplication. Actual shader/f32/FP16/filter/Image consequences require R024 §8.2.4 available observable comparisons for the supported input domain/backend envelope; no hidden arithmetic is inferred measured or universal pointwise GPU precision asserted. Essential additional numerical bounds remain OPEN for selection by the delegated architectural reviewer before execution.

### 6.1 Binding MRT4 representation and numerical precedence — 2026-11-06

The preceding retained paragraph's nearest-conversion statement is an IDEAL conditional model, not a universal hardware final-conversion guarantee. R027 §§7.3–7.4 now govern ACTUAL numeric qualification: exact ideal tone and exact actual alpha=k/N, zero-alpha zeroRGB, finite nonnegative RGB<=alpha remain mandatory with no epsilon. For the controlled2^-26/EV16 versus2^-24/EV14 N1/N4 oracle at every k, require bit-identical paired actual arrays and <=2^-20 absolute ideal RGB error; the expressly listed high/mixed/black/late-depth stress fixtures use <=2^-11. The measured N4 two-near/two-far tone-control0.5004878044128418 produced final0.5 despite mathematical nearest half0.50048828125; it is within2^-11, not2^-20. Neither bound automatically qualifies the entire source domain/backend. Earlier OPEN additional-bound wording is superseded ONLY for these selected fixtures; wider domain/backend acceptance still requires the delegated architectural reviewer's declared comparison rule and independent qualification under the preserved R024/HAL370 trust boundary. No new source floor, tolerance-based alpha repair or arbitrary executor bound.

All four scene planes form one immutable target-local source/Context/resource-generation association, cleared0, written and depth-tested together in ONE existing PBR scene pass. Original finite0<=L<=60000 and source/evaluator/certificate/geometry/material/normal/light identities remain unchanged, with component-wise identity transport of computed source-linear f32 and no universal exact-real/subnormal claim. No gain, pre-exposure, emissive floor, source rewrite or extra scene pass per channel. Existing complete-caster depth-only shadows and HAL372 derivative-valid coverage/Mask behavior stay unchanged. Current private writer/tone/target keys distinguish MRT4 R32Float/order/N/b1..b4 from old FP16 storage and legacy pipelines; no stale old scene is relabeled. Exposure-only re-tone requires the exact complete live bundle plus real new tone/commit. Resize/source/cancel/device closure, compatible original last-good and queue-safe retirement remain owned by the same existing participant.

The final-format selector is PRIVATE to the already-admitted 3D participant's existing CustomRenderPass resource/configuration owner under this S1 contract; its engineering and proof remain gated. No public Image/renderer format selector, second publisher, raw texture escape or generic2D tone pass is granted. HAL-263A's existing preassociated LOW-LEVEL ingress receives the filterable FP16 final without conversion or extra gamma/alpha association; the ordinary compositor owns eventual native surface encoding. The HDR scene is never an Image front.

Before allocation/writes, require actual enabled/guaranteed R32Float render+unfiltered texture-load N1/N4, at least4 color attachments/16 color bytes per sample/4 tone texture bindings, common Depth32Float[N], unchanged final Rgba16Float sample1 render/binding/FILTERABLE Image view/sampler, physical dimensions/limits and complete reservations. No additional Device feature is requested; adapter-only flags are not enabled capability. Unsupported capability or capacity refuses with truthful compatible last-good, no format/quality/count fallback, sample4-to1 downgrade, alpha repair or changed native surface/generic Image path.

## 7. Architecture, engineering and capability proof stages

**Adopted / Binding [S1] — 2026-11-03: qualification of §7 stages.** S1 remains documentary authority, S2 a separately authorized complete implementation/proof unit, S3 independent acceptance of a versioned supported source/backend class, and S4 native/quality/performance/JC acceptance. Exact source meaning and actual host/Device/target correctness remain mandatory. CPU universal pointwise GPU-arithmetic simulation is not a product correctness promise or S2/S3 requirement.

S3 still needs controlled derivative-valid WGSL and actual capabilities plus representative predeclared observable N1/N4 clipping, near/helper-sensitive UV/LOD, Mask, A/B/C and shadow cases and the unchanged original three-asset external-consumer FP16/Image conformance fixture. Keep existing authorized criteria, residual variance, compatibility/exclusions and R028 §12. No invented precision bound, hidden-intermediate claim, per-frame S3 replay/readback or known failing backend acceptance follows. This S1 adoption establishes the documentary contract only; it establishes no S2 execution or S3/S4 conformance.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

These are separate owner decisions/evidence states, not UI phases, callbacks or an automatic permission ladder. R024 §8.2.4 and R028 §§13/15 require rigorous source/host checks plus bounded observable GPU conformance. S3 accepts a declared versioned source input domain/backend capability envelope supported by representative actual tests and the mandatory unchanged original three-asset external-consumer conformance fixture at N1 and N4, not a frozen source/frame/device-metadata tuple. Exact test provenance, runtime per-candidate guards and accepted domain/envelope are distinct.

| Stage | Required decision/evidence | What it DOES NOT establish |
|---|---|---|
| S0 | Historical stage: architecture and later amendment candidates selected and recorded as inactive drafts before their separate attributable S1 decisions | No binding change, engineering grant, altered refusal or backend/public support. |
| S1 | Attributable coordinated independent adoption of exact parent/feature text under JC's delegation/veto. Original source-domain and any later numerical-evidence amendment remain separately attributable decisions. | No source implementation, Device execution, automatic S2 work or reopening of S3/S4. |
| S2 | Separately authorized COMPLETE bounded implementation/evidence unit with rigorous source/host/certificate/packed-byte/resource/capability/ABI checks. Predeclare representative fixture/prediction and authorized falsifiable comparison using available FP16/Image, genuine probe or coverage/Device outputs. An essential additional numeric comparator stays OPEN until the delegated architectural reviewer selects the bound before execution; no hidden internal measurement is inferred. An authorized external API candidate remains S2 until S3. | No mock/fallback, source/role change, unselected tolerance, replacement shader, false hidden telemetry, self-selected contract or universal GPU proof. Candidate visibility is not public acceptance. |
| S3 | Independent acceptance of an explicitly versioned supported SOURCE INPUT DOMAIN and BACKEND CAPABILITY ENVELOPE: rigorous per-candidate guards; actual capabilities/controlled pinned compiler/ABI; representative predeclared observable N1/N4 adversarial cases; the mandatory unchanged original three-asset external-consumer conformance fixture through public production PBR to FP16/Image; authorized output comparisons, unchanged R028 §12 numeric metrics and full lifecycle evidence. Preserve exact assets/camera/device/backend/driver/source/target as evidence provenance, while declaring capability-family/platform/driver compatibility, regression coverage and exclusions for runtime use. | No universal per-fragment precision, direct hidden N/T/g/helper measurement from final pixels, out-of-domain/unsupported/failing class or full-family readiness from one component; no per-frame replay/S3 review for valid in-domain dynamic inputs; no native/performance/JC acceptance. |
| S4 | Actual canonical App::run N1/N4, complete source→shadow/scene→tone/FP16→Image→native trace, current-target/original-front/lifetime/refusal/no-use, unchanged native/quality/performance gates and independent JC acceptance. Dynamic requests remain subject to their ordinary source/host/capability/target checks within the accepted domain/envelope. | No broader product/physical truth, unsupported backend class or universal numeric guarantee from those observations. |

S2 permits separately authorized evidence-producing GPU tests before S3. Prewrite source/host/certificate/format/capability/target/resource checks still apply, including valid immutable checked-capture reuse. S3 requires independent acceptance of the declared input domain/backend envelope supported by representative actual tests and the unchanged original three-asset external-consumer conformance fixture, not per-frame owner approval. Within it, valid source/geometry/material, camera motion, view and target changes use ordinary checked successors/publication; they do not replay the entire oracle. The declared platform/driver compatibility family and regression coverage govern metadata changes; known failing drivers or changes outside the accepted class are not silently supported.

GPU-owned fixed-function/fragment numerics retain explicit residual variance under R024 §8.2.4. Require bounded observable conformance where universal quantitative vendor contracts are absent. Each case must predeclare an independent prediction, discriminating available output and authorized comparison; final pixels do not directly expose N/T/g, arithmetic intermediates, coefficients or helper execution. Missing discrimination is UNVERIFIED; additional essential comparison bounds stay OPEN for selection by the delegated architectural reviewer in a separate S2 authorization before execution, with no .03/.05 reuse for frame/normal/GS. Known-invalid source/host algebra, malformed certificates, stale/foreign identities, unsupported capabilities and exhausted resources retain prewrite refusal. A failed modeled GPU enclosure alone is not permanent source invalidity; implementation still needs its own grant.

No executor may change UNIT/.75 or BLUE128 routes, ideal3/32, packed1/16, normal1/510, scale64, normal_scale1024, roughness.045, light/shadow equations, complete FULL ideal source/evaluator-domain radiance<=60000, FP16/Image semantics or R028 §12 thresholds to obtain a pass. The radiance certificate covers every possible covered ideal value, never an average or universal GPU-f32 claim. Preserve original oracle assets/camera, fixed shader, old V1/legacy/2D/G1 and source/front rules; no triangle skip or repair. Ordinary valid dynamic inputs within an independently accepted domain remain lawful. Old S2 receipts retain their evidence scope; this candidate grants no implementation or S3/S4 acceptance.

## 8. Exact binding opt-in parent-delta and no-change matrix

**Adopted / Binding [S1] — 2026-11-03: qualification of §8's source/host/numeric cross-references.** Matrix requirements referring to R024/R028 numerical qualification SHALL use the adopted immutable-source, actual-host and observable-GPU split in R024 §8.2.4, R028 §§8/13/15 and this document's §§9–11. No row implicitly requires a proof-only per-FOV vertex/triangle sweep or a replacement frame certificate. HAL355 retained resources, actual host packing, complete source semantics, HAL002 phases, R025 actual shadows/coverage, R026, R027 and all existing ABI/writer/target/publication boundaries remain unchanged.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

Every DELTA below is adopted at S1 ONLY for the expressly selected optional family, never the baseline. HAL-355 §17.1 acknowledges these exact exceptions and their limited precedence; its baseline text remains unchanged. HAL-002/002A/354/357/266A/295A/263A/309 are not amended. The same-owner offscreen/linear-preassociated Image path stays within HAL-295A/266A/263A, so no new ingress or amendment of those parents is granted or needed for this selected boundary. Linked G1 remains excluded from extended requests. No row grants arbitrary geometry/GPU/light behavior or S2 engineering.

| Controlling owner / clause | Precise S1 DELTA or mandatory preserved boundary |
|---|---|
| HAL-355 §3.5 | Extend the complete flat structural snapshot with exactly the tagged shading/material association and retain its complete ModelState against the same structural identity/packet count. No new scene hierarchy, partial placement stream or semantic owner. |
| HAL-355 §3.6 | Admit only the named new typed resource kinds with the existing context/kind/slot/nonwrapping-generation and lease ownership discipline; every snapshot/pipeline/view/committed or in-flight user keeps the exact generation live. No slot rebinding through an old lease. |
| HAL-355 §3.7 | Extend requested/committed records with §4's complete immutable extension source/revision, profile/light/E/G/exposure/calibration and resource associations. Preserve independent base revisions, requested/committed observation and original-front provenance; no partial tuple or old request-ID relabel. |
| HAL-355 §3.8 | Qualify new profile/scene/final format/sample/resource classes in the current per-target compatibility key; renderer still supplies actual metrics. No caller-chosen extent or incompatible old front sampling. |
| HAL-355 §6 | Add ONLY §5's D-enabled PBR common metric packing under checked positive k; retain right-handed world, checked TRS/precision, winding, camera orientation and reverse-Z. D=0 and legacy scene units unchanged. |
| HAL-355 §7 | Admit only the named typed material/light/E/G/mip/float resource generations and finite capacities within the existing Context3D Device/Queue/lease owner; no public raw resource service or source decoder. |
| HAL-355 §8.1 | Add opt-in52-byte tangent stream and selected A/B/C or genuine-absence source/packed/UV/sign certificate; legacy36-byte stream unchanged. D present ONLY additionally admits §3.1/R-025's stride12 source-winding face-normal stream and checked deindexed two-buffer GPU expansion; D=0 primary52 packing/cache is unchanged. No renderer-synthesized tangent/UV/default source presence; explicitly producer-authored certified C remains the declared source interpretation in R-024 §7.1. |
| HAL-355 §8.2 | Preserve checked placements; apply R-024 positive-scale/selected-frame/common-UV-certificate/local-first obligations and §5's explicit calibrated working-space values, with actual representability proof before dispatch. |
| HAL-355 §8.3 | Admit tagged LegacyAppearance OR immutable PbrMaterial and affected structural successors; material/texture/sampler generations remain source-qualified and leased. D-enabled mixed legacy request refuses. |
| HAL-355 §8.4 | Extend full source admission/identity to the closed complete §4 content, independent extension revision and writer scope. No ancillary updates smuggled into old structural/model/view identities. |
| HAL-355 §9.1 | Add separate fixed PBR groups/material roles, PBR Object fragment visibility, conditional group3 and the EXPLICIT D-only numeric packing exception; old80/112/16 layouts/stages/values remain the baseline. |
| HAL-355 §9.2 | Add fixed PBR local-normal/local-tangent varyings and local-first fragment basis order. D present ONLY adds §3.1/R-025's matching flat geometric-normal location5, giving six user locations/19 scalars; the original centroid0/1/3/4 and center2 qualifiers remain exact. D=0 stays five locations/16 scalars. Existing legacy vertex/wrapper/world-normal interface remains exact. |
| HAL-355 §9.3 | Admit the fixed R-024 evaluator, R-025 direct, R-026 qualified E/G and R-027 bounded scene color/tone as their distinct controlled opt-in stages; do not silently remove legacy clamp/alpha1 or add general transparency. |
| HAL-355 §9.4 | Controlled Halcyon-owned shader/IR validation for the exact family/variants; no new public application WGSL globals/entry/bind-layout escape. Actual pinned validation required. |
| HAL-355 §9.5 | Add distinct family/ABI/profile/presence/fixed-shader/raster/target/depth/sample pipeline identities. No old-pipeline migration or material-identity-as-variant churn. |
| HAL-355 §10.1 | Exact profile-specific FOUR R32Float scene planes R/G/B/Coverage with common Depth32Float[N] and unchanged linear Rgba16Float[1] final pair; same private owner and actual target extent. Legacy surface-compatible pair unchanged. |
| HAL-355 §10.2 | Preserve exact HAL-266A physical extent/activity and incompatible-front withdrawal; inactive/zero targets allocate/render nothing. New resource classes remain target-local; no independent DPR/extent calculation or preparation phase. |
| HAL-355 §10.3 | Add ONLY R027 bounded source-linear MRT4 storage, same-sample tone-before-average and unchanged FP16 final. Preserve zero clear, binary covered samples, original depth/Mask/REPLACE, white Image and exact association; no HDR resolve, translucency, gain, Image repair or new feature. Conditional ideal nearest and actual finite numerical gates are in R027 §§7.3–7.4. |
| HAL-355 §10.4 | Extend the existing per-front ordered transaction to shadow/scene/tone/final writers on the SAME Queue before existing swap/registration/atomic commit. No global submission scheduler or GPU-completion fiction. |
| HAL-355 §10.5 | Include actual extension-source/calibration/light/E/G/exposure changes in dirtiness/dependency qualification. Equal complete inputs and stable target reuse the original compatible front with no work; no hidden uniform writes, uploads, encodes or registrations. |
| HAL-355 §11 | Commit only the whole extended requested/prepared/source/target tuple and leases. Failure keeps original compatible last-good; incompatible/retired target withdraws. Never fresh material/light with old shadow/bake or old-front relabel. |
| HAL-355 §12 | Add exact typed source/profile/metric/writer/capability failure causes under their real owners, without inventing Rust enum variants here. Refusal is not success/absence/default or a fragment-to-CPU error callback. |
| HAL-355 §13 | Charge every new logical/actual allocation, full source chain, fixed pool, slot, in-flight/old generation, compiler/pipeline and bounded pass/draw/tap work using effective finite limits; no guessed global capacity. |
| HAL-355 §14 | Capability/portability evidence is per actual pinned Device/backend/format/sample/limits; no browser/new-feature support from API names or draft arithmetic. |
| HAL-355 §15 | Keep all source/phase/2D/private-GPU/false-publication/unlimited-budget/private-scheduler prohibitions. Only the expressly listed family interfaces receive S1 exceptions to the baseline feature/ABI/format exclusions. |
| HAL-355 §17 | Narrowly admit only these named optional material/direct/E/G/HDR/mip feature contracts at S1; importer, runtime GI/physics/camera/new renderer/translucency/nonlisted features remain excluded. |
| HAL-354 §4.1 and mounted G1 | NO-CHANGE CHECK: one Update and exact exclusive mounted transfer stay binding. This first version refuses linked extended requests; a future tuple/cutoff extension requires separate HAL-354/HAL-266A owner adoption. |
| HAL-357 §§2/4/6–8 | NO-CHANGE CHECK: optional checked producer lowering and association remain mechanical, complete and flat; any new typed content transport requires its exact owner admission. No domain traversal or automatic extended API. |
| HAL-266A §§3–6, INV-014 | NO-CHANGE CHECK / compatibility review: actual current immutable target, indexed post-layout/pre-Paint GPU-local preparation and already-published input only. No semantic callback, target authoring, Timeline or G1 bypass. |
| HAL-295A §§3.2–3.4, INV-008 | Coordinate the existing participant's PRIVATE per-profile final-format configuration, never a new publisher. Existing admission/privacy/Device/Queue/enqueue/swap/registration/unregister/retirement authority remains exact. |
| HAL-263A §3.2, INV-002/004/005/006/008 | NO NEW INGRESS: existing low-level registration takes already premultiplied-linear filterable FP16 final. No normalization, second alpha, shader gamma or native surface-format change. Existing exact Image modulation remains. |
| HAL-309 §2 | NO-CHANGE CHECK: historical draft intent, S1 binding adoption, source implementation and executed proof are independent. S1 is recorded from the express delegated decision, not inferred from drafting or source presence. |
| HAL-002/002A | NO-CHANGE CHECK: sole WidgetTree/ID/phase/snapshot/sovereignty and amendment supremacy. No constitutional exception, extra UI phase or executor-created authority. |

The original2026-10-31 parent/feature adoption retained the R-023–028 technical bodies at that time. The2026-11-01 coordinated Revision02 now replaces ONLY R-024/R-028 source eligibility and corresponding HAL-370 source/proof references with the explicit frame/domain certificates above. Historical receipts retain their original scope. Unlisted equations/byte layouts/roles/owner algorithms remain unchanged; this is a three-document subordinate source-law update, not another parent exception or an engineering/public capability grant.

Numerical evidence references use R024 §8.2.4, R028 §§13/15 and HAL370 §7: rigorous per-candidate source/host guards plus representative predeclared available observable tests for an explicitly versioned input domain/backend capability envelope, with exact test provenance recorded separately from runtime admission. Valid in-domain dynamic inputs need no per-frame oracle/S3 replay; platform/driver compatibility follows declared capability-family regression coverage/exclusions. No hidden-intermediate measurement, unselected comparator or universal GPU guarantee is implied. This changes no HAL355 legacy behavior, HAL002 ownership/command determinism, R025/R026/R027 algorithm or FULL ideal radiance certificate, Image association or resource/publication guarantee.

### 8.1 Binding HAL381 named private presentation ABI — 2026-11-06 [S1]

Independent architectural review under JC's existing technical delegation and retained constitutional/product veto admits [HAL381](SPEC-HAL-381-Source-Qualified-Loading-Presence-and-Materialization.md) ONLY as the coordinated HAL355 §17.2/R027 §6.4 private Viewport presentation extension. This adds no source material/light/normal family, generic transparency, independent publisher, Device feature or default migration. Existing R024/R025/R026/R027/R028 source/numeric proofs and all original ABI/profile keys remain unchanged.

The exact new private preview ABI is Style64 group0/b0, original semantic View80 group0/b1, original semantic Object112 group1/b0, triangle-list36-byte p/e01/e02 vertices, flat actual transformed geometric normal, and HAL381's separate fixed waiting/ready Compose32 layouts. It renders proxy color Rgba16Float[N] plus Depth32Float[N], and exactly one Rgba16Float[1] presentation target under the same renderer/Context/target owner. Private fractional surface opacity is never canonical PBR coverage. One exclusive Viewport eligibility slot selects opaque G/T or terminal CanonicalOverNeutral reading genuine registered generation-pinned F; the latter emits `(F.rgb+(1-F.alpha)*B,1)` before inherited clipping ONCE. No B rectangle/second visible Image, canonical Image rewrite, generic Image shader change or fake PBR Committed state is admitted. Distinct private pipeline/layout/content keys cannot alias canonical PBR/HAL372/MRT4.

Qualify source/request/Context/mount/view/target/N/front generation and actual physical extent, real immutable content pin/write exclusion, HAL021/static disposition with the 160 ms default and the sole explicit duration exception in [HAL381 §14](SPEC-HAL-381-Source-Qualified-Loading-Presence-and-Materialization.md#14-explicit-materialization-duration--2026-11-07-adopted), cancellation/re-tone/resize/Device and truthful last-good. Charge `P*(12*N+8)` plus108*T expanded vertices, actual alignment/objects/upload/storage and every old/in-flight generation under the SAME view caps and aggregate. No fallback or cap increase. Actual N1/N4 self-occlusion, registered-front ordering, interior/rounded-edge output and native acceptance remain required separate proof; this adoption supplies no fabricated implementation or physical observation. Other §8 matrix rows and foreign numeric-boundary amendments remain unchanged.

The current private appearance is [HAL381 §13](SPEC-HAL-381-Source-Qualified-Loading-Presence-and-Materialization.md#13-spectral-presence--2026-11-07-adopted-appearance-revision). It uses the same Style64 slot and sizes, with source-local appearance coordinates and one private vertex-to-fragment vec3. The selected stencil reads only the existing proxy attachment. All other binding and resource rules above remain in force.

### 8.2 Binding HAL374 named private Outline profile — 2026-11-06 [S1]

Independent architectural review under JC's delegated technical authority and retained veto admits ONLY [HAL374 §8](SPEC-HAL-374-Source-Qualified-3D-Selection-Outline-and-Halo.md)'s first source-associated selected-visible Outline, coordinated with HAL355 §17.3/R027 §6.5. Explicit immutable occurrence-slot selection binds the exact already admitted PBR scene/snapshot/model and Context generations. Complete original opaque replay writes separate R32Float[N] exact slot+1 tags and Depth32Float[N] under identical view/placement/culling/clip/order and strict Greater. Current R024/R025/R026/R027/R028 and canonical Object112/Object128/Material64/MRT4 profile ABIs are unchanged. Unsupported coverage/discard parity refuses the effect; no fifth canonical output or new Device feature is selected.

Its distinct PRIVATE projected-source-edge pipeline uses the exact2 physical-pixel distance/AA, same-member neighboring winner, selected-union suppression, source-consistent depth and MAX rule; private Rgba16Float[N] stores premultiplied selected stroke, and one Rgba16Float[1] display-space OVER-F output uses actual registered content-pinned canonical F. Source/request/mount/view/target/N/physical-size/front-generation keys and private layout/content identities cannot alias HAL381 or canonical PBR. None bypasses all extra effect work and presents original F. One viewport output/one inherited clip, truthful canonical alpha/Committed/last-good and ordinary generic Image remain unchanged; this is neither HAL381 CanonicalOverNeutral nor general transparency.

Reserve `P*(16*N+8)` EXTRA minimum, actual expanded edge/CPU/uniform/staging/binding/pipeline work and all queue-safe current/pending/old overlap under the SAME view536870912 and aggregate1073741824 attachment caps. No cap increase, N4 downgrade, hidden flag, new source/selection registry, renderer, worker or clock. Existing feature-free Device R32Float N1/N4 render/load capabilities, exact tags0..16777216, replay winner parity, actual per-sample AA/occlusion and no-selection/front/clip/resource/native behavior must be independently qualified. S1 supplies no implementation or physical JC approval. All unrelated matrix clauses and foreign work remain unchanged.

### 8.3 Binding HAL375 first passive PBR HUD — 2026-11-07 [S1]

[HAL375 §9](SPEC-HAL-375-3D-Anchored-Spatial-HUD-and-Information-Connectors.md#9-adopted-first-passive-pbr-hud-profile--2026-11-07-s1), coordinated with HAL355 §17.4 and R027 §6.6, permits the exact canonical Depth32Float[N] sampled-usage variant and private per-anchor binary visibility for the current target/front. This is an optional surface presentation under HAL359/360. It changes no material, scene, original color/depth writer, tone or public Image ABI. The actual Feature-free Device, complete working set, existing limits and queue-safe generations govern admission. No replacement depth pass or fallback is admitted.

## 9. Finite combined admission and resource proof

**Adopted / Binding [S1] — 2026-11-03: replacement of qualification frequency and proof-only cost in §9.** Full original source/certificate/texel verification SHALL occur on genuine dependency changes. Exact independently checked still-live source/Context/generations/leases authorize warm reuse, without a digest-only trust cache or repeated unchanged scans. Actual changed camera/object/view matrices and actual host operations, target/format/samples, Device/capability and finite reservations remain checked. Required current R025 geometry/caster/receiver/cascade/depth/XY/PCF work and actual draws remain intact.

No mandatory production O(vertex count) or O(triangle count) sweep may be performed solely to enclose hypothetical GPU transform/clip/screen-determinant/reciprocal-w/helper/UV-footprint arithmetic on each FOV, including main and shadow-face UV checks. Do not shift the same work into a frame-local certificate, mandatory diagnostic replay or new proof service. Charge all actual retained metadata/source verification, necessary host work, GPU/staging/old/in-flight capacity under the existing ledgers; all resource sizes and limits below remain unchanged.

**Binding future S2 engineering/proof contract; execution requires a separate bounded implementation grant:**

1. Retain the original complete source validators and all source/frame/normal/mip/sampler/radiance meanings. Mechanically prove a new immutable capture is fully checked and exact still-live warm reuse performs zero redundant full source/texel/certificate scans. Source changes and invalid successors preserve original typed failures and the prior accepted association.
2. Remove mandatory proof-only per-view vertex/triangle GPU-model sweeps from the actual producer, including main color and shadow-face UV paths; prove they do not run or reappear as equivalent frame certificates/replay. Optional offline diagnostics must identify modeled host claims separately from actual GPU observations and add no default runtime work.
3. Validate actual changed camera pose/projection/near/far, actual submitted matrices/packing and actual host calculations at their owning boundaries. Prove genuinely invalid/nonfinite host values, stale source/Context/leases, unsupported capability and exhausted finite budgets still refuse before writes. Review each affected old test against its accepted contract: preserve real source/host failures; explicitly replace obsolete proof-only work/refusal expectations with reviewed evidence rather than silently changing assertions.
4. Preserve and verify original R025 source/casters/receiver clipping, cascades, depth/XY/PCF qualification, Mask/Opaque semantics and current three directional shadow faces where selected. Preserve R026 where present and R027 per-sample tone/FP16/ordinary Image. No stale map, altered shader/evaluator/sampler, quality reduction, source/normal repair or changed original asset is allowed.
5. Exercise the existing actual HAL N1/N4 owners and the unchanged original three-asset external-consumer conformance fixture with exact requested/prepared/Committed source/view/target/serial, original typed negatives, current shadow and FP16/Image outputs. Retain source generation, separate SurfaceId, extent/sample, no-use, unchanged-input zero-work, last-good, overlap, Queue completion/registration, retirement and full finite retained GPU/CPU credits. Retain warmed compatible free GPU capacity without structural object churn, refresh current shadow contents, and never overwrite current/old/in-flight resources.
6. Predeclare independently predicted observable clipping, near/helper-sensitive, UV/LOD, Mask, A/B/C/absence and shadow discriminators under authorized criteria for the intended supported S3 class. Controlled IR, actual Device formats/limits/sampling support and representative real Metal N1/N4 evidence are required. No universal unprovided GPU precision interval, guessed epsilon, shader replacement or production readback is admissible; an observed supported-class failure invalidates the affected envelope pending lawful disposition.
7. Preserve every R028 §12 numeric/temporal/native/performance criterion. If CPU effect is measured, record actual profile, source/target/camera, diagnostic mode, all samples/tails and request-to-matched-Committed scope. Distinguish same-binary controlled evidence from cross-run context, diagnostic overhead, actual GPU timing and native presentation. Operation counts or CPU time alone establish no FPS/native speed or quality acceptance.
8. Deliver one complete owner-correct source unit only under that later grant; reconcile held work against the accepted policy before any resumption. No new renderer/worker/publisher/global registry, profile/API/ABI/Device-trust widening, or automatic migration of 2D/G1/legacy/V1 is authorized. Independent S3/S4 decisions and JC veto remain separate.

This is the required future proof boundary, not an executable task or permission to resume implementation now.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

Reserve ALL fallible source/CPU/host/capability/target/resource work before candidate writes. Context budgets and actual Device hard limits both apply; a logical subtotal is not an admitted aggregate memory budget. Use checked arithmetic for extents/products/offsets/alignment, all dynamic slots and complete lifetime multiplicity.

| Selected resource set | Exact minimum logical payload |
|---|---:|
| R-025 active fixed shadow depth pools | 54,525,952 bytes per complete set |
| R-025 uniform payload | 608+1984=2592 bytes per live qualified slot, before actual alignment |
| R-026 complete E generation | 6,406,144 bytes |
| R-026 indirect block | 112 bytes per live draw slot, before actual alignment |
| Five distinct fully mipped1024² RGBA8 material roles | 5×5,592,404=27,962,020 bytes |
| R-027 FOUR R32Float scene[N]+common depth[N]+BOTH final[1] textures | (20*N+16)*W*H bytes; +8*N*W*H over former private scene policy |
| R-0271920×1080,N4 attachment minimum | 199,065,600 bytes |
| Shadow+E+five role chains+that HDR attachment example | 287,959,716 bytes BEFORE other resources |

The revised subtotal is54,525,952+6,406,144+27,962,020+199,065,600=287,959,716, not a capacity promise or an excuse to omit unchanged uniform blocks/alignment, geometry, FOUR scene texture/view objects and tone references/bindings, samplers, staging/uploads, modules/layout/pipeline variants, warmed capacity, old fronts or ALL still-leased current/pending/in-flight/retiring generations. Count P=W*H active target pixels, not4P. Charge shared allocations once under actual identity/leases while retaining every still-live liability; Direct shadow/source/independent texture/single-target/aggregate caps remain unchanged. Checked full preflight precedes allocation/writes; oversized N4 or old+new overlap refuses and preserves only compatible original last-good, never a cap increase, early cancellation release or format/count fallback. Logical payload is not portable driver VRAM size.

In addition, charge exact finite certificate witnesses/cell coverage/metadata, source verification work/storage and any explicit C render-corner duplication with all old/current/pending/in-flight generations. A certificate digest, source bound or one small descriptor is not a substitute for actual complete cost or independently checked inequalities. Warm unchanged captured content performs no repeated source scan, optimizer, certificate search or content upload.

The native four-group interface and all-present10 textures/8 samplers/6 uniforms require actual stage limit/format/filter/binding validation. Shadow work remains at most31 maps with bounded exact occluder draws and nine comparisons per light/sample; E/V2 sampling and per-draw/per-target work remain finite under their owners. No silent fewer-light/shadow, downresolution, truncated chain, role omission, one-sample or8-bit fallback follows from exhaustion.

## 10. Transaction, lifecycle, failure and no-use

**Adopted / Binding [S1] — 2026-11-03: replacement/qualification of §10 step 2 and failure classification.** At the existing current-target preparation boundary, consume the exact still-live checked source capture; validate affected actual submitted host data, required actual host work, full target/Context/Device/capabilities and all finite reservations before writes. A new source requires its original complete validation; a FOV-only change performs no repeated source scan or proof-only GPU vertex/triangle sweep.

Keep steps 1 and 3–5, changed-dependency R025 shadow rendering, scene/tone order, one captured Queue, submit/swap/registration and atomic truthful Committed association unchanged. No current-view source or actual host check is replaced by a previous front or a cached GPU output. An inconclusive predicted GPU interval alone is not source failure or mandatory production work. Actual invalid source/host data, stale generations, exhausted resources, unsupported backend eligibility and observed failures retain their exact causes and compatible original last-good; incompatible/retired targets withdraw.

Equal complete inputs still perform no writes/encode/submit/publication; inactive/absent paths introduce no scans, GPU work or idle service.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

1. The application/source publishes the complete checked immutable unlinked intent under existing Update authority, retaining truthful source revisions and full leases.
2. HAL266A supplies current target metrics. Each candidate must validate the COMPLETE key including color/sample format, linked exclusion, source-mode/packed/common-UV association, host checks, actual capabilities and finite reservations before writes, with valid immutable proof reuse. S3 supports a versioned input domain/backend envelope from representative observable tests; exact evidence camera/target/driver/source is not a per-frame key, and valid dynamic successors do not replay the original oracle. No cache hit admits an unproved certificate or changed source meaning.
3. Encode required changed-dependency shadow writes, scene evaluation and per-sample tone/final writes under the same participant and captured Device/Queue. An unchanged dependency is reusable only with exact identity and live leases; no stale source becomes current by cache hit.
4. Every writer submit for that candidate returns before the existing CustomRenderPass swap; registration publishes that final view through the existing low-level seam; atomically associate the whole committed tuple. After successful registration queueing, that CPU commit is nonallocating and infallible under the existing exclusive owner. If publication queueing can fail, keep the candidate uncommitted and transactionally preserve or restore the prior compatible front/leases as HAL-355 §10.4 requires. Queue ordering, not a CPU GPU wait, provides later sampling visibility.
5. Never write a sampled published front. Keep old/in-flight generations leased, unregister before final drop, and preserve existing context/Device lifetime and per-target retirement. No private recovery epoch or second publisher.

Source/model/material/target/capability/capacity failure keeps an existing compatible last-good with its ORIGINAL tuple and exact status; an incompatible target or retirement withdraws it. Missing/invalid E/G, maps or roles refuse with their own cause instead of being erased, replaced by implicit ambient, or relabeled a successful black output. CPU/source/prepare/submit/register/GPU/native observations never masquerade as one another.

Unselected optional profiles, no Context3D, inactive/zero/retired targets and genuinely absent D/E/G have no unrequested allocation, scan, shader pass, job, upload or idle demand. Inactive-target lifecycle withdrawal is still performed by its existing owner when required; no hidden rendering work is created. Equal complete inputs and stable metrics reuse a compatible front without dynamic writes/encode/submit/swap/registration. No background rebuild, automatic GI rebake, extra clock, semantic post-layout fetch or2D branch is added.

## 11. Paired invariants and prohibitions

**Adopted / Binding [S1] — 2026-11-03: replacements/qualifications for the named §11 pairs.** Unlisted invariants and all existing owners, source/ABI, Queue, Image and lifecycle rules are unchanged.

- **INV-H370-SOURCE / FORBID-H370-SOURCE:** Preserve complete original immutable source meaning, A/B/C/absence, normal/radiance certificates, exact source/Context/generations and leases. Fully validate genuine changes and reuse exact still-live proofs. No unchanged-source full scan on FOV, source repair, digest-only trust or new frame-local GPU-model certificate.
- **INV-H370-METRIC / FORBID-H370-METRIC:** Check actual host positive-k transforms, calibration/origin, submitted camera/object/view/normal matrices and actual necessary host arithmetic/packing; preserve current R025 preparation. GPU transform/clip/interpolation behavior uses the declared observable backend envelope, not a mandatory whole-source per-FOV precision simulator. No scene/metre mixture, altered math or guessed precision.
- **INV-H370-RESOURCES / FORBID-H370-RESOURCES:** Reserve and retain every actual source/CPU/GPU/staging/old/in-flight charge under finite existing limits. No proof-only vertex/triangle sweep, equivalent frame certificate, unbounded cache/service or quality loss is required or authorized.
- **INV-H370-PROOF / STAGES:** Exact source and actual-host correctness, controlled compiler/Device capability, representative predeclared observable GPU conformance and separate native/quality/performance/JC acceptance remain distinct. No universal GPU arithmetic theorem, per-frame oracle/readback, unsupported/failing envelope acceptance or S3/S4 promotion follows.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

**INV-H370-SCOPE.** Only exact separately JC-adopted parent/feature versions and later bounded engineering grants can operate the listed optional family.  
**FORBID-H370-SCOPE.** No self-adoption, blanket R-series activation, constitutional exception or implementation permission from S1 adoption alone.

**INV-H370-LEGACY.** Baseline ABI/pipelines/appearances, D=0 packing and existing legacy/linked clients remain exact and selectable.  
**FORBID-H370-LEGACY.** No automatic migration, changed old shader meaning, generic2D branch or D-enabled mixed-legacy substitution.

**INV-H370-SOURCE.** All selected source/material/model/view/light/E/G/exposure/certificate/context facts form one attributable immutable leased association. Include original-versus-render-frame interpretation, actual packed vectors and complete common-UV normal certificates, with exact retained source/corner provenance.\
**FORBID-H370-SOURCE.** No partial setters, mutable light collection, inferred source identity/presence/physical truth or post-Update semantic callback. No automatic C fallback, original smooth/Mikk claim for derived C shading, source-role erasure or shader hint/repair.

**INV-H370-WRITER.** First-version extended publication is direct checked UNLINKED ordinary flat source only; linked ownership remains exclusive.  
**FORBID-H370-WRITER.** No widening, tunneling through, impersonating or bypassing mounted G1; linked extended requests refuse before mutation.

**INV-H370-REVISION.** Original structural/model/view and independent extension revisions are immutable/nonwrapping and preserve actual content equality and complete requested/committed provenance.  
**FORBID-H370-REVISION.** No guessed epoch, wrapped revision, unchanged-ID content mutation, unnecessary base-revision rewriting or old-front relabel.

**INV-H370-METRIC.** D-enabled positions/view/model/normal/shadow transforms and attenuation share checked positive-k metric space before packing; world directions/local-first order remain exact. Host representability is rigorously checked; host clip intervals do not prove GPU multiply/clip precision. R024 §8.2.4 qualifies observable GPU consequences within the declared domain/envelope, without hidden telemetry.
**FORBID-H370-METRIC.** No scene/metre mixture, hidden k/rotation, double calibration, arbitrary matrix/Y reflection, baseline packing change or unproved precision success.

**INV-H370-ABI.** Dedicated controlled profiles preserve the exact52/80/112/64/608/1984/112/16 layouts, conditional bindings and four-group boundary with actual Device proof.  
**FORBID-H370-ABI.** No public WGSL/global binding/raw GPU entry, fifth group, dummy direct/E resources or treating counts as support evidence.

**INV-H370-EG.** Independent producer owns E preintegration/world mapping and static two-bounce G transport/patch/GI6 generation; Halcyon only validates and evaluates qualified data.  
**FORBID-H370-EG.** No renderer GI solver, automatic bake, source-frame shader rotation, foreign-G6 relabel, default environment/ambient or dynamic GI claim.

**INV-H370-IMAGE.** The same private final owner publishes tone-before-average LINEAR Rgba16Float sample1 front/back through existing preassociated low-level Image ingress.  
**FORBID-H370-IMAGE.** No HDR scene Image,8-bit/sample downgrade, new ingress/publisher, generic2D tone pass, native format change, gamma or alpha repair.

**INV-H370-RESOURCES.** Exact source/pool/slot/target bytes and actual overhead/work/old/in-flight generations are finitely reserved before writes under Context and Device limits. Finite certificate verification/coverage and expanded-corner metadata/storage/leases are included.\
**FORBID-H370-RESOURCES.** No guessed global budget, hidden history/churn, unbounded work, downresolution/role loss or allocation success inferred from arithmetic.

**INV-H370-TARGET.** Preparation and publication consume one actual current renderer-owned target snapshot and compatible per-target resource class.  
**FORBID-H370-TARGET.** No caller DPR/extent, wrong-surface source, shared incompatible front, Layout/Paint semantic mutation or new phase.

**INV-H370-LAST-GOOD.** A committed or compatible retained front keeps its original complete source/material/light/bake/exposure/target association and leases.  
**FORBID-H370-LAST-GOOD.** No stale maps/bake with fresh source, partial commit, false current status or sampling an incompatible/retired target.

**INV-H370-QUEUE.** All candidate writers use the existing captured Device/Queue and precede existing swap/registration/commit with lease-safe retirement.  
**FORBID-H370-QUEUE.** No second renderer/Device/Queue/publisher/scheduler, write to sampled front, early drop or publication-as-GPU-completion claim.

**INV-H370-STAGES.** S0 candidate, S1 adoption, separate complete S2, independent S3 versioned input-domain/backend-envelope acceptance from representative observable tests plus the mandatory original three-asset external-consumer conformance fixture, and actual S4/JC remain distinct. Each candidate retains ordinary guards; in-domain dynamic changes are not fresh per-frame S3 units.
**FORBID-H370-STAGES.** No circular ban on authorized S2 proof execution, public partial feature, mock/placeholder/fallback or selected-domain change to obtain a pass.

**INV-H370-PROOF.** Source/host checks, compiler/ABI/capabilities, bounded available observable GPU evidence, lifecycle/native/performance and JC decisions retain exact executed provenance. S3 separately declares the supported input domain/backend envelope and capability-family/regression compatibility; evidence pins are not the per-frame runtime key.  
**FORBID-H370-PROOF.** No hidden-intermediate measurement from final pixels, unselected comparator, universal guarantee, fake receipt, unsupported/out-of-domain/failing class or unobserved native success; no rejection of valid dynamic inputs merely for differing from a recorded fixture.

**INV-H370-ABSENCE.** Unselected/inactive/zero-use optional paths create no unrequested work; original owners retain only necessary lifecycle retirement.  
**FORBID-H370-ABSENCE.** No all-widget scan, invisible idle pass, background generator, implicit light/E/G resource or default2D/legacy migration.

## 12. Future proof, failure and adoption oracles

**Adopted / Binding [S1] — 2026-11-03: qualification of §12.** Every source/host/numerical table reference inherits the exact boundary adopted in R024 §8.2.4, R028 §§6/8/13/15 and this document's §§9–11. Original source/frame/normal/radiance validity and actual R025/R027/target/lifetime obligations remain; helper/clip/LOD numerical conformance uses representative predeclared observable GPU outputs, not a mandatory per-frame emulator or hidden telemetry.

The unchanged original three-asset external-consumer conformance fixture at N1 and N4 and independent native/quality/performance/JC gates remain separate. The §9 adopted reform supplies the complete future S2 proof contract; documentary adoption alone grants no implementation or automatic completion of held work. Existing historical observations/receipts retain their exact earlier scopes.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

No test is created or executed by this documentary unit. The table specifies later owner/stage evidence on intended paths, including reachable confusable failure boundaries; it creates no standalone unsupported-space audit program.

| Owner / stage | Required positive evidence and exact negative boundary |
|---|---|
| Independent geometry/tangent producer; S2→S3 | Exact original normals/UV/indices and ALL original signed default-Mikk corners. Verify A old proof, B all9 packed cross bounds, genuine-absence d and explicit C original-local F/det/U/H/handedness/packed identical corners with source-proven singularity and derived-shading disclosure. Reproduce reported six sofa faces[23,25,214,704,706,895] without hard-coded eligibility; no simultaneous Metal centroid assumption. Missing original data, unproven mode/sign/corner map or renderer-created frame refuses. |
| Application source + Context; S2→S3 | Complete flat typed extension identity/revisions/leases and material successors; stale source/revision, partial ModelState or cross-context material refuses without changing the prior complete request/front. |
| HAL-354/266A writer owner; S2→S3 | Actual unlinked extended publication plus original legacy and linked G1 behavior. An extended call to an active linked mount, including equal content, refuses without any requested/committed/target mutation. |
| Metric CPU/geometry/appearance; S2→S3 | k!=1, proper rotation, nonuniform positive scale and off-camera caster demonstrate same ideal projection/world normal, actual metric attenuation and cube face-axis depth. Nonfinite/unrepresentable conversion, wrong inverse matrix or mixed LegacyAppearance under D refuses before writes; old legacy numerical packing stays unchanged. |
| Material evaluator; S2→S3 | Preserve local/world-GS counterexample, independent-weight A/B/C/absence source certificates and packed bytes. Predeclare representative N1/N4 output discriminators for local-first behavior, C clipping/partial coverage, normal-role/side/sign/Mask/material/scale and backface/nonuniform cases. Compare actual final FP16/Image, genuine available source-stage outputs or coverage/Device results under an authorized criterion, not direct hidden N/T/g, unit_nonzero/inverseSqrt or helper telemetry. Missing discrimination is UNVERIFIED; any essential additional numeric bound awaits selection by the delegated architectural reviewer before separately authorized S2 execution, with no .03/.05 borrowing. R024 §8.2.4 accepts a versioned input domain/backend envelope, not each recorded frame. Source/capability or observed failure returns exact cause, never a replacement normal. |
| Direct/shadow raster/backend; S2→S3 | Actual R-025 nine Boolean nearest comparisons, .25/.75 versus ref.5 discriminator, six-chart centers/ties/cross-face d_major, full masked/off-camera casters and current maps. Fractional filtered compare, wrong cube reference, stale mask/light or uncovered near/tap domain refuses/fails the real owner proof. |
| E/G producer and source admission; S2→S3 | World-aligned E prefilter/LUT and independent qualified static T(L0)+T(T(L0))/WORLD GI6 patches, exact eight presence layouts and requalified actual shading-mode/material dependencies. Unknown orientation, foreign G6 without transport, changed model/light/visibility bake or missing E evidence refuses; no implicit removal or ambient fallback. |
| HDR/radiance producer; S2→S3 | Independently verify R027 §4.1 FULL ideal source/evaluator-domain direct+emission+E/G bound for all possible covered values, finite/nonnegative/<=60000, with selected source frame/packed association/model/view/EV. No missing/stale certificate, individual-light cap, average or readback replaces it. Use R024 §8.2.4 predeclared available observable radiance/tone/FP16/Image comparisons within the domain/envelope; no universal actual GPU-f32 proof or hidden arithmetic measurement follows. |
| Naga/Device/raster; S2→S3 | Exact fixed/conditional layouts,6 uniform blocks/10 textures/8 samplers all-present, stage/dynamic offsets, shader IR and pinned backend/sample/format limits. A compiled module or adequate count alone is insufficient; unsupported capability refuses the new profile. |
| CustomRenderPass/Image owner; S2→S3 | Actual private FP16 final pair, per-sample N1/N4 tone-before-average, alpha=covered_sample_count/N storage/filtering and existing Image modulation/registration. Missing filterable FP16 or exact textureLoad/MSAA capability refuses before writes; old8-bit counterexample is not a permitted fallback. |
| V2 producer/sampling/backend; S2→S3 | Original first-moment chains and MIN/MAG/U/V, derivative-before-discard and unchanged multiscale numeric oracles. Full SAME-UV joint mip/address/LOD cells, real response-corner1/510 witnesses/coverage, all6 MIN/NPOT/ties/seams/ends and finite work. Reproduce five supplied correlated normal cases, hardest bound .0030084210845814116>1/510, original BLUE126/124 unchanged and predeclared actual N1/N4 observable sampler/normal-response and helper-sensitive gradient comparisons through FP16/Image or genuine available source-stage outputs, plus unchanged quality metrics, under R024 §8.2.4. Do not claim direct hidden TBN/normalization/helper measurement. Essential additional numeric bounds await selection by the delegated architectural reviewer before execution; .03/.05 apply only to their exact R028 §12 sampling/temporal metrics. S3 accepts the declared input domain/backend envelope, with ordinary per-candidate guards for dynamic inputs. Raw-eight-hull failure alone is not actual failure; stale/unresolved/incorrectly independent UV certificate refuses. No new source/GPU success is claimed by text. |
| Context/target/lifetime; S2→S3 | Checked complete reservations, unchanged-input zero work, all old/in-flight leases, resize/collapse/retire and same-Queue front order. Capacity failure preserves compatible original last-good; incompatible target withdraws rather than reusing a differently keyed front. |
| Existing native owner + JC; S4 | Canonical App::run sample1 AND4 with actual textured floors/walls/furniture and real material/light changes; source→shadow/scene→FP16 final→Image→native evidence, failure/current-target/old-front/no-use and no2D regression. Screenshot/generated concept/prepared receipt is not actual output or quality acceptance. |
| Existing backend/performance owners + JC; S4 | R-028's unchanged120-frame temporal/error, genuine GPU timing/workload ratio, zero steady-state uploads and native visual gates on each claimed pinned backend. Missing timer, observed LOD transition or JC/native evidence remains UNVERIFIED, not zero error or success. |

Future mechanical proof belongs at existing src/tests/three_d_context.rs, three_d_geometry.rs, three_d_appearance.rs, three_d_snapshot.rs, three_d_raster.rs, three_d_backend.rs, three_d_viewport.rs and custom_render_preparation.rs. External native proof belongs at tests/app_frame_realization_native.rs; these names establish placement, not an assertion of already-running fixtures. Stage/claim records must identify actual source, revision, Device/backend, format/sample/target, owning test and result.

## 13. Documentary verification and disposition

For the2026-11-01 S1 integration, read all changed authoritative spans in R-024/R-028/HAL-370, the exact accepted Revision02 source and the coordinated source/proof/status matrix. Verify precisely those THREE owned diffs, whitespace, hashes, current Git state and required health summary; preserve the proposal, unlisted binding laws and every source/test/asset/foreign byte. The earlier eight-document adoption and creation receipts retain their historical scope. Do not mistake health diagnostics for expanded-source/GPU proof; no executable test or runtime change is performed.

```text
ws halcyon
read docs/specs/SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md 1 1000 max-lines 1000
read docs/specs/SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md hash
read docs/specs/SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md hash
read docs/specs/SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md hash
state health summary
state git
```

Current result is the exact coordinated source-domain Revision02 integrated as S1 binding in R-024/R-028/HAL-370 under delegated independent technical review and JC's retained veto. Original UNIT/.75/BLUE sufficient inputs and earlier proofs are preserved; original-source/mode/packed distinction, source-proven explicit C and real common-UV normal certificates are now law. Old-domain private S2 and internal unlinked writer remain accepted only for their tested inputs. Expanded source/producer/packed-f32/Naga/Device/Metal proof is NOT ACCEPTED; separate bounded engineering and same-scope tests remain required. No public S3/native S4 or new source implementation is granted.

**HAL-370 remains Adopted / Binding [S1]; expanded inputs are NOT implemented/qualified by this adoption; old private S2 acceptance is scope-limited; S3/S4 CLOSED. JC retains veto.**

## 14. Proposed S1 — coordinated admission of two derived source frames

**Proposed / Inactive / Non-binding.** Independent architectural review selected the zero-callback draft
on 2026-11-07 in
`HAL-R024-CANONICAL-MIKK-ZERO-DECLARED-GEOMETRIC-SOURCE-FRAME-S1-DRAFT-01`.
`HAL-R024-DUAL-GEOMETRIC-SOURCE-CLASSES-S1-DRAFT-02` adds a second
separately declared class for review.
[R024 §18](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#18-proposed-s1--two-source-qualified-geometric-rendering-classes)
and this appendix require an independent coordinated adoption. Existing
authority, JC's veto, and the separate S2, S3 and S4 gates remain in force.

### 14.1 Proposed qualification of §4.1

Within the existing unlinked opt-in writer, the two named producer
declarations would qualify §4.1 and require express later coordination
with R024 §7.1.2's binding A/B/C and “no fourth guessed frame” rule.
Their inputs are complete original mapped triangles with exact positive
area, nonzero original UV determinant, genuine TANGENT absence and
complete final pinned default-Mikk results from the qualified producer's
controlled pack/source transaction under R024 §18.2.

- `PbrDefaultMikkZeroGeometricFrameV1` requires an exact-zero xyz vector
  at a verified original corner. Retain the raw signed bits and apply
  the §4.1 nonzero-tangent exception only to the proven zero.
- `PbrDefaultMikkNonzeroOutOfDomainGeometricFrameV1` requires three
  finite nonzero signed original vectors and all R024 §18.4 proofs.
  Require strict exclusion from the sufficient A cone, exact full
  support weights and all nine KKT inequalities with `d² < 1/256`
  for B, and positive outward separation from both signed tangent
  hulls in both original and actual packed smooth domains for C.

Keep every original attribute, index, triangle, final raw signed callback
and immutable source identity. Qualify the same original POSITION/UV
geometric F/U/H, actual bit-identical packed frame and all nine
outward `1/16` bounds for both classes. The renderer consumes already
verified directions. The proposal preserves A/B/C, the prohibition on
guessed frames, and explicit producer selection and disclosure of
derived shading. Shader flags, repair, automatic fallback and relaxed
validity for other source classes are forbidden.

The typed addition is CPU source evidence at the existing geometry owner.
Require distinct per-triangle variants for zero versus nonzero exclusion,
with the original data and existing A/B/C or genuine-absence records
separate. Preserve the existing exhaustive public frame enum and geometry
descriptor and the current external source-producer encoder. HAL independently verifies
source identity/digest association, complete original/render counts and
ordered final callback coverage, eligibility, derivation and actual packed
bytes before admission. Use the HAL380 bounded CPU verification and Context
registration boundary, with the existing owner, cancellation and complete
immutable successor checks.
The original digest, producer, interpretation, certificate and geometry
generation remain distinct. A changed interpretation advances the
affected structural and complete source association under §4.2.
[Geometry verification](../../src/three_d/geometry.rs#L1742)
[HAL380](SPEC-HAL-380-Nonblocking-PBR-Geometry-Certification-and-Admission.md)

**Primitive-wide genuine absence.** Under the proposed additive HAL380
input, bind the private `NotRequiredNoNormalRole` origin and producer
absence evidence to the exact original source identity and revision,
material assignment, primitive and occurrences, interpretation and
certificate generations and full original triangle roster. The producer
verifies authored TANGENT absence and records that the original capture
used no Mikk invocation or generated output. The original signed tangent
stream is empty. The existing HAL snapshot/material owner
independently checks genuine normal-role absence against the retained
current material lease for each association. An invalid present normal
role, a mapping flag or empty bytes cannot establish absence. Keep the
existing normal-only proof and inert PBR52 field. Retain all actual
mapped signed results under their unchanged source requirements.
[Additive origin](SPEC-HAL-380-Nonblocking-PBR-Geometry-Certification-and-Admission.md#63-callback-evidence-and-class-eligibility)
[Leased material association](../../src/three_d/snapshot.rs#L219-L238)

PBR52, D-only 52+12 face expansion, Material64, all groups and the
Device/Queue ABI keep their existing meaning. R025 receives the same
nondegenerate original geometric faces in the same order, with the same
Opaque/Mask, sidedness and complete off-camera caster proof. No R025
source-geometry exception is required. Requalify current Direct and R027
radiance for the changed source and frame under their existing owners.
R028 and HAL373 retain the complete original normal domain, mip chain
and `1/510` bound. HAL372 continues to consume an already admitted
source without deriving or selecting a frame. The HAL355 baseline,
linked-writer exclusion, target/front law and zero-view behavior apply.
[D packing and draw owner](../../src/three_d/raster.rs#L4430)
[R025 face normals](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md#4-direct-material-evaluation-and-finite-support)

### 14.2 Prepared source and future gates

The source-container owner would need independent adoption of a prepared-recipe extension
under FAB011 §§6–7 and FAB021 §5 for ordinary HCONT001 Inspector use.
Retain HCONT001 v1. Bind the unchanged original GLB and SourceBinding to
the typed derived recipe. Preserve exact render/corner association and
the complete original records in R8, including raw zeros.

Legacy schema1 R13/R14 retain the existing A/B/C/NormalOnly certificates.
The supplied inactive FAB021 §13 schema2 reference is R7 tag5
`A(R38/schema2)`, with `Legacy -> R13/schema1` and `Z/N -> R39/schema2`.
Both Z and N carry common R42. Z carries its mask and N carries
R40/R43/R44. R11/schema2 carries tangent origin. R41 is conditional on the
mapped DefaultMikk capture and absent for genuine normal-role absence.
Preserve original
lineage and account for each class in the prepared summary. Existing
recipes retain their exact meaning. Fabricated legacy R13/R14 records
cannot represent Z/N. This proposal assigns no binding wire discriminant
or parser permission.

Ordinary prepared opening would verify original root and SourceBinding,
digests, exact ordered captured data, configured producer and proof
metadata without Mikk regeneration. This verifies structural association
of producer-attributed evidence. The live capture provenance originates
in the qualified pack transaction under R024 §18.2. Independent proof of
historical execution after a full hostile rewrite is outside the selected
boundary. HAL independently checks geometry, material and source
certificates on admission. Canonical R41 input and output digest
preimages, R11/R3/R34 symbol and generation bindings, and actual future
producer qualification remain separate source-producer S1 gaps.
[Capture boundary](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#182-proposed-zero-callback-source-class)

The supplied FAB021 schema2 R11 kind2 `NotRequiredNoNormalRole` remains a
Proposed / Inactive wire-source claim. Prepared use requires independent
FAB011/FAB021 wire adoption and producer provenance, followed by the
same HAL origin and material-association checks.

**Preserved invariants.** Every selected triangle has the exact original
source and verified packed interpretation. All archive, derived,
certificate, staging and live/retiring charges belong to the existing
Context/target owners. Keep R027's same-Queue publication, compatible
last-good on typed refusal and incompatible withdrawal. Forbid stale
source or frame substitution, partial front publication, source repair,
normal-role removal, another renderer or new Device capability.

**KOMPLEMENT 902.779.61 remains NOT ADMITTED.** R024 §18.5 records
the reported 1,492/3/5 candidate partition of the complete original
roster and its evidence gaps. The gates are separate.

1. **S1.** Independently review and adopt the exact coordinated R024,
   HAL370 and HAL380 §6 text before treating either mapped interpretation
   or the primitive-wide origin as binding.
2. **S2.** Separately authorize the HAL checker, producer-side proof
   generation and render packing in their respective owner scopes.
   Verify complete source, host, Device and resource checks.
3. **S3.** Independently qualify the supported source and backend scope
   from real source, CPU and N1/N4 GPU evidence, including Direct,
   Mask, culling, coverage, registered FP16 Image and lifetime controls.
   Keep valid original Poly Haven sofa and desk and the ordinary external-consumer
   conformance controls. Record actual native tests separately under §7's native
   gate, including the exact source, target, frame and registration.
4. **Prepared Inspector.** FAB011/FAB021 adoption and separate
   engineering and same-source proof are required for the prepared
   HCONT route. Disclose the original versus derived shading.
5. **S4 and JC.** Apply the existing native and quality gates and obtain
   JC's separate appearance acceptance. Source proof or native test
   execution alone cannot establish the appearance result.

All gates for these proposed classes remain unmet. This appendix records
a proposal for review.
