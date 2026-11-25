# SPEC-R-023: 3D Visual Rendering Extension Architecture

## Historical drafting scope

This section and the following architecture review record the historical original authoring unit. Its original source receipt and the historical adversarial receipt in §17 are retained; §18 preserves the subsequent decision-lock/dual-space authoring receipt. Their task-time inactive/no-adoption statements do not override the current S1 header and §1.

- Objective: Specify one optional, domain-neutral 3D visual-quality extension foundation over HAL-355, with explicit parent-contract dependencies and source-to-native proof gates. This unit creates a draft, not a renderer or an implementation task.
- Source qualification: HAL-355, HAL-357, HAL-295A, HAL-266A, HAL-263A, adopted HAL-309, HAL-000 §11 and HAL-354's Update/publication boundary were checked. Existing R-series declarations and the relevant R-013, R-021 and R-022 bodies were inspected.
- Governing authority and status: HAL-002 Foundational and HAL-002A Permanent are supreme; HAL-355/357/295A/266A/263A are Adopted / Binding; HAL-354 is Binding; HAL-309 is Adopted. The explicit HAL-R-023-3D-VISUAL-RENDERING-EXTENSION-ARCHITECTURE-01 mandate authorizes this draft only. Existing Design, Draft and Proposed texts supply no additional authority.
- Authority change, if any: NONE. This entire document is Draft / Proposed / INACTIVE / NOT BINDING. Its requirements describe a proposed future contract; they do not amend a parent, authorize engineering, or activate any capability.
- Target layer and existing state verified: Halcyon 0.227.18-beta, master HEAD `17884db5b09862d8013805cefa232c7c71b2f810`, current working-tree source bindings in §15. Receipt baseline: nine tracked modified files, two existing untracked picker files, no staged files. R-023 was absent; literal `R-023` search across 480 SPEC Markdown files returned no references before creation.
- Owned mechanisms and files: CREATE ONLY `docs/specs/SPEC-R-023-3D-Visual-Rendering-Extension-Architecture.md`. All existing documents, Rust, tests, examples, manifests, recorded history and unrelated working-tree bytes are protected.
- Named invariants: HAL-002 phase/identity/snapshot/sole-tree ownership; HAL-355 AUTH-001/002/003, ABSENCE-001, complete tuple, finite resources and truthful publication; HAL-266A INV-001/003/005/007/014; HAL-295A INV-003 through INV-010; HAL-263A INV-001 through INV-008. Proposed INV-R-023 / FORBID-R-023 pairs are in §10.
- Forbidden behavior: Parent-law override by draft, another renderer/device/queue/scheduler, retained 3D semantic graph, hidden light/global context, raw GPU escape, arbitrary shader bindings, 2D contamination, partial/mixed fronts, unbounded resources, guessed color transfer, double tone mapping or alpha association, GPU-derived application authority, and invented implementation/backend/native evidence.
- Existing mechanisms inspected and reuse decision: Reuse explicit Context3D resources, checked geometry/texture/sampler/appearance admission, flat RenderSnapshot plus complete ModelState, ordinary Viewport3D participation, HAL-266A metrics, HAL-295A front publication and normal Image composition. The actual ABI, LDR output and single-mip material texture limitations are qualified in §3; no replacement mechanism is selected.
- Relevant external or dependent boundaries: Applications own asset decoding, scene content, light placement/meaning, physical validity and consumer policy. Halcyon owns mechanical rendering and resource/target admission. The reference application motivates this work but supplies no architectural authority; no consumer source or assets are touched.
- Test ownership and placement: No tests added or executed. Future CPU/ABI/resource/raster/viewport proof belongs at the existing `src/tests/three_d_*.rs` owners; retained target proof at `src/tests/custom_render_preparation.rs`; external canonical runtime proof at `tests/app_frame_realization_native.rs`. §12 defines the additional proof, not claims that current tests cover new features.
- Exact runnable verification commands and expected proof: §14 gives Grip file discovery, complete readback, hash and status commands. Expected proof is one complete inactive Markdown artifact with valid owner/source references and preserved baseline bytes. No Cargo check, build, test or native launch can prove adoption of this draft and none is run.
- Output or artifact completeness boundary: One self-contained foundation containing current baseline, owner/request/color/resource/target laws, paired invariants/prohibitions, exact parent-extension matrix, prospective subordinate scopes, conformance gates, status separation and source bindings. It contains no proposed Rust signature or delegated implementation choice.
- SemVer and changelog impact: N/A — inactive documentation only; no product/API/release change, no manifest or changelog edit.
- Out of scope: Parent-SPEC edits, adoption, runtime implementation, shader experiments, importers/assets, camera/Capture or target-eligibility policy decisions, device recovery, 2D R-series modifications and creation of R-024 through R-028 files.

## Historical architecture review

- Governing SPEC and read status: Supreme HAL-002/002A and exact affected HAL-355/357/295A/266A/263A/309 clauses source-read; HAL-354 Update and current R-series status checked. Source bindings are explicit in §15.
- Widget policy or N/A: N/A — a rendering architecture draft with no widget behavior change.
- Interaction added and policy authority: NONE. Existing input, gesture, focus, Capture, navigation and semantic commands retain their owners.
- Applicable FORBID rules: Complete Constitution; HAL-355 2D-CONTAMINATION, HIDDEN-CONTEXT, RAW-GPU-ESCAPE, ARBITRARY-SHADER, FALSE-PUBLICATION, TARGET-MISMATCH, PRIVATE-SCHEDULER and UNLIMITED-BUDGET; HAL-263A association and HAL-295A/266A lifecycle/phase prohibitions.
- Existing framework mechanism inspected: Current Context3D content/appearance/raster, complete snapshot/model values, Viewport3D, indexed WidgetTree preparation, App consuming path and CustomRenderPass submit/swap/publication.
- Reinvention risk: The extension stays inside the existing specialized 3D offscreen owner and ordinary Image seam. It introduces no general scene framework, GPU service, shader escape, bus or second timing authority.
- Demo/example compliance: No example changed or launched. Showcase shader source is evidence of a narrower existing appearance capability, not general lighting or native conformance.

**Author:** JC  
**Date:** 2026-10-31  
**Status:** Adopted / Binding [S1 — foundational optional visual architecture under HAL-370]  
**Layer:** Optional 3D visual-quality extension architecture  
**Technical decision record:** HAL-R-023-3D-VISUAL-RENDERING-EXTENSION-ARCHITECTURE-01  
**Final owner and veto:** JC  
**Adoption:** PERFORMED 2026-10-31 through the delegated architectural reviewer's coordinated S1 decision under JC's express decision delegation; JC retains final veto.  
**Implementation and proof:** NOT IMPLEMENTED; S2 NOT AUTHORIZED; S3 source/numerical/compiler/Device/backend and S4 native/performance/JC proof OPEN.  
**Implementation readiness:** NOT IMPLEMENTATION-READY for engineering or user-facing capability admission; the selected architecture is binding, while the remaining gates are distinguished in §13.1.  
**Current adoption record:** HAL-370-R023-R028-S1-COORDINATED-BINDING-ADOPTION-01. Earlier decision-lock/correction identifiers retain historical provenance.

## 1. Purpose, scope and status gate

HAL-355 already supplies the retained, typed, depth-correct offscreen 3D substrate. Its BASELINE deliberately excludes a material/lighting/shadow/environment/HDR/mipmap system. A modern indoor view needs those capabilities to have explicit owners and compatible contracts before engineering can introduce them. Replacing the substrate or hiding new inputs in its fixed appearance ABI would defeat that boundary.

R-023 defines the common architecture for optional visual-quality extensions: one complete immutable render intent, mechanical engine evaluation, finite typed resources, current target qualification, atomic front publication and the existing Image composition seam. Feature-specific mathematical models and contracts belong to the coordinated R-024–028 scopes; implementation admissions remain separate.

The first demonstrable vertical is a modest native indoor room with actual textured floor, walls and furniture, source-qualified lights, and shadows that respond to moving occluders. It is not a promise of photorealism, physically validated dimensions, indoor collision/Walk, offline path tracing or a game engine. Opaque and alpha-mask geometry come first. Glass and general fractional translucency are deferred and receive no authority here.

All MUST/SHALL/FORBID language below is binding at S1 within [HAL-370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md)'s exact opt-in scope and HAL-355 §17.1's narrow acknowledgment. HAL-370 §1 records the delegated architectural reviewer's affirmative adoption under JC's express acceptance delegation; JC did not thereby personally review the technical SPECs and retains final veto. This coordinated decision adopts R-023–028 together, not by inheritance from R-023. Constitution and all unextended parent boundaries remain controlling. S1 supplies no S2 engineering grant, implemented capability or S3/S4 proof.

## 2. Governing authorities and existing R-series boundary

