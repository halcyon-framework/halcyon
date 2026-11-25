# SPEC-HAL-393: Dynamic Specular Flow on Analytic Surfaces

**Authoring instruction:** JC, 2026-11-18, author specifications for dynamic reflections on existing Halcyon PBR and 2.5D surfaces.
**Status:** ADOPTED / BINDING [S1 first two-source material-motion design], 2026-11-18. Source implementation and native/visual acceptance are separate. Historical Draft/Inactive labels below are superseded only for the S1 selection in section 8.
**Scope:** Two independently controlled moving specular fields on one existing analytic material, including overlap, visible coalescence, separation and source-dependent color.
**Owners:** HAL-012 appearance, HAL-021 animation Timeline, HAL-354 retained publication, HAL-386/389 analytic optics and HAL-085A shader ABI.
**Companions:** HAL-394 (iridescence) and HAL-395 (HDR/Bloom); neither is activated here.

## Pre-Flight

- Objective: Document a concrete, implementable-first visual outcome and existing-owner boundaries for moving reflected light.
- Standing objective: Two differently colored lights move independently on a surface, cross, visually merge and separate, preserving the actual material and sharp UI.
- Continuation disposition: S0 document only; no engineering task, renderer change or test success claim.
- Governing authority and status: HAL-002 is supreme and HAL-002A permanent; existing named HAL owners retain their adopted/binding scopes. HAL-384 remains inactive.
- Authority change, if any: NONE.
- Target layer and existing state verified: SurfaceAnalytic and GlassSurfaceOptics already accept two checked AnalyticSurfaceLight records each. Their shaders calculate independent elliptical light envelopes, directed specular response, bevel normals and source RGB; no private light-motion owner exists.
- Owned mechanisms and files: This new SPEC only; future execution stays with existing rendering, appearance and Timeline owners.
- Named invariants: INV-393-001 through INV-393-009, and FORBID-393-001 through FORBID-393-006.
- Forbidden behavior: New clock, source-discovering light manager, mutable shader/paint state, arbitrary extra sources, spoofed HDR/Bloom, changed layout, widget ownership or input.
- Existing mechanisms inspected and reuse decision: HAL-021 Timeline plus HAL-354 immutable Update-owned material publication; HAL-386/389 two-source shader. REUSE, not replacement.
- Relevant external or dependent boundaries: Component/application author owns animation intent and light parameters. Material interference is HAL-394; true Bloom is HAL-395.
- Test ownership and placement: Current analytic_surface CPU/Metal renderer tests, retained-value and renderer_z_order tests, and existing glass_material_lab public/native consumer after an admitted task.
- SemVer and changelog impact: N/A, documentary-only.
- Out of scope: Physical light collisions, ray tracing, new 3D material features, spectral evaluation, HDR output and global default skins.

## Halcyon Pre-Flight

- Governing SPEC and read status: HAL-002/002A and the accepted scope owners listed above.
- Widget policy or N/A: N/A; display material, no new interaction.
- Interaction added and policy authority: None.
- Applicable FORBID rules: Sole WidgetTree, read-phase purity, immutable snapshot, no hidden state, HAL-021 single time, HAL-084 graph and HAL-359 strict Z.
- Existing framework mechanism inspected: AnalyticSurfaceLight, SurfaceAnalytic, GlassSurfaceOptics, existing WGSL light math and retained Update.
- Reinvention risk: A new material registry or animation engine would duplicate lawful owners and is excluded.
- Demo/example compliance: Existing lab only after authorization; no independent event or Escape handling.

## 1. Observable feature

At one surface, two independently authored reflected highlights have different hue, position, ellipse radius, rotation, source direction and intensity. They approach, cross, form a visually continuous bright patch under the actual owning material equation, then separate. Their paths can be straight or curved when authored as finite existing Timeline projections.

No source exerts a force on another. "Meeting" and "merging" describe overlap of reflected contributions, not particle collision, feedback, deletion, reparenting or a new transient semantic state. A separate authored source may taper to zero or change track; no executor may infer a physical interaction model from the appearance request.

