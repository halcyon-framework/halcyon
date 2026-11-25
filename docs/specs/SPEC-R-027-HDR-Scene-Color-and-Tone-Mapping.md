# SPEC-R-027: HDR Scene Color and Tone Mapping

## Binding feature-free MRT4 linear scene amendment — 2026-11-06 [S1]

**Adopted / Active / Binding [S1 technical contract only].** Independent architectural review under JC's delegated technical authority and retained constitutional/product veto selects and adopts `HAL-R027-HAL370-MRT4-FEATURE-FREE-S1-ADOPTION-01` jointly in R027 and HAL370. The reviewed seven-clause candidate SHA256 is `bfdc6a9a7a4ee5ce7cc75c767e1c0d3ebb37756120a17a0d14b43146638329f2`; §16 records the exact scope, source/proof distinction and remaining gates.

The current clauses below supersede ONLY the former single Rgba16Float private scene, single b1 scene-load interface, old attachment-byte formula and unconditional actual nearest-rounding premise. The final linear Rgba16Float pair, native surface/Image ABI, source mathematics, coverage, exposure/tone order, existing Device/Queue and lifetime owners remain unchanged. Earlier FP16-scene descriptions and authoring receipts are historical within that superseded scope. This documentary amendment grants no production edit, Device feature request, S2 implementation or native/physical acceptance.

## Historical FP16-correction scope — 2026-10-31

This section and the following architecture review retain the original correction-task status, one-file scope and receipts. Their inactive/no-adoption statements were true at that cut; the current S1 header and §1 govern present authority without upgrading any source/backend evidence.

This is the current HAL-R027-ALPHA-QUANTIZATION-ARCHITECTURE-CORRECTION-02 documentary unit. The original A–J authoring unit and its beforeimages are historical provenance in §14; the architect's fixed linear-FP16 final-front decision below supersedes only the contradicted surface-format final-storage premise, not the other selected A–J decisions.

- Objective: Correct the known 8-bit sRGB final-front premultiplied-alpha counterexample by selecting one fixed linear Rgba16Float single-sample final pair for this inactive profile, with exact parent/format/filter/identity/budget/proof obligations.
- Source qualification: HAL-002/002A/309, R-027/R-023/R-024, affected HAL-355/263A/266A/295A/354/357 clauses and actual renderer/pass/target/registration source were requalified. Complete source reads matched the required beforeimages. This is a proposed-contract mathematical correction and supplies no new production defect or native-execution claim.
- Governing authority and status: HAL-002 Foundational and HAL-002A Permanent remain supreme; HAL-355/263A/266A/295A/354/357 binding; R-023/R-024/R-027 Draft / Proposed / INACTIVE / NOT BINDING. JC's exact mandate authorizes this correction only; adopted HAL-309 governs truthful status.
- Authority change, if any: NONE effective. Only the proposed opt-in final format and necessary future parent-extension obligations change; no parent adoption, implementation permission or runtime capability is conferred.
- Target layer and existing state verified: Halcyon0.227.18-beta, master HEAD312e25c7a6a6d923ff4db26a311cff32fdb717a4, wgpu25.0.2/Naga25.0.1. Required exact beforeimages: R-027 SHA2568355e3f4a17ca50a214aefa25dd8342ff12c2003068efbbe7f6d8e9e04cf4862 (458 lines / 58253 bytes), R-023 bc3cbed9c0bfc8e3df5bdcbdfeda4f893d04b0b62c94f5f081eccb404a0c7f36 (483 / 78241), R-024 99d745f68e26690c7c695444a3126627d13ef17b2c60e488b088a2a055b36e53 (578 / 95993). Initial state: tracked modified R-023, untracked R-024/R-027, zero staged; no other changed path.
- Owned mechanisms and files: ONLY this existing R-027 file. R-023/R-024, every accepted SPEC, doctrine, source/test/example/asset, manifest, native surface and unrelated state are protected byte-for-byte.
- Named invariants: All eleven existing INV-R027/FORBID-R027 names in §10; HAL-263A §3.2 and INV-002/004/005/006/008, HAL-355 target/front/budgets, HAL-295A same-Queue publication and HAL-266A current indexed preparation; constitutional owner/phase/ID laws.
- Forbidden behavior: Runtime/API/WGSL/pipeline changes, parent activation, baseline surface/2D changes, Image alpha repair, approximate 8-bit sRGB called exact premultiplied, a new global/publisher/renderer, alternative curve/format/fallback, native/GPU proof from this draft or self-adoption.
- Existing mechanisms inspected and reuse decision: Existing Image requires a filterable float single-sample view; CustomRenderPass currently forces gpu.format. Reuse its private identity/front/back/same-Queue publication owner only after an exact admitted per-profile internal-format extension; no parallel subsystem or public raw texture API.
- Relevant external or dependent boundaries: Complete <=60000 source certificate, R-024's material/producer/f32 proof, R-025/R-026 light integration and actual wgpu25/Naga25/Device/FP16/filter proof remain OPEN. Official transfer/format references are evidence, not runtime authority; no consumer edit.
- Test ownership and placement: Documentary proof only now; future existing three_d_raster/backend/context/viewport and native Image-association owners must prove both old k2/k3 counterexamples, exact binary16 coverage storage and actual scene->final->Image/output behavior. No test, shader or source is added/run here.
- Exact runnable verification commands and expected proof: §14 gives Grip full read/hash/state commands; verify one corrected inactive artifact, unchanged tone/coverage oracles and eleven names, exact R-023/R-024 hashes and protected source hashes. No Rust/build/GPU/native job is requested or run.
- Output or artifact completeness boundary: One coherent corrected R-027 with known contradiction, fixed linear display-LDR FP16 final, rigorous ideal representation/filter argument, complete identity/target/format/lease/budget/parent updates and all seven implementation gates still OPEN.
- SemVer and changelog impact: N/A — inactive document only; no executable/API/release change, package remains0.227.18-beta.
- Out of scope: HDR scene format, per-sample curve/order, EV range, R-023/R-024 edits, HAL parent/source adoption, baseline defect declaration, R-025/026/028 files, actual sample execution, consumer applications, widgets/2D effects and successor work.

## Historical architecture review

- Governing SPEC/read status: Supreme HAL-002/002A; binding affected HAL-355/263A/266A/295A/354/357 and adopted HAL-309 read; all R drafts remain inactive.
- Widget policy or N/A: N/A — document-only 3D output correction, no widget.
- Interaction added and policy authority: NONE; input, focus, Capture and semantic owners remain unchanged.
- Applicable FORBID rules: Constitution owner/phase/ID; HAL-355 no 2D contamination/raw escape/false publication/partial state; HAL-263A premultiplied sample and one alpha; HAL-295A private same Queue; HAL-266A current indexed preparation.
- Existing framework mechanism inspected: Private CustomRenderPass final pair/registration, its current gpu.format allocation, existing Image filterable float/sampler layout and actual target/source lifecycle.
- Reinvention risk: Sole existing offscreen owner receives a future opt-in internal format extension; no generic 2D service, second publisher/device or new phase/clock.
- Demo/example compliance: N/A — no example, demo, runtime or surface changed.

**Author:** JC  
**Date:** 2026-10-31  
**Status:** Adopted / Binding [S1 — HdrSceneColorToneMapV1 under HAL-370]  
**Layer:** Optional 3D scene-color and final-output profile  
**Technical decision record:** original HAL-R027-HDR-COLOR-ALPHA-ARCHITECTURE-DRAFT-01 A–J, with final-storage premise corrected by HAL-R027-ALPHA-QUANTIZATION-ARCHITECTURE-CORRECTION-02  
**Final owner/veto:** JC  
**Adoption:** PERFORMED 2026-10-31 through the delegated architectural reviewer's coordinated S1 decision under JC's express decision delegation; JC retains final veto.  
**Implementation / proof:** NOT IMPLEMENTED / S2 NOT AUTHORIZED / S3 source/numerical/compiler/Device/backend OPEN / S4 native/performance/JC OPEN. NOT IMPLEMENTATION-READY for engineering or user-facing admission.  
**Current adoption record:** HAL-370-R023-R028-S1-COORDINATED-BINDING-ADOPTION-01.

## 1. Purpose and profile boundary

The named opt-in profile is **HdrSceneColorToneMapV1**. Its one private scene bundle has four source-linear R32Float planes in fixed order R,G,B,Coverage. It applies the selected exposure/tone curve to each covered sample before coverage averaging and publishes the unchanged **linear Rgba16Float single-sample final front containing display-LDR premultiplied color**. Its final pair is independent of native surface format; unchanged 2D output owns ordinary surface encoding after Image composition. Neither floating-point storage nor this profile is HDR display output or a physical-validity claim.

HAL-355's current legacy LDR profile remains byte/behavior compatible at sample count1 and4. It retains its existing appearance shader ABI, direct surface-format attachment, wrapper clamp and hardware resolve. This opt-in contract does not repurpose that wrapper, reinterpret an old appearance, change a default, or migrate any unlinked/2D consumer.

This document selects neither light units nor a lighting/GI algorithm. R-024's material response cannot create incident illumination. R-025/R-026 must separately admit the actual bounded light/environment inputs and their complete integration. Emission is part of the bounded scene-color result, not authority for an engine light source or indirect transport.

Normative language is binding at S1 in [HAL-370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md)'s exact opt-in scope and HAL-355 §17.1's acknowledgment. HAL-370 §1 records delegated independent technical adoption, not implementation or JC's personal technical review. S2 engineering requires separate authorization; S3/S4 remain OPEN. Ordinary draft existence or documentary review alone never supplies adoption or runtime proof.

## 2. Binding authority and verified baseline