| Authority | Exact declared status / affected scope | Relationship to this S1 contract |
|---|---|---|
| [HAL-002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md), §§2–5, 8–10 | Foundational; immutable except explicit amendments | Permanent retained ownership, IDs, snapshots, phase separation and sovereignty remain intact; no constitutional amendment is proposed. |
| [HAL-002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md), §2.2 | Permanent Appendix | No non-amendable principle may be weakened. |
| [HAL-355](SPEC-HAL-355-Native-3D-Rendering-Substrate.md), §§3–17 | Adopted / Binding | Sole baseline 3D substrate; §17.1 acknowledges only HAL-370 §8's opt-in exceptions. §11 below preserves the exact boundaries. |
| [HAL-370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md), §§1/4–8/12–13 | Adopted / Binding [S1] | Exact coordinated family/source/metric/ABI/final-format exceptions and stage gates; no S2 grant. |
| [HAL-357](SPEC-HAL-357-Shared-3D-Spatial-Domain-Composition.md), §§1–2, 4.2, 6–8, 12 | Adopted / Binding | Optional one-level producer composition still lowers before flat raster preparation; it does not admit lights, a renderer hierarchy or new material ABI. |
| [HAL-354](SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md), §4.1 | Binding | Existing Update/cadence, complete immutable publication, mounted G1 transfer and proportional invalidation are preserved. |
| [HAL-295A](SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md), §§2–4 | Adopted / Binding | Admitted offscreen ownership, same Queue, enqueue/swap/publication, privacy and baseline device lifetime. |
| [HAL-266A](SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md), §§3–7 | Adopted / Binding | Final immutable target metrics, indexed post-layout/pre-paint preparation and target-local lifecycle, not another phase. |
| [HAL-263A](SPEC-HAL-263A-Image-Alpha-Contract-Correction.md), §§3–5 | Adopted / Binding | Final sampled Image is premultiplied linear; exact tint/coverage equation and declared ingress are unchanged. |
| [HAL-309](SPEC-HAL-309-Documentation-System.md), §§2, 7–8, 12–14 | Adopted | Authority, implemented source, executed proof and native acceptance remain different claims. |
| [R-013](SPEC-R-013-GPU-Shadows-and-Soft-Masking-Pipeline.md), §§1–3, 5, 13 | Design | Analytical rounded-rectangle 2D UI shadows; no 3D light/occluder/depth proof or permission. |
| [R-021](SPEC-R-021-RoundedRect-Pipeline-Contract.md), §3 | Active (Binding) | 2D RoundedRect routing/scissor contract remains untouched. |
| [R-022](SPEC-R-022-SDF-RoundedRect-Pipeline.md), §§1, 6–10 | Draft | Its proposed 2D pipeline does not become binding through R-023. |

The paired HAL-363/364 provisions marked Proposed inside existing documents remain inactive. R-023 imports no historical-capture, choreography, device-epoch or execution permission from them. Camera/gesture conflicts and pre-Update target-eligibility proposals remain separate; this contract resolves neither.

The full filename/status inventory is in §16. Status testimony in that inventory is not a fresh implementation or runtime audit of those 2D systems.

## 3. Current verified baseline — source facts, not native proof

The following is the inspected HAL-355 baseline at the exact historical working-tree hashes in §15. It is not evidence that an S1-adopted extension is implemented or that a backend is conforming.

| Boundary | Existing authority and actual source | Consequence for extension work |
|---|---|---|
| Explicit resources | HAL-355 §§3.1, 5.2, 7; `context.rs:1617–1658, 2085–2236` | Context3D captures existing GPU resources; typed context-bound registration/leases are reusable. No new device, queue, global context or public raw GPU resource is needed or admitted. |
| Triangle content | HAL-355 §§6, 8.1–8.2; `content.rs:8–12, 370–447` | Position, optional normal, UV and linear vertex color already exist in the fixed 36-byte stream. Triangle lists support unindexed/u16/u32 forms. Missing-semantic defaults do not prove authored normal/UV presence. Tangents/extra streams are not baseline. |
| Textures/samplers | HAL-355 §§7, 9.1; `content.rs:59–218`; `context.rs:2565–2720` | Decoded RGBA8 Unorm or sRGB, typed 2D texture and min/mag/address policy exist. Upload creates exactly one mip level (`context.rs:2622–2643`). A mip-filter field in the sampler implementation is not a mip-chain capability. |
| Flat request | HAL-355 §§3.5–3.8, 8.3–8.4, 11.2; `snapshot.rs:79–200`; `viewport.rs:815–970` | One immutable structural snapshot, complete model value and checked view/sample request already exist. No independent scene-light/environment input exists in that inspected flat packet/appearance ABI. |
| Appearance ABI | HAL-355 §§9.1–9.4; `content.rs:495–557`; `appearance.rs:355–438, 1685–1732` | Controlled Naga admission and composition exist. Exactly three baseline groups, 80/112/16-byte blocks and one texture/sampler apply. Wrapper clamps linear RGB to [0,1], surviving alpha 1. No scene-light or shadow-map access may be inferred. |
| Existing diffuse example | `examples/showcase/tab_viewport_3d.rs:104–121` | Consumer-authored WGSL normalizes the supplied normal and uses a constant light direction for a diffuse-like term. This is a genuine narrower appearance, not an engine light store, PBR model, moving-light system, shadow pass or native-quality proof. |
| Raster/depth | HAL-355 §§6.5, 8.5, 10.1–10.3; `raster.rs:24–50, 153–180, 667–722, 1556–1619` | Reverse-Z Depth32Float/clear 0/Greater, CCW/back culling, opaque/mask, sample 1/4 and one ordinary offscreen color pass exist. No intermediate HDR/tone-map or shadow pass is baseline. |
| Target/preparation | HAL-266A §§3–6; `custom_render_prepare.rs:177–253`; `widget_tree.rs:1393–1435`; `app.rs:3267–3275` | Final logical geometry and renderer metrics determine activity/physical render extent. Indexed participants prepare before Paint. Local zero coverage can be inactive even when the root target is positive. This says nothing new about a navigation consumer's earlier eligibility. |
| Publication/composition | HAL-355 §§10.4–11.6; `viewport.rs:1250–1372, 1442–1479, 1502–1528`; `custom_render_pass.rs:518–542`; `app.rs:18952–19008` | Submitted offscreen writes precede swap/registration and committed state. Paint emits ordinary target-bound full-UV WHITE Image. Normal compile/render/output submission remains engine-owned. Call sites do not prove GPU completion, scan-out or visual acceptance. |

HAL-355 §17 explicitly excludes lighting/PBR/shadows/environment, asset loaders/mip generation and HDR/tone mapping from baseline. HAL-355 §17.1/HAL-370 §8 now admit only the named opt-in contracts, without implementing them or changing those baseline exclusions. These boundaries are not evidence that normals, UVs, textures or depth are absent. No glTF/GLB importer or decoded-image file format is supplied by this contract; applications must deliver validated resource content through an independently admitted producer.

## 4. Ownership and integration contract

The extension SHALL retain this single ownership chain:

1. Application owners prepare immutable scene content, placements, view and authored light/environment facts under their own semantics.
2. The existing Halcyon 3D owner admits complete, typed, version-qualified render input and resource leases under an explicitly enabled extension contract.
3. The canonical frame owner finishes lawful publication before its consuming preparation. Optional HAL-357 lowering finishes at the flat boundary; no renderer-side domain traversal appears.
4. HAL-266A supplies the exact final current target metrics. The admitted 3D participant evaluates only mechanical rendering and manages its own bounded offscreen resources.
5. Every writer to a candidate front is enqueued on the existing baseline Queue before HAL-295A swap/publication and atomic commit.
6. The existing Image path composes the final front. Layout, clip, overlays, z-order, target selection and presentation remain with their current owners.

This is an ownership sequence, not a new public API or extra frame phase. “One engine GPU/submission infrastructure” means the existing device/queue and enqueue/publication authority. It does not falsely require one `Queue::submit` call for an entire frame: HAL-355 §10.4 and HAL-295A INV-008 permit the existing correctly ordered per-view submissions.

| Concern | Sole authoritative owner | Permitted mechanical extension role |
|---|---|---|
| Asset meaning, file decoding, physical units, furniture/room identity | Application/asset producer | Validate typed submitted geometry and materials; never certify physical correctness from rendering. |
| Authored lights, placement, enablement and meaning | Application scene owner | Evaluate an explicitly supplied immutable light set; no implicit/global light or invented semantic entity. |
| Original HDR environment decoding, E irradiance/specular prefilter and BRDF-LUT derivation; static two-bounce G transport, visibility integration, patch partition and six WORLD-axis irradiance lobe generation | Independent application/source producer under R-026 | Receive and source-qualify complete attributable immutable E/G data; never infer source truth, run G transport, auto-bake, reproject foreign G6 in the shader or own static GI semantics. |
| Admitted runtime direct lighting/shadow visibility and material evaluation of source-qualified E/G | Halcyon specialized 3D context/participant after separate feature and parent admission | Mechanically evaluate already-qualified input and bounded shader passes, maintain coherent source/target/front resources; no semantic application writeback, no second renderer or GI precomputation. |
| Material model, channel interpretation and shader admission | Halcyon 3D feature contract | Admit typed roles and finite values/resources; application supplies content, not arbitrary bindings. |
| Camera/navigation, Capture and movement policy | Existing application/widget/input owners | Consume immutable view only. No new camera controller, capture exception or Walk/physics rule. |
| Widget/tree/mount identity | WidgetTree | Use qualified IDs and lifetime; never hold peer widget/tree borrows. |
| Target activity, extent and output compatibility | Renderer/HAL-266A | Consume the immutable current snapshot; own no OS/DPR inference. |
| GPU resource identity, leases and finite budgets | Explicit Context3D and per-view resource owner | Add only separately admitted typed resource generations and accounting. |
| Front registration/retirement | Per-view CustomRenderPass/HAL-295A | Publish one complete final front through the existing lifecycle. |
| Final Image color/alpha/composition | HAL-263A and existing Image/compositor owners | Deliver already conforming sampled content; no repair or tone map in generic 2D. |
| Cadence, Timeline and invalidation | Existing App/HAL-354/HAL-021 owners | Consume published values; no private clock, worker policy or wake loop. |

