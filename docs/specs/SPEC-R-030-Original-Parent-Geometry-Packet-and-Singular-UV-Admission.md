# SPEC-R-030: Original-Parent Geometry Packets and Singular-UV Admission

**Status: Draft / INACTIVE / NOT BINDING — complete technical S1 candidate.**

**Date:** 2026-11-03. **Architecture selection:** Delegated technical review under JC's explicit
`HAL-R030-BED-PARENT-GEOMETRY-PACKET-AND-UV-SINGULAR-ADMISSION-S1-CANDIDATE-01`
mandate. JC retains constitutional, product and physical appearance approval.
This file records the selected candidate; it does not independently adopt it.

**S0 correction — 2026-11-03:** The delegated architectural reviewer's
`HAL-R030-MIKK-SINGULAR-SAME-CENTROID-SIGNED-INTERPOLATION-S0-CORRECTION-01`
replaces this candidate's false face-constant-sign and independent-nine-pair
requirements for the exact-zero-UV MikkUvSingular route with the same-centroid
signed interpolation and six-control sufficiency proof in section 6. Original
R024 A/B/C and NormalOnly remain binding and unchanged; this correction is not
an operative parent amendment, S1 adoption or permission to implement.

**S0 source-margin correction — 2026-11-03:** The delegated architectural reviewer's
`HAL-R030-SOURCE-DERIVED-POSITIVE-FRAME-MARGIN-S0-CORRECTION-01` supersedes the
fixed universal 1/16 requirement previously placed on the original-Mikk option
of this inactive exact-zero route. That option requires a rigorously positive
source-derived norm bound and a separately certified finite-error condition.
The six-control witness is optional sufficient evidence, not the only possible
source proof. No old R024 A/B/C bound, signed-frame rule or NormalOnly contract
is reduced, and no operative source class or implementation is activated.

**S0 actual-zero interpretation correction — 2026-11-03:** The delegated architectural reviewer's
`HAL-R030-ACTUAL-ZERO-FRAME-SOURCE-DEFINED-GEOMETRIC-INTERPRETATION-S0-01`
records the supplied exact classification of the formerly unresolved 206 faces:
203 have an original same-centroid N/Mikk cross zero and three are zero-free.
Section 6.8 selects a separate, explicitly producer-declared derived zero-UV
geometric render frame. Original P/N/UV/index and all original signed Mikk data
remain immutable; different render normal/tangent XYZ is disclosed as a changed
shading interpretation. No shader fallback, source repair or activation follows.

No public API, source admission, frame mode, policy limit, shader or backend
capability changes through this document. S1 adoption, a separate S2 execution
grant, S3 source/backend qualification and S4 native/JC acceptance are distinct.
The original Bed is **not claimed supported, packed or rendered** here.

## Historical drafting scope

- Objective: specify lossless whole-original-parent packet-set admission and
  explicit mapped zero-UV frame interpretations for the complete original Bed;
  handle the 203 supplied exact original-frame zeros by a declared source-derived
  geometric render frame, preserving original data, signs and separate proof gates.
- Standing objective: native application PBR presentation of the unaltered original Bed reference scene;
  preserve original geometry, material association, normals, UVs and provenance.
- Review authority: The delegated architectural reviewer selects the technical
  contract; documentary source qualification records it for independent review.
  No implementation or successor selection follows from this artifact.
- Source qualification: Affected R024/R025/R027/R028/HAL355/HAL370 clauses
  and the current geometry/material owners were inspected.
- Governing authority: HAL002/002A remain supreme. Accepted parent contracts
  retain their scopes; the proposed deltas in section 3 have no present effect.
- Target state: Halcyon 0.230.1-beta, candidate correction baseline `c0a6e91d`;
  implementation source remains unchanged from the inspected `3fea4fc8` baseline.
- Owned file: this document only. Existing source, tests, specifications,
  historical proposal, consumer files and private source bytes are read-only.
- Invariants: the paired `INV-R030-*` and `FORBID-R030-*` requirements in section
  11, including full coverage, exact global provenance and original role truth.
- Reuse: existing Context3D typed generations/leases, certified PBR52 geometry,
  flat structural snapshots, R025 shadows and R027's sole FP16/Image publisher.
- External boundary: The source producer owns the original scene and whole-primitive Mikk
  production; Halcyon independently validates submitted source and certificates.
- Verification: documentary algebra, actual shader concordance and complete
  source/heading/link/invariant/diff inspection only. No Cargo, GPU, numerical
  source experiment, private intake or native test runs.
- Completeness: complete proposed semantic inputs, checks, transactions, finite
  bills and future proof gates; no unselected numeric caps or fabricated proof.
- SemVer/changelog: none for this inactive documentary candidate.
- Repository effects: one owned local documentary unit; no remote publication,
  history rewrite, source mutation, cleanup or unrelated staging.
- Non-goals: R029 shading, container wire/packing, importing or opening Bed data,
  a scene renderer, widget behavior, implementation and S3/S4 acceptance.

## Architecture boundaries

- Governing SPEC/read status: HAL002/002A, HAL355, R024 source/frame laws, R025
  source/shadow/currentness, R027 output, R028 sampling and HAL370 staged admission.
- Widget policy: N/A. Interaction added: NONE.
- Applicable prohibitions: no ownership/phase bypass, fake original, incomplete
  front, hidden cache, fabricated original tangent/UV derivative or role omission;
  any derived render-frame interpretation is explicit and separately revisioned.
- Existing mechanism inspected: public certified geometry descriptor, original
  identity/corner maps, complete certificate verifier and Context budget owners.
- Reinvention risk: reuse those owners through a distinct opt-in packet-set
  ingress; no second renderer, Context, Device, Queue or publisher.
- Demo/example compliance: N/A; no demo or runtime changes.

## 1. Exact objective, evidence boundary and stage

The consumer-side source scan reported 2,168,998 original triangles in five
original primitives. It identified 27,448 exact zero-UV-determinant triangles
with positive geometric face area at original node 1 / mesh 1 / primitive 1
whose material has a genuine normal map. Another 43 zero-UV triangles belong to
genuinely normal-unmapped material 0. It reported finite original P/N/UV values.
These are supplied source-owner findings, **not a Halcyon private-file scan**.

The subsequent supplied source-only receipt for the original Bed reference scene reports successful canonical
whole-primitive Mikk 0.3.0 output for all 3,473,112 corners of the measured mapped
primitive. All 27,448 exactly UV-singular positive-area mapped faces have valid
reported original corner XYZ/sign outputs: 21,371 faces have mixed corner signs
and 6,077 have uniform signs. These are original source facts, not invalidity
caused by a supposed shader requirement that signs be face-constant.

On the 6,077 uniform-sign faces, an exploratory independent-nine-pair witness
search found candidate witnesses for 5,935 and did not find one for 142. The 142
are **not proved impossible or barycentrically degenerate**. Neither that search
nor the original sign counts establishes the corrected six-control certificate,
the complete normal/mip domain or actual Bed GPU acceptance.

The subsequently supplied six-symmetric-control numerical search reports 25,007
candidate witnesses and 2,441 not found among the 27,448 mapped singular faces.
It does **not** establish outward-certified positive bounds for all 25,007.
The supplied exact-dyadic follow-up over those 2,441 reports:

| Supplied source result | Exact scope and evidentiary meaning |
|---|---|
| 1,686 faces have an exact barycentric witness with `|cross(n,t)|^2 < 1/256` | Refutes a source-universal norm lower bound 1/16 for complete Bed; these are not 1,686 proved zeros |
| 2,235 faces have a strict positive cross-norm certificate over the entire simplex from exact rational Bernstein subdivision | Reported mathematical source positivity, not a GPU/f32 safety margin or an automatically adopted R030 verification method |
| 206 faces were unresolved under that finite subdivision budget | Historical proof-budget checkpoint, superseded by the exact classification below; all faces remain required for complete Bed |

The 1,686 counterexample set and 2,235 positive-certificate set are overlapping
types of evidence within the same 2,441, not disjoint counts to add. At that
earlier checkpoint no exact zero had been found; that observation did not prove
the unresolved faces nonzero. A positive source norm smaller than 1/16 remains
consistent with an exact witness below squared norm 1/256.

The later supplied source-owner exact-dyadic elimination receipt for the original Bed reference scene classifies
**all of those formerly unresolved 206 faces**: 203 have an exact zero of
`C(a)=cross(sum_i a_i N_i,sum_i a_i T_i)` on the closed barycentric simplex;
202 of these have an interior zero and original triangle **834878** has an edge
zero. The three zero-free original triangle IDs are **840436, 861518, 921015**.
There are **zero remaining unresolved cases within this 206-face set**. These
are absolute original triangle IDs within node 1 / mesh 1 / primitive 1, not
packet-local identities or counts over the entire scene.

The 203 exact zeros rule out any strictly positive whole-triangle lower bound
on the original same-centroid smooth/Mikk cross for those faces, regardless of
search depth, smaller margins or more accurate arithmetic. If n is zero the
shader's normal/GS premise is undefined; otherwise cross(n,t)=0 gives local
`g=t-n*dot(n,t)/dot(n,n)=0`, whose normalization is undefined. This does not make
the original finite Mikk corner output, nonzero-area geometry or artist asset
invalid. It proves that the original interpolated frame is not defined everywhere
under that ideal interpretation. It is not a GPU observation or Bed admission.