## 2. Source-grounded first profile

The existing AnalyticSurfaceLight stores unit positive-facing light direction, straight-linear color (alpha 1), strength in [0,1], center in local UV, positive ellipse radii and unit rotation. Current shader functions derive Gaussian-like envelopes and roughness-sensitive specular/Fresnel terms from the actual rounded SDF bevel normal. Two slots on SurfaceAnalytic and two on optional GlassSurfaceOptics are distinct material-response lanes, NOT a pooled four-light scene.

First delivery shall use exactly two existing slots of one selected evaluator. It changes neither the 688-byte current analytic parameter record nor the HAL-385 Frosted optical record. Existing bounded color equations, clipping and halo separation remain authoritative. A dynamic center alone moves envelope brightness; it does not imply a moving physical point light or changing direction.

## 3. Proposed invariants

**INV-393-001 OWNER:** Each light's intended values and lifetime are component/application-authored. Halcyon projects and validates them. Neither OS, compositor nor GPU infers intent, tracks neighbors or creates a global lighting scene.

**INV-393-002 TIME:** HAL-021 owns one engine timestamp/Timeline advance. The owning Widget Update samples current values and publishes a complete immutable pair through HAL-354. A Paint-only sampling path is legal solely under already accepted HAL-021 conditions; renderer/GPU cannot advance or choose time.

**INV-393-003 INDEPENDENCE:** Both slots retain independent center, extent, direction, color, strength and orientation. Zero intensity of one leaves the other and its base material response unchanged. Swapping source order may change only behavior already defined by the particular original material equation, not composite Z.

**INV-393-004 COALESCENCE:** Intersections evaluate both sources per fragment with the existing material's combination rule. No synthetic join/split state is introduced, and additive physical energy is not claimed when the present material applies hue/brightness caps or bounded blends.

**INV-393-005 CHECKED SAMPLES:** Every published frame uses a complete finite source value admitted by its current constructor. Invalid interpolation samples cannot silently clamp, partially replace a pair or mutate last valid published material state. A typed failure is observable at the owning request boundary.

**INV-393-006 MATERIAL CAUSALITY:** Current authored roughness, bevel, Fresnel-like response, source color, face/coating selection and transparency remain operative. A floating colored stripe drawn above an otherwise unaffected material fails the objective.

**INV-393-007 TERMINAL STATE:** Finite playback publishes its exact final optical values, then becomes retained-clean and stops demanding frames. Infinite playback has an explicit lifetime and cancellation owner under HAL-021; unrelated static surfaces remain idle.

**INV-393-008 GEOMETRY/ORDER:** Actual four-corner SDF, inherited clip, target DPR, strict different-Z order, sharp foreground, hit-test/focus/capture and scroll geometry are preserved. Only optical presentation changes.

**INV-393-009 INDEPENDENT EFFECTS:** Iridescence, HAL-388 SurfaceEmission/LocalAmbientGlow, HAL-358 contour, real HAL-385 backdrop and HAL-395 Bloom never activate merely because lights move.

## 4. Prohibitions

- **FORBID-393-001:** No private clock, shader time accumulator, background mutator or second Timeline.
- **FORBID-393-002:** No shared mutable/global light bus, cross-widget reference or implicit source discovery.
- **FORBID-393-003:** No new shader ABI, target, pipeline, render pass or resource registry for the first two-slot result.
- **FORBID-393-004:** No unadmitted photon collisions, physics, virtual particles or hidden join state.
- **FORBID-393-005:** No decorative raster streak reported as material reflection or physical illumination of adjacent elements.
- **FORBID-393-006:** No change to accepted 3D PBR R-024-028, default skins or unrelated text/input/scene semantics.

## 5. Proof and later admission