## 5. Immutable input and stage truth

A visual-quality request SHALL be complete and immutable for its declared rendering profile. Its qualification SHALL bind the exact source/context, structural snapshot and geometry generations, complete model state, view, material/texture/sampler generations, authored light/environment values, mechanical quality/color policy and target-compatible preparation. No invented Rust carrier is specified here.

Equal revision/identity SHALL imply equal immutable content within its declared domain. Changed content requires an admitted successor; generations and ordering identities must not wrap or resurrect retired sources. Any extension of the currently fixed request tuple requires the parent admission identified in §11. The existing `Viewport3D::set_request` and mounted G1 transfer are not claimed to carry new light/material/environment fields today.

| Changed source | Required future qualification | Work boundary |
|---|---|---|
| Geometry/topology/material binding | New qualified structural/resource identity under the adopted extension | Re-admit only affected structural dependencies; retain leased old generations while genuinely used. |
| Complete object placements | Exact complete model-state identity/revision | Update mechanical visibility/shadow dependencies from those placements; do not reclassify packet order as semantic identity. |
| Camera/view only | View identity/revision | Reuse immutable material/light content; any view-dependent shadow or lighting dependency must be explicit. |
| Authored light/environment or material parameters | Their separately admitted immutable identities/revisions and exact associations | Invalidate every dependent rendering result without silently changing geometry/view/semantic revisions. |
| Target extent, format, samples, activity or device validity | Existing current target compatibility/lifecycle, plus any admitted extension attachment dependencies | No fake application revision and no sampling incompatible fronts. |
| No effective dependency change | Identical qualified input and compatible front | No quality-driven upload, encode, submission, publication or idle cadence. |

The feature contract SHALL define complete mechanical coherence, not claim that unrelated application values are one semantic transaction. Preparation cannot fetch a live scene, invoke a semantic callback, repair a partial light/material set, sample Timeline, or consult mutable application data.

The observable stages remain distinct: source admitted; request accepted; target prepared; GPU writes enqueued; front published/tuple committed; renderer composed/submitted/presented; actual visible result observed. An accepted request is not a committed image; submission is not GPU completion or proof of display.

## 6. Materials and texture-role boundary

R-024's S1-adopted metallic-roughness material scope SHALL explicitly own base color, tangent/normal interpretation, roughness, metallic, occlusion and emissive roles. Admission must bind every used channel/resource and any required geometry semantic to the exact material generation. Missing required data, unsupported representation or stale/cross-context resource is a typed refusal, not an invisible substitute.

Color roles and numeric data roles SHALL be distinguished before sampling:

- Base/albedo color and emissive color must declare their source encoding and decode into the admitted linear-light working representation exactly once.
- Normal, roughness, metallic and occlusion channels are numeric data. They must not receive an sRGB color decode or Image alpha normalization merely because their bytes are stored in a texture.
- Existing vertex-color values remain linear numeric `b/255` under HAL-355 §8.1. Texture storage interpretation cannot silently reinterpret them.
- R-024 selects PbrMetallicRoughnessRealtimeF32V1: declared effective roughness max(source r,.045), conservative source cones/normal texture/scale domain, fragment-LOCAL Gram–Schmidt before L^-T/L dual transforms, a52-byte opt-in tangent stream and a dedicated typed material family. Producer-side default MikkTSpace is selected for eligible missing-TANGENT glTF normal maps. Absent normal role bypasses tangent/TBN work. These are S1 binding opt-in choices, not current baseline features; S2 authorization, producer execution and f32/Naga/backend conformance remain OPEN.
- Opaque/mask classification retains alpha-one surviving fragments and explicit discard. Alpha in a material texture does not authorize general translucent geometry.

The current single texture/sampler can serve its admitted baseline role. It SHALL NOT be repurposed through undeclared binding arrays, ownerless global bindings, shader tricks or an invented packing scheme to claim the new multi-channel contract. A changed material ABI or geometry stream must be expressly admitted alongside its HAL-355 dependency; existing baseline appearances keep their ABI and behavior.

Mip construction/filtering for each role must preserve that role's interpretation under R-028. Color, normals and scalar data cannot be assumed to share one correct reduction merely because they share storage dimensions. R-024/R-028 own the selected channel/filter algorithms; this foundation adds no alternative.

## 7. Light, shadow and environment boundary

Application-authored light and environment inputs SHALL be explicit immutable values with attributable source, revision and lifetime. The application owns their placement, intended meaning and relationship to its content. Halcyon owns only validated mechanical evaluation. The feature-specific contract must state coordinate, intensity/unit, finite-value and color interpretation; neither rendered brightness nor a geometry scale may establish physical truth.

R-025 SHALL require shadow results attributable to the exact admitted light set, occluding geometry/material-mask generations and complete model placements used for the color result. Moving furniture, changing a mask or moving/disabling a light must invalidate the dependent shadow result. A new color front paired with stale incompatible shadows is not a complete commit. The finite shadow resources, sampling/visibility algorithm and failure behavior require separate exact admission.

R-013's 2D SDF falloff, focus ring and rounded-rectangle shadow have no 3D light, occluder or surface-receiver geometry. They SHALL NOT be reused as proof or authority for R-025 object/contact shadows. No screen-space decorative blob may be presented as source-qualified geometric shadow correctness.

R-026 SHALL distinguish environment radiance/IBL from direct lights, and IBL from a separately proved GI approach. An environment image, ambient constant, occlusion channel, “PBR” label or compiled shader cannot by itself establish indirect transport or dynamic GI. Environment resources remain explicit, leased and finite; no global environment singleton or implicit light discovery is admitted.

The selected shadow and indirect algorithms, light kinds/counts, units, update dependencies and quality limits belong to the later architect-owned contracts. R-023 assigns their owners and proof obligations without selecting an algorithm or allowing implementation to decide one silently.

## 8. Resources, targets and failure transaction

All additional textures, samplers, buffers, material/light/environment values, pipelines, shader modules, intermediate attachments and in-flight work SHALL remain explicitly owned by the existing 3D context/view infrastructure. Public ownership remains typed context/kind/slot/nonwrapping generation plus lease, not raw wgpu resources or caller-supplied binding authority.

An admitted extension SHALL expose finite effective bounds for its actual resource/work dimensions: material and texture residency including all mip levels; light/environment data; shadow attachments and views; HDR/intermediate bytes; pipeline variants; upload/CPU staging; in-flight storage; pending/committed/last-good leases; passes and per-preparation work. Checked arithmetic and live device/format limits remain necessary but are not an aggregate memory/work budget. This document sets no guessed numeric capacity or global memory governor.

Resource reservation, validation and all fallible preparation SHALL complete transactionally before exposing a candidate. Every intermediate input and output needed by that front must be compatible and live. No registered front may be written as a back resource; a generation cannot be recycled while a pending, committed, compatible last-good or in-flight owner still leases it.

HAL-266A's final target snapshot remains authoritative. Full physical render extent and visible composition coverage retain their different purposes. The participant must not size from OS DPR, old metrics, a shadow crop or a cached primary target. Any future resolution policy for private shadow/environment work must be explicitly admitted and cannot reinterpret the final Image extent or target activity.

Inactive/zero-area/retired targets SHALL perform no extension rendering, allocation/growth or dynamic writes and cannot keep an incompatible front sampleable. Existing lifecycle cleanup, unregistration and safe lease release remain required. Resize, surface, sample, format, mount and device-lifetime changes qualify all affected intermediates and output, not only the final color attachment. Removal cancels future preparation and retires publication/resources through existing owners. No private device-recovery epoch is introduced.

Typed outcomes SHALL distinguish invalid source/representation, stale identity/revision, unsupported profile/device/format, exhausted finite budget, target inactivity/incompatibility/retirement and terminal device failure. Failure must not silently downgrade rendering, drop required channels/lights/shadows, substitute a new profile or report partial success. An application may explicitly request a separately valid baseline successor; the engine may not pretend that successor satisfied the rejected quality request.

If the old complete front remains compatible, HAL-355's `FailedWithLastGood` truth remains: old front and every associated material/light/model/view identity stay old; requested failure is separately visible. An incompatible target or ended lifetime withdraws the front. One target's failure cannot reattribute or corrupt another target or block unrelated 2D output.

## 9. HDR and final Image seam

Current HAL-355 baseline output stays finite linear RGB clamped to [0,1], alpha 1 for surviving samples, transparent clear and coverage-derived premultiplied front. Its direct surface-compatible target and sample-1/4 resolve path SHALL remain unchanged for baseline consumers.

Any HDR scene-color capability requires R-027 and the explicit HAL-355 extensions in §11/HAL-370 §8. The same admitted color/HDR/tone-mapping chain is also a prerequisite to physically based indoor **quality acceptance whenever the required valid scene radiance exceeds [0,1]**, even if an experiment never allocates an HDR attachment. Clamping that radiance through the baseline wrapper before the chain loses the response and cannot establish the target quality. Explicitly limited LDR experiments remain possible only under their separately admitted scope and must be labeled as such; they do not close this quality gate. R-024 §8.3 carries the same dependency. R-027 owns the selected radiance representation, attachment formats, exposure and tone-mapping policy. The admitted extension SHALL own exactly one explicit scene-color-to-final-color transform before the existing Image composition seam. Generic UI/Image composition must not tone-map the scene again or tone-map ordinary 2D colors as a side effect.