The minimum supplied positive source-certificate value is reported as
`L = 164619484011542148401456026519 / 6584959359641412873073654946758393856`,
approximately `2.499931662759823e-8`. It is a reported certified lower-bound
value, **not the true minimum**, not E_gpu and not proof of binary32 safety.
The actual certificate must identify the bounded polynomial, norm versus
squared-norm units, original/packed interpretation and covered domain before L
can be used; section 6.4 forbids silently treating it as delta_source.

These are supplied source-owner receipts, not a Halcyon replay or inspection of
private execution bytes. The delegated architectural reviewer independently checked rational values of two
exact witnesses and their comparison with 1/256, not the private execution
dataset. The later 203-zero/three-zero-free classification is also supplied by
the source owner; Halcyon has not replayed its private execution bytes. These
receipts supersede the earlier queued/no-result and 206-unresolved checkpoints;
they do not establish full source or backend acceptance. Halcyon opens no private
assets and does not infer that the 25,007 numerical candidates are all certified.

The reported original size exceeds current admission limits in the existing
whole-geometry route. The per-parent byte/vertex/index counts, packet sizing,
complete Mikk capture/provenance for independent verification, certificate
outcomes and simultaneous resource bill must come from attributable complete
source evidence. Scene triangle count alone is not a per-mesh byte or Device
admission calculation.

The candidate removes neither triangles nor genuine normal mapping. It defines
one reusable checked original parent and a complete set of bounded derived GPU
packets. Singular UVs get a distinct explicitly selected frame interpretation:
either independently qualified original-Mikk interpolation where possible, or
section 6.8's source-derived geometric rendering. The latter changes shading
meaning, not original data or geometric topology; it is never an implicit fallback.

All SHALL/SHALL NOT statements below are **proposed requirements effective only
after lawful coordinated adoption**. Until then the active source contract and
its existing refusals remain unchanged.

## 2. Governing owners and unchanged contracts

- [HAL002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md) and
  [HAL002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md): ownership, phases,
  immutable snapshots, identity and explicit authority.
- [HAL355](SPEC-HAL-355-Native-3D-Rendering-Substrate.md) sections 7/8/9.1.1/10/11/13:
  typed Context resource families, geometry, finite retained capacity and fronts.
- [R024](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md) sections
  7/8/9/10: original source meaning, selected frames, packing, roles and material.
- [R025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md) sections 3/6/7/8/10/11:
  real original caster/receiver geometry, current shadows, radiance and custody.
- [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md): same-Queue scene/tone/final
  and the one truthful linear-FP16 ordinary Image front.
- [R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md): actual
  role/mip/sampler/normal-domain certificates and observable backend qualification.
- [HAL370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md) sections 7/9/10/11:
  stages, complete changed-source checks, exact warm reuse and current targets.

R029 remains a separate inactive material proposal. Geometry admission under
R030 would not authorize sheen, missing material semantics or an unsupported
combination. Existing admitted D/E/G families retain their own exact contracts;
this candidate adds no light, environment, GI or material support class.

All source/frame data remains in original mesh LOCAL authoring space. Model,
metric and render-relative transforms remain in their existing owners, applied
exactly once. [PM004](SPEC-PM-004-Geometry-Authoring-Space-Contract-Drift.md)
remains applicable; splitting packets cannot bake or duplicate a placement.

## 3. Live-source conflicts and required coordinated adoption

The implementation baseline inspected at `3fea4fc8` was requalified unchanged
at candidate revision `c0a6e91d`. Source references identify the current
mechanism, not an implemented extension or a promise that line numbers persist.

| Current owner | Verified behavior | Proposed R030 distinction |
|---|---|---|
| [material_api.rs](../../src/three_d/material_api.rs), 262–304 | `PbrOriginalIdentity3D` has source/revision/node/mesh/primitive; `PbrOriginalGeometry3D` carries full vertices, indices and signed corners with tangent provenance | Retain those original facts in one typed, Context-bound parent capture; never substitute packet identity for original identity |
| [material_api.rs](../../src/three_d/material_api.rs), 368–407 | `PbrFrameCertificate3D` has OldCone, AuthoredSmooth, SourceGeometric and NormalOnly; one `PbrGeometryDescriptor3D` carries original/render/mapping/certificates/mapped and byte/work bounds | Separate opt-in complete packet-set input and distinct exact-zero frame interpretations/certificates; MikkUvSingular and the derived geometric option are proposed semantics, not current enum variants or a selected new ABI |
| [material_api.rs](../../src/three_d/material_api.rs), 415–504 | `admit_pbr_geometry` checks render mesh capacity, reserves conversion CPU storage and converts the whole submitted original/certificates | A shared original capture must not be copied or rescanned once per child packet; each child still needs bounded actual render conversion/admission |
| [geometry.rs](../../src/three_d/geometry.rs), 962–1037 | `certified_capture_bounds` requires original elements = render elements, whole triangle certificate count and complete signed corners; counts source/certificate bytes and verification items | The old equality stays for the old operation; the distinct set proves aggregate coverage and exact per-range mapping against one complete parent |
| [geometry.rs](../../src/three_d/geometry.rs), 1580–1628 | Local loop triangle is currently the original absolute triangle; original signed corners use `3*triangle+k` | New packet checks must explicitly translate local triangle to absolute `start+local`, including every signed-corner lookup |
| [geometry.rs](../../src/three_d/geometry.rs), 1313–1362, 1676, 1728, 1758–1759 | `derived_source_frame` requires an outward-certified nonzero UV determinant; mapped C/A/B call it | Both new options require exact determinant zero and retain original smooth/Mikk provenance without invoking UV inversion; only the explicitly declared geometric option derives different render XYZ from original positions |
| [geometry.rs](../../src/three_d/geometry.rs), 1642–1647, 1699–1707, 1763–1805 | Current mapped input requires equal corner signs, mode-specific original/render direction correspondence and exact original signs; NormalOnly refuses mapped input; mapped frames check all nine independent cross pairs against at least 1/16 | Keep old A/B/C guards; new options retain each original sign but separately certify either original smooth/Mikk render directions or constant geometric render directions, with correct source/render interpretation and finite-error evidence |
| [pbr.rs](../../src/three_d/pbr.rs), 899–931 | Normal vec3 and tangent vec4 both use perspective-centroid interpolation; local GS uses their XYZ and B is interpolated tangent.w times cross(N,T), with no sign(w) or division by w | Original-Mikk positivity is one option; constant certified Fp/Up at all three corners is a distinct declared input interpretation consumed by the same unchanged shader, with original per-corner w and separate numeric/map gates |
| [context.rs](../../src/three_d/context.rs), 29–110 | Finite policy includes per-mesh bytes/vertices/indices, resident geometry, CPU source/cache, uploads, packet counts and retained generations | Distinguish bounded whole-parent CPU capture from each bounded render mesh while charging aggregate and overlapping ownership |

R024 sections 7.1/7.1.1 presently require nonzero UV Jacobians for mapped input;
7.1.2 selects exactly A/B/C; 7.1.3's C derives dPdu/dPdv using a nonzero
determinant; 7.2 refers mapped surfaces to A/B/C. An adopted R030 therefore needs
an **explicit coordinated, opt-in qualification of those clauses** for the new
exact-zero mapped options. R024 section 7.1.1 also requires a common corner sign, and
section 7.2's existing source proof permits independent normal/tangent weights.
The opt-in qualification must expressly select common ideal centroid weights and
interpolated signed w for the new route, without altering old A/B/C obligations,
their nine-independent-pair proof or NormalOnly. It must distinguish the unit
N/T frame from the possibly non-unit B=w*cross(N,T). Original-Mikk rendering
needs its own source-positive certificate; the derived geometric rendering
instead needs the exact face/edge construction, constant packed render-frame
certificate and truthful original-versus-derived identity in section 6.8.
Its rendered XYZ is not required to equal normalized original smooth/Mikk XYZ;
the complete original arrays remain retained and independently verified.

The derived zero-UV option is **not old mode C**: mode C uses nonzero-UV dPdu/dPdv
and its UV-derived H, whereas section 6.8 uses original ordered geometry, a
longest-edge convention and each original Mikk w. It does not claim recovered
authored UV orientation. Source and render shading interpretations must remain
distinct even if particular packed directions happen to coincide. The new
exact-zero options do not inherit a universal 1/16 condition on the original
cross polynomial. Old A/B/C retain their accepted independently weighted 1/16
obligations, sign rules and original applicability. Recording these conflicts
does not amend R024 or current geometry guards now.

HAL355 section 8.2 still requires each GPU packet to draw its entire own geometry
once with zero base vertex/first index/first instance and one instance. R030
preserves that rule: ranges select original CPU provenance during construction,
not runtime subrange draws from a giant shared GPU mesh. Sections 7/8/13 need
explicit recognition of the new typed parent/set source owner and finite bills;
they supply no implicit unbounded parent exception.

HAL370's exact source/profile association must include the parent, complete set,
absolute ranges, certificates and material/role generations. Its 2026-11-03 rule
against repeated unchanged source scans and per-FOV GPU-model proof sweeps stays
binding. R025/R027 geometry, shadow, radiance, ABI and publication laws are not
relaxed. No implementation may resolve these conflicts by weakening old guards.

## 4. Proposed public semantic inputs and ownership

The records below define complete input **semantics**, not compiled Rust names,
wire bytes, new shader attributes or an API already available to consumers.

### 4.1 One complete original parent

One immutable typed original parent belongs to one Context resource family and
one original `(source, revision, node, mesh, primitive)` identity. It contains:

