# SPEC-R-024: Typed PBR Metallic-Roughness Materials and Textures

## Adopted real-time numeric proof-boundary reform — 2026-11-03 [S1]

**Adopted / Binding [S1].** The coordinated adoption record below activates the complete 2026-11-03 reform reviewed at source revision `8d449eeb` across R024/R028/HAL370.

The dated reform clauses below are effective binding replacements or scope qualifications for their named clauses. Retained original text remains binding except to the exact extent superseded/qualified by this adopted boundary. The 2026-11-01 source-domain/backend-trust adoptions and all older unrelated S1 text and historical receipts retain their original scopes; JC's reserved constitutional/product veto remains.

The adopted contract removes mandatory production proof-only per-FOV GPU-numeric prediction sweeps while preserving original source validity, actual host checks, Context/Device/custody, actual R025 work, R027 and the existing quality gates. This documentary S1 adoption grants no implementation or S2 engineering; a separately authorized complete S2 unit remains required.

**S3/S4 remain CLOSED.**

**Earlier adopted status and dated records continue below, subject only to the exact 2026-11-03 qualification.**

## Historical drafting scope

This section and the following architecture review preserve the historical original authoring unit. Historical adversarial findings/receipts remain in §16 and the D1–D10/dual-space receipt in §17. Their task-time inactive/no-adoption statements do not override the current S1 header and §1; source receipts and mathematical decisions retain their original scope.

- Objective: Specify one complete proposed typed metallic-roughness material/content contract over the existing HAL-355 substrate, including exact channel/default semantics, immutable qualification, finite evaluation obligations and the parent admissions required before implementation. This is not a material demo or engine implementation.
- Source qualification: HAL-002/002A, adopted HAL-309, HAL-355, inactive R-023 and affected HAL-357/263A/266A/295A sections were checked. Actual geometry, texture, sampler, appearance, context/generation/lease, snapshot, controlled Naga, raster, viewport and target/publication paths were requalified.
- Governing authority and status: HAL-002 Foundational and HAL-002A Permanent are supreme; HAL-355/357/263A/266A/295A remain Adopted / Binding and HAL-309 Adopted. HAL-354 is Binding. R-023 is architect-reviewed only as a complete NONBINDING design baseline for downstream drafting. The explicit HAL-R-024-TYPED-PBR-MATERIALS-SPEC-DRAFT-01 mandate authorizes this draft only.
- Authority change, if any: NONE. This entire document is Draft / Proposed / INACTIVE / NOT BINDING. All normative language below describes a proposed future contract; it neither changes existing law nor authorizes an implementation.
- Target layer and existing state verified: Halcyon 0.227.18-beta, master HEAD `312e25c7a6a6d923ff4db26a311cff32fdb717a4`, clean tracked/index/untracked state before creation. R-024 was absent. Exact current source bindings are in §15. Existing one-texture appearance ABI, no tangent stream, single mip level and clamped LDR output are preserved facts.
- Owned mechanisms and files: CREATE ONLY `docs/specs/SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md`. All existing authority, R-023, source, tests, examples, manifest, history and user state are protected byte-exact.
- Named invariants: HAL-002 retained identity, immutable snapshots and phase/owner separation; HAL-355 typed resources, complete requested/committed tuple, reverse-Z, opaque/mask, finite budgets and 2D absence; HAL-263A final premultiplied-linear Image; HAL-266A current target preparation; HAL-295A same-Queue enqueue/publication. Proposed INV-R-024 / FORBID-R-024 pairs appear in §11.
- Forbidden behavior: Self-adoption/parent override; unordered or ownerless material bindings; raw GPU/shader-layout escape; hidden global material registry; guessed color transfer/tangents/UVs; silently dropped required channels; another renderer/device/queue/clock; 2D pollution; implicit lighting/HDR/mips/importer; stale or mixed fronts; invented GPU/backend/native-quality proof.
- Existing mechanisms inspected and reuse decision: Reuse explicit Context3D typed resource generations and leases, validated geometry/texture/samplers, immutable flat snapshot plus complete model state, controlled Naga admission, ordinary Viewport3D preparation and the existing offscreen/front/Image path. Only the explicitly proposed material profile and separately admitted parent ABI may extend those owners.
- Relevant external or dependent boundaries: Application/producer owns file/format decoding, authored parameters, content provenance, placements and semantic/physical meaning. Halcyon owns mechanical material validation/evaluation and GPU resources. R-025 lighting, R-026 environment, R-027 HDR and R-028 sampling quality remain separate, uncreated and unauthorized by this unit. The reference application supplies context only, not framework authority.
- Test ownership and placement: Documentary verification only. Future content/resource proofs belong at existing `src/tests/three_d_context.rs`, `three_d_geometry.rs`, `three_d_snapshot.rs`; shader/raster/backend proofs at `three_d_appearance.rs`, `three_d_raster.rs`, `three_d_backend.rs`; target/front proof at `three_d_viewport.rs` and `custom_render_preparation.rs`; external canonical native proof at `tests/app_frame_realization_native.rs`. §13 specifies future oracles, not current test coverage of PBR.
- Exact runnable verification commands and expected proof: §14 supplies Grip discovery, complete readback/hash and status commands. Expected proof: exactly one new complete inactive Markdown artifact, references/source assertions checked, R-023 and protected files unchanged. No Cargo build, test or native execution is warranted or run.
- Output or artifact completeness boundary: One self-contained contract with definitions, typed role/channel/default tables, storage/sampling interpretation, material response and safety, geometry qualification, immutable identity/lifetime/budgets, exact parent/ABI matrix, closed refusal meanings, proof obligations and adoption gates. No invented callable API or unassigned implementation choice is presented as current capability.
- SemVer and changelog impact: N/A — inactive document only; no release, API, version, manifest or changelog change.
- Out of scope: Runtime/shader implementation, parent/R-023 edits, adoption, R-025 onward files, importers/assets, light/shadow/environment/HDR/mip systems, camera/Capture decisions, physical truth, general translucency, UI and consumer-application changes.

## Historical architecture review

- Governing SPEC and read status: Supreme HAL-002/002A, adopted HAL-309, binding HAL-355/357/263A/266A/295A and affected HAL-354 read at the scopes identified in §§2/12/15; full R-023 read and rebound.
- Widget policy or N/A: N/A — inactive material/rendering proposal; no widget behavior change.
- Interaction added and policy authority: NONE. Existing event, Focus/Capture, navigation and semantic command ownership is untouched.
- Applicable FORBID rules: Complete Constitution; HAL-355 2D-CONTAMINATION, HIDDEN-CONTEXT, RAW-GPU-ESCAPE, ARBITRARY-SHADER, UNBOUNDED-GEOMETRY, FRACTIONAL-TRANSPARENCY, DYNAMIC-CHURN, FALSE-PUBLICATION, TARGET-MISMATCH, SEMANTIC-GPU-AUTHORITY, PRIVATE-SCHEDULER and UNLIMITED-BUDGET; HAL-263A association and HAL-266A/295A phase/lifecycle prohibitions.
- Existing framework mechanism inspected: Actual public resource descriptors/registration, context-owned generational leases, semantic-presence validation, fixed vertex/fragment ABI, target preparation, submit/swap and target-bound Image emission.
- Reinvention risk: No general material bus, scene graph/ECS, global registry, raw binding interface, new renderer or second application/GPU infrastructure. Future typed material work stays at the specialized 3D boundary.
- Demo/example compliance: NONE changed or executed. The existing consumer diffuse shader is narrower evidence, not scene lighting or a PBR proof.

**Author:** JC  
**Date:** 2026-10-31  
**Status:** Adopted / Binding [S1 — PbrMetallicRoughnessRealtimeF32V1 under HAL-370]  
**Layer:** Optional specialized 3D material content / mechanical shading contract  
**Owner:** Halcyon 3D Substrate Architecture  
**Technical decision record:** HAL-R-024-TYPED-PBR-MATERIALS-SPEC-DRAFT-01  
**Final owner and veto:** JC  
**Adoption:** PERFORMED 2026-10-31 through the delegated architectural reviewer's coordinated S1 decision under JC's express decision delegation; JC retains final veto.  
**Engineering authorization:** Prior separately authorized private S2 units are independently accepted over their tested old input domain; the expanded Revision02 source domain has NO engineering authorization from this documentary unit. S3/S4 remain CLOSED.\
**Implementation / proof:** Old-domain private R-024–R-028 S2 and internal unlinked Material writer ACCEPTED; expanded source/producer/packed-f32/compiler/Device/Metal proof OPEN and NOT ACCEPTED. Public S3 and native/performance/JC S4 remain CLOSED.\
**Implementation readiness:** Expanded-source law is S1 ADOPTED, not implemented or GPU-qualified by this record; old private implementation proof is not broadened. Separate bounded engineering authorization and same-scope executed proof are required before public admission.\
**Current adoption record:** HAL-R024-R028-HAL370-S1-REAL-SOURCE-COORDINATED-ADOPTION-01. Original adoption/correction mandates retain historical provenance.

**Binding interpolation amendment — 2026-10-31:** Independent architectural review under JC's express technical-decision delegation and continuing veto selects HAL-R024-MSAA-CENTROID-CONVEXITY-CONTRACT-CORRECTION-01 / SUPERVISOR RESOLUTION 02: PBR position, LOCAL normal, linear vertex RGBA and LOCAL tangent/sign use perspective-centroid; UV0 retains perspective-center. This corrects the covered-domain premises in §§5/7/8/11/13 without changing source constraints, byte layouts or the legacy shader. R-027's independently accepted private S2 output owner is protected; this documentary amendment executes no PBR compiler/GPU/native proof and does not resume R-024 S2 or admit S3/S4.

**Binding source-domain Revision02 adoption — 2026-11-01:** Independent architectural review under JC's express technical-acceptance delegation and continuing constitutional/product veto affirmatively adopts `HAL-R024-R028-HAL370-S1-REAL-SOURCE-COORDINATED-ADOPTION-01`. The reviewed Revision02 proposal is preserved outside the active source tree, SHA-256 `81b768ded17fc89bded86bf653dca51c5a6546a82e679edcf856fb69374e67f5` (75,195 bytes/493 lines). The original proposal remains historical nonbinding material; the coordinated operative clauses in R-024/R-028/HAL-370 are now S1 binding. Old private R-024–R-028 S2 and the internal unlinked Material writer are accepted ONLY for previously tested old inputs. Expanded source/packed-f32/Naga/Device/Metal proof, public S3 and external-consumer native S4 are NOT ACCEPTED; this documentary adoption grants no new engineering work.

**Binding backend-numeric trust-boundary amendment — 2026-11-01:** Independent architectural review, under JC's approval of the policy change and technical-acceptance delegation with final veto retained, affirmatively adopts `HAL-R024-R028-HAL370-BACKEND-TRUST-S1-COORDINATED-ADOPTION-01`. The selected historical source is archived outside the active source tree, SHA-256 `0269fe6a3d8ec3021aea6b16cb5b355d6e75533a14551eb26b3d80034a1c5519` (84,056 bytes/286 lines); the original candidate remains historical DRAFT/nonbinding material. Its coordinated operative contract is now **Adopted / Binding [S1]** in R024/R028/HAL370: rigorous per-candidate source/host checks, bounded available observable backend comparisons and versioned input-domain/capability-envelope acceptance, with exact evidence provenance kept separate. Earlier decisions/receipts retain their original scope; no shader, source, runtime or test changes are executed, no S2 engineering is granted, no earlier S2 acceptance is broadened and no new Device or original external-consumer success is claimed. The unchanged original three-asset external-consumer conformance fixture remains reported RED at triangle63 before N1 FP16/Image, N4 not reached; **S3/S4 remain CLOSED**. Additional comparator bounds and backend-envelope acceptance require their separate authorized evidence/decisions.

**Binding real-time numeric proof-boundary reform adoption — 2026-11-03:** Independent architectural review under HAL370 §§1/7 and JC's express reform mandate with continuing reserved constitutional/product veto affirmatively adopts `HAL-R024-R028-HAL370-REALTIME-PROOF-S1-COORDINATED-ADOPTION-01`. The accepted source is the complete `HAL-R024-R028-HAL370-REALTIME-NUMERIC-PROOF-REFORM-01` at revision `8d449eeb` (60 R024, 66 R028 and 74 HAL370 added lines). Its coordinated 2026-11-03 clauses are now **Adopted / Binding [S1]**, effective 2026-11-03. They remove mandatory production per-FOV proof-only O(triangle/vertex) CPU simulation of GPU transform/clip/determinant/reciprocal-w/helper/UV/LOD/footprint numerics and equivalent renamed frame certificates. Original source certificates and exact still-live source/Context/generation/lease reuse, actual submitted host calculations, current target/Device/capabilities/finite custody, actual R025 shadows/cascades/depth/XY/PCF, R027 FP16/Image, R028 §12 and the 2026-11-01 source/backend split remain binding. Only the expressly inconsistent proof-only per-FOV obligation is superseded; older unrelated S1 text and receipts are unchanged. This is no constitutional amendment, S2 engineering grant, implementation/GPU/native result or S3/S4 activation; **S3/S4 remain CLOSED**.

## 1. Purpose and authority gate

HAL-355 provides typed geometry, textures and a controlled programmable appearance inside one retained offscreen viewport. Its baseline does not provide a typed multi-role physical-material system. R-024 defines the opt-in material contract while preserving every existing baseline appearance, target, depth, publication and Image behavior.

The source factors, channels and defaults retain their core glTF 2.0 meaning. The one named opt-in rendering profile is **PbrMetallicRoughnessRealtimeF32V1**. Its declared real-time approximation, conservative source/transform domain and exact local-frame-first evaluator are specified below; they are not full glTF optical accuracy or universal valid-asset admission. Source roughness zero is accepted unchanged and evaluated through the expressly selected effective roughness, not an ideal delta mirror. This is not a glTF/GLB reader, scene importer or public capability admission; prior private implementation remains scoped to its tested old inputs. Applications decode and qualify their resources; Halcyon receives explicit immutable values. No mesh, material or rendered image certifies dimensions, collision, furniture identity or physical validity.

Every MUST/SHALL/FORBID below is binding at S1 in [HAL-370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md)'s exact coordinated scope and HAL-355 §17.1's acknowledgment. HAL-370 §1 records the delegated architectural reviewer's affirmative decision under JC's express technical-acceptance delegation; JC retains final veto, without a claim of personal technical review. R-023's earlier nonbinding review at SHA-256 `00ca077084293f59c793f86560709dd57a8d9d558535fbe2193cf36b2792a6f5` remains historical. The current S1 adoption binds the selected contract, not source implementation, S2 engineering or S3/S4 proof.

The closed scope is opaque and alpha-mask metallic-roughness surface content: base color, normal, metallic-roughness, occlusion and emissive, with exact defaults and attributable resources. R-024 defines how these data participate in the specified material response when lawful incident lighting is supplied. It does not supply direct lights, shadows, environment illumination, GI, HDR/tone mapping or mip chains, nor promise visible PBR quality without those independently admitted systems.

## 2. Governing and reference boundaries

| Authority / reference | Exact affected scope and status | Controlling boundary |
|---|---|---|
| [HAL-002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md), §§3–5, 8–10; [HAL-002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md), §2.2 | Foundational / Permanent | Sole retained ownership, ID relationships, immutable frame inputs, phase separation and sovereignty. No constitutional amendment. |
| [HAL-355](SPEC-HAL-355-Native-3D-Rendering-Substrate.md), §§3, 5–17 | Adopted / Binding | Existing resource, geometry, appearance, flat request, target, publication, budget and baseline output law. §12 lists exact extension prerequisites. |
| [HAL-357](SPEC-HAL-357-Shared-3D-Spatial-Domain-Composition.md), §§1–2, 4.2, 6–8, 12 | Adopted / Binding | Optional one-level producer lowering ends at the flat pair; no renderer material/domain hierarchy and no synthetic domain for flat consumers. |
| [HAL-263A](SPEC-HAL-263A-Image-Alpha-Contract-Correction.md), §§2–5 | Adopted / Binding | Final sampled Image and its alpha association/modulation only; not material texture decoding. |
| [HAL-266A](SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md), §§3–4, INV-001/003/005/006/007/009/010/014 | Adopted / Binding | Final current target metrics; post-layout/pre-paint render-local work from already admitted immutable input. |
| [HAL-295A](SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md), §§3–4 | Adopted / Binding | Participant-owned offscreen resources, renderer privacy, same-Queue enqueue-before-publication, safe retirement and baseline device lifetime. |
| [HAL-354](SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md), §4.1 | Binding | Update-owned complete publication and existing mounted flat G1 cutoff; not automatic carriage of new material fields. |
| [HAL-309](SPEC-HAL-309-Documentation-System.md), §§2, 7–8, 12–14 | Adopted | Draft, binding authority, implemented source, executed backend proof and JC native acceptance are separate claims. |
| [R-023](SPEC-R-023-3D-Visual-Rendering-Extension-Architecture.md), §§4–13 | Adopted / Binding [S1] under HAL-370 | Common ownership, finite resources, baseline compatibility, color/output seam and staged conformance; only HAL-370 §8 supplies the exact opt-in parent exceptions. |
| [Khronos glTF 2.0 specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html), §§3.7.2, 3.8, 3.9, 5.19–5.22, Appendix B | External format/material reference, not Halcyon adoption authority | Named channel/default conventions and mathematical model; Halcyon's explicitly narrower admitted profile still governs accepted input. |

The primary reference read identifies itself as glTF 2.0.1 (2021-10-11), generated from the KhronosGroup/glTF `main` source at commit `8e798b02d254cea97659a333cfcb20875b62bdd4` when inspected on 2026-10-31. Its material conventions do not authorize a Halcyon importer, optional glTF extension or shader ABI. Mathematical expressions below state the selected material model independently; they are not executable WGSL or evidence of an implemented pipeline.

HAL-018/190 and Proposed HAL-363/364 acquire no status here. R-013 concerns analytical 2D UI shadows; it supplies no material illumination or object-shadow authority. At R-024's original authoring cut, R-025–028 below named prospective owners only; no such file was created or adopted by R-024. Current S1 status — 2026-10-31: [R-025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md), [R-026](SPEC-R-026-Environment-and-Indirect-Lighting.md), [R-027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md) and [R-028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md) are independently scoped Adopted / Binding contracts coordinated through HAL-370, not implementations. S2 authorization and executed producer/backend/native proof remain OPEN; this V1 evaluator/one-mip normal contract is unchanged.

## 3. Current verified production baseline

The source observations in this section preserve the original authoring baseline; the current header separately records later accepted old-domain private S2/internal writer. They do not establish implementation or proof of the newly widened source domain.

| Actual owner and source lines | Current fact | Precise missing material connection |
|---|---|---|
| `src/three_d/content.rs:8–12, 370–447`; HAL-355 §8.1 | Fixed 36-byte position/normal/UV0/linear-quantized-color stream with semantic-presence evidence. No tangent member/stream. | A normal map cannot acquire a qualified tangent basis from baseline fallback normals/UVs. New tangent representation/interpolation needs parent admission. |
| `content.rs:59–134, 137–218` | Decoded tightly packed RGBA8 Unorm or sRGB; explicit UV address and nearest/linear min/mag sampler descriptors. | No typed material-role association, file decoding, multi-role interpretation or mip chain. |
| `content.rs:495–557`; `context.rs:1098–1151, 2205–2236` | One optional appearance texture/sampler pair, captured as leases. | No five-role material descriptor, independently typed factor block or multiple material bindings. |
| `context.rs:896–958, 2085–2202, 3534–3772` | Context/kind/slot/generation-qualified resources, checked replacement, explicit retirement and retained leases. | Reusable ownership mechanism; no current material resource kind or public material admission operation is inferred. |
| `context.rs:2338–2435`; `appearance.rs:355–438` | Semantic requirements and controlled Naga parsing/IR composition/validation. | New material evaluation must have an admitted owned ABI and corresponding structural validation; application globals cannot supply it. |
| HAL-355 §§9.1–9.4; `raster.rs:24–50, 842–887`; `appearance.rs:1685–1732` | Three baseline groups, 80/112/16-byte blocks, one sampled texture/sampler, fixed vertex input; wrapper forces finite clamped RGB and alpha one. | Material factors, role textures and tangent inputs do not fit by implication. Current `DEFAULT_COLOR_FACTOR` is WHITE, not a hidden PBR parameter block. |
| `context.rs:2565–2720` | Active preparation uploads descriptor texels directly to one D2 layer and one mip level (`2629`); sampler mip filter does not create mips. | R-024 cannot claim mip, compressed-image, HDR-texture, UV-transform or extra-layer support. |
| `snapshot.rs:79–200, 411–510`; `viewport.rs:815–970` | Flat geometry/appearance packets, structural leases, matching complete model state and checked request equality/revisions. | Future material generations/profile must be completely associated at this owner boundary, not independently fetched during preparation. |
| `raster.rs:667–722, 1556–1619`; `viewport.rs:1250–1372` | CCW/back culling, reverse-Z depth, sample 1/4, replace writes, one ordinary offscreen pass; submit/swap precedes local commit. | No incident-light binding or lighting/shadow/HDR stage is established. |
| `app.rs:3267–3275`; `widget_tree.rs:1393–1435`; `custom_render_prepare.rs:212–253` | Indexed current-target preparation after final layout; activity derives from render extent and visible coverage. | Extension work must remain inside this existing boundary, with no renderer semantic callbacks or target inference. |
| `viewport.rs:1442–1479, 1502–1528`; `custom_render_pass.rs:518–542` | Target activity/retirement and ordinary full-UV WHITE Image; same captured Queue enqueue before swap/publication. | Material failure cannot relabel old fronts or create another publisher/Image ingress. |
| `examples/showcase/tab_viewport_3d.rs:104–121` | Consumer appearance evaluates a constant-direction diffuse-like term from supplied normals. | This narrower shader is neither engine scene lighting nor evidence of implemented metallic-roughness materials or native PBR quality. |