The final registered front SHALL sample as premultiplied linear RGBA under HAL-263A, with zero RGB at zero alpha; viewport tint remains straight-linear opaque WHITE. Storage sRGB encoding/decoding, linear-light computation, tone mapping, coverage/resolve and alpha association must have an explicit ordered derivation and actual production-path proof. “sRGB” without a declared boundary is insufficient.

A nonlinear tone curve applied blindly to coverage-premultiplied RGB need not preserve `RGB = straight_color × alpha`. R-027 must therefore prove its selected transform/resolve/coverage ordering rather than treating a generic post-resolve tone pass as automatically conforming. This obligation selects no algorithm. Written geometry remains opaque/masked; fractional edge coverage is not general glass/translucency authority.

Material color input is not an Image publication. HAL-263A's high-level straight-sRGB-to-premultiplied normalization must not be applied to every 3D albedo/normal/data texture. Conversely a low-level final front is already conforming when registered: Image cannot repair it or multiply texture alpha twice. A new operative Image ingress requires explicit amendment of HAL-263A INV-004's closed set.

## 10. Invariant/prohibition pairs

**INV-R-023-AUTHORITY.** Every feature requires separately accepted exact feature and affected-parent authority before engineering admission; authority, source implementation, backend conformance and JC visual acceptance are independent states.  
**FORBID-R-023-AUTHORITY.** No draft/status inheritance, self-adoption, implicit parent override, or activation of R-024–028/HAL-363/364 through this document.

**INV-R-023-BASELINE.** Existing HAL-355 flat appearances, request validation, alpha/depth/target lifecycle and public consumers retain the unchanged baseline contract.  
**FORBID-R-023-BASELINE.** No silent ABI/output/profile migration, mandatory quality work, validation bypass or hidden rendering fallback.

**INV-R-023-SOURCE.** Each candidate binds one complete immutable source/view/model/material/light/environment/policy input set and the leases needed to render it; equal identity/revision means equal content.  
**FORBID-R-023-SOURCE.** No partial patches, mixed revisions, late semantic fetch, source-lost status, generation resurrection or source inferred from packet order/pixels.

**INV-R-023-OWNERSHIP.** Application owns semantic content/light facts; Halcyon owns mechanical GPU evaluation through explicit context and retained ID-qualified lifetimes.  
**FORBID-R-023-OWNERSHIP.** No retained semantic 3D scene graph/ECS, hidden light/global context, peer widget reference, geometry-to-physical certification or GPU/image semantic authority.

**INV-R-023-ABI.** Every new shader input, attribute, binding, stage and pipeline variant has an exact admitted owner, validation contract, compatibility identity and bounded resource use.  
**FORBID-R-023-ABI.** No undeclared texture/light/shadow access, arbitrary shader/global resource, raw GPU escape, textual wrapper linking or unilateral HAL-355 ABI relaxation.

**INV-R-023-RESOURCES.** Additional resources use typed generations/leases, finite effective limits and checked transactional reservations through the existing context/view owners.  
**FORBID-R-023-RESOURCES.** No unbounded light/pass/mip/history/staging allocation, device-maxima-as-budget claim, second device/queue or private recovery epoch.

**INV-R-023-PHASE.** Existing Update publishes authorized immutable values; final target preparation only performs admitted render-local work; Paint only emits the ordinary Image.  
**FORBID-R-023-PHASE.** No new clock/phase/loop/router/scheduler, render-time semantic callback, Timeline access in preparation or UI/application mutation in read phases.

**INV-R-023-TARGET.** All intermediates and final output are qualified to their actual current source/context/target dependencies; target-local activity, compatibility and retirement govern use.  
**FORBID-R-023-TARGET.** No old/root-only/OS target surrogate, synthetic 1×1 active target, cross-surface mutable state or incompatible front sampling.

**INV-R-023-PUBLICATION.** Every required pass completes enqueue ordering before the one final front publication/atomic tuple commit; compatible last-good retains its complete original provenance.  
**FORBID-R-023-PUBLICATION.** No partial-light/shadow/material front, publish-before-submit, front-as-back write, failure reattribution or submission-as-display claim.

**INV-R-023-COLOR.** Each texture role declares encoding and numeric meaning; lighting operates in its admitted linear-light domain and one explicit final transform precedes Image composition.  
**FORBID-R-023-COLOR.** No guessed sRGB, color decode on numeric normal/scalar data, hidden double decode/tone mapping or unapproved HDR storage under the baseline wrapper. A limited/clamped LDR experiment MUST NOT be accepted as the required indoor quality result when valid scene radiance exceeds [0,1].

**INV-R-023-ALPHA.** Final Image sampling remains HAL-263A premultiplied linear with correct coverage, WHITE tint and exactly-once alpha; opaque/mask remains the admitted object scope.  
**FORBID-R-023-ALPHA.** No alpha-association guessing/repair, post-filter premultiplication, dark/fringed resolve accepted as correct, or general glass/transparency claim.

**INV-R-023-ABSENCE.** No admitted extension means no extension resource/work/cadence; no 3D context/view means structural zero; ordinary 2D owners and baseline 3D remain unchanged.  
**FORBID-R-023-ABSENCE.** No extension types/fields/branches/resources in ordinary SceneGraph, SceneNode, Batcher, DrawPlan, SceneCache, spatial model or RenderGraph hot path.

**INV-R-023-SHADOWS.** 3D shadow truth derives from qualified actual lights, occluders, masks and placements under its separately admitted algorithm.  
**FORBID-R-023-SHADOWS.** No 2D R-013 analytical UI shadow, decorative blob or stale shadow attachment asserted as geometric light-transport proof.

**INV-R-023-PROOF.** Capability claims identify source revision, profile, resources, actual backend/target and executed source-to-output evidence; native quality requires JC's observed acceptance.  
**FORBID-R-023-PROOF.** No compiled-shader/coefficient/stat/screenshot-golden/CPU/headless-only substitute for native proof, no invented importer or realism/support claim.

## 11. Exact parent-extension and compatibility matrix

The exact opt-in authority cuts below are adopted through HAL-370 §8 and HAL-355 §17.1. HAL-263A/295A/266A retain their existing contracts: the same private publisher and preassociated-linear Image seam require no new ingress or parent amendment. S2 engineering authorization and actual source/backend proof remain prerequisites; a later real owner contradiction still requires exact resolution before code.

| Selected capability | Exact controlling baseline clauses | S1-coordinated contract; unchanged seam |
|---|---|---|
| Multi-texture typed material and independent parameters | HAL-355 §§6.1, 8.1–8.4, 9.1 (including no independent parameters), 9.1.1, 9.2–9.5, 13–14, 17 | R-024 §§7/8.2/8.4 select the named realtime profile, source-domain proof, tagged LegacyAppearance OR PbrMaterial,52-byte stream, local normal/tangent varyings and64-byte group2 b0 plus5role textures/5samplers. PBR-only ObjectBlock112 visibility becomes VERTEX\|FRAGMENT; current baseline group1 remains VERTEX. Coordinated parent adoption is S1 PERFORMED; S2 authorization, concrete integration/capacities and f32/Naga/Device proof remain OPEN. Fixed baseline/controlled resources/offscreen privacy unchanged. |
| Independent light input and direct mechanical lighting | HAL-355 §§3.5/3.7, 8.3–8.4, 9.1–9.4, 10.4–10.5, 11–13, 17 | R-025 plus explicit HAL-355 complete input, shader and light-resource/work extension. HAL-266A §§3.4/4 and INV-005/014 consume already-authorized inputs only; preserve Update/preparation cut and admit no semantic callback. HAL-295A INV-007 preserves application sovereignty. |
| Shadow resources and multiple offscreen passes | HAL-355 §§6.5, 9.4–9.5, 10.1/10.3–10.5, 11, 13–14, 17; HAL-295A §3.2 and INV-003/005/006/008; HAL-266A §§3.2–3.4/4 and INV-001/003/005/007/010 | R-025 explicitly extends HAL-355's target/pass/ABI/budget contract and defines any private shadow-depth convention without changing baseline reverse-Z. Coordinated HAL-295A/266A compatibility admission must keep every pass participant-owned, same-Queue, inside existing preparation and before final publication. Renderer-owned graph access/new callbacks remain forbidden; if required, STOP for an exact parent amendment. |
| Environment resources, IBL and a GI approach | HAL-355 §§7–10, 11, 13–14, 17; HAL-295A INV-003/007/008; HAL-266A §§3.4/4, INV-005/007 | R-026 plus exact HAL-355 resource-dimension/sampler/binding/input/work extension, and coordinated offscreen/target compatibility review. A current 2D appearance texture is not an environment/cubemap or GI API. No private continuous transport scheduler is admitted. |
| HDR color/output chain and tone-mapping stage | HAL-355 §§9.3, 10.1/10.3–10.5, 11, 13–14, 17; HAL-263A §§2–4 (INV-002/004–008); HAL-295A INV-003/004/008; HAL-266A §§3.2–3.4/4, INV-003/005/007 | R-027 plus explicit HAL-355 intermediate-format/output/pass extension is required before HDR storage **and before indoor quality acceptance needing valid radiance beyond [0,1]**. Coordinated HAL-263A review proves the ordered transform/coverage derivation with unchanged final Image equation; changing sampled association/ingress requires its own exact amendment. HAL-295A publication and HAL-266A final-target authority remain intact. No prior baseline clamp may be hidden as a completed quality chain; no HDR feature enters generic Image/2D rendering implicitly. |
| Mip chain, LOD and sampling quality | HAL-355 §§7, 9.1/9.4–9.5, 10.4–10.5, 13–14, 17; HAL-295A INV-003/008; HAL-266A §3.4/INV-005 | R-028 plus explicit HAL-355 texture/sampler/storage/work/profile extension and exact admitted preparation scope for any generation work. All mip levels and in-flight generation resources count against finite budgets. Final Image sampling/filtering remains under HAL-263A; it does not generate material mips. |
| Complete extension request and optional domain producer | HAL-355 §§3.5–3.8, 8.3–8.4, 11.2; HAL-357 §§1–2, 4.2, 6–8; HAL-354 §4.1; HAL-266A INV-014 | Each feature admission must close its actual immutable input/public transfer boundary. If it changes the flat tuple or optional composition metadata, expressly coordinate HAL-355/357 and affected HAL-354/266A scope. Do not infer extension carriage from existing flat G1 or require synthetic domains for flat consumers. |
| New device feature, recovery or runtime topology | HAL-355 §§7.4, 14–15, 17; HAL-295A runtime scope and INV-009 | Not authorized by this roadmap. Optional device negotiation, generic recovery or split runtime needs its actual parent owner decision; no feature may force it into baseline or 2D device creation. |

