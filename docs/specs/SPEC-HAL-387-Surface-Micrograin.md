# SPEC-HAL-387: Surface Micrograin

**Status:** ADOPTED / BINDING for the first material-local micrograin profile in section 7. Earlier candidate clauses remain only where consistent with section 7. Implementation and JC optical acceptance are separate.
**Date:** 2026-11-13.
**Owner:** Halcyon renderer/color transfer and existing appearance values.
**Confirmed JC direction:** Grain is independently meaningful and must not be inseparably bundled with Frosted Glass, Smoked Glass or glow.
**Governing owners:** HAL-002/002A, HAL-012, HAL-024, HAL-085A, HAL-354, HAL-359, HAL-085A section 6.8 and PM-026.
**First user result:** A stationary, fine-grained real material texture on glass that is neither flickering nor the previously rejected repeating dark-gradient dither.

## 1. Purpose and optical identity

Surface Micrograin describes small-scale visual material irregularity, such as matte etching or fine glass scatter. It is not a noise-based concealment of a broken gradient, post-process film grain across the entire screen, a third kind of glass or a new anti-banding solution. It is an explicit optical decoration supplied only to a component that authors it.

It is optional on independently adopted Frosted Glass (HAL-385) and Smoked Glass (HAL-386); it may be applied to another verified surface material with its own admitted integration. Those material definitions do not require grain. Text and pictograms must remain legible and free of inherited grain.

## 2. Appearance and sampling

**INV-387-001 EXPLICIT TARGETED INPUT:** A validated value carries immutable material-local seed, physical grain scale (candidate 0.75..=3.0 target pixels), signed pattern amplitude and local anchoring. The first admissible amplitude is 0..=2.0/255.0 *of SDR sRGB-encoded luminance*, never linear RGB or a target-independent '8-bit step'. The first target profile explicitly requires a known SDR sRGB output transfer (Rec.709 linear luminance weights and IEC sRGB encode/decode); HDR, PQ, HLG, wide-gamut or unknown output transfers are unsupported until separately qualified. No assumption that every macOS/native target is sRGB is allowed. These candidate bounds are not adopted product defaults.

**INV-387-002 STABLE MATERIAL COORDINATES:** Grain is attached to current material-local coordinates and its validated seed. It does not redraw randomly as frames advance, depend on wall time or jump because unrelated siblings repaint. A moving panel carries its grain with it; a fixed panel remains stable. DPR conversion and fractional positioning must not make the pattern swim or flicker.

**INV-387-003 SPATIAL CHARACTER:** The source pattern has bounded mean, finite variance, no persistent horizontal/vertical bands, no visually dominant tiling harmonic and no accidental checkerboard at 1x/2x. A deterministic hash is not itself a spatial-quality proof. A candidate blue-noise pattern, band-limited analytic noise or immutable shared tile may be selected by the renderer owner only after real output spectrum/aliasing comparison, not by a product shader script.

**INV-387-004 NO SEMANTIC PARTICLES:** No particles, moving dots, procedural scratches, glass damage, animated scanlines or frequency response derived from user input are implied. Grain is small-scale surface texture, not content or a status indicator.

## 3. Color and compositing