| Owner | Live binding seam / actual inspected source | Boundary preserved or requiring extension |
|---|---|---|
| HAL-002/002A | Retained sole ownership, immutable IDs/snapshots, semantic Update versus read phases | No constitutional amendment, cross-widget reference, semantic GPU state or extra phase is proposed. |
| HAL-354 §4.1 and mounted transfer contract | Immutable complete Update-owned values before target preparation | Future color settings travel in a qualified complete source request; this contract changes no existing G1 signature, receipt or writer rule. |
| HAL-355 §§9.1–9.5 | Fixed legacy ABI and fragment finite-to-zero/[0,1]/alpha-one wrapper; appearance.rs:1685–1732, raster.rs:387–456 | That wrapper cannot be used to preserve HDR. A distinct admitted profile/evaluator and pipeline key are required; no source implementation exists. |
| HAL-355 §§10.1–10.3 | Legacy sample1 back attachment; sample4 surface-format MSAA directly resolves to back; raster.rs:1556–1575, viewport.rs:465–525 | Both private HDR scene storage and the fixed linear Rgba16Float final pair/tone-before-resolve require express opt-in extension; the baseline is unchanged. |
| HAL-266A §§3–6 | Renderer-owned final physical extent/activity; custom_render_prepare.rs:177–253 | Consume actual current metrics after Layout. Do not derive another extent/DPR, public navigation eligibility or semantic resize event. |
| HAL-295A §§3–4, INV-008/009 | Same captured Queue, offscreen producer, submit/swap/registration; custom_render_pass.rs:498–542,611–633 currently allocates gpu.format | Reuse this exact private owner after scoped internal-format admission, never a second publisher. No renderer-private escape, completion wait, graph or recovery model. |
| HAL-263A §3.2, §§4–5 | Closed low-level preassociated Image ingress; wgpu_renderer.rs:2082–2097 requires filterable float2D/sample1 plus filtering sampler | The final linear Rgba16Float view must meet that exact binding/alpha contract. No HDR scene view, tone, gamma, repair or second premultiplication in Image. |
| HAL-357 §§2/6–8 | Optional producer composition lowers to complete flat requests | No new scene/light hierarchy, environment transport or HDR permission follows from lowering. |
| HAL-309 §§2/7–8/12–14 | Status/source/proof truth | Source existence, proposal, adoption, compilation, GPU output and native/JC acceptance remain separate. |

Current Context3D public texture content admits RGBA8 numeric/sRGB descriptions, not an HDR scene attachment (content.rs:59–134). Current target resources reserve existing budgets, create matching Depth32Float and optional surface-format4× color, then use CustomRenderPass (viewport.rs:465–525). That pass currently allocates its sample1 front/back in gpu.format (custom_render_pass.rs:611–633), and target-bound Paint emits WHITE Image (viewport.rs:1502–1511). Headless WgpuRenderer selects Bgra8UnormSrgb (wgpu_renderer.rs:1435–1436); Image requires filterable float sampling (:2082–2097). These are unchanged legacy source facts, not an existing configurable FP16-front API or executed backend proof. HAL-370 §8/HAL-355 §17.1 now adopt the exact §10.1/private-pass opt-in format contract, not its implementation; the native 2D surface format stays unchanged.

Cargo.toml:14–15 pins wgpu25.0.2 and Naga25.0.1. General WebGPU/WGSL descriptions or a newer library release cannot establish acceptance by that exact compiler/device/backend. This unit executes neither compiler nor device.

## 3. Color domains and representation

All RGB quantities use the **same declared linear sRGB/Rec.709 primaries** as correctly decoded material color. R-027 introduces no second primary conversion. Role-specific material decode stays R-024 work; numeric normal/MR/AO textures remain numeric.

| Quantity / stage | Defined domain and association | Storage / transfer owner |
|---|---|---|
| Complete shaded scene result L_i | Straight RGB, finite, nonnegative and<=60000 per covered sample; coverage separately0/1 | Complete separately admitted surface/light/environment/emission result and upstream certificate |
| Private scene C_i | Four R32Float[N] planes R,G,B,Coverage represent the computed source-linear f32 payload and exact binary coverage; no transport gain/exposure | One PBR/depth pass, four unfiltered same-sample texture loads; no HDR hardware resolve |
| Exposure e | Positive checked f32 multiplier from immutable EV[-16,16] | Application setting, CPU qualification and Uniform16 |
| Covered tone U_i | Component-wise T_e(C_i), straight linear RGB | Fixed controlled tone evaluator, unchanged selected curve |
| Final ideal P | RGB=sum U_i/N, alpha=k/N; premultiplied linear display-LDR | Tone pass writes directly to linear Rgba16Float sample1 final back |
| Stored final F / Image sample | Actual qualified linear binary16 conversion of the toned/averaged result, no sRGB encode/decode; single-sample filterable float view | Same final-front/registration owner and Image sampler; exact association plus the expressly qualified numerical envelope in §7.4, no universal nearest-rounding claim |
| Native surface output | Existing 2D composition of the Image and other content; normal surface encoding only if its format requires it | Unchanged native compositor/output path, not a new 3D tone/gamma pass |

The scene transport's ideal map is component-wise identity on the already computed source-linear binary32 payload. Keep source L_i, shader evaluation, actual stored/load-visible C_i and final conversion distinct. R32Float does not assert exact representation of every positive real/subnormal or universal GPU arithmetic accuracy; the original finite nonnegative <=60000 source domain is unchanged. No source minimum, amplification, extra exposure, emissive floor, normalization or source/material rewrite is admitted. Exposure remains AFTER intermediate storage; §7.4 separately qualifies the measured radiometric pairing and final-format error.

Scene alpha is opaque/mask/depth coverage only. Discard writes nothing; covered black has RGB0 and alpha1. No general translucency, alpha-to-coverage or color-derived coverage is admitted. Final alpha is k/N, not a tone-mapped/translucent source alpha.

**Scene and final remain distinct content domains and now have different storage.** The private four-plane R32Float scene may contain certified radiance above1 and is never registered as Image. Only the toned premultiplied display-LDR Rgba16Float final is sampled as Image, with required RGB<=alpha<=1 and §7's actual-format qualification. Native surface format remains a third distinct identity.

No shader sRGB encoding/decoding occurs between tone output, final FP16 storage and Image sampling. Actual final Device filterability, conversion, view and sampler behavior still require proof. R-027 never infers encoding from values or uses Image as an alpha repair service.

## 4. Complete source request and radiance certificate

### 4.1 Upstream proof obligation

Before dispatch or any HDR write, a complete qualified upstream source SHALL provide a conservative certificate that **every component of every possible covered output sample in the admitted request is finite, >=0 and <=60000**. This covers the selected material/effective-roughness response, full light/environment set, emission, model/view-dependent evaluation and the complete integration/combination rule. Bounding an individual light/factor is not proof of the final sum.

The actual light/environment/integration owner supplies the finite bounds, source/version references and combination proof. Halcyon's mechanical admission verifies that the evidence matches the complete tuple and admitted evaluator domain; it never invents light units, lights, integration semantics or a guessed coefficient bound. No per-pixel enumeration or GPU readback can replace this pre-write certificate. An absent, stale, incomplete or unprovable certificate is a typed source/profile refusal, not a black frame, clamp, partial draw or successful fallback.

Executed R-024 regularized-evaluator/f32 proof and actual R-025/R-026 radiance certification remain OPEN. This contract does not establish that any real furniture/material/light request can already satisfy the certificate. A future producer's concrete certificate representation and validator require separately authorized S2 engineering and S3 qualification under the S1 complete-source contract; no Rust type or method is invented here.

### 4.2 Semantic and mechanical qualification

The application publishes immutable semantic facts through the existing lawful Update/source owner: material/light/environment/model/view identities and revisions, profile/color-space choice, explicit or genuinely omitted exposure, and attributable bound evidence. No application supplies renderer-private texture views or guesses physical target truth.

At the existing HAL-266A preparation boundary, the participant combines that immutable source with current renderer-owned target metrics and context/device lifetime, completes all mechanical qualification/reservations, and assembles a target-admitted candidate. Its complete requested/committed record SHALL associate:

- Profile/version, source request identity, structural snapshot, complete model and view, material/light/environment identities and all source revisions/leases used in shading.
- Bound-certificate issuer/version, input-domain/evaluator association and the exact complete inputs it proves.
- Color-space/profile settings, source exposure identity/revision, EV and checked multiplier; omission/default is attributable, never a substitute for invalid input.
- Current context generation and captured Device identity/lifetime, owning viewport/concrete mount and target lifetime, surface/target identity and physical W/H/activity; the complete four-plane R32Float scene storage class/order/count1|4 and common Depth32Float; the distinct fixed linear Rgba16Float final count1/filterable view; unchanged native surface format as a separate compatibility fact.
- Every private scene-HDR/depth/final-front/back resource generation/lease, both8-byte-per-texel final textures and reservations, tone ABI/shader/pipeline class, final format/sample/filter identity and exact final Image registration binding/generation.
- Requested-versus-committed stage and original content provenance, including a compatible reused HDR-content identity when §5.3 applies.

This is the required complete association, not testimony that the current public Viewport3D request already contains color/light/HDR fields. The mechanical preparation may mutate admitted rebuildable GPU/cache state only; it may not author scene semantics, run another Update, or let Layout/Paint alter the request.

## 5. Exposure, fixed tone curve and controlled uniform

### 5.1 Explicit finite exposure

The application owns one immutable signed exposure_ev in inclusive [-16,+16] stops. Default0 is allowed only when exposure is genuinely unspecified under the admitted profile/source contract. NaN, infinity, out-of-range or conflicting/stale exposure is refused, never clamped or replaced by0.

The CPU computes e=2^exposure_ev as checked finite positive f32 and captures its exact bytes with the source revision. The admitted endpoints give e in [2^-16,2^16], i.e. [1/65536,65536]. Exposure does not change the source ceiling60000; it is applied once, after HDR storage, by the tone pass. No auto exposure, histogram, prior-frame feedback, focus/OS brightness, hidden white point, per-view global singleton or consumer-selected raw tone shader exists in this profile.

### 5.2 Exact ABI and curve

ToneMapUniform is exactly one16-byte,16-byte-aligned vec4<f32>:

| Byte offset | Component | Meaning / admission |
|---|---|---|
| 0 | x | Checked positive finite exposure multiplier e |
| 4 | y | Exactly0 |
| 8 | z | Exactly0 |
| 12 | w | Exactly0 |