| Semantic field | Required meaning |
|---|---|
| Original identity | Exact original source/content revision and node/mesh/primitive, not an editor object, image, resource slot or packet ordinal |
| Full original vertices | Exact original ordered positions, normals, UV0 presence/values and applicable color data, including vertices not referenced by a particular packet |
| Full original elements | Exact whole primitive TriangleList order, represented as original unindexed/U16/U32 topology with checked in-range element resolution |
| Signed original corners | For genuine mapped input, one finished canonical whole-primitive Mikk tangent vec4 for every original element/corner, not one per deduplicated vertex |
| Tangent provenance | Attributable default-Mikk producer, revision, supported version/configuration, original P/N/UV/index inputs, UV association and signed output ordering |
| Interpretation and proof revisions | Original raw bits, declared normalization/packing meaning and revisions, source/certificate dependency association and finite source-verification bounds |

The original identity contains no semantic Scene authority, Context resource ID,
GPU handle or camera/target state. The enclosing typed handle separately proves
Context/resource kind/live generation. Matching numeric IDs or a digest alone
does not prove equal original bytes, source custody or an already checked capture.

The parent owns one checked CPU capture. Immutable child packets, structural
snapshots and complete candidate/current fronts lease that capture through the
existing owners. Sharing does not copy the complete original per packet. There
is no global parent lookup, automatic cross-Context sharing or source cache.
Changed original bytes or captured tangent/provenance/interpretation require a
truthfully revisioned immutable successor, never mutation of a leased capture.
Original-content and derived certificate/packing revisions remain distinguishable.

### 4.2 One complete packet-set input

A set contains one typed parent and a nonempty ordered list of packet range
records. Every record contains its original triangle start and positive count,
bounded actual render vertices/indices, exact render-to-global-source mapping,
one certificate per covered original triangle and declared finite verification
and conversion bills. The set also binds the original primitive's real material,
normal-role presence and applicable source/profile/packing generations.

All children retain the same original parent identity and original primitive
material/placement association. Their GPU geometry and derived packet ordinals
are distinct resources, not new original mesh/primitive identities. A material
successor follows its own original provenance; packet boundaries do not authorize
different factors, omitted textures or per-chunk semantic materials.

A consumer receives a complete typed set generation, not independently publishable
successful prefixes. No isolated child range may be offered through the old
whole-original descriptor as a newly complete original. The proposed ingress
must make the parent/set distinction mechanically visible, not a caller Boolean.

### 4.3 Portable prepared records

Portable CPU records must retain the typed original identity, complete parent
association, absolute triangle ranges and corner maps, signed Mikk provenance,
raw source/packed meaning, certificate identity/revisions and material/role
association. They are immutable application data, not a Context lease, GPU handle,
previously trusted Halcyon proof or permission to omit independent verification.

## 5. Exact lossless coverage and local/global correspondence

Let the parent have E original elements and T=E/3 triangles. E must be nonzero,
divisible by three and fully resolvable. A range record `(start,count)` denotes
the half-open interval `[start,start+count)`, not a start/end pair.

For packet records in supplied order, require first start=0, every count>0,
each next start=previous start+previous count, and final start+count=T. Every
sum/product, index conversion and allocation size is checked before use. These
conditions prove natural-order complete coverage exactly once, without a bitmap,
hole, overlap, duplicated tail, silent truncation or source-dependent search.

For local triangle l in packet j, absolute triangle t=start_j+l and corner k is
the exact original element `3*t+k`. Resolve original vertex g through the full
parent's original element stream. Resolve packet render vertex r through packet
element `3*l+k`. Require the packet's render-to-source map at r to equal g.
The packet contains exactly `3*count_j` render elements and exactly `count_j`
certificates. Each certificate names the parent original identity and **absolute
t**, plus its own nonzero attributable identity/revision and selected frame.

The signed original tangent lookup is always `3*(start_j+l)+k`. Packet-local
GPU indices are never original indices or certificate triangle identities merely
because their numeric values happen to match. All packet render indices must
independently fit their local vertex arrays and admitted integer representation.
No unchecked narrowing converts a global source index into a GPU-local one.

Positions, UV0 presence/values and applicable colors remain exact under the
existing original/render bit correspondence. Source normals and signed tangents
retain original raw bits and the declared mode's certified normalization/packing
relation. Triangle winding, corner order, source material and placement remain
unchanged. Seam expansion may duplicate render vertices; it must preserve every
global source/corner association and every mode/sign seam. It cannot weld
different tangent corners, reorder connectivity or delete zero-UV triangles.

The producer generates Mikk **once over the complete original primitive**, before
packet construction. It never invokes Mikk separately on each range or generates
tangents inside Halcyon preparation. Children reference the already finished
consecutive global corner sequence with immutable provenance. Partitioning cannot
change the Mikk neighborhood, smoothing, handedness or original output sequence.

## 6. Role-exact frame admission

Frame eligibility is per original triangle; actual material/normal-role presence
remains bound to the original primitive. A mapped packet may contain both
exact-zero and nonzero UV triangles with their respective explicit certificates.
Do not reorder triangles by mode, require a single guessed packet-wide frame,
or treat an unmapped triangle certificate as erasing the primitive's normal map.

### 6.1 Genuine absence of a normal texture

When the bound material genuinely has no normalTexture, mapped=false and the
existing NormalOnly route applies. Keep original normals and their actual packed
interpretation. Its certified unit witness d must establish the existing outward
lower bounds `d dot N_i >= 1/16` for all three actual packed normals. No mapped
UV Jacobian, generated tangent, TBN or normal-texture certificate is required.

Zero UV determinant alone is not a refusal on this absent-normal route. Other
present roles keep their actual finite UV, sampler, mip and source requirements.
A declared, required, invalid, stale or unavailable normal texture is not absence;
neither a zero normal scale nor a missing certificate can erase its presence.
The 43 reported material-0 triangles need this truthful role distinction, not
manufactured mapped frames. Existing structural unused-field rules remain.

### 6.2 Nonzero UV determinant: unchanged A/B/C

For genuinely normal-mapped triangles only, use exact original source values to
classify the determinant
`(u1-u0)*(v2-v0)-(u2-u0)*(v1-v0)` before choosing a certificate route. If it is
nonzero, all existing applicable A/B/C source confidence, geometric derivation,
signed-corner, normalization, packing, singularity and cross-witness predicates
remain unchanged. An interval containing zero does not prove exact zero and does
not authorize the new route. No epsilon, failed cone or inconclusive sign test
turns a nonzero triangle into either new exact-zero option.

Mode C remains an explicitly declared old source shading interpretation with its
own prerequisites; it is not selected automatically. This candidate authorizes
neither an extra blanket frame escape nor changes to old A/B/C meanings.
Their existing face-constant corner-sign guards and independently weighted
nine-pair source obligations remain binding in their original scopes.

### 6.3 Exact-zero mapped domain and explicit interpretation choice

Both proposed exact-zero options are distinct from A/B/C and from NormalOnly.
They are eligible only when the material truly has a normal map, the original
ordered triangle has strictly positive geometric face area, and the original UV
determinant is **exactly zero**. Exact classification concerns original finite
source values interpreted as exact reals; it is not rounded equality of an
uncertified floating computation. All three original UV0 values must be present.

Required evidence for each eligible triangle is:

1. Finished signed corners from the declared canonical whole-original Mikk run,
   bound to the full original P/N/UV/index stream and global corner positions.
   A claimed generator name/version without actual signed output is insufficient.
2. Finite nonzero attributable original normals and tangent directions, exact
   tangent signs in {-1,+1}, and their declared mathematical normalization.
   Unit interpretation is not a claim that rounded stored triples have exact
   unit norm; actual normalization/packing errors must be enclosed.
3. Exact original/render/corner correspondence under the **declared** interpretation,
   including the original Mikk sign at each global corner. Each original w_i is
   exactly -1 or +1; signs may differ within a triangle. Preserve those individual
   signs bit-for-bit in render corners, including necessary source-corner seams.
   Neither packet construction nor the derived geometric option flips or
   homogenizes signs. The original authored normals and full signed Mikk XYZ/W
   remain immutable provenance, including when different render XYZ is declared.
4. An explicit producer-selected, revisioned frame interpretation and its
   independently checkable certificate:

   - **MikkUvSingular original-frame option:** preserve original smooth/Mikk render
     XYZ and certify a strictly positive bound on its actual packed same-centroid
     cross over the full simplex. Section 6.4's global/patchwise witness is optional
     sufficient evidence; another finite method needs separate selection. This
     option cannot qualify the 203 supplied exact-zero cross faces.
   - **Derived zero-UV geometric render-frame option:** use section 6.8's exact
     oriented face and longest oriented edge, certified constant packed Fp/Up
     render XYZ and original individual Mikk w. This intentionally differs from
     original smooth/Mikk shading; it does not require an original-frame positive
     certificate or root search and may be declared consistently for every
     positive-area exact-zero mapped triangle. It is not an automatic fallback.
5. The original normal-map role, normal scale, mapping, encoding, mip/sampler
   generations and normal-domain source certificates under R024/R028, without
   omissions or new default/fallback meaning, including the positive sampled-qz
   obligation in section 6.5. Section 6.7's separate finite-error inequality and
   definedness certificate is mandatory before the new mapped frame is runnable.