The first five feature scopes cannot be implemented by merely labeling the current fragment function “PBR.” HAL-370 §8 resolves their exact S1 ABI/output/resource/target exceptions; all baseline clauses remain binding and unchanged for unopted consumers. Real implementation and conformance remain unperformed.

## 12. Feature ownership and staged conformance

### 12.1 Subordinate scope ownership only

The architect-selected scopes in [R-024](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md), [R-025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md), [R-026](SPEC-R-026-Environment-and-Indirect-Lighting.md), [R-027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md) and [R-028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md), together with this R-023 foundation, are Adopted / Binding [S1] under HAL-370. Adoption was an express delegated technical-review decision, not an inference from file existence or documentary review. All remain NOT IMPLEMENTED / S2 NOT AUTHORIZED / S3–S4 OPEN / NOT IMPLEMENTATION-READY. Numbering identifies independent feature ownership, not execution or acceptance order: R-027's admitted color/HDR/tone-map chain must precede any earlier-numbered feature's indoor quality acceptance that needs out-of-range scene radiance. No required authority, engineering or proof gate may be bypassed.

| Feature SPEC | Separate S1 binding feature scope | Remaining engineering/proof dependencies |
|---|---|---|
| [R-024 — Typed PBR Metallic-Roughness Materials and Textures](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md) | PbrMetallicRoughnessRealtimeF32V1; r_eff=max(r,.045), local-frame-first GS/dual-transform certificate, qualified tangent profile, 52-byte geometry/64-byte material ABI and producer-side default MikkTSpace route | Selected architecture and exact parent/ABI exceptions are S1 binding. S2 authorization/integration, actual producer output, source f32 margins and Naga/Device/backend/target/native proof remain OPEN. Baseline and one-mip V1 normal semantics unchanged. |
| [R-025 — Direct Lighting and Geometric Shadows](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md) | PbrDirectLightShadowV1; at most one directional/four point/four spot sources, geometry shadows with PCF9, six-chart point-PCF D7 and 54,525,952-byte fixed logical depth pools | Requires admitted R-024 material/local-frame evaluation, calibrated complete light/caster/mask source, group3 b0–b5 within four groups, finite complete radiance and shadow/pass/front ownership. Parent, producer, numerical/backend/native proof remain OPEN; R-027 precedes quality needing radiance>1. R-013 supplies no substitute. |
| [R-026 — Environment and Indirect Lighting](SPEC-R-026-Environment-and-Indirect-Lighting.md) | PbrEnvironmentIndirectV1; world-qualified distant E with six 32² irradiance faces, eight independent one-mip 128² specular cubes and 128² BRDF LUT; independent source-baked STATIC two-bounce diffuse GI6; group3 b6–b10 and eight Direct/E/G static variants | Exact source-world canonicalization, producer preintegration/static transport, same R-024 material policy, conditional bindings within four groups, complete certified radiance and finite lifetime/resources remain required. Parent, producer, f32/backend/native proof remain OPEN. No implicit ambient or dynamic GI; R-027 precedes quality needing radiance>1. |
| [R-027 — HDR Scene Color and Tone Mapping](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md) | HdrSceneColorToneMapV1; per-covered-sample tone before coverage averaging and fixed single-sample LINEAR Rgba16Float display-LDR final | Complete <=60000 scene-radiance certificate, exact parent/internal-format/ABI/target/pass admission and actual f32/FP16/filter/Device/backend/native proof remain OPEN. Required before HDR storage and indoor quality acceptance needing radiance beyond [0,1]; final premultiplied-linear Image and native surface ownership remain distinct. |
| [R-028 — Texture Sampling Quality and Rendering Conformance](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md) | PbrTextureMipSamplingF32V2 paired with the unchanged R-024 V1 material evaluator; complete role-correct mip chains, six MIN modes and explicit gradient sampling | Separate V2 parent/material sampling admission, qualified producer chains/samplers, finite residency/work and actual sampled f32/Naga/Device/backend/native/performance acceptance remain OPEN. No silent V1 normal-map upgrade or mips for R-026 E, shadows or R-027 output. |

The first indoor-materials/shadows vertical uses only the separately admitted dependencies actually needed for its declared profile. It cannot claim environment/GI/HDR/mips it has not implemented and proved. Before physically based indoor quality acceptance that needs valid radiance beyond [0,1], R-027 and the coordinated parent color/output chain must be admitted, implemented and proved; baseline clamping or an explicitly limited LDR experiment does not satisfy that condition. It cannot silently clamp or approximate a required HDR/sampling contract to avoid an unadopted prerequisite. No full-PBR/photorealism claim follows from the modest vertical.

### 12.2 Proof gates — all unexecuted by R-023

| Gate | Required evidence and falsifiable outcome | Nearest actual owner / proof limit |
|---|---|---|
| C0 — Exact authority and source | Reviewed feature text, exact parent extension/amendment decisions, source/manifest/ABI revision and independent engineering admission | Governing owners/JC; S1 records the exact authority decision only, not the remaining source/S2/proof requirements. |
| C1 — Immutable admission and lifetime | Complete geometry/model/view/material/light/environment/policy association; equal/no-op behavior; current generations/leases; finite admission refusal on intended supported ingress; removal/in-flight retirement without history growth | Existing `src/tests/three_d_context.rs`, `three_d_geometry.rs`, `three_d_snapshot.rs`, `three_d_domain.rs`, `three_d_viewport.rs`; CPU proof is not rendering proof. |
| C2 — Shader, transfer and real GPU stages | Exact PBR family/ABI/stage and role transfer; actual sampling; source cones/normal blue/scale certificate; LOCAL normal/tangent interpolation/GS and dual-transform counterexample oracle; source r=0 versus r_eff=.045, safe cancelled visibility and actual f32 margins; mask/depth/sidedness. Light/HDR/mips only if admitted | `three_d_appearance.rs`, `three_d_raster.rs`, `three_d_backend.rs` and actual resource/pass owners. Selected ideal proof, parsing, layout arithmetic and regular coefficients are not actual Naga/device/native conformance. |
| C3 — Real native indoor vertical | One real canonical App::run room with actual glTF-origin furniture decoded into admitted resources and separately source-qualified consumer-owned UV/textured floor and walls. Evidence binds actual mesh/UV/texture sources, per-surface material-channel mapping, authored light identities/revisions, model/view/target associations, real GPU stages and sampled native frames. R-027's admitted chain is required before quality acceptance needing radiance beyond [0,1] | External consumer/runtime proof extends `tests/app_frame_realization_native.rs` under a later task. The producer supplies lawful content and missing UV/textured room surfaces explicitly; Halcyon does not invent them. No loader or quality result is established here. |
| C4 — Lighting and moving geometry | Change light placement/enablement and move actual furniture through complete model-state successors; prove corresponding illumination and shadow response at the committed source-qualified front, including occluder/mask dependencies | Real native pass/resource evidence plus JC observation. A static ambient coefficient, decorative shadow or visually plausible screenshot does not satisfy the causal oracle. |
| C5 — Target/failure/currentness | Positive → zero/inactive/collapsed → positive target, incompatible resize/DPR/format/sample change, unmount/replacement/retirement, finite-budget/shader/resource failure and compatible last-good. No old front is relabeled, sampled incompatibly or resurrected | Existing custom-render preparation/viewport/retained owners plus native target sequence. This is rendering lifecycle proof, not a resolution of pre-Update navigation eligibility. |
| C6 — Compatibility and absence | Same ordinary 2D and flat baseline behavior; compiled-but-unused extension gives no specialized work; no context/view gives structural zero; unchanged qualified active views reuse; warmed view/model changes do not recreate structural resources | Source structural comparison and owner-local counters; actual retained/native comparison for claimed behavior. No new instrumentation branch in normal 2D hot paths. |
| C7 — Quality/performance/backend acceptance | Source/build/profile/adapter/backend/format/target/sample evidence; sampled frames; declared workload/warm-up; measured CPU/GPU/resource/upload/present costs with explicit method, noise and acceptance thresholds fixed before evaluation | Metal/Vulkan/DX12 recorded separately under HAL-355 §14.2; unavailable rows unverified. Browser remains separately proof-gated. JC owns native visual acceptance. |

