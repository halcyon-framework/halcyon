# SPEC-HAL-394: Iridescent 2.5D Surface Response

**Authoring instruction:** JC, 2026-11-18: include color-shifting reflection tied to the light and surface in the existing Halcyon material exploration.
**Status:** ADOPTED / BINDING [S1 first thin-film RGB-surrogate optical design], 2026-11-18. Section 10 records implementation and verification. Historical Draft/Inactive labels are superseded in the exact section 9 scope.
**Scope:** Optional angle-dependent material hue response on the current analytic UI face/coating, not an automatic full 3D PBR extension.
**Owners:** HAL-012 appearance, HAL-085A shader ABI, HAL-386/389 material optics; HAL-393 motion and HAL-395 HDR/Bloom remain independent.

## Pre-Flight

- Objective: Specify what an actual iridescent material must do, distinguish it from decorative color animation and record the precise technical admission still missing.
- Standing objective: Reflected color shifts reproducibly with the optical angle and surface parameters while independently colored moving highlights remain attributable to their sources.
- Continuation disposition: Inactive documentary result only; no shader development is authorized.
- Governing authority and status: HAL-002/002A supreme. The existing adopted 2.5D material scopes remain binding. HAL-384 and R-029 are inactive and grant nothing here.
- Authority change, if any: NONE.
- Target layer and existing state verified: The current material uses two checked RGB sources, Fresnel-like roughness response, synthetic SDF bevel normals and a fixed screen-facing view; it has no source-qualified thin-film spectral evaluator.
- Owned mechanisms and files: This new draft only; any future optical math stays in the existing material/shader owner.
- Named invariants: INV-394-001 through INV-394-008 and FORBID-394-001 through FORBID-394-006.
- Forbidden behavior: Uncaused timed rainbows, fake environment reflection, spectral-accuracy claim from RGB input, hidden global state or silent 3D material migration.
- Existing mechanisms inspected and reuse decision: SurfaceAnalytic, GlassSurfaceOptics and existing HAL-386/389 shading are reused, not replaced.
- Relevant external or dependent boundaries: Application authors material selection. Source wavelengths/illuminant would require new explicit evidence. HAL-395 is separate output policy.
- Test ownership and placement: Existing analytic pipeline CPU oracle, current renderer GPU tests and real glass_material_lab after an admitted task.
- SemVer and changelog impact: N/A for S0 documentation.
- Out of scope: True spectral scene transport, caustics, ray tracing, arbitrary 3D clearcoat, automatically animated skins or HDR display.

## Halcyon Pre-Flight

- Governing SPEC and read status: HAL-002/002A, HAL-012/085A and HAL-386/389 in their actual adopted scopes.
- Widget policy or N/A: N/A; material shading only.
- Interaction added and policy authority: None.
- Applicable FORBID rules: HAL-002 retained/phase/identity law, HAL-012 no semantic authority, HAL-359 order and HAL-085A binding ABI.
- Existing framework mechanism inspected: Existing checked two-light glass coatings, screen-facing view and current bevel-normal evaluation.
- Reinvention risk: A separate rainbow-overlay pipeline would bypass material ownership; forbidden.
- Demo/example compliance: The existing glass material lab remains the only proposed first native comparison, without new widget semantics.

## 1. Observable meaning

Iridescence is a material response whose hue depends on light incidence, view direction, surface optical properties or a declared local film parameter. An animated RGB source moving over a flat surface changes brightness/location; it does not automatically prove interference. A per-frame rainbow gradient uncorrelated with these inputs is not iridescence.

The first consumer is an optically authored existing GlassSurfaceOptics coating or SurfaceAnalytic front. Absent iridescence is exactly the current material. The selected material is optional instance appearance, not a hidden runtime capability switch.

## 2. Geometry and material boundary

The present view basis is effectively V=(0,0,1). Most of a flat analytic face has N=(0,0,1); a curved bevel supplies changing N. Consequently, moving an ellipse center alone cannot guarantee angle-dependent color variation. Changing the checked light direction, the real local bevel normal or a separately admitted spatial film property can cause a valid optical shift. A general interior normal field, per-pixel thickness map or camera parallax is not already supported.

The two independent light colors remain source facts. A material interference response must compose with, not overwrite, the original roughness/Fresnel/rim and source colors. Neither this S0 nor HAL-393 authorizes adding a new shader group, texture, light slot or 3D PBR family.

## 3. Prospective invariants

**INV-394-001 ANGULAR CAUSALITY:** Hue variation must correspond to a changed checked optical angle or material optical value. Screen position or time alone cannot be its hidden cause.

**INV-394-002 MATERIAL-LOCAL OWNER:** All optical parameters are immutable, finite, typed material values under HAL-012/354. The shader is a pure evaluator, not a source/animation authority.