Neither option asserts an invertible UV Jacobian, increasing-u/v basis,
dPdu/dPdv or UV-derived orientation sign. Neither calls `derived_source_frame`
on a zero determinant. Each scalar w is the actual whole-original Mikk output;
no division by zero, fabricated derivative or sign repair is allowed. The
original-frame option cannot substitute render directions; section 6.8 separately
declares geometric render XYZ without rewriting source directions or claiming
an authored UV basis. Existing local-GS and dual-transform evaluation stays
unchanged. The scalar w is interpolated, not forced to a corner sign: no
sign(w), division by w or new shader branch is introduced.

Canonical Mikk output alone is not a packed-frame or full sampled-normal proof.
Missing, zero or nonfinite direction data, a corner sign outside {-1,+1}, or
source/render sign mismatch remains an attributed source error. A legitimate
mixed-sign source triangle is not such an error. All 27,448 Bed triangles remain
required. The supplied 203 exact original-cross zeros close that source-math
question, not the geometric option's packing/normal-domain/backend/visual gates.
An unknown certificate is not success; an actual zero of the original frame is
not silently relabeled source authoring invalidity or repaired original data.

### 6.4 Original-Mikk option: positive bound and optional six-control certificate

This section applies only to the MikkUvSingular **original-frame option**, not as
a prerequisite for section 6.8. The unchanged shader's normal vec3 and tangent
vec4 declarations both use
`@interpolate(perspective,centroid)` and the vertex stage passes their original
values through. For this opt-in route, the ideal model evaluates those varyings
at the **same** covered centroid with the same perspective weights. Lawful
positive-W clipping preserves their common convex association with the original
triangle. Let N_i,T_i be its actual packed binary32 XYZ triples interpreted as
exact reals, w_i the retained individual signed Mikk corner values, and
`a_i >= 0`, `sum_i a_i=1`. Then:

```text
n(a) = sum_i a_i N_i
t(a) = sum_i a_i T_i
w(a) = sum_i a_i w_i, hence -1 <= w(a) <= 1
C_ii = N_i cross T_i                         for i=0,1,2
C_ij = (N_i cross T_j + N_j cross T_i)/2     for i<j
cross(n(a),t(a)) = sum_i a_i^2 C_ii + sum_{i<j} 2*a_i*a_j C_ij
sum_i a_i^2 + sum_{i<j} 2*a_i*a_j = (sum_i a_i)^2 = 1
```

There are exactly six cross controls: C_00,C_11,C_22,C_01,C_02,C_12. The
off-diagonal controls are symmetric **averages**, not independent cross pairs.
The six polynomial weights above are nonnegative and sum to one. A mathematically
UNIT witness b may outward-certify `b dot C_ij >= delta > 0` for each of these
six actual packed-coefficient vectors. Delta is an independently checked finite
positive source bound, not a universal constant, user tolerance or chosen
epsilon. The convex identity then proves `b dot C(a) >= delta` and hence
`|C(a)| >= delta` throughout the simplex, where `C(a)=cross(n(a),t(a))`.

Finite source-only barycentric patches are also permissible. Their certificate
must prove exact complete coverage of the original simplex, disjoint interiors
and explicit coverage/ownership of every shared boundary and vertex. On EVERY
patch, independently certify a UNIT b_patch and positive delta_patch with
`b_patch dot C(a) >= delta_patch` throughout that patch, using its actual exact
restriction of the original polynomial and outward bounds. No uncovered region,
sample grid or guessed equality of different source coefficients qualifies.
The source-derived aggregate `delta_source = min_patch(delta_patch) > 0` then
bounds `|C(a)|` everywhere in the covered source domain. A single global witness
is the one-patch case. These are proof subdomains only: no physical triangle,
render packet, Mikk neighborhood or original corner is split or changed by them.

Every consumed delta_source must be a rigorous finite positive lower bound on
the **norm** `|C(a)|` for its explicitly associated face/domain. A differently
represented certificate must supply a checked conversion in matching units:
for example, if it bounds `|C(a)|^2 >= L > 0`, a certified norm lower bound must
satisfy `0 < delta_source <= sqrt(L)` with all representation errors included.
A value bounding a squared norm cannot be directly compared with a norm error.
The reported L in section 1 does not name its certificate polynomial/units by
its numeric value alone; those source records must resolve that association.
Neither that L nor any reported minimum is an automatically selected GPU margin.

With the actual positive norm bound, n,t and their cross are nonzero and the
unchanged ideal local GS operation has `|g| >= delta_source/M_N`, for a finite
certified positive upper norm `M_N >= max_i |N_i|`. Retain a finite certified
positive upper norm `M_T >= max_i |T_i|`, original-direction normalization/packing
enclosures and the quantitative bounds needed for every actual division/
normalization. Strict source positivity is not by itself proof that finite f32
execution is safe.

The producer supplies the source-bound witnesses, actual coefficient derivations,
every patch and its exact coverage association when used, positive lower bounds,
norm bounds and proof revisions. All proof metadata and finite verification work
remain charged under section 7. Halcyon independently checks original/corner/packed
association, every coefficient/bound, coverage and witness representation/
normalization errors. On the optional six-control route, treating an uncertified
rounded cross or average as exact, checking just the three diagonal controls,
or trusting a Boolean, digest or a fragment grid is insufficient.

This is an optional sufficient source certificate for a common ideal centroid,
not a claim that the GPU computes identical intermediate weights or exact real arithmetic.
Actual f32 interpolation, clipping, normalization, GS, transform and sampling
behavior still needs its own source/host and supported-backend error evidence.
No arbitrary numerical epsilon or blanket GPU guard is selected here. The
certificate cannot be transferred to different interpolation qualifiers/points.
There is no universal 1/16 lower norm or projection requirement on this new
exact-zero route. The old independently weighted nine-pair and 1/16 obligations
for adopted R024 A/B/C remain binding. Only the coordinated opt-in S1 qualification
could activate the new source-derived margin rule; it does not assert that the
old independent-weight source proof already used equal weights.

### 6.5 Both options: interpolated sign and mapped-normal legitimacy

At an ideal covered centroid w(a) may be zero even though all original corners
have exact valid signs. In its ideal interpretation, the unchanged shader forms
orthogonal unit world directions Nhat,That after local GS, proper dual transforms
and its existing orthogonality correction, then uses `B=w(a)*cross(Nhat,That)`.
B need not have unit
length. It neither normalizes B nor applies sign(w). Let q be the ideal decoded,
XY-normal-scale-adjusted and normalized map vector corresponding to the shader's
q at `pbr.rs` line 930. Its subsequent mapped vector is:

```text
u = qx*That + qy*w(a)*(Nhat cross That) + qz*Nhat
|u|^2 = qx^2 + qy^2*w(a)^2 + qz^2 >= qz^2
```

Thus a strictly positive qualified qz makes u nonzero even when w(a)=0. Sign
interpolation crossing zero alone is not a source invalidity or an undefined
mapped normal. This identity does not assert a unit orthonormal TBN with B of
length one or change the original normal texture. It applies once either declared
frame has been independently qualified; it does not erase the geometric option's
different shading interpretation or define the original smooth/Mikk frame at
its actual cross zero.

The source owner reported a Bed normal-texture **level-0 only** blue range 203..255. That
does not supply a complete positive bound for the actual normalized qz across
the real 13-level mip/filtered/sampler/normal-scale domain. Its complete source
certificate, actual f32/Metal stability and derivative/sampling behavior remain
UNPROVED here and must be qualified under R028 and the source owner. No bound
for normalized qz is silently inferred from raw level-0 blue values, and no
normal-scale, mip/filter, shader or positive-Z threshold is changed.

### 6.6 Failed sufficient tests versus the now-proved original-frame zeros

Failure to establish a global or patchwise six-control witness is not proof that
cross(n(a),t(a)) vanishes for any actual ideal centroid weights. The earlier
nine-pair search and its 142 not-found cases establish no such impossibility
either. The supplied 1,686 exact witnesses below squared norm 1/256 refute the
former universal 1/16 margin, not positivity itself. The later exact elimination
receipt is different evidence: of the formerly unresolved 206, **203 have proved
zeros** and **three are zero-free**, with none unresolved in that set. The edge
zero is original t834878; zero-free IDs are t840436, t861518 and t921015, while
the other 202 proved zeros are interior. Preserve exact original attribution and
the supplied receipt's scope; Halcyon has not personally remeasured those faces.

For an otherwise unqualified face with no proved zero, the original-frame option
may independently certify `C(a)` nonzero over the exact simplex and derive an actual
positive quantitative norm bound with a complete finite method separately
selected and qualified by the delegated architectural reviewer, or return an attributable refusal for lack
of an established certificate for that option. The optional projection-witness
route is not mandatory when another certificate is lawfully selected and verified.
The reported exact rational Bernstein source positives are evidence, not automatic
adoption or implementation of that verifier in Halcyon. A nonzero claim without
an independently certified usable delta_source does not satisfy section 6.7.

For the 203 proved zeros, no positive whole-triangle original-frame margin exists.
More subdivisions, smaller thresholds or exact arithmetic cannot make that
specific source interpretation defined everywhere. Do not repeat that search,
call those 203 merely unresolved, or describe an absent positive certificate as
the remaining way to admit them unchanged under the original-frame option.

Section 6.8 is a separately selected **derived rendering interpretation**, not a
proof that the original interpolation became nonzero. It may be explicitly chosen
on the entire exact-zero mapped domain without determining which faces have
roots. An original-frame refusal does not trigger it: the producer must submit
its distinct interpretation, revision and complete source-bound certificate.
No extra solver/budget, numerical floor, source repair, omitted role or runtime
fallback is selected. Original geometry and Mikk provenance remain untouched.

