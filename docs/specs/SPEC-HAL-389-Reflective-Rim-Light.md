# SPEC-HAL-389: Reflective Rim Light

**Status:** ADOPTED / BINDING for the independent optional rim in section 7, using the single HAL-386 section 8 analytic-face owner. No automatic integration into other materials or controls; implementation and visual acceptance remain evidence-gated.
**Date:** 2026-11-13.
**Owner:** Halcyon appearance and existing geometry/SDF shading owner.
**Confirmed JC direction:** Reflective Rim Light and emissive Edge Glow are distinct, independently useful effects.
**Governing owners:** HAL-002/002A, HAL-012, HAL-024, HAL-085A, HAL-354, HAL-359. HAL-358 already owns emitted luminous contour; inactive HAL-384 contains related finite 2.5D rim proposals.
**First user result:** A dark glass or metal rounded surface receives a direction-sensitive narrow reflected highlight along *only illuminated parts* of its edge, with no outer halo and no all-around constant stroke.

## 1. Definition and optical distinction

Reflective Rim Light represents material reflection of a supplied finite lighting basis at grazing surface angles. It does not emit light, illuminate neighboring pixels beyond its own surface, create a glow source or require a decorative separate outline. A rim is meaningful only for actual admitted geometric surface coverage with a defined edge/bevel response. HAL-358 Edge Glow instead emits bright core/aura/halo from an explicit contour and remains separately controlled.

This proposal is usable on appropriate solid, Smoked Glass, Frosted Glass front surface and metallic materials where their existing shading/appearance owner supplies suitable geometry and source light. It must never be appended automatically to every pane or become an implicit selected/focused state.

## 2. Proposed optical input and response

**INV-389-001 INPUTS:** The immutable material optical inputs are finite straight-linear rim reflectance color, intensity, physical/logical bevel width, roughness and one or two explicit light direction/color/intensity records. Light direction is normalized in current material-local 3D view coordinates, with positive facing component. Product appearance selects the values; no mouse/OS light, global light manager or time-varying world assumption is implied.

**INV-389-002 REAL EDGE AND AA:** Reflectance comes from the actual owner-supplied rounded rect SDF and its current radii, including asymmetric corners, and from a finite *inward* bevel. A bright duplicate rectangle, static equal-strength border or invented second silhouette is forbidden. Distinguish the mathematical boundary d=0 from raster AA: the reflected face color is multiplied by current own rounded-SDF coverage, which may be fractional for a bounded subpixel region outside d=0. Exact zero is mandatory only where real own coverage is zero, not at every mathematically outside sample with still-valid AA coverage.

**INV-389-003 DIRECTED REFLECTION:** A validated 2.5D profile derives a synthetic bevel normal from the actual SDF gradient within the authored bevel and evaluates bounded directed light and roughness-dependent specular/grazing terms against the current material-view basis. Distinct eligible light directions must create different illuminated edge regions. Zero rim strength, or zero effective radiance for the rim-reflection term, makes only the rim contribution exactly zero. The resulting face equals the same fully evaluated base material with its rim term disabled, preserving all independently authored ambient, diffuse and other specular illumination; neither condition turns the base material unlit. Rim reflection is a *shading term in the existing face evaluation*, not an extra standalone stroke or any claim of 3D thickness, complete energy-conserving PBR or environment reflections. If that face evaluator lacks lawful normals/lighting it reports unsupported; it never paints arbitrary gold borders.

**INV-389-004 NO EMISSION AND COVERAGE:** The reflected rim contributes to RGB only where the ordinary surface's *actual AA coverage* is nonzero; outside the finite SDF AA support it is exact zero and requires no halo margin. Existing rounded and ancestor coverage is applied once by its owner. No external glow, contour-front emission, boost to effective source alpha or out-of-surface 2D light is implied. HAL-358 owns a separate emitted LuminousContour, which accepts explicit geometry; automatic surface-outline conversion to HAL-358 is NOT currently admitted and would require its own source-qualified geometry contract. HAL-388 ambient pool remains a separately authored effect.

**INV-389-005 SINGLE FACE SHADER:** Rim reflection must be calculated in the admitted base material's own geometry/SDF shading path using its actual radius, light basis and alpha. It is not a second painting operation, independent glow pipeline, hidden Button underlay or shader graph. Unsupported base materials explicitly refuse the reflective request. Halo, focus, content and standard Border remain distinct material/appearance owners and require their normal strict Z and clipping.

## 3. Geometry, color, retained dependencies

**INV-389-006 PHASE:** The component owns rect/radius and valid local appearance selection; renderer owns SDF, current target conversion, normal evaluation and clipped fragment output. No paint-time geometry mutation, new layout owner, second focus/capture signal or dynamic light callback is admitted.

**INV-389-007 COLOR:** The response uses straight-linear authored inputs and emits finite, gamut-qualified premultiplied-linear output under the existing clip/AA law. Contribution cannot bypass the chosen material's bounded luminance and alpha contract. It must preserve independent text/icon contrast and mandated focus cues.