**INV-394-003 LEGAL GEOMETRY:** Use the actual admitted SDF normals and view/light basis. Do not fabricate depth, thickness, per-pixel camera rays or external environment samples.

**INV-394-004 ZERO IDENTITY:** Absent/zero iridescence produces the exact previous material, alpha, clipping, retention and output transfer.

**INV-394-005 SOURCE COLOR:** Source RGB affects reflected hue under one declared response law. A colored-source change cannot silently become an unrelated global hue animation.

**INV-394-006 INDEPENDENCE:** Optical hue is not emission, Bloom, Frosted blur, grain, halo or LuminousContour; none turns on implicitly.

**INV-394-007 VALID OUTPUT:** The future selected RGB/material and alpha result is finite and follows existing straight-linear/premultiplied and gamut requirements. Invalid numerical output is refused, never masked by per-channel saturation.

**INV-394-008 RETAINED:** Unchanged material and illumination cause no idle animation. Any HAL-393 movement is sampled only under its existing Timeline owner and yields stable frame-causal presentation.

## 4. Exact unresolved optical decision

Current lights are RGB, not an illuminant spectral power distribution. Physically faithful thin-film interference would need source spectra, refractive-index models, optical thickness, angular Fresnel phase, sampling/integration and a declared mapping into Halcyon's linear working primaries. Those inputs and computations are NOT currently established.

Two technically different models remain unselected in S0: a bounded *thin-film-inspired RGB approximation* explicitly not claiming spectral accuracy, or a source-qualified spectral model with a defined illuminant/sampling proof. A subsequent S1 decision must select one, identify exact fields, ranges, angle convention, reference equation, energy/color bounds, shader ABI, CPU reference and real backend cost before any implementation task. An executor may not invent a fit, thin-film thickness, IOR, spectral table or public constructor.

## 5. Prohibitions

- **FORBID-394-001:** No timed rainbow wheel, color-cycling noise or static rainbow texture pretending to be material interference.
- **FORBID-394-002:** No claimed spectral accuracy from unspecified RGB light data.
- **FORBID-394-003:** No new global camera/light/normal authority, GPU time loop or private material manager.
- **FORBID-394-004:** No silent change to existing roughness, Fresnel, material alpha or non-iridescent defaults.
- **FORBID-394-005:** No new 3D R-024/029 variant by analogy with this UI proposal.
- **FORBID-394-006:** No implementation/adoption inferred from this draft.

## 6. Proof before activation

One ordinary native glass comparison must show fixed light/angle equals fixed output, controlled incidence change causes repeatable hue shift, different authored material parameters yield different response, original source hue remains visible, and zero response equals the current surface. Test the actual bevel and a fixed interior separately: do not invent interior angular variation where both N and V are fixed.

Independent CPU reference, 1x/2x Metal readback, near-black/brighter source cases, mirrored direction, rounded clipping, alpha, strict Z and foreground clarity are necessary. HAL-393 source crossing and final stillness are separate temporal proof. JC visual acceptance is independent. S0 grants no shader, material API or 3D change.

## 7. Constitution

HAL-002 sole owner, ID relations, immutable scene and pure read-only GPU evaluation remain intact. HAL-002A does not require amendment. This specification remains inactive until a lawful optical model is selected and adopted.

## 8. Audited first-profile optical decision (2026-11-18)

**Status: S1 technical candidate / INACTIVE / NOT BINDING.** This section resolves section 4's previously open evaluator choice for the FIRST 2.5D glass coating, while preserving its historical S0 record. It is a precise candidate decision by the architectural author under JC's corrective mandate, NOT adoption or execution permission. HAL-002/002A and the separate existing HAL-386/389 material/085A shader contracts take precedence until coordinated acceptance.

### 8.1 One selected model and honest name

The selected first evaluator is **Normalized Three-Wavelength Thin-Film RGB Surrogate V1**, not a spectral renderer. It evaluates the lossless two-interface, s-polarized thin-film *intensity reflectance* at three fixed surrogate wavelengths. Those three values modulate the existing source RGB channels without pretending to integrate a real illumination spectrum or claiming physical colorimetric fidelity. The fixed samples are lambda_R=650 nm, lambda_G=530 nm and lambda_B=460 nm. RGB is otherwise Halcyon's existing straight-linear sRGB/Rec.709 color domain. No texture, lookup asset, normal map, global environment or new light field is admitted by this candidate.

The explicit immutable optional coating parameters are:
- `strength` f32 in [0,1], zero canonicalizes to no film;
- `thickness_nm` f32 in [100,900];
- `film_ior` f32 in [1.2,1.7];
- fixed incident-medium index n0=1.0 and fixed substrate index ns=1.8 (not material runtime state).