### 6.7 Separate finite-error and definedness gate

An exact positive source certificate is NOT a runnable-profile or GPU safety
certificate. For either option, delta_source bounds the norm of the **selected
packed render-frame** cross. Define
`C_render(a)=cross(sum_i a_i N_render_i,sum_i a_i T_render_i)`: this is section
6.4's C(a) for original-Mikk rendering and constant `cross(Fp,Up)` for section
6.8. An original-source cross zero is not the geometric render-frame cross.
Use the selected option's bound without transferring certificates between
interpretations. Neither new mapped frame is runnable unless a separately
certified finite worst-case E_gpu establishes actual nonzero/defined execution
on the declared source/currentness and supported-backend domain. Require
`0 <= E_gpu < delta_source` with matching norm units and the same covered domain.

The certificate must explicitly relate the bound to the actual unchanged f32
centroid N/T interpolation, clipping, cross/separation, dot/projection/GS,
normalization, host transforms and normal sampling. It must propagate the real
source scales, upper/lower bounds and arithmetic errors into a comparison with
the same `|C_render|` norm quantity. Do not directly sum unrelated errors in different
units or compare E_gpu to a squared-norm L. In matching norm units the reverse
triangle inequality gives residual separation at least `delta_source-E_gpu>0`;
the full certificate must also establish every actual denominator, normalization
and mapped-vector operation finite and defined, including the separate positive
sampled-qz/filter premise and any overflow/underflow behavior.

C_render is a source proof polynomial, not a new shader output or GPU telemetry.
Bounding only a hypothetical computed cross while ignoring the shader's actual
GS arithmetic does not qualify. Bind all source/packed/proof revisions, individual
signs, normal-scale/mip/filter domain, actual host/placement transforms, shader/
compiler/capability/backend class and applicable target/view domain to the error
certificate. Current input/lease/Device checks stay in their existing owners.
No full per-FOV CPU GPU simulator, proof replay, readback admission or new runtime
service is authorized by this documentary condition.

E_gpu must come from an independently computed/observed certificate with declared
coverage and rigorous finite-error interpretation, never a guessed tolerance,
epsilon, clamp, observed sample maximum called worst-case, or sampling-only
claim. Representative N1/N4 observations remain necessary under R028 but do not
alone prove a bound over an unsampled continuous domain. This is not a universal
precision theorem for arbitrary drivers. Missing domain coverage, a missing bound,
incompatible units or failure of the strict inequality leaves **S3 BLOCKED** and
the new mapped-frame run unqualified. The reported value near 2.5e-8 proves no
binary32 safety. Preserve the real 13-level sampled-qz/filter gate and full R027
radiance certificate; neither is discharged by cross-norm positivity.

### 6.8 Explicit source-derived zero-UV geometric render-frame option

This is a distinct opt-in **derived shading interpretation**, not a new public
type/enum/ABI selection, old mode C or an implicit response to an admission
failure. Its eligibility is exactly section 6.3's genuine present normalTexture,
exact original UV determinant zero and strictly positive original 3D face area.
The producer may declare it consistently on every such original triangle; no
per-face root search, exact-zero cross witness or failed original-Mikk certificate
is a prerequisite. Nonzero/inconclusive UV input stays outside this option.

**Exact source construction, original mesh LOCAL authoring space.** Interpret
the original finite P0,P1,P2 source values as exact reals with their ordered
corner/winding identities. Define:

```text
e01 = P1-P0; e02 = P2-P0
c = cross(e01,e02); Q_face = dot(c,c) > 0
F_ideal = c/sqrt(Q_face)
v0 = P1-P0; v1 = P2-P1; v2 = P0-P2
Q_i = dot(v_i,v_i), i=0,1,2
j = the first index in (0,1,2) attaining max(Q_0,Q_1,Q_2)
U_ideal = v_j/sqrt(Q_j)
```

The edge comparison and tie are **exact** on the original values: equal squared
lengths select v0 before v1 before v2. No rounded near-tie, tolerance, reversed
edge, packet-local ordering or view direction selects the orientation. The source
certificate identifies j and independently proves `Q_j>Q_i` for every i<j and
`Q_j>=Q_i` for every i>j, precisely implementing that first-maximizer rule. It also
certifies Q_face>0 and Q_j>0; a rounded or inconclusive area computation cannot
make a zero-area face eligible. No UV inverse, dPdu/dPdv or authored tangent-space
orientation is recovered by this deterministic geometric convention.

Every candidate edge lies in the original ordered face plane: v0=e01,
v1=e02-e01 and v2=-e02. Thus c dot v_i=0. Positive face area makes c nonzero and
ensures a nonzero selected edge. Consequently F_ideal and U_ideal are finite
ideal unit orthogonal directions with
`|cross(F_ideal,U_ideal)|=1`. At all three ideal render corners set normal=F_ideal
and tangent XYZ=U_ideal. Any convex interpolation, including the common centroid
and ideal clipping, preserves these constant XYZ values; local GS gives
`n=F_ideal, t=U_ideal, g=U_ideal`. The exact proof needs no assumption that the
original smooth/Mikk C(a) was nonzero and does not claim finite GPU arithmetic.
Under the existing proper invertible object linear transform L, both
`L^-T F_ideal` and `L U_ideal` are nonzero and their dot product is
`F_ideal dot U_ideal=0`. The existing ideal dual-transform/frame order therefore
remains defined; its actual host/packed/f32 errors still need qualification.

**Actual render data and immutable originals.** Emit the SAME independently
certified binary32 Fp in all three render normal XYZ fields and the SAME certified
binary32 Up in all three tangent XYZ fields of the existing PBR52 layout. At
corner k retain its exact original canonical whole-primitive Mikk w_k in {-1,+1},
including mixtures. Preserve exact original P/UV/color, winding, material and
global source/corner maps. The complete original authored normal array and ALL
original signed Mikk XYZ/W stay in the immutable parent as original evidence;
they are not overwritten by Fp/Up or relabeled as the rendered smooth frame.
Mode/packing/certificate revisions distinguish original source identity from
derived shading identity even where particular values coincide.

All three render XYZ pairs must be equal by their actual stored bits, but a
render vertex shared with another face may require existing seam duplication
to preserve both declared frames and individual original corner signs. This
does not split/reorder/delete logical triangles, regenerate Mikk per packet or
change source geometry. Qualification checks source-to-render correspondence
under this explicit derivation, not a false claim that Fp/Up normalizes original
authored N/Mikk XYZ. No new attribute, shader branch, uniform or draw owner is
selected; existing Context/Device/Queue, R025 and R027 remain the sole pipeline.

**Packing and source proof.** Ideal orthogonality is not exact stored f32
orthogonality. Supply outward derivation/normalization/packing enclosures relative
to F_ideal/U_ideal, the exact edge/face witness, actual packed bytes and finite
positive upper norm bounds for Fp/Up. Independently establish a strictly positive
norm lower bound delta_geom on cross(Fp,Up). For example, the ideal unit witness
`b_geom=cross(F_ideal,U_ideal)` gives the sufficient real bound

```text
|Fp-F_ideal| <= epsilon_F; |Up-U_ideal| <= epsilon_U
b_geom dot cross(Fp,Up) >= 1-epsilon_F-epsilon_U-epsilon_F*epsilon_U
```

Here epsilon_F/epsilon_U are certified nonnegative norm-error upper bounds,
not selectable tolerances. Expanding `(F_ideal+error_F) cross (U_ideal+error_U)`
and bounding its three error terms proves the inequality by Cauchy-Schwarz;
`b_geom dot cross(F_ideal,U_ideal)=1` exactly.

Any consumed lower bound must include witness representation and all outward
evaluation errors and be rigorously positive; no numerical epsilon, fixed new
minimum, assumed unit binary32 tuple or midpoint pass is selected. A direct
outward-certified packed-cross check with a certified unit witness is also
sufficient. All corners are constant XYZ, so their six controls (and any cross
pairs) coincide; the source bound is on this declared packed frame, not on the
original Mikk polynomial. With M_F>=|Fp| finite and positive, the exact packed
local GS has norm at least delta_geom/M_F. Apply section 6.7 in matching norm
units using delta_source=delta_geom and require the separately qualified finite
error/definedness certificate for the actual host/backend domain. Original-frame
zeros supply no such GPU certificate; ideal cross norm1 is not one either.

**Same shader and full normal role.** The existing shader receives constant
centroid XYZ and interpolates the original per-corner w. It performs its same
local GS, proper transforms and orthogonality correction. There is no sign(w),
division by w, normalization of B or alternate shader. The original normal map
remains PRESENT, with unchanged UV, encoding, normal scale, texels, mips and
filters. Section 6.5's mapped-vector identity applies only after the geometric
frame is qualified: w crossing zero alone is harmless if the separately certified
normalized qz is positive throughout the **full actual 13-level/filter/sampler/
normal-scale domain**. Level-0 blue203..255 is not that certificate. Preserve
R028 sampling/derivative and R027 complete source-radiance/FP16 obligations.

**Provenance, effects and failure.** Bind the interpretation and its revision,
original identity/absolute triangle/corners, exact area/edge comparisons,
Fp/Up derivation/packing witnesses, each original w, material/normal-role and
sampling generations, certificate revisions, finite bounds and verification
bill to the admitted immutable set. Errors distinguish unproved zero-UV/area,
wrong exact edge or tie, absent declaration, false original/render provenance,
nonconstant packed render XYZ, sign mismatch, failed packing/margin/normal-domain
proof and actual finite-resource/Device/backend refusal. Attribute failures to
the true original triangle/corner and certificate stage under section 9; no
source prefix or automatic retry with this interpretation is published. Sections
7/8 charge and lease all metadata and preserve old-front/rollback truth.