Native evidence SHALL distinguish request acceptance, target preparation, enqueue, publication, renderer output/presentation call and actual observed quality. An observer/stat counter is not a GPU timing instrument or display-completion witness. A deterministic mathematical/color oracle is appropriate for the owning stage; pixel/style snapshot goldens are not the visual acceptance criterion.

The native room gate requires actual textured geometry and authored lights, not procedural boxes presented as imported furniture. Its evidence must identify material channels actually exercised and any unused/deferred capability. No test is added or native execution requested in this documentary unit; exact runnable implementation commands belong to the later admitted source unit.

Supervisor-supplied consumer evidence for this correction identifies the reference application's `src/presentation.rs:49–82` as solid-palette appearance and `unit_cube:318–366` as positions/normals with UV absent. These paths/ranges are attributable task evidence, not a fresh consumer-source audit by this Halcyon unit. They do not currently satisfy C3's textured floor/wall ingress. The consumer must separately supply source-qualified UVs, textures and mappings; engine material support cannot supply missing asset facts by implication. Existing consumer AABB checks prove only their stated geometric bounds, never physical contact, shadows or native visual success. No consumer file is changed.

## 13. Constitutional consistency and reserved decisions

| Governing boundary | S1 result / exact condition |
|---|---|
| HAL-002 §§3.1/3.4/4.1/4.6; HAL-002A §2.2 | Sole WidgetTree ownership and ID relationships unchanged; immutable resource/input values contain no peer widgets or durable UI references. |
| HAL-002 §§3.2/3.3/3.6/4.7/4.10/5/8–9 | No async UI writer, read-phase mutation or new frame phase; snapshots and existing render-local preparation consume already-authorized input. |
| HAL-002 sovereignty | Platform remains transport; application scene semantics and engine UI/render authority stay separate. No capture/routing policy is inferred from GPU output. |
| HAL-355 §§5, 9–17 | Baseline ABI/output/lifecycle remains binding; only HAL-370 §8's named opt-in exceptions are S1 adopted through HAL-355 §17.1, with 2D zero-use and finite budgets retained. No implementation is authorized. |
| HAL-357 §§1–2/6–8/12 | Optional lowering remains one-level/flat and semantically neutral. No renderer domain graph, new light-domain inference or synthetic flat-consumer domain. |
| HAL-295A §§3–4 | Same offscreen privacy, one baseline device/Queue, per-front enqueue-before-publication and unregister-before-final-drop. No compositor/graph access. |
| HAL-266A §§3–7 | Current final immutable metrics, indexed preparation, no Timeline or semantic fetch, no late paint GPU work and no inactive-target render. |
| HAL-263A §§3–5 | Final sampled premultiplied-linear Image and exact modulation remain mandatory; intermediate color changes require their own admitted derivation and any necessary parent amendment. |
| HAL-354 §4.1 | One Update authority, complete immutable owner publication and proportional invalidation remain; existing flat G1 is not silently widened. |
| R-013 / R-021 / R-022 | No 2D shadow/rounded-rect contract is repurposed, overridden or promoted. |
| HAL-309 §§2/7/13–14 | Draft intent, inspected baseline implementation, executed tests, backend support and native visual acceptance are stated independently. |

The coordinated S1 decision resolves the exact authority cuts identified in §11 through HAL-370 §8, without constitutional weakening or a blanket parent override. It does not establish that an implementation passes source, numeric, Device, lifetime or native proof.

The following architect-selected profiles, algorithms and ABIs are S1 binding in R-024–028/HAL-370; they are not unassigned choices for an executor. Runtime engineering authorization and actual producer/CPU/f32/Naga/Device/backend/native/perceptual proof remain reserved/open at their respective owners:

- R-024's exact profile, typed shader/geometry bytes/stages, roughness/guard policy, local-frame-first ideal certificate and producer MikkTSpace route are S1 adopted in §§7/8.2/8.4 under HAL-370. S2 authorization, concrete public/code/capacity integration and producer/f32/Naga/backend/native proof still require their owning decisions/evidence; no executor may weaken the selected domain to manufacture success.
- R-025 documents PbrDirectLightShadowV1's calibrated light model/limits, geometry-shadow pools, PCF9/six-chart point D7, group3 b0–b5 and coherent failure/publication dependencies. Its parent, source-radiance, numerical/device/resource and native-quality gates remain OPEN.
- R-026 documents PbrEnvironmentIndirectV1's canonical world-aligned distant E, source-baked static two-bounce diffuse G6, group3 b6–b10, eight Direct/E/G variants and finite retention/currentness. Producer transport/preintegration, parent and executed numerical/backend/native proof remain OPEN.
- R-027 documents HdrSceneColorToneMapV1's scene/final LINEAR FP16 formats, explicit exposure, per-sample tone-before-coverage ordering and complete <=60000 source certificate. Parent-format S1 adoption is PERFORMED; S2 authorization and actual source/f32/Device/filter/native proof remain OPEN.
- R-028 documents PbrTextureMipSamplingF32V2's role-correct chains, six MIN modes, explicit-gradient sampling, finite resource law and selected quality/performance thresholds. V1 remains unchanged; parent admission, producer and sampled/backend/native/performance proof remain OPEN.

These selections are adopted S1 architecture contracts, not an executable implementation task. The foundation retains ownership, coherence, compatibility, failure and proof obligations; the delegated adoption decision does not close runtime engineering authorization or actual source/compiler/device/native/JC acceptance. A later capability needing more than these boundaries permit must return the precise owner conflict for independent decision.

### 13.1 S1-adopted architecture versus remaining engineering/proof blocks

**Neither R-023 nor R-024 is IMPLEMENTATION-READY for engineering or user-facing admission.** The material-profile architecture and ideal-math solution from HAL-R024-ARCH-DECISIONS-LOCK-01 and its D3/D5/D7 correction are now S1 adopted through HAL-370. Their ideal proof does not demonstrate source/backend conformance. Historical missing-choice wording is superseded only in the named scope; the following gates retain their names and remain OPEN for the precise S2/S3/S4 requirements, with S1 adoption recorded rather than repeatedly demanded.

| Tracked gate | Architect-selected S1 result | Remaining true block / responsible owner |
|---|---|---|
| BLOCK-R024-PARENT-ABI | PbrMetallicRoughnessRealtimeF32V1, tagged immutable MaterialHandle association,64-byte group2/5+5roles,52-byte geometry/LOCAL varyings, PBR-only Object112 VERTEX\|FRAGMENT layout (§8.4) | Exact parent exceptions S1 adopted; S2 authorization and concrete integration/finite capacity/Naga/device compatibility proof OPEN under HAL-370. Legacy stages/ABI remain unchanged. |
| BLOCK-R024-TANGENT-PRODUCER | Producer-side default MikkTSpace route and source/UV/seam/sign provenance requirements SELECTED | Actual source producer supplies versioned generator/revision, real furniture output, conservative-profile validation, bounded failure and conformance proof. Halcyon architect/JC admits finished data; no framework importer or executed generator is inferred. |
| BLOCK-R024-FINITE-EVALUATOR | Ideal solution SELECTED: convex LOCAL source cones, local GS norm(g)>=3/32, then exact L^-T/L dual-transform orthogonality; normal-map positive-z certificate and guarded direction/BRDF policy | Actual f32 host/source/normalization/GS/transform/filter/evaluator error margins plus Naga/backend/target proof OPEN. An implementation unable to meet the selected constants returns concrete evidence to architect/JC; no guessed epsilon/fallback. |
| BLOCK-R024-ZERO-ROUGHNESS | Architecture DECIDED for this limited profile: source zero retained; all direct/indirect material evaluation uses r_eff=max(r,.045) and selected cancelled Smith algebra | Actual shader/backend r=0/r=.045/near-grazing bounded-light proof OPEN. No ideal-delta/full glTF optical accuracy claim or alternative floor/model admitted. |
| BLOCK-R023-MATERIAL-LIGHT-TRANSFER | Material association and R-025/R-026 conditional group3 interfaces are S1 adopted; material-only group3 remains unbound/unallocated | HAL-370 §§4–5 bind complete immutable input and unlinked flat-only first-version admission. S2 integration, public/currentness and finite radiance proof remain OPEN; linked extended requests refuse until separate exact HAL-354/266A admission. Flat G1 is not widened. |
| BLOCK-R023-QUALITY-OUTPUT | R-027 prerequisite before physically based indoor quality requiring valid radiance>1 is unchanged; exact output extension S1 adopted | S2 authorization, implemented ordered HDR/tone-map/coverage chain and S3/S4 proof OPEN under existing355/263A/266A/295A owners. Limited LDR experiments do not close quality. |
| BLOCK-R023-CONSUMER-SURFACES | Actual source-qualified furniture AND consumer-supplied UV/textured floor/walls required | Consumer/asset owner supplies real content and generated tangent provenance/profile evidence. AABB bounds, palette rendering and engine support do not supply missing content or prove contact/shadows. |
| BLOCK-R023-NATIVE-ACCEPTANCE | Existing C0–C7/D0–D9 stage-aware compatibility/target/no-use and JC native quality gates retained | Authorized implementation owners must execute actual source-to-backend/native evidence and JC review. Draft/math/layout arithmetic cannot close it. |