The four components occupy bytes0..15; there is no hidden EV, curve selector, time, color matrix or auto-exposure state. Host packing, actual uniform binding offset/alignment and every queue-safe in-flight slot require proof against Device limits. The16-byte block size is not a claim that all buffer offsets have16-byte device alignment.

For each covered sample and RGB component x, the fixed curve is:

~~~text
0 <= x <= 60000
e = 2^exposure_ev > 0
T_e(x) = (e*x) / (1 + e*x)
~~~

There is no shader linear-to-sRGB conversion. No gamma, alternate curve, white balance, local adaptation, gamut mapping, bloom or artistic color matrix is admitted. The final Rgba16Float pair stores linear toned color without transfer encoding; only the unchanged native surface output performs ordinary format encoding after Image composition. §7 owns coverage once.

In ideal arithmetic T_e(0)=0; derivative e/(1+e*x)^2 is positive; denominator>=1; and T_e is continuous, strictly increasing and in [0,1) for this finite nonnegative domain. Maximum e*x is60000*65536=3,932,160,000, far below maximum finite f32, so the selected real-valued numerator/denominator do not require overflow or singular handling.

These exact-real facts are not a GPU precision proof. In particular, f32 rounding can make 1+e*x equal e*x at large magnitude and tone may round to1. Source evaluation/R32Float transport, tone/average f32 arithmetic, actual final binary16 conversion and Image filtering need the declared domain/backend qualification in §§7.4/12. The known 8-bit sRGB association failure remains excluded by the final format, never excused by tolerance. No arbitrary epsilon, clamp, curve replacement or universal nearest-rounding claim is supplied.

### 5.3 Exposure-only reuse

Exposure changes create a new complete requested output identity. They can reuse existing HDR content only when the entire scene/material/light/environment/model/view/target/context/sample tuple is proved identical to that content's original identity and every required lease remains live. The new tone pass uses the new uniform and commits a new final front association; it cannot relabel a prior front without execution.

If that exact equality/currentness proof fails, HDR content must be rerendered through the ordinary participant path. A compatible last-good final front is historical output, not an interchangeable HDR source. A newer intermediate write retires the overwritten content identity; no unbounded history, semantic replay store or speculative exposure cache is admitted.

## 6. Attachments, sample-load ABI and pass graph

### 6.1 Exact target contract

For each admitted active nonzero view, the **SCENE** is ONE private target-local bundle of FOUR R32Float color textures in fixed location order R,G,B,Coverage, each at exact physical W×H, one2D layer, one mip, N exactly1 or4, usage RENDER_ATTACHMENT | TEXTURE_BINDING, with one common matching Depth32Float[N]. The bundle's source/Context/target/resource-generation association is immutable while its content is live. All four attachments are written in the SAME existing PBR raster/depth pass. A surviving fragment writes the original evaluated straight source-linear RGB components to locations0,1,2 and binary coverage1 to location3; each plane clears to0. Discarded/uncovered samples write no color or depth. Existing reverse-Z/depth/culling/Mask and derivative-valid source texture evaluation remain unchanged. No gain, source-material change, blending, alpha-to-coverage, sample-mask export, extra scene pass per channel or duplicate lighting evaluation is admitted; existing complete-source shadow passes remain unchanged.

No scene HDR hardware resolve target is attached, even at N4. Individual scene samples stay available until tone. The **FINAL** private front/back pair SHALL instead be fixed **Rgba16Float, linear, sample count1**, exact W×H, one2D layer and one mip, with render-attachment and texture-binding use. Both final textures contain display-LDR premultiplied toned color; they are not HDR scene sources, sRGB views or surface-format-dependent allocations. The tone pass replaces final back RGBA with §7's P, then the same private pass owner submits/swaps/publishes it.

Current CustomRenderPass::create_offscreen_texture forces gpu.format, and HAL-355 §10.1 binds the legacy pair to surface-compatible format. HAL-370 §8/HAL-355 §17.1 adopt the exact **profile-specific internal final-format extension of that same pass/registration owner**. Its implementation still needs S2 authorization and S3 proof. This contract selects no new Rust method, public raw texture API, second publisher or parallel subsystem. Legacy LDR/pass callers remain gpu.format; the real native2D surface format and compositor are unchanged.

Before allocation/writes, qualify the ACTUAL enabled Device and guaranteed R32Float RENDER_ATTACHMENT | TEXTURE_BINDING support for requested N1/N4, exact unfiltered sample loads, at least4 color attachments and16 color bytes/sample, and4 fragment-stage tone texture bindings. Match Depth32Float[N] and the unchanged Rgba16Float sample1 render/binding/FILTERABLE Image view/sampler. Adapter-only optional flags or a format enum are insufficient. Request no additional Device feature, including FLOAT32_FILTERABLE or TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES. Unsupported format/count/layout/limits/filtering refuses before writes; no N4-to-N1, FP16-scene, alternate-format or quality fallback.

### 6.2 Private controlled tone layout

The tone conversion is a separate Halcyon-owned specialized pipeline, not an application WGSL entry or a change to the PBR material/light layout:

| Group / binding | N1 class | N4 class | Access / stage |
|---|---|---|---|
| group0 / b0 | ToneMapUniform16 | Same exact uniform | Read-only uniform, fragment |
| group0 / b1 | R: R32Float texture_2d<f32> | R: R32Float texture_multisampled_2d<f32> | Read-only unfiltered Float texture, fragment |
| group0 / b2 | G: R32Float texture_2d<f32> | G: R32Float texture_multisampled_2d<f32> | Same exact sample-count class |
| group0 / b3 | B: R32Float texture_2d<f32> | B: R32Float texture_multisampled_2d<f32> | Same exact sample-count class |
| group0 / b4 | Coverage: R32Float texture_2d<f32> | Coverage: R32Float texture_multisampled_2d<f32> | Same exact sample-count class |
| Sampler / other groups | None | None | No resource or PBR group3 assignment |

For one in-bounds physical pixel q and sample i, reconstruct C_i=(load(R,q,i).x,load(G,q,i).x,load(B,q,i).x,load(Coverage,q,i).x). R32Float's unused returned components are NOT coverage. N1 uses textureLoad at mip0; N4 uses the same q at sample indices0,1,2,3 on ALL FOUR planes, without a mip argument. Bindings b1..b4 are unfiltered read-only float textures with the exact single-/multisampled layout class; no sampler or optional filtering feature. Do not mix samples, pixels, source/target generations or plane order. Only coverage==1 admits that sample's RGB to the unchanged tone equation; clear coverage0 contributes zero.

A fixed controlled fullscreen draw SHALL cover exactly one output per physical destination pixel. HDR and final back use matching W×H local pixel coordinates, independent of the viewport's logical/on-screen origin. Pixel indices must remain inside [0,W)×[0,H); N4 indices inside [0,4). No out-of-bounds-load result, DPR recomputation or logical-coordinate compensation is admissible. Any participant scissor/viewport must be proved to initialize/convert all published pixels under actual target metrics; a stale unconverted back region cannot be published.

The private scene-writer and tone pipeline/target keys include the MRT4 R32Float storage class, fixed R/G/B/Coverage order, four-output/b1..b4 ABI, exact sample count and shader/layout identity, plus the unchanged linear Rgba16Float final count1/view/fixed state. They cannot alias or rebind the old single-FP16 scene or legacy surface-format pipeline. This is one ordinary controlled representation, not a runtime feature switch or fallback. Exposure/resource instance identity is not a pipeline variant. The tone pass replaces final RGBA with premultiplied P, without blending/gamma/second alpha; only later Image sampling uses the existing qualified filtering layout. PBR groups and View80/Object112/Object128/Material64 inputs, interstage data and shadow-only entries remain unchanged.

### 6.3 Ordered sample graph

~~~text
Update: complete immutable source request + explicit exposure + bound evidence
    -> ordinary Layout: logical geometry only
    -> existing HAL-266A preparation: final immutable current target metrics
    -> validate complete candidate, certificate, capabilities, leases and budgets
    -> reserve/prepare all compatible private resources and controlled pipelines
    -> ONE existing PBR raster/depth pass: complete bounded straight radiance
         into R32Float R/G/B/Coverage[N], common Depth32Float[N], all clear0
         NO hardware HDR resolve
    -> one private fullscreen tone pass:
         load .x of all FOUR planes at the SAME physical pixel/sample,
         apply unchanged exposure/tone separately to every covered sample,
         average tone colors and coverage into linear Rgba16Float final back[1]
    -> same captured Queue submit returns
    -> CustomRenderPass back/front swap
    -> existing registered Image publication
    -> atomic complete target-local committed association
    -> ordinary read-only Paint: target-bound WHITE Image
    -> existing Image compile/composition/output submission
         normal native surface encoding only at actual2D output, no second tone/gamma
~~~

The HDR scene path replaces the profile's legacy direct-LDR raster destination; the controlled conversion is the additional output pass. This is not a new application phase, scene graph, general Image/compositor pass or scheduler. Any future additional light/shadow scene passes are separately owned/bounded R-025/R-026 work, not authorized by the plural label here. Submission return, registration, sampled output, GPU completion, native presentation and JC visual acceptance remain distinct stages.

### 6.4 Binding distinct HAL381 private presentation boundary — 2026-11-06 [S1]

Independent architectural review under JC's existing technical delegation and retained veto adopts only the [HAL381](SPEC-HAL-381-Source-Qualified-Loading-Presence-and-Materialization.md) presentation extension coordinated with HAL355 §17.2 and HAL370 §8.1. This does NOT modify §§6.1–6.3/7.1/8/10's canonical four R32Float source-linear/binary-Coverage planes, same-sample tone, exact alpha k/N, original final Rgba16Float pair, source radiance/material/normal/shadow law or Device features.

