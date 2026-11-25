# SPEC-R-029: Factor-Only Sheen Opt-In PBR Material

**Author:** JC
**Date:** 2026-11-03
**Status:** Draft / INACTIVE / NOT BINDING — S0 documentary candidate only
**Proposed profile:** PbrMetallicRoughnessSheenF32V1
**Architecture selection:** Factor-only sheen for the original Bed reference scene.
**S0 reference correction:** Exact projected-area grazing correction, selected
through delegated technical review on 2026-11-03; documentary baseline c5c75b5c, no adoption or implementation grant.
**Implementation and conformance:** NOT AUTHORIZED / NOT RUN under this draft.
**S1/S2/S3/S4 for this profile:** NOT GRANTED. JC retains final veto.

## Historical initial S0 drafting scope

- Objective: Specify the complete selected factor-only sheen material, its exact
  reference equations, source/lighting association, resource custody, proof gates
  and unresolved numerical decisions in this one new inactive document.
- Standing objective: Preserve the original artist material and high-quality
  GPU-first real-time PBR without weakening source, actual host, shadow, HDR or
  publication correctness. This document does not establish Bed support.
- Review authority: The delegated architectural reviewer selected the mathematical
  design; the draft records it and identifies exact gaps. JC retains reserved approval.
- Adoption boundary: This S0 artifact requires independent review.
  No implementation, source intake, benchmark or successor work follows.
- Source qualification: Affected R024/025/026/027/028, HAL355 and HAL370 clauses
  and public material/source owners were inspected.
- Governing authority and status: HAL002/002A supreme; adopted parent contracts
  listed in §1 remain binding. This draft supplies no adopted parent delta.
- Authority change, if any: None. Only S0 document authoring is authorized.
- Target layer and existing state verified: Halcyon e71b1791,
  0.230.1-beta; current five-role descriptor and MaterialBlock64 contain no
  sheen profile or fields. Source/test files are outside this unit.
- Owned mechanisms and files: Create only this SPEC-R-029 document. Preserve
  existing foreign R024/R028/HAL370 edits and the historical proposal.
- Named invariants: PROFILE, SOURCE, LOBE, ENERGY, LIGHT, NUMERIC, RESOURCES,
  CURRENTNESS, ABSENCE and PROOF in §12.
- Forbidden behavior: No V1 mutation, simplified visibility, unscaled additive
  sheen, source repair, shader/API/ABI change, silent unsupported-role omission,
  implementation, private model access, acceptance or runtime claim.
- Existing mechanisms inspected and reuse decision: Existing typed material and
  role leases, immutable source admission, R025/R026 light owners, R027 final
  publisher and HAL355 target/capacity ownership remain the mechanisms.
- Relevant external or dependent boundaries: The application owns original
  factors/decoding/container data; Halcyon owns admitted evaluation and resources.
  Official Khronos schema and implementation notes are references, not authority.
- Test ownership and placement: Future proof belongs in existing material,
  Context3D, R025/R026, R027 and real N1/N4 owners after separate authorization.
  No executable tests are added or run here.
- Exact runnable verification commands and expected proof: Through Grip select
  halcyon, read this document/headings and inspect its new-file diff. Perform one
  bounded Markdown/link/status audit and whitespace check. Expect exactly this
  new draft, no accepted-status claim and no other owned file change.
- Output or artifact completeness boundary: One complete selected reference
  contract with explicit open decisions. Open implementation mathematics is
  reported as a blocker, never replaced with a guessed value or approximation.
- SemVer and changelog impact: N/A; inactive design adds no executable behavior,
  supported profile, public interface or data format. No metadata changes.
- Out of scope: All source/tests/shaders, R024 amendments, private Bed bytes,
  importer/HAC wire, native UI, assets, performance work and runtime admission.

## Architecture boundaries

- Governing SPEC and read status: HAL002/002A; HAL355 resource/generation law;
  R024 source/material and 2026-11-03 proof boundary; R025 real shadows/radiance;
  R026 indirect; R027 FP16/Image; R028 sampling; HAL370 stages.
- Widget policy or N/A: N/A, material documentation only.
- Interaction added and policy authority: None.
- Applicable FORBID rules: No authority bleed, source/normal repair, extra light,
  unbudgeted resource, stale front, universal GPU proof or self-adoption.
- Existing framework mechanism inspected: Typed descriptor/five roles,
  MaterialBlock64/material leases and existing source/target/final owners.
- Reinvention risk: A separately qualified material profile reuses those owners;
  no new renderer, source registry, scheduler or lighting system.
- Demo/example compliance: N/A; no example, runtime or private source changes.

## 1. Authority, status and proposed scope