This changes shading: geometric face normals and the chosen edge orientation
can produce faceting, boundary discontinuities and visibly different cloth/
normal-map response from original smooth/Mikk shading. It is not bitwise-original
lighting, an artist-source repair or a claim that this convention reconstructs
an authored tangent basis. Producer/consumer provenance must expose that choice;
native physical appearance is JC's separate decision. Independent source/host,
supported-backend N1/N4 and full normal/radiance proofs remain gated. No Bed
frame, material or native success is established by this inactive proposal.

## 7. Finite capacity and accounting before effects

No unlimited original parent, packet count, certificate, verification work or
in-flight generation is admitted. A **finite owner-selected effective profile is
required before S2**. This candidate selects no numeric caps or packet size and
does not claim that current defaults admit Bed.

| Charged owner | Required complete bill |
|---|---|
| Original parent CPU capture | Actual full original vertex/index arrays, all retained signed corners, provenance/identity/metadata and checked capture records, once per actual retained capture |
| Parent verification | Full changed-source vertex/index/corner validation and exact count/arithmetic checks, with finite declared total work; no repeated whole-parent pass per packet or FOV |
| Packet CPU records | Every actual render array, local index array, global-source map, per-absolute-triangle certificate/checked frame, descriptor and collection capacity |
| Conversion and scratch | Actual fallible transient storage and peak coexistence with producer/captured arrays; not falsely counted as shared just because their values agree |
| Packet GPU geometry | Each actual PBR52 vertex/index allocation, required R025 supplemental geometry and associated allocation/alignment overhead, under unchanged per-mesh and Device limits |
| Whole set and snapshot | Sum of packet residency, source/cache bytes, draw/placement/object/slot counts, bindings/variants, upload/staging and every real aggregate overhead |
| Concurrent generations | Old, requested, checked-out, submitted, current/last-good, retiring and independently leased generations at their actual peak, including retained parent and packet CPU/GPU charges |
| Current target | Existing full shadow, scene/MSAA/depth/tone/final, sampler/view/binding and dynamic capacity, without double counting or omitting genuine shared ownership |

Each render packet still satisfies current `max_mesh_bytes`,
`max_vertices_per_mesh`, `max_indices_per_mesh`, canonical layouts and actual
Device buffer/index/usage limits. Current policy fields also include
`max_resident_geometry_bytes`, `max_cache_residency_bytes`,
`max_retained_dynamic_cpu_bytes`, `max_upload_bytes_per_frame`,
`max_draw_packets_per_snapshot`, `max_resource_slots` and
`max_retained_generations`; their owning meanings remain distinct.

The new whole-parent CPU obligation must have an explicit finite profile and
reservation under the Context owner. A per-packet maximum or producer-declared
`maximum_certificate_bytes` / `maximum_verification_items` alone cannot bound the
complete parent/set or authorize an aggregate policy bypass. Record exactly
which existing ledgers charge retained versus transient bytes and any required
explicit new policy dimension before implementation. Do not silently charge
large shared parent arrays as zero, duplicate them per child, or repurpose a
GPU mesh ceiling as an unlimited CPU-source allowance.

Under that explicit finite profile the complete CPU parent may be larger than
one permitted GPU mesh packet. This is the proposed distinct owner boundary;
it changes neither the old whole-original operation nor any per-packet ceiling.

All sizes, capacities, counts, offset products, conversions and alignment use
checked arithmetic and fallible reservation. Frame upload limits cover the whole
candidate, not one child at a time to evade the aggregate. This candidate grants
no background upload, extra frame, idle work or cross-frame streaming mechanism.
If the complete admitted work does not fit, refuse before candidate writes.

## 8. Transaction, source currentness and frame publication

### 8.1 Complete CPU source admission

Resolve the live Context and typed parent; validate the entire immutable parent
on actual new content/derivation dependencies, then all ordered ranges, exact
maps, roles, certificate identities/frames and complete finite bills. Process
packets and covered original triangles in supplied natural order so data errors
retain deterministic original attribution. Reserve all actual CPU/conversion/set
resources before installing a successful complete set generation.

CPU source admission creates no GPU work by itself under HAL355 section 7.1.
Success returns the complete immutable CPU set association, not a rendered image
or successful GPU allocation. If any range/certificate/allocation fails, install
no successful prefix, expose no child as a complete original, leave old usable
generations unchanged and release only certified unused candidate reservations.

### 8.2 Exact reuse and successors

The checked parent and per-packet certificates are reusable only through exact
immutable captures, Context family/live generation, source/proof dependency
association and held leases. No repeated full original copy, Mikk run or unchanged
source/certificate scan is required for another packet or FOV. Local mapping and
certificate checks are performed once on their actual changed packet inputs;
sharing a validated parent does not skip those checks.

A changed source, map, range partition, packed frame, certificate or material
association creates a truthful immutable successor with complete requalification
of changed dependencies. Repartitioning does not invent a new original source
or reset its absolute triangle identities. Failed successors do not overwrite
accepted parent/set proofs, requested source or last-good committed tuples.

### 8.3 Actual target admission and one current front

The existing active viewport preparation independently validates the complete
scene source, all set memberships/leases, current placements and actual host
matrices, target/Device/sample/format/capabilities and aggregate finite GPU and
upload reservations. Deferred GPU materialization remains in this existing
phase. Every required child must be admitted for the candidate before its writes;
a successful prefix cannot be rendered/published as complete Bed geometry.

Required changed-dependency R025 shadows, scene color and R027 per-sample tone
then run under the same captured Device/Queue and existing encoder/submission
owner. Only the complete actual submitted/swapped/registered final association
becomes Committed. The tuple binds parent/set/packet generations and absolute
ranges, certificates, material/model/source, current view/target/sample, HDR,
request revision/serial and actual publication stage. A CPU-admitted set or a
queued registration alone is not GPU completion or native appearance acceptance.

Pre-submit failure discards the unsubmitted candidate under the existing stage
contract. Submitted-but-unpublished work retains all used resources and charges
until observed writer completion and other actual use/lease protections close.
No new completion inference, Queue submit, poll, wait or publisher is introduced.
A compatible old front keeps its exact old tuple and failure status; an
incompatible/retired target or terminal Device follows existing withdrawal.

### 8.4 Parent and set retirement

Use existing typed retirement and generation checks. Retirement prevents new
admission through a retired handle; it does not free parent bytes or packet GPU
resources still held by snapshots, pending work, fronts or in-flight uses. Release
charges only as the corresponding physical/CPU ownership actually ends, with
existing Image unregistration and Queue-completion rules intact. Different
Context/Device families cannot borrow successful source proofs from each other.

The ownership graph is directed from complete set/snapshot/packet leases to the
parent capture. The parent has no child-set back-reference, UI object, Queue,
Device, callback or global registry ownership. No parent/set/Context strong cycle
or idle lifetime service is authorized.

## 9. Error evidence and concrete intended cases

Every refusal must identify the failure stage, Context/resource/set generation
where applicable, original source/revision/node/mesh/primitive, absolute original
triangle and corner/witness pair when available, packet ordinal/range and local
coordinates where useful, and the owning typed cause. Do not report packet-local
triangle zero as original triangle zero. A whole-parent/header or capacity error
has explicit parent/set attribution rather than a fabricated triangle witness.

Existing applicable source errors remain exact. The new ingress additionally
distinguishes malformed/incomplete coverage, foreign/stale parent, global/local
mapping mismatch, certificate identity/range mismatch, wrong role/frame route,
unproved exact-zero predicate, incomplete canonical Mikk provenance/output,
unestablished selected-frame or sampled-normal certificate and missing/failed
finite-error coverage or inequality. The derived option additionally attributes
exact face/edge/tie evidence, declared interpretation/revision, original/render
identity and constant packed Fp/Up/sign mismatches. Record optional control/patch
identity only when used by the original-frame option; record the selected render
frame's actual delta_source, polynomial/units, E_gpu/domain and bound evidence.
Mixed valid original corner signs
and interior w=0 are not errors by themselves. Distinguish genuinely invalid
data from a sufficient proof not established; neither a failed witness search
nor a capacity/arithmetic/allocation refusal proves the original mesh invalid
or permits publishing a reduced source.