A proposed `ThinFilmParams::try_new(strength,thickness_nm,film_ior)` returns typed NonFiniteInput or InvalidParameters; `GlassSurfaceOptics::try_with_thin_film(self,Option<ThinFilmParams>)` retains immutable source values and zero-as-absence. Existing `GlassSurfaceOptics::try_new` and all old constructed outputs remain unchanged. Only explicitly authored coating instances use the film. Do not extend the separately certified 3D R-024/029 families or legacy SurfaceRimLight.

### 8.2 Exact angular calculation and optical factors

Use the already checked light direction L and V=(0,0,1). H=normalize(L+V) is the local specular microfacet half-vector already used by the shared glass coating evaluator. Its cosine with view `ci=clamp(dot(V,H),0,1)` is the defined film incidence angle; the geometric bevel normal N still controls the ordinary specular lobe P=pow(max(dot(N,H),0),q). For the admitted light hemisphere H is never zero. This choice allows a flat face with fixed N and V to change its reflected hue when the AUTHOR varies L; moving only the ellipse center does not change the film angle. No invented camera/parallax/normal field is implied.

For n=n_film, ns=1.8 and `sin2=1-ci*ci`:
```text
cf = sqrt(1 - sin2/(n*n))
cs = sqrt(1 - sin2/(ns*ns))
r01 = (ci - n*cf)/(ci + n*cf)
r12 = (n*cf - ns*cs)/(n*cf + ns*cs)
phi(lambda) = 4*pi*n*thickness_nm*cf/lambda
R(lambda) =
    (r01*r01 + r12*r12 + 2*r01*r12*cos(phi(lambda))) /
    (1 + (r01*r12)*(r01*r12) + 2*r01*r12*cos(phi(lambda)))
```

The source domain makes all denominators positive, every radicand positive, and ideal R(lambda) in [0,1]. CPU reference uses f64; WGSL evaluates f32 with bounded ordinary rounding. An observed backend value outside [-0.00001,1.00001] or nonfinite is a failed backend/profile qualification, not an excuse for a silently different algorithm. Within that explicit numeric error envelope, rounding to [0,1] is ONLY representation correction. A zero R_max has the exact all-zero modifier.

Let `R3=(R(650),R(530),R(460))` and `Rmax=max(R3)`. For Rmax>0 set `hue=R3/Rmax`, otherwise hue=(0,0,0). Define the actual normalized film factor `M=(1-strength)*vec3(1)+strength*hue`. The original checked source's RGB `C_i` is replaced ONLY inside the shared glass coating's two specular/rim reflection terms by `Cfilm_i=C_i*M_i`; the original L, envelope E, alpha1, P, rim angular shape, roughness, existing body and emission are unchanged. The original `peak_i=max(C_i)` calculation in `glass_optics.wgsl` is performed on `Cfilm_i` for this coating, and the exact existing bounded A/R, normalized hue and convex `wf/wr` equations remain in force. Thus each modulated RGB component is in [0,1], no energy is added above the corresponding original source channel and existing premultiplied output remains bounded. `strength=0` MUST bypass all new work and reproduce the exact unchanged original `C_i` and output, not a recomputed approximate equivalent.

This normalized three-line RGB surrogate intentionally trades physical spectral brightness for visible reproducible hue differentiation. It is NOT the R-024 BRDF, no Fresnel energy-conservation proof is claimed and no spectral color accuracy follows from its three sample wavelengths.

### 8.3 Precise ABI proposal, with lawful adoption required

The existing `GlassSurfaceOpticsParams` remains exactly 192 bytes with no undocumented use of its reserved lanes. The existing `AnalyticSurfaceParams` 688-byte and `FrostedGlassParams` 1344-byte prefixes remain byte/field/meaning identical. The first proposed new format appends ONE 16-byte `film` vec4<f32> at the end of EACH existing per-material record: `(strength,thickness_nm,film_ior,0)`. The new totals are respectively 704 and 1360 bytes; dynamic slot strides continue to be calculated from the actual device uniform alignment. Zero/absent selection uses the old exact material branch and new trailing zeros. No new group, binding, shader stage, render pipeline, texture, target, Image, pass or SourceId. The shared pure `glass_optics.wgsl` function consumes the new `film` argument from either caller, avoiding different optical algorithms for Smoked and Frosted.

These totals are **candidate versioned ABI changes**, not permission to modify an already Binding HAL-085A/HAL-386/HAL-385 record. Exact coordinated amendments to those owning SPECs and their ABI guard tests MUST be adopted before code preparation; historical uniform layouts stay binding until then. This is the only new optical data in the first profile. HAL-395's independent HDR proposal is NOT silently packed in the same record.

### 8.4 Reproducible numeric and visual oracle

For n=1.4, ns=1.8, thickness=320 nm, white light, strength=1, and L with elevation5 degrees, the three CPU reference reflectances are approximately (0.10326,0.01061,0.06586). At elevation80 degrees the references are approximately (0.01436,0.02811,0.08006). The normalized hue at the first incidence is approximately (1.000,0.103,0.638), at the second (0.179,0.351,1.000). Azimuth does not change these values for a flat surface at otherwise equal elevation, because H.z is invariant under azimuth rotation; it DOES change which beveled edge is lit through dot(N,H).