An ordinary real material-lab scene must show: a still baseline; one moving source; two independent warm/cool sources; approach, crossing, maximal overlap, separation and terminal stillness; and two distinguishable material responses (Smoked face and Frosted coating) over their legitimate backdrops. Mirror a source direction with all other values held constant to reject a fixed painted highlight. Confirm no automatic exterior halo.

Use the existing material CPU reference and 1x/2x GPU tests for sampled finite values, source-zero identity, exact color/roughness/normal equations, inherited rounded clipping, strict Z, unchanged foreground and no idle invalidation. Native temporal evidence must show actual motion and terminal settlement; a collection of static test frames does not establish continuous behavior. JC visual acceptance is separate from numeric or GPU PASS.

More than two simultaneous sources in a single evaluator, a freely varying interior normal map, optical field collision or general animation graph is outside this first profile and requires its own evidence and lawful owner decision.

## 6. Constitutional disposition

The feature only projects immutable presentation data. HAL-002 retains sole WidgetTree, stable IDs, exact phase order, deferred mutations, immutable Scene Build, GPU read-only submission and Halcyon semantic sovereignty. HAL-002A is not amended. This file is S0 draft only.

## 7. Audited technical correction: closed first-consumer candidate (2026-11-18)

**Status: source-selected S1 technical candidate / INACTIVE / NOT BINDING.** JC authorized correction of the audited documents, not a constitutional amendment or automatic engineering/adoption. This section makes the previously underspecified first two-light consumer reproducible. It takes precedence over sections 2 and 5 only when their descriptive statements leave first-profile motion, interpolation, or a visual fixture unspecified; all existing accepted parent SPECs remain supreme. No production code was changed by this S0 correction.

### 7.1 Actual input and update ownership

The first unit is a fixed-geometry `SurfaceAnalytic` with an optional `GlassSurfaceOptics` coating. Its two `AnalyticSurfaceLight` inputs are complete immutable values created through `try_new`; new values are built by the owning Widget in existing `Update` and published through its normal HAL-354 paint invalidation. There is NO Timeline animation of `AnalyticSurfaceLight` itself. HAL-021 currently supports `f32`, `f64`, `Point` and `Color` in its bounded `AnimationValue`, not a whole light struct.

For the first named source, start exactly one `Animation<Point>` for local-UV center from (0.20,0.50) to (0.80,0.50), duration 2.4 seconds, `Easing::Linear`, `Playback::Once`. For the second source, independently start an equal-duration `Animation<Point>` from (0.80,0.50) to (0.20,0.50) at the same admitted frame timestamp; do not emulate independent tracks with a new scheduler. Both Y coordinates are 0.50. At sample parameter p in [0,1], centers are c0=(0.20+0.60*p,0.50) and c1=(0.80-0.60*p,0.50). The per-source radius is (0.20,0.25), rotation=(1,0), direction=(0,0,1), and checked strength=0.40. Colors are linear RGB source0=(1.00,0.25,0.10) and source1=(0.08,0.35,1.00), with alpha1. These values are fixture-only, not framework defaults.

The owning `Update` evaluates BOTH handles against the same HAL-021 frame before constructing the candidate pair. `Active(v)` and `Completed(v)` supply values. `Completed` publishes exactly the terminal endpoints, clears owned handles and ends animation demand. `Cancelled`, `Missing`, `TypeMismatch` or start-capacity failure produce an explicit owner-visible failure and terminate the affected animation demand; they do not use stale partial data as a successful new frame. An already-committed valid material is not mutated by a refused successor. Unmount or material replacement cancels every owned active handle. A successful frame with identical source values remains retained-clean. No second advance, render-time clock, OS wake or hidden self-restart occurs.

### 7.2 Angular reconstruction, not invalid unit-vector lerp