The local-frame-first proof resolves the former mathematical architecture error, not the GPU numerical proof. The old world-interpolate-then-GS counterexample remains a mandatory oracle (§8.2). R-025–028 are separately scoped S1 binding contracts under the express coordinated decision; no source work, proof closure or successor authorization follows from their existence or adoption.

## 14. Documentary verification and adoption gate

Run through the actual Grip proxy in the admitted Halcyon workspace:

```text
ws halcyon
find SPEC-R-023* under docs/specs
read docs/specs/SPEC-R-023-3D-Visual-Rendering-Extension-Architecture.md max-lines 1000
read docs/specs/SPEC-R-023-3D-Visual-Rendering-Extension-Architecture.md hash
git status
```

Complete all emitted continuations. Compare baseline assertions with their cited source and amended S1 status/dependency clauses with HAL-370's delegated adoption record and HAL-355's narrow acknowledgment. The original creation-unit and later paired-correction receipts below remain historical. Verify exactly the eight adoption-owned document diffs and unchanged technical contracts, source, Constitution, other parents, HEAD and index. No build/test/native job is part of this verification.

Before any later implementation, a separately bounded S2 feature/API/resource/test unit must be authorized under HAL-370 §7. S1 adoption is performed, not proof of source support or permission to execute that unit. JC retains final veto; no review/adoption receipt automatically starts a successor.

Current disposition: **Adopted / Binding [S1] under HAL-370; NOT IMPLEMENTED / S2 NOT AUTHORIZED / NOT IMPLEMENTATION-READY**. Concrete integration and producer/f32/Naga/backend/target/consumer/native evidence remain OPEN in §13.1. No runtime implementation, release/native/G1 acceptance or successor is claimed.

## 15. Historical authoring source bindings and read coverage

All hashes below are complete-file SHA-256, observed on 2026-10-31 against HEAD `17884db5b09862d8013805cefa232c7c71b2f810` plus the explicitly preserved working-tree state. A path with local changes is bound to these bytes, not silently to its HEAD blob. Source lines cited above are relative to these bindings.

| Authority path under `docs/specs/` | SHA-256 |
|---|---|
| SPEC-HAL-002-Constitution-and-Architectural-Invariants.md | `8b1c3f3217ecfffb7b06502dce17e70984887f403afba59d023bacfed56f46db` |
| SPEC-HAL-002A-Constitution-Amendment-Guidelines.md | `f6ab1927a311c5945bc224860d642834d4edde5c3371ead6a0576d8ffb05cc56` |
| SPEC-HAL-355-Native-3D-Rendering-Substrate.md | `edc1bcb236f9ce104d79841ba2649767e0067aa0d1bc6a979b2c3cabbe6ed585` |
| SPEC-HAL-357-Shared-3D-Spatial-Domain-Composition.md | `2288f2c325c6f4a145a694420ea6d583afdce91ad8184a45787ee9a3bccd451c` |
| SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md | `fe115aa0e61225ac8e5e2f3162604f4c36b3072fc7640103c049f4646a357ec4` |
| SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md | `d9cf0098ad19a2338acb562187ecb079cc0051f3aa2c83c5894ccc7f6ede6232` |
| SPEC-HAL-263A-Image-Alpha-Contract-Correction.md | `e30603ba9f3e83d48fc5b288dd8a82a37acf721df79ff15dbde18db647ca1456` |
| SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md | `6d3e0f2041d2af1aab92a0216c37afff481cb6b0012f0469b7ed25b36c288c82` |
| SPEC-HAL-309-Documentation-System.md | `883eabdce15a3b6d145a2de788bcb223a0a86e6c784f7ff8497c93d4ac599fc6` |
| SPEC-R-013-GPU-Shadows-and-Soft-Masking-Pipeline.md | `3503c75b8ab4b3f81458fcb28f14c64f7e7c0beaec31cc2b9a54aa705213abce` |
| SPEC-R-021-RoundedRect-Pipeline-Contract.md | `a64a21edf1caa3b4d951c91697cf85377efccce58f19adc9ccf1a2eae9f6ac82` |
| SPEC-R-022-SDF-RoundedRect-Pipeline.md | `79ce246c0fd9b8598d9c92ad262bf680e8c8a61983d6334fff1519f9e58a1742` |

| Implementation / reference path | SHA-256 |
|---|---|
| `src/three_d/content.rs` | `eb1f4ef4252a1b1cbb6a97262508f7b4f2e816125e965cab27fb08f3053bda59` |
| `src/three_d/appearance.rs` | `243c418472dffa019b3004ec33a3a2ae4c63743a6a42f35c482cd427a07bfc2f` |
| `src/three_d/raster.rs` | `192b891d7d1d399379f6dd1b28cd5a8c58bedac7b121c741499478407bccb25b` |
| `src/three_d/context.rs` | `9cc31da306c5a1beb2ffb599f8df57fd6aa4999cadaf47bc9bacc950647593ef` |
| `src/three_d/viewport.rs` | `39621fff482c963d9555654b2ecee9da1fec6b240de8a56ae036ebb83fd5aa6a` |
| `src/three_d/snapshot.rs` | `1da0202cab6dd2881eb04c36d43fcdfa2df2a900fb9e0eae0cb3e764a4dc0189` |
| `src/core/app.rs` | `3d6a9947001f148e9c3f2a532e50380302e94b84be7b693b69c4462edd385039` |
| `src/core/widget_tree.rs` | `5f4d80dea5cf9842fa7e1967e232f628a9b8710a8f1d43bbbcd6a6048499a710` |
| `src/renderer/custom_render_prepare.rs` | `b8d9128e06758ceb66e64ac9eac66472eae833865b14412c4d83ee85f2b04888` |
| `src/renderer/custom_render_pass.rs` | `93520bad97dd74565a84828bf0087fc578e5323d1d92fb0ac6b20ac47fcb9503` |
| `examples/showcase/tab_viewport_3d.rs` | `1587b6fe7153e2709f3005123cb066efcfd79dab20911d8763b92ef9f21b499e` |
| `tests/app_frame_realization_native.rs` | `5737eaf55cd499bed8def0aa544c44977edfe6dc7f0fea97d90e8bc3d8149a4f` |

The source review qualified actual content descriptors, Context3D members/registration, Viewport3D preparation and its indexed production caller against HAL-355, HAL-309 and the owner clauses cited in this draft. No API is inferred from a filename or example.

## 16. Historical original R-series filename and declared-status inventory

Inventory before this draft: 20 files. The status column quotes the document's declaration; it does not independently establish its implementation/conformance. No existing row is modified or promoted.

| Filename under `docs/specs/` | Declared status |
|---|---|
| SPEC-R-001-Clip-Stack-Rework.md | Implemented (Stages 1-4 Complete) |
| SPEC-R-002-GPU-Clip-Shaders-and-Rounded-Masks.md | Retired — Superseded by SPEC-R-020 (GPU Scissoring & Stencilless Clip Regions) |
| SPEC-R-003-GPU-Path-Rendering-Primer.md | Stage 2A Implemented |
| SPEC-R-004-GPU-Path-Renderer.md | Implemented (Stage 20 Complete) |
| SPEC-R-005-GPU-Distance-Field-Renderer.md | Retired — SDF functions inlined in shadow/gradient shaders per SPEC-R-013-v2. Full SDF pipeline deferred indefinitely. |
| SPEC-R-006-GPU-Vector-Stroke-Engine.md | Retired — Stroke rendering handled by SPEC-R-004 path pipeline tessellation. Dedicated stroke engine deferred indefinitely. |
| SPEC-R-007-GPU-Blend-Engine-v1.md | Retired — Premultiplied alpha blending sufficient for current needs. Advanced blend modes deferred indefinitely. |
| SPEC-R-008-GPU-Composition-Graph-v1.md | Retired — No current consumer. Composition handled by existing scene graph and batcher. |
| SPEC-R-010-GPU-Procedural-Pattern-Fills-v1.md | Retired — No current consumer. Deferred indefinitely. |
| SPEC-R-011-GPU-Blend-Modes-v2.md | Retired — Depends on R-007 which is retired. Advanced blend modes deferred indefinitely. |
| SPEC-R-012-GPU-Gradients-v2.md | Design |
| SPEC-R-013-GPU-Shadows-and-Soft-Masking-Pipeline.md | Design |
| SPEC-R-015-GPU-Border-Effects-v2.md | Design |
| SPEC-R-016-Stroke-Tessellation-v2.md | Deferred — After core visual polish (R-013-v2, R-012-v2, R-015-v2) |
| SPEC-R-017-Advanced-Fill-Tessellation-v2.md | Deferred — After gradient pipeline (R-012-v2) |
| SPEC-R-018-Pattern-and-Gradient-Fill-Engine-v1.md | Deferred — After gradient pipeline (R-012-v2) |
| SPEC-R-019-Nine-Patch-and-Border-Image-Rendering.md | Adopted / Binding (separate implementation declaration: Implemented) |
| SPEC-R-020-GPU-Scissoring-Layered-Rect-Clips-and-Stencilless-Clip-Regions.md | Complete |
| SPEC-R-021-RoundedRect-Pipeline-Contract.md | Active (Binding) |
| SPEC-R-022-SDF-RoundedRect-Pipeline.md | Draft |

There is no standalone R-009 or R-014 file in this inventory. R-012 and R-013 declare their respective older documents superseded; this draft neither recreates nor activates those absent files. At this historical inventory cut, R-023 was the one new inactive artifact and R-024–028 were prospective scopes only.