| Case | Required disposition |
|---|---|
| Illustrative parent T=8, ranges (0,3),(3,5), exact local/global maps and valid per-triangle certificates | One complete admitted set if all finite resources and later target checks pass; packet 1 local triangle 0 remains original triangle 3 |
| Same exact parent/set and a changed FOV | Reuse checked immutable source; perform actual changed host/target and required current R025 work, with no source/Mikk/GPU-model proof replay |
| Positive-area exact-zero-UV triangle, genuine no-normal-map material, valid NormalOnly certificate | Use the unchanged absent-normal semantics; retain every triangle and other role requirements |
| Positive-area exact-zero-UV mapped triangle explicitly selects original-frame MikkUvSingular, with a positive original-frame norm bound and full sampled-normal/error certificates | Eligible only after coordinated adoption and validated E_gpu<delta_source plus actual source/target checks; cannot cover a face with a proved original-frame zero |
| The same eligible domain explicitly selects the derived zero-UV geometric option, with exact oriented face/longest-edge/tie proof, constant certified Fp/Up and each original Mikk w | No original smooth/Mikk root or positive-cross proof is needed; qualify actual declared packed frame, complete normal/error/source/target gates and disclose changed shading; no automatic fallback or Bed success |
| Ranges (0,3),(4,4) for T=8, overlapping ranges, omitted tail or swapped packet order | Refuse complete set; never return a valid-looking prefix or fabricated complete original |
| Packet certificate uses local triangle 0 instead of absolute start, or sources signed corner 3*l+k from a nonzero-start packet | Exact original-association refusal before publication |
| Normal texture present but NormalOnly submitted; nonzero/inconclusive UV determinant labeled either new exact-zero option | Refuse wrong route; preserve original role and existing A/B/C checks |
| Mikk directions missing/zero/nonfinite, a corner w outside {-1,+1}, or render w differing from its actual original corner | Exact source-qualified refusal; a valid within-triangle sign mixture is not a sign mismatch and must not be homogenized |
| Interpolated w(a)=0 with qualified qz>0 and an established frame certificate | The ideal mapped-vector norm remains positive by section 6.5; do not add sign(w), divide by w or invent a zero-sign refusal |
| Original-frame option: a global six-control witness is unestablished on an otherwise genuine source face | Not a mandatory projection-witness failure when a lawfully selected complete alternative or verified finite patch certificate proves source positivity; if no certificate for that option is established, retain exact unresolved attribution, not a degeneracy claim or automatic geometric selection |
| Reported exact witness has squared cross norm below 1/256 but a complete source certificate proves strict positivity | Reject the former universal 1/16 new-route requirement, not the authored triangle; the source-derived bound still needs the separate matching-unit E_gpu condition |
| A supplied original-frame zero among the 203 is submitted with a purported positive original-Mikk whole-triangle bound | The claim cannot hold; exact attributed refusal of that selected certificate, without classifying original geometry/Mikk output invalid or auto-selecting another interpretation |
| One of the three formerly unresolved but now zero-free faces, or another unproved numeric candidate, is submitted for original-frame rendering without a usable certified norm bound | Zero-free/search status alone is not the complete source/packing/backend certificate; no fabricated positive margin or claim that any of the former 206 remains unresolved |
| Equal exact maximum edge lengths, a rounded winner different from the exact rule, or different Fp/Up among one geometric triangle's corners | First listed maximizer is required; refuse incorrect edge/tie or nonconstant declared-frame data with original attribution, no epsilon/tangent repair |
| Source norm positivity is certified but E_gpu is unavailable, in incompatible units or not strictly below delta_source | S3 blocked and new mapped-frame run unqualified; source positivity and a small reported L do not imply finite f32/Metal safety |
| Different original bytes behind an unchanged identity/proof generation, or wrong/retired Context | Refuse changed/stale/foreign capture; no inherited proof; apply existing old-front versus terminal-withdrawal rules |
| Parent/set/old-generation aggregate exceeds a finite limit though each packet fits | Typed full-candidate capacity refusal with compatible old front intact |

These cases protect the actual parent/packet/singular-UV ingress. They are not a
catalogue of unrelated formats or hypothetical unsupported renderer families.

## 10. Complete scene and dependent renderer boundaries

One parent/set covers one complete original primitive. The five original Bed
primitives require their own truthful parent/material associations. The application retains
the complete original scene/node/placement membership and assembles all required
sets into one complete immutable structural request. Halcyon validates supplied
membership/resources; a primitive-complete certificate is not a claim that the
application has supplied every primitive of its semantic Bed scene.

Expand each complete set in original packet order with stable attributable draw
ordinals and unchanged original primitive placement/material. Preserve existing
draw classification; no new reorder, hidden instancing or partial-source draw
authority follows. Render packets are physical subdivision, not editor/source
semantic objects. The old whole-original public geometry operation and all old
five-role material families remain available with their exact meanings.

R025 must see the entire original ordered caster/receiver geometry after set
expansion, including off-camera casters and every required Mask source. Preserve
current directional cascades and all real active depth/XY/PCF coverage, spot/point
work when selected, full per-face draw replication and exact source/HDR
association. Packet boundaries cannot cull a caster, reset an original triangle
certificate, shrink a shadow budget or produce a new-light/old-map mixture.

R028 source normal/mip/cell/sampler/UV requirements and controlled derivative-valid
sampling remain unchanged except the expressly proposed singular-route source
qualification in section 6. The full positive sampled-qz premise and its finite
numerical/backend evidence, including section 6.7's strict source-margin/error
condition on the selected render frame, must be established, not inferred from
level 0, original-frame positivity or the ideal geometric cross norm of one.
R025's `shader_source_with_material(PBR_WGSL)` composition retains the actual
same-centroid normal/tangent and interpolated-w evaluation. There is no shader
substitution to make this proof pass. Section 6.8 supplies explicit constant
geometric render directions through the same layout while retaining original
source arrays. It is a declared shading change, not bitwise-original lighting,
automatic fallback, sampled-UV replacement, source-normal repair or omitted map.
R025 real shadow topology, winding, off-camera casters and depth/XY/PCF work do
not change with this shading choice. No per-FOV proof-only GPU emulator is introduced.
R027's actual radiance certificate, tone, FP16/Image and final publisher remain
unchanged. R030 geometry success does not complete R029 material admission.

## 11. Paired invariants and prohibitions

| Invariant | Paired prohibition |
|---|---|
| **INV-R030-AUTHORITY:** this candidate remains inactive until attributable coordinated adoption; later engineering/backend/native gates remain separate | **FORBID-R030-AUTHORITY:** no document presence, local commit, source receipt or GPU observation self-activates a profile/API or S3/S4 |
| **INV-R030-PARENT:** one exact complete original identity/capture with truthful source and signed-Mikk provenance under one Context family | **FORBID-R030-PARENT:** no partial chunk relabeled a complete original, digest-only trust, semantic editor authority or hidden global parent cache |
| **INV-R030-COVERAGE:** ordered contiguous positive ranges cover every original triangle/corner exactly once | **FORBID-R030-COVERAGE:** no holes, overlaps, reordered source, dropped UV-singular triangles or successful prefix called complete |
| **INV-R030-ABSOLUTE:** every map/certificate/signed lookup binds the same absolute original triangle and corner | **FORBID-R030-ABSOLUTE:** no packet-local triangle/index or resource slot masquerades as an absolute original identity |
| **INV-R030-MIKK:** canonical Mikk signed corners come from one full original primitive and retain inputs/configuration/revisions and each exact individual corner sign, separately from explicitly derived render XYZ | **FORBID-R030-MIKK:** no per-packet Mikk, fabricated original tangent, mixed-sign erasure/repair, unproved generator output or changed original normals/Mikk XYZ/W |
| **INV-R030-ROLE:** genuine absence uses NormalOnly; actual normal presence remains mapped with its true texture/certificate dependencies | **FORBID-R030-ROLE:** no invalid/present normal map relabeled absent, omitted or replaced by a dummy role |
| **INV-R030-SINGULAR:** each new mapped option requires exact original UV determinant zero, positive original face area and a declared source/packed interpretation; geometric rendering uses exact oriented face/longest-edge/tie proof | **FORBID-R030-SINGULAR:** no epsilon, inconclusive-zero classification, invented UV derivative, nonzero-UV escape, silent option switch or old-mode-C reinterpretation |
| **INV-R030-FRAME:** retain originals and individual signs; qualify either original-Mikk positivity or explicitly derived constant Fp/Up, with selected-frame matching-unit E_gpu<delta_source and full sampled-normal premise; old A/B/C 1/16 stays unchanged | **FORBID-R030-FRAME:** no positive original-frame claim on the 203 proved zeros, invented uniform sign, universal new 1/16 floor, hidden source repair/render-frame substitution, guessed GPU/error pass or assumed Bed frame success |
| **INV-R030-FINITE:** parent, packets, transient conversion, aggregate GPU/CPU/uploads and all retained generations have checked finite bills | **FORBID-R030-FINITE:** no unlimited parent, per-packet-only aggregate claim, uncharged copy/staging, cap invention or hidden streaming work |
| **INV-R030-ATOMIC:** complete set admission and complete truthful target publication are distinct atomic owner operations | **FORBID-R030-ATOMIC:** no exposed source prefix, partial Image, publication before all required packets or false GPU completion |
| **INV-R030-CURRENTNESS:** exact Context/source/proof/target generations and leases govern reuse, successors and retirement | **FORBID-R030-CURRENTNESS:** no stale/foreign capture promotion, early free, in-place mutation, strong cycle or old front relabeled current |
| **INV-R030-PIPELINE:** original R025 geometry/shadows and R027 same-Queue FP16/Image stay under the existing producer; derived shading is explicitly declared and subject to separate JC appearance acceptance | **FORBID-R030-PIPELINE:** no second renderer/Queue/publisher, skipped face/caster, unselected shader/quality change, hidden shading reinterpretation or authority transfer to a packet |
| **INV-R030-PROOF:** declared source, host, resource and actual observable backend evidence remain attributable and separate | **FORBID-R030-PROOF:** no private-data scan, numerical/backend/native pass, Bed support or performance claim from documentary status |

## 12. Future implementation and conformance contract

No test names below are claimed to exist or have run. A later bounded S2 task
must map these claims to exact filters in existing owner modules before execution.