**INV-389-008 RETAINED:** Illumination values, own geometry, radius, material generation, target scale and actual clip determine retained identity. Stable lighting does not require an animation clock or recurring frame. Real changed lighting invalidates only the owner-correct material fragment and affected composition, not unrelated widgets or UI semantics.

## 4. Collision with HAL-384

**INV-389-009 SINGLE ANALYTIC OWNER:** The design resolution is one finite rim-shading term in the existing HAL-384 proposed AnalyticSurface SDF/light evaluation where that owner is lawfully adopted, not a competing generic rim renderer. HAL-389 states its independent optical meaning and correct coverage law; HAL-384's existing two-light, roughness, bevel and explicit rim_strength remain the candidate mechanical starting point. The currently proposed HAL-384 fixed rim-color mix alone does *not* prove directional rim reflection; before adoption, coordinate one exact direction-sensitive formula and its numerical bounds inside HAL-384/HAL-085A, retaining private Button SDF and accepted consumer visuals. No inactive proposal implements or activates another.

## 5. Prohibitions

**FORBID-389-001:** No disguised 1px border, equal-intensity full perimeter, unexplained gold outline, repeated overlaid strokes, halo, refraction, backdrop read or self-emissive material.
**FORBID-389-002:** No cross-widget light tracing, global mutable light, independent perpetual repaint, arbitrary PBR mesh, shader graph or inferred source direction from OS pointer.
**FORBID-389-003:** No layout/clip/hit changes, no extra source alpha to make the edge brighter, no material substitution, no overriding mandatory focus/selection truth.

## 6. Conformance and acceptance

Before admission prove finite input validation, zero-rim/no-light exact ordinary baseline, source light direction reversal producing different edge highlights, roughness and asymmetric-corner variation, SDF normal stability, exact zero outside the renderer's real finite AA coverage (but lawful fractional coverage near d=0), bounded premultiplied linear RGB and unchanged alpha, retained reuse, no duplicate stroke/pipeline and 1x/2x GPU proof. Native comparison of the *same surface* must isolate ordinary Border, reflective Rim Light, HAL-358 emitted Edge Glow and HAL-388 ambient halo. JC selects optical balance and approves the real result. The existing HAL-358 contour requires explicit source contour geometry, not an invented automatic surface conversion.

No code, adoption of HAL-384, new public API or native visual acceptance follows from this candidate.

## 7. Adopted S1 reflective rim in the shared analytic face (2026-11-13)

**Binding under JC's implementation mandate.** The exact mechanical owner is HAL-386 section 8 with coordinated HAL-012, HAL-085A and HAL-084 amendments. This adopts reflective rim as an independently optional SurfaceRimLight value in the same SurfaceAnalytic Smoked Glass evaluation, not a second pipeline or mandatory material decoration. The inactive HAL-384 constant rim-color mix is not adopted.

The required directional term is HAL-386 section 8.6: g=clamp(sum(E*light_strength*Y(light_RGB)*D*P),0,1), w=e*rim_strength*rim_color.a*g, C=S when w=0 and otherwise mix(S,rim_color.rgb,w). S remains the fully evaluated lit base face. A zero rim or zero effective source radiance affects only that term; base illumination, alpha, clips, geometry and content are preserved. Weight is bounded to 0.20 and zero away from the inward bevel; own antialiasing is handled by the actual four-corner SDF coverage. No halo, separate stroke, backdrop read or extra source alpha.

HAL-386 section 8 defines the complete checked inputs, geometry, 384-byte shader record, one-round-ancestor admission, retained behavior and required tests. Native comparisons must isolate rim-off from rim-on on an otherwise identical material and reverse the eligible source direction without moving the actual surface. This profile does not silently extend private Button shading, Solid/Gradient consumers, Frosted Glass or Metal profiles; those integrations require their own explicit owner admission. Existing HAL-358 emitted contour and HAL-388 ambient halo are preserved.

This design authorizes implementation; it does not establish implementation or visual acceptance.

## 8. Coordinated S2 optical coating rim (2026-11-14)

**ADOPTED / BINDING.** JC's request for configurable credible glass admits HAL386 section11's shared GlassSurfaceOptics on both existing direct Smoked Glass and current-backdrop Frosted Glass. Its optional reflected edge uses the actual coating bevel normal, the two explicitly authored lights and the exact independent wr equation; rim_strength0 leaves the fully evaluated coated face and base material unchanged. Its width/slope/falloff, hue/alpha and strength are authored values, not a mandatory uniform border. This added optical layer does not silently change the original SurfaceRimLight constructor or its S1 output. The lab isolates the new rim with the legacy rim absent.

The shared pure shader helper is the only owner of this new coating calculation. Neither consumer duplicates its math, invents a light manager or turns it into emission. HAL388 surface emission and HAL358 emissive contour stay independently selected and separately named. Thin and thick appearances use parameters on one silhouette, not parallel renderers. Existing clip, AA, geometry, retained and source-custody rules remain unchanged. Numeric correctness and direction reversal are necessary evidence, not JC visual acceptance.
