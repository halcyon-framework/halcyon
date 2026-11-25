# SPEC-HAL-378: Source-Qualified PBR Material Crossfade

**Author:** JC, with product intent recorded through delegated technical review, 2026-11-05.
**Status:** DRAFT / INACTIVE / NOT BINDING.
**Technical authority:** None newly adopted. No source S2, shader change, public API, implicit animation policy or native capability is authorized by this document.
**Implementation:** This exact capability is unimplemented/unverified; related existing features have only their respective scope.
**Owner:** Halcyon original PBR source/material and target/HDR presentation owners; caller only requests qualified visual transition
**Supreme law:** HAL002 Constitution, HAL002A amendment procedure, adopted HAL309 documentation law.
**Related sources:** HAL021, HAL355/370/372, R024/R025/R027/R028 and HAL359 as separately applicable; HAL363/364 remain Proposed

## Historical contract scope

- Objective: record the product-visible effect, exact separate ownership, existing source constraints and S1 decisions that remain open.
- Standing objective: source-qualified, native and responsive application presentation through Halcyon; not a static viewer, screenshot trick or infrastructure-for-infrastructure's-sake.
- Review authority: JC owns product goal and native visual acceptance; the delegated architectural reviewer makes explicit technical selections and independently verifies; implementation of selected S1 requires separate authorization.
- Adoption and implementation: draft is not a runtime authority; select and adopt exact technical S1 with code/owner evidence before issuing an implementation task. Existing HAL372 S2 continues independently.
- Governing and status: no change to HAL002, HAL002A, HAL021, HAL355, HAL357, HAL372, HAL068/069 or any Draft/Proposed text. Those contracts retain their actual stated statuses.
- Authority change: NONE. No implicit adoption, new renderer/consumer behavior or additional Scene writer.
- Existing implementation: R024 and later R-series own exact source material roles, normal/texture validation, reverse-Z, R027 FP16 and HDR target. HAL372 v1 explicitly excludes Material Crossfade. Existing PBR blend=None and Mask semantics prevent claiming that fractional visibility is a supported material transition. HAL355 rejected generic fractional translucent objects in its present binding scope.
- Owned scope: only this new Markdown file; no code, existing SPEC, assets, tests or foreign working changes.
- Invariants and prohibited cases: sections 3 and 4. Reuse existing source/target, Update, Timeline, GPU, text and layout owners rather than creating any competing machinery.
- Dependent boundary: The application owns real product evidence, physical meaning, user configuration and authoritative semantic updates; HAL owns appearance, projection, frame timing, routing, target and renderer mechanics.
- Verification now: file content, classification, source/authority separation and intact reference names. This is a document-only check, not Cargo/native proof.
- Test owner after adoption: actual nearest source/backend/current native App test paths, selected after S1 source inspection.
- Completion boundary: user-facing goal and safety/ownership intent with explicit unresolved technical design, not a prematurely executable implementation plan.
- Version: N/A, documentation-only inactive Draft with no public behavior effect.
- External/destructive effects: NONE.
- Out of scope: translucent material engine, blending arbitrary unrelated scenes, editing manufacturer material declarations, HAL372 entry/reveal and new source decoder

## 1. Intended user-visible outcome

Where two distinct fully qualified actual visual material presentations are available for the same object or a valid source successor, show a smooth, visually coherent bounded transition between the two without changing or disguising original manufacturer materials, normal/roughness/metallic roles or render depth. The effect is a temporary representation of two source-qualified states, never an invented 'new physical material'.

## 2. Verified existing boundary

R024 and later R-series own exact source material roles, normal/texture validation, reverse-Z, R027 FP16 and HDR target. HAL372 v1 explicitly excludes Material Crossfade. Existing PBR blend=None and Mask semantics prevent claiming that fractional visibility is a supported material transition. HAL355 rejected generic fractional translucent objects in its present binding scope.

An existing animation or render program does not imply this source-qualified effect is supported. Do not reclassify drafted generalized presentation mechanisms or technical proposals as active simply to fill a gap.

## 3. Owner-correct data flow and required invariants

The application explicitly supplies an accepted product material choice/current and successor identities; HAL validates both real admitted PBR sources and exact material/texture/normal dependencies. A temporary current-target visual occurrence may display a blend of their separately qualified radiance under a future selected color/depth/coverage contract. Semantic choice is accepted by the domain owner and is never controlled by a shader clock or GPU pixel readback.

INV-378-SOURCE: source A and B independently admitted and immutable; no ghost material. INV-378-COLOR: linear HDR/FP16 and tone ownership retained. INV-378-NORMAL: original normal mapping and R028 UV derivative obligations not bypassed. INV-378-DEPTH: near/far and Mask coverage stay truthful. INV-378-TIME: HAL021 engine time only. INV-378-TARGET: complete valid old/current fronts or explicit refusal. INV-378-RESOURCES: finite residency/overlap accounting.

Unknown data, unqualified motion, unowned presentation transitions and failures must remain explicit. A declined effect must not invalidate a fully admitted ordinary source or silently redraw a stale previous scene.

## 4. Forbidden substitutions

FORBID-378-OPAQUE-AS-ALPHA-HACK, FORBID-378-POST-IMAGE-FADE, FORBID-378-UNQUALIFIED-TRANSLUCENCY, FORBID-378-BYPASS-PBR-NORMAL, FORBID-378-HIDDEN-MATERIAL-WRITE, FORBID-378-UNBOUNDED-DOUBLE-RESIDENCY, FORBID-378-SEMANTIC-SHADER-AUTHORITY.

No second WidgetTree, renderer, frame clock, global effects registry, independently driven scheduler, shader with unverified source math, speculative assembly engine, or OS-controlled semantic operation. Input and visual presentation are separate from domain changes.

## 5. OPEN technical decision register: S1 cannot be inferred

Decide which material transitions are lawful under existing R024 source roles and HAL355 excluded transparency, exact two-source geometry/identity matching and changed-topology refusal, premultiplied/linear color and HDR/tone composition, depth and Mask/alpha-to-coverage/shadow policy, shader or same-target dual-source execution without alternate renderer, N1/N4 sample correctness, resource overlap/budget, retarget and cancellation, reduced-motion/no-effect, stage error precedence and visible conformance reference. If such an effect genuinely requires a scoped amendment, propose it to the lawful owner rather than implying current permission.

The delegated architectural reviewer must make and record these exact choices after inspecting current owner source. Implementation cannot decide between alternatives. The unresolved design is not an excuse to mark the effect implemented or invent missing behavior.

## 6. Required causal proof after a separately adopted S1

Actual original PBR textured samples with different qualified material choices, differing normals/roughness/emission, source identities, occlusion, Mask geometry and shadow casters; verify original endpoints, intermediate linear radiance and tone, N1/N4, different-source refusal, target loss, camera movement and physical native appearance. Also verify unchanged original PBR no-effect route.

Actual backend pixels, native App/Screen effects, physical OS input and JC visual acceptance require their own demonstrated evidence, not a headless proxy or a successful render submission.

## 7. Status and adjacent contracts

Not interchangeable with HAL372 binary Object Reveal or generic 2D transition; original PBR/Mask/shadow/toning laws prevail. A Draft cannot adopt a previously excluded HAL355 transparency feature.

**DRAFT / INACTIVE / NOT BINDING**, 2026-11-05. No executable S2 work begins from this document alone. A later S1 must explicitly resolve all relevant behavior and compatibility with current binding owners, within constitutional limits, before technical implementation.