These are source facts at §15 hashes, not newly executed conformance. Current shader compilation or existing texture support does not prove the S1-adopted extension works. No current method named or typed as a new material API is asserted in this document.

## 4. Owner and complete-material model

### 4.1 Ownership

| Concern | Sole owner | Material obligation |
|---|---|---|
| File/container/image decoding; authored parameters and source mapping | Application/producer | Supply complete decoded immutable content and explicit role/encoding/UV semantics. No Halcyon file importer or guessed asset correction. |
| Material meaning, selection of authored lights, physical/product semantics | Application | Retain upstream authority. Mechanical admission or an attractive image certifies none of these meanings. |
| Material role validation, shader evaluation and interpolation | Specialized Halcyon 3D owner | Evaluate only the admitted version/profile and geometry/resource evidence. No runtime semantic fetch. |
| Texture/sampler/material generations, leases and residency | Explicit Context3D resource owner | Context-bound typed identities, checked replacement/retirement and finite effective budgets; no hidden global registry. |
| Flat structure/model/view publication | Existing producer and canonical frame owner | Publish one complete immutable association before preparation; no peer widget borrow, partial material patches or new clock. |
| Incident direct/environment light and integration | Separately admitted R-025/R-026 owners | Supply qualified illumination before a material can claim an illuminated output. No implicit ambient/light source. |
| Target/front/output | HAL-266A, HAL-355 and HAL-295A | Render only for current compatible active target; atomically publish complete source-qualified front. |
| Final color transform and Image | Future R-027 when needed, then unchanged HAL-263A | Never silently change the existing LDR wrapper or ask generic Image composition to decode/tone-map materials. |

### 4.2 Complete immutable identity

A logical material SHALL contain the explicit `PbrMetallicRoughnessRealtimeF32V1` version/profile when selected, named role membership and required-role declarations, all resolved source factors, opaque/mask classification and cutoff, the exact texture and sampler lease for every present role, and each role's admitted UV/encoding/basis interpretation. Public roles are named typed associations; the private fixed GPU binding order is §8.4, never caller-assigned semantic array indices. This document supplies the S1-adopted ABI and structural association, not a public callable Rust API or expanded-source implementation claim.

Material, texture and sampler identities SHALL be independent typed context/kind/slot/nonwrapping-generation values with leases. Equal immutable material identity means equal factors, role membership, bindings, profile and classification. Changing any of those facts creates an admitted successor material generation and the affected structural snapshot successor under HAL-355 §8.3. Replacing a texture or sampler does not mutate an old material's captured lease or silently redirect it to the newest slot generation.

Each drawing packet SHALL have exactly one tagged structural shading association: **LegacyAppearance(existing AppearanceHandle)** OR **PbrMaterial(distinct typed MaterialHandle)**. The latter is an immutable context/kind/nonwrapping-generation-qualified material with retained leases, not a global registry or second scene hierarchy. Profile/factor/binding/classification changes create material and structural snapshot successors with complete requested/committed provenance; one packet cannot be interpreted by both families. Complete ModelState/view changes remain separate, retain structural material resources/bind groups/pipelines after warm-up, and cannot patch animated factors independently. Optional HAL-357 lowering preserves the exact flat tagged association; current flat G1 does not silently gain new fields.

Each provided role remains represented and validated even when a factor makes its immediate numerical contribution zero. A zero emissive factor is not permission to ignore a stale explicit emissive resource. An absent role has the exact §5 default; a present invalid role does not become absent. Required-role declarations override optional absence: a required missing role rejects the candidate.

The complete geometry/material/source association SHALL additionally capture §7.1's explicit per-triangle A/B/C mode or genuine-absence proof, original raw and interpreted/packed direction distinction, certificate revision/work/digest and exact normal-map chain/sampler/scale dependencies. Original source mesh meaning and a derived C rendering frame are distinct even if some bytes coincide. Changed interpretation, packing or certificate requires the owning immutable geometry/material/structural and complete extension successors; independent D/E/G/HDR identities are not numerically equated or rewritten merely to alias a revision.

## 5. Roles, channels and defaults

### 5.1 Role table

All values below are resolved once into a complete immutable logical material. “Absent” means genuinely omitted, not failed, unsupported, stale, denied or over budget. Source factors/defaults retain the stated semantics. The selected PBR ABI additionally requires deterministic context-owned role-correct 1×1 bindings for absent optional roles (§8.4); this is explicit structural profile cost, not a fallback for present invalid data. It creates no allocation/work on unselected baseline/2D paths.

| Named role | Stored channels / sample interpretation | Factor and default | Exact absent-optional-role meaning |
|---|---|---|---|
| Base color | Straight RGBA; RGB stored sRGB and decoded once; A linear. RGB is diffuse color for dielectric and normal-incidence reflectance color for metal. | Base factor RGBA, each in [0,1], default (1,1,1,1). Multiply the sampled value and admitted linear vertex color. | Texture contribution (1,1,1,1); factors/vertex color still apply. No texture does not mean black or missing material. |
| Tangent-space normal | Linear numeric RGB → signed XYZ; OpenGL +Y; alpha unused. | Normal scale finite, default 1; scales decoded X/Y, not Z, before normalization. It is not restricted to [0,1] by an invented color rule. | Use qualified geometric/shading normal; no perturbation and no tangent requirement from this absent role. Never fabricate a normal map. |
| Metallic-roughness | Linear numeric G = perceptual roughness, B = metallic; R/A unused by this role. | Roughness and metallic factors each [0,1], defaults 1 and 1; multiply their named samples independently. | G = 1, B = 1 before factors. No implicit dielectric default: default metallic remains 1 unless producer supplies another factor. |
| Occlusion | Linear numeric R only; G/B/A unused by AO. | Strength s in [0,1], default 1; effective AO = 1 + s × (R − 1). | AO = 1, no occlusion attenuation. Applies only to a separately qualified indirect term. |
| Emissive | RGB stored sRGB, decoded once; A unused. | Linear RGB emissive factor in [0,1], default (0,0,0); multiply decoded RGB. | Texture contribution (1,1,1); factor still applies. Default material emission is zero. |

“Unused” above is an explicit channel semantic, not permission to discard another named role. AO R and metallic-roughness G/B may reference the same qualified linear texture generation with independently explicit role bindings, samplers and coordinates. That does not introduce an unordered packed-role ABI. A base-color/emissive sRGB lease cannot be reused as a linear numeric normal/MR/AO view merely because the bytes look compatible; another admitted storage/view interpretation and its charged ownership are required.

The source-declared rendering basis in §7.1 is part of normal-role provenance, not a role/channel conversion. A/B retain original smooth/Mikk interpretation; C deliberately supplies a geometric source shading frame and must disclose its difference from the original normals/tangents. The SAME original normal role, UV, numeric decode, source X/Y scale, alpha/Mask and all material factors remain; a present map never disappears because the original smooth basis was singular.

### 5.2 Resolved material values

Let `B` be the sampled base RGBA after RGB decoding, `C` the resolved linear base factor, and `V` the PBR perspective-centroid-interpolated straight-linear vertex RGBA (WHITE when genuinely absent under HAL-355). Each admitted source vertex RGBA component is in [0,1]; §7.2's covered-primitive convexity preserves that range independently for RGB and alpha. Let `Tmr` be the linear metallic-roughness sample, `Tao` the linear occlusion sample and `Te` the decoded emissive RGB. The resolved values are:

```text
base_rgb = B.rgb × C.rgb × V.rgb
mask_alpha = B.a × C.a × V.a
metallic = Tmr.b × metallic_factor
roughness = Tmr.g × roughness_factor
emission = Te.rgb × emissive_factor
ao = 1 + occlusion_strength × (Tao.r − 1)
```

All color products above occur after the applicable texture RGB decode. Numeric factors and existing vertex color receive no sRGB transform. Emission is not multiplied by base color, base alpha or AO. No other channel is implicitly used. In the ideal COVERED-centroid domain, qualified role sampling, centroid vertex RGBA and the unchanged factor ranges give `base_rgb` and `mask_alpha` in [0,1], as well as resolved metallic/roughness/AO in [0,1]; invalid factors reject rather than silently clamp into an apparently valid material. UV0 alone remains perspective-center: its explicit sampler/address rules qualify texture values even when that UV evaluation lies outside the primitive. This is not a vertex-color clamp, a common-point assumption or an executed f32/sampler proof.

### 5.3 Opaque, mask and sidedness

Opaque is the default classification. It ignores material alpha for visibility and every surviving written sample has alpha 1. Mask defaults its finite nonnegative cutoff to 0.5; it discards when `mask_alpha < cutoff` and writes alpha 1 otherwise. A cutoff above 1 deliberately rejects all ordinary [0,1] mask samples; it is not silently clamped. Neither mode enables fractional blending, alpha-to-coverage or general transparency. A declared Blend/transmission/glass requirement is unsupported, not an Opaque success.

Single-sided is the default and retains CCW/back culling. A deliberately double-sided material must agree with the admitted packet raster class and use no culling. The new PBR evaluator flips the fully evaluated mapped shading normal for back-facing lighting after the qualified basis/map evaluation; it does not flip source winding, model transforms or culling behind the caller's back. Contradictory material/packet classification rejects. Legacy arbitrary appearances acquire no new mandatory shading behavior. Final edge/resolve coverage remains HAL-355/HAL-263A, distinct from material alpha.

## 6. Storage, sampling and coordinate contract

### 6.1 Role-qualified color transfer

Color texture RGB SHALL use explicitly declared straight sRGB storage and a matching admitted sRGB-sampling interpretation; alpha stays linear. At a texel boundary, for a normalized stored channel `c`, the transfer is:

```text
decode_srgb(c) = c / 12.92                         for c <= 0.04045
                 ((c + 0.055) / 1.055)^2.4        otherwise
```

Normal, metallic-roughness and AO SHALL use linear numeric sampling with no color decode. For current normalized RGBA8 numeric storage, each byte contributes its normalized numeric value. An sRGB view cannot stand in for a numeric-data view. Role declaration, texture format/view encoding and sampler must agree before GPU materialization; no histogram/content heuristic supplies missing encoding.

The existing `Rgba8UnormSrgb` and `Rgba8Unorm` resource formats are reusable storage mechanisms, not existing multi-role admission. The extension SHALL not both predecode bytes and sample them as sRGB, nor sample encoded color as Unorm and pretend it is linear. sRGB sampling must decode RGB before the relevant linear filtering/evaluation; factors multiply afterward. This is a production sampling-path obligation, not permission to apply a nonlinear decode after filtering encoded values.

Material textures do not enter Image normalization. Base-color alpha is straight material mask data, so base RGB must not be premultiplied by that alpha on material upload. HAL-263A's high-level Image conversion is not a material loader. Conversely the final registered viewport front must already satisfy premultiplied-linear Image sampling, with no repair or second alpha multiplication at composition.

### 6.2 Coordinates, role bindings and sampler truth

UV0 is the minimum profile. Every present sampled role explicitly names its source coordinate set; that set must actually be present and finite in the geometry, not merely baseline fallback `(0,0)`. Coordinate origin/orientation must match the admitted producer mapping; for the glTF-compatible mapping, (0,0) addresses the image's top-left. The OpenGL +Y normal convention describes tangent-vector signs; it does not authorize a hidden image or V-coordinate flip.

UV1, nonidentity per-role texture transforms, atlas indirection and extra vertex attributes require exact separately admitted parent scope. Until then the request is explicitly unsupported. An implementation must preserve per-role coordinate/transform meaning when supported; it cannot silently use UV0, ignore a transform, combine different role coordinates or infer a shared sampler. Producer conversions are explicit new source content, not evidence that Halcyon implemented the original unsupported representation.

Each present role binds an independent explicit texture and sampler lease. Sharing an immutable lease is allowed only when all of its usage, encoding, coordinate and lifetime requirements hold. No semantic relationship follows from two roles sharing a texture, slot or sampler. A producer resolving a file format's omitted sampler must supply the resulting explicit policy; the current context's LINEAR_CLAMP fallback must not silently replace authored wrapping.

This V1 profile admits no mip-chain construction or quality promise. Single-level nearest/linear/address mechanisms require an explicitly matching source sampling contract. R-028's distinct V2 sampling contract is S1 adopted under HAL-370 without changing V1; its implementation and proof remain gated. A required mip/LOD/anisotropic/compressed/other sampling feature cannot be reported as satisfied by `mipmap_filter` over one level; unadopted or unqualified capability refuses. Requested semantics and actual sampler/storage capability remain observable.

## 7. Source domain, local frame and producer tangents

### 7.1 PbrMetallicRoughnessRealtimeF32V1 geometry domain

Every PBR triangle requires real nondegenerate original geometry and attributable original normals; a present normal map additionally requires real UV0, a nonzero UV Jacobian and finished signed source tangent provenance. HAL-355's +Z packing fallback establishes neither original presence nor a certificate. All placements retain finite positive scales, max(scale)/min(scale)<=64 and every HAL-355 transform/representability requirement; reflection/shear remain unsupported. The selected source mode below changes input qualification only, never local GS, dual transforms or the GPU layout.

#### 7.1.1 Original data, direction interpretation and LOCAL authoring space

Actual source POSITION, NORMAL, UV0, indices and proper node TRS SHALL remain attributable to their original accessor/primitive/corner identity. Missing TANGENT on a normal-mapped glTF primitive SHALL use the existing producer-owned default MikkTSpace route with actual per-corner vec4 output, exact signs and necessary seam splits. No tangent generation moves into Context3D, packing, preparation or the shader.

Each original direction SHALL remain finite, nonzero and byte-for-byte attributable. For the smooth interpretation, direction semantics SHALL be declared as mathematical normalization `v*=v/|v|`, with original raw values/bits retained, including raw f32 when original. The producer SHALL record original Mikk inputs and ALL signed corner outputs, normalization/packing revision, outward error enclosures and actual final f32 packed triples. Zero/nonfinite source directions refuse; no epsilon, component clamp or guessed unit tolerance makes them valid. This source interpretation does not edit original assets.

F64 normalization followed by f32 storage SHALL NOT be described as exact unit binary storage. Let `N_i,T_i` below denote the ACTUAL packed binary32 triples interpreted as exact reals; ideal unit directions are separately named. Qualification SHALL use the packed triples and their certified relation to the declared rendering interpretation. An old already-qualified source/generation retains its interpretation and bytes. A changed interpretation, per-triangle mode, packing or certificate is captured as a complete immutable successor, not relabeled as the old source.

Every triangle SHALL retain its original nondegenerate positions/winding and original UVs/index/corner correspondence. Mapped triangles require finite actual UV0, nonzero UV Jacobian, attributable finished original tangent data and exact rendering `tangent.w` in `{−1,+1}` shared by all three render corners after seam splitting. Modes A/B preserve the original signed smooth/Mikk interpretation; mode C explicitly supplies the geometric interpretation in §7.1.3 with its OWN UV-derived sign. Original Mikk results remain retained even in mode C; present is not relabeled missing. Other sampled roles retain their real UV requirements when normal is absent.

All source frame derivation SHALL occur in the ORIGINAL mesh LOCAL authoring space. Proper node TRS and existing Object transforms apply afterward exactly once, with positive finite scale and ratio<=64 and original representability checks. A world/metric frame pre-baked into a local stream and transformed again is invalid, as required by PM-004/006. No reflection, shear or triangle-in-pixel-center restriction is introduced. Only explicitly selected mode C sets the rendering normals to the geometric face; this is never inferred by Halcyon from an admission failure.

#### 7.1.2 Exactly three mapped source-frame modes

Each mapped source triangle SHALL have one explicitly producer-selected mode below. These are source qualification modes, not shader branches, public API spellings, a runtime search order or permission to retry a failed candidate with fabricated data.

| Mode | Required source meaning and nondegeneracy certificate |
|---|---|
| A — Original sufficient UNIT/.75 cones | Retain the old qualified source interpretation, normal/tangent cones against orthogonal original `N_face,T_uv`, packing and old ideal `3/32` GS proof. §8.2.2 proves old-domain preservation. |
| B — `AuthoredSmoothFrame` | Preserve declared original normalized smooth normals and actual signed Mikk corner tangents; certify ALL nine cross pairs on the actual packed vectors as below. No forced face normal or common N/T weights. |
| C — `SourceGeometricFrame` | For the original smooth-frame singular situation failing A/B, explicitly derive the source triangle's rendering F/U/H from original POSITION+UV under §7.1.3. Retain original normals/Mikk and disclose the changed shading interpretation. No renderer fallback or asset-name/index allowlist. |

Mode A SHALL retain ideal UNIT source directions Nstar_i and Tstar_i, qualified UNIT N_face and in-plane T_uv with N_face dot T_uv=0, and EVERY vertex inequality Nstar_i dot N_face>=3/4 and Tstar_i dot T_uv>=3/4. Its original source interpretation and exact equal per-triangle signs remain, with actual packed-vector/normalization enclosures under §7.1.1. The mapped old angle proof and ideal |g|>=3/32 are §8.2.2's sufficient subdomain, not a universal bound for B/C.

For mode B the direct certificate SHALL establish:

```text
delta_F = 1/16
M_N >= max_i |N_i|, finite
M_T >= max_j |T_j|, finite
|b| = 1
lower_outward(b · (N_i × T_j)) >= delta_F  for EVERY i,j in {0,1,2}
```

All nine cross pairs are required, including `i!=j`. The chosen `b`, nine certified lower bounds, finite upper bounds and source/packed enclosures SHALL be part of the immutable producer certificate. A test only of the three per-vertex pairs, or a finite fragment grid, is insufficient. A conservative interval whose lower endpoint does not reach1/16 is not a certificate even if a rounded midpoint passes.

Mode A remains sufficient independently; its packing slack can establish the nine inequalities as shown in §8.2.2. Mode C has its explicit constant-frame proof and packed qualification in §7.1.3. Failure to find a mode-B witness does not itself prove an original singularity or authorize mode C: the producer must capture its explicit mode-C source choice, original failure/singularity evidence and complete geometric certificate. No source is refused SOLELY for failing .75 when another selected mode is fully certified. If the selected mode fails qualification, the outcome is typed refusal; there is no fourth guessed frame or removal of a present normal role.

#### 7.1.3 Explicit SourceGeometricFrame: original triangle derivation and packed certificate

Mode C is an independently named, versioned SOURCE SHADING INTERPRETATION for the original smooth-frame singular situation in §7.1.2. The source SHALL prove existence, over §7.2's independent convex normal/tangent weights on its exact original interpreted/packed data, of nonzero n,t with n cross t=0; exhaustion of a B witness search, a failed .75 test or a tiny rounded residual is not that proof. Retain the source-bound singularity witness/verification and the separately authored C choice. It is not a claim that authored smooth normals or Mikk tangents were packed unchanged. Eligibility is not hard-coded to the six sofa indices: a future source triangle with the same source situation and bounded proof may declare this mode. A renderer may neither infer it from a failed cone/certificate nor generate it during preparation.

Using original ordered local source f32 positions and UVs interpreted as exact real values, the producer SHALL define and outward-certify:

```text
e1 = p1-p0; e2 = p2-p0
Aface = |e1 × e2| > 0
F = (e1 × e2)/Aface
du1 = u1-u0; dv1 = v1-v0; du2 = u2-u0; dv2 = v2-v0
det = du1*dv2-du2*dv1 != 0
dPdu = (e1*dv2-e2*dv1)/det
dPdv = (-e1*du2+e2*du1)/det
G = dPdu-F*dot(F,dPdu); |G| > 0
U = G/|G|
H = sign(det), exactly -1 or +1
Nstar_0 = Nstar_1 = Nstar_2 = F
Tstar_0 = Tstar_1 = Tstar_2 = U
tangent.w at all three render corners = H
```