When an authorized light animation also varies azimuth, elevation or ellipse rotation, it uses already admitted `Animation<f32>` scalar tracks, never a new Timeline payload. The authored angle endpoints are FINITE degrees: azimuth and ellipse rotation in [-180,180], elevation in [1,90]. For endpoints a,b in degrees choose the single shortest arc `delta=((b-a+180).rem_euclid(360))-180`; an exact -180 tie becomes +180. At sample p, `angle=a+p*delta`. Elevation is the ordinary linear interpolation `el=a+p*(b-a)` and does not wrap. Compute the unit direction in f64 from `(cos(az)*cos(el),sin(az)*cos(el),sin(el))` and ellipse rotation in f64 from `(cos(rot),sin(rot))`, converting degrees to radians exactly once and then casting components to f32. These are validated by the existing constructor, which normalizes only already near-unit finite inputs. This reconstruction, unlike direct vector lerp, remains in the positive-facing hemisphere and preserves the unit-rotation rule. Black, out-of-domain, invalidly timed or nonfinite author input is refused before publication, never repaired by shader clamps. RGB/strength/center/radius tracks use existing independently admitted scalar/Point/Color interpolation and the same checked complete-pair publication.

The first 2.4-second fixture has static source directions, rotations, radii and colors. Angular-track behavior is an independent optional numeric/CPU proof, not a hidden dependency of the first visible crossing.

### 7.3 Coating fixture and executable numerical oracle

Use the *existing* `GlassSurfaceOptics` equations in HAL-386 section 11.3 and `glass_optics.wgsl`, not an additive light graph. For the diagnostic central flat-face pixel (u,v)=(0.50,0.50), N=V=(0,0,1), both light directions=(0,0,1), so specular P0=P1=1. The coating has `roughness=0.28`, `normal_reflectance=0.50`, `reflection_strength=1`, `rim_strength=0`, `reflection_tint=(1,1,1,1)` and body Q=(0.08,0.08,0.08,1). The existing required bevel parameters remain valid but do not affect a flat-face sample.
Exact source construction for the numerical fixture: use a linear SurfaceGradient from p0=(0,0) to p1=(1,0), stops (0,0.30,1), with each stop straight-linear color=(0.08,0.08,0.08,1). Construct SurfaceAnalytic::try_smoked_glass with opacity1, body bevel_width2, bevel_slope0.5, roughness0.28, ambient_strength0, diffuse_strength0, specular_strength0, max_interior_uplift0, legacy rim None, grain None and emission None. Own rect is 320x160 logical with four corner radii12. Apply GlassSurfaceOptics with bevel_width1.75, bevel_slope0.94, roughness0.28, normal_reflectance0.50, reflection_strength1, rim_strength0, rim_falloff1.5, white reflection and rim tints, and the section7.1 light pair. The test's central pixel must be inside the flat face with no bevel influence; all values are checked through current constructors. This fixture does not change existing lab defaults.

For one source at center offset dx, its unnormalized field energy is `e=0.40*exp(-0.5*(dx/0.20)^2)`. Let `A=e0+e1`, `W=0.50*min(A,1)`, and `M=(e0*C0+e1*C1)/A` for A>0. The original coating result is `Q1.rgb=(1-W)*Q.rgb+W*M`, alpha1. At p=0, both centers are 0.30 away from the pixel and `Q1.rgb` is approximately (0.139736,0.108569,0.141035); at p=0.5, both centers exactly meet at that pixel and Q1.rgb=(0.264000,0.168000,0.268000); at p=1 the central pixel returns to the p=0 result. These are reference calculations, not observed GPU pixels. Distinct colored peaks exist at left/right when separated; the merged area is one geometric overlap with a mixed hue, not a collision state. These numbers expose failures to interpolate, preserve independent colors, or apply the current coating response.

**Acceptance never claims that every material or every valid light strength visibly brightens on crossing.** HAL-386's current face-cap and coating `min(A,1)` can saturate energy; the specified fixture is intentionally below that cap. A consumer that cannot reproduce the fixture must report a real visual/numeric failure, not call a static RGB stripe equivalent.

### 7.4 Exact execution boundaries and coverage