The separately typed, charged, source-associated private proxy target may hold fractional presentation opacity; its ONE private composite is opaque G/T against HAL381's display-linear B. It never becomes a canonical PBR scene/front or changes F. Only a fully committed, actually drained/registered and generation-content-pinned matching canonical F may enter H=`F.rgb+(1-F.alpha)*B` and the HAL381 temporal blend, with its 160 ms default and the sole explicit duration exception in [HAL381 §14](SPEC-HAL-381-Source-Qualified-Loading-Presence-and-Materialization.md#14-explicit-materialization-duration--2026-11-07-adopted). Terminal retires that temporary composite and uses the viewport-only CanonicalOverNeutral presenter, emitting `(H,1)` from genuine F before ONE inherited clip. No B rectangle, stacked Images, second tone/exposure, scene resolve, source alpha rewrite, cross-source depth equivalence or new general Image/postprocess ingress is authorized. The generic Image shader remains unchanged.

Both private presentation stages have ideal output `c*RGB+(1-c)*U`; actual rounding/filter/surface compatibility at interior and rounded edges is an explicit GPU/native gate, never a universal nearest-half/8-bit scanout claim. Registered Image metadata alone cannot pin immutable GPU content. Re-tone/source/target/cancel/Device transitions must maintain actual generation-specific read/write exclusion and queue-safe retirement. In addition to unchanged `P*(20*N+16)`, charge HAL381's `P*(12*N+8)`,108*T vertex payload, real overhead and all overlapping lifetimes under the same single/aggregate caps. Ordinary PBR and its compatible last-good remain independently eligible on optional preview refusal. This narrow boundary supersedes only exclusions that would otherwise forbid this named private presentation; it does not widen canonical fractional alpha, material blending or general transparency.