All values and enclosing bounds SHALL be finite and representable under the existing policy. An interval containing zero for required area, determinant or projected tangent norm cannot certify the mode. No arbitrary area/UV/length epsilon, zero replacement or threshold change is selected. The producer SHALL certify original winding, UV determinant sign, projected tangent magnitude, unit/orthogonality interpretation, source/corner mapping and actual packed output, including arithmetic and witness representation errors.

The exact real equations give `F·dPdu=F·dPdv=0`, hence G=dPdu, `|F|=|U|=1` and `F·U=0`. The projection is nevertheless explicitly checked when constructing finite data. The geometric handedness follows from the ORIGINAL UV orientation:

```text
F · (dPdu × dPdv) = Aface/det
dot(H*cross(F,U), dPdv) = Aface/(abs(det)*|G|) > 0
```

Thus H orients the shader bitangent toward increasing v. It SHALL NOT be copied blindly from original Mikk.w when original normal orientation differs from F. Retained original Mikk sign and derived H are separately attributable. UV orientation is not repaired or flipped to obtain a preferred sign.

Because all three IDEAL render corners are identical, any independent convex normal and tangent weights give `n=F`, `t=U`, `n×t=F×U`, `g=U` and `|g|=1`. The argument does not need the varyings to use the same centroid point. Clipping preserves the constant source-frame identity in ideal interpolation. Local GS and the original proper dual transforms in §8.2.2 remain unchanged.

Actual PBR52 storage SHALL use the SAME certified binary32 Fp at all three normal corners, the SAME Up at all three tangent corners, and exact H. It SHALL NOT assert `|Fp|=|Up|=1` or `Fp·Up=0` by fiat. With checked packing/derivation error upper bounds `epsilon_F,epsilon_U` relative to ideal F/U, the exact projection and a sufficient outward numerical certificate are:

```text
bC = F × U, a certified ideal unit witness
bC · (Fp × Up) >= 1-epsilon_F-epsilon_U-epsilon_F*epsilon_U
certified_lower = lower_outward(1-epsilon_F-epsilon_U-epsilon_F*epsilon_U)
```

The certified lower bound, including any witness-enclosure/evaluation error, SHALL be at least `1/16`; a rounded midpoint does not qualify. Equivalently the actual packed constant frame can be checked directly with the same nine inequalities (all nine coincide) and finite `M_F>=|Fp|`, `M_U>=|Up|`; then `|g_packed_ideal|>=1/(16*M_F)`. A sharper checked constant-frame lower bound may be recorded, but ideal norm(g)=1 is not actual GPU arithmetic evidence. §8.2.4 requires rigorous derivation/packing plus bounded observable interpolation/frame consequences under predeclared output comparisons; hidden Fp/Up/GS values are not claimed directly measured. This retains the same source contract/shader and qualifies a declared input domain, not one frozen C frame.

The source output uses the ORIGINAL PbrVertex52 primary layout. Only the explicitly declared shading-frame fields differ for a mode-C triangle. Original position/UV/color and geometric triangles remain unchanged; seam expansion may duplicate render corners with a complete original-index/corner map but SHALL NOT change original logical connectivity, triangle order/winding or caster topology. R-025's independently derived geometric face and D-only supplemental stream remain original-source facts. No new attribute, uniform, binding, draw pass or shader fallback is introduced.

For the six reported sofa faces, mode C is an adopted source route to a deliberately different shading frame, not a claim that this S1 adoption has emitted or GPU-tested that buffer. Faceting, triangle-boundary discontinuities and changed normal-map appearance are possible because the original map may have been authored against smooth/Mikk shading. The producer and inspector SHALL disclose that meaning; native visual suitability requires separate native consumer observation and JC review. Physical geometry, semantic room walls/doors and shadow occluders are not modified by this shading interpretation.

#### 7.1.4 Genuinely absent normal role

Without a normal role there is no TBN, tangent, mapped UV-Jacobian or normal-texture proof obligation. Other present roles retain their real UV requirements. The old normal cone is sufficient, or a direct source-certified UNIT witness d SHALL satisfy:

```text
lower_outward(d · N_i) >= 1/16  for i=0,1,2
```

Then `d·n=sum_i a_i(d·N_i)>=1/16`, so `|n|>=1/16` and proper `L^-T` preserves nonzero. This is the exact absent-normal ideal proof. Dummy tangent/sign fields stay structurally unused; a present invalid map can never select this branch.

#### 7.1.5 Immutable mode, source and certificate association

**Adopted / Binding [S1] — 2026-11-03: qualification of §7.1.5 and §§7.1–8.2.3 source proofs.** Complete original geometry, UV, role, sampler, mip, A/B/C or genuine-absence, normal-domain, packing and source-radiance certificates SHALL remain required. Validate them when their actual immutable source/model/placement dependencies change; reuse independently checked still-live captures only with exact source, Context, generations and leases. A camera/FOV change alone SHALL NOT require rechecking unchanged certificate cells, texels or original corners. Source derivation and packing error enclosures remain source facts, not a new per-view GPU-interpolation certificate. Nothing here changes a source constant, mode, normal, tangent, sampler, evaluator or shader.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

A certificate SHALL bind exact original source/content revision; accessor/node/primitive/triangle and corner/index correspondence; original NORMAL and ALL original signed Mikk outputs; declared per-triangle mode and source shading interpretation/revision; render-corner expansion/packed bytes; generator/certificate algorithm and revisions; material/profile, normal-scale, sampler and relevant role generations. Original semantic mesh identity and derived render-frame identity SHALL remain distinct even if some output bytes coincide. Original/packed enclosures, mode-specific witnesses, checked bounds, complete cell coverage, verification work/storage counts and deterministic digest SHALL be immutable producer facts, mechanically checked by the existing owner.

Mode C SHALL be attributable as an intentionally derived shading frame, including which original corners differ and why the mode was selected. The source producer's and inspector's provenance must expose original versus derived meaning; it SHALL NOT call that triangle's output bitwise-original smooth/Mikk shading. This is a future producer/consumer obligation, not a UI edit or producer implementation in this unit.

A UNIT witness means an actual mathematical unit direction with a certified representation/normalization, not a rounded tuple asserted unit by equality or an unexplained tolerance. For example, an exactly defined normalization of a finite nonzero witness can be enclosed outward when checking inequalities. Every stored approximation/error in that representation participates in the lower-bound calculation. No public Rust type, serialized schema or shader parameter is selected by this mathematical notation.

The verifier SHALL check the bounds and complete association, not trust a Boolean validity flag, digest alone, maximum declared number or a few sampled pixels. The producer finds witnesses under its declared finite deterministic method; preparation verifies captured certificates and performs no semantic callback, runtime solver, mip generation or source repair. Arithmetic/work/storage overflow or exhausted existing policy is a capacity refusal before writes. Old/current/pending/in-flight source and certificate ownership remains charged and leased through the existing transaction.

### 7.2 Exact local-frame-first evaluation order

The DISTINCT PBR family uses `@interpolate(perspective, centroid)` for location0 render-relative position, location1 LOCAL normal, location3 straight-linear vertex RGBA and location4 LOCAL tangent/sign. Location2 UV0 alone uses `@interpolate(perspective, center)`. Each qualifier MUST match exactly between the corresponding VS output and FS input (§8.4.3); both sampling modes are perspective-correct. The existing legacy all-center interface remains unchanged.

In the IDEAL covered-centroid model, lawful positive-W clipping and perspective interpolation give each centroid varying separately convex weights; clipped values remain convex combinations of original triangle values. Form local n=sum(w_i^N*N_i) and, only for a present normal role, t=sum(w_i^T*T_i), without common weights or evaluation points. Ideal centroid receiver/color remain on/in their qualified triangle/range; center UV0 may extrapolate independently. Exact VS/FS qualifiers and local-first program remain mandatory. Actual GPU interpolation/helper consequences require §8.2.4 predeclared observable comparisons for the accepted domain/envelope, not direct hidden N/T/g/helper telemetry or an unsupplied vendor interval. Known-invalid source/host UV or unsupported capability refuses; an indeterminate modeled GPU enclosure is not source-invalid proof. No sample-frequency shader, fallback, pixel-center source restriction, extra varying or UV clamp follows.

For mapped surfaces first compute in LOCAL fragment space `g = t - n*(dot(n,t)/dot(n,n))`. Only then transform `n_world = L^-T n` and `g_world = L g`, using existing ObjectBlock normal_to_world and the upper-left 3×3 of model_relative. Normalize the world vectors with qualified magnitude-safe arithmetic. The existing world correction is solely the fixed numerical correction, a no-op ideally; it never replaces LOCAL GS. Source/IR checks establish the sequence and §8.2.4 uses predeclared observable output consequences, not direct hidden GS measurement or an invented universal roundoff bound. Reconstruct `B_world = sign * cross(N_world,T_world)` from normalized world N/T.

Decode sampled numeric normal RGB as `q = 2*RGB-1`, apply source normal scale only to q.x/q.y, then normalize with stable guarded arithmetic. The mapped direction is `normalize(T_world*q.x + B_world*q.y + N_world*q.z)` using the qualified orthonormal frame; the OpenGL +Y convention is unchanged. A flat signed tangent normal (0,0,1) reproduces the qualified shading normal. Double-sided reversal is applied to the evaluated mapped normal as §5.3 states.

If the normal role is genuinely absent, use the qualified `normalize(L^-T n)` path and NEVER sample the normal default or consume the dummy tangent/sign. No tangent stream evidence, tangent cone, TBN or normal-texture certificate is required by that absent role. The normal-only certificate in §7.1.4 is mandatory on that genuine-absence path; mapped surfaces instead carry their explicit A/B/C mode. No implicit C selection follows from a failed candidate. No fabricated arbitrary tangent or fallback normal is admitted. Mirrored UV sign does not admit reflected model placement.

### 7.3 Required producer-side missing-TANGENT route

For eligible glTF-origin normal-mapped assets without TANGENT, the producer SHALL supply a declared supported **default MikkTSpace** path based on positions, normals and the normal texture's UV association. This architectural route is SELECTED, consistent with the [Khronos geometry recommendation](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#meshes); it is not a framework importer or GPU-preparation generator.

The source producer owns the versioned attributable implementation, exact input geometry/UV/normal/primitive identity, generator revision/configuration, necessary seam/split correspondence, finished tangent vec4 output and validation. The finished original signed Mikk data and corner/seam correspondence remain attributable in EVERY mapped mode. Modes A/B retain their declared smooth/Mikk rendering interpretation; mode C additionally supplies its explicitly derived F/U/H rendering fields and UV-derived sign under §7.1.3, retaining ALL original normals/Mikk outputs separately. The selected source/packed/UV/handedness certificate and finite declared input/output/work/storage bounds are mandatory. Failure, degeneracy or unsupported content has an exact source-qualified cause; no normal-map removal or fabricated tangent may disguise it. Halcyon only consumes/validates finished data under the S1-adopted 52-byte ABI.

**BLOCK-R024-TANGENT-PRODUCER — route SELECTED; evidence and accepted input domain remain distinct.** Verify real producer/default-Mikk provenance, signed corners/seams, bounded work, selected frame and actual packed certificates deterministically. Existing accepted S2 records retain exact provenance. S3 qualifies the declared source input domain/backend envelope using §8.2.4 representative observable tests plus the mandatory original three-asset external-consumer conformance fixture at N1 and N4; legitimate new in-domain source frames need ordinary guards, not fresh S3 per frame. No generator/renderer inference or public/native acceptance follows here.

## 8. Metallic-roughness evaluation and numerical safety

### 8.1 Material response, not a light system

The following regular-domain reference model is preserved: GGX/Trowbridge-Reitz, separable Smith, Schlick, dielectric F0=.04 and Lambertian dielectric diffuse with the stated metallic partition. Here `r` denotes the reference roughness argument. For **every evaluation in PbrMetallicRoughnessRealtimeF32V1**, substitute §8.2.1's `r_eff` for that argument; the source `roughness` resolved in §5.2 remains unchanged. Let `c = base_rgb`, `m = metallic`, and unit N,V,L be the qualified final shading normal and surface-to-view/light directions in one space. Positive contributing hemispheres use `nv=N·V>0`, `nl=N·L>0`; H is normalized only for a finite nonzero V+L. On the regular nonsingular domain:

```text
alpha = r^2
a2 = alpha^2
f0 = (1 − m) × (0.04,0.04,0.04) + m × c
F = f0 + (1 − f0) × (1 − clamp(V·H,0,1))^5
c_diff = (1 − m) × c
D = a2 / (pi × ((1 − (N·H)^2) + a2 × (N·H)^2)^2)
G1(x) = 2x / (x + sqrt(a2 + (1 − a2) × x^2))
G = G1(nl) × G1(nv)
f_specular = F × D × G / (4 × nl × nv)
f_diffuse = (1 − F) × c_diff / pi
f_material = f_diffuse + f_specular
```

The Smith expression is the selected separable form, not height-correlated or undocumented Schlick-GGX substitution. Input dot products are constrained to their geometrically valid [-1,1] or [0,1] domains before square roots/powers; this handles qualified direction arithmetic, not invalid source factors. §8.2.1 gives the exact cancelled visibility evaluation for positive nl/nv, without a later division by 4*nl*nv. A valid response is nonnegative, has no diffuse at m=1, and retains dielectric F0=.04 at m=0.

The nonnegative-response premise includes §5.2/§7.2's COVERED-centroid color domain: `c` and `m` in [0,1] imply componentwise `f0` and Schlick `F` in [0,1]. With the regularized positive `a2`, qualified unit-domain dots and positive contributing hemispheres, D and the cancelled visibility denominator are positive, and both diffuse and specular are nonnegative. Undefined-direction/noncontributing branches remain §8.2.3's specified zero reflected contribution. This ideal proof does not authorize center-extrapolated negative vertex colors, a shader color clamp or an f32/backend success claim.

This is an energy-aware real-time metallic/dielectric partition, not a proof of exact energy conservation or multiple scattering. No multiple-scattering compensation, alternative lobe, clearcoat, specular-color/IOR extension, anisotropy, sheen or transmission is selected. A later change to this declared response requires an explicit material version/contract decision; shader optimization cannot quietly change it.

### 8.2 Selected real-time domain, exact ideal proof and guarded evaluation

#### 8.2.1 Source roughness versus evaluator roughness — D1/D2

**PbrMetallicRoughnessRealtimeF32V1** is one explicit opt-in versioned approximation. Preserve the complete authored/core scalar factors and resolved source `r` in [0,1], including r=0. All eventual direct AND indirect material evaluations in this profile use:

```text
r_eff = max(r, 0.045)
alpha = r_eff * r_eff
a2 = alpha * alpha
f_spec = F * D / ((nl + sqrt(a2 + (1-a2)*nl*nl))
                 * (nv + sqrt(a2 + (1-a2)*nv*nv)))
```

The last equation is for positive nl,nv and a2>0 and is EXACT algebra for §8.1's separable Smith response: substituting G1 cancels 4*nl*nv. It is not another visibility model. D, Schlick F, F0=.04 and the metallic/diffuse partition are otherwise unchanged. `alpha>=0.002025`, `a2>=0.000004100625` in ideal arithmetic; with unit-domain dots D and both visibility denominator factors are positive. The source r=0 and r=.045 therefore share the same evaluator roughness without changing their stored source identity.