**INV-387-005 EXACT SDR LIGHTNESS RESPONSE:** Work on the material's straight-linear face RGB C with its existing alpha a, not on already premultiplied interpolated framebuffer pixels. Define linear Y=dot(C,(0.2126,0.7152,0.0722)), s=srgb_encode(Y), and signed deterministic spatial sample n in [-1,1] with zero mean. Let d=n*author_amplitude, s'=clamp(s+d,0,1), and Y'=srgb_decode(s'). If a=0 or Y=0, do not introduce new colored energy (output face remains black where Y=0). Otherwise let N=C/max(C.r,C.g,C.b), Q=N*(Y'/dot(N,Rec709)), then use a single hue-preserving gamut normalization Q=Q/max(1,max(Q.r,Q.g,Q.b)). Re-premultiply Q by the original a exactly once in the ordinary renderer output path. No channelwise clamp, unguarded Y'/Y, alpha change, second sRGB conversion or separate post-blend screen perturbation is allowed. The output-profile-aware encoded luminance excursion is measured after gamut normalization and MUST remain within 2/255 for this SDR profile.

**INV-387-006 MATERIAL-LOCAL ORDER:** Grain is evaluated in the own material face before its final premultiplied/AA/ancestor clip projection, never as postprocessing on an already composited backdrop. On Frosted Glass it modulates the separate authored tint/material face only; it must not alter the actual Gaussian source sample F or add frequency content to a supposedly blurred background. On Smoked Glass it modifies the admitted direct material face. It does not touch text/icons or act on the full screen.

**INV-387-007 ZERO-EQUIVALENCE:** Strength zero produces the exact ordinary baseline path without material noise sampling or extra resource work. An absent grain selection has no effect on existing quiet gradient or solid surfaces.

## 4. Lifetime, resource and performance

**INV-387-008 RETAINED:** Equal style, seed, geometry, target scale and material generation yield stable output without continuing frame demand. Real changes invalidate only the affected fragment. No new source Image per widget, independent worker, mutable global noise table or hidden phase is admitted.

**INV-387-009 EXACT ADMISSION AND COST:** Source grain inputs and the actual output transfer/format are checked by the existing appearance/renderer owners. Unknown transfer or HDR/wide-gamut without a separately adopted profile returns typed unsupported, not a silently incorrect near-black dither. Static material grain requires no extra full-screen pass or frame clock; a shared immutable lookup pattern, if selected, is renderer-owned with bounded memory. The existing PM-026 correction for gradient anti-banding is preserved unchanged.

## 5. Forbidden recurrence

**FORBID-387-001:** Do not reintroduce PM-026's near-black display-transfer amplification as decorative grain. Anti-banding dither and visible grain are independently authored, independently bounded mechanisms.
**FORBID-387-002:** Do not randomize by frame, render arbitrary screen-wide noise, derive stable seeds from hidden globals, affect text/glyph/color semantics, change alpha or clipping, or compensate for missing blur/reflection.
**FORBID-387-003:** Do not substitute unverified periodic coordinate hashing for spatial-quality proof or silently clamp excessive requested grain.

## 6. Future conformance and adoption

Before implementation the renderer owner must qualify the actual spatial spectrum and finite noise sample n, with fixed seed and no frame correlation; prove repeat-frame bitwise stability; local anchoring through subpixel movement, DPR, scroll and resize; exact SDR transfer equations and hue-preserving gamut normalization for RGB black, near-black, near-white, saturated colors and a=0/partial/full; the measured encoded luminance change <=2/255 on the supported transfer; explicit HDR/wide-gamut/unknown-target refusal; no modification of anti-banding gradients and no recurrence of PM-026. Real 1x/2x native captures must show no periodic bands or distracting texture. JC separately accepts actual material texture. No source implementation is established by this candidate.

No source behavior or runtime capability changes follow from this inactive candidate.

## 7. Adopted S1: filtered material-local micrograin (2026-11-13)

**Status: ADOPTED / BINDING under JC's standing independent-effects implementation mandate.** The first consumer is the existing HAL-386 SurfaceAnalytic face, with HAL-389 rim remaining independently optional. This section supersedes inconsistent candidate clauses above, not existing working material behavior. It adds no Frosted Glass implementation, product skin adoption, renderer family, pass, texture or global noise state.

### 7.1 Material contrast domain

The authored amplitude is in **reference sRGB-encoded material luminance**, not monitor code values. The reference uses linear Rec.709 weights (0.2126,0.7152,0.0722) and the sRGB transfer defined below. It is an internal numeric authoring unit converted back to the existing bounded linear material RGB before ordinary composition. It does NOT depend on OS monitor ICC discovery. This expressly replaces the candidate's proposed target-format refusal: there is no invented proof that a display profile is sRGB, and no unsupported-appearance error is disguised as SurfaceError. Actual output-transfer, compositing and display conversion remain with existing renderer/platform owners. Initial physical evidence is required on the existing Bgra8UnormSrgb Metal path; no HDR, other-gamut, physical-scanout or universal two-display-code accuracy is claimed. The reference-luminance bound below is exact before composition, not a per-channel or post-display error bound.

### 7.2 Checked value and one consumer

Add immutable core::paint::SurfaceMicrograin with private scale_px: f32, amplitude_srgb: f32 and seed: u32. Its try_new(scale_px, amplitude_srgb, seed) returns Result<Self,SurfaceMicrograinError>, distinguishing NonFiniteInput, ScaleOutOfRange and AmplitudeOutOfRange. Scale is in [0.75,3.0] current target pixels; amplitude is in [0,2.0/255.0]. No input clamping or implicit seed. All u32 seeds are legitimate values.

Preserve SurfaceAnalytic::try_smoked_glass and push_analytic_surface signatures. Add SurfaceAnalytic::with_micrograin(self, Option<SurfaceMicrograin>) -> Self and a read-only micrograin() getter. The existing constructor stores None. Canonicalize a zero-amplitude input to None, so it has the same semantic retained identity and exact output as no grain. Grain is an ordinary authored appearance value, not a runtime feature switch. Existing Solid/Gradient/Image/NinePatch, Shape 0..5, Button material, glow and contour paths remain unchanged. No standard control gains grain automatically.

### 7.3 Exact stable, footprint-filtered field

Let p be current logical fragment position, o the actual scroll-projected material origin, D current admitted target DPR and s=scale_px. Define q=(p-o)*D/s; take integer b=floor(q) and f=q-floor(q) separately, with floor converted to bounded i32. All weight arithmetic uses f, not subtraction of large absolute lattice coordinates. The finite existing geometry/DPR domains keep lattice indices within i32.

Define the integral of the unit triangular reconstruction kernel:
T(z)=0 when z<=-1;
T(z)=0.5*(z+1)^2 when -1<z<0;
T(z)=1-0.5*(1-z)^2 when 0<=z<1;
T(z)=1 when z>=1.
For j in {-1,0,1,2}, define wx[j]=s*(T(f.x+0.5/s-j)-T(f.x-0.5/s-j)); define wy analogously. These are exact box-footprint integrals of a bilinearly reconstructed lattice field. They are not screen-coordinate random dots, Gaussian blur or a claim of perfect band limitation.

At lattice index (ix,iy), reinterpret signed i32 bits as u32. Use wrapping u32 arithmetic:
h=(uint_bits(ix)*0x9e3779b9) XOR (uint_bits(iy)*0x85ebca6b) XOR seed;
h=(h XOR (h>>16))*0x7feb352d;
h=(h XOR (h>>15))*0x846ca68b;
h=h XOR (h>>16);
v=float(h>>8)*(2.0/16777215.0)-1.0.
Sum wx[j]*wy[k]*v(b.x+j,b.y+k) for j,k in {-1,0,1,2}, divide by sum(wx)*sum(wy), and bound only the computed rounding residual to [-1,1]. Both weight sums are positive throughout the admitted scale domain. There are at most 16 lattice evaluations, no stored tile, texture sample, mutable state, clock or random reseeding.

The field has statistically zero-centred lattice values; an arbitrary finite crop need not have exactly zero sample mean. It is continuous under fractional translation and attached to material-local origin. Equal inputs reproduce equal pixels; integer-pixel motion carries the pattern with the surface. Fractional movement and a DPR change require footprint resampling, not impossible byte equality between different sample grids. DPR changes preserve the authored physical grain size and seed, with a newly evaluated local grid. Resize does not stretch the field to normalized UV. No clock or idle frame demand is introduced.

### 7.4 Exact colour operation and independence

Apply the texture to the already lit, luminance-capped and gamut-normalized face S from HAL-386 section 8.5, BEFORE its independently authored rim mix. Let Y=dot(S,Rec709). If amplitude is zero, max(S)=0, or computed Y<=0, return S unchanged before noise or transfer evaluation. Otherwise compute sY=encode(Y), sY2=clamp(sY+n*amplitude_srgb,0,1), Y2=decode(sY2), N=S/max(S), Q=N*(Y2/dot(N,Rec709)), and Sg=Q/max(1,max(Q)). The denominator dot(N,Rec709) is at least 0.0722 for a nonzero valid face; no Y2/Y division or subnormal luminance reciprocal is used.

encode(x)=12.92*x for x<=0.0031308, otherwise 1.055*x^(1/2.4)-0.055.
decode(x)=x/12.92 for x<=0.04045, otherwise ((x+0.055)/1.055)^2.4.
All transfer inputs are in [0,1]. Uniform gamut normalization preserves hue; do not clamp individual RGB channels. The final reference excursion abs(encode(Y(Sg))-encode(Y(S))) is <=amplitude_srgb apart from explicitly bounded floating-point rounding. The smooth illumination cap still governs S; the separately bounded material microvariation is not additional lighting and is not silently re-clamped away.

Continue the existing rim equation with Sg replacing only S. Keep rim weight, normal, lights, opacity, own AA and ancestor mask unchanged; final RGB remains final_face*alpha. Text/icons are separate untouched draws. Nothing perturbs an already composited framebuffer or backdrop source. Black and zero opacity do not gain energy, alpha does not change, and absent grain executes the exact previous face/rim operations.

### 7.5 One shader ABI extension

Extend only AnalyticSurfaceParams from 384 to 416 bytes, retaining the first 384 bytes and their offsets exactly. Append micrograin: vec4<f32> at bytes384..399 as (scale_px,amplitude_srgb,0,0); append micrograin_seed: vec4<u32> at bytes400..415 as (seed,0,0,0). Absent/zero grain writes both slots as zero. Keep dynamic slot stride align_up(512,device_alignment), group0 Globals, group1 binding0, vertex format, PipelineId13 and SceneNodeKind23 unchanged. Update min_binding_size and format/resource size validation to 416, not stride or occurrence limits. No new GPU allocation, uniform slot, pipeline or draw per material. Shader branches on amplitude>0 before evaluating the field. This is not a flag-packed float or reuse of unrelated reserved prefix lanes.

### 7.6 Required coherent result

Complete checked public use, retained invalidation, CPU/WGSL math, actual GPU evidence and one ordinary native example examples/surface_micrograin.rs. Preserve the existing smoked_glass example. Show identical glass faces with grain absent, restrained and at the permitted maximum, plus sharp foreground text; no product integration or input simulation is required. Capture only that owned App window; no Computer Use or global keyboard/mouse injection.

Place proof in existing paint, analytic pipeline, renderer_z_order and retained_value_updates test owners, with names containing analytic_surface_micrograin so the existing analytic_surface filter covers the complete unit. Prove exact no-grain/zero identity; deterministic seed changes; black, near-black, white and saturated colours; bounded reference excursion and hue; alpha0/0.5/1 and unchanged clips/rim; stable repeat frames; integer and fractional translation against an independent footprint oracle; DPR1/2, resize, retained replacement/removal and unchanged text.

For spatial qualification, evaluate 512x512 fields for seeds387 and305419896 at scales0.75,1.5,3: abs(mean)<=0.02, row/column-mean standard deviation <=0.15 of field standard deviation, and absolute normalized autocorrelation at lags16 and32 in both axes <=0.05. Inspect the actual GPU spectrum and native grain for short tiling/axis lines; the hash alone is not evidence of visual texture quality. Supervisor CPU exploration for seed387 gave standard deviations0.2815/0.3504/0.3736 and row/column ratios below0.10 at those scales; these are algorithm qualification, not GPU/native proof.

Run `verify package halcyon test analytic_surface check timeout 600 output summary` through Grip's typed verification surface. Reuse unaffected contour/glow evidence rather than broad suite loops. Keep exact native/source/GPU evidence distinct and preserve pre-existing diagnostics. JC alone grants optical acceptance. No physical output claim follows from reference-domain maths.

## 8. Independent source and retained-evidence review (2026-11-13)

Independent review checked the checked SurfaceMicrograin constructor, zero-amplitude canonicalization, the 416-byte record preserving its original 384-byte prefix, the exact footprint integral and pre-rim colour operation, and real retained invalidation/text isolation against section 7. Live source and the archived source-evidence.json agree at the reviewed owners.

The typed initial check completed with zero errors and 118 warnings. The executed analytic_surface test run has 16 passing tests and one failed new fixture; the corrected owning test has its separate typed exit-zero 1/1 receipt. These establish passing evidence for the 17 selected tests, not one fresh 17/17 run. The retained earlier compile receipt records four Color namespace compilation errors, whose corrected explicit paint Color uses are present in the reviewed source. It is not an unresolved running job or a new failure of the final source.

Independent review checked the twelve stored GPU spatial records against the specified bounds: maximum absolute mean 0.00494384765625, row/column ratio 0.09370506383007525, absolute lag correlation 0.007498292120192468. The native snapshot bytes match SHA256 d4db255df6682d4ed045f3ec1fc96e4f16019a8a045e7a6d2da3592c6a039d51. Capture provenance, execution and numerical evidence are qualified; independent review has not accepted the screenshot's aesthetics, monitor accuracy or physical scanout. No equivalent test or capture was repeated. Technical source/API and focused CPU/Metal acceptance is granted within these limits. Existing winit diagnostics and the separate broad-ordering timeout remain outside this result.

## 9. Adopted S2 authored texture range (2026-11-14)

JC requires the optical material properties to be configurable rather than a fixed synthetic recipe. Extend the checked SurfaceMicrograin amplitude_srgb domain from[0,2/255] to[0,8/255]; scale remains[0.75,3] target pixels and every u32 seed remains explicit. This is an authoring-range extension, not an automatic stronger texture or an sRGB-monitor assumption. All previous admitted values, zero-to-None identity, exact16-sample field, transfer/gamut/hue math and material-local anchoring remain byte/operation-equivalent. Existing negative-boundary tests must move to the new declared boundary for this intentional change, not lose nonfinite/negative/out-of-range coverage.

The reference-luminance excursion bound is the chosen amplitude, including the new range, before composition. It is never a universal two-code per-channel display claim. Repeat only amplitude-dependent new/extreme/hue/premultiplied proof for the widened interval; prior unchanged seed/spectrum proof remains valid. In the S2 material study restrained grain stays0.6/255 by explicit fixture choice, with0 and8/255 inspectable controls. Grain still affects only the analytic body or frosted tint before the new coating; it never perturbs foreground text, sampled backdrop, clipping, opacity or reflected-light semantics. It is independently removable and must not be used to counterfeit absent surface optics.