Historical task-time status note — 2026-10-31, HAL-R027-HDR-COLOR-ALPHA-ARCHITECTURE-DRAFT-01: R-024 and R-027 now exist as separate Draft / Proposed / INACTIVE / NOT BINDING / NOT IMPLEMENTATION-READY files. R-025/R-026/R-028 remain prospective without separate files. This note updates file/status facts only; the historical inventory rows, selected D1–D10, parent laws, authority, implementation and native-proof gates are unchanged.

Historical task-time status — 2026-10-31, HAL-R023-R026-CURRENT-R-SERIES-COHERENCE-CORRECTION-01: R-023–028 now all exist as Draft / Proposed / INACTIVE / NOT BINDING / NOT ADOPTED / NOT IMPLEMENTATION-READY files. This supersedes only the present-inventory reading of the preceding historical R-027 task-time note; no parent authority, source, adoption, implementation or executed-proof stage changes.

## 17. Historical adversarial correction receipt — 2026-10-31

This preserves the original A–F review and then-OPEN decisions. The subsequent architect decision lock/dual-space correction is §18; the current selected design versus remaining proof/adoption gates are R-024 §§7/8.2/8.4 and this §13.1. Historical claims of missing architectural choices are not a competing current rule.

The historical `HAL-R-023-R-024-ADVERSARIAL-CORRECTION-01` record covers the bounded proposed-document correction below. JC retains final decisions/veto. This was not acceptance or engineering authority.

| Historical review field | Source-qualified correction boundary |
|---|---|
| Objective | Correct both inactive drafts for supplied decisions A–F without discarding their sound baseline, channel/default or regular-model clauses. |
| Source qualification | Both original drafts and the affected HAL-355 §§8–17 and HAL-357/263A/266A/295A/354 owner clauses/source were requalified. |
| Status / authority change | Both Draft / Proposed / INACTIVE / NOT BINDING; NONE effective. Neither implementation-ready nor runtime-admitted. |
| Target / beforeimages | Halcyon 0.227.18-beta, master HEAD `312e25c7a6a6d923ff4db26a311cff32fdb717a4`; original R-023 SHA `00ca077084293f59c793f86560709dd57a8d9d558535fbe2193cf36b2792a6f5` (397 lines/58922 bytes), R-024 SHA `23a7f11e4c3dfd750d67406c6df2283a7f1eed79a93797c2602466b2966bb4f2` (424 lines/68540 bytes). |
| Owned files | Only this R-023 and `SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md`; the original R-024 was already untracked. |
| Invariants | Permanent owners/phases/identities; complete immutable source/resource/front association; finite evaluation/resources; exact linear/alpha seam; 2D absence and stage truth. |
| Forbidden | Parent/code/assets/consumer edits, new ABI/coefficients, importer/global state/second renderer, self-adoption, runtime proof and R-025+ successor work. |
| Reuse | Existing typed geometry/normal/UV, leases, Naga, flat snapshot/model, offscreen preparation/publication and Image owners only. |
| External boundary | External-consumer asset/surface/AABB findings are explicitly supervisor-supplied, not new consumer inspection or physical/native proof; Khronos is reference only. |
| Proof owner | Documentary changed-clause/dependency review and full readback; future C0–C7 and R-024 D0–D9 remain unexecuted. |
| Verification | Existing Grip read/hash/status commands, full continuation coverage and before/after protected hashes. No Cargo/build/test/native job. |
| Completeness | Six corrections, explicit unresolved blocks/owners and complete corrected documents; no guessed mathematical solution or callable API. |
| SemVer / release | N/A; manifest, changelog and runtime unchanged. |
| Out of scope | All runtime/parent authority, asset generation, other current work and future feature drafting. |

Halcyon-specific pre-flight: rendering proposal only; no widget policy, interaction or demo changes. Constitution and all HAL-355/263A/266A/295A FORBIDs remain intact. Existing specialized 3D owners are reused; no generic bus or duplicate renderer is admitted. The mandatory runtime-bug/UI/example branches do not trigger for this documentary correction.

Fresh protected authority/source hashes match R-024 §15's listed baseline (excluding the deliberately corrected R-023 receipt hash); R-023 §15 retains its original historical source receipt. Source qualification for this correction includes `content.rs:8–12,59–218,370–447,495–557`, `context.rs:896–980,2085–2180,2338–2435,2600–2655`, `appearance.rs:355–438,1635–1732`, `raster.rs:24–50,667–722`, `snapshot.rs:79–200,411–510`, `viewport.rs:815–970,1250–1372,1442–1528`, `custom_render_prepare.rs:177–253`, `custom_render_pass.rs:518–542`, `app.rs:3267–3275,18952–19008` and the narrower Showcase diffuse shader at `tab_viewport_3d.rs:104–121`. Current semantic member/definition inspection confirms the fixed geometry, single texture, sampler, snapshot/model and checked viewport boundaries.

Disposition: documentary correction delivered for independent review; every §13.1 implementation/quality block remains explicit. No source/backend/native acceptance or successor work follows.

## 18. Historical paired architect decision-lock pre-flight and source receipt — 2026-10-31

HAL-R024-ARCH-DECISIONS-LOCK-01 is completed with the architect's HAL-R024-D3-D5-DUAL-SPACE-CERTIFICATE-CORRECTION-02 replacing ONLY the false D3/D5 world-space premise and its D7 varying/stage dependency. All other D1–D10 choices remain selected as supplied. JC retains final authority/veto.

| Historical review field | Current source-qualified boundary |
|---|---|
| Objective | Concretize the selected named realtime material/numerical/producer/typed-ABI design and corrected local-frame-first ideal proof in the two inactive drafts only. |
| Source qualification | R-023/R-024 were compared against identical current bytes; affected HAL355/263A/266A/295A/354/357 owners and real source seams were requalified. |
| Governing authority/status | Supreme002/002A; named HAL parents binding. Drafts remain Draft / Proposed / INACTIVE / NOT BINDING, not implementation-ready. |
| Authority change | NONE effective; no parent/Constitution adoption, source or implementation authorization. |
| Target/beforeimages | Halcyon0.227.18-beta, master312e25c7a6a6d923ff4db26a311cff32fdb717a4; R-023 SHA32fca8ebf6aaed25f220244fba54c0c6854374a1b6249d20e123f597efde08e5 (448lines/71270bytes), R-024 SHAd94296f49eb4fcb220d54bddcba2edbd1c242d0fa73f10083c068b5c35902904 (460lines/79186bytes), exact matches. |
| Owned files | Only existing R-024 and directly dependent R-023 paragraphs. Pre-existing tracked R-023 modification and untracked R-024 preserved, no staged files. |
| Named invariants | Existing role/color/basis/response/lease/front/absence/proof pairs; baseline ABI, typed complete provenance, linear premultiplied Image, finite resources and phase/sovereignty. |
| Forbidden | Source/asset/consumer/parent changes, legacy ABI/stage change, new GPU runtime/importer/global, hidden numerical repair, f32/native claim from ideal math, self-adoption or R-025 successor. |
| Reuse | Existing Device/Queue/context leases/snapshot/model/target/offscreen publication. Only a separate inactive proposed PBR family/interface is selected. |
| Dependencies | Producer MikkTSpace execution, exact HAL parent adoption/integration, actual f32/Naga/Device proof, R-025/R-026 light transport and R-027 output remain separate. Khronos/Filament/WGSL/wgpu are reference only. |
| Test owner | Future geometry/context/appearance/raster/backend/viewport/internal and external native surfaces; no test/source added or run now. |
| Verification | Grip ws/read full max-lines1000/hash/state git and protected hash comparison; full proof/ABI consistency review. No Cargo/build/native job. |
| Complete result | Exact selected D1–D10 with dual-space amendment, mathematical derivation/counterexample oracle, ABI tables and truthful remaining gates; no executor-selected replacement policy. |
| Version/changelog | N/A, inactive documents only; manifest/runtime unchanged. |
| Out of scope | Implementation/API source, parent adoption, future feature drafts, actual asset generator/loader, full glTF optical support and native acceptance. |

Halcyon pre-flight: (1) governing owners/status as above; (2) widget policy N/A, no widget; (3) no interaction; (4) all Constitution/355/263A/266A/295A FORBIDs preserved; (5) live fixed geometry, Naga, ObjectBlock and target/front seams inspected; (6) no second renderer/generic bus or hidden ownership; (7) no demo/example change.

Exact current baseline seams: `raster.rs:387–401` exposes View80 to vertex/fragment; `:402–416` exposes Object112 to VERTEX ONLY; `:417–456` is the legacy16-byte/one-texture/one-sampler three-group layout; `:24–50` is stride36. `appearance.rs:1811–1862` gives80/112-byte blocks, `:1863–1902,2030–2043` emits the legacy WORLD normal and `:2094–2099` is perspective-center. New PBR local varyings and fragment Object visibility therefore require a separately adopted profile layout; no existing public capability is inferred.

All protected authority/source hashes are freshly rebound to the unchanged baseline listed in R-024 §15 (its old R-023 hash remains historical only); §15 and §17 original receipts are not rewritten as new runtime evidence. Current source reads also cover content descriptors, generational context leases, controlled appearance admission, complete packet/model request and target submit/swap/commit. The exact-math solution and64/52-byte arithmetic are documentary proof ONLY. Full f32, actual Naga/Device/backend and native/JC visual evidence remain unexecuted.

Disposition: paired inactive design correction for independent review only. No implementation readiness, parent adoption, backend conformance or R-025 successor.