0.045 is the architect-selected FP32 regularization/antialiasing approximation, not an inferred glTF constant, hidden fallback, universal optical truth or ideal delta mirror. The [Filament numerical discussion](https://google.github.io/filament/main/filament.html) reports the FP32 analytical-light precedent; it does not confer Halcyon authority or prove this profile on a backend. No additional floor/epsilon/finite peak/IOR/lobe choice is delegated. A different ideal-mirror profile needs separate authority.

**BLOCK-R024-ZERO-ROUGHNESS — architecture DECIDED for this limited profile; implementation/backend proof OPEN.** Legal core zero is accepted as source input and is not fed to alpha=0. Full glTF optical accuracy is not claimed. Required r=0/r=.045/near-grazing actual-shader and bounded-light proofs remain unexecuted.

#### 8.2.2 Source-frame and dual-transform certificate — D3/D5

**A: sufficient old cone subdomain.** The following original angle proof retains its exact scope and ideal3/32 margin.

Let c=3/4, theta=acos(c), `n=sum(w_i^N Ni)` and, for mapped surfaces, `t=sum(w_i^T Ti)`. The COVERED-centroid premise in §7.2 supplies separately convex weights `w_i^N>=0`, `sum(w_i^N)=1` and `w_i^T>=0`, `sum(w_i^T)=1`, without requiring the same centroid point. UNIT source vectors give n·N_face>=c and |n|<=1, hence |n|>=c and direction(n) within theta of N_face. Independently, |t|>=c and direction(t) within theta of orthogonal T_uv. Their angle phi lies in [pi/2-2theta,pi/2+2theta]. Since theta<pi/4, `sin(phi)>=cos(2theta)=2c²-1=1/8`.

LOCAL Gram–Schmidt gives `g=t-n*(n·t)/(n·n)`, `n·g=0`, `|g|=|t|sin(phi)>=c/8=3/32>0`, and `n·n>=9/16`. These are ideal-arithmetic inequalities over the admitted COVERED-centroid interpolation domain, not arbitrary pixel-center extrapolations or independent CPU pixel/filter samples. No fragment enumeration is needed for this ideal theorem; actual program consequences require §8.2.4 bounded observable comparisons for the declared domain/envelope, without hidden-intermediate measurement claims or universal pointwise GPU error intervals.

**B and certified packed C: independent convex frame proof.** Use §7.1.2's actual packed vectors, delta_F=1/16 and finite M_N/M_T. C's ideal unit constant frame and separately certified packed constant frame are distinguished in §7.1.3.

For each actually covered fragment, lawful clipping and the existing matched perspective-centroid qualifiers supply separately convex weights:

```text
a_i >= 0, sum_i a_i = 1
c_j >= 0, sum_j c_j = 1
n = sum_i a_i N_i
t = sum_j c_j T_j
```

No common centroid point or equality `a_i=c_i` is assumed. Bilinearity and the nine inequalities give:

```text
b · (n × t)
 = sum_i sum_j a_i c_j [b · (N_i × T_j)]
 >= delta_F * sum_i sum_j a_i c_j
 = delta_F
|n × t| >= delta_F
|n| <= M_N, |t| <= M_T
|n| >= delta_F/M_T > 0
|t| >= delta_F/M_N > 0
n·n >= (delta_F/M_T)^2 > 0
g = t - n*(n·t)/(n·n)
n·g = 0
|g| = |n × t|/|n| >= delta_F/M_N > 0
```

This is mode B's ideal nondegeneracy theorem OVER ACTUAL PACKED triples. `M_N,M_T` are necessarily positive because the cross lower bound is positive. It does not use exact-unit packed storage or an unproved same-weight interpolation. Modes A/C use their respective certified bounds while retaining the same local evaluation order.

LOCAL `g` SHALL still be formed before nonuniform transformation. For the original proper invertible model linear part L:

```text
n_world = L^-T n
g_world = L g
(L^-T n) · (L g) = n · g = 0
```

Invertibility preserves nonzero vectors. Qualified normalization therefore gives orthonormal world N/T and `B=sign*cross(N,T)`. The world correction remains the fixed floating-point correction, never a replacement for local GS; §8.2.4 separates source/IR validation from predeclared observable output tests and does not claim direct GPU correction-intermediate measurement. The original .75-cone angle proof and its3/32 ideal bound remain valid for that sufficient subdomain; `delta_F/M_N` is the certified general-route bound, not a claim that every new input has3/32.

The independent color weights likewise preserve every straight-linear source vertex RGBA component in [0,1], including alpha; §5.2's qualified texture sampling and factor products therefore preserve nonnegative bounded base color/mask alpha. The position weights preserve membership in the clipped source triangle in the correct perspective interpolation domain, including composition back to qualified source positions. Neither conclusion assumes that position, N, T, color and center UV0 share one evaluation point. Source role sampling still obeys its explicit address/filter and numerical domain; UV0 is not covered by the centroid proof.

For genuine normal absence, §7.1.4's old cone or d projection proves n nonzero; proper inverse-transpose preserves it, with no tangent branch. For mapped C, identical ideal F/U at all three corners give n=F,t=U,g=U independent of the two centroid locations. Packed Fp/Up use the certified bound in §7.1.3, not an assertion of exact unit binary storage.

**Formal old-domain preservation.**

For the old ideal UNIT domain choose orthonormal `f=N_face`, `u=T_uv`, `b=f×u`, and `c=3/4`. In this preservation proof write `Nstar_i,Tstar_j` for the IDEAL UNIT source directions, distinct from the actual packed `N_i,T_j`. For any old source normal and tangent, including different corners, write `A=Nstar_i·f>=c`, `D=Tstar_j·u>=c`. Unit lengths imply:

```text
|Nstar_i·u| <= sqrt(1-A^2)
|Tstar_j·f| <= sqrt(1-D^2)
b · (Nstar_i × Tstar_j)
 = A*D - (Nstar_i·u)*(Tstar_j·f)
 >= A*D - sqrt(1-A^2)*sqrt(1-D^2)
 >= c^2 - (1-c^2)
 = 2*c^2 - 1
 = 1/8 > 1/16
```

Thus ALL nine ideal pairs of every originally qualified mapped triangle have positive slack; equal interpolation weights and per-corner orthogonality are unnecessary. No old triangle is required to switch shading interpretation to mode C. The original cone/angle proof and its ideal `|g|>=3/32` remain the stronger mode-A subdomain claim. For absent normal, choosing `d=f` gives `d·Nstar_i>=3/4>1/16`.

Packing error must be included, not hidden by calling rounded data UNIT. For ideal unit vectors and componentwise round-to-nearest binary32 packing with gradual underflow, `u32=2^-24` gives a vector error bound `epsilon <= u32 + sqrt(3)*2^-150 < 2^-23`. Consequently:

```text
|b · (N_i × T_j) - b · (Nstar_i × Tstar_j)|
 <= epsilon_N + epsilon_T + epsilon_N*epsilon_T
1/8 - 2*epsilon - epsilon^2
 > 1/8 - 2^-22 - 2^-46
 > 1/16
```

The normal-only projection loses at most `epsilon_N`, retaining its margin as well. Any preceding source interpretation/f64 normalization or witness-representation error SHALL be included through its actual outward enclosure; the final nine inequalities are checked on the actual packed triples. The old interval-containing1 constructor is not a substitute for that enclosure or proof of exact unit storage. Old supported source meaning is retained without editing its asset; unqualified numerical assumptions are not grandfathered into new GPU claims. Backend flush/interpolation behavior belongs to §8.2.4, not this source-packing theorem.

As an ideal strict-extension example, take f=+Z, u=+X and constant `Nstar_i=(1,0,1)/sqrt(2)`, `Tstar_i=(1,0,−1)/sqrt(2)`, signs+1. Both old cone dots are `1/sqrt(2)<3/4`, but b=+Y gives every ideal cross-pair projection1. Actual packed triples still require their outward certificate. This is a mathematical example, not an external reference-asset result or executed test.

**Mandatory counterexample/repaired-order oracle.** Use N_face=(0,0,1), T_uv=(1,0,0), b=2sqrt(2)/5; source frames NA=(1/5,b,4/5), TA=(4/5,-b,1/5), NB=(1/5,-b,4/5), TB=(4/5,b,1/5), vertex pattern A/B/A, equal signs and weights(1/4,1/2,1/4). Each pair is unit/orthogonal and cone dots are4/5. L=diag(1,1,4) satisfies the selected ratio. Local n=(1/5,0,4/5), t=(4/5,0,1/5), n·n=17/25, n·t=8/25, so g=t-(8/17)n=(12/17,0,-3/17). Thus n_world=(1/5,0,1/5), g_world=(12/17,0,-12/17) are nonzero and their dot is12/85-12/85=0.

The rejected order instead interpolates already transformed n/t to (1/5,0,1/5) and (4/5,0,4/5)=4*n_world, then yields ZERO under world-only Gram–Schmidt. Per-vertex normalization cannot rescue that old symmetric example. This is an exact mathematical oracle, not executed GPU/native evidence. The old claim that local cones survive separate transforms and world-only orthogonalization is expressly withdrawn; no legacy renderer behavior is changed.

##### 8.2.2.1 Independent MSAA center-extrapolation counterexamples

Use equal positive clip W (an orthographic view permits it), local pixel axes right/up, and triangle `A=(0,35/256)`, `B=(-1,28/256)`, `C=(1,28/256)`. Its positive CCW signed double area is `7/128`. Pixel center `P=(0,0)` has exact perspective weights `(-4,5/2,5/2)` and lies OUTSIDE. A possible covered off-center sample `S=(3/8,1/8)` has weights `(4/7,3/112,45/112)`, all strictly positive. Center evaluation can therefore extrapolate even when a fragment has covered samples; center weights are not necessarily convex.

**Frame witness.** Choose `N_A=(0,0,1)`, `N_B=(3/5,0,4/5)`, `N_C=(-3/5,0,4/5)` and `T_A=(1,0,0)`, `T_B=(4/5,0,-3/5)`, `T_C=(4/5,0,3/5)`. Every source N/T pair is unit and orthogonal, with normal/tangent cone dots at least `4/5>3/4`. Source UV0 `(.5,1),(0,0),(1,0)` has nonzero Jacobian and `T_uv=+X`; all tangent signs are +1. Proper placement L=identity and a qualified normal map with blue>=128 satisfy the selected domain. At P, `n=t=(0,0,0)` EXACTLY. At the possible covered S, `n=(-9/40,0,32/35)` and `t=(32/35,0,9/40)` are nonzero. Thus source cones alone do not justify center-based N/T guards; the covered-centroid premise is necessary for the stated proof.

**Independent color witness.** Instead choose all `N_i=(0,0,1)`, L=identity and genuinely absent normal-map role, so semantic T/TBN is bypassed and the normal is exactly +Z everywhere. Legal linear vertex RGBA8 values are A=(255,255,255,255), B=C=(0,0,0,255). Center RGBA at P is exactly `(-4,-4,-4,1)`; at S it is `(4/7,4/7,4/7,1)`. With base and MR roles absent and factors all1, center evaluation gives `c=(-4,-4,-4)`, `m=r=1`. At qualified shading/view/light/half directions `N=V=L=H=+Z`, `r_eff=alpha=a2=1`, `F0=F=-4` per channel, `D=1/pi`, the cancelled specular denominator is4, diffuse is0 and `f_material=-1/pi` per channel. This violates §8.1's nonnegative response independently of the frame witness; valid source colors do not stay bounded under center extrapolation. Centroid vertex RGBA supplies the missing convexity without changing source values, BRDF or Mask semantics.

Both witnesses are exact IDEAL rational/algebraic oracles, not executed shader failures or an adopted negative-center variant. S is a theoretical possible covered position, NOT an assertion about Metal4x or another backend's fixed sample pattern. Future GPU proof must observe backend positions or experimentally establish genuine non-center coverage with center outside. The amended frame/color/position centroid policy claims convexity only for actually covered fragments; no noncovered/helper invocation, sample shading, runtime fallback or triangle-in-pixel-center restriction is inferred.

#### 8.2.3 Normal-map filtering and total guarded operations — D4/D5

For this V1 profile a present normal map retains ONE current mip level of RGBA8 Unorm NUMERIC data, original UV0 and source-selected nearest/linear/address semantics. Original finite normal_scale s in [-1024,+1024] is applied only to decoded X/Y AFTER filtering. Actual original bytes and roles remain unchanged. All BLUE>=128 is a SUFFICIENT fast proof: each decoded z>=1/255, convex filtering preserves it, and scale leaves z unchanged. It is not a universal necessary gate after the 2026-11-01 source-domain adoption.

Otherwise the complete single-level sampling domain SHALL have the exact common-UV certificate in R-028 §6, restricted to V1's one-level sampler: one constant response per nearest region or four REAL bilinear domain-corner responses, with complete addressing/seam/tie/NPOT coverage. Define q_k(s)=(s*(2R/255-1),s*(2G/255-1),2B/255-1). Within each certified cell, the actual ideal response q is a nonnegative normalized Bernstein combination of its REAL response corners Q; a mathematical unit h with lower_outward(h dot Q)>=1/510 at every corner proves |q|>=1/510 throughout the cell. Corners are responses at actual common UV boundaries, not an unconstrained hull of arbitrary source texels. A valid raw-texel separator remains a sufficient shortcut only. Capture exact source/scale/sampler/role identity, finite complete coverage, witnesses, outward bounds and work/digest before writes; omitted or unresolved cells refuse. R-028 extends this SAME proof to its actual adjacent-level common-UV domain without changing V1's one-level policy.

With the separately qualified orthonormal TBN, nonzero q maps to a nonzero direction. No negative-BLUE clamp, normal-Y/scale repair, automatic C fallback or role deletion is admitted. A failed attempted separator is not itself proof of a reachable zero; report the exact cell/cause. Actual decode/filter/scale/normalization/helper-sensitive output requires §8.2.4 observable comparisons within the declared domain/envelope; the complete ideal/source certificate remains rigorous and hidden intermediate or helper execution is not inferred as directly measured.

The controlled evaluator must guard operands BEFORE normalization, reciprocal, square root or GGX evaluation; use max-component scaling where needed for magnitude-safe normalization rather than square huge unnormalized vectors. Clamp qualified dot products to their geometrically valid unit domains before sqrt/pow. Zero-magnitude or undefined view/light/H directions and noncontributing hemispheres have exactly ZERO reflected BRDF contribution for that light; emission is independent. No NaN-then-LDR-sanitization is a successful material response.

Certified A/B/C packed source or genuine absence, complete ideal normal response and proper transforms make frame/Q guards unreachable only in the stated ideal covered-centroid theorem, not actual GPU/helper arithmetic. Preserve source checks and the fixed program; use §8.2.4 representative observable conformance for the declared input domain/backend envelope, with predictions/comparators fixed before execution. Do not claim hidden frame/normal/helper values from final pixels; missing discrimination or an essential unselected bound leaves its gate OPEN. No fallback, clamp, NaN repair, CPU fragment error or post-publication rewrite follows. Zero/undefined light contribution stays the admitted BRDF branch, not a replacement normal.

#### 8.2.4 Deterministic source/host qualification and bounded observed backend conformance

**Adopted / Binding [S1] — 2026-11-03: replacement of the deterministic boundary and qualification frequency in §8.2.4.** The renderer SHALL preserve exact accepted source meaning, valid host-submitted data, controlled shaders, supported backend conformance and finite owner-correct resources. It SHALL NOT require a universal pointwise CPU proof of GPU arithmetic as production admission.

| Boundary | Mandatory work | Reuse and limit |
|---|---|---|
| Immutable source | Complete original geometry/material/UV/role/format/mip/sampler provenance and generations; A/B/C or genuine absence; full normal-domain and ideal radiance certificates; actual source/model validity and source packing/derivation arithmetic. | Independently validate on a genuine dependency change. Exact still-live captures, Context and leases authorize reuse; an ID, digest or unproved Boolean does not. FOV alone performs no full source/texel/certificate rescan. |
| Actual changed host input | Check the supported camera pose/projection/near/far, source/model association, actual submitted object/view/normal matrices and their finite representable storage; all arithmetic actually performed by the host, including required inversion, calibration/origin and packing. Check current physical target, format/samples, Context/Device/capabilities, reservations and serials. | The owning changed-input boundary validates the actual data and affected dependencies. This does not require propagating every source vertex through a hypothetical GPU transform, clip or raster calculation. |
| Actual R025 work | Preserve current source/caster geometry, camera receivers, directional cascades, depth candidates, XY/PCF coverage, Mask dependencies, map content and required draws under R025. | These are required host preparation/rendering dependencies. The reform neither deletes their predicates nor reuses stale shadow content. |
| GPU execution | Controlled derivative-valid WGSL, the selected sampling/evaluator semantics and actual supported Device/compiler/backend class; independently accepted representative observable conformance under the versioned S3 domain/envelope. | GPU clipping, interpolation, helper/derivative/filter behavior and fragment numerical consequences use predeclared observable N1/N4 evidence. Finite tests supply no universal precision theorem, and no per-frame oracle/readback/S3 replay is required. |

The first paragraph currently called **Deterministic source/host boundary**, every “per-candidate” use in this section and the source/host column of its stage table SHALL be read with these four boundaries under this adopted reform. Actual source normalization, derivation, certificate, witness and packing arithmetic still includes its real error bounds. Ideal source-frame/normal-domain theorems and their constants remain unchanged. “Projection/clip representability” SHALL mean validation of actual submitted host data and necessary actual host calculations, not an obligation to simulate GPU results for each source corner.

Production admission SHALL NOT unconditionally perform an O(vertex count) or O(triangle count) view-dependent sweep solely to predict GPU packed transforms, clip intervals, screen determinants, reciprocal-w/helper bounds, UV planes or texture footprints. It SHALL NOT relocate the same work into a mandatory frame-local certificate, a differently named verifier or a required source-replay step. Separately useful offline diagnostics may retain a model with its scope labeled; that model is neither actual GPU telemetry nor mandatory runtime admission.

The existing GPU execution, versioned-envelope, provenance and observable-comparison paragraphs and table retain their intended source/program/conformance claims under this boundary. An inconclusive modeled GPU interval alone SHALL NOT be a malformed-source or mandatory per-frame refusal. An actual invalid source certificate, nonfinite/invalid submitted host value, failed actual host operation, stale lease, unsupported capability, exhausted limit or observed supported-backend failure retains its true cause. No guessed epsilon, silent fallback, normal repair, changed shader or blanket backend pass follows.

All existing §8.2.4 constants, R028 §12 criteria, predeclared-comparator authority and residual GPU-variance limits remain. S3/S4 are not adopted or reopened by this S1 adoption.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

**BLOCK-R024-FINITE-EVALUATOR — two separate proof boundaries.** The selected evaluator, source law and fixed shader are unchanged. S3 qualifies an explicitly declared versioned supported SOURCE INPUT DOMAIN and BACKEND CAPABILITY ENVELOPE, using independently accepted representative actual conformance evidence plus the mandatory original three-asset external-consumer conformance fixture. Exact historical S2 test provenance remains immutable; it is not a frozen list of the only legal future source values, cameras or targets. This section grants no engineering work or public capability.

**Deterministic source/host boundary.** The producer and Halcyon validator SHALL rigorously establish original immutable source/provenance and raw-versus-derived meaning; A/B/C or genuine absence; actual packed bytes and their declared interpretation; complete ideal common-UV/LOD normal certificates; finite source UVs/factors; checked CPU/host source/model/projection/clip representability; complete revisions, schemas, capacities, leases and target/publication rules. Outward enclosures include actual host normalization, derivation, witness, packing and validation arithmetic errors. A digest, Boolean, rendered average or GPU sample set cannot replace those checks. **Host versus GPU clip:** exact-dyadic calculations, nominal clips and outward intervals derived from packed matrices/positions prove only their stated modeled host predicates. They do not establish actual GPU matrix-multiply, clipping, viewport or raster precision without an applicable execution contract or accepted bounded backend evidence.

**GPU numerical execution boundary.** Fixed-function coverage, sample placement, clipping/interpolation rounding, derivative-quad/helper behavior, GPU FMA/separate evaluation, underflow/flush/rounding, native texture decode/filter/sample precision and resulting fragment arithmetic require applicable capabilities, the controlled pinned compiler/pipeline and bounded actual conformance. Where the vendor/API provides no deterministic quantitative contract, admission SHALL NOT require or assert a universal pointwise error interval. Available deterministic bounds may strengthen evidence; missing ones remain explicitly residual hardware variance. A finite suite establishes observed conformance of its predeclared cases and supports independent acceptance of the declared domain/envelope; it proves neither every fragment nor a pointwise numerical theorem for all future inputs.

**Versioned input domain and backend envelope.** The S3 decision SHALL declare the supported source eligibility/certificate routes, profiles, roles/formats/samplers, transform/view/target/sample classes, finite bounds and required capability family, controlled shader/compiler/backend configuration, platform/driver compatibility policy and known exclusions. Independently accepted representative N1/N4 adversarial tests and the unchanged original three-asset external-consumer conformance fixture support that declared domain; the decision is not an approval of one immutable frame. Every candidate SHALL pass the applicable source/host/certificate/format/capability/target/resource checks, including lawful reuse of previously independently checked immutable certificate captures with exact generations and leases. Within the accepted domain/envelope, legitimate new geometry/material, changed source values, camera motion, view and target changes require these ordinary checks and complete successor/publication rules, not a complete replay of that conformance fixture or a fresh S3 owner decision per frame. Unsupported eligibility/device/backend/feature classes, out-of-domain data, changed shader semantics or known failing drivers are not silently admitted.

**Evidence provenance versus runtime admission.** Each executed record SHALL retain its exact original assets, source/certificate/packed bytes, camera/target, actual device/backend/OS/driver/compiler and result. Those immutable facts reproduce the evidence; they are not the runtime per-frame admission key. Platform/driver compatibility SHALL be stated through the declared capability family and its regression coverage and exclusions. An OS/driver build-metadata change alone neither forces a new owner decision for each frame nor proves a new capability class qualified. Apply the declared compatibility/regression policy; changes outside it or evidence of failure require owner reassessment of the affected envelope. No universal cross-device portability, automatic shader/source-eligibility widening or new trust registry is authorized.

The boundary SHALL follow the actual dependency of a failed predicate. A negative/zero-containing conservative host interval constructed solely to enclose undocumented GPU coverage/interpolation/rounding is **indeterminate backend evidence**, not by itself proof of invalid source. It SHALL NOT become a permanent source-invalid predicate solely for that reason after the corresponding amendment and implementation are authorized. Required finite source/host algebra, invalid certificates, stale identities, unsupported actual capabilities and exhausted resources still refuse under their exact typed cause before writes. An actual source zero, invalid source division or host overflow is not reclassified as GPU uncertainty. No whole qualifier or unrelated guard may be bypassed merely because one sufficient GPU enclosure is inconclusive.

**Observable acceptance and predeclared comparison.** Before execution, every GPU proof case SHALL independently predeclare its fixture, source-grounded prediction, tested distinction, available observation surface/coordinates and falsifiable output comparison with its already authorized acceptance criterion. Permitted observations are actual final R027 FP16/registered ordinary Image output, genuine outputs of an existing source-stage probe at its stated stage, or actual observable coverage/Device results. The unchanged production PBR shader does not expose local N/T/g, dot/divide/inverse-square-root intermediates, plane coefficients or helper-lane execution. Source/IR inspection can identify the controlled sequence, and an independently predeclared normal-role toggle, side/sign branch, C constant-frame response, material/Mask or scale discriminator can test its output consequences. Final pixels or a dependent response SHALL NOT be described as direct measurement of those hidden values or execution steps. Missing/non-discriminating output leaves the claimed gate UNVERIFIED; one plausible image is not a frame/GS proof.

**Comparator authority.** Existing R028 §12 .03 sampling and .05 temporal thresholds apply ONLY to those exact metrics; they SHALL NOT become TBN, normal, GS or hidden-arithmetic tolerances. Existing authorized exact relations or output bounds may be used only for their stated claim. If an additional numerical comparator is essential and its acceptance bound is not already selected, that test gate SHALL remain OPEN until the delegated architectural reviewer selects the exact bound in a separately authorized S2 task BEFORE execution. The executor may not invent a tolerance, borrow one from another stage or introduce a production/test-only replacement shader.

The following stage table defines the complete split. `delta_F=1/16`, `delta_Q=1/510`, and `M_N,M_T` are the existing certified packed-vector bounds; all inequalities in the source/host column are mathematical/source-validation obligations.

| Stage | Deterministic source/host obligation | Required bounded backend evidence; limit of claim |
|---|---|---|
| Source interpretation/packing | Retain raw original smooth normals and all signed Mikk corners. Check declared normalization, source/packed enclosures and actual bytes. For C verify original singularity, source-local face/determinant/projected U/H, handedness, correspondence and identical Fp/Up/H at all three render corners. Rounded triples are not asserted exactly unit. | Check the exact supplied stream on the host, then test its production output consequences, including a predeclared C constant-frame/clipping discriminator. FP16/Image or a genuine existing probe output is the observation; packed data/IR inspection is not direct GPU internal-frame telemetry. |
| Interpolated mapped normal/tangent | Preserve independent convex-weight ideal theorem: B gives `norm(n)>=delta_F/M_T` and `norm(t)>=delta_F/M_N`; A retains its stronger cone bounds; C retains its constant ideal frame and separately certified packed frame. Do not assume common N/T centroid weights. | Predeclare covered/partial/clipped N1/N4 fixtures and independently predicted normal-role, side/sign or frame-sensitive output distinctions. Compare actual FP16/Image or an available genuine source-stage output under an authorized criterion. Local N/T values and interpolation residuals are not directly observed; a non-discriminating output leaves the relevant claim UNVERIFIED. |
| Cross/frame conditioning | Preserve all nine outward packed cross inequalities and ideal bilinear proof. When source/host perturbation bounds are used, `norm(Delta(n×t))<=M_T*epsilon_n+M_N*epsilon_t+epsilon_n*epsilon_t`; include host evaluation error. | Use predeclared conditioning-sensitive output discriminators through the unchanged production frame/evaluator path. The source cross bound does not measure computed GPU GS or supply an interpolation epsilon; source/IR inspection and observed output consequences remain distinct. |
| Actual local GS | Preserve the exact local-first expression, qualified nonzero ideal denominator and mode-specific ideal g bound. Host validation of its source predicates remains finite and rigorous. | Predeclare a fixture/prediction that distinguishes the required local-first GS result from the rejected ordering, using an actual normal-dependent FP16/Image response or a genuine existing probe output where available. Dot/divide/product/subtract and g remain internal, not directly measured. Apply only an authorized output criterion; absent discrimination or an unselected essential comparator leaves the gate OPEN/UNVERIFIED, with no rescue normal. |
| Absent-normal path | Require genuine absence and the original normal cone or complete d projection `>=1/16`; preserve ideal nonzero inverse-transpose result and bypass dummy tangent/normal-map data. | Use a predeclared presence/absence-sensitive production output comparison and source/IR validation of the bypass. Do not claim to measure hidden normalization or unused-branch execution from a final pixel. Genuine absence and the existing dummy-data bypass remain mandatory. |
| Dual transforms | Check positive proper transforms, ratio<=64, original absolute representability/invertibility, checked host normal matrix and submitted bytes. Ideal nonzero lower bounds use their actual qualified transforms; ratio alone proves no absolute bound. | Use predeclared scale/rotation and side/sign-sensitive output cases through the required local-first production program, with positive nonuniform scale through64. Actual FP16/Image or genuine current probe outputs test consequences; source/IR checks establish the selected sequence, not direct observation of GPU transform/normalization intermediates. |
| Scaled sampled normal | Verify original stored-parent numeric bytes, complete real common-UV/LOD/address corners, original post-filter X/Y scale and exact sampler/scale/generation association. Preserve `norm(q)>=delta_Q` and `norm(q)<=sqrt(2*s*s+1)<=sqrt(2097153)` in the ideal domain; cover all source cells and permitted boundary outcomes. | Predeclare sampler/scale/adjacent-level/boundary cases and compare actual available source-stage probe responses or normal-dependent FP16/Image. State exactly which raw sampled outputs a genuine probe exposes; do not call hidden post-scale q, native filter internals or helper execution measured. Keep delta_Q unchanged and use only the authorized comparator for the claimed observable stage. |
| Normalizing sampled q | The conditional mathematical theorem remains: if `norm(e)<=epsilon_Q<delta_Q`, exact normalization changes direction by at most `2*epsilon_Q/(delta_Q-epsilon_Q)`. Validate source q nonzero and its immutable complete certificate. | Predeclare output-sensitive normal/scale cases through the unchanged normalization path. Observe only genuine existing probe outputs or actual normal-dependent FP16/Image; inverseSqrt and normalized-q intermediates remain internal. The conditional error theorem supplies no GPU error premise or output tolerance; any essential new comparator remains OPEN pending selection by the delegated architectural reviewer before execution. |
| Actual mapped frame/output | Preserve the selected ideal orthonormal mapping, double-sided rule, exact GGX/Schlick/cancelled-Smith equations, source factor limits and complete independent radiance certificate. Controlled Naga/ABI validation must match the fixed program. | Compare actual final R027 FP16 and registered ordinary Image through the unchanged production evaluator and enabled R025/R026/R027 chain, using predeclared normal-role, side/sign, C-frame, material/Mask or scale discriminators. Only available genuine probes/coverage/Device results may supplement them. Finite pixels do not directly measure N/T/B, g, mapped normalization or hidden arithmetic; missing discrimination is UNVERIFIED and no new tolerance or replacement shader is admitted. |

All original constants and source interpretations remain: UNIT/.75 route A and ideal3/32; packed1/16 B/C; genuine-absence d proof; BLUE>=128 sufficient normal route and ideal1/255; general post-scale1/510; proper scale ratio64; normal_scale in[-1024,1024]; roughness.045; every R028 §12 threshold. R027 §4.1 remains a rigorous conservative FULL ideal source/evaluator-domain bound: every component of every possible covered value for the complete admitted material/direct/emission/E/G/model/view request is finite, nonnegative and <=60000. It is not an average, a sampled estimate or universal actual GPU-f32 precision proof. The evidence classification changes no source constraint, original data, shader/ABI/BRDF, scale/filter rule or R027 contract. R028 .03/.05 metrics supply no TBN/normal/GS tolerance.

Before S3 acceptance of the declared versioned source input domain/backend capability envelope, separately authorized S2 work SHALL provide: (1) rigorous deterministic checks and known-invalid typed refusals; (2) controlled pinned Naga/wgpu/backend and actual feature/format/sample/filter/limit validation; (3) the mandatory unchanged original three-asset external-consumer conformance fixture at N1 AND N4 through actual material, R025/R026 when present, R027 FP16 and ordinary Image; (4) representative independently predeclared N1/N4 adversarial cases for original triangle63, subpixel/partial/center-outside/clipping, helper-sensitive gradients, A/B/C/absence, six exceptional C faces, five correlated normal cases, signs/scales/samplers and all required material/sample/filter paths; (5) unchanged R028 §12 metrics and resource/front/resize/refusal evidence. Each claim uses an available observable output and falsifiable authorized comparison, not hidden-intermediate telemetry. Required native/performance/JC gates remain separate. No original oracle asset/camera edit, triangle exclusion, output repair or replacement shader is permitted.

Evidence SHALL separate exact test provenance and measured outputs from the declared supported domain/envelope and per-candidate admission checks. Record predeclared predictions, comparisons, authorized thresholds, actual observable results/residuals and unobserved/non-discriminating claims. Missing observations are not zero error. No new tolerance is selected here; an essential additional numerical comparator stays OPEN until the delegated architectural reviewer selects its bound before execution under separate S2 authority. A failed observed conformance case or unsupported capability blocks acceptance of the affected domain/envelope and returns an attributable witness. Test readback is not production admission or source authority. Known preparation/publication failures retain original compatible last-good; incompatible full keys withdraw. No detection/rollback of every unobserved GPU arithmetic fault is promised.

HAL370 §7 stages remain distinct: S1 adopts the contract; separately authorized S2 produces bounded implementation/evidence; S3 independently accepts the declared versioned source input domain/backend capability envelope with its representative tests, the mandatory original three-asset external-consumer conformance fixture and per-candidate guards; S4 separately requires native/quality/performance and JC acceptance. Valid in-domain dynamic source/view/target changes need no per-frame S3 replay. Unsupported/out-of-envelope classes and known failing drivers gain no support by analogy; compatibility metadata is handled by the declared family/regression policy, not frozen to each recorded frame. V1 and earlier supported behavior remain unchanged. No document, public symbol, compile or one image grants S3/S4.

### 8.3 Incident light, AO, emission and output

For a lawfully admitted incident radiance contribution, reflected output is the material response weighted by its positive cosine and that radiance under the separately owned integration. R-025 supplies direct-light facts/evaluation and shadows; R-026 supplies environment/indirect integration. R-024 supplies neither a global light nor a default ambient term, and there is no scene illumination available through the current appearance ABI merely because these equations are written.

AO attenuates only the explicitly qualified indirect term. It must not automatically darken independent direct illumination or emission, stand in for object shadows, or claim GI. If no indirect term is admitted, there is no indirect contribution for AO to attenuate; the role's declared data and validation remain intact. Emission contributes surface radiance independently of direct light, but does not illuminate another surface or create an application/engine light. Turning emissive geometry into a GI source requires separate transport authority.

Material BRDF or accumulated radiance is not necessarily bounded by 1. Existing HAL-355 §9.3 still owns finite RGB sanitization/clamping and alpha-one baseline output; this material contract does not remove the baseline rule, store HDR or introduce exposure/tone mapping. Before physically based indoor quality acceptance that requires valid scene radiance beyond [0,1], the complete R-027 color/HDR/tone-map chain and coordinated HAL-370/355 contract within existing263A/266A/295A must be implemented and proved, even if a limited LDR experiment does not store HDR. Baseline clamping before that chain cannot establish the required response or recover its lost radiance. Explicitly limited LDR experiments may demonstrate only their declared narrower result; they do not satisfy the indoor quality gate. R-023 §§9/11/12 carry the same dependency. No generic Image stage may supply an implicit tone map, double decode or double alpha association. Claims about physical response, stored LDR output and observed native quality must name their different stages.

### 8.4 Selected dedicated typed PBR ABI — D6/D7/D8

This is a DISTINCT Halcyon-owned shader/pipeline family for PbrMetallicRoughnessRealtimeF32V1, not dynamically injected application WGSL. The existing Device/Queue, resource caches, render participant and offscreen/Image publication are reused. Baseline 36-byte geometry, old View/Object/Appearance layouts, world-normal varying, wrapper, pipeline keys and existing public appearance behavior remain EXACT and selectable. HAL-370 §8/HAL-355 §17.1 supply the adopted opt-in exceptions; separate S2 authorization is still required before source implementation.

#### 8.4.1 Groups and MaterialBlock

| Group/binding | S1 PBR meaning / stage | Preserved baseline boundary |
|---|---|---|
| group0/b0 | Existing exact ViewBlock80 bytes, existing view purpose/visibility | No baseline byte/schema change. |
| group1/b0 | Existing exact ObjectBlock112 bytes; PBR-specific visibility VERTEX\|FRAGMENT | Its current baseline layout is VERTEX ONLY (`raster.rs:402–415`) and remains so. Only the distinct PBR layout grants fragment access to L/model_relative and L^-T/normal_to_world. |
| group2/b0 | MaterialBlock uniform EXACTLY64 bytes, FRAGMENT | The old family retains its16-byte AppearanceBlock and old group2 b0/b1/b2. |
| group2/b1..b5 | Five typed sampleable nonmultisampled D2 textures, FRAGMENT: base color, normal, metallic-roughness, AO, emissive in that order | No unordered public role array or binding-array feature. |
| group2/b6..b10 | Five explicit filtering samplers, FRAGMENT, corresponding respectively to b1..b5 | Present role sampler policy is captured immutably; no hidden replacement. |
| group3 | UNBOUND and unallocated in R-024 | R-025/R-026 separately own the S1-adopted light/environment/shadow schemas, coordinated within the native four-group profile limit. No invented light source or color result. |

| MaterialBlock byte offset | Exact16-byte field | Components / interpretation |
|---|---|---|
| 0 | base_color_factor: vec4<f32> | Source RGBA factor. |
| 16 | emissive_rgb_and_normal_scale: vec4<f32> | XYZ source emissive factor; W source normal scale. |
| 32 | mr_ao_cutoff: vec4<f32> | X metallicFactor; Y source roughnessFactor; Z occlusionStrength; W alphaCutoff. |
| 48 | role_and_mode: vec4<u32> | X role presence bits: base bit0, normal bit1, MR bit2, AO bit3, emissive bit4; all higher bits zero. Y Opaque=0 / Mask=1. Z,W MUST be zero. |

The four vec4 members each have WGSL alignment16 and size16, yielding offsets0/16/32/48, struct alignment16 and size roundUp(16,48+16)=64 under the uniform layout rules. This is host/shader byte layout proof, not evidence that a Rust representation/Naga module compiles. Binding/dynamic offsets must additionally obey the actual Device alignment; a64-byte block does not imply a64-byte dynamic stride. ObjectBlock retains model_relative at0 and the padded normal matrix at64 (112bytes total); only the new PBR stage visibility changes.

#### 8.4.2 Genuine absence, required defaults and immutable leases

Genuinely absent optional roles bind deterministic context-owned role-correct 1×1 default textures/samplers, while their presence bit is FALSE. Their semantic values remain §5: base RGBA ones; MR G/B ones; AO R one; emissive RGB ones before the source factor. Color defaults obey sRGB sampling and data defaults numeric sampling. The absent normal binding is deterministic but is NEVER sampled or sent through TBN; its value cannot claim tangent evidence. The existing deterministic context fallback sampler policy suffices for a constant1×1 absent role; no present role's authored sampler/texture is replaced by it.

These explicit absent-role bindings are mandatory structural resources for the selected PBR family and count against finite metadata/residency/sampler/bind/upload budgets. They are NOT a hidden rendering fallback. A required-missing role rejects. A present stale/invalid/unsupported role always retains its actual failure and NEVER receives an absent default. Shared actual generations are allowed only when encoding/UV/lease requirements match and real resource costs remain charged. No default material resources are allocated for unselected PBR, legacy-only or ordinary2D paths.

#### 8.4.3 Vertex stream, local varyings and structural pipeline identity

| Vertex byte offset | PBR source attribute / format | Vertex input location |
|---|---|---|
| 0 | position Float32x3 | 0 |
| 12 | local normal Float32x3 | 1 |
| 24 | UV0 Float32x2 | 2 |
| 32 | linear vertex color Unorm8x4 | 3 |
| 36 | local tangent xyz plus sign w, Float32x4 | 4 |

The PBR stride is EXACTLY52 bytes (36+16), separate from legacy stride36. Vertex fetch offsets/stride are multiples of4; vertex attribute layout is not a WGSL uniform-struct layout and does not inherit vec4 uniform alignment16 at byte36. Actual vertex-format/device and pipeline validation remains required. Missing tangent may occupy a deterministic dummy only when the normal role is genuinely absent; semantic presence remains false and the evaluator cannot consume it.

| PBR location on VS output AND matching FS input | Exact value | Required interpolation qualifier |
|---|---|---|
| 0 | render-origin-relative position vec3<f32> | `@interpolate(perspective, centroid)` |
| 1 | LOCAL normal vec3<f32>, interpolated before nonuniform transform | `@interpolate(perspective, centroid)` |
| 2 | UV0 vec2<f32> | `@interpolate(perspective, center)` |
| 3 | straight-linear vertex color vec4<f32> | `@interpolate(perspective, centroid)` |
| 4 | LOCAL tangent xyz/sign vec4<f32>; equal per-triangle +/-1 sign | `@interpolate(perspective, centroid)` |

This PBR location1 is explicitly NOT the legacy world-normal varying. The local values feed §7/§8.2's fragment-local Gram–Schmidt and dual transforms using PBR-visible group1. The D=0 interface's five user locations contain16 scalar components (3+3+2+4+4); builtin position/front_facing remain their actual separate raster interfaces. Qualifiers MUST match exactly on both stage interfaces, including UV0 center; no sample invocation is added. The sole additional D-present location is the narrowly admitted geometric normal below. Centroid varyings may use distinct covered evaluation points, and center UV0 need not share any of them. Equal source signs and convex centroid interpolation preserve the ideal +/-1 sign; a sign discontinuity is rejected, not averaged. The legacy all-center bindings, vertex36 and uniform bytes remain unchanged. Reflection remains unsupported and double-sided evaluation follows §5.3.

**Binding R-025 D-only exception, 2026-10-31:** Independent architectural review under JC's express technical delegation and retained veto adopts the exact HAL-370 §3.1/R-025 receiver-face interface. Primary PbrVertex52 and offsets0/12/24/32/36 remain unchanged. ONLY D-present adds stream slot1, stride12 Float32x3 local source-winding face normal, vertex input location5; matching VS output and FS input location5 are vec3<f32> `@interpolate(flat)`, without a sampling modifier. The controlled D expansion draws a nonindexed triangle list in original source index/triangle order: three canonical52 corner copies plus three identical qualified face-normal copies per original triangle. Thus D uses two buffers, six attributes/user varying locations and19 scalar components; D=0 retains its original pack/cache and five/16 interface with zero supplemental work. The source face is derived in checked f64 from original f32 positions, converted to finite nonzero usable f32 and transformed by Object112's true normal matrix; double-sided back-facing receivers reverse it. All corners agree, so clipping and provoking-vertex choice cannot substitute another face. This value serves R-025 geometric bias/coverage, NEVER the independent shading N, normal map, local GS or BRDF. No derivative face derivation, Ni substitution, reserved slot, new group/binding or mandatory feature is admitted. Both expanded buffers, metadata/uploads and retained generations count against existing finite policy; actual Device limits must admit2 vertex buffers/6 attributes/19 interstage components before writes.

Per-packet shading identity is the LegacyAppearance OR PbrMaterial tag in §4.2, never both. Material handles/generations and exact role leases remain structural. Uniform-factor changes create material/structure successors; model/view-only changes reuse material resources/bind groups. The PBR pipeline key includes family/ABI/profile, fixed internal shader identity, cull/raster, target color/depth and sample state. Material identity itself is NOT a pipeline-variant key. The internal evaluator undergoes complete controlled Naga/IR/interface validation; no new public arbitrary-WGSL entry, caller globals or raw GPU layout authority is admitted.

The pinned implementation currently uses wgpu25.0.2/Naga25.0.1. This profile requests no new mandatory device feature, but must check the actual admitted Device limits for3used groups, group2's11bindings,5sampled textures plus5samplers per fragment, uniform binding sizes/counts/alignment,5vertex attributes/52stride and interstage inputs, including every aggregate resource used by the real pipeline. Library default maxima or this arithmetic are not device/backend proof; the S1-adopted R-025/R-026 group3 schemas must be checked separately without exceeding the four-group profile. Concrete Naga/device/native tests remain OPEN.

For §13 D0/D3, the preceding five-attribute/16-component checks describe D=0. D-present additionally requires actual Naga25/wgpu25/Device matching flat location5, two-buffer/six-attribute/19-component validation, original-index deindex correspondence on adjacent shared-index triangles with different faces, clipped/partially covered N4 execution, proper positive nonuniform transforms and double-sided receiving orientation. The R-025 geometric-versus-shading bias counterexample must observe required comparison0 versus the incorrect Ni-derived1 through the real PCF path. No flat face value changes the original five qualifiers, declared source/packed frame certificate or local shading evaluator; D=0/legacy regressions remain mandatory.

## 9. Admission, refusal and front truth

**Adopted / Binding [S1] — 2026-11-03: replacement of proof-only admission requirements in §9.** The complete request SHALL retain exact source/model/view/target/Context association, current capabilities, finite reservations and original typed failures before writes. A genuine source dependency change requires the original full source checks; a warm FOV-only successor reuses the independently validated unchanged source and performs §8.2.4's actual changed-host checks plus required R025 work.

The refusal table's “invalid UV/basis/certificate” categories SHALL continue to mean invalid original source or actual required host data, not an inconclusive prediction of GPU clip/helper/footprint arithmetic. A known invalid program, unsupported backend class or observed failure still prevents the affected admission/acceptance under its actual owner. Runtime conformance readback, an assumed precision bound or source repair is not a replacement.

Keep the existing ordered prewrite transaction, requested-versus-committed distinction, exact compatible last-good, incompatible withdrawal and independent target ownership. Removing proof-only sweeps grants no partial front or source-admission bypass.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

The specialized owner SHALL validate the complete material/profile and all dependent geometry/resource leases before corresponding wgpu operations. GPU materialization remains demand-driven at the existing active HAL-266A boundary; CPU content registration alone grants no GPU work. Every fallible reservation, validation and candidate-state construction completes before exposing a changed request/front. No preparation semantic callback or live material lookup outside captured immutable generations is admitted.

| Input / lifecycle condition | Required outcome | State that must remain truthful |
|---|---|---|
| Optional role genuinely absent, not declared required | Use exactly §5's semantic default and §8.4's deterministic context-owned1×1 texture/sampler bindings with FALSE presence bit | These are explicit finitely charged structural PBR resources. No failed present resource is disguised as absence, and absent normal never consumes tangent/TBN. |
| Declared-required role missing | Typed missing-required-role refusal | Old requested/committed values are not partially edited. |
| Present invalid factor, role format/channel, UV, normal/tangent/basis or alpha/raster mismatch | Typed invalid-material/geometry-association refusal with actual role/cause | Never substitute another factor, normal map, UV, mask or appearance. |
| Data lacks its selected A/B/C/genuine-absence or complete source normal-domain/UV/scale/sampling certificate, or requires unsupported capability. | Typed attributable source/profile refusal before writes. Source/host validation is rigorous; backend numerical uncertainty/conformance is classified under §8.2.4 rather than called malformed source. | Preserve the prior complete association; no repair, role loss, blanket backend pass or invented fragment CPU error. |
| Wrong context/kind, stale/freed/exhausted generation or ended device lifetime | Exact resource/lifetime refusal under existing owners | No successor/recycled slot inherits the old material/source. |
| Finite slot/byte/binding/shader/pipeline/upload/staging/in-flight budget or hard device limit exceeded | Typed capacity/limit refusal before partial commit | Device maxima are not an aggregate budget; explicit fallback is a new caller decision. |
| Equal complete immutable dependency set, stable compatible target/front | No-op reuse; no material admission/upload/write/encode/submission attributable to repetition | A frame tick or equal request is not a new material generation. |
| Valid changed candidate rendered, all writes enqueued and publication queued | Atomic target-local commit of its complete profile/material/geometry/model/view association | Request accepted, prepared, enqueued, published and actually displayed remain separate. |
| New candidate fails with compatible last-good front | Existing FailedWithLastGood with original complete material/source tuple and new failed request cause | Old output is old; it is not relabeled as using the failed material. |
| Inactive/zero/incompatible/retired target or terminal device | Existing no-render/withdrawal/retirement law | No synthetic 1×1 target, incompatible sampling, stale resurrection or private recovery. |

These are category meanings, not new public Rust error spellings. Known validation/compiler/resource errors must remain known; no catch-all success or unknown result may hide them. One view's failure cannot corrupt another view's material/front or unrelated 2D work. A caller may explicitly choose a separately valid baseline successor, but the engine must not present that successor as satisfaction of the rejected PBR request.

## 10. Resource, lifetime and no-use discipline

Texture and sampler leases remain independently owned immutable context resources, and material generations retain every role dependency they use. A material/structure replacement does not revoke an already leased requested, committed, compatible last-good or in-flight generation. Releasing the newest handle alone does not authorize overwriting old texels or rebinding old material identity. Lookup retirement and safe final release follow existing context ownership; no unbounded revision history or retired-material tombstone list is allowed.

Finite effective context policy SHALL account for live material slots/generations, factor/role metadata, CPU decoded content, resident textures and their actually admitted levels, samplers, per-material bindings, shader/IR/pipeline variants, upload bytes, staging and queue-safe in-flight ownership. Shared immutable storage is charged according to actual ownership; multi-role references may not hide duplicate materializations or count an unlimited graph as one small descriptor. All counts/products/offsets use checked arithmetic and live device limits. Total device VRAM and sibling context count remain outside a false context-global guarantee.

Admission/reservation and GPU preparation SHALL be transactional under HAL-355 §§7/10–13. A factor or binding generation change may perform the bounded necessary structural work; stable model/view-only changes must retain material leases, textures, samplers, binding state, shaders and pipelines after warm-up. No hidden dynamic material churn, shader-string regeneration or private clock supplies animated material meaning. The existing immutable Update/publication owner remains the only lawful producer of changed intent.

No selected material extension means no extension-specific registration, allocation, scan, upload, pipeline creation, pass, submission or cadence, including absent-role PBR default resources. No Context3D/view means HAL-355 structural zero. The ordinary SceneGraph, SceneNode, Batcher, DrawPlan, SceneCache, 2D spatial model and RenderGraph hot path receive no material fields/types/branches/dependencies. Baseline solid, vertex-color, textured-unlit, textured-mask and consumer WGSL appearances retain their current ABI, pipeline keys, factors, output and behavior without migration.

An active material view uses the exact HAL-266A SurfaceId/metrics and full physical render extent; material texture dimensions, clipping or OS density cannot become target authority. Inactivity performs no material-driven growth/write/render, while required lifecycle cleanup remains lawful. Removal unregisters publication and releases leases only after existing safe in-flight/publication ownership ends. The captured baseline device lifetime grants no device epoch/replay/recovery promise.

## 11. Invariant/prohibition pairs

**Adopted / Binding [S1] — 2026-11-03: qualifications/replacements for the named §11 pairs.** Unlisted pairs and all ABI, source interpretation, evaluator and lifetime rules are unchanged.

- **INV-R-024-BASIS / FORBID-R-024-BASIS:** Preserve all original mode-specific source certificates, local-first program order and dual-transform requirements. Validate affected source dependencies once and reuse exact still-live captures. GPU interpolation/frame consequences belong to §8.2.4's supported domain/envelope; they SHALL NOT require a production per-FOV vertex/triangle precision simulator.
- **INV-R-024-BOUNDS / FORBID-R-024-BOUNDS:** Charge all actual source validation, required host preparation, retained metadata, GPU capacity and old/in-flight generations. No mandatory frame-local certificate or relocated proof-only sweep may reinstate the removed GPU-model work. Actual R025 preparation and finite source-certificate verification are preserved.
- **INV-R-024-PROOF / FORBID-R-024-PROOF:** Distinguish rigorous original source/actual-host checks from representative predeclared observable GPU conformance and separate native/JC acceptance. An inconclusive modeled interval is not source-invalid evidence; actual source/host/capability failures remain typed. No universal GPU precision, hidden-intermediate measurement, guessed tolerance, per-frame S3 replay or unsupported backend acceptance is asserted.
- **INV-R-024-COMPLETE / FRONT / FRAME:** The foregoing changes only proof ownership/frequency. Exact immutable source meaning, current submitted matrices and target, same-Queue complete publication, original last-good, phases and owners remain mandatory.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

**INV-R-024-AUTHORITY.** Feature adoption, affected-parent admission, concrete engineering authorization, implementation and executed/native conformance are separate gates.  
**FORBID-R-024-AUTHORITY.** No self-adoption, draft inheritance, implicit HAL-355 override, R-025–028 activation or runtime permission from this document.

**INV-R-024-BASELINE.** The named opt-in PbrMetallicRoughnessRealtimeF32V1 family preserves every existing legacy appearance/ABI/output/key without migration or new mandatory features. Only its distinct PBR vertex/material/layout interfaces may use §8.4.
**FORBID-R-024-BASELINE.** No default shader/profile replacement, raw layout/WGSL escape or hidden downgrade of a rejected material.

**INV-R-024-COMPLETE.** One immutable qualified material binds named roles, resolved factors, classification/profile and every resource/geometry dependency; equal identity means equal content. Original and derived frame meanings, packed/certificate generations and complete normal-domain association remain exact.\
**FORBID-R-024-COMPLETE.** No unordered meaning-by-index texture array, partial patch, mutable model sharing, render-time semantic callback or mixed-generation front.

**INV-R-024-ROLES.** Only genuine optional absence uses exact defaults; required missing and present invalid/unsupported roles have explicit distinct refusal.  
**FORBID-R-024-ROLES.** No present invalid/stale resource replaced by an absent-role default, ignored provided resource, dropped required channel or success-by-omission. Only genuine optional absence uses the explicitly charged deterministic bindings in §8.4.

**INV-R-024-COLOR.** Color texture RGB is sRGB-decoded once before linear evaluation; factors/vertex colors/data roles and alpha retain their declared numeric meaning. PBR vertex RGBA uses covered perspective-centroid interpolation to preserve each source component in [0,1].  
**FORBID-R-024-COLOR.** No data-role sRGB decode, double color decode, guessed format, encoded-space filtering repair, material Image-premultiplication or center-extrapolated vertex color repaired by a shader clamp.

**INV-R-024-CHANNELS.** Roughness G, metallic B, AO R, signed normal RGB/+Y and emissive/base roles use the exact §5–7 interpretation. C changes source shading interpretation only, never original role/UV/normal-scale/channel meaning.\
**FORBID-R-024-CHANNELS.** No silent packing swizzle, normal-Y inversion, wrong factor domain or AO applied to unrelated direct light/emission.

**INV-R-024-BASIS.** Mapped triangles retain explicit certified A/B/C meaning, original/corner provenance and actual packed N/T/sign; genuine absence has its normal-only proof. Independent-weight ideal convexity, LOCAL GS and dual transforms remain exact source/program obligations. §8.2.4 separates rigorous source/host checks from bounded observable frame consequences for the declared input domain/backend envelope, without hidden frame telemetry or universal GPU bounds.
**FORBID-R-024-BASIS.** No fallback-as-presence, inferred fourth frame, world-interpolate-then-GS substitute, wrong transform, sign averaging, derivative basis, unadopted reflection, hidden importer or GPU-preparation frame/tangent generation. Explicit producer-selected certified C is a DECLARED source interpretation, never falsely unchanged original smooth/Mikk shading. A genuinely absent normal role must not consume dummy tangent/normal-map data.

**INV-R-024-COORDINATES.** PBR receiver position uses covered perspective-centroid interpolation; UV0 stays perspective-center with each role's explicit supported coordinate/sampler association. Unsupported transforms, sets or sampling profiles refuse.  
**FORBID-R-024-COORDINATES.** No center-extrapolated receiver position, assumed common point across varyings, ignored UV1/transform, hidden atlas, silent wrapping substitution or mip promise from one-level storage.

**INV-R-024-RESPONSE.** Source channels/factors remain unchanged; this named real-time profile explicitly evaluates r_eff=max(r,.045) with the regular GGX/Schlick/metallic partition and exact cancelled separable-Smith visibility. Zero/undefined direction or noncontributing light branches return zero reflected contribution, independently of emission; GPU acceptance requires §8.2.4 predeclared observable comparisons for the versioned domain/envelope; missing discrimination or an essential unselected bound remains OPEN, and source algebra is not universal GPU-finiteness proof.
**FORBID-R-024-RESPONSE.** No hidden or additional floor/epsilon, NaN-as-success, model substitution, silent rejection of legal source r=0, full optical/glTF accuracy claim, invented fragment CPU rejection or final LDR clamp used as numerical proof. The declared .045 approximation is not a hidden repair.

**INV-R-024-LIGHT.** Incident direct/environment transport requires its separate owner; emission is surface radiance and AO only qualifies indirect response.  
**FORBID-R-024-LIGHT.** No implicit/global light, emissive-to-GI authority, decorative-shadow substitution or renderer semantic scene owner.

**INV-R-024-ALPHA.** Material scope is Opaque/Mask, surviving alpha one; final Image is already premultiplied linear under existing coverage/resolve law.  
**FORBID-R-024-ALPHA.** No fractional object transparency, hidden alpha-to-coverage, Image repair or double alpha application.

**INV-R-024-LEASES.** Independent material/texture/sampler generations are context/kind-qualified and retained through all real requested/committed/last-good/in-flight use.  
**FORBID-R-024-LEASES.** No raw GPU handle, stale slot rebinding, generation wrap, global registry or overwrite of leased content.

**INV-R-024-BOUNDS.** Actual metadata/resources/bindings/pipelines/upload/staging/in-flight work are finitely admitted and transactionally charged. Include finite source/certificate verification/coverage metadata, source corner expansion and all retained generations; no runtime certificate search.\
**FORBID-R-024-BOUNDS.** No unlimited history/cache, device-limits-as-budget or model/view-only material churn.

**INV-R-024-FRAME.** The existing Update publishes complete values; HAL-266A preparation consumes them mechanically; Paint emits only ordinary Image.  
**FORBID-R-024-FRAME.** No second phase/clock/scheduler, Timeline sampling in preparation, late semantic writer or peer widget/tree reference.

**INV-R-024-FRONT.** Requested and committed complete material provenance remains target-local; enqueue precedes publication and compatible last-good remains old truth. A changed mode/certificate never relabels an old front; incompatible full target keys withdraw it.\
**FORBID-R-024-FRONT.** No partial material commit, front-as-back, failed request relabel, target mismatch or submission-as-display assertion.

**INV-R-024-ABSENCE.** Unselected extension and ordinary 2D/baseline paths retain zero extension work and unchanged owners/topology.  
**FORBID-R-024-ABSENCE.** No 3D material branch/resource/dependency in ordinary 2D rendering or mandatory runtime service.

**INV-R-024-PROOF.** Record exact executed source/camera/target and compiler/Device/backend/driver evidence. Separately declare the supported versioned input domain/backend envelope, required per-candidate checks and capability-family/regression compatibility. Valid in-domain source/view/target changes need no fresh S3 per frame. Source/host proof, actual available observable comparisons and JC/native acceptance remain distinct; no hidden-intermediate measurement or universal GPU precision claim follows.
**FORBID-R-024-PROOF.** No document, coefficient, compiled shader, synthetic headless output or screenshot golden substituted for native material/quality proof.

## 12. Exact parent/ABI compatibility and future admission matrix

Every required item below is a binding coordinated S1 obligation under HAL-370 §8/HAL-355 §17.1, not a current-source capability. The named profile, numerical policy, local-frame-first order, tagged material identity and exact §8.4 interfaces are adopted. S2 authorization, concrete source/API integration, finite effective capacities and executed f32/Naga/GPU proof remain separate; these settled architectural choices are not delegated back to an executor.

All remaining numerical-proof references, including §14, use §8.2.4 rigorous source/host checks and predeclared available observable comparisons for a versioned supported input domain/backend capability envelope. Exact evidence provenance is not the per-frame runtime key; valid in-domain changes require ordinary guards, not a complete replay of the original three-asset external-consumer conformance fixture. No hidden arithmetic telemetry, unselected TBN/normal/GS tolerance or universal vendor-error predicate is implied.

| Existing controlling clause | Exact current constraint | Required coordinated future material admission / preserved boundary |
|---|---|---|
| HAL-355 §8.1; §§6.1/8.2 | Legacy36-byte stream; per-vertex normalization; positive TRS; no tangent | The S1 contract selects the opt-in52-byte PBR stream/tangent offset36, explicit A/B/C mapped source-frame or genuine-absence normal proof, complete UV/sign and correlated normal-domain certificate, with old cones/BLUE sufficient, local interpolation/GS and <=64 scale domain. Legacy stream unchanged; producer MikkTSpace output and f32 proof remain qualified obligations. |
| HAL-355 §8.3; §§3.5–3.7/8.4 | Flat geometry/AppearanceHandle/raster/ordinal packets with structural leases, complete model and tuple | The S1 contract selects one tagged LegacyAppearance OR typed PbrMaterial(MaterialHandle) association, immutable role/factor generations and structural successors. Preserve front provenance; no silent G1/domain widening. |
| HAL-355 §9.1 | Legacy groups0/1/2,80/112/16-byte blocks, one texture/sampler; ObjectBlock vertex visibility | S1 DISTINCT PBR layout: View80; same Object112 bytes but VERTEX\|FRAGMENT visibility in that family ONLY; group2 Material64 atb0, five role texturesb1–5/five samplersb6–10; group3unbound. Legacy layouts/stages remain exact. |
| HAL-355 §9.1.1 | Persistent structural bindings and target-isolated queue-safe dynamic storage | PBR factors/role/default resources are structural immutable generations; reuse through model/view changes; charge finite actual metadata/bindings/residency/in-flight work and obey Device offset alignment. |
| HAL-355 §9.2 | Legacy sole vertex entry and four perspective outputs including world-normal location1 | S1 separate PBR vertex family: location1 LOCAL normal, added location4 LOCAL tangent/sign, fragment-local GS then ObjectBlock dual transforms. No reinterpretation of legacy world-normal output, application vertex stage or fabricated normal presence. |
| HAL-355 §9.3 | Legacy one-texture appearance function, finite/clamp[0,1], alpha1/Mask | Admit fixed controlled PBR surface evaluator and declared .045 effective-roughness profile/guards, not a new arbitrary WGSL entry. Material BRDF is not incident-light/final-color output. R-027 remains necessary before indoor quality needing radiance>1; baseline wrapper unchanged. |
| HAL-355 §9.4 | Caller library has no globals/entry/overrides; controlled Naga/IR wrapper/limits | PBR evaluator/interface is Halcyon-owned fixed IR with full validation and finite compiler resources, no public application WGSL globals/layout escape. Concrete Naga/source proof OPEN. |
| HAL-355 §9.5 | Complete legacy pipeline key | Distinct family/ABI/profile/fixed-shader/cull/color/depth/sample key; material identity is not a variant key. All old keys/pipelines remain valid without migration. |
| HAL-355 §§10–11 | Surface-compatible LDR front/back/depth/sample 1/4, current target activity, enqueue/swap/commit, last-good/withdrawal | Preserve those laws for material evaluation; qualify complete material dependencies and failures. Multi-pass lights/shadows/HDR are not admitted here. No changed target source, front owner or publication stage. |
| HAL-355 §13; §§7/14 | Finite typed context resources/work and baseline Device features/limits | Explicitly charge material roles/defaults and generations; actual Device limits for64UBO/5textures/5samplers/11group2bindings/52stride/interstage/stage visibility. Selected ideal-domain proof is not executed f32/backend proof or a numeric capacity. |
| HAL-355 §15 | No 2D contamination/raw GPU/arbitrary shader/partial state/dynamic churn/false publication/private scheduler | All prohibitions continue literally; the new material profile grants no generic escape or semantic GPU authority. |
| HAL-355 §17 | PBR, lighting, multi-texture systems, mip generation and HDR excluded from baseline | Expressly admit R-024's narrow typed material feature with its affected clauses; do not promote excluded sibling systems or call current baseline PBR. |
| HAL-263A §§2–5, INV-002/004–008 | Final sampled Image premultiplied linear, closed ingress, exact tint/coverage equation | No material decode amendment is implied: this owner controls only final Image. Prove unchanged final seam. Any actual new Image ingress/association needs its own amendment; HDR belongs first to R-027/HAL-355. |
| HAL-295A §§3.2–3.4, INV-003–010 | Existing explicit offscreen admission/privacy, same Queue, per-front order, safe retirement and no-participant absence | Material resources remain private to admitted 3D owner; use existing publication. No new renderer/queue/graph authority, public raw GPU resource or device recovery. |
| HAL-266A §§3–4, INV-001/003/005/006/007/009/010/014 | Final current immutable metrics; indexed post-layout/pre-paint cache/lifecycle work only; equal-input reuse | Consume complete qualified material values before preparation; no source/material derivation callback, Timeline access, wrong-target or inactive render. Native root/local activity remains existing law, not a new navigation-eligibility promise. |
| HAL-357 §§1–2/4.2/6–8; HAL-354 §4.1 | Optional lowering to exact flat pair; one timed Update and owner G1 cutoff | Any actual input/metadata extension must be expressly coordinated at its true producer owner. No synthetic domain, semantic relation, peer borrow or renderer hierarchy; unchanged flat consumers remain legal. |
| HAL-002/002A and HAL-309 | Permanent owner/phase/identity laws; honest status/evidence | No constitutional amendment is proposed or proven necessary. Drafting does not confer source conformance, engineering permission or backend support. |

The architecture in §§7/8.2/8.4 is S1 adopted, including the 2026-11-01 source-only Revision02: original UNIT/.75 mode A; original smooth/Mikk mode B's nine packed cross bounds; explicitly derived and disclosed geometric mode C; genuine-absence normal proof; complete common-UV normal certificate; unchanged local GS/dual transforms, roughness policy, typed identity and ABI. Old private S2 and the internal unlinked writer are accepted only over their old tested domain. Expanded producer/packed-f32/Naga/Device/Metal/target proof remains OPEN; public S3 and native S4 remain CLOSED. This record supplies no implementation authorization.

## 13. Future proof oracles — none executed by this documentary adoption

**Adopted / Binding [S1] — 2026-11-03: qualification of §§12–13 and the D3/D4/backend oracles.** The source/frame/normal/radiance laws and independently predeclared observable N1/N4 comparisons remain required. The adopted current-host boundary is the 2026-11-03 reform in §8.2.4; no oracle wording requires a production full-vertex/full-triangle prediction sweep or per-frame conformance replay.

A separately granted complete S2 unit SHALL satisfy the coordinated future engineering/proof contract in HAL370 §9's adopted 2026-11-03 reform: original source and actual-host typed refusals, zero redundant warm source scans and proof-only sweeps, unchanged controlled shader/quality, actual R025/R027 and ownership/lifetime outputs, and the unchanged original three-asset external-consumer conformance fixture at N1 and N4. Review old tests that assert proof-only refusal/work against the accepted contract before changing them; no silent expectation repair or resumption of a held unit follows from S1 adoption alone.

Dated records in §§14–17 retain their exact scopes; the current adoption/engineering gates remain controlling. No historical receipt reinstates the per-FOV GPU-model obligation removed by this adopted reform. No tests, benchmark, implementation or measured improvement are supplied by this documentary S1 adoption.

**Existing operative source text below remains binding except to the exact extent superseded/qualified by the adopted 2026-11-03 boundary above:**

| Gate / intended supported path | Falsifiable oracle | Nearest existing proof owner and limit |
|---|---|---|
| D0 — authority / typed ABI / profile | Exact accepted profile/parent/engineering scope; host/Naga offset0/16/32/48/size64, group2b0–10 roles/flags/reserved-zero, vertex52/offset36, LOCAL varyings and PBR-only Object112 VERTEX\|FRAGMENT layout. Pinned Naga25.0.1/wgpu25.0.2 and real Device compile AND execute N1/N4 PBR with VS/FS qualifiers matching exactly: locations0/1/3/4 perspective-centroid,2 perspective-center. Legacy36/ObjectVERTEX/all-center/wrapper/key unchanged. Certificates and C's source-mode metadata add NO GPU attributes/groups/branches; C stays PBR52 and preserves D-only12/flat5. Missing normal bypasses dummy TBN; actual Device limits and finite structural default costs verified | Governing owners/JC and `three_d_context.rs`, `three_d_appearance.rs`, `three_d_raster.rs`, `three_d_backend.rs`. This document's byte arithmetic/qualifiers are not executable shader/API proof. |
| D1 — role defaults and completeness | Each absent optional role yields §5's exact value; required missing fails; present invalid/unsupported remains distinct even when factor is zero. Defaults include metallic/roughness 1 and emissive zero, not guessed dielectric/black texture | `src/tests/three_d_context.rs`, `three_d_snapshot.rs`; CPU descriptor/admission proof only. |
| D2 — color/data sampling | Known sRGB texel 0.5 gives about 0.214041 linear RGB, while numeric 0.5 remains 0.5; alpha unchanged. Separate channel probes isolate MR G/B, AO R, normal RGB and ignored channels; factors/vertex color are linear and applied once. The independent white/black vertex witness must yield bounded centroid RGBA/alpha on actually covered N4 fragments; the rejected ideal center variant yields RGB-4 and metallic BRDF-1/pi. No runtime color clamp substitutes for convexity | `three_d_appearance.rs`, `three_d_raster.rs`, `three_d_backend.rs`; compare actual upload/view/GPU samples with deterministic role oracles and declared quantization tolerance. Observe coverage/qualifiers; no Image normalization or assumed abstract S sample. |
| D3 — source certificate / producer / dual transforms | Exact original normals, signed Mikk corners and index/seam/UV provenance; qualified A/B/C or genuine-absence mode and actual packed certificates under §7.1. Reproduce A's old cone/3/32 and packing slack, B's ALL9 bounds with independent N/T weights, C's original local face/det/U/H, positive bitangent·dPdv and identical packed corners, and the normal-only d proof. Cover source-proven singularities on sofa triangles[23,25,214,704,706,895] without a hard-coded allowlist; retain original data and disclose derived shading. Preserve the world-GS zero counterexample and predeclare a discriminating real N4 partial/center-outside production output case for the selected source frame/receiver behavior. Observe actual FP16/Image, genuine existing probe outputs or coverage/Device results; do not claim hidden N/T/g/receiver intermediates directly measured. Missing discrimination is UNVERIFIED and any essential additional comparator awaits selection by the delegated architectural reviewer before execution. | Existing three_d_geometry/context/appearance/raster/viewport/backend owners; the external conformance fixture's `src/tests/pbr_source.rs` for actual producer/source data. Pin Device/backend/count and establish actual coverage; no shared-centroid, theoretical Metal sample or completed C-buffer claim. S3 evidence supports the declared input domain/backend envelope under §8.2.4; exact fixture provenance is not a per-frame runtime admission key, and measured outputs are not universal interpolation-error bounds. |
| D4 — numerical/source versus actual evaluator | Preserve source r0 and r_eff=.045, independent GGX/Schlick/cancelled-Smith/nonnegative-source/metal/F0/zero/H/scale predictions and rigorous A/B/C packed, BLUE128 and common-UV1/510 source certificates. Predeclare representative N1/N4 partial/clipped/center-UV/helper-sensitive, six C face/five correlated case, sign/scale/sampler/double-sided/emission output discriminators. Compare actual final FP16/Image, genuine available source-stage outputs or observable coverage/Device results using authorized criteria; no hidden frame/GS/normalization measurement or replacement shader. | Existing three_d_appearance/raster/backend tests. Missing discriminating output is UNVERIFIED; if another numeric comparator is essential, its exact bound requires selection by the delegated architectural reviewer under separate S2 authorization before execution. R028 .03/.05 metrics are not TBN/normal/GS tolerances. §8.2.4 qualifies a declared domain/envelope with per-candidate guards, not each recorded frame or universal GPU precision. |
| D5 — AO/emission/mask | AO changes only the independently supplied indirect term; no direct/emissive attenuation or shadow claim. Emission is visible surface contribution without invented scene light. On actual N1/N4 geometry, including observed center-outside N4 coverage, centroid vertex alpha and center-UV sampled alpha preserve the unchanged Mask threshold/discard and surviving alpha1; Opaque ignores mask alpha and double-sided orientation is exact. Observe real typed material/geometry through the controlled PBR shader, accepted private R-027 scene/tone/FP16 final and ordinary Image composition | `three_d_raster.rs`, `three_d_viewport.rs`, `three_d_backend.rs`, with separately admitted illumination where required. R-027's accepted private S2 owner is reused; no substitute test-only material shader, changed tone/coverage path or public S3/S4 claim. |
| D6 — identity, refusal, budget and retirement | Original/render-mode/certificate/texture/sampler/material successors preserve leased old associations; wrong/stale/required-missing/unsupported/budget failures never partially change request/front. Include finite witness/coverage/work, metadata and old/current/in-flight costs. Missing/unresolved cells or stale interpretations refuse; failed raw hull alone does not. Saturation is finite and typed; retirement/in-flight retention stabilizes without history | `three_d_context.rs`, `three_d_snapshot.rs`, `three_d_viewport.rs`; distinguish CPU request acceptance from GPU publication. |
| D7 — target and front | Material successor changes committed source only after real writes/enqueue/swap; compatible last-good stays old on failure; zero/inactive/local collapse/resize/sample/format/unmount/device lifetime withdraw or retire correctly, then truthful restoration | `three_d_viewport.rs`, `custom_render_preparation.rs`, external canonical native fixture. No old front relabel or render-target metrics from prior frame. |
| D8 — baseline/no-use | Existing unlit/procedural/textured/mask consumers retain baseline ABI/output. No selected extension gives zero material work; no 3D context/view gives structural zero; warmed view/model changes keep material resources/bindings/pipelines stable | Existing 3D/retained source surfaces; no added instrumentation in ordinary 2D hot paths. Actual release/native comparisons belong to a later admitted unit. |
| D9 — native material provenance and output | Real canonical App::run room with actual glTF-origin furniture plus separately source-qualified consumer-owned UV/textured floor/walls; exact original-versus-derived basis disclosure/asset/generator/role/lease and material/profile/model/view/target evidence through actual GPU/native stages. Indoor quality needing radiance beyond [0,1] additionally requires the admitted implemented R-027 chain | `tests/app_frame_realization_native.rs` under separate authorization; actual adapter/backend/format/samples/frame and JC visual review. No supplied importer/UV data is inferred from engine support; AABB bounds and LDR experiments do not prove contact/shadows/target quality. |

**Attributable source feasibility, not new implementation proof.** The supplied external-consumer source revision `d4408a1`, `src/tests/pbr_source.rs` SHA-256 `0609229f57e33db247e5b69d692dc1b5dc94d5594a0aa650b815cd403a3bc327`, reports20,284 triangles/60,852 signed default Mikk0.3.0 corners:20,278 pass the nine-pair candidate and six sofa triangles listed in D3 have existential independently convex nonzero parallel/antiparallel original-frame witnesses, including normalized-then-packed variants. Tiny rounded cross residuals alone are not exact-zero proof; the reported source-bound analysis is the evidence. Their weight locations need not coincide and no simultaneous actual Metal occurrence is asserted. Mode C is an adopted source route for such a proven situation, not an emitted or GPU-qualified furniture buffer. Original JPEG/mesh/Mikk bytes remain unchanged; future source-producer/inspector and Halcyon tests must expose original versus derived meaning and requalify R-025 response, R-026 dependent G and R-027 complete radiance.

Tests must identify the exact executed stage: descriptor validation, source admission, shader composition, GPU resource creation, sampling/evaluation, request acceptance, target preparation, enqueue, publication, Image composition, native presentation and observed result. Compilation or a headless image does not establish display/quality. Metal/Vulkan/DX12 rows and any later browser claim remain separately qualified under HAL-355 §14; unrun rows remain unverified.

R-025 lighting/shadows, R-026 environment/GI, R-027 HDR and R-028 mip quality supply only their separately accepted proof obligations when actually needed. R-024 cannot claim those gates from material data or use lack of a device as evidence of conformance. No pixel/style snapshot goldens or photo-realism acceptance threshold is introduced here.

## 14. Documentary verification, adoption gate and deferred scope

Use the actual sole Grip proxy surface in the admitted workspace:

```text
ws halcyon
find SPEC-R-024* under docs/specs
read docs/specs/SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md max-lines 1000
read docs/specs/SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md hash
read docs/specs/SPEC-R-023-3D-Visual-Rendering-Extension-Architecture.md hash
git status
```

Complete every emitted continuation and inspect ALL changed authoritative spans. This 2026-11-01 unit owns exactly R-024, R-028 and HAL-370; read their hashes, exact three-file diff/whitespace result, current Git state and required health summary. The original eight-document adoption and historical creation/correction receipts retain their original scopes. No Rust/GPU/producer/asset test is performed; health diagnostics do not prove expanded inputs. The immutable reviewed proposal stays historical and unchanged.

Before code, a separate bounded S2 source/CPU/host/GPU proof-producing unit requires explicit authorization under HAL-370 §7. S1 profile/parent adoption is performed, not proof of interface/capacity integration, real producer MikkTSpace output, f32/Naga/GPU/target support or S3 public capability readiness. The .045/source-zero treatment and ideal dual-space solution remain fixed. Indoor quality beyond[0,1] still requires the admitted R-025/R-026 illumination and R-027 output chain to be implemented and proved. No current setter/texture field/Naga parse bridges these gates.

Outside this material-only contract and not engineering tasks authorized here: glTF/GLB or image loaders; UV1/transforms/atlases/extra streams beyond a separately admitted tangent basis; compressed/HDR/mipped textures; light/shadow/environment/GI systems; HDR/tone mapping; general translucent/refraction/transmission/glass; extended PBR lobes; reflected models, skeletal/morph/deformation systems; device recovery; renderer hierarchy, GPU semantic/physical authority or new UI/GPU infrastructure. R-025–028's exact lighting/E/G/HDR/V2-sampling contracts are separately S1 adopted under HAL-370; V1's evaluator and one-level normal SAMPLING semantics remain unchanged; source eligibility is the coordinated Revision02 law. Prior separately authorized private S2 and the internal unlinked writer remain accepted only over their tested old domain; no expanded-source or sibling engineering grant follows here. Unlisted features remain excluded, not executor backlog tasks.

Current disposition: **Adopted / Binding [S1], including coordinated source-domain Revision02, 2026-11-01; old-domain private S2/internal unlinked writer ACCEPTED; expanded source/packed-f32/Naga/Device/Metal NOT ACCEPTED / engineering requires a separate scope; S3/S4 CLOSED.** No old proof is broadened, historical receipt rewritten, public capability exposed or implementation authorized by this documentary integration. All independent parent/lifetime/G1/native/release boundaries remain.

## 15. Historical authoring source bindings and read coverage

Full-file SHA-256 values were rebound on 2026-10-31 against clean HEAD `312e25c7a6a6d923ff4db26a311cff32fdb717a4` before this sole new document. All cited code lines refer to these bytes. R-023's historical receipt baseline remains its original record and is not rewritten to the current HEAD.

| Entry / authority path | SHA-256 |
|---|---|
| `AGENTS.md` | `66d3a0842f60bf12b34382b20b877a19dfa778e0a2dac972d3c126e3ad6f13a5` |
| `PRE-FLIGHT.md` | `12f1bfecccbac7170a88aaa2d55a5c176686d251b40d25bdeb97258b86087636` |
| `docs/ENGINEERING.md` | `24e9edcccf85d2b0db2f4dac905c943fc234fd217df691e981814c391906b624` |
| `PROJECT-ENTRY.md` | `d844a6774e918b103087facb3a03749f27380dcc1f89489bc99d839d4a3910a1` |
| `docs/specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md` | `8b1c3f3217ecfffb7b06502dce17e70984887f403afba59d023bacfed56f46db` |
| `docs/specs/SPEC-HAL-002A-Constitution-Amendment-Guidelines.md` | `f6ab1927a311c5945bc224860d642834d4edde5c3371ead6a0576d8ffb05cc56` |
| `docs/specs/SPEC-HAL-309-Documentation-System.md` | `883eabdce15a3b6d145a2de788bcb223a0a86e6c784f7ff8497c93d4ac599fc6` |
| `docs/specs/SPEC-HAL-355-Native-3D-Rendering-Substrate.md` | `edc1bcb236f9ce104d79841ba2649767e0067aa0d1bc6a979b2c3cabbe6ed585` |
| `docs/specs/SPEC-HAL-357-Shared-3D-Spatial-Domain-Composition.md` | `2288f2c325c6f4a145a694420ea6d583afdce91ad8184a45787ee9a3bccd451c` |
| `docs/specs/SPEC-HAL-263A-Image-Alpha-Contract-Correction.md` | `e30603ba9f3e83d48fc5b288dd8a82a37acf721df79ff15dbde18db647ca1456` |
| `docs/specs/SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md` | `d9cf0098ad19a2338acb562187ecb079cc0051f3aa2c83c5894ccc7f6ede6232` |
| `docs/specs/SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md` | `fe115aa0e61225ac8e5e2f3162604f4c36b3072fc7640103c049f4646a357ec4` |
| `docs/specs/SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md` | `6d3e0f2041d2af1aab92a0216c37afff481cb6b0012f0469b7ed25b36c288c82` |
| `docs/specs/SPEC-R-023-3D-Visual-Rendering-Extension-Architecture.md` | `00ca077084293f59c793f86560709dd57a8d9d558535fbe2193cf36b2792a6f5` |

| Actual implementation/reference path | SHA-256 |
|---|---|
| `src/three_d/content.rs` | `eb1f4ef4252a1b1cbb6a97262508f7b4f2e816125e965cab27fb08f3053bda59` |
| `src/three_d/context.rs` | `9cc31da306c5a1beb2ffb599f8df57fd6aa4999cadaf47bc9bacc950647593ef` |
| `src/three_d/appearance.rs` | `243c418472dffa019b3004ec33a3a2ae4c63743a6a42f35c482cd427a07bfc2f` |
| `src/three_d/raster.rs` | `192b891d7d1d399379f6dd1b28cd5a8c58bedac7b121c741499478407bccb25b` |
| `src/three_d/snapshot.rs` | `1da0202cab6dd2881eb04c36d43fcdfa2df2a900fb9e0eae0cb3e764a4dc0189` |
| `src/three_d/viewport.rs` | `39621fff482c963d9555654b2ecee9da1fec6b240de8a56ae036ebb83fd5aa6a` |
| `src/core/app.rs` | `3d6a9947001f148e9c3f2a532e50380302e94b84be7b693b69c4462edd385039` |
| `src/core/widget_tree.rs` | `5f4d80dea5cf9842fa7e1967e232f628a9b8710a8f1d43bbbcd6a6048499a710` |
| `src/renderer/custom_render_prepare.rs` | `b8d9128e06758ceb66e64ac9eac66472eae833865b14412c4d83ee85f2b04888` |
| `src/renderer/custom_render_pass.rs` | `93520bad97dd74565a84828bf0087fc578e5323d1d92fb0ac6b20ac47fcb9503` |
| `examples/showcase/tab_viewport_3d.rs` | `1587b6fe7153e2709f3005123cb066efcfd79dab20911d8763b92ef9f21b499e` |
| `tests/app_frame_realization_native.rs` | `5737eaf55cd499bed8def0aa544c44977edfe6dc7f0fea97d90e8bc3d8149a4f` |
| `Cargo.toml` | `59290cea9b2d1a7750458a586509a4c98786d935faa1170ea1e756c9b62bef0f` |

Fresh full read coverage: AGENTS 1–1504, PRE-FLIGHT 1–207, ENGINEERING 1–284, PROJECT-ENTRY 1–758, HAL-002 1–637, HAL-002A 1–225, HAL-309 1–472, HAL-355 1–2043 and R-023 1–397. Affected parents read: HAL-357 1–174/507–548/684–825/1029–1076; HAL-263A 1–262/381–408; HAL-266A 1–172/195–275/525–744; HAL-295A 1–142/276–470/593–624; HAL-354 1–43/333–448. Actual source ranges and public symbol/member inspection qualified the production facts in §3; no new API was inferred from a title or text search. Existing 3D/internal/retained/native test paths were found, not executed or claimed to cover new material behavior.

## 16. Historical adversarial correction receipt and then-unresolved admission — 2026-10-31

This section preserves the earlier correction receipt and its then-OPEN decision table. It is historical, not a second current contract: the current profile selections/ideal proof and remaining proof-only versus parent/consumer gates are §§7/8.2/8.4/17 and corrected R-023 §13.1.

HAL-R-023-R-024-ADVERSARIAL-CORRECTION-01 corrects only the two inactive drafts under the delegated architectural reviewer's exact A–F decisions. JC retains final ownership/veto. No new architecture, coefficient, Rust interface or authority is selected by the executor.

Beforeimages were requalified at Halcyon 0.227.18-beta, master HEAD `312e25c7a6a6d923ff4db26a311cff32fdb717a4`: R-023 SHA `00ca077084293f59c793f86560709dd57a8d9d558535fbe2193cf36b2792a6f5`, 397 lines/58922 bytes; R-024 SHA `23a7f11e4c3dfd750d67406c6df2283a7f1eed79a93797c2602466b2966bb4f2`, 424 lines/68540 bytes. Initially tracked/index state was clean and R-024 was already untracked. Original §15 hashes remain the original receipt; the R-023 value is its original reviewed beforeimage, not a claim that its corrected bytes are unchanged.

The current correction pre-flight and protected-source/read coverage are recorded completely in R-023 §17. For this artifact the exact deltas are §7's mandatory source-producer MikkTSpace path and OPEN conformance gate; §8.2's two OPEN mathematical implementation blocks; §8.3's pre-quality HDR/output dependency; corresponding BASIS/RESPONSE pairs, parent matrix, D3/D4/D9 and adoption gate. Channel roles/defaults, color decoding, regular GGX/Smith/Schlick equations, ownership, leases, alpha and baseline compatibility remain intact.

| Outstanding admission | Exact owner and evidence needed | Current state |
|---|---|---|
| Parent version / typed factors, bindings, tangent ABI / finite budgets | Halcyon governing 3D/core owners and JC; exact coordinated HAL-355 §§8.1/8.3/9.1–9.5/13/17 and concrete compiler/resource interface | BLOCK-R024-PARENT-ABI, OPEN; no current field is an inferred extension. |
| Required producer route | Actual source producer with Halcyon architect/JC admission; exact source-qualified default MikkTSpace conformance, seam/sign/correspondence, bounded generation/validation failure and finished-data ABI | BLOCK-R024-TANGENT-PRODUCER, OPEN (§7). |
| Global validity / guarded finite evaluator | Halcyon 3D architect/JC; sufficient supported interpolation/filter/basis domain, mathematical rules and implementable rejection cut with proof | BLOCK-R024-FINITE-EVALUATOR, OPEN (§8.2); vertex-only checks do not close it. |
| Legal r=0 and near-singular domain | Halcyon 3D architect/JC with illumination/integration owner; exact end-to-end supported-domain policy and proof before code/R-025 coherent response | BLOCK-R024-ZERO-ROUGHNESS, OPEN (§8.2); not executor-selected regularization or exclusion. |
| Complete material/light/model/view source transfer | Actual HAL-355/354/266A and affected HAL-357 owners, prospective R-025/026; exact immutable input/identity/admission/settlement | BLOCK-R023-MATERIAL-LIGHT-TRANSFER, OPEN; current flat G1 is not silently widened. |
| Required indoor color/output chain | R-027 plus HAL-355/263A/266A/295A owners; correct admitted implemented radiance-to-final-Image chain before quality needing >1 | BLOCK-R023-QUALITY-OUTPUT, OPEN; limited LDR evidence is a proper subset. |
| Consumer surfaces and actual feature/native acceptance | Source/asset owner supplies real furniture and UV/textured floor/walls; authorized implementation owners prove D0–D9/C0–C7; JC observes native quality | BLOCK-R023-CONSUMER-SURFACES / BLOCK-R023-NATIVE-ACCEPTANCE, OPEN. No AABB/contact/shadow/visual equivalence. |

The glTF geometry recommendation and legal roughness domain were rechecked in the primary Khronos 2.0.1 reference identified in §2. Supervisor-supplied external-consumer asset/surface facts are labeled as such; no consumer asset download, source mutation or new consumer audit occurred. No build/test/native evidence is produced by this documentary correction. Both drafts remain Draft / Proposed / INACTIVE / NOT BINDING, neither implementation-ready; stop at delivery for independent review, with no R-025 or runtime successor.

## 17. Historical architect decision lock and dual-space correction receipt — 2026-10-31

HAL-R024-ARCH-DECISIONS-LOCK-01 D1–D10 are incorporated with D3/D5/D7 replaced ONLY by HAL-R024-D3-D5-DUAL-SPACE-CERTIFICATE-CORRECTION-02's local-frame-first premise and PBR-only fragment interface. JC retains final veto. These are architect decisions, not executor-selected policies. Both drafts remain Draft / Proposed / INACTIVE / NOT BINDING; no parent change or engineering permission results.

Exact beforeimages at Halcyon0.227.18-beta/master312e25c7a6a6d923ff4db26a311cff32fdb717a4: R-023 `32fca8ebf6aaed25f220244fba54c0c6854374a1b6249d20e123f597efde08e5` (448lines/71270bytes), R-024 `d94296f49eb4fcb220d54bddcba2edbd1c242d0fa73f10083c068b5c35902904` (460lines/79186bytes). The prior pre-edit STOP changed neither. Only these two document paths are owned now; all preceding author/source hashes remain historical receipts.

| Decision | Selected proposed contract / still-open proof |
|---|---|
| D1/D2 | Named opt-in realtimeF32V1; unchanged source r including0, explicit .045 r_eff and exact separable-Smith cancellation (§8.2.1). Actual shader/backend accuracy OPEN. |
| D3/D5 corrected | All-PBR normal cone; mapped tangent/UV/sign domain; LOCAL GS >=3/32 before dual transforms; exact counterexample solution and deterministic contributing-direction guards (§§7/8.2). Actual f32 margins/IR/backend proof OPEN, not another missing ideal-math preference. |
| D4 | Single-level numeric normal map, every blue byte>=128, scale[-1024,1024]; convex-z proof (§8.2.3). Actual upload/sampling/error margins OPEN. |
| D6/D7 corrected |64-byte group2 material block and5+5named bindings;52-byte opt-in vertex stream, LOCAL normal/tangent varyings; same Object112 bytes but new PBR-only VERTEX\|FRAGMENT layout (§8.4). Parent/source/Naga/Device proof OPEN; legacy layout untouched. |
| D8 | Structural LegacyAppearance OR typed immutable PbrMaterial handle, factor/material successors and stable leases/pipelines; fixed internal evaluator, no public arbitrary WGSL; group3unbound (§§4.2/8.4). Concrete integration/finite capacities and front proof OPEN. |
| D9 | Material BRDF only; complete material/light source transfer, R-025/R-026 and necessary R-027/native acceptance remain separate OPEN gates. No invented light/final color. |
| D10 | Producer-side default MikkTSpace path SELECTED; actual generator revision/conformance and real sofa/armchair output proof OPEN, not architecture delegated to executor. |

Current required16-field pre-flight and seven Halcyon fields are recorded in corrected R-023 §18 for this single paired unit. Fresh source seams include `raster.rs:24–50,184–218,387–456` (Object VERTEX-only and legacy pipeline), `appearance.rs:1811–1902,2030–2043,2094–2099` (exact blocks/world-normal baseline) and the unchanged source/authority hashes listed in §15, freshly rebound for protection. No Rust source/test/example/asset/consumer/parent edit, build, Cargo test or native launch occurred.

External references are primary-source terminology/math/layout evidence ONLY: [Khronos glTF2.0.1 geometry/material/Appendix B source at8e798b0](https://raw.githubusercontent.com/KhronosGroup/glTF/8e798b02d254cea97659a333cfcb20875b62bdd4/specification/2.0/Specification.adoc); [Filament roughness numerical precedent](https://google.github.io/filament/main/filament.html); [WGSL alignment/uniform rules](https://www.w3.org/TR/WGSL/#alignment-and-size); [pinned wgpu25.0.2 limits](https://raw.githubusercontent.com/gfx-rs/wgpu/v25.0.2/wgpu-types/src/lib.rs). Registry rereads timed out; the same exact-commit Khronos source supplies the verified reference. No external reference adopts a Halcyon feature.

Current disposition: selected proposed architecture and exact ideal proof complete for independent review; producer/code/parent/backend/native conformance NOT established. No self-acceptance or R-025 successor.

## 18. Proposed S1 — two source-qualified geometric rendering classes

**Proposed / Inactive / Non-binding.** On 2026-11-07 the delegated architectural reviewer
selected the zero-callback class
for drafting under
`HAL-R024-CANONICAL-MIKK-ZERO-DECLARED-GEOMETRIC-SOURCE-FRAME-S1-DRAFT-01`.
`HAL-R024-DUAL-GEOMETRIC-SOURCE-CLASSES-S1-DRAFT-02` adds the separately
declared nonzero class in §18.4 for review.
This appendix and [HAL370 §14](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#14-proposed-s1--coordinated-admission-of-two-derived-source-frames)
await independent coordinated adoption. JC retains product authority and final
veto. Existing binding text governs. S2, S3 and S4 require separate authority
and evidence.

### 18.1 Source condition and current refusal

The supplied producer observations identify IKEA KOMPLEMENT 902.779.61
triangle 151 corner 0, original indices `[228,231,239]`, and 302.779.59
triangle 453 corner 0, original indices `[472,476,483]`. Both report the
finished canonical Mikk value `(0,0,0,-1)` on nondegenerate geometry with a
nonzero UV determinant. The inspected source producer finishes the callback
before checking its unit direction and refuses this value as `UnitDirection`
at `src/presentation/pbr_source.rs:1224–1238`. The existing R024 §§7.1.1, 7.1.5
and 7.3 and HAL370 §4.1 require complete valid original signed tangent
evidence. HAL also checks every raw tangent direction before selecting a
frame. Mode C separately requires a singularity witness involving nonzero
original smooth normal and tangent directions. A zero callback establishes
neither a unit tangent nor that witness.
[Original tangent gate](../../src/three_d/geometry.rs#L1775)
[Mode C witness gate](../../src/three_d/geometry.rs#L1884)

Five additional 902.779.61 triangles are reported to have nonzero canonical
outputs outside A/B/C. They remain outside the zero-callback class. Full
original admission cannot follow from qualifying the zero-callback class
alone. See the candidate roster and evidence boundary in §18.5.

### 18.2 Proposed zero-callback source class

`PbrDefaultMikkZeroGeometricFrameV1` would be an explicit per-triangle
producer-authored rendering interpretation for a present normal role and
genuinely absent source TANGENT. Validate the complete ordered original
primitive, its exact source revision, node, mesh, primitive, material,
accessors, index and corner association, actual original local f32
POSITION/NORMAL/UV0 and color presence, and the original proper node
transform and checked placement under current rules. The affected triangle
must have exact positive geometric area and exact nonzero UV determinant.

Qualification originates in the actual controlled pack/source transaction.
The qualified producer verifies the original glTF/GLB root and closure,
genuine authored TANGENT absence and the present normal role before
invoking pinned default-Mikk. Require successful generator return and a
final assigned signed vec4 for every original indexed corner. Retain the
complete original input and all final raw outputs. Bind that capture to
the exact source and producer revisions, generator configuration and
checksum, adapter revision, ordered input/corner correspondence and the
separate generated-frame proof. The source-producer reference pin is
`mikktspace 0.3.0`, default 180 degrees, with the nalgebra default feature,
adapter revision 1 and package checksum
`7d0b56b403871a8f992ca626d52cc0a690d4841baea8955dc4af6304ac62f8b0`.
Every callback output must be present, finite and signed with exact w of
`-1` or `+1`. The declared triangle must have at least one callback whose
xyz components are exactly zero. Preserve the exact bits of all four
components at each of its three corners, including signed zero and the
actual w. Preserve all other original callback records and normals.

The reviewed external producer setter can overwrite an occupied slot. Successful
return and complete final assignments establish final coverage at pack
time. Neither the `Option` slots nor `callback_reads` establish at-most-once
setter execution. The pinned emission loop describes the shape of a
successful run. HAC carries the final capture and supplies no historical
setter trace. A stronger multiplicity condition would require separate
pack-time specification and checkable enforcement.

Ordinary prepared opening would check source root and binding, digests,
the exact ordered capture, configured producer and proof metadata without
regenerating Mikk. Those checks establish structural association for
producer-attributed data. With a hostile full-HAC rewrite and recomputed
hashes, independently establishing historical execution would require an
externally trusted asset-origin channel outside this S1 boundary. This
proposal adds no signature, attestation or auxiliary service. Refuse
provenance that cannot be qualified under the controlled producer
boundary. A digest, declared `invocation_completed` flag, source name or
producer claim alone cannot qualify the capture or replace HAL's checks.

Retain the complete `PbrOriginalGeometry3D.signed_tangents` count. Never
normalize an admitted exact-zero raw record or replace it with the rendered
tangent. This limited exception would apply only to the exact zero corners
covered by the independently checked certificate for this interpretation.
All other original and packed validations still apply. For triangles with
nonzero canonical outputs, the current A/B/C routes apply. The distinct
proposed class in §18.4 has its own declaration and proof. Refuse a
present invalid authored TANGENT, missing or malformed callback,
nonfinite component, invalid sign, unqualified source identity,
nonzero-only input claimed as this interpretation, degenerate geometry
or singular UV. All original triangles remain in order with their
original winding, indices and material lineage.

### 18.3 Shared derived frame and CPU evidence

Both proposed classes use §7.1.3's exact local-source construction and
outward checks for the geometric frame, including its bitangent
orientation test. The three positions and UVs are the ordered original
local binary32 values interpreted as exact reals.

```text
e1 = p1 - p0, e2 = p2 - p0
C = cross(e1,e2), Aface = |C| > 0, F = C/Aface
du1 = u1 - u0, dv1 = v1 - v0
du2 = u2 - u0, dv2 = v2 - v0
det = du1*dv2 - du2*dv1 != 0
dPdu = (e1*dv2 - e2*dv1)/det
dPdv = (-e1*du2 + e2*du1)/det
G = dPdu - F*dot(F,dPdu), |G| > 0
U = G/|G|, H = sign(det)
dot(H*cross(F,U), dPdv) > 0
```

Independently check the exact original winding and UV sign, finite
representability, packing enclosures and actual render-corner mapping.
All three PBR52 corners must carry bit-identical checked Fp and Up triples
and exact tangent.w = H. For one checked unit separator b, require
`lower_outward(b · (Fp_i × Up_j)) >= 1/16` for all nine actual packed
pairs, with the finite packed norm upper bounds of §7.1.3. Reject
unresolved enclosures and mismatched positions, UVs, colors, packed
directions or signs. Retain the original smooth normals and raw Mikk bits
separately. Seam expansion may duplicate render vertices with exact
original corner associations. The source and Inspector receipt must
disclose the selected class, the derived frame and possible faceting or
changed normal-map appearance. Visual equivalence to authored smooth
shading remains unproved.

For both classes, use §7.1.3's direct packed proof. Define b as the
mathematical normalization `s/sqrt(s·s)` of a finite nonzero binary64
seed s, with a certified outward enclosure under §7.1.5. The direct
witness may be chosen independently of the ideal perturbation witness
`bC = F × U`. Keep bit-identical Fp and Up and exact H at all three
render corners. Check every finite supplied lower against
`1/16 <= claimed <= lower_outward(b · (Fp_i × Up_j))`. Require separate
finite positive norm upper bounds `M_F >= |Fp|` and `M_U >= |Up|` on the
actual packed triples, and finite nonnegative Euclidean error upper
bounds `epsilon_F >= ||Fp - F||₂` and `epsilon_U >= ||Up - U||₂` relative to
the exact original-derived F and U. Each supplied upper must reach its
own independently verified outward upper endpoint. The errors are
separately verified source facts. The perturbation expression
`1-epsilon_F-epsilon_U-epsilon_F*epsilon_U` remains an alternative proof
under §7.1.3. It adds no threshold to the direct route and cannot replace
any of its nine checks. An understated or unresolved bound yields typed
refusal even when the nine direct projections pass.

The proposed per-triangle certificates belong in an additive CPU input
with distinct variants for the zero and nonzero classes. Keep
`PbrGeometryDescriptor3D`, existing source literals and the exhaustive
`PbrFrameCertificate3D` alternatives intact.
The external source-container encoder and summary match that enum exhaustively in
`src/presentation/asset_container/wire.rs:260–292,674–685`. The new input
would retain one certificate per original triangle, either an existing
frame record or the explicitly selected class's typed eligibility and
packed-frame proof. For the mapped classes, bind original identity, source
digest, all input and callback corners, producer and interpretation
versions, derivation and checked render-corner output. Verify source
identity, digest correspondence and complete input/callback coverage against
the qualified producer's immutable original capture under §18.2. The
existing HAL geometry checker and Context owner independently verify the
original and render counts, exact bits and corner mapping, original
normal and signed Mikk evidence, and class-specific frame and numerical
proofs under the current Context and leases. Associate each raw zero with
its exact zero-callback certificate before skipping its unit-direction
check. Require all §18.4 exclusion proofs on a nonzero-class record
before checking its derived frame. Both classes validate the packed
geometric frame independently of Mode C's singularity branch. Counts and
original/render mapping checks
remain complete.
[Public frame alternatives](../../src/three_d/material_api.rs#L370)
[Complete count check](../../src/three_d/geometry.rs#L1070)
[Original/render correspondence](../../src/three_d/geometry.rs#L1809)
[Packed lower bound check](../../src/three_d/geometry.rs#L2029)

### 18.4 Proposed nonzero canonical Mikk class outside A/B/C

`PbrDefaultMikkNonzeroOutOfDomainGeometricFrameV1` would be an explicit
producer-selected source shading interpretation for an original mapped
triangle with positive exact area, nonzero original UV determinant, and
genuine glTF TANGENT absence. Require the same qualified producer and
complete pinned default-Mikk input and final callback capture under
§18.2. All three original xyz vectors must be finite and nonzero, each
with exact w of `-1` or `+1`. Preserve the original smooth NORMAL and
every signed Mikk bit, independently of the derived frame. Require all
three proofs below
for the same triangle. Bind the raw vectors and the actual normalized
and packed smooth candidates to the same source and checked §7.1.1
normalization/packing revision. The packed smooth candidate used in
these proofs is the source's smooth interpretation before derivation.

A producer-declared N B/C proof revision selects one deterministic
canonical packing of the mathematical unit directions from the original
indexed NORMAL and ordered captured Mikk xyz. Bind the declaration to
the qualified source and certificate and independently reconstruct the
six pre-derived packed triples. Retain all raw bits, including signed
zeros and original w, separately. B and the packed C domain use only
the declared triples. N renders the derived frame and needs no separate
persisted smooth stream. Legacy retains its own declared packed bits.
Refuse an unsupported or mismatched interpretation without fallback.

1. **Outside Mode A.** For at least one required original unit normal
   or tangent cone comparison from §7.1.2, prove a strict outward upper
   bound below `3/4` on the dot with its corresponding qualified
   original `N_face` or `T_uv`. An unresolved lower bound or an
   unsuccessful attempt to prove sufficiency does not establish exclusion.
2. **Outside Mode B.** For the actual packed smooth triples, form all
   nine exact cross products `c_ij = N_i × T_j`, including the off-diagonal
   pairs. Supply exact rational weights for the full set, with
   `w_ij >= 0` and `sum(w_ij) = 1`. Record the complete positive support
   and every zero weight. Let `q = sum(w_ij * c_ij)` and `d² = q·q`.
   Verify all nine exact KKT inequalities `q·(c_ij - q) >= 0` and
   equality at each positive support weight. The inequalities extend
   to the whole convex hull and certify q as its closest point to zero.
   Require the strict exact upper certificate `d² < 1/256`. For every
   unit b, `min_ij(b·c_ij) <= b·q <= sqrt(d²) < 1/16`. Thus no unit
   separator meets Mode B on that exact packed smooth candidate.
3. **Outside Mode C.** Prove strict homogeneous separation through
   the origin for the normal and both signed tangent hull families.
   Check this separately on the original nonzero directions and on
   the actual normalized and packed smooth triples. Positive exact
   normalization preserves the original generated cones. For each
   domain, let `N_i` and `T_j` denote that domain's three normal and
   three tangent vectors. For each `s` in `{+1,-1}`, bind a finite
   nonzero separator `a_s` with a strictly positive outward lower
   bound for every `a_s·N_i` and every `-a_s·(s*T_j)`. These bounds
   cover all three vertices of each hull and hold on their convex
   combinations and preserve sign on positive multiples. The two
   signed tests exclude common nonzero rays for parallel and
   antiparallel directions in both domains. The choice of `s` is
   independent of the original tangent.w handedness. Affine
   separation of unscaled hull points alone does not prove exclusion.

Apply the same exact original POSITION/UV F/U/H derivation and
bit-identical packed Fp/Up/H and nine-pair `1/16` proof from §18.3.
The output retains every original triangle, position, UV, color, index,
winding, material and R025 face/caster association. This class changes
the rendered frame and requires the same explicit source and Inspector
disclosure. A missing, unresolved or mismatched proof refuses. A failed
heuristic search, partial support enumeration, rounded residual,
asset-name or triangle-index allowlist, and automatic fallback carry
no eligibility.

Binding §7.1.2's three-mode rule and “no fourth guessed frame” require a
narrow express later amendment, coordinated with HAL370 §4.1, to admit
these named producer-authored classes. A/B/C alone cannot authorize
them. Keep the prohibition on guessed frames and all current rules
for A/B/C. This proposal selects no runtime fallback.

### 18.5 Invariants, refusal and future proof

- **Original custody.** Retain original GLB bytes, complete original
  attributes and indices under the source identity. Retain every final
  signed callback result for all original indexed corners of a
  present-mapped source, including the exact raw bits. Preserve any
  actual authored tangent. The separately qualified branch with a
  genuinely absent normal role, no authored TANGENT and no Mikk invocation
  or capture has an empty original signed tangent stream.
  Under the proposed private origin in [HAL380 §6](SPEC-HAL-380-Nonblocking-PBR-Geometry-Certification-and-Admission.md#6-dual-class-complete-cpu-source-ingress-clarification),
  every original triangle keeps §7.1.4's normal-only proof through the old
  normal cone or certified packed unit-d projections at least `1/16`.
  The finite PBR52 tangent/sign field remains unused structural padding
  under §8.4.3 and supplies no original tangent evidence.
  Forbid fabrication of a tangent, triangle deletion, material or normal
  role substitution, an asset-name allowlist and automatic frame selection
  following refusal. The interpretation and certificate have their own
  immutable version and generation.
- **Complete verification.** Keep all R028 original normal chains, bytes,
  scale, sampler, complete common-UV proof and `1/510` floor. Retain R025's
  same actual original faces and all eligible casters, receiver, Mask and
  sidedness obligations. Keep §8.2.4's source/host and observed-backend
  evidence split. Forbid epsilon classification, shader repair and
  view-driven producer work.
- **Bounded publication.** Check counts, certificate bytes/work, retained
  original and render copies, seam expansion, source/mapping/producer and
  Context generations, actual Device/target and current radiance. Bound
  exact rational work and temporary proof storage under the existing
  policy. Charge all old/current/pending/in-flight ownership. Refuse
  invalid or exhausted candidates before writes. Publish a complete successor under §§9–10
  and R027, with the exact compatible old front on refusal and normal
  incompatible withdrawal and Queue-safe retirement. Forbid partial
  publication, source relabeling and a silent fallback.

**KOMPLEMENT 902.779.61 is NOT ADMITTED.** The reported original has one
primitive with 1,500 triangles and 4,500 original indexed corner
occurrences. The candidate partition is 1,492 A/B, three zero-callback
candidates and five nonzero-class candidates. The following values are
reported approximations to exact Mode B squared optima.

| Original triangle | Original indices | Reported d² |
|---:|---|---:|
| 254 | `[264,263,312]` | 0.00169490832043718 |
| 255 | `[312,313,264]` | 0.00198687356044944 |
| 256 | `[314,312,263]` | 0.00168159873947262 |
| 258 | `[268,264,313]` | 0.00065754309713464 |
| 259 | `[313,315,268]` | 0.00115651161857412 |

These numbers are below `1/256 = 0.00390625`. The reported five
derived-frame nine-pair minima are about 0.99999998 or higher, with
`H = -1` matching the reported Mikk sign. The reported complete R028
proof has 18,195,799 cells and lower bound `0.0027174434928823456 > 1/510`.
These are supplied evidence, not independently reproduced.
The exact rational receipts are unavailable in the current Halcyon
workspace. The decimal table cannot establish support, KKT, A/C exclusion
or actual packed-frame proof. Verify the original rational receipts
against the qualified original capture and exact packed bits before claiming
any of the five exclusions. These triangle IDs are proof locators.

After separate authority, reproduce the original refusal and triangle 151
corner 0 bits, verify all source inputs and outputs and the full
partition, and cover independent nondegenerate geometry and both UV
orientations. Include zero/nonzero cross-class mismatch, all-zero
positions, zero area and UV, missing, forged, authored, nonfinite and
invalid-sign callbacks, failed or incomplete A/B/C exclusions, wrong
corner mapping or packed frame, stale Context and capacity refusal.
Include a B bound at equality and a missing off-diagonal KKT inequality.
For C, cover parallel and antiparallel counterexamples in either domain.
Prove actual N1/N4 source/CPU/GPU and registered FP16 Image with
Direct/Mask, seams, culling, complete off-camera shadows, compatible
old front and Queue retirement. Keep the valid original Poly Haven
sofa and desk and the original external-consumer conformance controls. Record native test results
separately and gate prepared Inspector work and JC appearance review
under [HAL370 §14](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#14-proposed-s1--coordinated-admission-of-two-derived-source-frames).