| Existing owner | Required positive and reachable-negative proof |
|---|---|
| [three_d_geometry.rs](../../src/tests/three_d_geometry.rs) | Whole original versus complete multi-packet corner enumeration for unindexed/U16/U32, nonzero starts, seam duplicates, absolute certificate/sign indexing, exact ordered reconstructed P/N/UV/material association, and deterministic hole/overlap/map/refusal evidence |
| [three_d_context.rs](../../src/tests/three_d_context.rs) | One checked parent capture across packets/FOV, no full-copy/scan multiplication, per-packet plus aggregate/old-generation charges, failed final child rollback, live/wrong/retired Context, source/proof successors and release only after real leases close |
| Geometry/context public ingress | Unchanged old whole-original admission/refusals and genuine NormalOnly; original-frame option only with a usable source-positive certificate; explicit geometric option with exact face/edge/tie, bit-identical Fp/Up across corners, original uniform/mixed signs and truthful original/render correspondence; both require matching-unit finite-error and full sampled-normal evidence; old A/B/C 1/16 unchanged |
| [three_d_viewport.rs](../../src/tests/three_d_viewport.rs) | Real N1/N4 cold and warm complete sets, source/view/serial/material/parent/set association, multi-SurfaceId, failed successor old-front truth, no partial publication and actual current shadows/FP16/ordinary Image |
| [three_d_raster.rs](../../src/tests/three_d_raster.rs) and [three_d_backend.rs](../../src/tests/three_d_backend.rs) | Existing unchanged centroid normal/tangent/sign layout and normal/mip/Mask paths, independently predicted mixed-sign/interior-w=0 and packet-boundary discriminators, actual main/shadow draw coverage and Device limits; no new shader or mandatory per-view GPU-model proof |
| Original consumer, separately authorized | Complete original five-primitive Bed source/Mikk/certificate/range/resource receipt, then actual N1/N4 full source-to-shadow/scene/tone/FP16/Image and eventual native JC appearance; no asset substitution or private source transfer |

For a deterministic admitted small source, compare the complete old whole-original
render and packet-set reconstruction with the same normal/frame/material and
placements. Compare all original corner/source identities and available R025
transforms/active faces/depth/coverage witnesses. Actual FP16/Image comparison
must use independently declared predictions and owner-authorized criteria;
source equality alone is not an assertion of measured GPU bit equality. When
testing the derived geometric option, compare the **same declared derivation**
on both whole-original and packetized controls; never claim equivalence to
original smooth/Mikk shading. That visible interpretation change stays explicit.

Future geometry/context proof must independently verify the chosen interpretation.
For original-frame rendering, expand the six-control identity/weights and check
any consumed source delta, packed coefficients, complete finite patch coverage
and norm/squared-norm units. For geometric rendering, verify original winding,
positive area, exact three-edge comparisons and first-maximizer tie rule, ideal
orthogonality, packed Fp/Up identity at all corners, source/derived revisions and
unchanged individual original signs. Include deterministic cases with each edge
as the unique winner and exact ties, plus reachable misdeclared edge/tie/constant
frame evidence. No rounded longest-edge guess or invented UV inverse qualifies.
Both require certificate domain/currentness, actual definedness and the selected
render frame's strict E_gpu<delta_source condition, full sampling and current
source/target proofs. Preserve old A/B/C 1/16 tests. Neither a failed sufficient
test nor the now-proved original-frame zeros authorizes silent source repair.

The supplied canonical output, numerical search and exact source follow-up in
section 1 are not a complete accepted runnable frame receipt. The 25,007 numeric
candidates have not all been independently outward-certified; the reported 2,235
positives and three later zero-free faces need usable attributable certificates
if original-frame rendering is selected. The formerly unresolved 206 are now
203 exact original-frame zeros and three zero-free, with none unresolved there.
No positive original-frame bound can close those 203. The geometric option instead
requires its complete source-derived/packed frame and provenance proof; it does
not depend on numerical witness search counts or original-frame root detection.
The 1,686 sub-1/256 witnesses remain distinct from zero proofs, and reported L
is not a GPU margin. Do not add overlapping categories into a fabricated full
qualified count. Complete real 13-level/filtered/normal-scale qz, selected-frame
finite-error coverage and predeclared backend observations remain required.
Do not widen brute force on proved zeros, replace real triangles by synthetic
success, invent thresholds or silently weaken A/B/C tests. Genuine absence and true
mapped zero-UV are separate positive claims. Preserve source/radiance, current
R025 work and R027 lifetime tests; unchanged-input and FOV checks must show no
new source scans or obsolete per-view GPU-model sweeps.

## 13. Explicit decisions required before activation and engineering

| Gate | Required owner decision/evidence | Present disposition |
|---|---|---|
| S1 coordinated scope | Adopt typed parent/set ownership and the two explicitly distinct exact-zero mapped interpretations: qualified original-frame MikkUvSingular or source-derived geometric render frame, with original individual signs, truthful provenance and finite-error gates; qualify only the new class, preserving old R024 A/B/C 1/16, signs and weights | NOT ADOPTED; this document is the corrected S0 candidate |
| Finite effective profile | Select concrete parent/set/packet/certificate/work/CPU/transient/GPU/upload/retained-generation limits, ledger mapping and actual Device ceilings from the complete source/resource bill | OPEN FOR ARCHITECTURAL DECISION before S2; no numeric caps selected here |
| Source/frame acceptance | Actual full original/canonical-Mikk capture and exact parent/corner/role evidence; for original-frame rendering usable positive norm certificates, or for declared geometric rendering exact face/edge/tie derivation and constant packed Fp/Up with original w and distinct shading revision | OPEN; former 206 classified as 203 exact zeros plus three zero-free, none unresolved; no full original-Mikk positivity or complete geometric admission inferred |
| Remaining interpretation-specific source proof | Original-frame certificates apply only where truly positive; geometric certificates instead verify original geometry and declared render-frame derivation/packing without root classification; retain finite charged verification and original provenance | OPEN pending actual submitted/checked certificates; no wider search on 203 proved zeros, default fallback, fabricated original, new threshold or source repair |
| Source-to-GPU margin | Independently certified finite E_gpu on the actual supported source/host/shader/backend domain, with matching-unit E_gpu<delta_source for the selected packed render frame and all shader operations finite/defined | S3 BLOCKED while absent, unverified or failing; neither reported L nor ideal geometric norm of one is a GPU/f32 safety certificate |
| Full sampled-normal and numeric domain | Complete actual 13-level/filter/sampler/normal-scale positive-qz source bound and f32/Metal interpolation/GS/mapped-normal/derivative behavior under R028; level-0 blue203..255 alone is insufficient | OPEN; symbolic nonzero proof is not source-wide numeric/backend acceptance |
| S2 engineering | Separately authorize the exact typed public ingress and internal owner changes, generation/error mapping, full reservations/rollback and nearest-owner tests | NOT AUTHORIZED by this artifact |
| S3 backend domain | Independently accept the versioned source/backend envelope only with the validated source-to-GPU margin and necessary predeclared real N1/N4 observations, including singular-frame and packet/source seams, under unchanged R028 criteria | CLOSED and blocked without the finite-error certificate; no sampling-only worst-case claim, universal GPU theorem or runtime readback admission |
| S4 native/product | Actual complete Bed originals preserved in one native application PBR canvas, separately admitted material response, current shadows/front/lifetime, measured performance and JC physical appearance decision with disclosed geometric shading differences | CLOSED; no bitwise-original smooth/Mikk appearance claim; R029 and source/container/UI owners remain independent |

Semantic closure here does not claim a finite profile, source witness or runtime
implementation has been supplied. Those are explicit gated decisions/evidence,
not placeholders for guessed production values. No remaining gate authorizes
an easier substitute asset, partial original or weaker frame predicate.

## 14. Documentary verification and non-goals

Verify this corrected inactive file's full source, headings, local link targets, paired
invariant/prohibition identifiers and inactive status. Review section 3 against
`geometry.rs` 962–1037 and 1535–1775, the additional actual packed guard at
1782–1805, `material_api.rs` 280–460 and its conversion continuation, the actual
`pbr.rs` 899–934 centroid/GS/interpolated-sign code and `direct_shadow.rs`
1999–2010 composition. Independently check the exact local oriented face,
three ordered edges, exact maximum/tie rule, orthogonality/unit proof, constant
render XYZ and packing-witness inequality. Check the unchanged original versus
explicit derived identity and each retained sign against shader consumption.
For the optional original-frame route, check the six controls' expansion,
nonnegative unit-sum weights, positive-bound/patch/norm units and source-zero
exclusion. For both routes check the mapped-vector norm identity and strict
finite-error/full sampled-normal gate on the selected render frame. No selected
exact-zero clause may retain the superseded uniform-sign, independent-nine or
universal 1/16 rule, describe 203 exact zeros as still unresolved, or require a
positive original-Mikk proof to select geometric rendering. Old source guards
and adopted A/B/C 1/16 remain accurately described. Check
changed lines for whitespace and verify that all other working bytes/staged
entries remain unchanged. No external document or private asset is required to
interpret this candidate; parent specifications supply reference boundaries.

Documentary verification establishes only that this complete candidate is
internally consistent and source-grounded. It runs no Cargo/GPU/native test,
performs no Bed intake, and changes no operative source/API/ABI/policy/material/shader.
There is no container/wire design, renderer, global registry, per-chunk Mikk
service, background work, geometry simplification, source-frame repair or
selection of R029's response mathematics in this unit.

The candidate and precise remaining gates require independent S1 review.
Recording this candidate is neither adoption nor permission to start
implementation. S3/S4 remain CLOSED.