For the native visible-angle proof, use an independently authored coating fixture with roughness1 (the current exponent is then4), reflection_strength1, normal_reflectance0.50, rim_strength0, neutral-white incident RGB, film strength1/thickness320nm/film_ior1.4 and the same dark neutral material face. At elevation5 degrees its flat-face half-vector cosine is approximately0.737 and its specular P approximately0.295; the test must not reuse the unrelated old roughness0.28 lab preset, whose much narrower lobe would make the grazing comparison visually negligible. Elevation80 degrees has a near-normal, stronger lobe; compare normalized film hue independently of changing lobe intensity. These are optical test-only values, never framework defaults.

A controlled source-direction change must alter the hue without changing RGB input; an isolated center/position change MUST NOT alter the per-light film hue. Changing d or n must change the reference curve. For a zero-strength film, black source, zero source strength and absent film, baseline equality is mandatory. Test the actual (0,0,1) interior N and the original bevel normal with two source directions separately.

Backend proof: (a) f64 CPU reference at named sample wavelengths and incidence endpoints; (b) actual f32 WGSL/Metal 1x/2x comparison against the selected CPU oracle within an explicitly measured numeric error envelope and current encoded-output tolerance; (c) ABI/packed offsets/zero-branch invariants; (d) Smoked and Frosted source parity and old five fixed lab profiles unchanged; (e) native material lab with independently authored film parameters, moving/rotating real light and sharp foreground; (f) no extra idle work and no unadmitted shader/color transfer. Report source, GPU pixels and JC aesthetic judgment as separate facts.

### 8.5 Complete S0 correction and exact checks

This S1 candidate closes the first evaluator decision in section 4 without introducing the unbounded spectral engine or an alternative implementation branch. If actual backend or parent ABI admission fails, the candidate remains inactive; an executor cannot substitute rainbow noise or a second pipeline. It creates no implicit HAL-002 amendment, no R-024/029 capability and no display HDR.

Exact Grip read-only documentation checks after `ws halcyon`:

```text
read docs/specs/SPEC-HAL-394-Iridescent-2p5D-Surface-Response.md 1 5000
state capsule
```

A later separately adopted implementation would qualify the existing `verify package halcyon test analytic_surface check timeout 600` and `verify package halcyon test glass_material_lab timeout 600` surfaces plus the original ordinary native material lab. This document-only correction does not run or claim those checks.
## 9. Binding S1 technical adoption — 2026-11-18

**Decision:** ADOPTED / BINDING for the selected Normalized Three-Wavelength Thin-Film RGB Surrogate V1 of section 8, with the exact 650/530/460 nm sample law, optical input domains, angular half-vector convention, strength-zero identity, source-RGB modulation and test oracles. JC delegated the technical decision on 2026-11-18, retaining product/visual, constitutional and veto authority. Earlier S0 unresolved alternatives and section 8 candidate labels are historical for this selected first profile.

Scoped adoption includes the immutable optional ThinFilmParams on the current GlassSurfaceOptics coating, with the exact coordinated HAL-085A/HAL-385/HAL-386 versioned material-record additions 688->704 and 1344->1360 bytes. Current old material prefixes/constructors and non-film rendering remain unchanged. A three-sample RGB surrogate is explicitly NOT a spectral-radiometric or colorimetrically accurate simulation. The existing R-024/029 3D PBR family, additional maps/normal fields and HDR-display output remain outside this admission. Shader/source implementation, reference comparison, native performance and JC appearance acceptance remain OPEN.

## 10. S1 implementation evidence

Halcyon 0.259.0-beta exposes `ThinFilmParams::try_new` and `GlassSurfaceOptics::try_with_thin_film` in `core::paint`. Smoked and Frosted evaluate the same shader function. Rust packing and WGSL layout tests confirm the original prefixes and the 704/1360-byte records. Zero strength packs the same bytes as absence and produces identical pixels.

The focused analytic, Frosted and Glass Material Lab tests passed. The two f64 reference endpoints matched section 8.4. Metal reflectance readback covered 45 incidence, thickness and index combinations within 0.00001 of the f64 result. Smoked and Frosted center and bevel samples at 1× and 2× matched within two encoded color codes. All 12 saved legacy material outputs matched byte for byte. The package check and offline, locked example build passed.

The ordinary macOS lab displayed a mauve reflection at 5° and a blue reflection at 80° under the same white source color. The native Swap angles control exchanged their elevations and the measured center RGB values exchanged exactly. The 2× captures and pixel observations are in `hal394-native-rmvv7u7n`. JC's appearance approval remains open.