The current public constructors, group0 Globals, Pipeline13/14, two light records, actual clip/AA and existing group1 layouts are unchanged. The material's Update invalidation is frame-causal and does not create RenderGraph work beyond the already admitted material paths. `glass_material_lab` retains its five fixed reference profiles, six clipping cells, current background Timeline, old Slider/FormBinder controls and strict Z. The new crossing comparison occupies an additional eligible ordinary material comparison under current lab ownership; it cannot overwrite baseline fixtures. Source directions, strength, color, scale and geometry are authored values; only the two centers move during the first scenario.

Focused future qualification must prove `glass_material_lab` start/active/completed/cancelled/invalid sampled cases, the complete first-frame/terminal publication, exact values at p=0,0.25,0.5,0.75,1, the unit-circle and antipodal-angle tie cases, and frame-rate invariance at identical engine times. The current CPU coating reference and real 1x/2x GPU readbacks must match the selected profile within the existing owner-declared output tolerance. Source-level invariants and a few static frame snapshots are not proof of native real-time playback; an ordinary native app must show the actual approach/cross/retreat, sharp captions, valid clips and a true idle endpoint. No unrelated broad suite or duplicated optical fixture is requested.

### 7.5 Status, compatibility and commands

The selected two-source motion is a bounded existing-API candidate. Future S1 authorization may adopt it only with an explicit owner decision and an executable task; the current document is NOT such adoption. Further fields, more than two sources within one evaluator, HDR and color interference have separate proof and authority boundaries. No change to HAL-002, HAL-002A, HAL-021, HAL-085A or existing 3D R-series is selected.

For this document-only correction the exact reproducible Grip commands, after `ws halcyon`, are:

```text
read docs/specs/SPEC-HAL-393-Dynamic-Specular-Flow-on-Analytic-Surfaces.md 1 5000
state capsule
```

For separately authorized later implementation the narrowly scoped existing test surface is `verify package halcyon test glass_material_lab check timeout 600` plus `verify package halcyon test analytic_surface timeout 600`. A native result is separately required, not inferred from these commands.
## 8. Binding S1 technical adoption — 2026-11-18

**Decision:** ADOPTED / BINDING for the complete source-verified first two-source SurfaceAnalytic/GlassSurfaceOptics motion profile of section 7. JC expressly delegated technical S1 ratification on 2026-11-18; JC retains product/visual judgment, constitutional authority and veto. The earlier S0 and section 7 candidate status lines are historical, not the present status of this exact first profile.

Adopt exactly the two 2.4-second HAL-021 Animation<Point> source-center tracks, independent static colors/directions, unit-safe angular reconstruction when authored, checked complete material publication at normal Update, exact terminal and cancellation semantics, no new clock, GPU/material manager or ABI. The current HAL-386/389 face and coating equations are unchanged. First comparison and numerical cases in section 7 are the accepted implementation target, not a claim of live evidence. HAL-012/021/354/359 retain their established ownership. More than two lights in one evaluator, physical light interactions, automatic film or Bloom, and 3D PBR changes are not admitted. Implementation, Metal/native and JC appearance acceptance remain OPEN.

## 9. S1 execution evidence — 2026-11-18

The existing `glass_material_lab` mounts the additional Crossing reflections comparison. It starts the two finite Timeline tracks together and publishes one checked coating on the existing analytic face from Widget Update. The six reference cells, their background tracks, the original action sample and the editing controls remain available.

The focused `glass_material_lab_crossing` verification passed all three test cases, including the public and internal inclusions of the mounted Timeline regression and the Metal readback. The five requested phases matched the checked source positions. The tests qualified cancellation, missing and mismatched handles, invalid position refusal, mount retirement and the settled endpoint. At 1× and 2× the rendered samples agreed with the coating reference within two color codes. The package check and the offline, locked example build passed.

One ordinary macOS example run produced eleven original 2× window captures in `hal393-native-tcbumgqq`. The initially warm-left and cool-right reflections converged into one bright mixed center, crossed and separated in reverse order. The caption stayed sharp. The last three sampled face interiors matched byte for byte after completion. JC's final appearance judgment remains open.