The governing parents are [HAL002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
[HAL002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md),
[HAL355](SPEC-HAL-355-Native-3D-Rendering-Substrate.md),
[R024](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md),
[R025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md),
[R026](SPEC-R-026-Environment-and-Indirect-Lighting.md),
[R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md),
[R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md) and
[HAL370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md).
Their current adopted scopes are unchanged.

The 2026-11-03 S0 projected-area correction named above replaces only this
inactive draft's former fitted visibility and dependent mathematical claims.
It retains the Charlie distribution, source factors, layering, parent boundaries
and original authoring evidence. The exact reference proofs below resolve
R029-GRAZING-DOMAIN at S0 mathematics only; all later admission gates remain.

SHALL/MUST in this draft describe proposed obligations, conditional on a later
attributable coordinated S1 decision. They do not alter current production law.
HAL370's existing closed profile/ABI family does not automatically include R029.
The numerical and interface decisions in §14 remain open; no S2 code grant,
public S3 envelope or native S4 approval is supplied.

R024 §8.1 expressly selects GGX/separable-Smith/Schlick plus Lambert and excludes
sheen. R029 therefore proposes a distinct tagged profile,
**PbrMetallicRoughnessSheenF32V1**, rather than adding fields to
PbrMetallicRoughnessRealtimeF32V1. It layers the selected sheen over the
unchanged R024 base response; it does not reinterpret an old material.

This is a versioned real-time material approximation with an explicit 0.045
sheen evaluator floor. It is not a claim of exact cloth microgeometry,
multiple scattering, exact optical glTF equivalence at every roughness, or
universal device arithmetic.

## 2. Intended subset and current source evidence

The dispatch reports two factor-only KHR_materials_sheen materials in the
original private Bed, with:

| Source fact supplied by the owner | Exact value / implication |
|---|---|
| sheenColorFactor | (0.19999998807907104, 0.19999998807907104, 0.19999998807907104), linear RGB |
| sheenRoughnessFactor | 0.5, independent of base roughness |
| Sheen textures | None authored; neither sheen texture role is needed for this subset |
| Base-color texture | Genuinely absent; existing base factors/default rules still apply |
| Existing MR/normal resources | Two shared original 4096-source PNGs; their distinct role/encoding interpretation remains |
| Authored TANGENT | Absent; no original tangent may be invented or silently substituted |

These are supplied intake facts, not findings from opening the model in this
unit. No private file, URI, blob, image, mesh or binary is copied or inspected.
They do not establish a valid normal certificate, a complete radiance bound,
a backend result or current Bed rendering.

For the supplied roughness, s_eff=0.5 and alpha_g=0.25 exactly in the ideal
reference. The floor therefore does not change this authored roughness.
The color value above is not replaced with the different rounded value 0.2.

The proposed initial subset adds only the two scalar-factor semantics to an
otherwise independently valid R024 material. All five old texture roles,
mip/sampler meanings, alpha classification and sidedness remain. Absence of an
authored tangent still requires an actually admitted R024 A/B/C or
genuine-normal-absence route; this profile grants no frame repair.

## 3. Owners and reuse boundary

| Concern | Owner and obligation |
|---|---|
| Original material meaning, extension presence, values, decoding and packaging | Application/source producer; no renderer import or inferred asset semantics |
| Profile and typed descriptor admission | Existing specialized Halcyon material owner, after a future explicit interface decision |
| Base response, normal frame, role sampling | Existing R024/R028 mechanisms with their original source checks |
| Sheen response and its bound | Controlled versioned material evaluator under this new profile, after §14 closure |
| Direct illumination and genuine geometry shadows | R025; no new light, changed cascade or substituted visibility |
| Indirect illumination | R026; composition with sheen is unqualified and blocked under §6.2 |
| Resources, immutable generations and leases | Existing Context3D; no global material/LUT registry |
| Per-target preparation, submission and final output | Existing HAL266A/HAL355/HAL295A owner and R027 FP16/Image path |

One packet retains one immutable material/profile association. A scene containing
old V1 and new-profile packets must not retag all packets or reinterpret an old
64-byte block. Its exact mixed-profile layout/pipeline admission is part of the
unresolved future interface decision, not a permission granted here.

## 4. Complete immutable material source

### 4.1 Factors, defaults and presence

The source contract is:

| Input | Proposed admissible source value | Default when genuinely omitted inside a present sheen extension |
|---|---|---|
| sheenColorFactor C | Exactly three finite linear-RGB components, each in [0,1] | (0,0,0) |
| sheenRoughnessFactor s | One finite scalar in [0,1] | 0 |
| sheenColorTexture | Unsupported in this factor-only profile | No role/binding |
| sheenRoughnessTexture | Unsupported in this factor-only profile | No role/binding |

The factor limits/defaults follow the official [Khronos schema][R2].
Neither factor is gamma-decoded. The source roughness is not base metallic-
roughness G, a shininess exponent, or a derived value from a texture/name.

A declared sheen texture is a typed Unsupported-source/profile result, even
when its factor is zero, its bytes are unavailable, or a producer marks that
extension optional. Missing data cannot make a declared role genuinely absent.
Do not fetch/decode an unsupported role merely to manufacture a default.
Required missing old roles still report their original typed source cause.

An absent sheen extension uses the exact old V1 path. A present, explicitly
selected new-profile material with C=(0,0,0) remains a distinct source/profile
identity even though its sheen contribution is zero. Zero contribution does
not excuse invalid or unsupported explicit dependencies.

KHR_materials_unlit and KHR_materials_pbrSpecularGlossiness are incompatible with
the referenced extension. Clearcoat, transmission and other unselected layer
compositions gain no admission through R029. Already supported unrelated
families remain unchanged; no new mixed extension is inferred.

### 4.2 Identity, successor and source admission

A future admitted capture SHALL bind the exact profile/version, source
identity/revision and extension presence; original C/s and their packed
representations; all original base/normal/MR/AO/emissive factors and roles;
sampling/encoding/frame/certificate dependencies; Opaque/Mask/cutoff/sidedness;
geometry/model/structural association; the selected evaluator/reference and
P/E-representation versions; Context family and every resource generation/lease.

A changed sheen factor, presence, profile, certificate or dependent resource
creates a complete material and affected structural successor. Equal IDs,
equal roughness or equal optical output do not authenticate different source
captures. No mutable factor patch or old-front relabel is lawful.

The first new immutable generation receives full original source validation,
including the normal/mip/sampler and complete radiance checks plus the new
profile checks. Exact independently checked still-live captures may be reused
on FOV-only changes under R024 §8.2.4/HAL370 §9. No repeated unchanged texel scan,
global trust cache or view-dependent GPU simulator is introduced.

## 5. Selected complete ideal material equations

This section defines real-arithmetic reference functions, not executable WGSL,
an approved f32 evaluation order or an admitted approximation table.

### 5.1 Directions and noncontributing branches

Use the SAME qualified final mapped/back-face-correct shading normal N and
surface-to-view/light unit directions V,L as R024. Source geometric face
normals remain the separate R025 shadow/bias inputs.

Set nv=N·V and nl=N·L. Reflection contributes zero if nv<=0 or nl<=0, before
forming a reciprocal, half-vector, power, projected-area or visibility evaluation.
R025's no-light, out-of-range and coincident-point zero branches remain earlier
noncontributing cases. Independent emission is not removed.

For nv>0 and nl>0, H=(V+L)/||V+L|| is mathematically defined because
N·(V+L)=nv+nl>0. Let nh=N·H. Qualified ideal dots lie in their unit domains.
Invalid/nonfinite source or actual host data is not converted into a zero-light
branch. New f32 endpoint/underflow guards remain §14 decisions.

For C=(0,0,0), the sheen response is zero and base attenuation is one without
evaluating E, D or lambda. Base roles and all declared source dependencies must
already be valid. This is a sheen-math branch, not a source-validation shortcut.

### 5.2 Independent sheen roughness and Charlie distribution

~~~text
s_eff = max(s, 0.045)
alpha_g = s_eff^2
D_chi(nh,s) = (2 + 1/alpha_g)
             * (1 - nh^2)^(1/(2*alpha_g)) / (2*pi)
~~~

Thus alpha_g is in [0.002025,1] in the ideal reference. It is NOT squared again
as the GGX a2 parameter, and the base r_eff remains independently unchanged.

D_chi is nonnegative and finite for nh in [0,1]. At nh=1 it is zero, with a
strictly positive exponent. Its maximum is
D_max(s)=(2+1/alpha_g)/(2*pi). The projected NDF normalization is exactly:

~~~text
integral_hemisphere D_chi(N·M,s) * (N·M) dω_M
 = (2+1/alpha_g) * integral_0^1 u^(1+1/alpha_g) du
 = 1
~~~

Here u=sin(theta_M). NDF normalization is the first identity used in §5.6's
projected-area and ideal energy proofs. It is not by itself proof of the whole
layer, a finite f32 implementation or actual GPU output.

### 5.3 Exact projected-area Charlie visibility

The selected reference is the EXACT projected-area construction below.
Khronos's fitted lambda is no longer the selected visibility; its former S0
selection is retained only in the clearly superseded history at §16.1.
No cutoff, epsilon, fitted substitute or positivity repair is introduced.

Let N=(0,0,1), nu in [0,1], and
V_nu=(sqrt(1-nu^2),0,nu). Azimuth symmetry makes this representative view
sufficient. On the positive half sphere H_plus={H: ||H||=1, N·H>0}, define:

~~~text
P(nu,s) = integral_H_plus D_chi(N·H,s) * max(V_nu·H,0) dω_H

H(u,phi) = (sqrt(1-u^2)*cos(phi), sqrt(1-u^2)*sin(phi), u)
dω_H = du dphi

P(nu,s) =
    integral_u=0..1 integral_phi=0..2*pi
        [(2+1/alpha_g)/(2*pi) * (1-u^2)^(1/(2*alpha_g))]
        * max(sqrt(1-nu^2)*sqrt(1-u^2)*cos(phi) + nu*u, 0)
        dphi du
~~~

The endpoints add only measure-zero hemisphere boundaries. There is no extra
u weight in this area integral. The positive part max(V_nu·H,0) is the exact
geometric projected area, not a later clamp on a failed BRDF, E or A.

For positive contributing cosines, define the exact correlated masking term:

~~~text
lambda_exact(nu,s) = P(nu,s)/nu - 1                     for nu>0

G2(nv,nl,s) =
    1 / (1 + lambda_exact(nv,s) + lambda_exact(nl,s))

Q(nv,nl,s) = nl*P(nv,s) + nv*P(nl,s) - nv*nl

f_chi(N,V,L,s) = D_chi(nh,s) / [4*Q(nv,nl,s)]
~~~

The last line is the selected stable ALGEBRAIC REFERENCE for nv>0,nl>0.
Multiplying the G2 denominator by nv*nl proves that it is exactly
D_chi*G2/(4*nv*nl). It selects no f32 instruction order, approximation,
quadrature, storage layout or GPU implementation.

P is defined also at nu=0 for the area/limit proofs; lambda_exact is not
evaluated there. The original noncontributing branches still return zero for
nv<=0 or nl<=0. No infinite lambda value or extended boundary evaluation is
required in a runtime calculation.

The ideal proofs of P>=nu, positive Q, reciprocity, uniform weighted bounds and
E<=1 are in §5.6. The source color multiplies the unit lobe exactly once.
No additional Fresnel, metallic multiplier, artistic terminator softening or
multiple-scattering term is selected. Simplified Ashikhmin visibility,
the superseded fitted lambda and unscaled additive sheen are not this profile.

### 5.4 E is the reflectance of THIS exact unit-color lobe

For 0<nu<=1 and fixed s, use N=(0,0,1) and
V_nu=(sqrt(1-nu^2),0,nu). For mu in [0,1] and phi in [0,2*pi), let
L_mu_phi=(sqrt(1-mu^2)*cos(phi), sqrt(1-mu^2)*sin(phi), mu).

~~~text
E(nu,s) =
    integral_phi=0..2*pi integral_mu=0..1
        f_chi(N,V_nu,L_mu_phi,s) * mu dmu dphi
~~~

The hemisphere boundary mu=0 uses the noncontributing rule. E includes no C,
base response, extra pi normalization, light, AO or emission. The same s_eff,
alpha_g, D, exact P and correlated visibility define both E and f_chi.
Section 5.6 proves 0<=E<=1 in this ideal reference for every nu>0.
It is not a GGX DFG,
a Lambert constant or an albedo from a different cloth model.

This integral and §5.3's P integral are the mathematical references. No runtime
quadrature, LUT size, coordinates/filter, error bound, endpoint extrapolation,
coefficient precision or interpolation scheme is selected. P includes nu=0;
E is used only after the positive-cosine branch. Any stored E endpoint and its
relationship to the positive-cosine limit require a separate representation
decision. A finite table without certified relation to these SAME integrals
is not a source certificate.

### 5.5 Two-direction energy-aware base scaling

Let q=max(C.r,C.g,C.b), and let f_base be the EXACT existing R024 regularized
GGX/Lambert response, including its metallic partition and normal semantics.

~~~text
A(nv,nl,C,s) = min(1 - q*E(nv,s), 1 - q*E(nl,s))

f_layer(N,V,L) =
    A(nv,nl,C,s) * f_base(N,V,L) + C * f_chi(N,V,L,s)
~~~

Products involving C are componentwise. A scales BOTH base diffuse and base
specular with the same scalar; it does not scale independent emission.
Sheen roughness and color are not multiplied by base roughness, base RGB,
metallic, AO or alpha. Mask still controls which samples survive.

Section 5.6 proves finite 0<=E<=1 for the EXACT reference at every positive
cosine. For all admitted factors 0<=q<=1, both q*E values lie in [0,1], so
0<=A<=1. The original nonnegative base and the nonnegative f_chi give a
nonnegative f_layer. Section 5.6 also states the conditional integrated energy
bound using the base's OWN bound; it does not strengthen R024's promises.
These are symbolic ideal results, not an assertion that a chosen P/E table,
f32 evaluator, source radiance certificate or GPU has been qualified.

No max(A,0), E saturation, arbitrary cosine floor, replacement visibility or
view-only albedo-scaling shortcut is selected. A failed obligation does not
license changing these equations.

### 5.6 Ideal projected-area, grazing and energy proofs

**R029-GRAZING-DOMAIN: RESOLVED at S0 reference-mathematics level ONLY**
by HAL-R029-EXACT-PROJECTED-AREA-GRAZING-CORRECTION-S0-01.
The former fit's failure is historical (§16.1), not a property of the newly
selected reference. No source factor or actual input is modified by this proof.

#### 5.6.1 Signed projected area and nonnegative lambda

For each s, D_chi is nonnegative and azimuth-symmetric. The horizontal
components of integral_H_plus D_chi(H_z,s)*H dω_H vanish by symmetry;
the vertical component equals one by §5.2. Therefore:

~~~text
integral_H_plus D_chi(H_z,s) * (V_nu·H) dω_H = nu

P(nu,s) >= integral_H_plus D_chi(H_z,s)*(V_nu·H) dω_H = nu

lambda_exact(nu,s) = P(nu,s)/nu - 1 >= 0                for nu>0
~~~

The inequality follows pointwise from max(x,0)>=x and D_chi>=0.
P(1,s)=1, so lambda_exact(1,s)=0. These are exact real identities.

#### 5.6.2 Continuity and a uniform strictly positive area bound

Write alpha_min=0.002025. D_chi(u,s) is jointly continuous for
u in [0,1], alpha_g in [alpha_min,1], and is bounded above by
D_max_all=(2+1/alpha_min)/(2*pi). The projected positive part is continuous
and bounded by one. Integrating this bounded continuous integrand on the
fixed finite hemisphere makes P continuous on the compact (nu,alpha_g)
domain [0,1]×[alpha_min,1].

For a constructive uniform lower bound, take the fixed patch

~~~text
1/2 <= u <= 3/4,    -pi/6 <= phi <= pi/6   (phi modulo 2*pi).
~~~

It contains H=(1,0,1)/sqrt(2). Throughout this patch:

~~~text
H_z >= 1/2
H_x >= (sqrt(7)/4)*(sqrt(3)/2) = sqrt(21)/8 >= 1/2

V_nu·H >= [sqrt(1-nu^2)+nu]/2 >= 1/2       for all nu in [0,1]

1-u^2 >= 7/16
D_chi(u,s) >= [3/(2*pi)]*(7/16)^(1/(2*alpha_min))
solid_angle(patch) = (1/4)*(pi/3) = pi/12

P(nu,s) >= P_min
P_min := (1/16)*(7/16)^(1/(2*alpha_min)) > 0
~~~

The second dot inequality uses the nonnegative components of a unit
two-vector: sqrt(1-nu^2)+nu>=1. The D inequality uses 2+1/alpha_g>=3 and,
for a base in (0,1), 1/(2*alpha_g)<=1/(2*alpha_min).
Thus P is strictly positive even at nu=0, uniformly across all selected
roughnesses. Its actual compact-domain minimum is at least this P_min.

This patch and symbolic bound are a proof construction, not a source cutoff,
f32 stored constant, quadrature threshold, LUT choice or claimed tight
radiance certificate. No numerical sampling is used or required here.
In particular, no grazing eta is introduced.

#### 5.6.3 Positive reciprocal visibility and weighted-lobe bound

For nv>0,nl>0, P(nl,s)>=nl implies:

~~~text
Q = nl*P(nv,s) + nv*(P(nl,s)-nl) >= nl*P(nv,s) > 0

0 <= f_chi*nl
   <= D_max(s)/(4*P(nv,s))
   <= D_max(s)/(4*P_min)
   <= D_max_all/(4*P_min)
~~~

This uniform finite bound has no positive lower limit on nv or nl.
It bounds the cosine-weighted response required by direct lighting, not an
unweighted BRDF at the doubly grazing boundary.

Q is symmetric under nv↔nl. H=unit(V+L), hence D_chi(nh,s), is unchanged
under V↔L. The exact sheen response is reciprocal. A is also symmetric;
no nonreciprocal artistic terminator term has been introduced.

As nu approaches zero, P(nu,s) approaches the positive P(0,s).
lambda_exact therefore diverges rather than retaining the historical fit's
finite limit, while the selected weighted response remains bounded by the
expression above. The zero-cosine branch remains unchanged; no infinite
lambda evaluation is an implementation instruction.

#### 5.6.4 Half-vector Jacobian and E<=1

Fix V with nu=N·V>0. Positive incident L corresponds uniquely to the set

~~~text
S_V = {H in H_plus : V·H>0,
       N·[2*(V·H)*H - V]>0}

L = 2*(V·H)*H - V
dω_L = 4*(V·H) dω_H.
~~~

To check the Jacobian, use spherical coordinates about V: if H has polar
angle beta, then L has polar angle 2*beta, with the same azimuth.
sin(2*beta)*2 dβ dphi = 4*cos(beta)*sin(beta) dβ dphi,
and cos(beta)=V·H>0. This establishes the stated change of solid angle.

Since lambda_exact(nl,s)>=0:

~~~text
G2 <= 1/(1+lambda_exact(nu,s)) = nu/P(nu,s)

E(nu,s)
 = (1/nu) * integral_S_V D_chi(H_z,s)*G2*(V·H) dω_H
 <= (1/P(nu,s)) * integral_S_V D_chi(H_z,s)*(V·H) dω_H
 <= (1/P(nu,s))
       * integral_H_plus D_chi(H_z,s)*max(V·H,0) dω_H
 = 1.
~~~

Every integrand is nonnegative, so 0<=E(nu,s)<=1 for EVERY nu>0.
The preceding finite weighted bound also establishes integrability.
The restriction to S_V and the positive half-vector dot are essential;
no below-surface reflection is counted as a positive incident direction.

#### 5.6.5 Nonnegative layering and the base's own energy bound

For 0<=q<=1, the proven E bounds give 0<=A<=1 for all positive nv,nl.
Consequently f_layer is nonnegative wherever the original f_base is.
At fixed nu, A<=1-q*E(nu,s). If the original base has a valid
per-channel directional reflectance bound B_base_energy,j, then:

~~~text
R_layer,j(nu)
 := integral_L_plus f_layer,j(N,V,L)*nl dω_L

R_layer,j(nu)
 <= (1-q*E(nu,s))*B_base_energy,j + C_j*E(nu,s)
 <= (1-q*E(nu,s))*B_base_energy,j + q*E(nu,s)
 <= max(B_base_energy,j,1).
~~~

If the base is independently proved energy-bounded by one, this yields a
bound of one for the layer. R024 declares an energy-aware real-time base,
not an unconditional exact energy-conservation theorem; this correction
does not strengthen that declaration or assume a missing base bound.
Independent emission is outside this reflected-energy inequality.

All results above concern the selected IDEAL reference only. Finite f32,
P/E quadrature or LUT errors, endpoint handling, actual backend behavior and
the complete R027 finite-scene bound remain OPEN under §14. No clamp,
cosine cutoff or source restriction closes them implicitly.

#### 5.6.6 Exact ideal corollary for source sheen roughness 0.5

Recorded on 2026-11-04 under
HAL-R029-BED-ROUGHNESS05-EXACT-P-DELTA-Q-COROLLARY-S0-01, with these exact
identities selected by the delegated architectural reviewer.
This is an S0 corollary inside the INACTIVE draft, not adoption or executable
material behavior. It applies when the original source sheen roughness is
**exactly s=0.5** in the ideal model, as supplied in
[§2's Bed source facts](#2-intended-subset-and-current-source-evidence).
Then s_eff=0.5, alpha_g=1/4, p=1/(2*alpha_g)=2 and:

~~~text
D_chi(u,0.5) = (3/pi)*(1-u^2)^2
~~~

**Derivation of P from the original hemisphere integral.** For this proof only,
extend the even polynomial D to the full unit sphere as
D_even(H)=(3/pi)*(1-H_z^2)^2. This extension is an integration device, not a
below-surface BRDF or a change to the contributing domain. Put w=V_nu·H and
z=N·H. The identity max(w,0)=(w+|w|)/2, §5.6.1's signed projected integral,
and evenness under H→-H give:

~~~text
P_Bed(nu) = nu/2 + (1/4)*integral_sphere D_even(H)*|w| dω_H

z = nu*w + sqrt(1-nu^2)*sqrt(1-w^2)*cos(psi)
dω_H = dw dpsi,    -1<=w<=1,    0<=psi<2*pi
~~~

The azimuth averages of cos(psi)^2 and cos(psi)^4 are 1/2 and 3/8; odd
powers integrate to zero. With integral_0^1 w dw=1/2,
integral_0^1 w^3 dw=1/4 and integral_0^1 w^5 dw=1/6, the required moments are:

~~~text
J0 := integral_sphere |w| dω_H       = 2*pi
J2 := integral_sphere |w|*z^2 dω_H   = (pi/2)*(1+nu^2)
J4 := integral_sphere |w|*z^4 dω_H   = pi/4 + (pi/2)*nu^2 - (pi/12)*nu^4

integral_sphere D_even(H)*|w| dω_H
 = (3/pi)*(J0 - 2*J2 + J4)
 = 15/4 - (3/2)*nu^2 - nu^4/4

P_Bed(nu) := P(nu,0.5)
 = 15/16 + nu/2 - 3*nu^2/8 - nu^4/16,    0<=nu<=1
~~~

This is a polynomial identity obtained by integration, not a fit to sampled
P values. Separate endpoint substitution gives P_Bed(0)=15/16 and
P_Bed(1)=(15+8-6-1)/16=1. At nu=1/2 it gives 279/256 exactly; at
nu=1/1024 it gives 15/16+2^-11-3*2^-23-2^-44 exactly.

Retained numerical observations from
HAL-R029-PROJECTED-AREA-PE-NUMERIC-DOMAIN-EVIDENCE-01 are compatible with these
identities: reduced GL values at nu=0,1/1024,1/2 were respectively
0.9374999999999997, 0.9379879236220742 and 1.0898437499999745; the independent
original two-dimensional integral at nu=1 gave 0.9999999999999998.
Those are historical binary64 observations with the previously reported
refinement limits, not the derivation, certified enclosures or backend proof.
No numerical calculation was rerun for this documentary corollary.

**Exact Delta and positive Q decompositions.** Factoring the polynomial yields:

~~~text
Delta_Bed(nu) := P_Bed(nu) - nu
 = (1-nu)*(nu^3+nu^2+7*nu+15)/16 >= 0,    0<=nu<=1

P_Bed(nu) = 15/16 + nu*(8-6*nu-nu^3)/16 >= 15/16
~~~

For the first inequality, both factors are nonnegative on the closed interval.
For the second, 8-6*nu-nu^3>=1 and nu>=0. These are exact real bounds, not
bounds on any chosen floating evaluation. For strictly positive nv,nl, the
ORIGINAL correlated-visibility denominator therefore has both exact forms:

~~~text
Q(nv,nl,0.5)
 = nl*P_Bed(nv) + nv*P_Bed(nl) - nv*nl
 = nl*Delta_Bed(nv) + nv*P_Bed(nl)
 = nv*Delta_Bed(nl) + nl*P_Bed(nv)

Q >= (15/16)*nv
Q >= (15/16)*nl
Q >= (15/16)*max(nv,nl) > 0
~~~

Each decomposed expression sums nonnegative ideal terms without the explicit
subtraction in the original Q formula. This establishes a non-cancelling
**ideal algebraic** decomposition only. It selects no polynomial instruction
order or finite evaluation, and proves neither f32 underflow avoidance nor
positivity of computed tiny dots, products, Delta or Q on a backend. The
original noncontributing branches still precede positive-cosine evaluation.

**Exact normal-incidence E reference.** Set nv=1 and mu=nl. Then P_Bed(1)=1
gives Q(1,mu,0.5)=P_Bed(mu). The original half-vector has
nh=sqrt((1+mu)/2), so D_chi(nh,0.5)=(3/pi)*((1-mu)/2)^2. The azimuth is
absent from this integrand; integrating its original measure over 2*pi gives:

~~~text
E(1,0.5)
 = 2*pi * integral_0^1 [D_chi(nh,0.5)*mu/(4*P_Bed(mu))] dmu
 = (3/8) * integral_0^1 [mu*(1-mu)^2/P_Bed(mu)] dmu
~~~

The denominator is at least 15/16 throughout [0,1]. At mu=0 the original
noncontributing rule gives zero, and at mu=1 the Charlie numerator is zero.
There is no division by sqrt(1-nv^2) or an artificial partial-azimuth band.
This is exactly §5.4's unit-color E at normal incidence, not a different
reflectance, an E(0) assignment or a runtime integration prescription.

The previously completed
HAL-R029-NORMAL-INCIDENCE-E-REMAINING-NUMERIC-CLOSURE-02 reported
Gauss-Legendre order256 as 0.029571954662974364 and composite Simpson8192 as
0.029571954662974378, hence the numerical observation
E(1,0.5) approximately 0.0295719546629744. Agreement and refinement of these
binary64 computations are **not a certified error bound**, selected storage
precision, finite f32 evaluator, complete scene bound or GPU/native result.

**Scope and unresolved decisions.** This corollary leaves the full general
selected s domain and s_eff=max(s,0.045) unchanged. It neither absorbs the
original linear sheen color C into P/E nor changes §5.5's two-direction
layering. It selects no shader branch, source-domain restriction, table,
polynomial f32 order, new ABI, cutoff or guard. The original absence rules,
unsupported R026 combination and accepted old material profiles remain intact.
The [§7 proof boundaries](#7-source-actual-host-and-backend-proof-boundaries)
and every [§14 open decision](#14-open-decisions-and-exact-implementation-blockers)
remain in force: R029-E-REPRESENTATION, F32-EVALUATOR, FULL-RADIANCE, TYPED-ABI,
INDIRECT-COMPOSITION, SOURCE-INTEGRATION and OBSERVABLE-CRITERIA are all OPEN.
S1/S2/S3/S4 are not granted by this exact-ideal corollary.

#### 5.6.7 Exact one-dimensional E for source sheen roughness 0.5

Recorded on 2026-11-04 under HAL-R029-BED05-EXACT-ONE-DIMENSIONAL-E-REDUCTION-S0-01,
selected by the delegated architectural reviewer. This is an
exact-ideal S0 identity inside the INACTIVE draft, only for s=0.5. It reduces
the SAME [§5.4 hemisphere reference](#54-e-is-the-reflectance-of-this-exact-unit-color-lobe),
not the material model or its general roughness domain.

**Angular reduction and its domain.** Use §5.4's exact unit V_nu and L_mu_phi,
with 0<nu<=1, 0<mu<=1 and 0<=phi<2*pi. The following A is local to this proof,
not §5.5's base attenuation. Define:

~~~text
A = 1 + nu*mu
B = sqrt((1-nu^2)*(1-mu^2))
d = nu+mu > 0,       c = d^2/2
||V+L||^2 = 2*(A+B*cos(phi))
H_z^2 = c/(A+B*cos(phi))
A^2-B^2 = (nu+mu)^2 = d^2,       A>|B|>=0
~~~

All denominators are strictly positive on this domain. For arbitrary A>|B|,
the substitution z=tan(phi/2), with the angular interval split at the coordinate
pole phi=pi,
gives the first angular integral below. Differentiating it with respect to A
at FIXED B gives the second: locally the denominator stays uniformly positive,
so differentiating under the finite angular integral is valid. Substitute d
only after differentiating sqrt(A^2-B^2):

~~~text
I1 = integral_0^(2*pi) dphi/(A+B*cos(phi))
   = 2*integral_(-infinity)^infinity dz/[(A+B)+(A-B)*z^2]
   = 2*pi/sqrt(A^2-B^2) = 2*pi/d

I2 = integral_0^(2*pi) dphi/(A+B*cos(phi))^2
   = -partial_A I1 = 2*pi*A/(A^2-B^2)^(3/2) = 2*pi*A/d^3
~~~

At B=0 (nu=1 or mu=1), A=d>0 and these are directly the constant-denominator
integrals 2*pi/A and 2*pi/A^2. There is no division by B or endpoint partition.
Using D_chi=(3/pi)*(1-c/(A+B*cos(phi)))^2 yields:

~~~text
integral_0^(2*pi) D_chi dphi
 = (3/pi)*(2*pi - 2*c*I1 + c^2*I2)
 = 6*(1 - 2*c/d + c^2*A/d^3)
 = (3/2)*[4 - 3*(nu+mu) + nu*mu*(nu+mu)]
~~~

The original Q is phi-independent and positive for nu,mu>0. Nonnegativity and
the existing uniform bound justify exchanging integral order. Integrating only
this original Charlie factor, with §5.4's original measure, therefore gives:

~~~text
E(nu,0.5) = (3/8)*integral_0^1
  mu*[4-3*(nu+mu)+nu*mu*(nu+mu)] / Q(nu,mu) dmu,    0<nu<=1

Q(nu,mu) = mu*Delta_Bed(nu) + nu*P_Bed(mu) > 0
~~~

Here P_Bed and nonnegative Delta_Bed are EXACTLY
[§5.6.6's polynomials](#566-exact-ideal-corollary-for-source-sheen-roughness-05).
The nonnegative-factor Q identity is algebraic, not a chosen finite evaluation.
At mu=0 use the original noncontributing value zero. At nu=1 the bracket is
(1-mu)^2 and Q=P_Bed(mu), reproducing §5.6.6's normal-incidence formula exactly.

**Exact continuous grazing limit.** On the ORIGINAL L hemisphere, the
cosine-weighted integrand is uniformly bounded by
[§6.1.1's 4/(5*pi)](#611-exact-ideal-direct-response-bound-for-source-sheen-roughness-05).
That constant is integrable on the finite hemisphere. For each fixed mu>0,
the half-vector and Q have continuous limits as nu approaches zero from above;
the mu=0 boundary has measure zero. Dominated convergence consequently applies
to the original two-dimensional reference. In the reduced expression,
Q(0,mu)=mu*P_Bed(0)=mu*15/16 for mu>0, so cancelling that mu gives:

~~~text
lim_(nu->0+) E(nu,0.5)
 = (3/8)*(16/15)*integral_0^1 (4-3*mu) dmu
 = (3/8)*(16/15)*(5/2) = 1
~~~

Continuity at positive nu follows by the same bound and pointwise continuity.
Thus the positive-domain E has a mathematical continuous extension with limit1
at grazing. The corner nu=mu=0 is not evaluated; its value is immaterial to the
integral. This selects no stored E(0), endpoint interpolation, runtime quadrature
or zero-cosine fragment behavior. All nv<=0/nl<=0 rules remain unchanged.

**Retained numerical concordance, not error certification.** The delegated architectural reviewer supplied
the following 65-digit-quadrature observations (digits shown below). Retained
original L-hemisphere and H-reflection observations from
HAL-R029-PROJECTED-AREA-PE-NUMERIC-DOMAIN-EVIDENCE-01, and the independent
normal-incidence control already recorded in §5.6.6, are compatible with them:

| nu | Supplied one-dimensional observation | Retained original L hemisphere | Retained independent control |
|---|---|---|---|
| 1/1024 | 0.9894643546304061908538896138108347 | 0.9894643548934227 | H reflection: 0.9894643546304059 |
| 1/2 | 0.2096228803467250989500477195679275 | 0.20962288034673043 | H reflection: 0.209622880346725 |
| 1 | 0.0295719546629743795789209943639056 | 0.029571954662974843 | Normal-incidence GL256: 0.029571954662974364; Simpson8192: 0.029571954662974378 |

These are historical/supplied numerical observations with their prior finite
refinement limits, not rigorous enclosures. No numerical process or prior 2D
experiment was rerun for this documentary proof.

The exact reduction leaves the selected general s domain, correlation, layer,
source factors and [§7.2 reference boundary](#72-proposed-source-host-and-ideal-reference-semantics-s0-only)
unchanged. Every [§14 decision](#14-open-decisions-and-exact-implementation-blockers)
remains OPEN, particularly R029-E-REPRESENTATION: no finite P/E representation,
LUT, error tolerance, storage precision, f32 operation order, ABI, supported
source/backend domain or radiance certificate is selected. R029+E/G remains
UNSUPPORTED; R029 remains Draft / INACTIVE, with no S1/S2/S3/S4 grant.

#### 5.6.8 Exact logarithmic grazing asymptotic for source sheen roughness 0.5

Recorded on 2026-11-04 under HAL-R029-BED05-E-GRAZING-LOGARITHMIC-ASYMPTOTIC-S0-01,
selected by the delegated architectural reviewer. This is an
INACTIVE exact-reference proof for s=0.5 only. Use
[§5.6.7's original one-dimensional E](#567-exact-one-dimensional-e-for-source-sheen-roughness-05),
write t=nu in (0,1], mu in [0,1] and c0=15/16; log below is the natural logarithm.

**Exact polynomial identities.** With the unchanged P_Bed and Delta_Bed, let:

~~~text
N(t,mu) = 4-3*(t+mu)+t*mu*(t+mu)
F(t,mu) = mu*N(t,mu)/Q(t,mu)
E(t,0.5) = (3/8)*integral_0^1 F(t,mu) dmu

H(t,mu) = 15-t*mu*(6+mu^2-mu*t+t^2)
Q(t,mu) = (mu+t)*H(t,mu)/16
~~~

To check the factorization directly from [§5.6.6](#566-exact-ideal-corollary-for-source-sheen-roughness-05),
expand 16*Q=15*(mu+t)-6*t*mu*(mu+t)-t*mu*(mu^3+t^3) and use
mu^3+t^3=(mu+t)*(mu^2-mu*t+t^2). On the closed unit square,
0<=mu^2-mu*t+t^2<=2, hence 7<=H<=15 and |H-15|<=8*t*mu.
This identity also holds at t=mu=0; positivity of Q is asserted only when
t+mu>0. It bounds the ideal bracket, not finite f32 evaluation or underflow.

Let F0(mu)=(4-3*mu)/c0, including its continuous-in-mu value at mu=0.
This auxiliary limiting expression is not a new reflected-boundary evaluation.
For t>0, including mu=0, expansion of (4-3*mu)*Q-c0*mu*N gives EXACTLY t*K:

~~~text
F0(mu)-F(t,mu) = t*K(t,mu)/(c0*Q(t,mu))

K(t,mu) = [60-24*mu^2+3*mu^3-4*mu^4+3*mu^5
           -24*mu*t+3*mu^2*t-4*mu*t^3+3*mu^2*t^3]/16
K(0,0) = 15/4
~~~

For example, after cancelling the t-independent terms, sixteen times that
numerator is t*[60-24*mu^2+3*mu^3-4*mu^4+3*mu^5
-24*mu*t+3*mu^2*t-4*mu*t^3+3*mu^2*t^3], verifying every coefficient.

**Uniform bounded remainder.** The coefficient sums in K, together with
mu,t<=1, give |K-15/4|<=(17/8)*(mu^2+t*mu). Consequently:

~~~text
|K-H/4| <= (17/8)*mu^2+(33/8)*t*mu
          <= (33/8)*mu*(mu+t)

F0-F = (64/15)*t/(mu+t) + R(t,mu)
R(t,mu) = t/(mu+t) * [256*K/(15*H)-64/15]
|R(t,mu)| <= t/(mu+t) * 256/(15*7) * |K-H/4|
           <= (352/35)*t*mu <= (352/35)*t
~~~

These are uniform real inequalities for 0<t<=1 and 0<=mu<=1, not a numerical
fit or an interchange of a singular derivative with the integral. Since
(3/8)*integral_0^1 F0(mu) dmu=1, integration now gives:

~~~text
1-E(t,0.5) = (8/5)*t*log((1+t)/t) + r(t)
|r(t)| <= (3/8)*(352/35)*t*integral_0^1 mu dmu = (66/35)*t

E(t,0.5) = 1-(8/5)*t*log(1/t)+O(t),    t->0+
~~~

The last step uses 0<=t*log(1+t)<=t^2. For the mathematical continuous
extension E_cont(0)=1 established in §5.6.7, the endpoint difference quotient
(E(t,0.5)-1)/t=-(8/5)*log(1/t)+O(1) tends to -infinity. Thus E is continuous
at grazing but has no finite right endpoint slope. This statement concerns
that difference quotient, with no derivative/integral exchange. It selects no
stored E(0) and changes no nv<=0/nl<=0 fragment branch.

**Retained observation and scope.** At t=1/1024 the leading term
(8/5)*t*log(1/t) is exactly log(2)/64. The retained observation
E=0.9894643546304061908538896138 gives a deficit approximately0.0105356453695938.
For an independent consistency check, log(2)=2*integral_0^(1/3) dz/(1-z^2)
implies 2/3<log(2)<3/4. Together with 0<log(1+t)<t, the full logarithmic term
at this t lies in (1/96,3/256+1/655360). The observed deficit lies in that
interval, whose width 1/768+1/655360 is less than 66/(35*1024). It is therefore
compatible with the proved remainder bound. This single uncertified finite
observation does not prove the coefficient; the exact identities and inequalities
above do. No prior numerical integration or other experiment was rerun.

Continuity on the extended compact domain permits finite uniform-in-nu linear
approximation in principle. Its grazing error must nevertheless account for
this logarithmic boundary layer; a finite endpoint-slope assumption is invalid.
No grid, transform, table, polynomial, size, accuracy threshold or runtime method
is selected. The rational constants bounding the EXACT reference remainder are
not f32, storage or backend acceptance tolerances. The general s domain,
source factors, correlation/layer and [§7.2 boundary](#72-proposed-source-host-and-ideal-reference-semantics-s0-only)
remain unchanged. All seven [§14 decisions](#14-open-decisions-and-exact-implementation-blockers)
stay OPEN, including E-REPRESENTATION and F32-EVALUATOR; R029+E/G remains
UNSUPPORTED, and R029 remains Draft / INACTIVE with no S1/S2/S3/S4 grant.

## 6. Lighting, source bounds and HDR

### 6.1 Actual R025 direct lighting

Replace only the material kernel in a FUTURE admitted new-profile draw:

~~~text
direct_rgb =
    sum_j f_layer(N,V,L_j) * max(0,N·L_j)
          * E_incident_j * V_shadow_j

scene_rgb = direct_rgb + independent_emission
            + separately qualified indirect, if any
~~~

R025 still owns all directional/point/spot sources, units, finite support,
coincident-point branch, real current caster/receiver geometry, cascade fit,
depth/XY/PCF qualification, Mask casters and actual map content. Each of the
nine comparison results and the complete shadow factor remain unchanged.
No sheen factor creates light or changes source intensity, exposure or shadows.

The source certificate must bound the COMPLETE new material, not reuse an old
base-only radiance witness. Let B_base,j be a valid bound on
f_base,j*nl over the actual admitted source/evaluator domain. Where the §5.5
conditions have independently established 0<=A<=1:

~~~text
f_layer,j * nl <= B_base,j + C_j * B_chi
~~~

The EXACT reference now has the uniform cosine-weighted bound
B_chi_ref=D_max(s)/(4*P_min) from §5.6, valid for all positive nv/nl without
any grazing eta. This proves ideal finiteness, not a tight practical light
certificate. A future implemented B_chi must rigorously include its P/E/f32
representation and evaluation error over the actual declared source domain.
The integrated E<=1 theorem does not bound a punctual-light peak by one and
does not by itself establish the complete scene RGB<=60000 obligation.

Apply the original directional incident bound and point/spot
E_incident<=I/(radius_m^2), including their source support/spot factors and
0<=V_shadow<=1. Sum every light and independent emission, plus any separately
qualified indirect bound. A black texel, mean roughness, scalar factor cap,
shadow test, or finite incident term alone does not bound the sum.

R027 continues to require every possible covered ideal scene RGB component to
be finite, nonnegative and <=60000 BEFORE writes, under the exact complete
source/material/model/view/light/evaluator association. F32/backend margins are
a separate declared conformance obligation. No late 60000 clamp, exposure
reduction, per-pixel CPU rendering, GPU readback or favorable average repairs
a missing bound.

#### 6.1.1 Exact ideal direct-response bound for source sheen roughness 0.5

Recorded on 2026-11-04 under
HAL-R029-BED-ROUGHNESS05-DIRECT-RADIANCE-IDEAL-BOUND-S0-01, selected by the
delegated architectural reviewer. This is an INACTIVE S0
source-specific corollary, not a new-profile executable source certificate.
It uses only the exact s=0.5 case in
[§5.6.6](#566-exact-ideal-corollary-for-source-sheen-roughness-05) and the
original linear Bed factors supplied in
[§2](#2-intended-subset-and-current-source-evidence); no private source is
remeasured or admitted here.

**Uniform cosine-weighted sheen bound.** For positive nv,nl, the qualified ideal
half-vector has 0<=nh<=1. Thus D_chi(nh,0.5)=(3/pi)*(1-nh^2)^2 is nonnegative
and at most 3/pi. The exact inequalities Q>=nl*P_Bed(nv)>0 from
[§5.6.3](#563-positive-reciprocal-visibility-and-weighted-lobe-bound) and
P_Bed(nv)>=15/16 from §5.6.6 give the complete chain:

~~~text
0 <= f_chi(nv,nl,0.5)*nl
   = D_chi(nh,0.5)*nl / (4*Q(nv,nl,0.5))
   <= D_chi(nh,0.5) / (4*P_Bed(nv))
   <= (3/pi) / (4*15/16)
   = 4/(5*pi)
~~~

The original §5.1 noncontributing branches give zero reflected response and
satisfy the same bound. It is finite, nonnegative and uniform in ideal view/light
angles, with no positive minimum cosine cutoff. It bounds a cosine-weighted
direct response, not an unweighted BRDF or a claim that the complete layered
BRDF integrates to one.

**Conditional combined base-plus-sheen bound.** For each actual material, let
re=max(original_base_roughness*min_actual_MR_green,0.045) be the effective base
roughness lower bound from its complete original R025 source validation.
The original MR role, factor and minimum must actually be qualified; min=1 is
the original genuine-absence default, not permission to omit present MR data.
This re is independent of the source sheen roughness s=0.5. Current
[R025 source validation](../../src/three_d/direct_shadow.rs), lines498–552,
bounds the qualified old base's per-channel f_base,j*nl by 1+re^-6. That envelope
remains conditional on its original source/evaluator assumptions; it cannot
automatically be relabeled an executable certificate for the new profile.

Under those assumptions, 0<=C_j<=1 and §5.5's independently established 0<=A<=1,
nonnegativity of both base and sheen gives:

~~~text
0 <= f_layer,j*nl
   = A*(f_base,j*nl) + C_j*(f_chi*nl)
   <= (1 + re^-6) + C_j*4/(5*pi)
~~~

For the supplied original Bed materials, C_j is the unchanged linear source
factor 0.19999998807907104 in each channel. No factor repair or new storage,
rounding, evaluation or shader instruction order is selected by this identity.

**Conditional complete direct-only scene bound.** For each channel j, let the
ACTUAL admitted R025 source inputs define:

~~~text
Incident_j = directional_irradiance_j
             + sum_point intensity_j/(radius_m^2)
             + sum_spot intensity_j/(radius_m^2)

SceneIdealUpper_j = ((1 + re^-6) + C_j*4/(5*pi))*Incident_j
                   + Emissive_j
~~~

An absent source contributes zero. Each positive radius and all source units
remain the qualified R025 inputs. Source-support/spot multipliers and every
shadow visibility stay in [0,1], so they cannot increase each source's incident
upper bound. The actual nonnegative incident magnitudes need not be <=1 and
remain fully multiplied and summed.
Summing EVERY actual direct source and adding the unchanged independent emission
therefore bounds direct-only scene RGB for that material/packet. Every material/
packet and channel must satisfy SceneIdealUpper_j<=scene.hdr.maximum_rgb[j]
with the actual declared maximum itself <=60000 before candidate writes. No
average over materials, light omission, exposure change, clamp or shadow change
can establish that predicate.

Actual Bed Incident_j, validated MR minima/re, Emissive_j and scene certificate
values have not been supplied to this corollary. Consequently it establishes
neither SceneIdealUpper_j<=60000 nor actual Bed source admission. This is ideal
reference arithmetic only: no finite f32 error, tiny-dot safety, P/E approximation,
source-byte proof or N1/N4 Device/backend result is included.

The special case does not narrow the general R029 roughness domain or replace
§5.6's general bound. R029+E/G stays UNSUPPORTED; this direct-only expression
does not qualify indirect transport. The
[§14 FULL-RADIANCE gate](#14-open-decisions-and-exact-implementation-blockers)
still requires the actual implemented P/E/D/Q/A and base error margins, complete
direct-light/coverage/shadow association and, only if separately enabled, a
qualified indirect bound. All seven §14 decisions remain OPEN. R027's original
FP16/Image, <=60000 cap and source/currentness rules are unchanged; S1/S2/S3/S4
are not granted.

### 6.2 R026 composition is not silently inherited

The ideal angular reference for reflected indirect light uses the SAME
f_layer integrated against its actual incident angular radiance and positive
cosine. Both the per-light A(nv,nl) dependence and the Charlie lobe matter.
Emission remains separate.

Existing R026 E diffuse/specular assets and GGX split-sum LUT were qualified
for their declared old response. G6 supplies its declared irradiance model,
not a new Charlie angular transport certificate. They cannot be relabeled as
sheen preintegration or automatically implement the two-direction A integral.

**OPEN FOR ARCHITECTURAL DECISION — R029-INDIRECT-COMPOSITION.** Complete E/G transport
inputs/approximation, normal/world association, sheen integration, base
attenuation, AO placement, source bounds, finite resources and actual backend
proof must be selected coherently before mixing R029 with R026. AO never
attenuates independent direct light or emission; existing V1 AO behavior stays
unchanged.

Until that complete composition is separately qualified, a new-profile request
with E and/or G present is typed Unsupported. Do not omit sheen, invent
environment light, reuse a GGX LUT, or choose the external note's simplified
view-only scaling as a fallback. C=0 does not authorize ignoring an otherwise
unsupported explicit source/profile combination.

With D/E/G all genuinely absent, reflected contribution is zero. Valid
independent emission retains its original meaning; sheen supplies no ambient.

### 6.3 R027 output remains the existing output

Preserve actual scene Rgba16Float and Depth32Float at N1/N4, tone each covered
HDR sample before averaging, fixed linear Rgba16Float final[1], existing
target-bound ordinary Image and one existing same-Queue publisher. Alpha,
coverage and native output encoding remain the original contracts.

A material change is not permission to change exposure, tone curve, resolve
order, attachment format, final registration or serial identity. The reported original Bed reference scene's
authored lights/exposure are not tuned by this specification.

## 7. Source, actual-host and backend proof boundaries

The adopted 2026-11-03 R024/R028/HAL370 separation remains mandatory:

| Boundary | Required evidence and limit |
|---|---|
| Immutable source | Full original factors/geometry/UV/role/mip/sampler/A-B-C or absence/normal-domain and complete radiance proof on actual changes; exact live capture reuse thereafter |
| Actual host input | Finite supported camera/object/view/normal/metric data, actual normalization/packing/other performed host arithmetic, source association, current Context/Device/target and finite reservations |
| Genuine R025 work | Current caster/receiver/cascade/depth/XY/PCF and Mask dependencies, all required faces/draws; no stale content |
| New numerical implementation | Exact relation of implemented D/P/exact-visibility/E/scaling to the selected ideal reference, representable guarded operations and a complete bound in the declared source/backend domain |
| Backend | Controlled derivative-valid shader, actual supported formats/features/sample/filter/limits and predeclared observable versioned N1/N4 conformance |
| Native product | Separately observed canonical App::run output, quality/performance and JC visual acceptance |

No new profile may restore mandatory production O(vertex/triangle) GPU clip,
helper, reciprocal-w or UV-footprint simulation per FOV. Source/actual-host
proof is not weakened, and a finite GPU test set is not a universal precision
theorem. No per-frame readback, simulator, S3 replay or hidden normal repair
may replace these boundaries.

The original five-role derivative/sampling order, including derivatives before
divergent Mask discard and controlled gradient sampling, remains R028's.
A sheen-only factor branch must not invalidate those derivatives or bypass
source validation.

### 7.1 Inactive conditioning note: indistinguishable packed directions

Recorded on 2026-11-04 under
HAL-R029-PACKED-DIRECTION-INDISTINGUISHABILITY-S0-PROOF-01, selected by the
delegated architectural reviewer. This is an exact mathematical
conditioning example inside the INACTIVE draft. It changes no accepted source,
actual-host input interpretation, shader or backend contract.

**Exact ideal inputs.** Let v=2^-149, d=v/4=2^-151, N=(0,0,1) and
x=sqrt(1-v^2). Choose identical original sheen factors with s=0.5 and define:

~~~text
V  = ( x,                         0, v)
L0 = (-x,                         0, v)
L1 = (-sqrt(1-v^2-d^2),            d, v)
~~~

Each squared length is exactly1, all radicands are positive, and
N·V=N·L0=N·L1=v>0. Both pairs therefore belong to the mathematical positive
unit-direction domain of §5.1. This says nothing about their reachability from
an actually admitted scene, camera or host transformation.

**Identical independent componentwise binary32 packing.** Assume IEEE
round-to-nearest-even with gradual subnormals for this proof only. The smallest
positive value is v; 0<d<v/2 lies strictly below its midpoint with +0 and rounds
to +0. Immediately below1, the adjacent binary32 value is1-2^-24, with midpoint
1-2^-25. Put q=v^2+d^2=(17/16)*v^2. Since 0<q<2^-25<1:

~~~text
1-2^-25 < 1-q < sqrt(1-q) <= sqrt(1-v^2) < 1
~~~

Both x magnitudes thus round to1, and their negatives round to-1. The v and
N components are exactly representable. Raw binary32 bits are identical:

~~~text
N32       = (00000000, 00000000, 3f800000)
V32       = (3f800000, 00000000, 00000001)
L0_32=L1_32=(bf800000, 00000000, 00000001)
~~~

The packed N/V/L, nv/nl and material factors contain no distinction between
these two ideal pairs. No claim about hardware flush behavior is involved.

**Different exact ideal responses.** For L0, S0=(0,0,2v), H=N, t=0 and
D_chi=W0=0. For L1, let r=sqrt(1-v^2-d^2) and
dx=x-r=d^2/(x+r)>0. Then S1=(dx,d,2v), and the same original lobe gives:

~~~text
t1 = (dx^2+d^2)/(dx^2+d^2+4*v^2) > 0
T  = Delta_Bed(v)+P_Bed(v) = 15/8 - 3*v^2/4 - v^4/8 > 0
W1 = (3/pi)*t1^2/(4*T) > 0
~~~

Here Q(v,v)=v*T by [§5.6.6](#566-exact-ideal-corollary-for-source-sheen-roughness-05),
so the cancellation of v is an exact identity, not a new evaluator. Since
d^2=v^2/16 and dx>0, t1>1/65. The architect-supplied illustrative estimates
t1 approximately1/65 and W1 approximately0.00003013584721266657 are not a
certified numerical evaluation; no numerical process was run for this note.
Strict positivity follows from the exact formulas alone.

Any deterministic function with only these packed direction inputs, identical
material data and all other distinguishing inputs held equal returns one common
value y. For finite y, the triangle inequality proves:

~~~text
W1 = |W1-W0| <= |y-W0| + |y-W1|
max(|y-W0|, |y-W1|) >= W1/2
~~~

A nonfinite y cannot equal either finite ideal response. This is a lower bound
against the two PREPACKING unit-lobe W values, not against a complete final pixel;
actual lighting, base attenuation, emission, coverage and tone are not compared.

**Conditional scope and owner decision.** This excludes exact fidelity to BOTH
arbitrary ideal continuous prepacking directions from this identical packed
state alone. It does not show that either pair is generated by accepted glTF,
source camera/model or actual host arithmetic, that accepted law selects that
prepacking target, or that Halcyon admission or Metal has failed. A reference
defined on well-defined ACTUAL packed inputs has one common input here and may
be evaluated correctly under its expressly selected direction interpretation;
raw packed components are not automatically exact unit vectors.

[R024 §8.2.4](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md),
[R028 §15](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md) and
[HAL370 §9](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md) retain exact source,
actual host/packing and supported observable backend boundaries. The delegated
architectural reviewer must select the finite input/reference domain and authorized error
criteria before [§14 R029-F32-EVALUATOR](#14-open-decisions-and-exact-implementation-blockers)
can close; reserved JC judgment and separate adoption remain. No source
whitelist, cosine cutoff, repair, implementation or ABI is selected here.

The completed read-only
HAL-R029-BED05-SCALED-Q-AND-HALFVECTOR-NUMERIC-CANDIDATE-EVIDENCE-01 reported
64 cosine pairs and192 physical directions: scaled T/U removed all modeled
NaN/infinity, while packed-input information loss and ideal-bound overshoots
remained. That finite simulation is neither actual Metal evidence nor a
certified all-domain error bound. All seven §14 decisions remain OPEN,
R029+E/G remains UNSUPPORTED, and S1/S2/S3/S4 are not granted.

### 7.2 Proposed source, host and ideal-reference semantics (S0 only)

Recorded on 2026-11-04 under HAL-R029-SOURCE-HOST-IDEAL-REFERENCE-SEMANTICS-S0-01,
selected by the delegated architectural reviewer. This inactive
logical boundary follows [R024 §8.2.4](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md),
[R028 §15](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md) and
[HAL370 §§7/9](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md); it modifies none
of those parents and grants no executable work.

1. **Original source.** Original glTF geometry/material/factors, normals, mip,
   sampler and frame meaning remain immutable at their existing owners, with
   complete original ideal source-domain certificates and exact provenance.
   No source repair, whitelist or positive-cosine cutoff follows from §7.1.

2. **Actual host input.** The changed-input owner validates actual camera/object/
   view/normal data, source and revision association, source derivation,
   normalization, packing, finite submitted bytes and finite capacities, with
   the real error bounds for arithmetic it actually performs. Exact still-live
   qualified source/Context/generation/lease captures remain reusable. Invalid
   or unrepresentable actual host data retains its true refusal; no universal
   per-frame GPU prediction sweep is introduced.

3. **Ideal R029 reference.** D/P/Q/E and the layer functions retain their exact
   real meanings in §5. For separately predeclared numerical comparisons, the
   reference interprets source-qualified HOST-SUBMITTED finite values under
   their specified ideal geometry, transform and raster semantics. Qualified
   directions are mathematically normalized where that ideal reference requires
   it; raw packed N/V/L are not presumed exactly unit. The reference is not
   determined by arbitrarily unknown pre-host continuous directions and is not
   redefined by whichever erroneous intermediates a GPU happens to compute.
   Source-to-host derivation and packing errors remain separately attributable
   and rigorously checked at their owning boundary. A comparison to an ideal
   reference based on submitted values does not erase or certify away that
   source-to-host obligation. Hidden fragment inputs are not thereby observable.

4. **Backend conformance.** Controlled WGSL/Naga/wgpu/Device execution in the
   actually supported Metal N1/N4 envelope requires independent observable
   output conformance to predeclared source-grounded predictions under authorized
   comparison criteria. A finite set establishes no universal pointwise GPU
   precision theorem. Output pixels do not directly reveal hidden normal, dot,
   Q or other arithmetic values. A known supported-backend failure remains a
   failure; the reference cannot be fitted to its erroneous intermediates.

5. **Scope of the prepacking impossibility.** Section 7.1 proves only that
   identical finite packed data cannot reconstruct every differing ideal
   prepacking direction. It neither accepts changed source factors or
   unrepresentable host data, nor proves its theoretical witness reachable from
   admitted original reference-scene sources. It authorizes no guessed epsilon, correction shader,
   per-frame readback or new source-invalid predicate.

6. **Remaining numerical S1 decisions.** This selects the logical reference
   boundary only. Exact f32 WGSL operations, rounding/underflow/normalization,
   finite supported input domain, attributable error treatment and authorized
   observable criteria remain unselected by the delegated architectural reviewer. Every
   [§14 decision](#14-open-decisions-and-exact-implementation-blockers) stays OPEN:
   R029-F32-EVALUATOR, E-REPRESENTATION, FULL-RADIANCE, TYPED-ABI,
   INDIRECT-COMPOSITION, SOURCE-INTEGRATION and OBSERVABLE-CRITERIA. The split is
   not a certified shader/backend profile, source/host waiver, parent amendment
   or S1/S2/S3/S4 adoption. R029 remains Draft / INACTIVE / NOT BINDING;
   R029+E/G remains UNSUPPORTED.

## 8. Finite resources and lifetime

No block offset, size, padding use, enum representation, binding, WGSL or
public Rust signature is selected by this draft. In particular, current
MaterialBlock64, role_mode reserved words, five-role arrays and old pipeline
identity are not spare storage granted to R029.

Before future implementation, the material owner must supply the exact typed
profile/descriptor/program/ABI design and checked limits. Account for every
new material generation, factor/identity/certificate metadata, packed uniform,
alignment, binding/pipeline/program variant, P/E representation and any
associated CPU/GPU storage/upload/staging. Account for all old/pending/
committed/last-good/in-flight multiplicity through existing Context3D ledgers.
A P or E table cannot become an uncharged global object.

Reuse the existing target-local finite capacity and same Device/Queue owner;
no new renderer, source registry, scheduler, thread, recovery clock or extra
submit/poll is granted. Unchanged source/view/target causes no work.
Warm view changes retain compatible immutable material resources and refresh
the genuine current geometry/shadow dependencies.

Old and new material/program/P/E-resource generations retain their exact leases.
No source change overwrites a sampled, committed, compatible-last-good or
incomplete in-flight generation. Covering submission, observed completion,
registration/sampling ownership and legal retirement remain distinct.
No callback may create an ownership cycle or manufacture successful currentness.

An absent sheen extension retains the original V1 shader/layout/path and adds
no sheen scan, P/E lookup, uniform, binding, allocation, branch, pass or cadence
to old materials. Mixed-scene support must prove that separation rather than
route all draws through a slower unselected material family.

### 8.1 Proposed typed material-kernel ownership and routing (S0 only)

Recorded on 2026-11-04 under HAL-R029-MATERIAL-KERNEL-TYPED-ROUTING-S0-CONTRACT-01,
selected by the delegated architectural reviewer. These are
proposed logical obligations under §1, not an adopted interface or implementation.
The labels below name logical alternatives, not Rust type/function spellings,
enum representation, discriminant values or packed storage.

1. **Material-source ownership.** The future material kernel is an explicit
   immutable typed discriminator owned by each material source. Its closed logical
   alternatives here are **BaseR024** and **FactorOnlySheenR029**, the latter
   identifying the already proposed PbrMetallicRoughnessSheenF32V1 profile.
   Kernel identity is independent of material source identity/revision, V1/V2
   sampling family, scene-wide D/E/G presence, alpha/sidedness and native
   target/sample selection. Texture presence, product name, geometry, an enum
   zero value or scene light cannot automatically select the kernel.

2. **Distinct explicit input.** Existing PbrMaterialDescriptor3D and
   admit_pbr_material remain exactly the current BaseR024-only entry. R029 needs
   a distinct typed input contract retaining ALL unchanged base role, factor,
   presence, sampling and source-identity/revision meanings, plus separately
   present linear sheenColorFactor[3] and independent sheenRoughnessFactor.
   Actual extension presence survives C=0. Genuine extension absence remains
   BaseR024; there is no implicit conversion or default-to-sheen. Exact new Rust
   input/function/handle spellings and their representation remain S1 decisions.

3. **Admission and generations.** Each admitted material retains its own immutable
   typed kernel, validated factors, original five-role leases and actual source
   revisions. R029 adds its factor/domain and complete scene/capability proof
   gates; C=0 never skips invalid present-role, frame, normal or source-certificate
   checks. Existing material/source identity, strict revision/previous and Context
   rules stay with their current owner. A profile change must be a truthful
   distinct generation, never an alias or reinterpretation of an older handle.
   A failed successor cannot mutate an old complete lease or old front.

4. **Per-packet routing in one scene.** One ordered snapshot may contain both
   qualified logical kernels through their own material handles. Each packet's
   fixed kernel and sampling family, together with the scene's exact D/E/G
   presence, determine its controlled program variant. There is no hidden
   scene-wide material mode. Draw order, geometry/source leases, packet/snapshot
   identity and the existing encoder, Queue, target and HDR publisher are retained.
   Any R029 packet with E or G remains UNSUPPORTED until separately accepted R026
   composition, including C=0. Do not drop or reinterpret such a packet to publish
   a successful prefix. Existing D-enabled legacy restrictions remain unchanged.

5. **Explicit shared-layout compatibility.** Selected programs must have COMMON
   compatible group0 View80, group1 Object112 and applicable group3 D/E/G
   contracts. Before sharing a bind group, source/program admission must prove
   exact structural layout identity, including its binding types, visibility,
   dynamic-offset and minimum-size contract. Group3 absence remains absence when
   the selected lighting layout has none. Group2 may vary by typed kernel only
   after a complete explicit ABI and actual Device checks are selected. Current
   first-program View/Object/group3 creation is NOT such a mixed-kernel proof.
   No fifth group, old Material64 reserved-word use or R025/R026/R027 shader or
   lighting reinterpretation is granted.

6. **BaseR024 keeps its exact old path and cost.** Its old shader source,
   MaterialBlock64, five texture and five sampler bindings, code path, program
   family/cache, absence behavior, uploads, full source admission and per-draw costs remain
   IDENTICAL. A base-only scene adds no sheen evaluation, lookup, uniform,
   texture, bind group, program creation, cache lookup or cadence. Kernel
   discrimination is source typed routing, not a new branch in the old fragment
   shader. Future exact evidence must compare old-only behavior and charges and
   preserve each base packet's old path in mixed scenes; new-profile costs cannot
   be silently imposed on every old material.

7. **Existing resource and lifetime owners.** New R029 materials/programs and any
   eventual P/E resources use existing Context3D material, program, pipeline,
   texture and uniform owners, with unique actual charges, source revisions,
   in-flight and last-good generations. No hidden global, uncharged default P/E
   data, extra Queue, source registry or material-driven target authority is
   admitted. The source-proof/actual-host/backend boundary remains unchanged.
   Exact new uniforms, P/E representation, shader arithmetic, Device/capacity
   costs and error precedence are not selected by these logical obligations.

8. **Inactive gates remain explicit.** R029 stays Draft / INACTIVE / NOT BINDING;
   S1/S2/S3/S4 remain NOT GRANTED. Every
   [§14 decision](#14-open-decisions-and-exact-implementation-blockers) is OPEN.
   In particular, R029-TYPED-ABI is not resolved until exact descriptors/handles,
   all material-bind layouts, source error precedence, GPU bytes, program-key/cache
   identity and mixed-packet resource bounds are formally selected and independently
   reviewed. The [exact s=0.5 corollary](#566-exact-ideal-corollary-for-source-sheen-roughness-05)
   is ideal mathematics, not runtime code or a new restriction of the general
   selected source domain. No native Bed support is inferred.

The following matrix is logical and conditional for every R029-containing row;
it grants no current source/backend admission or completed ABI proof. V1/V2 below
means the per-material sampling family, not the logical material kernel. Every
present role must still match its material's sampling family.

| Packet kernels | Per-packet sampling | Scene presence | Proposed disposition |
|---|---|---|---|
| BaseR024 only | V1, V2 or their currently qualified mixture | Any currently qualified old D/E/G layout | Exact old source, program, layout, output and cost path; no R029 work |
| FactorOnlySheenR029 only | V1 or V2; may differ between packets | D=E=G=0 | Candidate only after full profile admission; valid independent emission, no reflected light or dummy D |
| FactorOnlySheenR029 only | V1 or V2; may differ between packets | D=1, E=G=0 | Candidate after all R029 gates; original R025 light kinds and genuine current shadows retained |
| BaseR024 and FactorOnlySheenR029 | V1/V2 may differ between packets | D=0 or1, E=G=0 | Candidate only with the explicit common-layout proof, per-packet program/group2 routing and complete resource bounds; original order and one publisher |
| Explicit FactorOnlySheenR029 with C=0 | V1 or V2 | D=0 or1, E=G=0 | Retain new kernel/source identity and all validation; ideal sheen=0 and A=1 do not convert it to BaseR024 |
| Any FactorOnlySheenR029 packet, including C=0 | V1 or V2 | E=1 and/or G=1, D arbitrary | UNSUPPORTED until separate R026 composition acceptance; no omitted packet, sheen-as-base fallback or partial publication |

**Concordance with current owners, not an implemented R029 route.** The completed
HAL-R029-ACTUAL-MATERIAL-ABI-PROGRAM-ROUTING-SEAM-CENSUS-01 at source eed6b735
established the following exact seams; the code remains unchanged by this draft.

| Existing source seam | Current fact and still-unselected future contract |
|---|---|
| [material_api.rs](../../src/three_d/material_api.rs), lines935–1068; [pbr.rs](../../src/three_d/pbr.rs), lines366–518 and599–725 | Base descriptor/admission, fixed source, Material64 and controlled material leases exist; distinct R029 input/kernel/handle storage and error precedence need exact S1 selection |
| [raster.rs](../../src/three_d/raster.rs), lines3049–3072,3604–3616 and4108–4165; [context.rs](../../src/three_d/context.rs), lines1878–1938 | Current program routing/cache uses sampling and D/E/G, then per-draw material bindings; exact new kernel program-key/cache identity is OPEN |
| [raster.rs](../../src/three_d/raster.rs), lines3727–3732,3846–3863 and4541–4624 | View/Object and lighting bindings currently come from the first compatible old program; new common structural identity and kernel-specific group2 bytes/bindings are not yet selected |
| [snapshot.rs](../../src/three_d/snapshot.rs), lines89–141 and153–230; [viewport.rs](../../src/three_d/viewport.rs), lines1205–1233 and2575–2632 | Immutable packets/material leases, whole source capture, revisions and front ownership already exist; the new typed kernel association must remain source-bound through them without a new scene/target owner |

This logical selection does not close R029-E-REPRESENTATION, F32-EVALUATOR,
FULL-RADIANCE, TYPED-ABI, INDIRECT-COMPOSITION, SOURCE-INTEGRATION or
OBSERVABLE-CRITERIA. Existing mathematical, source, normal/frame, shadow, HDR,
finite-resource and native/backend obligations elsewhere in this draft are not
relaxed, and current accepted parent contracts are unchanged.

## 9. Admission, transaction and front truth

All original source/actual-host/capability/target/resource checks and the new
profile/domain/certificate checks complete before candidate writes.
The current owner admits one complete immutable tuple; no application shader,
live semantic callback or material patch is consulted during preparation.

A changed requested material remains distinct from committed content.
A compatible failed successor retains the original complete last-good
material/source/HDR/target tuple with its typed new-request failure.
An incompatible/retired target withdraws under the original owner.
No sheen failure becomes an old image relabeled as the requested material.

The shadow/scene/tone commands use the existing admitted encoder/Queue path.
Only actual successful submit/swap/registration permits the corresponding
complete commit. Pre-submit cancellation, submitted-but-unpublished failure,
current sampling and final Queue completion retain their existing custody
rules; this profile creates no new publisher or recovery submission.

Typed categories below state obligations, not newly declared Rust error
spellings. Existing earlier source/lifetime failures preserve their real cause.
New-profile interface and error precedence must be concrete before S2.

## 10. Compatibility and proposed admission matrix

All new-profile rows below are conditional future obligations, not current
support. No S1/S2/S3/S4 gate is satisfied by this table.

| Material / source combination | Required disposition |
|---|---|
| Sheen extension absent | Exact current PbrMetallicRoughnessRealtimeF32V1, source/profile identity and old cost/output path |
| Explicit R029, finite factor-only C/s and valid old dependencies, D present, E/G absent | Candidate only after §14 numeric/ABI/bound decisions and separate S1/S2/S3 gates; all original R025 light kinds/shadows preserved |
| Explicit R029 with C=0 | New identity retained; ideal sheen=0 and A=1; no validation or unsupported-composition bypass |
| R029, D/E/G absent | No reflected light; independent valid emission only, after profile admission |
| R029 with E and/or G | Unsupported until the complete §6.2 composition is separately qualified |
| Either sheen texture declared, including missing bytes or zero factor | Unsupported factor-only source; never omitted/neutralized |
| Original base/normal/MR/AO/emissive role missing or invalid | Original default only for genuine optional absence; original typed refusal for required/invalid present data |
| Opaque/Mask and deliberately double-sided source | Original R024/R028 visibility, coverage and mapped-normal rules; sheen adds no Blend/transmission |
| Missing authored TANGENT with present normal map | Existing explicit admitted source-frame route/certificate required; no automatic tangent repair |
| Scene mixes old V1 and new-profile packets | Distinct per-packet identity; concrete mixed layout/batching admission OPEN before implementation |
| Other unselected material layer/extension or linked-source family | No inferred admission or migration |
| Legacy/G1/2D/nonmaterial paths | Unchanged; no new fields, resource work or renderer branch |
| Wrong Context, stale generation, invalid Device/target or exhausted budget | Original typed refusal/withdrawal/last-good law, never cache success |

## 11. Future proof obligations and negative cases

None are executed by this S0 document.

### 11.1 Deterministic source and mathematical controls

- Original factor/default/presence and packed-value provenance, including the
  supplied exact color and roughness; regularized s=0/0.045/0.5/1 references
  without changing stored source s or base roughness.
- Complete D, exact projected-area P including nu=0/1, P>=nu, nonnegative
  lambda_exact, reciprocal correlated visibility and stable algebraic identity;
  positive-hemisphere branches and independent emission remain.
- Preserve §5.6's symbolic normalization/positive-area/P_min/weighted-bound and
  half-vector-Jacobian proof of E<=1. Future P/E numerical representations need
  independent certified error treatment; the ideal theorem is not an executed
  quadrature, table, f32 or backend test.
- Literal two-direction A and complete material/direct/indirect radiance
  certificate. Test relevant correlated color/roughness/normal/light cases;
  never accept only a mean, a few samples or an old base-only bound.
- Absent sheen routes to the identical old profile, not a duplicate approximate
  evaluator. Present-zero sheen is a separate identity with its specified
  ideal equivalence and a predeclared actual-output comparison.
- Old V1, legacy/G1/2D, source validation frequency, warmed capacity, actual R025
  source/geometry reuse and finite retirement remain intact.

### 11.2 Typed refusals and custody controls

| Case | Required proof |
|---|---|
| NaN/infinite/out-of-range factor, wrong arity or invalid source packing | Attributable source/profile refusal before writes; no clamp/default |
| Declared sheen color or roughness texture, even zero factor or absent bytes | Unsupported-role result, not genuine absence or artist-material success |
| Required old role absent; invalid mip/sampler/UV/frame/normal certificate | Original typed source cause; unchanged A/B/C/absence and R028 semantics |
| Unqualified numerical representation/domain, uncertified P/E or incomplete full radiance certificate | No production admission of this profile; explicit blocker/refusal |
| Unsupported R026/layer/profile combination | No silent omission of sheen or missing light |
| Wrong/stale Context/resource/source/material/P/E generation or ended Device | Existing lifetime failure, no inherited receipt |
| Budget exhaustion, inactive/zero/incompatible target | Existing finite/no-use/withdrawal law |
| Valid-to-invalid changed factor/source/view candidate | Requested differs from committed; compatible old front remains attributable |
| Pre-submit abort or submitted/unpublished failure | No false commit, early lease release or unrelated-submit completion claim |
| FP16 overflow/nonfinite or observed failing supported backend | Failed affected conformance/envelope; no tone/clamp success or weakened bound |

### 11.3 Actual compiler, GPU and native evidence

After explicit S1 and S2 grants, predeclare independent predictions,
available observables and authorized comparison criteria before execution.
No tolerance is chosen by this draft or borrowed for a different error class.

Use the actual selected program and existing material/Context/R025/R027 test
owners on Metal N1 AND N4. Cover factor/roughness/grazing endpoints, normal
modes A/B/C and absence, original normal/mip/sampler cells and gradients,
Mask/cutoff/sidedness, current directional/point/spot shadow content and
depth/PCF, complete scene FP16 and registered ordinary Image. Preserve
R028 §12 quality/temporal/performance requirements unchanged.

The unchanged original three-asset consumer remains an old-profile regression
oracle. The original Bed can become a new-profile oracle only under a separate
owner-authorized source path, complete certificate and declared comparison;
this unit neither opens it nor claims that outcome. Do not change the source,
use a replacement shader or collect arbitrary aesthetic pixel goldens.

Record source/profile/shader/compiler/Device/backend/domain/target/N/provenance,
expected versus actual observable results and unresolved residuals. GPU
intermediates are not inferred from attractive final pixels. Physical native
quality/performance and JC visual acceptance require separate S4 evidence.

## 12. Paired invariants and prohibitions

- **INV-R029-PROFILE:** Explicit new immutable profile, old V1 retained exactly.
  **FORBID-R029-PROFILE:** No V1 field/ABI reinterpretation, silent migration or
  sheen-as-neutral source success.
- **INV-R029-SOURCE:** Original finite linear factors, independent roughness,
  exact presence/revision and all old role/frame certificates.
  **FORBID-R029-SOURCE:** No gamma change, factor repair, missing-role default
  for invalid data or unqualified texture extension.
- **INV-R029-LOBE:** Exact selected Charlie D, projected area P and correlated
  exact visibility, with the original noncontributing branches.
  **FORBID-R029-LOBE:** No fitted-lambda substitute, Ashikhmin, altered exponent,
  added Fresnel/terminator trick, fake ambient/emission or unscaled additive sheen.
- **INV-R029-ENERGY:** E belongs to that SAME exact lobe; §5.6 proves ideal
  0<=E<=1 and 0<=A<=1 with the literal two-direction scaling. Numerical and
  complete-scene bounds remain separate proof obligations.
  **FORBID-R029-ENERGY:** No guessed LUT, E/A clamp, view-only shortcut or
  normalization-as-energy-proof claim.
- **INV-R029-LIGHT:** Actual independently admitted incident sources, complete
  new bound, unchanged R025 geometry/shadows and emission once.
  **FORBID-R029-LIGHT:** No invented light/IBL, changed PCF/cascades, old
  base-only certificate or omitted sheen in an indirect combination.
- **INV-R029-NUMERIC:** Source/actual-host checks and declared numerical
  implementation/backend evidence remain distinct.
  **FORBID-R029-NUMERIC:** No unchecked overflow fallback, guessed GPU epsilon,
  CPU per-FOV simulator, per-frame readback or universal precision claim.
- **INV-R029-RESOURCES:** Finite exact Context-bound metadata/uniform/program/
  P/E-resource/staging/old-in-flight charges and original custody.
  **FORBID-R029-RESOURCES:** No hidden global, uncharged table, early free,
  extra Queue owner, scheduler or publisher.
- **INV-R029-CURRENTNESS:** Full source/material/view/light/HDR/target/serial
  tuple and truthful old-front failure survive every successor.
  **FORBID-R029-CURRENTNESS:** No factor patch, stale lease/map, old-front
  relabel, partial commit or material-driven target authority.
- **INV-R029-ABSENCE:** Unselected old profiles and genuine no-light/no-view
  cases retain their original output and zero additional sheen work.
  **FORBID-R029-ABSENCE:** No dummy role/light/job, old-material P/E lookup or
  invalid-to-absent conversion.
- **INV-R029-PROOF:** Draft, adoption, engineering, backend and native evidence
  are separate attributable stages.
  **FORBID-R029-PROOF:** No self-adoption, compile/image-as-support claim,
  private-data access or declaration that Bed is rendered.

## 13. S0/S1/S2/S3/S4 admission

| Stage | Required owner decision / evidence | Current R029 state |
|---|---|---|
| S0 | Complete selected inactive documentary candidate and exact open issues | This artifact only |
| S1 | Independent adoption of the exact profile, resolved semantic/evaluator domain and coordinated HAL370/interface/parent deltas under current authority and JC veto | NOT GRANTED |
| S2 | Separate bounded implementation grant; resolved numeric P/E/f32/ABI/source-bound decisions; complete producer, finite resources, errors, old profiles and actual tests | NOT AUTHORIZED / NOT RUN |
| S3 | Independent versioned supported source/backend-envelope acceptance with predeclared actual N1/N4 source-to-FP16/Image and lifecycle/quality evidence | CLOSED |
| S4 | Actual canonical native quality/performance/source-to-display evidence and independent JC product/visual acceptance | CLOSED |

An open issue is not consent to choose an executor default. S1 alone runs no
code; S2 proof is not public/backend admission; none of those is native
acceptance. Existing accepted old-profile S1/S2 receipts retain their scopes.

## 14. Open decisions and exact implementation blockers

**R029-GRAZING-DOMAIN is RESOLVED at S0 reference-mathematics level only**
by the exact projected-area correction and proofs in §5.6. It is not an S1
adoption, a finite numerical representation, a scene certificate or backend
conformance. No positive-grazing cutoff is required by the ideal reference.

Every REMAINING item below is **OPEN FOR ARCHITECTURAL DECISION**. No numerical
representation, table, layout, guard or approximation is selected to fill it.

| ID | Exact missing decision/evidence | Gate |
|---|---|---|
| R029-E-REPRESENTATION | Choose consistent P AND E numerical evaluation/representation, domain/endpoints, quadrature or LUT construction, finite dimensions, storage/filter/interpolation and certified approximation/error treatment preserving the exact-reference obligations | Before S2 implementation |
| R029-F32-EVALUATOR | Specify packed constants, actual f32 operation order, dot-domain endpoints, positive tiny products, power/projected-area/reciprocal/half-vector safety and actual backend comparison criteria; no arbitrary epsilon or source repair | Before S2 implementation/evidence |
| R029-FULL-RADIANCE | Supply a rigorous complete material/direct/emission and, if admitted, indirect source certificate with the new lobe/domain/error bounds; old R025 base-only certification is insufficient | Before candidate writes |
| R029-TYPED-ABI | Select exact typed source/profile/descriptor and error precedence, material/program/binding ABI and mixed-packet routing/finite costs, preserving current MaterialBlock64 and V1 exactly | Coordinated S1/interface decision before S2 |
| R029-INDIRECT-COMPOSITION | Select/qualify complete R026 E/G integration and data meaning, preintegration, scaling/AO placement, bounds, resources and N1/N4 evidence; current R029+E/G remains unsupported | Before that combination can be admitted |
| R029-SOURCE-INTEGRATION | Separately authorize the producer/container path carrying original presence/factors/frame/certificates; no lost authored sheen or private-data transfer | External owner before source ingress work |
| R029-OBSERVABLE-CRITERIA | Predeclare sufficient independently predicted numerical/quality observations and accepted criteria for new lobe/E/domain, preserving R028 §12 | Before S2 evidence, then independent S3/S4 |

The selected D/projected-area/exact-visibility/E/layer equations and their
ideal grazing/energy proofs are fully recorded. They are not represented as
an already certified finite f32 implementation or a complete scene certificate.
Until the required decisions and evidence exist, implementation/admission is
BLOCKED and this document remains Draft / INACTIVE.

## 15. Non-goals and documentary completion

This correction modifies only this inactive R029 draft. It changes no adopted
RFC, profile enum, public type, material block, texture role, shader,
BRDF implementation, source validator, renderer,
GPU resource, test, version, manifest, asset, application or UI. It does not
pack/load a model, change the HAC wire, introduce sheen textures, choose another
cloth model, adjust lights/exposure or open private source data.

Documentary completion requires: one unique R029 filename; consistent inactive
status and gates; exact distribution/projected-area/visibility/energy proof
and default/source/refusal readback; valid local links and balanced Markdown;
an owned document diff and
whitespace check; preservation of all foreign bytes and staged state.
No Cargo, Metal, timing or native test is run as part of this documentary unit.

## 16. Reference and source boundaries

- [R1: Official Khronos KHR_materials_sheen][R1], ratified extension, specifically
  factor semantics and the non-normative Charlie/albedo-scaling implementation
  notes, read 2026-11-03. Those notes allow implementation choice. The initial
  fitted-lambda choice is superseded history (§16.1); the CURRENT exact
  projected-area visibility is selected by the S0 correction task, not claimed
  to be the note's fit or imported engine authority. Two-direction scaling stays.
- [R2: Official factor/texture schema][R2], read 2026-11-03, for source arity,
  limits and defaults.
- [R3: Conty Estevez and Kulla, Production Friendly Microfacet Sheen BRDF
  (2017)][R3], Charlie distribution and projected-area masking background.
  Its coefficient fit is retained only as superseded history; neither that fit
  nor its separate artistic terminator modification is the current selection.
- Local source inspected at e71b1791: material_api.rs:935–997 shows current alpha,
  five named roles and descriptor; pbr.rs:470–545 shows MaterialBlock64 and
  five-role resource custody. Neither inspection supplies a sheen implementation.
- Only owner-supplied scalar/source facts in §2 enter this artifact. No private
  model evidence is reproduced. External references establish provenance,
  not Halcyon adoption, licensed asset redistribution, GPU or product support.

### 16.1 Superseded initial S0 fitted-lambda selection — history ONLY

The initial c5c75b5c draft selected the following fit from [R1]/[R3].
HAL-R029-EXACT-PROJECTED-AREA-GRAZING-CORRECTION-S0-01 replaces it with §5.3's
EXACT projected-area reference. These historical formulas/constants are
NOT the current evaluator, numerical representation, approved approximation
or fallback, and their S0 selection was never an S1 adoption.

Historical t=(1-alpha_g)^2 and
p=(1-t)*p_at_alpha1 + t*p_at_alpha0:

| Historical coefficient | p_at_alpha1 | p_at_alpha0 |
|---|---:|---:|
| a | 21.5473 | 25.3245 |
| b | 3.82987 | 3.32435 |
| c | 0.19823 | 0.16801 |
| d | -1.97760 | -1.27393 |
| e | -4.32054 | -4.85967 |

~~~text
ell_fit(x,alpha_g) = a/(1+b*x^c) + d*x + e

lambda_fit(x,alpha_g) =
    exp(ell_fit(x,alpha_g))                           for 0<=x<0.5
    exp(2*ell_fit(0.5,alpha_g)-ell_fit(1-x,alpha_g))   for 0.5<=x<=1

f_fit = D_chi / [4*nv*nl*(1+lambda_fit(nv,alpha_g)+lambda_fit(nl,alpha_g))]
~~~

That FORMER fit has finite lambda_fit(0)=exp(a+e). Its cosine-weighted
response has a positive constant/nu lower bound on an interior light patch
as nu approaches zero, so its former E_fit is unbounded and its literal
former base scaling can become negative for nonzero q. This records why it
was superseded; it is NOT a continuing divergence claim for the corrected
reference. The exact P(0)>0, visibility, E<=1 and scaling proofs are §5.6.
No historical cutoff, coefficient table or fit is silently used for runtime.

[R1]: https://github.com/KhronosGroup/glTF/blob/main/extensions/2.0/Khronos/KHR_materials_sheen/README.md
[R2]: https://github.com/KhronosGroup/glTF/blob/main/extensions/2.0/Khronos/KHR_materials_sheen/schema/material.KHR_materials_sheen.schema.json
[R3]: https://blog.selfshadow.com/publications/s2017-shading-course/imageworks/s2017_pbs_imageworks_sheen.pdf