The 2026-11-07 Spectral Presence revision defines G in [HAL381 §13](SPEC-HAL-381-Source-Qualified-Loading-Presence-and-Materialization.md#13-spectral-presence--2026-11-07-adopted-appearance-revision). Only that named private composer may read its own proxy color/opacity neighbors for the contour. H, genuine F, canonical Coverage, attachment custody and one-clip output retain the rules above.

### 6.5 Binding distinct HAL374 selected-stroke presentation — 2026-11-06 [S1]

Independent architectural review under JC's existing delegation and retained veto adopts only [HAL374 §8](SPEC-HAL-374-Source-Qualified-3D-Selection-Outline-and-Halo.md), coordinated with HAL355 §17.3 and HAL370 §8.2. Canonical §§6.1–6.3/7/8/10 four R32Float source-linear/binary-Coverage planes, one PBR/depth pass, tone order, alpha k/N, final FP16 pair and original source/material/normal/shadow laws remain unchanged. The private complete opaque replay is a separate typed R32Float[N] occurrence-tag/Depth32Float[N] evaluation, not a fifth MRT or replacement HDR source. Tags are exact positive binary32 integers through16777216, clear0, constant per original packet slot, integer-position/same-sample loaded, exactly decoded without tolerance and rejected on invalidity. No new Device feature is demanded.

Only the private Rgba16Float[N]2px MAX stroke plane and Rgba16Float[1] output may carry presentation alpha distinct from binary PBR coverage. After actual matching F is fully drained/registered and content-pinned, average stroke samples to S and write premultiplied `C=(S.rgb+(1-S.a)*F.rgb, S.a+(1-S.a)*F.a)`. Zero eligible stroke copies exact F half values; None selection bypasses C entirely. This is explicitly display-space OVER resolved F, not per-sample tone equivalence, HDR resolve, canonical alpha rewrite or HAL381 neutral presentation. Only ONE eligible viewport Image receives inherited clipping ONCE. Private C cannot become canonical PBR Committed or source evidence.

In addition to unchanged canonical `P*(20*N+16)`, charge `P*(16*N+8)` plus exact edge/CPU/uniform/upload/object costs and all source/front/replay/retiring overlap under existing single/aggregate caps, with real sharing and queue-safe read/write exclusion. Wrong source/view/target/sample/front, cancellation, Device or capacity refusal withdraws Outline alone. Original current/compatible last-good remains truthful. No feature/count/format fallback, global Image path or general transparency is admitted. Executed original-source N1/N4 winner/AA/depth/alpha/no-selection/rounded-clip and native proofs remain mandatory separate gates; no physical visual approval follows from S1.

### 6.6 Binding HAL375 current-depth read — 2026-11-07 [S1]

Only [HAL375 §9](SPEC-HAL-375-3D-Anchored-Spatial-HUD-and-Information-Connectors.md#9-adopted-first-passive-pbr-hud-profile--2026-11-07-s1) permits adding `TEXTURE_BINDING` to the admitted canonical `Depth32Float[N]` generation. It is still the depth attachment written by the one existing PBR pass. With actual feature-free Device qualification the private consumer may load each same-pixel N1/N4 depth sample after its producer on the same Queue. The source/target/registered-front generation and custody must match. Canonical MRT4, Mask, reverse-Z, tone, FP16 pair, alpha and Image remain governed by §§6.1–6.3/7. The HUD result enters only its optional surface schedule. Refusal does not replace the ordinary PBR front.

## 7. Coverage resolve, transfer and mathematical proof

### 7.1 Exact ideal sampled-front equation

Let N be1 or4, k samples survive, and each surviving HDR sample load be (C_i,1). Clear or discarded/uncovered samples are exactly(0,0,0,0). Define their tone contribution as0; covered black contributes color0 but coverage1. The ideal pre-quantization premultiplied-linear front is:

~~~text
P.rgb = (sum over surviving samples of T_e(C_i)) / N
P.a   = k / N
~~~

The tone pass computes this once. It must neither premultiply each already-covered contribution again nor feed coverage-premultiplied HDR into T. It never obtains coverage from RGB brightness or tone-maps alpha.

| Sample state | N | k | Ideal final (RGB, alpha) |
|---|---:|---:|---|
| Entirely clear / all discarded | 1 or4 | 0 | (0,0,0,0) |
| Covered black | 1 | 1 | (0,0,0,1) |
| Full constant color C | 1 or4 | N | (T_e(C),1) |
| Partial coverage, all covered samples C | 4 | 1,2,3 | (T_e(C)*k/4,k/4) |
| Partial/full mixed covered colors | 4 | 1..4 | (sum T_e(C_i)/4,k/4), never tone of their average |
| Black mixed with clear | 4 | k | (0,0,0,k/4); alpha still records black coverage |

For each channel, every finite tone contribution is in [0,1), so 0<=P.rgb<=P.a<=1; for k>0 each channel is strictly below P.a in ideal arithmetic. k0 is exactly zeroRGBA. This is premultiplied coverage color by construction, regardless of mixed covered colors. At N1 the same rule reduces to opaque tone color or clear. No general-translucency conclusion follows.

### 7.2 Mandatory nonlinear-order counterexample

For N4 with exactly one covered sample C=(4,2,1), e1:

~~~text
T(C)                  = (4/5, 2/3, 1/2)
correct P             = (1/5, 1/6, 1/8, 1/4)
raw resolved HDR RGB  = (1, 1/2, 1/4)
WRONG tone-after-HDR  = (1/2, 1/3, 1/5, 1/4)
~~~

The wrong result tones coverage-premultiplied color and is neither the selected per-sample result nor a conforming substitute. Dividing by alpha first is also not a general solution: with N4, two covered scalar samples0 and4, correct P=(1/5,1/2), while tone of their covered mean2 and repremultiplication gives (1/3,1/2). The same problem occurs with fully covered mixed colors. Nonlinearity prevents swapping tone mapping with averaging; no T(alpha*C)/alpha trick is admitted.

### 7.3 Known storage contradiction and selected linear-FP16 final seam

#### 7.3.1 Known old-policy counterexamples, not an open tolerance question

The original R-027 selected a surface-compatible final back. Real headless source chooses Bgra8UnormSrgb; independent RGB-sRGB/alpha-UNORM8 rounding does **not** preserve RGB<=alpha for the admitted toned coverage domain.

Take N4, covered C_i=(1,1,1) exactly representable in scene binary16, EV+16/e65536, all other samples clear. Each covered tone is65536/65537. With normal nearest8-bit quantization:

| Covered k | Ideal P.rgb per channel | Ideal alpha | RGB code / alpha code | Sampled sRGB-decoded linear RGB | Sampled alpha | RGB-alpha |
|---:|---:|---:|---|---:|---:|---:|
| 2 | 32768/65537 ≈0.4999923707218823 | 0.5 | 188 /128 | 0.5028864580325687 | 0.5019607843137255 | +0.0009256737188432 |
| 3 | 49152/65537 ≈0.7499885560828234 | 0.75 | 225 /191 | 0.7529422167760779 | 0.7490196078431373 | +0.0039226089329406 |

These admissible values produce sampled RGB>alpha. Pre-quant tone-before-average is correct, but the old independently quantized8-bit final cannot guarantee HAL-263A §3.2/INV-002's sampled premultiplied display-LDR contract. A future tolerance, nearest-only sampler, Image repair or approximate-premult success declaration does not resolve it. This is a known contradiction in the **inactive proposed R-027 final policy**, not a new baseline production defect or permission to edit HAL-355's existing LDR path.

The architect supersedes that final-storage premise with linear Rgba16Float sample1 for both final textures. Original tone/order oracles in §§7.1–7.2 remain exact and unchanged.

#### 7.3.2 Binary16 alpha-domain and filtering proof

For N1 or4 the only ideal coverage alphas are0,1/4,1/2,3/4,1, all exactly representable in IEEE binary16:

| Alpha | Binary16 bit pattern |
|---:|---|
| 0 | 0x0000 |
| 1/4 | 0x3400 |
| 1/2 | 0x3800 |
| 3/4 | 0x3a00 |
| 1 | 0x3c00 |

As an IDEAL MODEL, let r16 be nearest binary16 rounding of finite nonnegative values. Its monotonicity and exact alpha fixed points imply0=r16(0)<=r16(P.channel)<=r16(a)=a; ideal alpha is exactly a, zero alpha has zero RGB, and final values in[0,1] do not overflow. This is a conditional mathematical model, not a universal assertion that actual Device render conversion is nearest. The two old-policy witnesses round to0.5/0.75 with exact alpha under that model. Actual output must independently satisfy the exact association contract and the selected finite oracle bounds in §7.4.

A final texel F qualifies as premultiplied display-LDR only when ACTUAL finite channels satisfy0<=F.rgb<=F.a<=1, F.a=k/N exactly and F.rgb=0 when F.a=0. Monotonic conversion with fixed zero/coverage endpoints is a sufficient representation argument where source/backend evidence establishes it; nearest rounding is not assumed to obtain it. An observed association violation is a failure, never a tolerance allowance or Image repair.

For any real nonnegative filtering weights w_j summing to1, F_j.channel<=F_j.a implies sum_j w_j*F_j.channel<=sum_j w_j*F_j.a. Linear filtering of these premultiplied texels therefore preserves association/zero-alpha color in ideal arithmetic; nearest sampling is the one-texel case. This is a representation/filter proof, **not executed GPU proof**. Actual tone/average f32 arithmetic, FP16 render conversion, filtering precision, view/bindgroup and numerical margins must still establish the sampled contract on every claimed backend. No hidden clamp, arbitrary epsilon or new evaluator is selected; failure requires a concrete owner decision.

#### 7.3.3 Final Image and unchanged native surface

The tone shader outputs linear P into the final Rgba16Float back. There is no sRGB transfer at this final3D write or at Image sampling of its linear view, and none of the four private HDR scene views is registered as Image. The existing low-level preassociated ingress, filtering sampler and target-bound WHITE tint are reused only after Device/parent qualification; this contract does not claim CustomRenderPass already accepts the final format.

HAL-263A's existing equation remains exact: for sampled premultiplied T, straight tint C and clip coverage K, source RGB=T.rgb*C.rgb*C.a, source alpha=T.a*C.a, then all four multiply once by K and use existing source-over. No extra T.a, tone, exposure, gamma or alpha repair is added to Image, SceneGraph, Batcher, DrawPlan or RenderGraph.

Only the existing2D compositor/output owns ordinary native surface encoding, including sRGB encode when that actual surface format requires it, after composing the Image. The native surface is not replaced by FP16, and this private3D final is not a new HDR display or generic2D postprocess. Its content, format, sample/filter identity, lease and final registration generation remain fully attributable.

The current proof chain is source L -> four source-linear stored/loaded R32Float planes C -> SAME-sample f32 tone/coverage average P -> actual linear final binary16 F -> existing filtered Image sample/composition -> unchanged native output. Source/evaluator, transport and actual shader/conversion/filter precision remain separate qualification obligations. The known8-bit ingress violation stays excluded; it is never renamed an acceptable tolerance.

### 7.4 Adopted finite numerical oracles and qualification boundary

The delegated architectural reviewer selects the controlled radiometric oracle: unchanged source-linear2^-26 at EV16 and2^-24 at EV14, at N1 k=0,1 and N4 k=0..4. Both have exposed radiance2^-10 and exact ideal covered tone1/1025; ideal final RGB per component is(k/N)/1025, alpha=k/N. Require bit-identical actual final arrays between the paired requests and absolute per-channel RGB error<=2^-20 against that ideal for THIS oracle. Exact alpha, zero-alpha zeroRGB, finiteness and RGB<=alpha have NO epsilon.

The separately listed stress fixtures use absolute per-channel RGB error<=2^-11: full-coverage[60000;3] at EV±16; full-coverage[60000,2^-26,0] at EV16; covered black; and original late-Greater-depth controls with far60000, near2^-26, EV16 and k=1 at N1 or k=2 near samples at N4, retaining far samples elsewhere. Clear/coverage/depth correctness remains exact. At N4 the two-near/two-far tone mixture has a same-WGSL FP32-output control0.5004878044128418; actual final binary16 readback was0.5, while mathematical nearest half is0.50048828125. Its error is within2^-11, NOT2^-20. This observation establishes neither a universal rounding mode nor a substitute final format.

The prior M4 Max/Metal Features::empty feasibility fixture proved these controlled assertions only; no test is executed by this amendment. Its recorded14 pair frames had maximum ideal RGB error2.39116389577e-7;10 stress frames had maximum0.000487804750892. These bounds are NOT full-domain/backend promises or source admission epsilons. Exact ideal tone, source radiance units, full <=60000 certificate and R024/R026 source-zero branches remain unchanged.

Before any wider support claim, separately authorized S2/S3 must state the actual source/evaluator input domain, f32/normal-subnormal range exercised, EV, sample/Mask/coverage/depth cases, backend/Device capability family, final-format conversion/Image/filtering stage and predeclared comparison rule. Keep exact evidence provenance separate from runtime source keys. The delegated architectural reviewer independently selects any essential additional numerical bound and accepts the declared domain/envelope under the existing R024/HAL370 trust boundary; executor/producer cannot extrapolate either fixture tolerance, silently narrow source support or invent a blanket threshold. Unsupported/invalid target/capability or unqualified required domain refuses through its actual owner before publication. Any observed nonfinite/negative value, alpha mismatch, RGB>alpha, zero-alpha nonzeroRGB, wrong sample/depth result or oracle failure invalidates that qualification. No clamp, floor, hidden production readback, feature switch or self-authored acceptance may conceal it.

## 8. Finite resources, reuse and lifetime

Every resource is context/device/target-qualified and generation/lease-bound. The profile owns no raw public GPU handle, global registry, persistent radiance history, independent queue or device. It reuses existing resource/retirement owners and does not select a new public API.

For checked W,H>0 and N in{1,4}, minimum logical attachment accounting is:

| Retained allocation | Logical byte/sample charge |
|---|---|
| Four R32Float scene planes R/G/B/Coverage | 4*4*W*H*N =16*W*H*N bytes |
| Matching Depth32Float | 4*W*H*N bytes |
| Fixed linear Rgba16Float final front/back pair, sample1 | b_final=8; both textures cost16*W*H bytes |
| ToneMapUniform | 16 payload bytes per live/in-flight slot, plus actual required alignment/allocator charge |
| Views/bind groups/pipelines/command work | FOUR scene texture/view objects and FOUR tone references/bindings, actual object/storage costs, finite shader/layout/pipeline variants and live/in-flight reservations under unchanged owner limits |

With P=W*H and b_final=8, minimum logical attachment payload is **P*(20*N+16) bytes**:16NP scene +4NP depth +16P for BOTH final textures, an increase8NP over the superseded P*(12*N+16) scene policy. Count P active target pixels, never4P. Charge actual four textures/views, tone references/bindings, aligned uniform slots, uploads, modules/layouts/pipeline variants and all simultaneously retained current/pending/last-good/in-flight/retiring generations, with real sharing/dedup and unchanged Direct shadow/source/independent attachment and texture caps. This is a checked accounting identity, not portable VRAM size. Complete single AND aggregate reservations before allocation/writes. Oversized N4 or old+new overlap refuses and preserves only compatible original last-good; no cap increase, early release, count/format fallback or hidden allocation.

Every active target has isolated compatible resources: scene/depth countN and distinct fixed final count1/linear-FP16/filterable Image class, with native surface format separately qualified. Count, extent, context/device, target/mount lifetime or view/filter compatibility changes qualify a structural successor and withdraw incompatible registrations before Paint. The per-profile internal final-format configuration belongs to the existing private pass owner, not a new publisher. Submitted scene and both final generations remain leased through Queue-safe retirement; no early free/successor relabel. Device loss follows existing terminal lifetime, not private restoration or a new epoch.

Stable complete content/target may reuse resources without fresh shader/pipeline churn. Only exposure-only tone reuse with §5.3's exact content proof may avoid a scene rerender. Unrelated view/model/light/material changes invalidate such reuse; no partial stale tuple is displayed under fresh labels. Retargeting or a new frame number cannot resurrect a retired intermediate.

For an unused HDR profile, no Context3D, inactive/zero target or absent 3D participant, there is no extension-driven allocation, scan, buffer write, encode, pass, job, submission or Timeline demand. Necessary retirement of previously live resources is not forbidden by this absence rule. No ordinary 2D renderer type, Image shader or unopted legacy raster acquires an HDR branch.

## 9. Admission, publication and typed failures

All fallible source/target/capability validation, reservation, shader/pipeline/layout compatibility and resource preparation finish before HDR writes. After preparation the owner encodes scene then tone into the captured submission order; it cannot discover a routine source/profile failure after publishing part of a result. Queue/device asynchronous failure retains its actual terminal stage and cannot be relabeled as a synchronous source rejection.

The following are distinguishable **required typed failure causes**, not claims that Rust variants exist today:

| Cause class | Required qualification / outcome |
|---|---|
| Source/profile certificate missing, stale, mismatched, incomplete or unprovable | Refuse that candidate before dispatch/writes; identify source/evaluator association, no invented certificate or black success. |
| Nonfinite/negative/out-of-ceiling source domain; invalid exposure | Checked admission refusal with offending qualified domain/setting; never clamp, repair or default invalid input. |
| Stale/foreign/retired material, light, model, view, context, mount, target or resource | Reject exact old identity; cannot rebind a successor or mix generations. |
| Unsupported four-plane R32Float render/sample-load/4-attachment/16-byte-per-sample/4-tone-binding capability or final Rgba16Float render/binding/FILTERABLE Image compatibility, count/limits/Naga/ABI/backend | Refuse before writes; no alternate scene/final format, sample downgrade, new feature request or arbitrary shader fallback. |
| Finite capacity, checked extent/byte arithmetic or allocation/reservation failure | Refuse before writes with exact resource/target stage; release only uncommitted reservations safely. |
| Inactive, zero or incompatible current target | No HDR work/publication; incompatible old binding withdrawn under current target owner, not relabeled valid. |
| Existing terminal device/renderer lifetime failure | Retire/withdraw through existing owners; no new recovery, post-publish readback verdict or false successful output. |

A failed candidate cannot advance committed tuple, swap/publication status or an old front's provenance. A compatible last-good final front may remain under its **original complete tuple**, with the new failure/request separately attributable. An incompatible front is withdrawn. An intermediate HDR image or completed encoder is never a final front; Accepted/Prepared/Submitted/Committed/Presented are not interchangeable outcomes.

The sequence is scene writes -> tone writes -> same Queue submit returns -> final front swap -> authorized Image registration/publication -> atomic committed association. Existing deferred registration may be queued at its lawful boundary; the consuming renderer must resolve the correct target-bound registration before sampling. CPU enqueue/commit is not GPU completion or native scan-out. No new GPU-readback requirement may fabricate a synchronous pixel-domain result.

## 10. Invariant / prohibition pairs

**INV-R027-COLOR-DOMAIN.** Scene radiance and final display-LDR are distinct: fixed linear Rgba16Float final count1, no sRGB transfer before Image; only unchanged native output encodes its surface. Role/source/format/alpha associations remain attributable.  
**FORBID-R027-COLOR-DOMAIN.** No guessed transfer/primary conversion, UI/HDR conflation, negative/nonfinite scene, implicit LDR clamp, surface-format final inheritance or8-bit/sRGB final fallback.

**INV-R027-COVERAGE.** All four scene planes clear0 together; the same surviving opaque/Mask/depth sample writes straight RGB and Coverage1, so black owns coverage. Same-sample tone averaging gives exact alpha k/N in the unchanged binary16 final, finite nonnegative RGB<=alpha and zero-alpha zeroRGB. Actual conversion/association is qualified under §§7.3–7.4, not assumed universally nearest.\
**FORBID-R027-COVERAGE.** No fractional object transparency, color-derived alpha, blending, alpha-to-coverage, writable sample-mask heuristic or double coverage application.

**INV-R027-TRANSFER.** The fixed T_e applies per covered straight scene sample before averaging; linear final Rgba16Float is sampled without sRGB conversion. HAL-263A composes once; ordinary surface encode occurs only at unchanged native output.  
**FORBID-R027-TRANSFER.** No shader gamma, alternate/local curve, second encode/decode/exposure/alpha, HDR scene Image ingress, Image repair or approximate8-bit ingress called exact premultiplied.

**INV-R027-RESOLVE.** N1/N4 reconstruct every logical C_i from .x loads of R/G/B/Coverage at one identical physical pixel/sample; N4 tones each covered sample, then divides colors/coverage once by4. No cross-plane sample or generation mix.\
**FORBID-R027-RESOLVE.** No hardware HDR resolve before tone, T of averaged HDR, T(alpha*C)/alpha substitute, sampler filtering or out-of-bounds-load dependency.

**INV-R027-EXPOSURE.** Immutable source-qualified EV in [-16,16] produces checked finite positive e and exact ToneMapUniform16; default0 only for genuine omission; any reuse satisfies §5.3.  
**FORBID-R027-EXPOSURE.** No hidden auto exposure, invalid-to-default clamp, global feedback, raw tone shader, stale uniform or old-front relabel after exposure change.

**INV-R027-BOUND.** An upstream complete certificate proves every covered source component finite, nonnegative and<=60000 before dispatch; actual representation/evaluator error is a separate mandatory proof.  
**FORBID-R027-BOUND.** No guessed light bound, silent60000 clamp, per-pixel readback admission, hidden quantization success or real-math-as-f32/backend testimony.

**INV-R027-PROVENANCE.** The FOUR planes form one atomic complete source/Context/target/resource-generation scene bundle, cleared/written/read/retired together. Each candidate/committed/front retains its complete immutable source/color/exposure/material/light/environment/model/view/target association and truthful stage. Exposure-only reuse requires the same complete live bundle and a real new tone/commit; old FP16 storage cannot be relabeled MRT4.\
**FORBID-R027-PROVENANCE.** No mixed tuple, partial commit, stale HDR reuse, exposure-only relabel, intermediate-as-front or new success written over compatible old-front evidence.

**INV-R027-TARGET.** HAL-266A qualifies current physical W,H/activity/target identity; admitted view supplies N1/N4. All four R32Float scene planes and common depth match extent/count/lifetime, while the unchanged final pair is linear Rgba16Float count1 with a qualified filterable Image view/sampler and the same private publication owner. Native surface stays unchanged.\
**FORBID-R027-TARGET.** No logical/DPR guess, sample/extent mismatch, successor rebinding, hidden count/format downgrade, inactive work or incompatible last-good sampling.

**INV-R027-RESOURCES.** The existing Device/Queue/private pass own the atomic scene/tone/final association. Reserve P*(20*N+16) minimum payload, actual four scene textures/views/tone bindings, both8-byte final textures, alignment/uploads/modules/variants and every queue-safe live generation under unchanged finite single/aggregate limits.\
**FORBID-R027-RESOURCES.** No raw GPU escape, generic 2D postprocess, unbounded history/churn, hidden allocation, public HDR resource, new device feature demand or independent recovery/scheduler.

**INV-R027-ABSENCE.** Unused HDR, no Context3D/participant and inactive/zero targets have no extension work; legacy LDR/1×/4× and all ordinary 2D paths remain unchanged.  
**FORBID-R027-ABSENCE.** No default migration, 2D scan/type/shader dependency, idle tick/job, mandatory tone pass or profile resource cost on unopted consumers.

**INV-R027-PROOF.** Selected ideal math/ABI, source certificate, parent adoption, executed f32/Naga/Device/format/sample/output and JC native acceptance are separately recorded gates.  
**FORBID-R027-PROOF.** No adoption from draft/review, GPU/native success from ideal binary16/filter proof, known8-bit alpha excess excused by tolerance, screenshot golden quality proof or self-authorized successor.

## 11. Exact parent compatibility and adoption matrix

The exact opt-in HAL-355 deltas below are S1 adopted through HAL-370 §8/HAL-355 §17.1. HAL-263A/295A/266A retain their existing same-owner/current-target/closed-ingress contracts; no amendment or new ingress is needed for this boundary. S2 authorization and actual source/backend proof remain separate.

| Current owner / exact clause | Baseline boundary or required compatibility | S1 disposition and remaining proof |
|---|---|---|
| HAL-002 §§3–5/8–10; HAL-002A §2.2 | Sole tree/IDs, immutable Update snapshots, no read-phase semantic mutation, GPU not application authority | Preserve; no constitutional exception, cross-widget peer reference or new phase. |
| HAL-355 §9.3 | Sole baseline wrapper sanitizes/clamps linear RGB and writes alpha1 | Explicit distinct HDR-capable controlled evaluator/profile that preserves certified scene RGB; legacy source/wrapper unchanged. |
| HAL-355 §§9.1/9.4/9.5 | Closed binding/controlled IR/pipeline key | Admit controlled tone source classes and fixed Rgba16Float sample1 output/pipeline key; Image filter binding remains exact. No public shader escape or light group3. |
| HAL-355 §10.1 | Baseline surface-compatible final/direct LDR resolve | Current S1 profile delta is four R32Float scene planes/common Depth32Float[N], unchanged linear Rgba16Float[1] final pair and same private owner; tone before averaging. Legacy gpu.format/native2D remain unchanged. |
| HAL-355 §10.3 | Legacy color/resolve contract cannot supply HDR implicitly | Preserve per-sample tone/coverage, exact actual association and fixed FP16 final. Nearest conversion is an ideal conditional model only; §§7.3–7.4 define finite oracle and wider-domain proof limits. No baseline relaxation. |
| HAL-355 §10.4 | One validated tuple, prepare/encode/submit/swap/publish order | Extend complete candidate/certificate/exposure and scene->tone ordering within existing preparation/submission, no new scheduler. |
| HAL-355 §11 | Complete requested/committed/front and last-good truth | Associate ALL FOUR scene planes/common depth and final/filter/view/registration generations atomically; keep compatible last-good with original provenance, no partial or stale bundle. |
| HAL-355 §13 | Finite typed admission before work | Reserve P*(20*N+16) plus actual object/alignment/old-generation costs; count P target pixels; require four attachments/16 bytes per sample/four unfiltered tone textures and final Image compatibility, with unchanged limits and no feature request. |
| HAL-355 §17 | HDR/tone mapping and illumination are explicit non-goals | S1 admits only this opt-in output profile and the coordinated light/material profiles under HAL-370; no blanket baseline relaxation. |
| HAL-263A §3.2, INV-002/004/005/006/008, §5 | Exact sampled premultiplied linear, closed low-level ingress, no alpha conversion, sole modulation | Final linear Rgba16Float filterable view must satisfy existing Image layout/sampler and actual sampled RGB<=alpha; no Image alpha/gamma repair or new ingress selected. Any actual new ingress still needs explicit amendment. |
| HAL-295A §§3.2–3.4, INV-003/004/005/006/007/008/009/010 | Existing private offscreen identity, Queue/swap/registration/lifetime/absence | Scoped private per-profile final-format configuration and bounded HDR->tone->same-owner FP16 publication; no second publisher, surface/graph change, raw public format escape or completion wait. |
| HAL-266A §§3–6, INV-001/002/003/005/006/007/009/010/011/013/014 | Final target metrics, GPU-cache mutation only, per-target preparation and Image-only Paint | Preserve and prove new mechanical work stays indexed/target-local in that existing boundary; no semantic edits, all-widget traversal, second Update or eligibility invention. |
| HAL-354 §4.1 and mounted G1 clauses | Complete semantic publication before mechanical preparation | HAL-370 §4 admits only unlinked flat extended requests at S1; linked extended requests refuse until separate exact HAL-354/266A admission. Current G1 writer/receipt/cutoff semantics remain unchanged. |
| HAL-357 §§2/6–8 | Optional lowering yields complete flat substrate request, not shader/light/output authority | A future extended tuple must lower completely before preparation; no separate domain renderer or scene store. |
| R-023 §§11–13; R-024 §§7–8.4/10, S1 binding under HAL-370 | Optional architecture and conservative material/ABI constraints are binding, not implemented radiance | Require separate S2 authorization, material numeric proof, R-025/R-026 bounded integration and source certificate before claiming HDR indoor quality. |
| HAL-309 §§2/7–8/12–14 | Repository authority/status/source/proof distinctions | Record every gate truthfully; record delegated S1 adoption independently of absent implementation, ungranted S2 and unexecuted S3/S4. |

Historical FP16-correction receipt: The earlier authoring unit changed R-023's status text only. THIS correction changes no R-023/R-024 byte or parent: required protected hashes remain those in §§14–15. All other A–J decisions, the exact per-sample curve/order/exposure, original tone-order/mixed-sample examples and eleven named pairs are retained; only the contradicted final-storage premise and its necessary obligations are superseded.

## 12. Future conformance obligations — none executed here

### 12.1 CPU/source and mathematical oracles

| Intended-path oracle | Exact observable obligation | Nearest future owner |
|---|---|---|
| Exposure / payload | Genuine omitted EV0, explicit endpoints-16/0/+16, checked multiplier/finite range; refuse nonfinite/out-of-range; reserved uniform components0; exact16-byte payload and real aligned in-flight offsets | three_d_context.rs, three_d_appearance.rs, three_d_raster.rs |
| Source certificate | Complete source/evaluator/light/emission combination bound, foreign/stale evidence refusal, actual60000 boundary, exact f32 representation margins; no coefficient-only acceptance | three_d_context.rs, three_d_appearance.rs; source light/integration owner separately admitted |
| Curve ideal/f32 and MRT4 transport | Same T0/monotonicity/maximum; source-linear identity transport, §§7.3–7.4 pair/stress oracles and exact alpha/depth; wider-domain final conversion/filter qualification remains distinct | three_d_appearance.rs, three_d_raster.rs, three_d_backend.rs |
| Coverage order | Unchanged §7.2 one-covered(4,2,1) exact rational oracle, covered0/4 versus alpha trick; full/partial/clear/black/mask at N1/N4 | three_d_raster.rs, three_d_backend.rs |
| Known8-bit failure / binary16 final | Preserve old8-bit counterexamples; exact coverage0/¼/½/¾/1, actual RGB<=alpha and zero-alpha zeroRGB; distinguish ideal nearest model from measured final conversion and convex filtering | three_d_raster.rs, three_d_backend.rs; actual front→Image/native owner |
| Complete provenance | Exposure-only identical-content reuse versus scene/view/model/light/material/target change; no stale lease, partial commit or old-front relabel | three_d_context.rs, three_d_viewport.rs |
| Budget/lifetime | Checked8WHN/4WHN/16WH minimum final-pair payload, alignment/live slots/variants/in-flight retirement, fixed final/filter versus scene format/count, target/lifetime and last-good truth | three_d_context.rs, three_d_viewport.rs |

These are live intended profile ingress/ownership distinctions, not a mandate to test infinite unsupported formats, features or tone curves. No new test file, dummy public API, sampler fallback or shader experiment is created by this documentary adoption.

### 12.2 Pinned compiler and GPU sample/output oracles

Actual Naga25.0.1/wgpu25.0.2 must validate every existing legal PBR HDR writer family with four R32Float outputs in one scene/depth pass, unchanged source/texture/normal/Mask/reveal math, b0 Uniform16 and b1..b4 exact single-/multisampled unfiltered loads. Test true N1/N4, common reverse-Z depth, unchanged final Rgba16Float sample1 render/binding/FILTERABLE Image, and separate old/new target/pipeline compatibility. Use the existing Device without additional features; table/enum/compiler success alone does not prove rendering or sampled output. Existing depth-only shadows remain source-complete and unchanged.

On each claimed backend, independently observe scene samples -> tone/average -> final linear FP16 stored channels -> actual filtered Image sampling/composition -> unchanged native surface output. Verify N1/N4 clear, covered black, full/mixed color, geometry edge/mask and unchanged §7.2 tone-order oracles. Observe all final channels/alpha, not only a composed screenshot or uniform.

Mandatory new numeric fixtures reproduce both §7.3 old8-bit failures and demonstrate the corrected fixed-format final under the same C1/EV16/k2/k3 input; final alphas exactly0.5/0.75 and sampled association must hold. Also prove all coverage alphas, zero-alpha zeroRGB, between-texel convex filtering, near-zero/saturation f32 cases and actual FP16 conversion/filter precision. Complete source/evaluator/scene conversion margins remain separate from final storage. The old known RGB>alpha violation cannot pass as a tolerance; no epsilon/clamp/algorithm change is admitted by this executor.

If actual f32 evaluation, FP16 storage or sampler/binding behavior cannot meet the sampled contract, return its exact witness and required architect decision before claiming support. Test-only readback can observe these stages after execution; it is never runtime admission, synchronous shader failure or a post-publication content rewrite.

### 12.3 Canonical native/quality gates

Existing tests/app_frame_realization_native.rs is the external canonical App::run proof owner, not a claim that an HDR fixture already exists. Future proof must show actual positive physical target, real scene and tone work, fixed linear-FP16 final through the same Queue/swap/registered Image and unchanged2D output, actual final-format filtering, input/target lifecycle, collapse/retirement/restoration and compatible old-front truth. Headless output, a passed compile, an encoder, a receipt or a reported statistic is not native presentation.

The indoor quality vertical requires source-qualified textured floor/walls and actual furniture/materials, admitted R-025/R-026 illumination and the complete bound certificate. Missing consumer UVs/tangents/materials are not supplied by HDR. AABB or physical tests establish neither light transport, shadows nor visual quality. Native sampled frames/stage evidence and JC perceptual review remain separate from numerical conformance; no screenshot/pixel goldens establish photorealism.

Compatibility proof includes unchanged ordinary 2D/legacy LDR1×/4× behavior, no extension work when unused/inactive, no second clock/phase and no extra Image/compositor pass. It does not reopen unrelated camera, primary gesture or Walk policy.

## 13. Current decisions versus remaining blocks

| Gate | Current disposition / reserved owner |
|---|---|
| Selected representation/math/ABI | 2026-11-06 MRT4 S1 ADOPTED: four source-linear R32Float planes, unchanged tone/coverage/FP16 final; conditional nearest model and finite oracle scopes are explicit. Controlled prior feasibility is not production/source-matrix/native conformance. |
| BLOCK-R027-PARENT-ADOPTION | Exact HAL-355/private-pass final-format contract and §11 coordination S1 ADOPTED under HAL-370; OPEN — separate S2 authorization, concrete integration and actual Image/target/filter/format compatibility proof. |
| BLOCK-R027-RADIANCE-CERTIFICATE | OPEN — real source/integration owner must provide truthful complete finite<=60000 certificate; exact admitted validator/source compatibility not implemented or proved. |
| BLOCK-R027-MATERIAL-LIGHT | OPEN — R-024 conservative source/f32/evaluator/producer and future R-025/R-026 actual bounded incident/integrated radiance. Group3/light ABI, transport and real-asset support are not chosen by R-027. |
| BLOCK-R027-NUMERIC-TRANSFER | OPEN for production and wider input/backend-domain acceptance. §§7.3–7.4 fix the pair/stress criteria and exact association requirements; actual source evaluation/R32Float transport/tone/final/Image/filter behavior needs separately authorized proof. No universal nearest or blanket ideal-error claim. |
| BLOCK-R027-NAGA-DEVICE | OPEN for production: four-target writer variants, b1..b4 N1/N4 loads, common depth, exact limits/features-empty admission and unchanged FP16 final/Image compatibility; isolated feasibility alone is not full integration. |
| BLOCK-R027-RESOURCE-FRONT | OPEN for production: P*(20*N+16), four objects/bindings and unchanged caps; atomic bundle/current/last-good/retiring custody, mixed old/new ABI separation, resize/cancel/device/retone/publication through the existing owner. |
| BLOCK-R027-CONSUMER-NATIVE | OPEN — complete real consumer inputs/native frames/quality, unchanged baseline/absence evidence and JC visual acceptance. No consumer edit or support claim. |

The S1 contract is adopted while implementation and proof gates remain open. Neither this status table nor typed failure causes constitute a runtime capability. No source work, new authority change or successor follows automatically.

## 14. Documentary verification and historical beforeimage receipts

**Historical original authoring cut.** R-027 was absent; R-023 beforeimage was SHA256 a9b9d326f9595691665c74a8013f300fbdb9de2f29d0960a74d1116fb0327c94 and R-02499d745f68e26690c7c695444a3126627d13ef17b2c60e488b088a2a055b36e53. Original R-027 output was8355e3f4a17ca50a214aefa25dd8342ff12c2003068efbbe7f6d8e9e04cf4862,458 lines/58253 bytes; original R-023 status-only output wasbc3cbed9c0bfc8e3df5bdcbdfeda4f893d04b0b62c94f5f081eccb404a0c7f36,483/78241. These are historical receipts, not today's configurable runtime/source claim.

**Current correction cut.** HAL-R027-ALPHA-QUANTIZATION-ARCHITECTURE-CORRECTION-02 begins from exactly those three hashes, all freshly re-read and matched. R-023/R-024 must remain unchanged. Only R-027 is corrected; its resulting source SHA/line/byte receipt is reported after full readback rather than embedded as a self-referential hash.

Exact documentary Grip proxy block (sequential, not shell):

~~~text
ws halcyon
read docs/specs/SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md max-lines 1000
read docs/specs/SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md hash
read docs/specs/SPEC-R-023-3D-Visual-Rendering-Extension-Architecture.md hash
read docs/specs/SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md hash
read src/renderer/wgpu_renderer.rs hash
read src/renderer/custom_render_pass.rs hash
state git
~~~

Verification requires complete exact readback, all eleven unchanged pair names, unchanged §§7.1–7.2 equations/examples, no stale operative surface-compatible final-front clause, and all seven named BLOCK gates still OPEN. Compare32 protected full-file hashes: prior doctrine/Constitution/parents/R-013/021/022/runtime/test/example/manifest set plus R-024, R-023 and the newly qualified wgpu_renderer source. Command blocks stay within Grip's32-command limit. Existing untracked/modified R-023/R-024 are preserved, not attributed to this unit. No Cargo, Rust/source/shader test, GPU/native job or Git mutation is performed for this documentary correction.

## 15. Source bindings and external reference limits

Paths in this section are relative to the Halcyon repository. These current hashes bind inspected facts; they establish no runtime/native conformance.

| Live file | SHA256 | Source seam used |
|---|---|---|
| Cargo.toml | 59290cea9b2d1a7750458a586509a4c98786d935faa1170ea1e756c9b62bef0f | :1–15 package0.227.18-beta, exact wgpu/Naga pins |
| src/three_d/content.rs | eb1f4ef4252a1b1cbb6a97262508f7b4f2e816125e965cab27fb08f3053bda59 | :59–218,370–447,495–557 current texture/sampler/geometry/appearance, no public HDR material slot |
| src/three_d/appearance.rs | 243c418472dffa019b3004ec33a3a2ae4c63743a6a42f35c482cd427a07bfc2f | :1685–1732 finite/clamp/alpha-one legacy fragment; :1790–1862 existing View/Object blocks |
| src/three_d/raster.rs | 192b891d7d1d399379f6dd1b28cd5a8c58bedac7b121c741499478407bccb25b | :24–50,387–456,667–722 ABI/fixed replace/MSAA; :1556–1619 direct back/hardware resolve |
| src/three_d/context.rs | 9cc31da306c5a1beb2ffb599f8df57fd6aa4999cadaf47bc9bacc950647593ef | :896–980,2085–2236,2338–2435,2565–2720 typed generations/leases/budgets/Naga/current texture upload |
| src/three_d/snapshot.rs | 1da0202cab6dd2881eb04c36d43fcdfa2df2a900fb9e0eae0cb3e764a4dc0189 | :79–200,411–510 complete packet/model identity and checked input |
| src/three_d/viewport.rs | 39621fff482c963d9555654b2ecee9da1fec6b240de8a56ae036ebb83fd5aa6a | :465–525 target reservations/depth/MSAA, :815–970 checked request, :1250–1372 submit/commit, :1442–1511 activity/WHITE Image |
| src/renderer/custom_render_pass.rs | 93520bad97dd74565a84828bf0087fc578e5323d1d92fb0ac6b20ac47fcb9503 | :330–435 captured format, :498–542 same Queue/swap, :611–633 currently gpu.format/sample1 allocation (no configurable FP16-final API), :753–763 same low-level registration |
| src/renderer/wgpu_renderer.rs | 48a853ebdd21cd1c044f27dbf01282df0f31a14e14f15d5055bbbf192deff8c3 | :1419–1436 feature-empty headless Device/Bgra8UnormSrgb, :2082–2097 exact single-sample filterable-float Image view/filtering sampler |
| src/renderer/custom_render_prepare.rs | b8d9128e06758ceb66e64ac9eac66472eae833865b14412c4d83ee85f2b04888 | :177–253 actual target metrics/activity/conversion |
| src/core/widget_tree.rs | 5f4d80dea5cf9842fa7e1967e232f628a9b8710a8f1d43bbbcd6a6048499a710 | :1393–1435 indexed participant preparation |
| src/core/app.rs | 3d6a9947001f148e9c3f2a532e50380302e94b84be7b693b69c4462edd385039 | :3267–3275 preparation owner; :18952–19008 ordinary compile/render/output |
| tests/app_frame_realization_native.rs | 5737eaf55cd499bed8def0aa544c44977edfe6dc7f0fea97d90e8bc3d8149a4f | Existing external native proof location only; no HDR execution asserted |

| Protected authority | SHA256 |
|---|---|
| HAL-002 | 8b1c3f3217ecfffb7b06502dce17e70984887f403afba59d023bacfed56f46db |
| HAL-002A | f6ab1927a311c5945bc224860d642834d4edde5c3371ead6a0576d8ffb05cc56 |
| HAL-355 | edc1bcb236f9ce104d79841ba2649767e0067aa0d1bc6a979b2c3cabbe6ed585 |
| HAL-263A | e30603ba9f3e83d48fc5b288dd8a82a37acf721df79ff15dbde18db647ca1456 |
| HAL-266A | d9cf0098ad19a2338acb562187ecb079cc0051f3aa2c83c5894ccc7f6ede6232 |
| HAL-295A | fe115aa0e61225ac8e5e2f3162604f4c36b3072fc7640103c049f4646a357ec4 |
| HAL-354 | 6d3e0f2041d2af1aab92a0216c37afff481cb6b0012f0469b7ed25b36c288c82 |
| HAL-357 | 2288f2c325c6f4a145a694420ea6d583afdce91ad8184a45787ee9a3bccd451c |
| HAL-309 | 883eabdce15a3b6d145a2de788bcb223a0a86e6c784f7ff8497c93d4ac599fc6 |
| R-023, protected current before/after | bc3cbed9c0bfc8e3df5bdcbdfeda4f893d04b0b62c94f5f081eccb404a0c7f36 |
| R-024, protected current before/after | 99d745f68e26690c7c695444a3126627d13ef17b2c60e488b088a2a055b36e53 |

The official [WGSL specification, §§14.4.1/14.4.5 and17.7.4](https://gpuweb.github.io/gpuweb/wgsl/) defines vec4<f32> size/alignment16 and distinct unfiltered single-/multisampled textureLoad forms. It is reference evidence for this selected interface, not proof of acceptance by pinned Naga25.

Pinned [wgpu25.0.2 wgpu-types](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-types/src/lib.rs) supplies allowed_usages, MULTISAMPLE_X4, sample_count_supported and FILTERABLE/sample_type metadata; Rgba16Float has8-byte texel payload. These declarations are not actual Device/filter/pipeline/output proof. The [WebGPU format reference](https://gpuweb.github.io/gpuweb/#texture-formats) distinguishes sRGB transfer from filterable float sampling; native Image layout is separately source-qualified above. No newer wgpu feature or automatic backend success is inferred.

Historical FP16-correction disposition: the known8-bit contradiction and ideal linear-FP16 final solution retain their original authoring/adoption provenance. CURRENT S1 includes the binding 2026-11-06 feature-free MRT4 scene amendment in §16; the old single-FP16 scene and universal actual-nearest premise are superseded. Production MRT4 is NOT IMPLEMENTED or S2-authorized by this documentary unit; wider source/backend qualification and native/JC acceptance remain separate OPEN gates. Earlier source/tests keep their exact evidence scope.

## 16. Attributable MRT4 S1 adoption and exact execution boundary — 2026-11-06

Independent architectural review under JC's delegated technical authority and retained constitutional/product veto adopts the seven selected clauses through `HAL-R027-HAL370-MRT4-FEATURE-FREE-S1-ADOPTION-01`. Reviewed candidate SHA256 `bfdc6a9a7a4ee5ce7cc75c767e1c0d3ebb37756120a17a0d14b43146638329f2`; reviewed R027 beforeimage `b01fc7033e817d2fba61b3ad06b30c4981f86de57bb23369b39bd51b3c69698e`; live HAL370 beforeimage `783a3a1ae76576195976960ff820ec52208e93fe24e349d048691a3c40a4a90b`. The candidate remains historical proposal evidence; this attributable joint amendment supplies S1 authority, not executor self-adoption, a constitutional exception or production permission.

| Selected clause | Operative location / preserved boundary |
|---|---|
| 1 Fixed scene bundle | §§3/6: R,G,B,Coverage R32Float, one existing scene/depth pass, zero clear, binary coverage, source units unchanged |
| 2 Source domain | §§3/4: original complete0<=L<=60000/source/certificate/normal/material/light/geometry laws; no new floor or exact-all-real promise |
| 3 Tone ABI/order | §§5/6/7: Uniform16 b0, four unfiltered b1..b4 .x loads from the SAME sample, original per-sample exposure/tone then average into unchanged FP16 final |
| 4 Numerical truth | §§7.3–7.4: exact association, controlled bit-identical radiometric pair/2^-20 oracle, listed2^-11 stress, conditional ideal nearest model; wider-domain/backend acceptance OPEN |
| 5 Capabilities | §6.1/§10: existing Device/Queue, no additional feature; all legal writer families and original PBR inputs/picking/native Image preserved |
| 6 Finite custody | §§8/10/11: P*(20N+16), four real objects/bindings, unchanged limits, truthful pre-write refusal and compatible last-good |
| 7 Atomic identity/reuse | §§4/5.3/8/9/10: whole-bundle source/target/generation, real re-tone only for identical live content, mixed-ABI separation and queue-safe retirement |

Source qualification at this documentary cut: `src/three_d/hdr.rs:193–296,365–405,633–684` still owns the OLD one-view scene/tone/allocation/front path and12N+16 accounting; `src/renderer/wgpu_renderer.rs:1620–1621,1688–1689` requests the canonical ordinary Device with Features::empty. `src/three_d/raster.rs:4265–4270,4907–4932`, `pbr.rs:997–1004`, `direct_shadow.rs:2250–2265` and `indirect.rs:1034–1040` identify current controlled writer families. Those source owners, not another renderer, must later integrate four outputs and the bundle. HAL355 §17.1 delegates the listed profile-format deltas to HAL370 §8 and binds no contradictory FP16 intermediate; HAL355 is unchanged.

This unit edits ONLY R027/HAL370; no runtime/shader/Device/source/test/asset changes or build/GPU/native execution. Existing isolated Metal M4 Max Features::empty N1/N4 feasibility remains evidence ONLY for the stated transport/tone/depth fixtures, not a full original-PBR or native result. Prior1194 black-witness measurements are not rerun/reclassified. R024/R026 source-zero/residual causes remain OPEN.

After independent S1 readback, the delegated architectural reviewer must separately authorize ONE coherent S2 GPU/native production unit at the existing HDR/raster/viewport/context/test owners: integrate all legal original certified PBR writer/profile families; prove N1/N4 four MRT and unchanged source/normal/geometry/Mask/derivatives/shadows; all k and late depth plus exact §7.4 pair/stress; unchanged final Image/native surface; target/source/bundle/last-good/current/retiring/residency/cancel/resize/retone and mixed old/new key separation; exact single/aggregate/overlap reservation and oversized typed refusal before allocation, without new caps/fallback. Actual canonical native publication and JC physical visual review remain separate stages. No claim that this HDR remedy fixes every black wedge is admitted.
