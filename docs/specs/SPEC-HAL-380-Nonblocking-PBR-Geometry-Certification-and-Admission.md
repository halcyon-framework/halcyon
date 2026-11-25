# SPEC-HAL-380: Nonblocking PBR Geometry Certification and Admission

**Author / product owner:** JC
**Status:** Adopted / Active / Binding [narrow S1 technical contract, 2026-11-05]
**Technical adoption:** Independent architectural review under the maintainer's delegated technical authority.
**Implementation:** The original four-stage HAL source handoff is implemented at local revision `443872015d43b7350b51410460e805da0d65b763`. The production cost supplement below has separately required implementation and source tests; neither status admits application integration or native latency.
**Owner layer:** Existing Context3D qualified PBR source admission; CPU-only producer proof; current resource registry, not a new engine service
**Parent law:** Halcyon Constitution HAL002/HAL002A; HAL355, HAL357, HAL370, R024, R025, R028 and HAL373 only for the existing CPU custody/wake pattern. No parent amendment, R-series normal math or material/profile acceptance is changed.

**Narrow S1 supplement, 2026-11-05:** The adopted production CPU cost and receipt supplement adds only the public accounting contract in §2.1/§3. The original four-stage handoff retains its adopted scope.

## Pre-Flight

- Objective: remove the measured synchronous source-certification portion of the native asset inspector's final UI selection cost, while preserving the complete original R024 geometry/certification/successor and Context registration law through the SAME HAL independent mathematical verifier.
- Standing objective: the native user can choose a real prepared 3D asset without a long UI callback, while original full PBR and complete current/compatible previous source remain truthful and live. Separate HAL372 Object Reveal is a finished source/render S2; the original consumer integration was a separate work item at this baseline.
- Governing decision: this S1 extends only HAL Context's geometry CPU verification/admission path, leaving the existing synchronous API byte/semantic compatible. Parent normal and material profile and source certificate requirements remain mandatory.
- Historical source baseline and measurements: original native asset selection at Context3D::admit_pbr_geometry measured Barrel N1 19.190ms, chair N1 66.327ms, Sofa N4 19.744ms; total UI 23.074/72.864/25.886ms. These are one-run original attributable native samples, not a universal performance bound. Current material_api.rs:416-515 converts original and render CPU inputs; geometry.rs:1442-1518 try_new_certified allocates/validates all actual source and check_certified_input; pbr.rs:767-850 register_pbr_geometry invokes geometry.rs:1531-1544 validate_certified_registration, repeating the full expensive mathematical check before Context resource registration.
- Scope and existing owners: src/three_d/{material_api.rs,geometry.rs,pbr.rs,context.rs,mod.rs} and real nearest internal source/test owners only. No change to HDR shaders, R025/R028 GPU math, unrelated HAL372 effects, current R024/R028/HAL370/proposal foreign document edits, application source, original assets or artifact formats.
- Reuse decision: reuse exact existing g::PbrGeometrySource3D::try_new_certified/check_certified_input/validate_certified_successor, typed PbrGeometryDescriptor3D and Context3D register_controlled_source machinery. HAL373 normal_cpu and HAL372 object_reveal demonstrate CPU-only sealed and caller-worker patterns, but DO NOT give their specialized normal/reveal jobs ownership of geometry.
- Named invariants: INV-380-SINGLE-VERIFIER, INV-380-EXACT-SOURCE, INV-380-CPU-ONLY, INV-380-ONE-USE, INV-380-LIVE-CONTEXT, INV-380-FINITE-ESCROW, INV-380-SUCCESSOR, INV-380-OLD-API, INV-380-UI-BOUNDED, INV-380-LAST-GOOD.
- Forbidden: fake source certificate, producer Boolean or hash-as-verifier, math bypass or weakened normal cone/C frame/tangent/UV/Jacobian tests, material alpha repair, invented owner, Context/Device/Queue/GPU lease on CPU worker, moving registry mutation off UI, scanning original vertices or certificate elements on UI, an extra thread/scheduler/clock, sleep/polling, ENV feature gate, unbounded source retention, an arbitrary cache that replaces provenance.
- External boundaries: The application supplies immutable actual CPU input and existing one CPU worker. HAL independently verifies source and admits its Context resource; no consumer authority becomes HAL semantic state. Existing HAL355 complete fronts stay source-qualified.
- Test ownership: existing src/tests/three_d_context.rs, three_d_geometry.rs, three_d_snapshot.rs and nearest R024 normal/source tests; the original native asset-inspector fixture is a separate consumer proof. No standalone root test invented.
- Required verification: the existing `hal380_`, `hal373_` and `hal372_` test filters and the Halcyon package build must prove source mathematics, Context ownership and compilation. Record actual outcomes; this requirement supplies no result.
- Completion: a buildable public CPU-only source preparation/checked successor handoff and short Context final registration with complete old source API retained; source-level tests and proven no UI long loops. Physical Inspector latency cannot be claimed resolved before the original application integration and a native timing comparison.
- SemVer: additive public API classified under current Halcyon version/compatibility policy; no guessed version edit or new dependency.
- Out of scope: HAL372, 3D outline/HUD/text effects, geometry source redesign, importer, manufacturer semantics, general asynchronous Context registry, replacement of HAL373, dynamic asset remeshing.

## 1. Exact owner problem and decision

The current public Context3D::admit_pbr_geometry, in material_api.rs:416-515, calls conversion into PbrCertifiedInput3D and PbrVertex52, then g::PbrGeometrySource3D::try_new_certified. geometry.rs:1512 checks the ENTIRE real source and certificate and binds immutable certified frames. The next public registration internal pbr.rs:772 calls validate_certified_registration, which rechecks the SAME immutable input at geometry.rs:1533. It is illegal to remove the only mathematical check or to call registration with a fabricated pass Boolean.

**Selected architecture:** first run the EXISTING full HAL geometry converter/checker on the consumer's already-existing CPU worker, with immutable CPU input and source-qualified opaque result; then consume that HAL-sealed result through exact current Context predecessor admission in a short UI phase. ALL long original math, conversion and potential original/successor whole-data comparison occur on CPU, not the UI. HAL adds no worker or scheduler. The original synchronous operation continues to invoke all of its original validations.

## 2. Selected public and internal CPU contract [CLOSED S1]

1. Public PbrGeometryCpuPrepared3D::prepare(input:PbrGeometryDescriptor3D,maximum_cpu_bytes:usize,cancel:Arc<AtomicBool>)->Result<PbrGeometryCpuPrepared3D,PbrSourceError3D>. This runs on the caller's existing CPU worker. It MUST execute the exact original public conversion, original/render/indices/identity and normal+certificate math through shared existing HAL kernel. Cancellation checkpoints may only stop between bounded original work items without changing noncancelled results. The returned opaque non-forgeable sealed CPU value owns exact immutable normalized original/render source, checked frames/metadata/work and original source provenance. Its rust-visible private source is CPU-only and must be Send+Sync. The producer maximum is an upper cost bound, not an authority credential; HAL calculates full captured and peak transient allocation size independently with checked arithmetic before memory allocation and returns typed capacity errors. No Context, Device, Queue, original GPU source lease or Semantic Scene enters this preparation.
2. Public Context3D::begin_pbr_geometry_cpu_admission(&self, prepared:PbrGeometryCpuPrepared3D,previous:Option<&PbrGeometry3D>)->Result<(PbrGeometryCpuSuccessorJob3D,PbrGeometryCpuPending3D),PbrSourceError3D>. This runs on UI. It checks current Context/Device, exact prepared issuer authenticity, actual resource hard/device/policy limits, nonzero and finite lengths and escrow before capturing immutable previous CPU referents. Capture old PbrGeometrySource3D by O(1) cloning its immutable Arc arrays + packed certificate Arc, NEVER its PbrGeometryData3D GPU cache or ControlledResourceLease in the Send job. The UI !Send Pending alone keeps actual Context, verified predecessor handle/generation and required controlled retention/CPU escrow until worker proves exact successor. Begin performs O(1) scalar/Arc-key checks, no loop over vertices/indices/certificate data.
3. Public PbrGeometryCpuSuccessorJob3D::verify(self,cancel:Arc<AtomicBool>)->Result<PbrGeometryCpuResult3D,PbrSourceError3D>. This CPU-only Send job compares eligible original predecessor and new immutable HAL-sealed source using the SINGLE existing g::PbrGeometrySource3D::validate_certified_successor kernel, including same logical asset, original revision, changed original bytes/vertices/indices/tangents/certificate records and exact borrowed source origin. When previous None, no prior comparison is invented; a legitimate independent CPU result still seals that absence. Result is one-use non-Clone, bound to one private Context issuer and exact pending ticket/predecessor generation and the current immutable original source; no Context/GPU mutable storage crosses into CPU job.
4. Public Context3D::finish_pbr_geometry_cpu_admission(&self,pending:PbrGeometryCpuPending3D,result:PbrGeometryCpuResult3D,previous:Option<&PbrGeometry3D>)->PbrSourceResult3D<PbrGeometry3D>. This runs on UI only. It checks Context/Device live, exact ticket/source/source revision/previous kind slot/generation & still-live association and any policy limits that may change, then performs current controlled PbrGeometry registration. It MUST NOT invoke full try_new_certified, validate_certified_registration/check_certified_input, validate_certified_successor or iterate variable-sized CPU vertex/index/certificate contents. A typed private HAL-only fully verified geometry value enables the internal registrar after these full mathematical proofs; ordinary public unsealed input cannot enter that bypass. The original current Context registry, source generation, quota/slot behavior, retire/last-good and GPU lease remain authoritative.
5. The original Context3D::admit_pbr_geometry(PbrGeometryDescriptor3D,Option<&PbrGeometry3D>) and all current callers retain exact signatures/behavior and identical valid/refused source semantics. Internally share the same conversion/checker and controlled registrar owner, without a second divergent verification kernel. A shorthand pass flag, digest comparison, or producer saying "verified" is not HAL proof.
6. Exact source identity is original PbrOriginalIdentity3D including source/node/mesh/primitive/revision and current Context predecessor controlled key/generation; certification cannot be rebound to a different original or a stale predecessor merely because visual positions match. No rounded float-based key; compare actual existing scalar bits/identity where original source contract requires them.

### 2.1 Production scalar cost preflight and success receipt [ADOPTED S1 supplement]

Expose immutable `PbrGeometryCpuCost3D` through the existing public `three_d` module, with private fields and `usize` accessors `input_cpu_bytes()`, `additional_peak_cpu_bytes()` and `additional_retained_cpu_bytes()`. Add `PbrGeometryCpuPrepared3D::cost_for_input(&PbrGeometryDescriptor3D) -> PbrSourceResult3D<PbrGeometryCpuCost3D>` and `PbrGeometryCpuPrepared3D::cpu_cost(&self) -> PbrGeometryCpuCost3D`. The former is pure CPU accounting before moving the descriptor into preparation; the latter returns the successful sealed value's corresponding receipt. No external constructor or cost receipt grants geometry certification, Context authority, or permission to skip a check.

One HAL-owned checked formula, `geometry_cpu.rs::preparation_cost`, factors the original `preparation_peak` calculation and its exact allocation-identity deduplication to compute both receipts. `input_cpu_bytes` counts unique immutable public-input Arc allocations, including headers, once within this descriptor. `additional_peak_cpu_bytes` bounds concurrent HAL-created conversion Vec/Arc copies, source/certificate/checker buffers and fixed lifecycle overhead relative to that input. `additional_retained_cpu_bytes` bounds newly created private original-vertex, certificate, render-source and checked-frame allocations and their fixed overhead after successful preparation; moved/shared input Arcs are excluded from both additional terms. Conservative fixed lifecycle overhead remains explicit; it is not another charge for original source arrays. Check all size arithmetic and `additional_retained_cpu_bytes <= additional_peak_cpu_bytes` before allocation. Preflight and success receipts must be equal for the same immutable descriptor, including its actual within-input aliases.

`prepare(input, maximum_cpu_bytes, cancel)` keeps its signature and independently enforces the complete producer peak, `input_cpu_bytes + additional_peak_cpu_bytes`, before conversion. Actual new receipt storage belongs in the existing fixed-overhead calculation. The cost API performs no source proof, invokes no Context, and traverses no vertex/index/certificate elements; it inspects the existing fixed set of Arc identities, lengths and layouts. Existing synchronous source/refusal semantics and all four admission signatures remain unchanged.

## 3. Bounded CPU residency, cancellation and failure [CLOSED S1]

- **Adopted / Active / Binding S1 — final Context accounting scope.** This adopted clarification to §§3–4 retains the maintainer's technical authority and product veto. Producer's original descriptor CPU allocations retain their source/caller quota; HAL independently accounts immutable resulting normalized source, captured original source/certificate metadata, transient conversion/checker scratch, per-packet bindings and actual additional Arc control memory. Before cloning issuer/predecessor CPU referents on UI, reserve a real current Context charge using the established Context-owned dynamic/CPU reservation and escrow. While that `Context3DCore` is alive, its HAL380 CPU reservation and eligible predecessor escrow MUST remain charged and retained until all worker-owned immutable source references and the associated strong ticket have been released. Pending drop, cancellation, unmount, Device loss, revocation and dropping a nonfinal Context clone permit no early debit or reuse. The live collector may release a retired node only after the ticket has no strong owner and worker source custody has ended. Only destruction of that same core upon release of its last strong owner ends its finite quota, registry and admission domain. At that boundary its retired HAL380 CPU reservation and optional predecessor escrow MAY be dropped and debited while a Job or Result remains live. The debit records the end of Context accounting. Candidate and optional prior CPU backings remain valid under their actual strong Arcs until the last reference releases each backing. No live sibling Context receives the ceased quota. Independently held Queue, front and generation leases follow their existing lifetime rules.
- The UI Pending is !Send and owns the actual quota reservation and predecessor lease/revocation. CPU Job and Result are Send+Sync and keep only immutable source Arcs, checked scalars, the inert issuer/Binding and ticket, and cancellation tokens. They MUST NOT hold, directly or transitively, a Context3DCore, Device, Queue, GPU cache, registry reference, controlled lease, lease-bearing data object, Context quota owner or globally published custodian. UI cancellation and final teardown remain nonblocking without join/recv, a polling-only UI scheduler or a hidden global. The inert ContextId and ticket confer no ability to finish admission after final core destruction. A stale Result remains a refusal and cannot reattach, admit or publish under a new Context, source, slot or front. An independently active caller/task CPU budget that accounts for worker buffers surviving final Context destruction MUST retain the relevant charge under its own owner until its last covered worker Arc releases the backing. A CPU maximum argument or cost receipt supplies no such reservation. HAL373 §7 governs normal-certificate shutdown in its separate scope and supplies no evidence of this caller charge. This clarification establishes no process-global CPU high-water after final Context teardown.
- The original source remains eligible and the old complete source/front remains available when the optional prepared admission fails. For a new selected source, no half geometry handle or partially valid model/Scene can be installed; all existing transactional admission and rollback remain. Device loss, mismatched Context, source revocation, changed same-id bytes, capacity and altered previous generations each produce real typed refusal at the current legal owner.
- Current source math remains exactly HAL's R024/R028 normal and frame certificate law; the CPU move is a scheduling/admission operation, not source qualification weakening. No extra normal proof or worker-global source registry; HAL373 normal chain is independent.
- For the selected application integration, let `B` be already charged request-owned source/descriptor/other live baseline bytes, `R` the checked sum of earlier successful HAL prepared additional-retained receipts, and `M` the existing request maximum. Before each preparation, checked `B + R + cost.additional_peak_cpu_bytes() <= M` must hold. Call HAL preparation with `M`, compare its `cpu_cost()` to preflight, then checked-add only `additional_retained_cpu_bytes()` to `R`. Failure/cancellation is not a successful retained debit. After all geometry, the unchanged normal protocol receives checked `M - B - R`. The caller's baseline deduplicates shared source allocations across primitives; HAL's additional terms never charge those allocations again. Counts alone, identical per-primitive remaining quotas, arbitrary quota splits, private-size reimplementation or Debug parsing are not aggregate proofs.
- These scalar receipts do not reserve Context capacity. Begin/finish retain their independent live Context charges and predecessor escrow. Caller-owned input references remain charged by the caller while physically retained. No receipt changes source order, normal work/digest law, total quota, thread ownership or last-good eligibility.

## 4. Conformance and acceptance [CLOSED S1]

- Differential old synchronous versus new CPU-prepared path on REAL original supported geometry, mapped + absent normal, source tangent provenance, sharp/smooth/C modes, original/render seam expansion, U16/U32/unindexed and changed matching source successor: exact certified frame/source bytes, successor acceptance/rejection and resource identities, no weaker certificate outcome, including known RED invalid originals.
- A challenging original chair-like high triangle count demonstrates O(1) UI begin/finish with zero source vertex/triangle/certificate iteration; actual full original verification on worker. Explicit worker-thread identity, no Send Context/Device/GPU/lease, pending !Send compile guard, one-use result, reliable cancellation before/during/after. Real causal source tests MUST exercise `None` and `Some` with outstanding Job and Result under both Context lifetimes. While the Context lives, prove that charge and optional predecessor escrow survive Pending retirement through actual worker source and ticket release, with lawful collection afterward. At final destruction of the same core, prove that quota termination and retired escrow drop may precede worker Arc release while the held backings remain valid and stale results cannot reattach, admit or publish. Prove one-use currentness, counter and lease lifetimes, nonblocking UI and independent Queue and last-good custody. Any claimed surviving caller budget requires its own charge-lifetime proof. **Source-level conformance for this final-Context accounting clarification — independently VERIFIED / PASS on 2026-11-08.** Independent review verified the eight causal cases (`None`/`Some` × `Job`/`Result` × live/final Context) and the foreign FINISH control at `abad4903cf9fe1cae4d65fe55a87f7cbeec7153f`. Independent Grip runs exited 0 for `hal380_` (19/19), `hal373_` (13/13), `hal372_` (19/19), the Halcyon package build, and the targeted `PbrGeometryCpu` compile-fail doctests. Surviving caller/task CPU charge through final Arc release and process-wide CPU high-water remain unproven. [Eight causal cases](../../src/tests/three_d_context.rs#L3332-L3565) · [Foreign FINISH control](../../src/tests/three_d_context.rs#L3567-L3599)
- Current Context resource/device caps, last-good, old/new concurrent source and correct geometry/snapshot/normal+PBR rendering remain complete; original HAL372/373 paths still compile and test; N1/N4 original-source render and the original native application integration are separately qualified. This S1 contract does not declare a production improvement until a same-source measured canonical native comparison and actual physical click observation.
- Named assertions required: INV-380-SINGLE-VERIFIER, INV-380-EXACT-SOURCE, INV-380-CPU-ONLY, INV-380-ONE-USE, INV-380-LIVE-CONTEXT, INV-380-FINITE-ESCROW, INV-380-SUCCESSOR, INV-380-OLD-API, INV-380-UI-BOUNDED, INV-380-LAST-GOOD. Typed refusals required for deliberately invalid source, changed previous, Context mismatch, unavailable capacity, cancellation and Device failure.
- Cost-supplement proof requires actual two/multiple-primitive preparations with already charged source/descriptor input, differing and shared Arc allocations, exact preflight/success equality, peak admission at each step, additive retained receipts and a subsequent original normal proof against the same remainder. Verify checked overflow, pre-allocation budget refusal, cancellation, and existing Context escrow. Existing differential source/math/successor and zero-UI-source-visit tests remain required; this supplement supplies no native timing acceptance.

## 5. Disposition

ADOPTED / ACTIVE / BINDING S1 for this narrow measured HAL-only geometry certification/registration split. The prior immutable source and Context registry own all truth; this is not a new generic CPU/GPU subsystem. Implementation and independent verification retain this ABI, worker and semantic contract. Any proven source contradiction requires a decision by the governing contract owner. No S3 source-input-domain/backend-capability-envelope acceptance, native-consumer acceptance or maintainer appearance acceptance is inferred.

## 6. Dual-class complete CPU source ingress clarification

**Proposed / Inactive / Non-binding.** This candidate includes the primitive-wide origin clarification and coordinates [R024 §18](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#18-proposed-s1--two-source-qualified-geometric-rendering-classes) and [HAL370 §14](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#14-proposed-s1--coordinated-admission-of-two-derived-source-frames).
Sections 1–5 govern as written. This appendix grants neither admission nor implementation authority. The maintainer retains constitutional and product authority and final veto.

The numbered prepared-source records used below are local notation for an inactive producer-format candidate. They are not current Halcyon public API types. Their required source, capture, byte-association and mathematical checks are stated in this appendix; independent producer qualification and format adoption remain separate open prerequisites.

### 6.1 Pre-Flight

- **Objective** — Specify how one complete mixed-class input could reach HAL380's existing CPU proof and Context admission.
- **Standing objective** — Preserve faithful original source and complete, responsive PBR admission for supported assets.
- **Governing authority and status** — HAL002/002A control. HAL355, the adopted HAL370 and R024 clauses, R025, R027, R028 and HAL380 §§1–5 govern. The dual-class appendices are inactive.
- **Authority change, if any** — None. The proposed addition to §§2 and 2.1 depends on independent adoption with the source-class clauses.
- **Target layer and existing state verified** — HAL CPU geometry input, proof, checked successor and UI registration at `9b2604734d255f2523d46c9fe853c4d6fc547445`.
- **Owned mechanisms and files** — This appendix in SPEC-HAL-380 only.
- **Named invariants** — INV-380-SINGLE-VERIFIER, INV-380-EXACT-SOURCE, INV-380-CPU-ONLY, INV-380-ONE-USE, INV-380-LIVE-CONTEXT, INV-380-FINITE-ESCROW, INV-380-SUCCESSOR, INV-380-OLD-API, INV-380-UI-BOUNDED and INV-380-LAST-GOOD.
- **Forbidden behavior** — Fabricated legacy proof, fallback class selection, proof bypass, unbounded proof work, a second verifier or registrar, and partial publication.
- **Existing mechanisms inspected and reuse decision** — Reuse the private certified input, checker, HAL380 sealed handoff and controlled Context registry.
- **Relevant external or dependent boundaries** — The qualified source producer owns Mikk capture in the controlled pack transaction. Prepared input and wire changes require separate adoption by their source-producer owner.
- **Test ownership and placement** — N/A for this document unit. Existing HAL geometry, admission and rendering owners retain later proof responsibility.
- **Output or artifact completeness boundary** — One self-contained inactive appendix. Qualified producer and completed-capture evidence and bounded proof accounting still require exact owner decisions before adoption.
- **SemVer and changelog impact** — N/A. Documentary proposal only.
- **Out of scope** — Binding adoption, Rust and API design, producer or prepared wire changes, shader changes, source math execution, tests, GPU and native execution.

### 6.2 Complete typed input and shared owner

Today the public `PbrGeometryDescriptor3D` requires one
`PbrTriangleCertificate3D` with an existing `PbrFrameCertificate3D` on every
triangle. The converter lowers those alternatives into the private
`PbrCertifiedInput3D`. The current `prepare` and `cost_for_input` accept
only the legacy descriptor. A mixed source cannot truthfully use that
descriptor for its Z or N triangles.
[Public types and conversion](../../src/three_d/material_api.rs#L370-L543)
· [Private evidence](../../src/three_d/geometry.rs#L626-L682)
· [Worker preparation](../../src/three_d/geometry_cpu.rs#L234-L270)

The proposed additive preparation ingress shall take one complete immutable
geometry input with original and render attributes, ordered indices, exact
corner mapping and checked cost facts. One closed evidence sum shall cover
every original triangle in source order. Each record shall bind the original
source/node/mesh/primitive/revision, exact triangle and corner association,
and its own certificate identity and generation. Legacy A/B/C and genuine
normal-role absence retain their full existing meaning. Z
(`PbrDefaultMikkZeroGeometricFrameV1`) and N
(`PbrDefaultMikkNonzeroOutOfDomainGeometricFrameV1`) are distinct
producer-declared alternatives with their own proofs. Fabricated OldCone,
SourceGeometric or prepared R14 stand-ins, override maps, partial inputs and
automatic fallback are forbidden.

Both inputs shall lower to the same private certified source and triangle
checker. The additive worker entry shall produce the existing HAL-sealed
`PbrGeometryCpuPrepared3D` for the same Context registrar. The existing
public descriptor and frame enum, synchronous `Context3D::admit_pbr_geometry`,
and existing `prepare` and `cost_for_input` signatures and behavior remain
unchanged. The proposal selects no new public API name, field layout or ABI.
[Shared verification entry](../../src/three_d/geometry.rs#L1558-L1573)
· [Context registrar](../../src/three_d/pbr.rs#L784-L790)

For qualified evidence the Legacy descriptor cannot express, this inactive
proposal selects one HAL-owned additive-only CPU preparation entry family
on `PbrGeometryCpuPrepared3D`, reachable from an external source-producer crate
with the checker and registry private. A pure bounded cost view and its
matching worker preparation interpret the same complete immutable input,
caller ceilings, effective Context snapshot and checked receipt rules
under §8.7. The qualified source owner supplies the single §8.9 capture
and exact O/Eo/K backings across rebinding, with their actual index kind,
bits and Options, the full original/render/corner association, the closed
ordered roster and flat proof backing, and interpretation/certificate
revisions. Prepared input comes from the producer's qualified pack or checked
R41 reopen. Source-only input requires equivalent controlled source facts.
The cost view plans from scalars, identities and checked lengths only.
The worker checks byte, item and work bounds before untrusted roster scans
or dependent allocations, then validates roster, source transfer and class
proofs with bounded cancellation in the same checker. Missing or mismatched
qualification gives typed refusal. Only full success seals the existing
result with the matching checked receipt for the existing successor and
sole registrar. UI BEGIN/FINISH stay O(1) in source size. API spelling,
type construction and visibility, field layout and ABI, actual invocation,
the original-source and prepared-reopen qualification described in §8.9 and producer schema2 adoption, common FP/Legacy parity,
checkpoints and §8.17 soundness, and full work, Layout, peak and successor
receipts remain open.

### 6.3 Callback evidence and class eligibility

For Z and N, the qualified producer verifies the actual original glTF/GLB
root and closure, authored TANGENT absence and the present normal role in
the controlled pack/source transaction. Invoke pinned default-Mikk there
and require successful return and complete final signed vec4 assignments.
Bind original identity, revision and digest, whole ordered input and
corner capture, producer revision, generator version/configuration/checksum,
adapter revision, every final result at its exact indexed corner and the
separate generated-frame proof. Keep all raw bits. Every output shall be
finite with exact w of `-1` or `+1`. The final slots establish coverage under
R024 §18.2 and supply no at-most-once setter evidence.

Ordinary prepared opening would verify root and binding, digests, exact
ordered captured data, configured producer and proof metadata without
regenerating Mikk. This establishes structural association of
producer-attributed claims. It supplies no independent historical-run
proof against a hostile full-HAC rewrite with recomputed hashes.
An externally trusted asset-origin channel for that stronger claim is
outside the selected S1 boundary. No signature, attestation or auxiliary
service is selected.

HAL independently checks original and render counts, complete ordered
input and output coverage, exact bits and corner mapping, and original
normal and signed Mikk source conditions for the declared class. Bind
distinct interpretation and certificate versions before the exact frame
and numerical proofs. Retain material, Context, generation and lease
checks and the sealed HAL-only
verification result. The current producer/revision/version presence gate
alone does not establish provenance or final coverage. A digest, source
name, `invocation_completed` flag or arbitrary producer assertion cannot
replace qualification of the controlled capture or HAL verification.
Refuse an unqualified source.
[Capture boundary](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#182-proposed-zero-callback-source-class)
· [Current provenance gate](../../src/three_d/geometry.rs#L1754-L1765)

- **Z.** Require at least one original callback with exactly zero xyz.
  Bind each such raw record, including signed zeros and the actual w, to
  its exact original triangle and corner and to its checked Z
  evidence before both the raw `frame_unit` gate and the per-triangle
  smooth-candidate normalization. Only those zeros are exempt from
  normalization. Keep the raw w and all original records, verify all
  nonzero callbacks under their required checks and never fabricate a unit
  tangent for a zero.
- **N.** Require all three raw xyz vectors nonzero. Bind the original
  smooth triples and the actual normalized and packed smooth triples to
  the same source and checked normalization/packing revision. Before
  accepting the derived frame,
  prove all three exclusions from R024 §18.4.
  1. An outward upper bound strictly below `3/4` on at least one required
     original Mode A unit normal or tangent cone dot product.
  2. For all nine exact packed smooth crosses `c_ij = N_i × T_j`, full
     nonnegative rational weights including zeros, with sum one and
     `q = sum(w_ij * c_ij)`. Check all nine KKT inequalities
     `q · (c_ij - q) >= 0` and equality on the positive support. Require
     exact `q · q < 1/256`, which bounds the best possible Mode B
     unit-separator lower bound strictly below `1/16`.
  3. In each of the original and actual packed smooth domains, a finite
     nonzero homogeneous separator for each sign `s` in `{+1,-1}`.
     Require strictly positive outward lower bounds for every
     `a_s · N_i` and `-a_s · (s*T_j)`. This excludes both parallel and
     antiparallel nonzero common rays. The sign test is independent of w.

Missing, stale, malformed, unresolved or ineligible evidence shall refuse
the entire geometry. A failed search or failed legacy certificate supplies
no eligibility.
[Raw normalization](../../src/three_d/geometry.rs#L1767-L1781)
· [Smooth candidate](../../src/three_d/geometry.rs#L1844-L1859)
· [Existing C singularity check](../../src/three_d/geometry.rs#L1896-L1905)

**Primitive-wide no-tangent origin.** Only the additive complete geometry
CPU input may carry private `NotRequiredNoNormalRole` for a whole primitive.
Require an unmapped geometry capture, producer-verified authored TANGENT
absence from the exact original GLB/glTF, genuine absence of the original
material normal role, no Mikk invocation or generated capture, and an
empty original signed tangent stream.
Bind those facts to the original source identity and revision, material
assignment, primitive and occurrences, preserving all original attributes
and order. At material association the existing HAL snapshot/material
owner independently verifies the actual normal role against the retained
current lease. A mapping flag or empty bytes alone establish neither
absence fact. Refuse this origin if an authored tangent exists or a present
normal role is invalid. Preserve the actual source in either case.

Every original triangle retains the existing R024 §7.1.4 normal-only
proof through the old normal cone or certified packed unit-d projections
at least `1/16`. This origin excludes mapped A/B/C, Z and N evidence. Keep
the finite, unused PBR52 tangent/sign field under R024 §8.4.3. It supplies
neither an original signed tangent nor Mikk provenance. The same HAL
checker, sealed worker and Context registrar own admission.
[Original stream counts](../../src/three_d/geometry.rs#L1070-L1097)
· [Normal-only proof](../../src/three_d/geometry.rs#L2047-L2069)
· [Material role check](../../src/three_d/geometry.rs#L381-L403)
· [Snapshot leases](../../src/three_d/snapshot.rs#L219-L238)

### 6.4 Shared derived frame and original source

Both classes shall independently prove positive exact original face area,
nonzero original UV determinant and nonzero in-plane projected tangent.
Use the ordered original local binary32 POSITION and UV0 under R024 §18.3
to derive F from the oriented face cross, U from the normalized in-plane
projection of dPdu and H from the sign of the exact UV determinant.
Verify bitangent orientation, the actual packed direction enclosures and
finite norm bounds, bit-identical Fp and Up across all three render corners,
exact packed tangent.w = H and every one of the nine outward cross
projections at least `1/16` against the checked unit separator. This proof
is independent of the existing Mode C eligibility witness.

Retain the complete raw normal and callback streams, every original
triangle, original winding, positions, indices, UVs and material lineage.
Require one certificate per original triangle, one corresponding ordered
render element per original element and exact original-to-render mapping.
Seam expansion may duplicate render vertices with proven source
correspondence. Refuse any epsilon substitute or removal of a present normal
role. Keep original source revision and raw bits distinct from derived
interpretation, certificate generation and checked render bits. The
existing successor verifier shall compare those associations and exact
bytes and refuse stale generations or changes behind an unchanged
generation. A changed interpretation advances the affected structural and
complete source associations and requires current Direct and radiance
qualification.
[Complete counts](../../src/three_d/geometry.rs#L1070-L1096)
· [Corner order](../../src/three_d/geometry.rs#L1809-L1837)
· [Frame derivation](../../src/three_d/geometry.rs#L1421)
· [Packed proof](../../src/three_d/geometry.rs#L2008-L2046)
· [Successor check](../../src/three_d/geometry.rs#L1681-L1740)

### 6.5 Bounded preparation, successor and publication

The current fixed Arc inventory, fixed certificate-size and fixed work-item
accounting do not cover new out-of-line source, rational, KKT and separation
proofs. Under §§2.1 and 3 the additive ingress shall deterministically
preflight actual immutable input counts, allocation identities, bytes and
work with checked arithmetic. Charge every original, derived, frame and
certificate proof allocation, headers, conversion copies and temporary
exact-arithmetic scratch. Deduplicate only actual shared allocations. Bound encoded operands,
intermediate sizes and operation counts per proof and for the full input.
Before allocation, check the peak of simultaneous live input, retained data,
copies and scratch against the caller's existing limit. Preserve checked
aggregate accounting across earlier retained preparations and the normal
proof remainder. Preflight and the sealed success receipt shall agree on
the same immutable input and aliases. No receipt grants Context capacity.
Overflow, insufficient budget or an unbounded proof item shall produce a
typed refusal before the affected work. The existing worker shall check
cancellation between bounded items, including successor comparison.

The same HAL380 handoff shall keep CPU-only prepared data, Send+Sync job and
one-use result, and UI-only !Send Pending. UI begin and finish shall remain
O(1) in source and proof size, using checked scalar and fixed allocation
metadata, without proof traversal. Context shall reserve actual retained
and successor-work charges and predecessor escrow before cloning worker
referents. Account for simultaneous predecessor, current, candidate,
in-flight and retiring data without releasing charges before the actual
last use. Verify the exact previous source and generation on the worker.
Finish shall recheck the one-use ticket, live Context, Device, predecessor
and cancellation before registration. No Context, Device, Queue or
controlled GPU lease crosses into worker proof.

For `NotRequiredNoNormalRole`, seal the origin and absence evidence with
the exact original source identity and revision, interpretation and
certificate generations and complete ordered original triangle and corner
roster. Verify origin transitions and absence evidence independently in
the existing successor check, including when differing origins skip the
legacy tangent-bit comparison. Refuse stale or mismatched evidence and
same-generation changes to origin or proof. Forbid old-front relabeling,
source substitution and automatic promotion from a legacy DefaultMikk
label with an empty stream. Charge evidence, copies and bounded worker
verification in the additive preflight and fixed-cardinality Context
inventory. Preserve exact legacy normal-only input, conversion and charge
semantics. The absence-origin checks add no exact-rational Z/N work.
[Current successor gate](../../src/three_d/geometry.rs#L1681-L1739)
· [Legacy conversion](../../src/three_d/material_api.rs#L473-L507)

Retain the full R025 original face and caster sequence, R028's entire
original normal-domain proof and `1/510` bound, and current R027 radiance,
FP16 Image, front and Queue requirements. The existing PBR52 and D-only
52+12 layouts, renderer, target and Device feature set remain unchanged.
No new shading flag is authorized. Failure or cancellation yields a typed
refusal and no partial registration or publication. A compatible last-good
source/front retains its original provenance. Withdraw incompatible
fronts and retire submitted work at the existing Queue-safe boundary.
[Preparation cost](../../src/three_d/geometry_cpu.rs#L126-L211)
· [Allocation inventory](../../src/three_d/geometry.rs#L770-L800)
· [Certificate limits](../../src/three_d/geometry.rs#L1098-L1144)
· [HAL380 handoff](../../src/three_d/geometry_cpu.rs#L313-L442)
· [Context escrow](../../src/three_d/context.rs#L1951-L1989)
· [R025 face stream](../../src/three_d/direct_shadow.rs#L1817-L1854)
· [R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md#6-normal-chain-correlated-domain-certificate-and-quality-limits)
· [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md)

### 6.6 Decisions before adoption and separate proof gates

1. **Qualified producer and completed capture.** Fix the exact accepted
   evidence for the qualified producer and controlled pack transaction,
   genuine authored TANGENT absence, successful pinned execution and final
   signed coverage, exact source/corner correspondence and revision
   binding. Specify reader structural checks and independent HAL
   validation at their respective owners. This is the producer evidence
   gate referenced by §7. Historical-run verification after a hostile
   full-HAC rewrite is outside the selected boundary and implies no
   signature or attestation service.
2. **Peak proof budget.** Fix finite count, operand, intermediate and work
   bounds, exact storage and scratch accounting, and the allocation
   metadata and receipt formula that permit checked producer preflight and
   O(1) Context escrow. No limits or proof representation are selected here.

Independent S1 review and express adoption must coordinate R024
§§7.1.1–7.1.2/§18, HAL370 §4.1/§14 and this HAL380 clarification of §§2 and
2.1. The producer must separately qualify its provenance and adopt
the closed prepared-source recipe and wire format for this source. The
current encoder and prepared formats have their existing scope. The exact
rational cost and implementation decisions under §7 remain proposed.
The canonical R41 digest preimages, R11/R3/R34 symbol and generation
bindings, wire adoption and actual producer qualification remain separate
producer S1 gates. Separate HAL and producer S2 units require
their respective S1 gates. Actual original-source and CPU checks,
N1/N4 GPU and native execution, independent S3 review and JC physical
appearance acceptance remain distinct gates. Source and prepared Inspector
receipts must disclose derived shading and possible faceting.

**KOMPLEMENT 902.779.61 remains NOT ADMITTED.** The reported partition is
1,492 A/B, 3 Z and 5 N candidates across 1,500 original triangles and
4,500 indexed corner occurrences. This is supplied evidence, not
independently reproduced. This appendix records no source math, test,
GPU or native acceptance.

## 7. Fixed exact-rational verification and CPU peak-cost proposal for Z/N

**Proposed / Inactive / Non-binding.** This candidate supplies
analytical bounds for §6, [R024 §18](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#18-proposed-s1--two-source-qualified-geometric-rendering-classes)
and [HAL370 §14](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#14-proposed-s1--coordinated-admission-of-two-derived-source-frames).
Sections 1–5 retain their binding scope and §6 retains its proposed status.
This appendix supplies no admission, ABI selection or S2 grant. JC retains
constitutional and product authority and final veto.

### 7.1 Pre-Flight

- **Objective** — Verify the finite arithmetic and peak-cost basis of the selected Z/N candidate for independent review.
- **Standing objective** — Preserve faithful original source, complete proof and responsive Context admission for supported assets.
- **Governing authority and status** — HAL002/002A, HAL355, adopted HAL380 §§1–5, R024, HAL370, R025, R027 and R028 govern. HAL380 §6, R024 §18 and HAL370 §14 are inactive.
- **Authority change, if any** — None. This is a candidate for the later coordinated cost and verification decision.
- **Target layer and existing state verified** — HAL CPU source, cost, proof and Context custody at `1abc9ea7c07fa5bb3ce28a5c495fc5e47d17f381`.
- **Owned mechanisms and files** — This appendix in SPEC-HAL-380 only.
- **Named invariants** — INV-380-SINGLE-VERIFIER, INV-380-EXACT-SOURCE, INV-380-CPU-ONLY, INV-380-ONE-USE, INV-380-LIVE-CONTEXT, INV-380-FINITE-ESCROW, INV-380-SUCCESSOR, INV-380-OLD-API, INV-380-UI-BOUNDED and INV-380-LAST-GOOD.
- **Forbidden behavior** — Rounded or incomplete proof, unverified counts, hidden allocation or work, synthetic source association, fallback shading and partial publication.
- **Existing mechanisms inspected and reuse decision** — Keep the existing HAL checker, worker, sealed handoff and Context owner. Extend only the proposed proof accounting.
- **Relevant external or dependent boundaries** — The proposed R43 record uses the 512-byte operand limit stated in §7.1. Producer authentication and prepared-wire admission are separate owner decisions.
- **Test ownership and placement** — N/A for this document unit. Existing geometry and admission owners retain later proof responsibility.
- **Output or artifact completeness boundary** — One inactive appendix with checked bit, operation and arena bounds and explicit unresolved admission and cost choices.
- **SemVer and changelog impact** — N/A. Documentary proposal only.
- **Out of scope** — Binding adoption, S2, public API or ABI selection, Rust, Cargo, tests, GPU, native, source generation and producer or shader changes.

### 7.2 Encoded operands and exact B envelope

Require nine canonical nonnegative reduced rational weights `w_i=n_i/d_i`
with a positive denominator and one record for every ordered cross pair,
including zero weights and off-diagonal pairs. Each numerator and denominator
has at most 512 bytes (4,096 bits) in canonical unsigned little-endian
encoding. The reduced zero is `0/1`. Validate lengths, canonicality, counts
and association before dependent work. This is the selected candidate's operand limit; it grants no producer wire-format authority.

For each finite packed binary32 component, write the exact value as
`X·2^-149`. A subnormal lifts its fraction bits directly. A normal lifts
its 24-bit significand by the raw exponent minus one, at most 253 places.
The sign is separate and `|X| < 2^277`, including the largest finite
component. Each of the 27 cross coordinates is `C_ik·2^-298`, where
`C_ik` is the difference of two signed integer products. Each product is
less than `2^554` in magnitude, hence `|C_ik| < 2^555`. These bounds use
the full finite binary32 domain.

Use the selected product-denominator order. With `L=4096`, form
`D=∏d_i`, exact `E_i=D/d_i`, and `A_i=n_i E_i`. Check zero division
remainders and prove `ΣA_i=D` before forming `Q_k=Σ_i A_i C_ik`.
The largest bit counts of D, E_i and A_i are `9L`, `8L` and `9L`.
Before the equality check the sum of A_i is less than `9·2^(9L)`, which
takes at most 36,868 bits. After equality, nonnegative A_i bound every
signed partial sum of Q by `D·max|C_ik|`. The product also bounds the
LCM. The following bounds count magnitude bits.

| Quantity | Derivation | Bits at most |
|---|---|---:|
| D and A_i | `9L` | 36,864 |
| Q and its denominator | `q=Q/(D·2^298)`, `9L+555` and `9L+298` | 37,419 and 37,162 |
| S | `S=Σ_k Q_k²`, three squares bounded by `2^(2·37419+2)` | 74,840 |
| T_i | `T_i=Σ_k Q_k C_ik`, three products bounded by `2^(37419+555+2)` | 37,976 |
| KKT sides and optional difference | `D T_i` and S, with one further bit if subtracting | 74,840 and 74,841 |
| Unreduced d² denominator | `Δ=D²·2^596` | 74,324 |
| Strict threshold sides | `S< D²·2^588`, equivalent to `256S<Δ` | 74,316 for the shifted right side of the first form, 74,848 for `256S` |

For every ordered pair, `q·(c_i−q)=(D T_i−S)/Δ`. Compare `D T_i≥S`
exactly, with equality for every `A_i>0`. A single ordered comparison
suffices for the inequality and equality. This is the full KKT check, with
no subset search, rounded comparison or elided pair. The strict threshold
and equality with the stored reduced exact `d²` are distinct obligations.
[Source exclusion rule](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#184-proposed-nonzero-canonical-mikk-class-outside-abc)

### 7.3 Verified work ceilings and stored-distance reduction

For the common-denominator calculation, use magnitude lengths rounded up
to 64-bit limbs. An unsigned 64×64 product plus old digit and carry fits
128 bits because `(2^64−1)^2+2(2^64−1)=2^128−1`. A 1,170-limb magnitude
holds 74,880 bits. Store sign separately and reserve a guard limb in the
wide working slots. These bounds include the full digit product width
for each audited multiply.

| Product stage | Calls | Schoolbook MAC ceiling |
|---|---:|---:|
| 27 cross coordinates, two products each | 54 | `54·5²` |
| D, growing from one denominator to nine | 8 | `(1+…+8)·64²` |
| A_i | 9 | `9·64·512` |
| Q_k | 27 | `27·576·9` |
| S | 3 | `3·585²` |
| D² | 1 | `576²` |
| T_i | 27 | `27·585·9` |
| D T_i | 9 | `9·576·594` |
| Total | 138 | 5,163,588 |

The selected comparison order has at most
`27+9+27+3+27=93` top-level add/subtract calls for cross differences,
the weight sum, Q, S and T. This counts accumulations starting from
zero. Forming nine KKT differences instead would add nine subtractions.
Also price 11 result comparisons for the sum, threshold and KKT, the
dyadic denominator and threshold shifts, sign and length work, and
decoding and canonicality scans of 18 bounded integer fields. Each
product of a and b limbs has at most a+b initialization writes, a carry
write per row and an a+b scan of the result, besides the a·b MAC steps.
Price bounded copy, shift, add/subtract and compare passes over their
actual capped slots. The MAC total is only the multiplication term.

The nine exact E_i quotients need at most `9·36,864=331,776` restoring
division bit rounds. Each round shifts, compares and conditionally
subtracts using a partial remainder of at most 4,097 bits (65 limbs),
and writes one quotient bit. Price initialization, scans and all nine
zero-remainder checks. For each reduced weight, handle zero as 0/1
and reject two even operands. For two positive operands, a binary GCD
round halves an even operand or replaces the larger odd operand by
half its positive difference. The sum of their bit lengths strictly
decreases. This gives at most `9·2·4096=73,728` rounds on at most
64-limb values, plus the checks that the terminal gcd is one. Each
round uses bounded parity, compare, subtract and shift operations.

**Stored d² equality adds work.** The previous division and GCD counts
price the nine weights only. They cannot price reduction of `S/Δ`.
Before comparing the canonical numerator and denominator with the stored
reduced pair, S=0 yields 0/1. For positive S, reduction requires
`g=gcd(S,Δ)` and the exact quotients `S/g` and `Δ/g`. As a sizing
witness, a binary reduction with common powers of two extracted and
restored takes at most `74,840+74,324=149,164` additional rounds. Each
halving or half-difference decreases the sum of bit lengths. Price up to
two shifts and a checked common-power increment, or compare, subtract
and shift per wide round, plus the final bounded restoration shift.
Two restoring quotient divisions add at most 149,164 bit rounds in total,
with a partial remainder of at most 74,325 bits (1,162 limbs). Each round
uses the same bounded shift, compare and conditional subtract. Price the
two zero remainders and the canonical lengths and byte comparisons of
both quotients. Their matching byte spans are at most 9,355 and 9,291
bytes, respectively. Comparing canonical computed bytes directly requires
no second gcd of a matching stored pair.

For this analytical reduction schedule the combined ceilings are
**222,892 GCD rounds and 480,940 division bit rounds**, with the additional
rounds on wide operands. The multiplication total above is unchanged.
The computed reduced operands are bounded by the respective 74,840-bit
and 74,324-bit unreduced components. A stored pair must have an explicit
encoded-length cap and allocation charge. These mathematical ceilings do
not select that cap or show that a particular prepared receipt fits it.
A different reduction, cross multiplication by stored operands or hidden
library temporaries would require its own width, work and scratch proof.

### 7.4 Arena lifetime and bounded class work

One active B proof has a constructive magnitude payload of
`8·(18·64+18·5+27·9+10·576+3·585+2·1170+4·1171)=128,192 bytes`.
The terms cover decoded weight magnitudes, lifted components, cross
coordinates, D and nine A_i, Q, S and Δ, and four guarded working slots.
The following lifetime check applies to a single active record on the
existing worker, with bounded in-place word passes and no hidden copies.

- Weight gcd uses two working slots. Cross generation uses two product
  slots. Denominator construction uses one product slot. Each E_i
  calculation uses a quotient and a remainder while a third slot holds
  the pre-equality weight sum. Store A_i in its named slot.
- Q and S accumulate from one product at a time. During KKT, keep T_i,
  its dot-product temporary and the D T_i result in three slots, with a
  fourth available for a checked signed difference. S and Δ stay in
  their wide slots and the threshold uses a working slot.
- For stored d², keep S and Δ, use two mutable gcd copies, and restore the
  gcd in a working slot. Keep that gcd while dividing S and Δ in turn
  using one quotient and one remainder slot. Compare each quotient with
  its immutable encoded operand before reusing the slots. This bound
  assumes direct length and byte comparison with that input, or a copy
  within the remaining guarded slot after checking lengths. It assumes
  no additional full-width temporaries and no overlapping B evaluations.

This proves the magnitude payload for the stated lifetime witness. Sign,
length, carry, proof state, header, alignment and backing allocation
capacity are separate. The integer decode slots cover the magnitude
copies shown above. Typed decoded records, interval enclosures, immutable
proof and source arrays and any additional conversion copies require
their own charges. This is neither a complete peak receipt nor an actual
stack, heap or timing measurement.

Keep the legacy A/B/C and genuine-absence checks under their existing
bounded source loops. A legacy B certificate keeps its nine packed
pair checks. Z verifies the three original corner associations, the
authorized exact-zero records and the common F/U/H and packing checks.
N adds the strict original A exclusion, testing at most six normal and
tangent cone dots for an upper bound below 3/4, and C exclusion using
two domains by two signs, four separators with six positive bounds each.
Each of these 24 C dots is three interval products and two additions.
Price the finite binary64 witness and outward-enclosure work separately
from exact B. Both Z and N verify the common F/U/H derivation, the six
packed directions and all nine packed cross lower bounds at least 1/16.
Current binary64 enclosures and fixed dyadic expansions have bounded
operations. Inspection of the manifest, lock and checker identifies no
existing exact multi-limb rational facility for this proof.
[Intervals](../../src/three_d/geometry.rs#L803-L951)
· [Fixed determinant](../../src/three_d/geometry.rs#L1147-L1198)
· [Frame derivation](../../src/three_d/geometry.rs#L1421-L1544)
· [Packed pairs](../../src/three_d/geometry.rs#L2008-L2046)
· [Manifest](../../Cargo.toml)
· [Lock](../../Cargo.lock)

For one complete primitive, require `E_o=E_r=3t` and a mechanically
checked roster with `t_legacy+t_Z+t_N=t`. Check a versioned total of
`W_scan+t_legacy·W_legacy+t_Z·W_Z+t_N·(W_frame+W_A+W_C+W_B)+W_bind+W_successor`.
W_scan covers the full source scans, W_Z covers zero association and its
common packed frame proof, and W_B includes the wide distance reduction
and all non-MAC terms in §7.3. W_bind and W_successor cover bounded source
association and predecessor comparisons. Check every count, sum and
product under the selected source and request work ceilings. The existing
`V_o+E_o+V_r+E_r+t` count prices structural work items and cannot bound
N by itself. Price class-specific work only from the checked type and
complete association. An unverified low N count cannot reduce an initial
pre-allocation bound. Fix how the additive cost boundary obtains that
checked count without a UI source scan. Refuse malformed counts, lengths,
unbounded intermediate work or insufficient budget before new dependent
allocation or proof work. Check cancellation between records and within
long arithmetic loops at fixed bounded iteration intervals. The maximum
uninterrupted work and exact coefficients are S1 decisions.
[Count and certificate limits](../../src/three_d/geometry.rs#L1070-L1144)
· [CPU checkpoints](../../src/three_d/geometry_cpu.rs#L234-L270)
· [Successor verification](../../src/three_d/geometry_cpu.rs#L329-L343)

### 7.5 Producer preflight and Context escrow

The current preflight enumerates seven public Arc slots and sizes fixed
records, copies and checked frames. The sealed source enumerates nine
CPU allocation slots. Neither price includes the proposed rational and
interval proof storage. The additive path must have a physically
fixed-cardinality allocation-identity inventory for its immutable inputs,
retained proof backing and generated arrays, and a bounded temporary
work allocation schedule. A declared limit on a variable collection of
per-triangle Arcs does not give fixed-cardinality escrow. Count actual
capacities, headers, padding, alignment, and proven allocation aliases
including old and candidate sharing. Mechanically check proof counts and
spans against the actual backing. Preserve immutable identities and
checked scalar counts in the sealed value. UI begin and finish stay O(1)
in source size, with no traversal of the proof records.
[Existing input and peak accounting](../../src/three_d/geometry_cpu.rs#L126-L211)
· [Existing sealed inventory](../../src/three_d/geometry.rs#L770-L800)

For primitive j, let `I_j` be the charge of the unique caller-owned
immutable allocations. Include the full source and render inputs, typed
roster and certificate identities, callback and source association,
stored rational weights and d², and required frame and C proof data.
Let `A_j` be the additional retained charge from generated source,
converted certificate and checked frame arrays, any retained proof copies
and fixed lifecycle charges. An input allocation moved into the sealed
source remains in I_j. Let `P_j` be the fixed lifecycle charge plus the
maximum over preparation phases of all simultaneously live generated
blocks, decode/conversion copies and scratch. Include the full retained
phase in the maximum so `A_j≤P_j`. Charge producer-owned scratch in the
caller baseline while live. Account for actual simultaneous phases and
deduplicate only proven shared allocations.

Under §§2.1 and 3, check `I_j+P_j≤M` and
`B+R+P_j≤M` with checked arithmetic. B is the already charged live caller
baseline including the applicable immutable input allocations, and R is
earlier additional retained charge. Require the preflight and success
receipts to use the same immutable input and proven alias account.
On successful agreement add only A_j to R. Failure or cancellation
creates no success debit. The subsequent normal proof gets the checked
remainder `M−B−R`.

Context escrow is independent of that receipt. Price sealed reachable
allocations including moved inputs, custody overhead and any successor
scratch before retaining worker references. Deduplicate against the
eligible predecessor only where actual allocation identity proves prior
charge. Include all live old, current, candidate, pending and in-flight
ownership and registration overlap. Hold charges through the existing
ticket and Queue retirement boundaries. Preserve the same worker and
registrar, `PbrGeometryDescriptor3D`, `PbrFrameCertificate3D` and the
existing synchronous and CPU entry semantics.
Keep true normal absence, R025 original caster order, R028's complete
normal domain and 1/510 floor, and R027's current front and Queue law.
Typed refusal keeps only the compatible last-good with its original
provenance, withdraws incompatible fronts and grants no shadow fallback
or partial publication.
[Begin, verify and finish](../../src/three_d/geometry_cpu.rs#L313-L442)
· [Context escrow](../../src/three_d/context.rs#L1951-L1989)
· [Registration charge](../../src/three_d/pbr.rs#L849-L876)
· [R025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md)
· [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md)
· [R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md)

### 7.6 Unclosed S1 decisions and source status

Before binding S1, select the exact versioned checker schedule and
representation, including stored-d² reduction and operand framing and
limits. Fix the complete source and request count, byte and work ceilings,
cancellation granularity, and the C witness and trusted outward-bound
format and verification. Select the fixed allocation topology and exact
peak and escrow formulas with the actual layout, overhead and lifetimes.
The arena and loop bounds above demonstrate a conditional accounting
basis. They do not select a library, allocate resources or certify a
physical performance limit. Producer callback authentication remains
separate and open under §6.6.

The corrected reported triangle 258 `d²` decimal is
`0.00065754309713464`. The decimal is evidence reporting, not the stored
exact rational comparison. The complete five rational receipts and their
fit to the proposed 4,096-bit limit have not been independently established
in the Halcyon workspace. The five candidate triangles and **KOMPLEMENT
902.779.61 remain NOT ADMITTED**. This unit checks mathematical bounds
and documentation only. All coordinated adoption, producer and prepared
wire, S2, independent proof and JC physical appearance gates remain
separate. [Reported source evidence](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#185-invariants-refusal-and-future-proof)

## 8. Exact Z/N verifier and CPU escrow contract candidate

**Proposed / Inactive / Non-binding.** This candidate specifies a candidate for the additive input in §6 using the analytical
bounds in §7. Coordinate it with [R024 §18](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#18-proposed-s1--two-source-qualified-geometric-rendering-classes)
and [HAL370 §14](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#14-proposed-s1--coordinated-admission-of-two-derived-source-frames).
Sections 1–5 retain their binding scope. Sections 6–8 remain inactive.
JC retains product authority and final veto. This candidate grants no
admission, binding adoption or S2 authority.

### 8.1 Pre-Flight

- **Objective** — Specify the selected bounded exact verifier and its complete CPU work and custody boundary for independent review.
- **Standing objective** — Preserve exact original source and responsive complete admission for supported PBR assets.
- **Governing authority and status** — HAL002/002A, HAL355, adopted HAL380 §§1–5, R024, HAL370, R025, R027 and R028 govern. The proposed source and CPU extensions remain inactive.
- **Authority change, if any** — None. This is a candidate for later independent adoption.
- **Target layer and existing state verified** — HAL geometry checker, CPU preflight and Context escrow at `49152af2fd1ca433327d371f4227c5d1da50b325`.
- **Owned mechanisms and files** — Append §8 to SPEC-HAL-380 only.
- **Named invariants** — INV-380-SINGLE-VERIFIER, INV-380-EXACT-SOURCE, INV-380-CPU-ONLY, INV-380-ONE-USE, INV-380-LIVE-CONTEXT, INV-380-FINITE-ESCROW, INV-380-SUCCESSOR, INV-380-OLD-API, INV-380-UI-BOUNDED and INV-380-LAST-GOOD.
- **Forbidden behavior** — Fake legacy proof, rounded exact predicates, uncharged work or storage, stale source relabel, alternate registrar, feature gate, source-name whitelist and partial publication.
- **Existing mechanisms inspected and reuse decision** — Reuse the geometry checker, sealed worker and Context registrar. The proposed exact arithmetic is private to that owner.
- **Relevant external or dependent boundaries** — R41 preimages and producer schema2 remain inactive drafts. The source producer retains provenance and prepared-wire ownership.
- **Test ownership and placement** — N/A for this document unit. Existing geometry and admission owners retain future conformance responsibility.
- **Output or artifact completeness boundary** — One inactive candidate with explicit arithmetic, work and allocation conditions and the remaining decisions before adoption.
- **SemVer and changelog impact** — N/A. Documentary proposal only.
- **Out of scope** — Binding adoption, S2, public API or ABI selection, Rust, Cargo, tests, source or asset generation, producer, manifest, wire, shader, GPU and native work.

### 8.2 Complete additive source and fixed proof backing

The additive CPU input covers one immutable original primitive. Its closed
ordered per-triangle evidence sum is Legacy A/B/C/NormalOnly, Z or N. One
certificate and generation binds every original triangle to its source,
interpretation and exact ordered original/render corner correspondence.
The separately qualified private `NotRequiredNoNormalRole` origin is
primitive-wide and retains the normal-only route in §6.3.

Use one flat immutable `Arc<[u8]>` backing all variable-sized Z/N receipts
and exactly one flat immutable Arc of fixed-size per-triangle association
and offset records. Each record binds its class, source, certificate and
generation and the checked offset and length of applicable proof bytes.
Check complete roster length, exact ordered association, checked offset
plus length, actual backing bounds and class-specific lengths before
decoding or allocating dependent data. The roster is the sole per-triangle
association allocation for the additive input. Variable per-triangle Arcs,
a pointer graph, parallel legacy stand-ins and hidden overrides are
forbidden. Keep the ordinary original vertex, index, raw tangent, render,
corner map and checked-frame allocation owners.

The additive route enters the same underlying HAL mathematical checker
and returns the same sealed HAL-owned worker result to the sole Context
registrar. Retain the source bytes, qualified origin, proof identity and
successor generations. Existing `PbrGeometryDescriptor3D`,
`PbrFrameCertificate3D`, `PbrTangentProvenance3D`, synchronous
`Context3D::admit_pbr_geometry` and legacy HAL380 prepare, cost, begin,
verify and finish signatures and behavior remain exact. The proposal
selects no public name or record ABI. No generic bigint service, external
arithmetic crate, second verifier, resource owner, hidden global or
proof-based feature switch is selected.
[Legacy conversion](../../src/three_d/material_api.rs#L473-L543)
· [Shared verification entry](../../src/three_d/geometry.rs#L1558-L1573)
· [Context registration](../../src/three_d/pbr.rs#L784-L790)

### 8.3 Exact class checks and private arithmetic

- **Legacy.** Apply the existing selected A/B/C or genuine-absence proof.
- **Z.** Bind each exact-zero xyz and its original signed w to its checked
  original corner before both raw and smooth-candidate normalization
  gates. Keep all raw bits and the existing checks on other corners.
- **N.** Retain the strict original A upper bound below `3/4`, exact B
  exclusion below and strict homogeneous C separation in both original
  and actual packed smooth domains for both tangent signs. Each of the
  four C separators must check its six positive bounds. Bind actual
  original and packed bits before evaluating these proofs. Require all
  three exclusions before accepting the derived frame.
- **Genuine absence.** Use the existing normal cone or checked three
  packed normal projections at least `1/16`. The no-tangent origin
  requires independently qualified original TANGENT absence, no
  generated capture and an empty original signed stream. HAL checks
  genuine normal-role absence against the actual retained material
  lease. Preserve the unused finite packed tangent/sign field.

Both Z and N keep the original source F/U/H derivation and all nine
packed cross lower bounds at least `1/16`. Outward C and common frame
proofs retain the bounded HAL `FrameInterval` and fixed dyadic arithmetic.
Refuse unresolved enclosures. Preserve R025 original faces and casters,
R028's full applicable normal domain and `1/510` floor, and R027 radiance
and current front law. Shader, renderer, target and ABI remain unchanged.
[Original checks](../../src/three_d/geometry.rs#L1742-L1859)
· [Bounded intervals](../../src/three_d/geometry.rs#L803-L951)
· [Fixed determinant](../../src/three_d/geometry.rs#L1147-L1198)
· [Derived frame](../../src/three_d/geometry.rs#L1421-L1544)
· [Normal and packed pair checks](../../src/three_d/geometry.rs#L1993-L2069)
· [Leased material association](../../src/three_d/snapshot.rs#L219-L238)
· [R025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md)
· [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md)
· [R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md)

Use private checked sign and magnitude with little-endian u64 limbs.
Cap magnitudes at 1,170 limbs (74,880 bits) and provide one guard limb
in the working slots. Check lengths, sign, shifts, carries and output
width. Refuse an out-of-bound result with no truncation or unchecked
allocation. Only one N Mode-B proof may be active on the existing worker.

Require nine canonical reduced nonnegative R43 weights and a stored
canonical reduced nonnegative d² pair. Denominators are positive. Each
weight numerator and denominator is at most 512 bytes. Check each
encoded length and exact canonical form, including zero as `0/1`, and
bind all nine weights to the ordered packed binary32 cross pairs.
The stored pair has separate checked length and byte charges. §7's
computed quotient bounds are 9,355 and 9,291 bytes, respectively, and
do not select its prepared-wire limit.

Follow §7's product-denominator schedule. Form `D=∏d_i`, compute
`E_i=D/d_i` by exact division, form `A_i=n_i E_i` and require
`ΣA_i=D` before Q. Lift every finite packed binary32 component as
`X·2^-149`, including subnormal and exponent extremes, and compute the
27 cross numerators C at scale `2^-298` exactly. Form
`Q_k=Σ_i A_i C_ik`, `S=Σ_k Q_k²`, and `T_i=Σ_k Q_k C_ik`.
Check `D T_i≥S` for all nine pairs and equality at every positive
A_i. Check `S<D²·2^588`. Reduce `S/(D²·2^596)` with a wide GCD and two
exact restoring divisions, or canonical zero, and compare the computed
numerator and denominator to the stored canonical pair. That equality
establishes stored reducedness without a second GCD. Require zero
remainders on exact divisions. No rounded d², support search,
source-name rule or cross multiplication by stored operands.

### 8.4 Work and cancellation bounds

The selected order retains §7's 74,848-bit envelope, 138 long multiply
calls, 5,163,588 schoolbook MACs and 93 top-level add/subtract calls.
The complete count is at most 222,892 binary GCD rounds and 480,940
restoring-division bit rounds including stored d². The 128,192-byte
magnitude arena is one active proof's payload using §7.4's exact
lifetimes and four guarded working slots. It includes the named
decoded integer magnitudes. Price sign, length, carry and cursor state,
headers, alignment, proof backing, interval enclosures and all other
live arrays separately. Reuse these slots across N triangles on the
same worker.

Use a versioned checked work charge for B. For multiply operands of
a and b limbs, charge a·b MACs, up to a+b initialization writes, a
carry write per row, the a+b result scan and any bounded copies.
Price up to 93 signed add/subtract calls, including their sign
comparisons, carry or borrow, writes and scans at capped widths.
Charge comparisons and shifts for sum, KKT and threshold, and bounded
decoding, length and canonical checks of the weight and distance
fields. The division charge includes at most 331,776
narrow rounds with 65-limb remainder and 149,164 wide rounds with
1,162-limb remainder. Price each shift, comparison and conditional
subtract pass, quotient-bit write, zeroing, remainder check and
canonical quotient comparison. The GCD charge includes at most 73,728
rounds on 64-limb operands and 149,164 wide rounds. Price parity,
comparison, subtraction and shift passes, the common-factor counter
and restoration shift and the terminal checks.

In the checked formula below, each term is the sum over its actual
operations and capped limb or byte spans, multiplied by the fixed
coefficient for that primitive in the selected verifier revision.

```text
W_B = W_decode + W_mul + W_addsub + W_compare_shift
      + W_division + W_gcd + W_quotient_bytes
```

For example the multiplication term sums the priced MAC, initialization,
carry, scan and copy terms above for each product. The division and GCD
terms sum the per-round passes and scalar work with the appropriate
narrow and wide caps. Price proved upper bounds from checked lengths
and the selected widths before execution. Actual operand widths must
stay within those bounds. Keep the same preflight charge on success.
Use checked addition and multiplication throughout. Exact primitive
coefficients and fixed-size metadata work require an adopted bound.
The arithmetic loop counts alone cannot serve as a CPU receipt.

At most **256 primitive limb operations** may occur between cancellation
checks within every long arithmetic loop. Split multiply rows and
division/GCD scans, shifts, copies and zeroing into counted chunks.
A wide division pass alone can visit 1,162 limbs. A per-row or per-round
check cannot satisfy the selected limit by itself. Retain carries,
indices, quotient and remainder state and other resumable cursors
across checks. Carry the count across adjacent long loops until an
actual check. Check between triangles and during bounded successor
comparisons as well. Price checks and cursor bookkeeping independently
of arithmetic. Check each complete 256-operation chunk. For L counted
limb operations and E mandatory boundary checks, this cadence costs
at most `floor(L/256)+E` polls, with no zero-progress polling. Fix the
primitive definition, exact placement of boundary checks and charges
before adoption.
The numerical counts and magnitude payload hold if checkpoints
introduce no arithmetic replay, extra magnitude arrays or opaque
uninterruptible bulk passes. A helper that cannot pause within the
selected limit lacks the required bound.

The additive source worker may scan the bounded roster and proof
offsets in O(t) before allocating dependent storage. Check outer
counts and byte bounds first. Derive class counts from the checked
one-to-one source roster with `E_o=E_r=3t` and
`t_L+t_Z+t_N=t`. Refuse malformed counts, lengths, overflow or excess
work before dependent allocation or proof. An unverified low N count
cannot discount preflight. For checked per-class maxima and source
bounds, charge

```text
W_N = W_frame + W_A + W_C + W_B
W_j = W_scan + t_L·W_L + t_Z·W_Z + t_N·W_N
      + W_binding + W_successor + W_cancellation
```

W_scan covers the complete original/render vertex, element and roster
scans. W_L covers the existing bounded legacy proof and W_Z includes
zero-corner qualification and its common frame proof. W_binding
includes checked source and proof byte association. W_successor
includes exact predecessor comparisons. W_cancellation prices the
counted checks and state. Use the exact checked partition for a
class-priced bound. The whole-source ceiling also bounds the class
term by `t_max·max(W_L,W_Z,W_N)`, with separately bounded source
counts, proof bytes, association, successor and cancellation terms.
Sum checked bounds across the full request. Numeric source and
request ceilings remain decisions for S1. The existing structural
item sum cannot substitute for this bound.
[Current structural work](../../src/three_d/geometry.rs#L1070-L1144)
· [Worker cancellation](../../src/three_d/geometry_cpu.rs#L234-L270)
· [Successor check](../../src/three_d/geometry_cpu.rs#L329-L343)

### 8.5 Fixed allocation inventory, receipt and Context escrow

The live legacy cost path enumerates seven public input Arc positions.
The sealed source enumerates nine. Use fixed named positions for those
allocations plus the two new flat backing roles. The candidate inventory
has nine caller positions and eleven sealed positions, including inactive
positions. The old input and private triangle-certificate-array positions
are inactive for the additive path, which uses its one roster. No legacy
ledger change follows. The positions and actual roles are as follows.

| Inventory | Existing named positions | Additive roles |
|---|---|---|
| Caller | Original vertices, original indices, raw signed tangents, render vertices, render indices, render-corner map, legacy triangle certificates | Proof bytes and the fixed-size roster |
| Sealed | Render vertices, render indices, certified wrapper, original vertices, original indices, raw signed tangents, render-corner map, legacy private triangle certificates, checked frames | Proof bytes and the fixed-size roster |

Each occupied position binds the actual immutable backing allocation
identity, capacity and chargeable allocation layout. An unindexed or
inactive position carries no allocation. The roster and proof backing
each use one physically bounded Arc. The worker seals the fixed
inventory and scalar counts with source, origin and certificate
revisions. Check headers, padding, alignment and actual allocation
capacity and distinguish moves, shared Arc clones and fresh allocations.
Deduplicate only proven backing identity and charge the real live
overlap. An allocator layout or capacity cannot be inferred from a
slice address and logical length alone. Select and verify the charged
layout, provenance and overhead before adoption. Preserve the existing
legacy size-based charges and refusal behavior.
[Legacy input cost](../../src/three_d/geometry_cpu.rs#L126-L211)
· [Legacy sealed inventory](../../src/three_d/geometry.rs#L770-L800)

For primitive j, let I_j be the charge of the unique immutable caller
input allocations. Let A_j be the new additional retained allocation
charge, including generated arrays and retained lifecycle data, and
excluding moved or shared input Arcs. Let P_j be exact lifecycle
overhead plus the maximum simultaneously live additional allocation,
copy and checked scratch charge over all preparation phases, including
the retained phase. Count lifecycle overhead once in each charge.
Check `A_j≤P_j`. Charge the B arena and other released scratch only to
the relevant peak phase. Price all typed
decode, frame, provenance, conversion and copy storage at its actual
lifetime. The caller baseline B already includes the relevant input
allocations and any caller scratch still live. R is the earlier
successful additional retained charge.

Independently enforce `I_j+P_j≤M` and `B+R+P_j≤M` with checked
arithmetic and no duplicate input charge. Preflight and success
receipts must agree for the exact immutable input and actual alias
account. Success adds only A_j to R. Failure adds no success debit.
The following normal certificate uses the checked remainder
`M−B−R`. Retain actual charges until last use on cancellation or
failure.

Context performs independent reservation and escrow using the sealed
fixed inventory, actual capacity and live predecessor. Charge sealed
reachable allocations including moved input, pending custody and any
bounded successor scratch before retaining worker references. Deduplicate
against a predecessor only by proven allocation identity and prior charge.
Keep old, current, candidate, pending, in-flight and retiring ownership
and registration overlap charged through existing last-use and Queue
boundaries. UI begin and finish are O(1) in source size using sealed
scalars and fixed inventory checks. The same worker checks exact
successor source and origin/certificate revisions and bytes. The
sealed one-use result and existing Context registrar retain live
Device, generation, lease and cancellation checks. A refusal leaves
the compatible last-good front truthful, withdraws incompatible fronts
under R027 and supplies no partial registration or publication.
[Current begin and finish](../../src/three_d/geometry_cpu.rs#L347-L442)
· [Current successor kernel](../../src/three_d/geometry.rs#L1681-L1740)
· [Context escrow](../../src/three_d/context.rs#L1951-L1989)
· [Registration metadata](../../src/three_d/pbr.rs#L849-L876)

### 8.6 Evidence boundary and remaining decisions

Use the qualified producer's controlled pack transaction in §6.3 for
actual capture provenance. The prepared reader verifies structural
source and stream association. HAL independently checks the complete
mathematical and material proofs. The selected boundary adds no
signature or historical attestation infrastructure. The candidate's canonical R41 preimages are producer-format design inputs. They
carry no binding authority or qualified producer receipt.
[Capture authority](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#182-proposed-zero-callback-source-class)
· [Prepared gates](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#142-prepared-source-and-future-gates)

Before coordinated S1 adoption, fix the exact private verifier revision,
representation and source/count/byte ceilings, the flat proof and roster
fields and encoding, bit-operation coefficients and cancellation schedule,
C witness format and revision, and the full peak phase formulas and
physical allocation overhead. The 256-operation spacing is selected for
review. Exact primitive accounting, resumable state and the resulting
cost must meet it. Qualified producer and capture evidence and adoption of the
producer's prepared schema2 remain separately required. No actual fit of
the five rational receipts or GPU/native proof follows. **KOMPLEMENT
902.779.61 remains NOT ADMITTED.**

### 8.7 Pre-prepare source limits and charged layouts

**Proposed / Inactive / Non-binding.** This candidate supplies the additive limits source and accounting metric for
§§8.4–8.6. Legacy descriptors, frame certificates, entry points, receipts
and refusal semantics retain their binding scope.

**Preflight limits snapshot.** Before dispatch, the caller reads the
owning `Context3D::limits()` and passes only an immutable checked copy of
its effective `max_vertices_per_mesh` and `max_indices_per_mesh` (u32),
`max_mesh_bytes` (u64) and Device `max_buffer_size` (u64) to the existing
CPU worker. Use the actual configured values without substituting a
preset. The worker receives no live Context, resource handle, Device,
Queue or registry authority. The snapshot is a preflight ceiling. The
existing begin/finish owner rechecks applicable limits against the sealed
scalar counts and real current Context, Device and generation before
registration. A candidate exceeding a stricter live cap refuses.
[HAL355 budget authority](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#13-finite-budgets-and-device-limits)
· [Effective limits](../../src/three_d/context.rs#L1838-L1840)
· [Current render admission checks](../../src/three_d/pbr.rs#L792-L848)

**Outer source gate.** Before large roster scans or dependent
allocations, derive checked outer counts from actual immutable backing
lengths. Require nonzero original and render vertex counts, `t>0`,
`E_o=E_r=3t`, a full t-record roster, complete source-index/corner
stream lengths and one original-source mapping entry per render vertex.
Check the applicable raw tangent count under §6. An unindexed stream
uses its vertex count as its element count. Both vertex counts and both element
counts must fit the snapshot's corresponding mesh V and E limits.
Check `52·V_r+index_bytes≤max_mesh_bytes`, with index bytes zero when
unindexed or the actual checked index length times its element width.
Check `52·V_r` and index bytes separately against Device max_buffer_size.
Use checked conversions and arithmetic. Actual index bounds, ordered
corner correspondence, class and source binding require bounded worker
validation. Outer lengths provide no proof of contents. These original
and render count gates apply only to the additive profile.
[Current source counts](../../src/three_d/geometry.rs#L1070-L1097)

**Independent caller ceilings.** Require explicit nonzero finite
`maximum_cpu_bytes: usize` and `maximum_exact_verification_work: u64`.
The latter counts versioned priced work. Keep `maximum_certificate_bytes`
as the certificate byte cap and `maximum_verification_items` as the
structural-item cap. Both remain separate from exact work. There is no
default exact-work threshold. Before the roster scan, enforce the
caller ceilings on the actual input charge and the whole-source work
upper bound derived from checked lengths. Bound the class term by
`t·max(W_L,W_Z,W_N)` and include
bounded scan, binding, successor and cancellation terms from §8.4.
An unverified class count cannot discount this gate. The successor
term needs a checked comparison bound without access to live Context
state on the preparation worker. The existing owner checks the actual
eligible predecessor against that bound before successor work.

Scan the bounded fixed roster for class, offsets, masks, lengths and
source correspondence against the real proof backing. Derive class
counts from the checked roster. Price source scans, exact per-class
work, successor checks and required scratch before allocating dependent
data or running the expensive proof. Refuse absent or zero limits,
overflow, malformed or unbounded lengths and checked work above the
caller ceiling. Independently require isolated `I_j+P_j≤M` and caller
aggregate `B+R+P_j≤M` under §§3 and 8.5.
[Legacy worker interface](../../src/three_d/geometry_cpu.rs#L234-L270)
· [Structural item check](../../src/three_d/geometry.rs#L1128-L1144)

**Charged layout and custody.** For the additive path, allocation
capacity and overhead in §§8.5–8.6 refer to a HAL-owned logical charge.
Use a checked conservative requested allocation Layout footprint.
Include the known Arc header, element alignment and padding, and the
capacity actually requested by its owner at creation. Add explicit
fixed custody and boxed metadata charges and price temporary or
duplicate allocations in their live phases. For each `Arc<[T]>`,
record and verify the owner's backing allocation request, layout and
stable identity before sealing. A slice pointer and length do not
supply these facts. Untrusted source values cannot supply allocation
identity. Allocator-private overhead, fragmentation and OS RSS are
outside this metric. The charge establishes a deterministic policy
budget and supplies no physical RSS bound. The legacy HEADER and
size-based quota law remain exact.

Keep the one contiguous immutable proof byte Arc and one fixed-size
roster Arc in the fixed inventory. Count moved or shared backing once
per matching accounting scope. Deduplicate against the eligible
predecessor only when the same backing is already charged in that scope.
Price fresh copies and the 128,192-byte
one-active-B magnitude arena in the live peak phases. Released scratch
carries no retained charge. Preflight and success use the same
immutable allocation identities and cost formula. The sealed inventory
and scalar facts support O(1)-in-source-size UI begin/finish, with
independent Context escrow and generation overlap until actual last use.
[Legacy charge](../../src/three_d/geometry_cpu.rs#L46-L88)
· [Fixed sealed inventory](../../src/three_d/geometry.rs#L770-L800)
· [Context handoff](../../src/three_d/geometry_cpu.rs#L347-L442)

**Cancellation and open decisions.** The 256 counted limb-operation
spacing applies to the additive verifier. Count inner-loop reads,
writes, comparisons and shifts. Retain carry, cursor, GCD and quotient
state across checks. Split any wide pass that could exceed the
interval. The existing FnMut and AtomicBool supply worker control.
The exact counted schedule must establish the bound. Additive
preparation receives unverified source and scalar bounds, with no
source-valid result or Context lease. Preserve the existing CPU-only
sealed successor handoff.
[Checker callback](../../src/three_d/geometry.rs#L1558-L1565)
· [Worker cancellation](../../src/three_d/geometry_cpu.rs#L329-L343)

Exact primitive work coefficients and comparison bounds, C witness
encoding/revision, per-record proof bytes and the final peak phase
formula with owner-verified layout details remain pre-adoption
decisions. Qualified pack-time provenance, separate prepared and HAL
geometry checks, and source-owned absence evidence retain §6's scope.
Keep all R024 Z/N/A/B/C/NormalOnly proofs, R025 original casters,
R028's complete `1/510` domain, and R027 compatible last-good and Queue
law. Independent S1 and producer/wire gates remain open. **KOMPLEMENT
902.779.61 remains NOT ADMITTED.**

### 8.8 Exact R44 witnesses for N Mode C exclusion

**Proposed / Inactive / Non-binding.** JC retains final authority. N still requires its separate A exclusion, exact
B KKT and d² proof, and shared F/U/H derivation and nine-pair packed check.
Z and Legacy C retain their respective proofs. All current admission remains
under the binding contracts.
[R024 §18.4](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#184-proposed-nonzero-canonical-mikk-class-outside-abc)
· [HAL370 §14.2](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#142-prepared-source-and-future-gates)

**Records.** Per N triangle, R40 tag 6 carries `A(R44/schema2)` with exactly
four unique domain/sign pairs in order: original/+T, original/−T,
actual-packed-smooth/+T, actual-packed-smooth/−T. R44 tags 1–4 carry domain
`u8` (0 original, 1 normalized/packed smooth), sign `u8` (0 +T, 1 −T),
separator seed `f64[3]`, and six outward lower values `f64[6]`, respectively.
Require a finite seed with at least one nonzero component and six finite,
strictly positive lower values in every record. Interpret the seed as its
exact binary64 vector at its supplied magnitude. Use the record sign
independently of the original `tangent.w` handedness.

**Source domains.** For original triangle `t` and corner `k`, take raw
NORMAL binary32 from the checked original vertex index and raw pinned Mikk
xyz from signed callback slot `3*t+k`. Preserve their ordered source bits,
including signed zeros and original `w`. Require finite nonzero raw
directions and `w` exactly ±1. The original R44 domain uses the raw xyz
and its record sign is independent of `w`. Revision 1 applies only to
qualified mapped DefaultMikk N with genuine authored TANGENT absence.
Bind the original and material identity, ordered complete pinned capture
(R41 for prepared input), certificate and packing revision.

The producer's checked N declaration of `B/C_proof_revision = 1` in
§8.9 selects one canonical recipe. For each of the six raw triples, run `frame_points`
on exact widened finite f32, then `frame_unit` with its magnitude-safe
scaling, outward enclosure and strictly positive lower length bound.
For each component, compute `((lo + hi) * 0.5) as f32` with ordinary
binary64 arithmetic and binary32 narrowing in that order. Require the
packed triple to be finite and nonzero. Run `packed_direction` with the
unit enclosure and packed triple to verify both endpoint-to-f32 checks
and account for the directed packing error. These bits are the canonical
N smooth words before F/U/H substitution.

The zero-numerator branch of `FrameInterval::div` maps an exact raw zero
component of either sign to packed +0. For any other component narrowed to zero,
retain the sign defined by the revision's arithmetic. The numerical
round-trip equates the two zero signs and supplies no bit proof.
Authenticate the original words and the declaration independently, then
reconstruct the expected words by this exact recipe. A self-comparison
supplies no provenance. This revision adds no persisted smooth field.
Any future separately supplied expected words require a bitwise match.

Verify both proofs on one private stack-local six-triple bundle, with
one reconstruction for the nine B crosses and the packed R44 domain.
The raw R44 domain uses the original bits. A retains its original UNIT
test against the original face or UV tangent. R42 qualifies the derived
render separately after the exclusions. Legacy retains its declared
packing, including the original producer's direct `geometric_unit` narrowing. Changed
interpretation, packing revision or proof content follows the exact
nonwrapping source and certificate successor rules.

Require the common fixed FP profile for rounding, narrowing and exact
bit/refusal conformance on each claimed build. The shared leaf still
uses opaque `powi`. The future source-defined `2^-915` guard and endpoint
`x*x`, Legacy parity and owner checkpoints remain open. The paper fees
stay conditional. Physical CPU OOM, §8.17, full work and Layout/Context
receipts, Mikk qualification and producer schema 2 remain open.
[Original corner and smooth packing owner](../../src/three_d/geometry.rs#L1809-L1859)
· [Packing check](../../src/three_d/geometry.rs#L1012-L1027)

**Enclosure.** Use `FrameInterval::point` for each exact finite binary64
seed component and `frame_points` for exact widening of each finite binary32
triple. For the record's domain and sign `s`, check the first three bounds
on `a·N_i` and the next three on `−a·(s*T_j)` using `frame_dot` and directed
`FrameInterval::mul`, `add` and exact interval `neg`. The seed enters the
dot product directly without `frame_witness`, unit normalization or epsilon.
Require `0 < stored_lower <= verified_interval.lo` for all 24 inequalities
on the actual vectors. A nonfinite input, overflow, unmatched source bits,
an overstated bound or an unresolved enclosure, including a lower endpoint
at or below zero after underflow, yields typed refusal.
[Outward bounds](../../src/three_d/geometry.rs#L199-L218)
· [Interval operations](../../src/three_d/geometry.rs#L803-L875)
· [Point widening and dot](../../src/three_d/geometry.rs#L955-L976)
· [Existing normalizing witness helper](../../src/three_d/geometry.rs#L1006-L1010)

The six strict inequalities put every nontrivial nonnegative combination of
the normal generators on the positive side of the plane through the origin
and every such combination of the signed tangent generators on the negative
side. Positive rescaling preserves both signs, so the cones share no nonzero
ray. Both signs exclude parallel and antiparallel common rays in each domain.
Affine separation of bounded hull points alone is insufficient.

**Work and custody.** Each N triangle decodes four records of two `u8`
and nine `f64` values. Its 24 three-term dots use 72 interval multiplications,
48 interval additions, at most 12 interval sign operations and 24 lower
validations. Each multiply has at most four endpoint products, the guarded
exact-residual check and bounded outward steps. Each add has two endpoint
sums and bounded residual and outward steps. Reconstruct the six smooth
triples in the separately bounded source-interpretation stage and charge
that shared work once. Price record decoding and framing, finite and
nonzero checks, bit association, interval arithmetic including residuals
and outward steps, signs, lower comparisons and cancellation. Whole-source
cost uses the verified N count plus the separate source and roster work.
These operation counts are inputs to a work bound.

The four records occupy 296 scalar field bytes per N before R44 array and
TLV framing. Retain them in the single immutable proof-byte Arc at checked
roster offsets. Decode bounded records into fixed local interval scratch.
Charge the actual proof backing, roster, source and decoded copies, metadata
and temporary peak under §§8.5–8.7. Retained custody uses the two flat
allocations from §8.2. The existing worker checks the proofs and passes a
sealed O(1)-in-source-size handoff to the Context registrar.

**Open S1 decisions.** Fix the C proof and verifier revision, private proof
encoding, exact source bit identity rule, framing and offsets, finite work
coefficients and charged peak phases, and the complete 256-operation B
cancellation and successor boundary. Qualified controlled producer capture,
producer schema2 and coordinated HAL adoption remain separate gates. Preserve
the proof of genuine normal-role absence and original TANGENT absence, all
original R025 faces and casters, R028's complete original normal chain and
`1/510` bound, and R027's compatible last-good, front and Queue law. Retain
the existing renderer, ABI and Context ownership. A source name, triangle ID
or failed qualification supplies no alternate admission. **KOMPLEMENT
902.779.61 remains NOT ADMITTED.**

### 8.9 Canonical two-Arc private proof grammar

**Proposed / Inactive / Non-binding.** JC retains final product authority. This section proposes the private
record encoding, framing and offsets left open by §§8.6–8.8 for the
additive input of §8.2. Public geometry types, legacy preparation and
cost, and the sole Context registrar retain their current contracts.
The corresponding producer wire format remains a separate inactive candidate.
[R024 source modes](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#712-exactly-three-mapped-source-frame-modes)
· [HAL370 proposed coordination](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#14-proposed-s1--coordinated-admission-of-two-derived-source-frames)

**Primitive-wide owner.** Keep one immutable original identity
(`source` and `revision` as u64, `node`, `mesh` and `primitive` as u32),
original vertices, indices and raw signed tangents, and render vertices,
indices and corner map. Bind the qualified source and callback association,
source digest (mandatory for mapped DefaultMikk Z/N, otherwise where supplied),
actual source material/role, mapped flag, primitive-wide tangent origin,
and producer algorithm, version, capture and interpretation/proof revisions.
Keep separate
`maximum_certificate_bytes`, `maximum_verification_items`,
`maximum_exact_verification_work` and `maximum_cpu_bytes` ceilings, and
§8.7's effective Context limits snapshot. The existing material and
snapshot owner checks the role against the actual lease. The worker
receives immutable data and scalar bounds without a live Context or lease.

For mapped DefaultMikk Z/N, require a controlled source owner to qualify the
frozen root and closure, original occurrence and accessors, genuine authored
TANGENT absence, present source normal role, and pinned successful Mikk
invocation with every final signed corner in order. For prepared ingress,
the source producer independently recomputes R41 digests and checks the
frozen original occurrence and completed capture before handing over its checked R8 streams.
Source-only ingress uses equivalent checked facts and exact immutable
streams from its controlled producer transaction without a synthetic R41
wire record.
The source owner establishes digest correspondence. A self-consistent prepared-container
rewrite can reproduce the digests and completion claim, so historical
qualification depends on the controlled transaction.

Obtain one HAL-private additive source-capture input reference only from
the qualified source owner's checked handoff. It pairs the exact immutable
original backing references checked under the same root with one fixed,
inert primitive-wide expectation record. Bind the SourceBinding, canonical
original-input and signed-output H32 values (R41 meanings), original source
identity/revision, occurrence, node, mesh, primitive, material/normal role,
POSITION, NORMAL, UV0 and index accessor identities, actual index kind,
checked O/Eo/K and ordered index/corner coverage, producer identity,
generator algorithm and revision, version, package SHA-256, adapter/settings
revisions, capture generation, completion and primitive tangent origin.
This selects semantic facts with the same two flat proof/roster allocations
and original source streams. Private field layout and ingress implementation
remain open.

The input-side SourceBinding and two capture digests are the values carried
by that qualified reference. The same HAL checker establishes correspondence
by matching actual source, producer, certificate, material and corner
metadata and the O/Eo/K streams to the owner-verified expectation. In one
lossless ordered item handoff, verify the actual O fields, index kind/order
and all K raw signed f32 vec4s against those exact source-owner operands. For
a copied item, check destination bits and Option presence against its source
before releasing the witness, including signed zero and raw w. For an Arc
backing retained by move or share, verify its identity and immutable scope.
This binds the source owner's digest proof to the actual source. Duplicate
unchecked digest values supply no qualification. Check original/render and
ordered-corner association independently. Source parsing, hashing and Mikk
invocation stay with the source owner. Live material/normal lease, Context,
Device and generation checks stay with their existing owners. Missing facts
or backings, unqualified backings, mismatches and unsupported origins give
typed refusal without class relabel or Legacy fallback.

For the candidate's frozen POSITION/NORMAL/TEXCOORD_0 source profile, the source
owner independently checks original `COLOR_0` absence in the primitive
attribute set and requires original R8 color `None` at qualified pack and
prepared reopen. Refuse
`Some`, including `Some([255; 4])`. R41 tag 3 omits original color and the
white render default aliases these states. HAL requires the same qualified
`None` on each original item in the ordered O handoff. For source-only input
outside this profile, require independently qualified Option presence and
exact bytes when present. Preserve that state and refuse unproved color.
The prepared-reopen color check remains an open source-producer qualification gate.

Work remains open. If another independently checking owner later supplies
a separate digest receipt for the same fact, compare it byte-exactly. Charge its
32 checked byte items at up to 144 abstract units each, including their
polls. Apply the same conditional ceiling per extra fixed scalar comparison.
Audit the actual reference fields and backing checks, O/Eo/K transfers,
original color checks, prefix writes and seal/allocation inventory before
closing W_scan/W_bind. Reuse only proven identical visits.

Producer/consumer qualification rebinds file-local R9/R34/R41 symbols to
the actual HAL identities before preparation. Convert qualified prepared
or source-only evidence losslessly into the HAL-owned proof backing,
preserving source, certificate, version and proof meaning. The same HAL
checker validates the original association and mathematical proof.
Prepared-container TLV parsing stays at its source-producer owner. A filename or digest
alone supplies neither source association nor historical Mikk capture.
This backing is private to the additive input.
[Current original and descriptor](../../src/three_d/material_api.rs#L265-L307)
· [Role lease check](../../src/three_d/snapshot.rs#L219-L238)

**Roster and backing.** Use one immutable Arc of fixed-size logical roster
values and one immutable `Arc<[u8]>` of proof bytes. Each roster value has
this exact 56-byte canonical little-endian representation in field order.

| Field | Canonical type | Bytes |
|---|---|---:|
| triangle_ordinal | u64 | 8 |
| certificate_identity | u64 | 8 |
| certificate_revision | u64 | 8 |
| interpretation_revision | u64 | 8 |
| class | u8 | 1 |
| reserved | u8[7], all zero | 7 |
| proof_offset | u64 | 8 |
| proof_length | u64 | 8 |

The 56 bytes describe canonical data for checking and comparison. Actual
Rust storage and its charge use the owner-recorded Layout. Require the
ordinal to equal its sequence index, nonzero certificate identity and
certificate and interpretation revisions, and complete original/render
association to the primitive owner. A qualified primitive certificate
identity may repeat across triangles.

Check each u64 offset-plus-length addition, each offset, length and end
conversion to usize, and the actual backing bounds. Start at offset zero.
Every record starts at the previous checked end and the final end equals
the backing length. An
empty payload leaves the cursor unchanged. Reject overlap, gaps, trailing
data and shared proof bytes. The roster class selects the closed payload.
There is no additional per-record header, tag, count or padding. The
fields below include all class-specific discriminants and lengths. Store
IEEE float bits in little-endian, including signed zero. Check finiteness
where the proof requires it and require boolean bytes 0 or 1. Unknown
kinds, nonzero reserved bytes and wrong lengths refuse before dependent
allocation.

**Closed class payloads.** The class values and all lengths here are
HAL-private. The fields are listed in encoding order.

| Class | Evidence | Ordered payload | Bytes |
|---:|---|---|---:|
| 0 | Legacy OldCone | Empty. Recompute the existing cone proof from source and packed data. | 0 |
| 1 | Legacy AuthoredSmooth | separator f64[3], row-major lower f64[9]. | 96 |
| 2 | Legacy SourceGeometric | separator f64[3], row-major lower f64[9], singularity_kind u8, then the singularity fields below. | 103 or 104 |
| 3 | Legacy NormalOnly | separator f64[3], lower f64[3]. | 48 |
| 4 | Z | Compact R42 semantic fields, then zero-corner mask u8. | 170 |
| 5 | N | Compact R42 semantic fields, then the ordered N fields below. | 543–10,763 |

For class 2, retain the existing HAL singularity rules and bit-exact seed
and lower values.

| singularity_kind | Witness | Ordered fields after kind | Field bytes | Class 2 bytes |
|---:|---|---|---:|---:|
| 0 | VertexInCone | normal_vertex bool, vertex u8, basis u8[3], count u8, antiparallel bool | 7 | 104 |
| 1 | EdgePlanes | normals u8[2], tangents u8[2], reverse_ray bool, antiparallel bool | 6 | 103 |
| 2 | MixedCone | generators u8[4], reverse_coefficients bool, antiparallel bool | 6 | 103 |

For the additive input, genuine normal-role absence requires class 3 on
every triangle and qualified primitive-wide `NotRequiredNoNormalRole`
under the separately adopted source contract. Verify original TANGENT
absence, no generated capture and an empty raw signed stream. The current
legacy unmapped OldCone proof stays on the legacy route. An old
DefaultMikk label with an empty signed stream supplies no new origin
evidence. Existing legacy class meanings and entry points continue under
§§1–5.
[Public frame variants](../../src/three_d/material_api.rs#L370-L409)
· [Private legacy witnesses](../../src/three_d/geometry.rs#L602-L682)
· [Legacy unmapped OldCone](../../src/three_d/geometry.rs#L1957-L1976)

**Compact geometric frame.** Classes 4 and 5 start with these R42 semantic
fields in order. `derivation_revision:u64`, `H:i8` exactly −1 or +1,
`packed_Fp:f32[3]`, `packed_Up:f32[3]`, `unit_witness_seed:f64[3]`,
`packed_nine_lower:f64[9]` in row-major order, `packed_MF:f64`,
`packed_MU:f64`, `epsilon_bounds:f64[2]`, `pack_proof_revision:u64`.
The field sum is `8+1+12+12+24+72+8+8+16+8=169` bytes. Independently
derive F/U/H from the original ordered POSITION/UV, verify the packing,
magnitude and error enclosures, and require bit-identical Fp/Up and
tangent.w equal to H at all three source-mapped render corners. Verify
each of the nine packed cross lower bounds at or above `1/16`. Z and N
require qualified mapped DefaultMikk, genuine original TANGENT absence
and complete pinned signed-corner capture. N requires all original
tangent xyz vectors to be nonzero.

**R42 V1.** Require `derivation_revision = 1` and `pack_proof_revision = 1`.
Any other value in either field yields typed refusal. Derivation 1 follows
[R024 §18.3](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#183-shared-derived-frame-and-cpu-evidence)
on ordered original local binary32 POSITION/UV. Use the existing shared
`derived_source_frame_c` enclosure schedule. It runs `derived_source_frame`
with the area, determinant, projected-tangent and bitangent-orientation
checks, then refines U with three exact dyadic difference expansions
and `frame_unit` after orientation by H. Reuse this numerical helper without
requiring or fabricating Legacy Mode C singularity evidence. Preserve
separate original NORMAL, signed Mikk, authored TANGENT absence,
POSITION/UV/color/index/corner association and current normal-role lease
checks.
[Shared geometric derivation](../../src/three_d/geometry.rs#L1421-L1545)

Pack-proof 1 is the 169-byte direct proof of R024 §18.3. Check both
interval endpoints against the actual finite binary32 Fp and Up using
the shared `packed_direction` f32 round-trip. Separately require the R42 triples and
all three mapped render corners to match in actual bits, including
signed zero. The round-trip's numerical equality supplies no canonical
zero sign. H is the exact original UV determinant sign, and all three
render tangent.w values must equal it. Treat `unit_witness_seed` as an
exact finite binary64 vector s with at least one nonzero component,
preserving the stored bits. Enclose the mathematical unit
`b = s/sqrt(s·s)` with `frame_witness`, which uses the shared
magnitude-safe `frame_unit` check. Refuse an absent, invalid or unresolved
witness.
R44 homogeneous seeds retain §8.8's unnormalized meaning. For every
row-major pair, including off-diagonals, evaluate the outward projection
on the actual packed corners and require a finite supplied lower with
`1/16 <= packed_nine_lower[i,j] <= outward_lo(b · (Fp_i × Up_j))`.
[Witness and packing helpers](../../src/three_d/geometry.rs#L983-L1027)

Require finite strictly positive `packed_MF` and `packed_MU` with
`packed_MF >= frame_length(frame_points(Fp)).hi` and
`packed_MU >= frame_length(frame_points(Up)).hi` for the actual checked
triples. These bounds are independent of ideal unit lengths and Legacy's
`normal_max` and `tangent_max`. `epsilon_bounds[0]` bounds `||Fp - F||₂` and
`epsilon_bounds[1]` bounds `||Up - U||₂`, where F and U are the exact
original-derived directions. Each epsilon must be finite and nonnegative.
Use the individual `packed_direction` error uppers
`frame_length(frame_sub(frame_points(actual), expected)).hi` on
`(Fp, F enclosure)` for slot 0 and `(Up, U enclosure)` for slot 1.
Require each stored epsilon to be at least its own outward upper,
including equality. Keep signed-zero bits in the receipt and compare
bounds numerically. Stored zero refuses when its checked upper is positive.
Legacy's aggregate `maximum_pack_error` cannot supply either attribution.
Any invalid, stale or understated receipt yields typed refusal even
when the nine direct checks pass. The separate error receipts do not
add a perturbation threshold or replace any direct check.

The R42 V1 local paper ceiling is conditional on the specified future
graph. Its core charge is 714,912 abstract units per Z or N triangle.
Charge 311,648 for base and three dyadic F/U/H paths, 88,512 for six
packing/epsilon checks, 24,768 for the unit witness, 21,952 for two
packed norm receipts, and 220,032 for nine direct projections. Charge
24,832 for the checked 169-byte decode, 4,096 for framing and identity,
7,680 for six ordered corner binds, 10,752 for revision, receipt and H
guards, and 640 for fixed results.

The complete top-level [FrameInterval methods](../../src/three_d/geometry.rs#L803-L952)
include nested calls and fixed iterator folds in their charges.
Use point 32, checked 128, neg 64, add 512, sub 576, mul, div, square
and sqrt 2,048 each, and maximum_abs 128. The earlier 240-unit atomic
candidate fails on a finite inexact point product that takes at least
241 source primitives. The bound requires a future common source leaf
with the exact 2⁻⁹¹⁵ guard (binary64 bits `0x06c0000000000000`) and one
self-multiply per nonnegative square endpoint. The five current
`powi` sites (866, 897, 935, 911 and 915) have that lowering in the
inspected macOS ARM64 Rust 1.99.1 Dev and Release builds. A common
source revision still requires authorization and independent Legacy
bit and refusal parity on each claimed build profile.

The future CPU preparation owner checks before and after each of at
most 498 top-level leaf invocations (137 point, 12 checked, 3 neg,
64 add, 71 sub, 125 mul, 26 div, 36 square, 12 sqrt and 12 maximum_abs).
Price 996 leaf polls and nine outer polls for entry, the decoded record,
three original corners, three render corners and common exit. The 701
short checks cover 169 decode bytes, 29 guards, 11 base scalar groups,
27 refiner groups, 188 point/vector wrapper boundaries, 93 finite scans,
36 packing zero/endpoint groups, 147 dyadic component/item groups and
one result group. The 1,706 polls at up to 16 units each add 27,296.
Thus `W_R42_V1_paper <= 714,912 + 27,296 = 742,208` abstract units.
Use bounded per-item checks for this non-B graph without a scalar
operation counter or 256-scalar cadence. The separate 256-limb Mode B
and 256-byte successor rules in §§8.4 and 8.12 still apply. No source-sized
opaque Vec-to-Arc freeze is eligible.

Charge R42 once per qualified Z or N triangle. Price shared source scans,
the raw Z gate and N's A and C/R44 checks separately. Charge the isolated
[Mode B bound](#816-mode-b-numeric-work-and-cancellation-ceiling) of
365,283,608,364 including its polls once for N. Before the roster scan,
bound `t * max(W_legacy, W_Z, W_N)` with source and predecessor envelope
work. After roster and association checks, price the verified partition.
Use checked u64 arithmetic and the caller's `maximum_exact_verification_work`
with typed refusal on overflow or excess. The successful receipt must
match the checked preflight charge for that partition.

The current Legacy checker has no R42 path. This local paper bound
requires the exact visit and checkpoint graph and reopens on any extra
visit, changed leaf, poll or replay. Worker checkpoint plumbing, full
W_frame, W_Z, W_N and request work, source and normal-role provenance,
qualified Mikk capture, producer schema 2, requested Layout, phase peaks,
Context escrow, §8.17 soundness and coordinated adoption remain open.
This proposal grants no implementation or native admission. KOMPLEMENT
902.779.61 remains NOT ADMITTED.

For Z, the following mask is nonzero and uses only the low three bits.
Bit k equals the presence of exact raw zero xyz at the corresponding
original indexed corner, including either signed zero. Check original
raw w for exact −1 or +1, separately from H. Match the Z proof before both
normalization gates. Retain the actual raw bits and apply the required
checks to all nonzero directions. Z and N carry their own evidence
without manufacturing a legacy frame or singularity witness.
[Derived frame source rule](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#183-shared-derived-frame-and-cpu-evidence)
· [Raw gate](../../src/three_d/geometry.rs#L1775-L1781)
· [Smooth candidate](../../src/three_d/geometry.rs#L1844-L1859)

**Proposed / Inactive / Non-binding — Z pre-unit work.** Before the
untrusted roster scan, check `E_o = E_r = 3t`, mapped `K = E_o`, and
the byte and work ceilings, including `t * max(W_L, W_Z, W_N)` and the
shared source, binding and successor upper bounds. Then check the
complete one-to-one ordered roster, class, source/certificate identity
and ordinal, offsets, lengths, proof backing and source/corner mapping.
Require qualified mapped DefaultMikk, genuine authored TANGENT absence,
the actual normal role and the complete pinned original capture. The
worker uses independently qualified source association, with live role
and lease checks at the existing owner. A class or digest alone cannot
qualify the capture. [R024 §18.2](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#182-proposed-zero-callback-source-class)

Before K normalization, a bounded pass on the checked roster authenticates
each Z mask. Read the single checked payload byte at offset 169 and
require a nonzero mask with `(mask & 0xf8) == 0`. Read all three original
vec4s at slots `3*t+k`. For each xyz word, let its magnitude bits be
`bits & 0x7fffffff` and require them below `0x7f800000`. Set observed bit
k exactly when all three magnitudes are zero. Either zero sign qualifies
and a subnormal is nonzero. Require observed mask equality and exact raw
w bits `0x3f800000` or `0xbf800000` independently of H. Preserve all four
raw words. Only a verified set bit skips the two tangent normalization
gates. Both gates reread the bound byte under the same immutable source,
roster and proof bindings after this pass succeeds. All original normals
and unmasked tangents retain their full checks. Mismatch yields typed
refusal. The pass adds no retained mask backing or replacement tangent.

The pre-mask and K passes each use two fixed row items and two pass
boundary items. At 128 units per item plus a 16-unit poll this is a
conditional `576*(t+1)` shared traversal charge, assigned once in the
common scan/binding ledger. Move the six raw w load and guard items per
Z into the pre-mask visit, charged with their polls in `W_scan`.
For each entered nonzero K direction, `W_scan` separately prices
`P+UNIT = 24,768`, six 128-unit slot/view/load/outcome items and 61
polls (44 leaf checks at 22 top-level leaves, six wrapper edges, five
finite/unit guards and six item checks), for at most 26,512 units.
A masked zero incurs no unit traversal. The normal and role scans retain
full coverage. These items supply only part of the shared scan and
binding prices.

The Z-only mask lane has 29 pre-mask items (two span checks, byte,
shape, base, initialization, seven per corner, equality and outcome)
and 12 gate-use items (two span checks, a byte and three bit decisions
per use). At 128 each, its core is 5,248. The 41 item polls and six
entry/exit polls add 752 units, for at most 6,000.

The verified nonzero mask leaves at most two unmasked tangents. Check
those and all three original normals in the later smooth loop. The
earlier K scan retains no interval cache. Each
direction uses `P+UNIT+PD = 39,520` including full interval normalization
and `packed_direction`, with midpoint f32 narrowing priced in the
fixed items. At most five traversals plus 50 additional 128-unit items
(nine per direction for raw access, midpoint and outcome, plus five
setup/cursor/exit) give core 204,000. At most 175 top-level leaves
give 350 leaf polls. Add 70 wrapper checks, 70 finite/unit/zero/endpoint
checks and 50 item checks. The 540 polls add 8,640, for at most 212,640.
Entry/exit already belong to the mask-use lane. R42 separately charges
its three original and three render corner associations, original
F/U/H and all nine cross checks.

Thus `W_Z_extra <= 209,248 + 16*587 = 218,640` abstract units on paper
for this finite graph. It excludes R42 and shared source work. Refusal
visits a bounded prefix and exit. Additional visits or polls reopen
the bound. Non-B item checks require no scalar runtime counter and are
independent of the 256-limb B and 256-byte successor limits. This
candidate requires the common bounded FP leaf, Legacy parity and owner
checkpoints. Opaque source-sized freezes are ineligible and §8.17
remains open. Full Z and request work, source qualification and
physical CPU OOM semantics remain open. KOMPLEMENT 902.779.61 remains
NOT ADMITTED.
[Current worker control](../../src/three_d/geometry_cpu.rs#L253-L267)

**N fields after the compact frame.**

1. `A_family:u8` (0 normal, 1 tangent), `A_corner:u8` (0–2),
   `A_strict_upper:f64` outward-verified on its original direction and
   strictly below `3/4`, then `B/C_proof_revision:u64`. These fields total
   18 bytes. The only proposed N value is `1`. An unsupported value,
   including zero, or mismatched source declaration yields typed refusal
   without fallback or repair. Revision 1 selects the canonical packing
   and four homogeneous R44 records in
   [§8.8](#88-exact-r44-witnesses-for-n-mode-c-exclusion) and the exact B
   checker of §8.3 and [§8.16](#816-mode-b-numeric-work-and-cancellation-ceiling),
   including all KKT checks and strict `d² < 1/256`. R42's
   `derivation_revision = 1` and `pack_proof_revision = 1` are independent.
2. Exactly ten rational pairs, the nine full B weights in row-major
   `(i,j)` order and then the stored reduced d². Each is
   `length_num:u16LE`, `length_den:u16LE`, then exactly those numerator
   and denominator byte spans, respectively. Lengths are 1–512 each.
   Use minimal unsigned little-endian encodings, positive denominator,
   exact reducedness by gcd and canonical zero `0/1`. Verify the complete
   exact B proof and equality to the stored d² under §8.3.
3. Exactly four R44-like records in §8.8 order, original/+T,
   original/−T, packed-smooth/+T, packed-smooth/−T. Each has `domain:u8`,
   `sign:u8`, `seed:f64[3]` and `six_outward_lowers:f64[6]` in that
   order. Domain/sign use §8.8's values. Require the finite nonzero
   homogeneous seed, the original and actual reconstructed smooth
   source bits and all 24 strict directed lower checks. Each record
   is 74 bytes, totaling 296 bytes.

Consume exactly those fields and refuse any oversize, malformed or
unconsumed data. The structural minimum is `169+18+10*(4+2)+296=543`.
The maximum is `169+18+10*(4+2*512)+296=10,763` bytes. The private stored
d² pair has the same 512-byte operand limit as the weights. The computed
quotient bounds in §8.3 remain arithmetic bounds. The separate producer-format candidate's
11,681-byte full R38 upper bound includes its own framing and association; it is not the private proof backing's bound.

**Preflight and consumers.** Before the roster scan, check the actual
outer source counts, backing lengths and caller input and worst-case
work ceilings under §8.7. On the existing worker, check the complete
roster, cursor, lengths, association and exact LE decoding. Derive
class counts only from checked records. Charge all input, proof backing
and roster bytes, scan work, temporaries and bounded successor work
before expensive verification or dependent allocation. Enforce the
independent certificate-byte, input/peak and exact-work limits against
actual lengths as well as the per-kind bounds. A million N triangles
at the per-record maximum imply 10,763,000,000 proof bytes before the
roster or other charges. The caller and Context ceilings still apply.

Use one private accessor for the verified count and original identity
and ordinal at a checked triangle index. It reads the fixed roster for
additive input and the existing certificate array for legacy. Both
supply O(1) lookup and the same count and identity semantics used by
Outline and mip diagnostics. The roster and proof backing supply the
additive certificate facts without a second certificate array or an
old-enum stand-in.
[Outline count and topology](../../src/three_d/outline.rs#L744-L756)
· [Mip diagnostic association](../../src/three_d/mip_sampling.rs#L3539-L3545)

**Successor and custody.** Bind the sealed source and successor to actual
immutable original bits, primitive tangent origin and capture evidence,
class and roster, complete proof bytes and the packed interpretation and
certificate revisions. At equal effective original revision, refuse
changed original bits or reassigned origin or capture. At the same
certificate identity and revision for a source triangle, refuse changed
semantic proof, class or interpretation. For permitted old-to-additive
legacy comparison, compare the semantic fields and exact float bits of
the old certificate against the corresponding evidence. Compare
canonical bytes where both records use this grammar. A digest alone
cannot establish equality. Source and certificate changes require
separately checked nonwrapping successors. Keep the exact permitted
generation transitions and priced, cancel-polled comparison schedule
open for separate S1 review.

The nine caller and eleven sealed named positions of §8.5 have at most
eight and ten occupied, respectively, on this additive route. Each
corresponding legacy certificate-array position is vacant. Preserve the
old seven/nine inventory and HEADER-based legacy cost. Context escrow
uses owner-recorded requested Layout and proven same-backing aliases.
Charge live copies, generated checked frames, scratch and lifecycle
overlap separately according to actual lifetimes. The final peak and
receipt formulas remain open.
[Successor kernel](../../src/three_d/geometry.rs#L1681-L1740)
· [Legacy cost](../../src/three_d/geometry_cpu.rs#L126-L211)
· [Context handoff](../../src/three_d/geometry_cpu.rs#L347-L442)

**Open authority and evidence.** R024 §§7.1.1–7.1.2 and HAL370 §4.1
still govern current mapped source modes. The live legacy enums and
normalization gates cannot admit Z/N through the old input. This proposal
requires separate coordinated adoption and preserves R025's original
faces and complete casters, R028's complete `1/510` normal-domain proof,
R027's radiance, compatible last-good, front and Queue law, and actual
material-role and lease checks. Exact checker/reconstruction revision,
priced work and 256-operation cancellation proof, predecessor charge,
final phase peak/escrow and receipts, genuine qualified pack capture and
producer prepared wire remain open. Use the owner-local arithmetic of §8.3.
Source names and triangle IDs confer no eligibility. This proposal supplies
no source admission or S2 grant. **KOMPLEMENT 902.779.61 remains NOT ADMITTED.**
[R025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md)
· [R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md)
· [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md)

### 8.10 Initial successor lineage and bounded bit comparison

**Proposed / Inactive / Non-binding.** JC retains product authority and final veto. This candidate narrows the
open transition scope in §8.9. Sections 1–5 and the current public
signatures, legacy source proofs, acceptance and refusal semantics remain
governing.
[R024 immutable source law](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#715-immutable-mode-source-and-certificate-association)
· [HAL370 independent revisions](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#42-independent-revision-laws)

**Initial transition matrix.** Classify the two immutable sources under
the existing Context admission owner.

| Predecessor | Candidate | Initial disposition |
|---|---|---|
| Legacy | Legacy | Use the exact existing successor kernel and accepted source and refusal semantics. Its f64 witness `PartialEq` treats +0.0 and −0.0 as equal. Additive bit comparisons and provenance fields are outside this branch. |
| Legacy | Additive | Typed `CROSS-FORMAT-PREDECESSOR-UNSUPPORTED` refusal at the additive transition boundary. |
| Additive | Legacy | The same typed refusal at the additive transition boundary. |
| Additive | Additive | Require the same logical original source/node/mesh/primitive and the complete checks below. |

For a cross-format pair, refuse before dispatch of a worker successor
and before any replacement on the synchronous route. Preserve the
existing public Legacy operation for a legitimate legacy predecessor.
The refusal gives no legacy certificate conversion, fabricated
historical capture, source repair, implicit no-tangent promotion or
fallback enum. Cross-format migration requires separate authority.

For `previous=None`, require complete standalone HAL proof, original
identity and generation checks, and existing Context admission. Perform
no predecessor comparison or retrospective authentication. A separately
qualified independent admission and source switch follows the actual
source, Context, structural and complete extension identity owners. It
supplies no successor proof for the old geometry and cannot relabel an
incompatible old front.
[Legacy certificate fields](../../src/three_d/geometry.rs#L625-L650)
· [Current successor kernel](../../src/three_d/geometry.rs#L1681-L1740)
· [Standalone worker path](../../src/three_d/geometry.rs#L695-L707)
· [Registry generation](../../src/three_d/context.rs#L4751-L4776)

**Additive original lineage.** For a present predecessor require the same
logical original identity and nonwrapping source and certificate
revisions. Reject stale revisions, exhausted required successors or
mismatched identity. Bind capture, interpretation, packing and proof
versions independently.

At equal effective original source revision, compare actual original
POSITION and NORMAL f32 bits, UV option/presence and bits, color option
and values, original index kind/count/values/order, and every raw signed
tangent f32[4] bit. Compare the primitive-wide authored TANGENT presence
and genuine normal-role absence evidence, tangent origin, producer and
capture revision/evidence, and immutable source association. Refuse any
difference. An empty legacy signed stream or legacy DefaultMikk label
does not qualify `NotRequiredNoNormalRole`. That origin requires the
independent source and material-role facts in §6.3.

Changed original content requires a true higher original source revision.
Changed capture requires a true higher capture generation under its actual
owner. Requalify each affected association independently. Binding law
leaves original revision ownership for capture-only changes unresolved,
so the initial profile refuses those changes. A change confined to the
derived rendering can keep the original revision when every original bit,
origin, capture and source linkage is identical. A digest supplies no
replacement for actual comparison.
[Original vertex bits](../../src/three_d/geometry.rs#L576-L582)
· [Legacy origin-sensitive tangent check](../../src/three_d/geometry.rs#L1701-L1720)
· [Original and render association](../../src/three_d/geometry.rs#L1809-L1837)

**Certificate and render continuity.** For a matching original triangle
ordinal with the same certificate identity and revision, require exact
class, interpretation revision, source association and all class-specific
semantic proof bits. Compare f64 and f32 bits including signed zero,
flags and full canonical rational encodings. Compare the verified
class and its checked proof slice in each source. A legally newer
certificate on another triangle can change the byte offset of this
slice. Offset movement alone leaves the proof unchanged. A repeated
per-primitive certificate ID is lawful when bound to each exact ordinal.
A certificate identity mismatch at a matched ordinal refuses.

**Whole render.** For two additive seals with the same qualified original
identity/revision, exact O/Eo/K and source/capture continuity, triangle count
and ordered original topology, require whole-render equality when every
matched row retains its interpretation revision and none has a separately
lawful packing/mapping successor. Use checked original comparisons and the
actual qualified row and owner revisions to establish this scope. Compare V
cardinality and ordered projected PBR52 bits, Er kind/count/order/values
including Unindexed, and map cardinality/order/values. Refuse differences.
A certificate revision advance alone gives no exception.

**Mixed render.** With the original unchanged and a lawful row successor
outside the whole-render guard, scope equality to each matched row j with
unchanged interpretation. Check all three elements `e = 3*j+k` and resolve
oi from Eo[e].
Require exact Er kind and raw ri at each e (implicit ri=e for Unindexed),
exact map[ri]=oi and original-corner association, and bit-identical
corresponding projected PBR52 values. Equal certificate generation also
requires the class, source and proof checks above. An advanced certificate
needs fresh independent proof and keeps this render check at unchanged
interpretation. Treat raw ri renumbering or map reassignment as changed
render-corner expansion even if resolved oi and packed bits coincide. A
global Er kind change affects every matched row. Permit changed raw kind,
ri, map or packed values only with lawful interpretation and certificate
advances on every affected row, exact qualified oi/e association, complete
independent proof of its declared interpretation and the applicable
structural and full-extension successors.

An unchanged certificate generation cannot name changed class, source,
mode, interpretation, proof or packed frame. A changed class or proof at
a matching triangle requires the true advanced nonwrapping certificate
revision, appropriate interpretation, structural and full extension
successors, and fresh independent HAL mathematical certification.
Numerically identical render output cannot stand in for the same declared
meaning. Source, capture and role keep their independent owner checks.

**Original and derived changes.** A change confined to V cardinality, seam
duplication or split, Er representation or V-to-O mapping can keep the
original revision only with exact original continuity and the affected
rows' successors and proof above. A split may assign new ri to changed rows
while preserving every retained row's exact raw mapping and packed corners.
Refuse a change to shared V that changes a retained corner. Require complete
V, Er and map extent and source checks under §8.7 and §8.9, including
`Eo = Er = 3T`. Added or unused V supplies no row successor or original
change authority.

Changed original topology, cardinality, O/Eo/K, original index kind or
original source/capture content requires a genuinely higher original source
revision and complete recertification by original ordinal. Verify
complete rosters and original/render correspondence, including added or
removed ordinals. A prefix zip cannot qualify the change. Refuse
resurrection of an old ordinal under its unchanged certificate revision.
An ordinal absent from the immediate predecessor might have been removed
earlier. Its absence proves neither novelty nor its earlier revision.
Refuse an addition or return whose required generation continuity cannot
be established under the existing owner. Exact evidence for that case
remains an independent S1 gate. No historical registry is selected.

At a lawful higher original revision, matched source-bound certificates
require eligible source/certificate successors and full recertification.
For independently HAL-sealed versions with the same logical source, node,
mesh and primitive, a genuine nonwrapping higher original revision can
change derived packed bits at unchanged interpretation. Require a lawful
nonwrapping certificate advance for every affected matched ordinal and
refuse an unchanged source-bound certificate. Independently check the
revised root, occurrence, accessors, original O fields and Options, Eo kind
and ordered values, exact raw K bits and capture, and ordered corners.
Require fresh complete class proof and the original lineage and ordinal
coverage above. At unchanged interpretation, prove the same shading meaning,
class, algorithms, configuration, deterministic derivation and packing
profile, and interpretation and proof version meanings. The revision label
alone supplies no proof. A packed value or mapping change must derive from
the changed qualified original inputs under that same rule, including any
independently qualified same-rule seam expansion. Refuse a mapping change
without that provenance and derivation proof. A changed interpretation rule,
algorithm, packing profile or class requires lawful interpretation and
certificate advances. Require the actual owning geometry, material,
structural and full-extension successors and requalify affected R-025,
R-028 and R-027 dependencies. The whole and mixed render checks above
continue to govern equal original revisions.

Capture-only changes without lawful original
succession still refuse. Keep the per-ordinal checks on the worker and
price each repeated V/Er/map visit and bounded cancellation separately.
For Additive-to-Additive succession, rely on each seal's independently
verified F under its actual admitted §8.11 profile. Bitwise equality,
`f64::PartialEq`, numeric ordering and per-element F equality supply
no successor predicate. A separately qualified HAL profile can produce
different conservative bounds for identical producer evidence without
itself requiring a new original or certificate revision. Equal F values
supply no source or revision evidence. Apply the source, proof,
interpretation, packed-corner and owning-generation checks above.
Preserve §4 same-candidate source/frame and refusal conformance and
synchronous Legacy `f64::PartialEq`, including signed-zero behavior.
BEGIN and FINISH stay O(1) in source size. Keep `40F` as the retained
logical span and charge actual fixed profile, identity and size checks.
Omit automatic per-ordinal F value visits from `W_successor(Some)`.
Any later F value comparison requires separate authority and proved
256-byte two-token accounting. The prior
worksheet remains provisional arithmetic. Exact item/256-byte/two-token
accounting and complete successor work remain open.
[Packed render data](../../src/three_d/geometry.rs#L484-L503)
· [Current corner proof](../../src/three_d/geometry.rs#L1782-L1859)
· [Legacy certificate zip](../../src/three_d/geometry.rs#L1722-L1738)

**Predecessor comparison envelope.** At additive preparation require the
caller's checked envelope for expected predecessor presence and
representation. For a present predecessor it bounds original/render
vertex and element counts, raw corners, roster/proof bytes, fixed
identity/capture metadata sizes and calculated maximum successor byte
steps. Bound both sources with checked arithmetic before dependent
allocation or proof work. The independent
`maximum_exact_verification_work` must cover the reserved comparison as
well as candidate proof, scan and cancellation work. The envelope sets
a work ceiling. It supplies no predecessor authority. The next
triangle count cannot stand in for a bound on the previous source.

At UI begin the existing Context owner verifies the actual predecessor
family/key and logical source under the live lease. Check previous
presence, representation and sealed actual lengths and provenance
against the envelope using fixed inventory and scalar checks. Refuse a
mismatch or exceeded envelope or work bound before capturing predecessor
CPU referents or dispatching comparison. The existing Pending holds the
actual predecessor lease and escrow. The worker receives only immutable
Arc referents and the inert issuer, then verifies through the same
geometry checker and sealed successor job. UI begin and finish remain
O(1) in source size. Finish rechecks the exact ticket and current
predecessor, Device, Context and cancellation before registration.

The verifiable source of the envelope and the priced coefficients
require independent S1 review. If they cannot be obtained within this
boundary, refuse a non-None additive successor. A standalone
`previous=None` admission still requires all its own proof and costs.
[Legacy preparation](../../src/three_d/geometry_cpu.rs#L234-L270)
· [Worker successor job](../../src/three_d/geometry_cpu.rs#L328-L343)
· [Context begin and finish](../../src/three_d/geometry_cpu.rs#L347-L442)
· [Context escrow](../../src/three_d/context.rs#L1951-L1989)

**Separate byte cancellation bound.** For new additive predecessor
comparison, allow at most 256 actual compared or copied bytes between
cancellation checks. Count byte-wise element reads, equality
comparisons, copied and decoded bytes, and fixed scalar comparisons by
their actual encoded widths. Split operations into explicit bounded
loops or chunks. An opaque slice equality or bulk copy that exceeds the
bound cannot count as one operation or run between polls. Check at
entry, each completed segment of at most 256 bytes, record boundaries
and exit. Charge the polls, cursor and record work separately. This
byte bound is independent of §8.4's 256 counted limb operations for
arithmetic. Legacy polling and equality stay under their existing
contract.
[Legacy element checks](../../src/three_d/geometry.rs#L739-L765)
· [Worker cancellation](../../src/three_d/geometry_cpu.rs#L329-L337)

**Charge and authority boundary.** Bound comparison work from actual
checked original and packed render vertices, indices, corner maps, raw
signed tangents, immutable provenance, roster/proof backing and checked
certificate/frame results of both versions. Include applicable
normal-only legacy witness fields, byte scans, decoded or copied spans,
per-record/element work and polling. Context separately charges the
owner-recorded requested Layout and actual old/current/candidate and
in-flight overlap, including predecessor escrow through last use.
Deduplicate only proven actual backing aliases. Exact work
coefficients, peak phases and matching receipts remain open.

The geometry worker verifies captured source facts. Existing
material/snapshot/viewport owners retain actual role, texture, sampler,
model, scene, structural and full extension identity and generations.
Preserve R025 original faces and casters, R028's complete `1/510`
normal-domain proof, R027's compatible last-good, actual front and same
Queue retirement. Failed qualification supplies no alternative class.
This is an inactive candidate with independent S1, producer and prepared
wire gates. **KOMPLEMENT 902.779.61 remains NOT ADMITTED.**
[Material role leases](../../src/three_d/pbr.rs#L602-L675)
· [Snapshot association](../../src/three_d/snapshot.rs#L219-L238)
· [Extension successor](../../src/three_d/viewport.rs#L4330-L4341)
· [R025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md)
· [R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md)
· [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md)

### 8.11 Sealed predecessor envelope source

**Proposed / Inactive / Non-binding.** JC retains product authority. This section supplies the planning source
for §8.10. Sections 1–5 and all existing legacy signatures, proofs and
refusal semantics remain governing.
[R024 immutable source law](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#715-immutable-mode-source-and-certificate-association)
· [HAL370 independent revisions](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#42-independent-revision-laws)

**Additive source seal.** The existing geometry checker and sealer keep
one immutable fixed-scalar summary in the certified wrapper after
complete independent validation. Bind it to the actual immutable source,
original identity and capture, origin and mapped status, interpretation
and proof revisions, checked roster and proof backing, and owner-recorded
allocation identities and requested Layout charges. Record original and
render vertex counts O and V, original and render element counts Eo and
Er, raw signed corner count K, triangle count T, checked-frame count F,
proof length L, index kinds and widths, immutable metadata byte length,
finite verification-work upper receipt and requested Layout charge
summaries. The existing Context registrar associates the sealed source
with its actual resource provenance at registration. Per-triangle
certificate identities and revisions stay in the fixed roster Arc,
and exact proofs in the flat proof-byte Arc. The seal uses the one
existing checker and Context owner. Caller and producer claims
require independent HAL checks before sealing.
[Current certified wrapper and seal](../../src/three_d/geometry.rs#L1637-L1649)
· [Legacy cost and inventory](../../src/three_d/geometry_cpu.rs#L60-L126)
· [Current registrar](../../src/three_d/pbr.rs#L860-L876)

The existing HAL CPU preparation worker derives and independently checks
each conservative five-field f64 `F[j]` against the exact qualified
original, corner correspondence, packed output and class-specific
producer proof. Retain one result per original ordinal (`F = T`) in the
immutable checked-frame Arc. Bind the collection in the same fixed-scalar
seal to the immutable HAL-owned numerical derivation-profile identity
that actually generated it. The identity denotes the checker recipe and
aggregation, outward rounding and error semantics, applicable build/FP
profile and numerical revision. Producer, R42 pack-proof and row
interpretation revisions carry their own meanings. Require independent
qualification of the source numerics, common FP leaf and exact Legacy
bit/refusal parity for each claimed profile. A caller label or revision
increment supplies no qualification. The worker gives typed refusal for
missing or invalid profile identity, invalid bounds, wrong count or
association, or weakened or unproved result semantics. Price derivation,
validation and bounded writes at preparation and charge retained Layout
and peak/escrow separately. Exact Z/N five-field aggregation, private
profile representation and width, M visits and bounded work remain open.

The initial inactive FP envelope is limited to the installed macOS
`aarch64-apple-darwin` Rust/std 1.99.1 at
`58cf8f9c35b355199727627d07ae34a4e2846d67`, with distinct actual Dev
`opt-level=1` and Release profiles. Before adoption record each build
invocation, rustc/LLVM versions and lowering, target flags, hardware and
CPU features, and runtime FP mode. Verify nearest ties-to-even, gradual
underflow without FTZ/DAZ, signed zeros, correct f64 division and sqrt,
genuinely fused `mul_add`, and separate TwoSum and outward operations.
Preserve an oracle from the unchanged Legacy leaf before any candidate
source edit. In a separately authorized stage, compare it with the
candidate on each claimed profile. Require exact endpoint bits including
signed zero, point or widened result, typed refusal, exact affected source
and certificate outcomes, and checked-frame bits for A/B/C/NormalOnly
across existing public and worker paths, including RED originals. This
comparison must pass before sharing the candidate leaf. Then prove §4
same-candidate synchronous/CPU conformance separately.
The candidate uses exact `g = 2^-915` (bits `0x06c0000000000000`) for the
three guards and one `x*x` at each nonnegative nonpoint square endpoint
with the existing branch order, finite checks and directed bounds.
Distinguish `g*1` and `g/1` at the guard, and sqrt at `g`, `2^-916` and
`2^-914`. With `a = 1+2^-52` and `x = 1+2^-51`, check the multiply
and sqrt residual `a²-x = 2^-104` and the fused residual `-2^-54` for
rounded `1/3` times three against one. Below the guard, with
`m = 2^-1022`, distinguish the residuals of `(m*a)*a`, `m/a` and
`sqrt(m*x)` with exact magnitude `2^-1126`. Include subnormal and
round-to-zero squares at `2^-536` and `2^-538` at both endpoint sites,
zero crossing, `±0` sqrt, finite point and widened results, and overflow
or nonfinite refusals. Independently check future Z/N R42 F tuples with
positive normal quotients and predecessors. The existing mode/index,
corruption, successor and C round-cell fixtures retain their coverage.
The added discriminators are future qualification cases.
Price bounded leaves from the verified compiler/std expansion or finite
source schedule for division, sqrt, fused multiply-add and fixed iterator
work. Keep the 1,706 paper R42 polls, Mode B limb cadence, successor byte
cadence and six F preparation checks separate. This envelope supplies no
new fee or performance claim. An unverified profile or parity failure gives
typed Z/N refusal under the shared checker. The private F profile carrier,
shared FP change, the original asset-inspector integration and native admission, and full request
work remain open.
[Legacy differential fixtures](../../src/tests/three_d_context.rs#L3603-L3743)
· [C round-cell fixture](../../src/tests/three_d_geometry.rs#L146-L235)

For each independently qualified mapped Z or N original triangle `j`, this
inactive recipe carries five row-local scalars from the same R42 V1 checks
into the HAL result item. `C` is the minimum of all nine independently
computed outward lowers of `b · (Fp_i × Up_k)` for the checked unit witness
and actual packed corners. `MF_checked = frame_length(frame_points(Fp)).hi`
and `MU_checked = frame_length(frame_points(Up)).hi` are the actual norm
uppers. Bit-identical Fp and Up across the three mapped corners justify
one norm upper per direction.
`eF_checked` and `eU_checked` are the separate computed `packed_direction`
error uppers against the reconstructed ideal F and U. Check each stored
floor, norm upper and epsilon as required by §8.9. Use the computed
endpoints in the five-field result for the same ordinal and HAL profile.

```text
normal_lower                 = round_down(C / MU_checked)
tangent_lower                = round_down(C / MF_checked)
cross_lower                  = C
local_g_lower                = round_down(C / MF_checked)
maximum_direction_pack_error = max(eF_checked, eU_checked)
```

The nine projections and independent convex weights give `|n × t| >= C`.
The norm uppers give `|n| >= C/MU_checked`, `|t| >= C/MF_checked` and
`|g| = |n × t|/|n| >= C/MF_checked` for local Gram–Schmidt over actual
packed values. Require finite `C >= 1/16`, finite positive norm uppers and
finite nonnegative computed errors. The finite f32 component range and
checked norm graph give norm uppers below `2^132`. Each exact quotient
exceeds `2^-136` and is at most its corresponding true packed norm, below
`2^129`. Follow each of the three correctly rounded scalar f64 divisions
with its positive bit predecessor. The rounded quotients and predecessors
are positive normal, so the zero branch is unreachable. The `1/16` gate
applies only to the projections. Computed error uppers can be +0 or positive.
Preserve original and packed signed-zero bits, exact UV-derived H and the
mapped corner checks. This calculation needs no raw K-zero normalization.
Refuse nonfinite, invalid or unproved operands, results or source proof
with a typed error before sealing.

R42 derivation and pack-proof revisions, N canonical packing, producer
receipts and certificate revisions retain meanings separate from the HAL
numerical profile. The mapped Legacy field meanings alone establish no
bit equivalence. Exact operation order, qualified FP semantics and §4
same-candidate conformance remain independent gates. §8.10 governs
successor checks without an F-value predicate.
[Mapped field calculation](../../src/three_d/geometry.rs#L2008-L2046)
· [Directed rounding](../../src/three_d/geometry.rs#L199-L208)

Price the actual bounded endpoint handoff, min/max, three scalar divisions
and predecessors, result, slot and prefix writes, and required polls
before accepting a work increment. This recipe adds no FrameInterval
evaluation or retained array. Claim reuse or execution deduplication only
with an independently proved schedule and charge. The 640 fixed-result
units and one result-group poll establish no charge for an unshown write
or computation. The 742,208-unit R42 paper bound retains its stated scope.
The exact bounded result-item and poll census and complete work remain open.

The inactive non-B result schedule accumulates `C` from all nine computed
lowers and carries `C`, `MF_checked`, `MU_checked`, `eF_checked` and
`eU_checked` in local scalar state bound to the same source, original
ordinal, certificate/proof, packed corners and HAL profile. After the R42
checks, checkpoint before assembly. In order, evaluate `C/MU_checked`,
`C/MF_checked` and `C/MF_checked` again. Each separate group computes its
scalar quotient and `round_down` predecessor, validates both as finite and
positive, then checkpoints. Revalidate the carried bounds and association,
take the two-error max and assemble the exact typed five-field value.
Require checked ordinal conversion and bounds, `j == i` and `i < n`.
Compute checked `next = i + 1` and require `next <= n` before slot access.
The fifth checkpoint immediately precedes safe unique `Arc::get_mut` and
checked writable slot acquisition under §8.17. Failed guards or access
refuse before writing. Complete exactly one typed `MaybeUninit::write`,
then assign `i = next` without an intervening checkpoint. The sixth
checkpoint follows both operations. On refusal or cancellation, drop the
unsealed Arc and return a typed error without publication or panic.
After all source-admitted `n` ordinals, require `i == n` and one successful
final check immediately before the proposed once-only same-Arc seal. Count it
separately from the last row unless the actual trace proves one identical
check with no intervening work requiring a separate check. Each of the
six row events and one collection-end event checks the one existing
preparation token once. Successor two-token and byte checks and Mode B
limb checks retain their separate scopes. Audit actual min/max visits,
quotients, predecessors, guards, value assembly, unique access, write,
prefix and callback paths, including extra bounded checks. Prices remain
open. The 640 fixed-result units and 1,706 R42 checks retain their charges.
Credit an overlap only with a proved matching visit, boundary and token
coverage. The schedule supplies no incremental fee or seal authority.
[Preparation checkpoint](../../src/three_d/geometry_cpu.rs#L248-L267)
· [Successor checkpoint](../../src/three_d/geometry_cpu.rs#L350-L358)

For an inactive Z/N row bound to the same original, R42 proof and roster,
actual corner bits and HAL profile, initialize local f64 `C` to `+∞`.
Visit all nine pairs `(i,k)` in row-major order from `(0,0)` to `(2,2)`.
Take `c_ik` from the same computed outward `FrameInterval` lower used for
its R42 floor check. Require both values finite, exact row and corner
association and `1/16 <= stored_floor <= c_ik` before updating
`C = min(C,c_ik)` with that endpoint exactly once before advancing.
Preserve the existing pair checks and polls. In that row capture
`MF_checked` and `MU_checked` from the computed norm `.hi` endpoints at
their claim comparisons,
using the verified bit-identical Fp and Up across the three mapped
corners. Capture `eF_checked` and `eU_checked` from the separately computed
`packed_direction` error uppers for the verified F and U enclosures when
checking their claims. After all nine pairs and norm/error checks pass,
require finite `C >= 1/16`, finite positive norm uppers and finite
nonnegative errors tied to the checked ordinal and certificate. Pass
only the five f64 scalars with that checked association into the
existing six-event result and slot path. Any failed check, missing or
mismatched endpoint gives typed refusal without publishing a partial F.
Reuse the computed endpoints directly, without another FrameInterval
evaluation or retained array. Price the initialization, nine endpoint
transfers and min updates, four norm/error handoffs, actual association
checks, moves, guards and callbacks once. The R42 evaluations and 1,706
polls retain their charges. The 640 fixed-result units and one result
poll give no automatic credit. Assume zero overlap until actual source
and an admitted HAL/std profile establish the same visit and single-token
boundary. The incremental charge and preflight receipt remain open.

For §8.9 qualified original capture, the additive seal's occurrence key
is the source-owner-verified tuple of canonical SourceBinding root H32,
original node, mesh and primitive as u32, material as u64, four u64 original
accessor identities in POSITION/NORMAL/TEXCOORD_0/index order, and the actual
original index kind as u16. R12 kind 1 is U16 with componentType 5123 and
kind 2 is U32 with componentType 5125. The owner verifies these facts from
the frozen root and primitive. The 86 logical value bytes exclude framing
and other metadata and select no wire encoding, Rust layout or work charge.
At handoff and seal verify each version's explicit source/capture association
to its rebound HAL logical original identity and revision under the actual
owner. Keep this runtime binding distinct from the root digest, file-local
symbols and names, backing addresses and live Context key. BEGIN checks only
sealed fixed scalars and the live predecessor under its lease. In the worker
visit root/source association, occurrence fields and actual index kind in
that order, then the separate source-input and signed-output H32 facts,
provenance and generations. Count each actual visit once, including rereads.
At equal effective original revision, require exact key and runtime binding
equality plus §8.10's exact O/Eo/K bit and Option checks and producer/capture
and role continuity. Refuse any mismatch. The input and signed-output H32s
stay attached to the actual original streams, whose exact bits, including
signed zero and raw w, remain required. R41 tag 3 omits color. The frozen
producer profile's independently verified COLOR_0 absence and HAL's exact
original `None` check at the O handoff stay separate from the key. A lawful
higher nonwrapping original revision requires independently qualified candidate
occurrence and source under §8.10's same-logical-original requirement and
applicable source, certificate, structural and full-extension successors.
A key change requires those owners' authority. Capture-only succession and
unproved ordinal reappearance remain refusals. Remaining M fields, widths
and visits, mixed-generation render/frame visits, exact 256-byte and
two-token checkpoint accounting, and complete successor work remain open.
The earlier work worksheet remains provisional arithmetic. The original-source and prepared-reopen
checks described in §8.9 and the existing ingress, Layout and escrow gates remain open.

**Planning snapshot and envelope.** Before additive CPU preparation,
the caller may obtain a planning-only immutable scalar snapshot on UI
from the actual owning Context lease for an expected predecessor.
Use the sealed source summary and the live Context issuer/family,
kind, slot and generation associated with that lease. The snapshot
may go stale and supplies no admission credential. Only inert
qualification and scalar facts pass to preparation. The lease, live
Context, Device, registry, Queue and GPU stay with their owners.
This proposal selects no public ABI.
[Inert issuer](../../src/three_d/context.rs#L438-L455)
· [Controlled key](../../src/three_d/context.rs#L4472-L4491)
· [Live source lease](../../src/three_d/pbr.rs#L879-L890)
· [Registry lookup](../../src/three_d/context.rs#L5121-L5131)

Preparation requires explicit checked finite CPU and exact-work ceilings
and the caller's checked predecessor comparison envelope. For Some,
bind the expected actual issuer/family/kind/slot/generation, logical
original source identity and additive representation. Independently bound
prior and next O/V/Eo/Er/K/T/F/L, index kind and width, fixed provenance
metadata extent and owner-recorded Layout charge. Require a versioned
maximum comparison work and poll bound, including both sources' actual
spans under their checked limits. Price candidate scans and verification
within the independent work ceiling. The next T supplies no bound on
the prior. For None, explicitly seal absence and charge zero
predecessor comparison work. Keep the effective mesh/Device limits
snapshot of §8.7 separate from predecessor facts.
[Effective limits](../../src/three_d/context.rs#L1838-L1840)
· [HAL355 budget law](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#13-finite-budgets-and-device-limits)

**Live gate.** At BEGIN, match actual predecessor presence to the
envelope. For Some, acquire a fresh actual lease for the expected
key. Check live family/kind/slot/generation, logical
source/node/mesh/primitive, additive representation, sealed scalar
counts, metadata and charges against the envelope. Use O(1) scalar
and fixed inventory checks and recheck current Device and policy
limits. Reject stale, withdrawn, foreign,
over-budget, mismatched or unsealed prior sources before cloning
predecessor CPU referents or dispatching successor work. Some never
degrades to None. Pending retains the actual lease and escrow. The CPU
job holds immutable source Arcs and the existing inert binding.
FINISH rechecks live Context and Device, exact ticket and predecessor
generation, cancellation and applicable limits. Without a lawful prior
seal or checked envelope, non-None additive succession remains
unsupported.
[Current BEGIN and FINISH](../../src/three_d/geometry_cpu.rs#L347-L442)
· [Pending escrow](../../src/three_d/context.rs#L1951-L1989)

**Comparison span.** For each independently checked version i, use
the following conservative single-pass field span with checked arithmetic.

```text
S_i = 36*O_i + J(Eo_i) + 16*K_i + 52*V_i + J(Er_i) + 4*V_i
      + 56*T_i + L_i + 40*F_i + M_i
```

For each index stream J(E) is 0, 2E or 4E for Unindexed, U16 or U32.
M_i separately bounds its immutable metadata extent. L_i obeys
§8.9's proof lengths and actual byte caps. The 36-byte original
field span includes the maximum UV/color data. The roster's
56 bytes and each frame's 40 field bytes are logical widths. Count
option presence, kinds, fixed scalar work, repeated reads, comparisons,
copies, decoding and polls separately under versioned coefficients and
checked bounds. This span alone supplies no complete work bound.
The retained render source uses typed vertices. A bounded comparison
must account for exact packed values without assuming a contiguous
Vertex52 backing or invoking an unpolled full pack.
[Typed and packed render source](../../src/three_d/geometry.rs#L246-L277)
· [Current full pack](../../src/three_d/geometry.rs#L484-L503)
· [Original fields](../../src/three_d/geometry.rs#L569-L582)
· [Checked-frame fields](../../src/three_d/geometry.rs#L667-L674)

Maintain §8.10's maximum of 256 actual compared/copied bytes between
cancellation checks, including charged reads, decodes and scalars at
their actual widths. Retain cursor and mid-field state, and price and
check polls and exact copies. Opaque unbounded equality or copy cannot
satisfy the bound. This is separate from Mode B's 256 counted limb
operations. Legacy comparison and cancellation retain current semantics.
[Current worker cancellation](../../src/three_d/geometry_cpu.rs#L328-L343)

**Charge and authority.** Use only the owner-recorded logical requested
allocation Layout including Arc header, element capacity, alignment,
padding and explicit custody wrappers. The metric excludes allocator-private
memory and RSS. The proof backing and roster are the only two new
retained flat proof allocations. Keep existing source arrays and
checked frames charged without a per-triangle proof graph. Distinguish
input I_j, additional peak P_j, additional retained A_j and caller
baseline B plus earlier retained R. B includes already charged input.
Enforce checked `A_j <= P_j`, `I_j+P_j <= M` and `B+R+P_j <= M`.
Success adds only A_j to R. Context independently reserves
escrow for actual old, candidate, pending, in-flight and retiring
overlap through last use. Deduplicate only proven backing aliases in
the same charge scope.

Keep §8.10's matrix. Legacy→legacy uses the existing contract. Both
cross-format predecessor routes refuse. Additive→additive requires
complete independent source, certificate and interpretation lineage.
A standalone None requires full proof and Context admission.
Capture-only succession and unproved ordinal reappearance remain
unsupported under §8.10. TANGENT absence requires independent
source evidence. Material, snapshot, structural and complete
extension owners stay separate. Preserve R025 original
faces and casters, R028's complete `1/510` domain, and R027 last-good,
front and Queue custody.
[R025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md)
· [R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md)
· [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md)

**Open gates.** Versioned numerical work coefficients, visits and poll
counts, actual requested Layout records and full peak/escrow phases,
exact B cancellation proof and algorithm revisions still require
independent selection and proof. This section establishes no work-fit
receipt. Qualified Mikk pack provenance, producer schema2 and coordinated
R024/HAL370/HAL380 S1 remain open. The proposal grants no implementation
or admission. **KOMPLEMENT 902.779.61 remains NOT ADMITTED.**

### 8.12 Exact work ledger, stored d² reducedness and polling

**Proposed / Inactive / Non-binding.** JC retains final authority. This section clarifies inactive §§8.4 and 8.9
for the selected schedule. The arithmetic ceilings below are checked.
Exact implementation coefficients and a numeric work receipt remain open.
Binding §§1–5 and all legacy behavior continue to govern.
[R024 immutable proof law](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md#715-immutable-mode-source-and-certificate-association)
· [HAL370 independent revisions](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md#42-independent-revision-laws)

**Mode B widths and storage.** A finite binary32 component is
`X·2^-149` with `|X|<2^277`. A cross numerator satisfies `|C|<2^555`.
The upper magnitude widths under §§7.2–7.4 are as follows.

| Value | Bits at most |
|---|---:|
| D and A_i | 36,864 |
| E_i | 32,768 |
| Weight sum before equality | 36,868 |
| Q after ΣA_i=D | 37,419 |
| S | 74,840 |
| T_i | 37,976 |
| D T_i | 74,840 |
| Hypothetical KKT difference | 74,841 |
| Δ=D²·2^596 | 74,324 |
| Strict comparison right side D²·2^588 | 74,316 |
| Equivalent shifted left side 256S | 74,848 |

An unsigned 64×64 MAC with one old digit and one carry is at most
`(2^64−1)^2+2(2^64−1)=2^128−1`. Check signed magnitude and
carry/borrow separately. The 1,170 magnitude limbs hold 74,880 bits,
with a guard limb in each wide working slot. The pre-equality sum
uses a guarded slot. Keep the §7.4 named lifetimes, bounded in-place
passes and one active proof. They give 16,024 u64 limbs or 128,192 bytes
of magnitude payload. Sign, length, carry, cursor, decode, alignment and
allocation overhead are separate. Checks preserve the running state
without replay or additional full-width arrays.

**Counted visits.** The checked ceilings apply to the selected §7.3
schedule and widths. A span counts limb positions for one pass.

| Lane | Upper counts |
|---|---|
| Schoolbook products | 138 calls, 5,163,588 MACs, 55,565 initialization positions and 55,565 result-scan positions. At most 46,206 row-carry positions using the larger operand as the row bound. |
| Signed additions and subtractions | 93 top-level calls, 40,779 limb positions. |
| Sum, threshold and KKT comparisons | 11 comparisons, 12,277 positions from 577+1,170+9×1,170. |
| Restoring division | 331,776 narrow bit rounds ×65 and 149,164 wide ×1,162, totaling 480,940 rounds. One visit span totals 194,894,008 positions. |
| Binary GCD | 73,728 narrow rounds ×64 and 149,164 wide ×1,170, totaling 222,892 rounds. One visit span totals 179,240,472 positions. |

The signed span sums `27*9+9*577+27*585+3*1170+27*594` positions.
Compare the signed KKT sides directly, including equality on positive
support. Forming nine explicit differences would add nine subtractions
outside this schedule. The division widths include the one-bit growth
of the partial remainder to 4,097 or 74,325 bits before subtracting.
GCD uses the stated
conservative wide operand width, and the final common-power restoration
fits the original operands. The combined 374,134,480 division and GCD
positions cover one visit per applicable round. Each additional pass and
its scalar operations must be priced.

**Stored d² and reducedness.** Decode ten bounded rational pairs with
40 bytes of length fields and up to 10,240 operand bytes. Check lengths
1–512, minimal unsigned little-endian encodings, positive denominator
and the canonical zero `0/1`. Validate the nine weight pairs with
the counted narrow GCDs. For S>0, use the counted wide `g=gcd(S,Δ)` and
exact `S/g` and `Δ/g`. Require zero remainder for the nine E_i divisions
and the two positive-S quotient divisions. For S=0 use `0/1`.

Check stored lengths before comparison. Require exact canonical byte
equality to the computed numerator and denominator, including the
exact lengths. The computed quotient ceilings of 9,355 and 9,291 bytes
bound arithmetic output. Each stored operand remains capped at 512
bytes. A matching computed reduced pair proves reducedness of stored
d². The phrase “exact reducedness by gcd” in §8.9 applies to the nine
weight checks and the computed S/Δ reduction. The stored pair uses
the equality proof in §8.3. A second stored-pair GCD is outside the
schedule and could add 8,192 narrow rounds. The separate producer R43
wire requirement for canonical reduced d² retains its meaning.

**Symbolic work ledger.** Use checked sums of priced primitive visits.
Let s price one limb shift visit, c compare, a add/subtract with
carry/borrow, z trim, and x copy. Include their bounded reads and
writes. Let d and g price scalar work per division bit and GCD round,
including numerator-bit access, quotient-bit update, parity, guards
and common shift state as applicable. The following inequalities are conditional
on a verified primitive schedule that satisfies the stated pass caps.

```text
D(w) <= w*(s+c+a+z) + d
G(w) <= w*(2*s+c+a+2*z) + g

W_div <= 331776*D(65) + 149164*D(1162) + W_div_setup_end
W_gcd <= 73728*G(64) + 149164*G(1170) + W_gcd_setup_end

W_B = W_decode + W_mul + W_signed + W_compare_shift
      + W_div + W_gcd + W_quotient + W_state
```

Price MAC operand and old-digit reads, product and two additions,
result writes and carry extraction, initialization, row writes and
result scans. Price signed and comparison passes at their capped widths. Include binary32 lifts,
endian decode, canonical and zero checks, bound arithmetic, shifts
and restoration, length and sign work, terminal remainder checks and
both quotient comparisons. Charge setup/end copies with x and price
zeroing and repeated visits at their actual bounded spans. Count
cursors, scalar guards and any nonlimb decoding explicitly. Every
helper must expose its bounded visits. Fixed numeric prices and their
verified schedule require independent S1 review.

**Polling clarification.** For Mode B arithmetic carry a counter across
adjacent loops and check after at most 256 actual counted limb
operations. For L such operations and E explicit early, boundary,
entry and exit checks, `floor(L/256)+E` is an upper bound for a
schedule with full 256-operation chunks and every shorter forced
poll included in E. A maximum interval of 256 alone supplies no
such poll bound. Price polls and cursor work from the selected
schedule, including shorter segments. Preserve row, carry, borrow,
division remainder/quotient, GCD and copy positions across checks.

The additive successor has its own carried counter for at most 256
actual compared/copied bytes between checks under §8.10, charged
at actual encoded widths. Nonlimb decode, frame work, source scans
and proof parsing need bounded per-item checks and separate counts.
A 1,162-limb pass or 9,355-byte quotient comparison needs internal
counted checkpoints. An opaque uninterruptible helper such as a
full Vec pack is ineligible. Use the existing FnMut and AtomicBool
control. Legacy polling retains its current meaning.
[Worker checkpoints](../../src/three_d/geometry_cpu.rs#L328-L343)
· [Unpolled full pack](../../src/three_d/geometry.rs#L484-L503)

**Source and request bound.** Keep the 56-byte logical roster and
§8.9's private class lengths 0, 96, 103–104, 48, 170 and 543–10,763.
The §8.11 S_i formula is a single-pass logical field span. Charge
typed render projection, option presence, checked offsets and lengths,
repeated reads, comparisons, decodes, copies and polls. Use separately
checked predecessor and next extents and associated proof slices,
including where offsets move. A standalone None charges zero
predecessor comparison. Compare exact packed bytes under the selected
proof. The retained typed source supplies no retained Vertex52 Arc.

For N, C uses 24 three-term interval dots, 72 interval multiplications,
48 additions, up to 12 signs and 24 lower checks. Also price the R44
decode, finite/residual/outward guards, original and reconstructed
packed bit association including signed zero, A exclusion and shared
F/U/H, packing and normalization. Price Z's raw-zero association
before normalization and the full common frame proof. Price actual
Legacy helper calls and genuine normal absence. Shared work may be
charged once only under the verified reuse graph.
[Frame helpers](../../src/three_d/geometry.rs#L803-L1028)
· [Derived frame and dyadic helpers](../../src/three_d/geometry.rs#L1421-L1545)
· [Existing triangle and packed-pair checks](../../src/three_d/geometry.rs#L1809-L2069)

```text
W_N = W_frame + W_A + W_C + W_B
K = W_input + W_preflight + W_scan + W_bind + W_successor + W_cancel
W_early = K_cap + t*max(W_legacy,W_Z,W_N)
W_j = K + t_L*W_legacy + t_Z*W_Z + t_N*W_N
```

W_input checks the outer input and inventory. W_preflight prices the
bounded shape and roster checks. W_scan and W_bind cover actual source
visits, conversion and provenance association. W_successor uses the
independent comparison envelope. W_cancel prices all lanes' polls and
cursor bookkeeping once. K_cap bounds these nonclass terms from
checked lengths and the comparison envelope. Use W_early before
trusting class counts or scanning the roster. Use the exact verified
partition for W_j only after complete roster and source association. Sum the full request with
checked u64 arithmetic and enforce the explicit caller
`maximum_exact_verification_work`. Overflow or excess yields a typed
refusal before the affected work. The sealed success receipt uses the selected checked
preflight charge. BEGIN verifies the actual predecessor envelope under
the live lease using fixed metadata. Source-sized work remains on the
worker.
[Context handoff](../../src/three_d/geometry_cpu.rs#L347-L442)

The u32 index-count type alone gives a theoretical
`t <= 1,431,655,765` with 3t elements. At that count a per-triangle
charge above 12,884,901,891 exceeds u64 even before other terms.
A coefficient of 35 on just the combined round visit span yields
13,094,706,800 work units and exceeds that allowance. This calculation
supplies no default, Device limit or work-fit receipt.
[Index count type](../../src/three_d/context.rs#L29-L37)
· [HAL355 finite budgets](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#13-finite-budgets-and-device-limits)

**Open gates.** Select the exact versioned primitive coefficients,
helper and reuse graph, input/proof/predecessor visit multipliers,
poll schedules, the exact B cancellation proof and checker revisions.
Owner-verified requested Layout, P_j peak and Context escrow phases
remain separate. Qualified
producer capture, producer schema2, coordinated R024/HAL370/HAL380
adoption and actual asset and native evidence remain open. Preserve
true normal absence, R025 original faces and casters, R028's full
`1/510` domain, and R027 last-good, front and Queue custody. This
inactive proposal grants no S2 or admission.
**KOMPLEMENT 902.779.61 remains NOT ADMITTED.**
[R025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md)
· [R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md)
· [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md)

### 8.13 Proposed / Inactive / Non-binding — requested Layout and peak/escrow phase obligations

**Proposed / Inactive / Non-binding.** JC retains final authority and veto.
Binding §§1–5, including §3's last-worker-release requirement, continue
to govern. This section records a live source conflict and grants no
implementation or change to legacy admission, costs or refusals.

**Current legacy ledger.** The seven input Arc positions use logical
payload sizes and `HEADER`. The five additional peak phases are
`a=o+oa`, `b=oa+t+ta`, `c=oa+ta+p+r`, `d=oa+ta+r+ra` and
`e=oa+ta+ra+f+fa+cwrap`. Here `o/oa`, `t/ta`, `r/ra` and `f/fa`
are the original-vertex, private certificate, typed render and checked-frame
Vec/Arc charges respectively. `p` is the intermediate packed Vertex52 Vec
charge and `cwrap` is the certified wrapper charge. The peak adds fixed
overhead to the largest phase. The retained model counts
`oa+ta+ra+fa+cwrap` plus fixed overhead. Existing conversion moves the
original and render indices, raw signed tangent and corner map backings.
It generates private vertices, certificates and frames. These are source
observations and modeled bounds. They establish neither the allocation
requests of the existing Arc/Vec conversions nor allocator residency.
[Legacy cost and preparation](../../src/three_d/geometry_cpu.rs#L19-L270)
· [Input conversion](../../src/three_d/material_api.rs#L473-L559)
· [Typed render and frame sealing](../../src/three_d/geometry.rs#L1567-L1650)

**Fixed inventory and requested Layout.** Use the §8.5 named inventory of
nine caller and eleven sealed positions, with at most eight and ten occupied
respectively. The caller positions cover original vertices and indices, raw
signed corner tangents, render vertices and indices, the render-to-original
map, one flat proof byte Arc and one fixed-record roster Arc. The sealed
positions cover the corresponding retained backings with checked typed
render vertices, plus the checked-frame Arc and certified wrapper. The
legacy certificate-array position is vacant in each additive inventory.
Charge live copies and lifecycle allocations outside these positions at
their actual phases. Keep the seal with the existing certified wrapper.

At the existing allocation owner, verify each stable backing identity and
record requested element count and capacity, element alignment, known Arc
header and padding, and a checked conservative requested Layout footprint.
The logical charge excludes allocator-private overhead, fragmentation and
process RSS. Arc pointer/length, `size_of_val`, the 56-byte canonical roster
value and the 128,192-byte magnitude payload alone cannot supply a backing
Layout. For a foreign input without owner-proven Layout, require an
independently proved conservative charge and reservation before dependent
allocation, or refuse the additive source. Before a `Vec` to `Arc` slice
conversion or `Arc::new`, reserve for any overlapping old and new backings
using verified upper footprints where exact requests are unavailable.
An unknown footprint without a proved upper bound refuses. No allocator
layout, hook, platform introspection or global accounting owner is selected.
[Legacy fixed inventory](../../src/three_d/geometry.rs#L770-L800)
· [HAL355 finite budgets](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#13-finite-budgets-and-device-limits)

**Preparation phase live sets.** The following are additive charge
obligations for the existing CPU preparation owner. Count every earlier
allocation still live in a later phase, including independent copies.

| Phase | Backings and scratch to account for while live |
|---|---|
| Input and original conversion | Caller input includes original/render vertices and indices, raw signed tangents, the corner map, and any supplied proof and roster backings. Charge HAL-owned decoding, original copies and original Vec/Arc overlap as additional. Upstream producer source and scratch still held belong to caller baseline `B`. |
| Proof/roster conversion and parse | Charge any fresh flat backings, their builders or freeze copies, and temporary R43 endian decode, proof framing and source-association state. Reuse of an already charged input backing requires actual alias evidence. No second certificate array or per-triangle retained Arc. |
| Render construction | Charge the live packed Vertex52 intermediate and typed render Vec together when both are needed. Include requested capacities and any index or corner-map copies. |
| Typed render freeze | Charge the typed Vec and checked render Arc together during any overlapping freeze. A retained typed render Arc supplies no retained Vertex52 buffer. |
| Source and frame verification | Account for the live immutable source/proof backings, the checked-frame Vec, and the one active Mode B arena together. Price the arena's 128,192-byte payload and separate sign, carry and cursor storage. Include live interval enclosures, C witness decoding, normalization, proof-bit association, source scans and checkpoint state. |
| Frame Arc and wrapper seal | Account for the checked-frame Vec/Arc transition, the certified wrapper and its fixed summary, and every earlier backing still live. |
| Prepared retention | Count the live source, proof, roster, checked render and frame backings, wrapper and retained lifecycle charges. Include this phase in the peak. Released scratch and conversion buffers carry no retained charge. |

[Checked-frame builder](../../src/three_d/geometry.rs#L1805-L1810)
· [Frame and interval helpers](../../src/three_d/geometry.rs#L803-L1028)

For primitive `j`, `C_p` is the checked sum of unique additional requested
Layout charges and verified conservative conversion reservations live in
phase `p`. Exclude caller-charged input and proven aliases. Count fixed
lifecycle overhead `H_j` once, outside `C_p`. If wrapper lifetimes differ by
phase, place those charges in the applicable `C_p` live sets instead of
adding them again as fixed overhead. Set `P_j = H_j + max_p C_p`, including
the retained phase, and check `A_j <= P_j`. `A_j` is the exact additional
retained charge, including retained lifecycle data and excluding moved or
already charged shared input. Enforce both `I_j + P_j <= M` and
`B + R + P_j <= M` with checked sums before dependent allocation. `I_j` is
unique immutable input charge and `B` already includes that input. The
preflight and successful sealed receipts must agree for the same actual
input identities and charge formula. Only success adds `A_j` to `R`.
Failure adds no success debit. Actual held references stay charged until
last use under their governing scope.

**Independent Context escrow.** BEGIN and FINISH use the live Device and
mesh limits, the pending quota, the §8.11 predecessor seal and the one-use
result checks. Check the predecessor under the live lease and reserve the
pending charge before capturing its CPU referents for the worker. Price
actual reachable source backings and the ticket,
Binding, Pending custody and boxed charge, including the predecessor lease
and escrow Vec when present. Also price the controlled source wrapper,
registry record Arc, slot and retired-vector capacity growth, and the
registration overlap with old, current, candidate, pending, in-flight and
retiring ownership. Cloning an Arc retains the existing backing. Distinct
wrappers and grown allocations have their own charges. Deduplicate only
actual identical backing already charged in the same accounting scope.
The caller receipt supplies no Context quota. Keep the existing reservation,
registry and Queue custody boundaries. O(1) in source size excludes
vertex, triangle and proof scans on UI. Fixed lifecycle allocations and
registry work remain chargeable.
[BEGIN and FINISH](../../src/three_d/geometry_cpu.rs#L347-L442)
· [Context pending charge and escrow](../../src/three_d/context.rs#L1951-L1989)
· [PBR source wrapper and predecessor lease](../../src/three_d/pbr.rs#L860-L890)
· [Registry registration and retirement](../../src/three_d/context.rs#L4743-L4850)

**Refusal and cancellation.** Preflight the checked budgets before dependent
allocation and charge attempted capacities, copies and temporaries in their
live phases. Existing fallible Vec reservations return typed errors. A policy
byte bound does not prove successful Vec-to-Arc or Arc allocation, and the
current infallible allocation sites do not supply a typed failure path. The
additive capacity and refusal proof remains open at those sites. Worker
failure, cancellation, stale predecessor, Device loss and registration
refusal permit no partial source publication. Keep any compatible last-good
front under its original tuple and withdraw an incompatible front under R027.
While Context remains live, a retired pending charge retains its predecessor
escrow while the worker ticket has a strong reference. Keep immutable lease
generations, R025's complete original faces and casters, and R028's full
`1/510` normal-domain proof.
[R025](SPEC-R-025-Direct-Lighting-and-Geometric-Shadows.md)
· [R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md)
· [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md)

**BLOCKER — last Context teardown under binding §3.** `Pending::drop`
retires its charge into `Context3DCore::normal_cpu_retirements`. The CPU
job keeps immutable source Arcs and `Binding._ticket`. Its issuer
`ContextId` retains only an identity Arc. If the last `Context3DCore` drops
while that job still holds the ticket, `NormalCpuRetirements3D::drop`
unconditionally drains the retired nodes, including those with live
tickets. Dropping their dynamic storage leases reduces the charged counters
and releases predecessor escrow while worker source Arcs may still live.
Those Arcs keep their data valid. The source path violates the literal
unconditional last-worker-release escrow requirement in §3, lines 59–60.
The loss of the Context quota owner at teardown supplies no exception to
that requirement. It needs a separate owner-approved binding-law resolution
and source conformance evidence. This appendix selects no remedy and leaves
the no-Context/Device/GPU/registry/lease worker boundary and nonblocking UI
cancellation intact, including the prohibition on waiting or joining in
UI Drop.
[Binding §3](#3-bounded-cpu-residency-cancellation-and-failure-closed-s1)
· [Binding, worker and Pending drop](../../src/three_d/geometry_cpu.rs#L278-L326)
· [Inert issuer](../../src/three_d/context.rs#L438-L455)
· [Collection, teardown and counter release](../../src/three_d/context.rs#L893-L953)
· [Context core ownership](../../src/three_d/context.rs#L1690-L1719)

**Open gates.** Establish owner-recorded requested Layout provenance and
overhead, numeric upper reservations, closed phase maxima and Context
escrow charges against the real allocation sites. Separately close the
binding teardown conflict, versioned CPU work and poll coefficients, Mode B
checkpoint proof, algorithm revision, qualified controlled Mikk capture,
producer schema2 and coordinated HAL adoption. This section grants no S1
adoption, S2 or source admission. **KOMPLEMENT 902.779.61 remains NOT ADMITTED.**

### 8.14 Final Context escrow lifetime amendment candidate

**Proposed / Inactive / Non-binding.** The candidate addresses
only HAL380's terminal Context accounting. Binding §§1–5 govern as written.
This appendix grants neither adoption nor implementation authority.

**Source basis.** BEGIN reserves CPU charge with either predecessor case.
For `Some`, Pending also holds a controlled predecessor lease in its escrow
and Job gets a plain immutable prior source clone. Job and Result retain
candidate data and a Binding with inert `ContextId` and a strong ticket.
Pending holds the Context core through its Context handle. When Pending
drops, the live Context collector retains the retired node while the ticket
has a strong owner. If Pending's Context is the final core owner, core
destruction drains that list regardless of ticket count. The retired node's
Context charge and its optional predecessor lease can end while Job or
Result still holds CPU source Arcs. Those Arcs keep their storage valid.
This is the static accounting conflict recorded in §8.13. Binding §3,
lines 59–60, and the §4 gate at line 69 have no such exception today.
[Binding §3](#3-bounded-cpu-residency-cancellation-and-failure-closed-s1)
· [Binding §4](#4-conformance-and-acceptance-closed-s1)
· [BEGIN and CPU handoff](../../src/three_d/geometry_cpu.rs#L278-L395)
· [Inert ContextId](../../src/three_d/context.rs#L438-L455)
· [Live collection and final drop](../../src/three_d/context.rs#L893-L954)
· [Core ownership](../../src/three_d/context.rs#L1690-L1719)
· [Predecessor lease](../../src/three_d/pbr.rs#L879-L890)

**Proposed §3 live-Context rule.** Throughout the life of a Context core,
the HAL380 reservation and any predecessor escrow MUST cover worker custody
until the worker's last immutable source references and strong ticket are
released. While the core survives, the existing collector may reclaim a
retired charge after that release. Pending drop, cancellation, predecessor
revocation, unmount, Device loss and drop of any nonfinal Context handle
provide no early debit or free-capacity assertion. Keep the UI handoff O(1)
in source size, the existing finite Context admission and ticket collection,
and nonblocking UI cancellation and teardown without wait or join.

**Proposed §3 terminal exception.** Only destruction of a `Context3DCore`
on release of its last strong owner ends its entire finite accounting and
admission domain. At that boundary, the Context-owned retired HAL380 CPU
reservation and its optional predecessor escrow lease MAY end even if an
inert Job or Result still owns immutable CPU Arcs. Classify that drop as
the end of Context accounting. Each source backing remains owned until its
actual last strong Arc releases it. A surviving worker buffer is still
live physical storage.
No quota becomes available to a live sibling Context. No old key or result
may admit, reattach, publish or reuse resources in the terminated domain,
and a new Context may not accept its stale identity. FINISH still requires
the live matching Context, Pending, ticket and predecessor checks.

The Context registrar keeps resource authority. The worker may hold only
the existing immutable CPU data and inert proof binding. Context, Device,
Queue, GPU resources, registry, controlled lease and Context quota stay
outside worker ownership. This exception would end only the retired HAL380
escrow lease. Independently held generation leases still protect their
actual users and submitted work follows existing Queue-safe retirement.
Compatible last-good fronts retain their exact original provenance.
The exception supplies no cache eviction, earlier front release, new
publication path, owner, registry, worker or scheduler.
[HAL355 ownership](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#4-ownership-matrix)
· [HAL355 explicit Context](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#52-explicit-context-ownership-and-injection)
· [HAL355 lease law](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#73-leases-and-retirement)
· [HAL355 finite budgets](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#13-finite-budgets-and-device-limits)
· [R027 Queue and front law](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md)
· [FINISH association checks](../../src/three_d/geometry_cpu.rs#L398-L442)

**Independent caller budget.** An independently active non-Context caller
or task budget MUST keep its surviving worker buffers charged through
actual final Arc release under that budget. The consumer must prove its
accounting owner, covered allocations and lifetime through shutdown.
HAL373 §7's normal-certificate shutdown rule is a separate precedent.
It supplies no HAL380 caller-budget proof. A CPU maximum argument or
receipt alone proves no such custody. Without that evidence, process-global
CPU high-water after final Context teardown remains unproven. Ending a
Context quota supplies no bound on bytes still live in the process.
[HAL373 shutdown custody](SPEC-HAL-373-Frame-Bounded-PBR-Normal-Certificate-Admission.md)

**Proposed §4 conformance gate.** Preserve the other §4 requirements.
For escrow expiry, distinguish live-Context last-worker release from the
final-core-only accounting boundary in the proposed §3 exception. Before
claiming conformance to an adopted amendment, require finite causal proof
of both branches. Exercise `None` and `Some` with an outstanding Job and
with an outstanding Result in each branch.

1. BEGIN must reserve the actual charge and, for `Some`, retain the current
   predecessor lease. Show the worker has immutable candidate and optional
   prior CPU custody with the ticket and inert issuer. In the successful
   Result case, account for the prior CPU clone released by verify and the
   retained candidate and ticket.
2. Keep a separate live Context clone, drop Pending and exercise the existing
   collector with a strong worker ticket. Prove that slots, CPU charge and
   any predecessor escrow survive. After actual worker source and ticket
   release, show the existing collector reclaims that charge once.
3. In the terminal case prove every other Context clone is gone and actual
   final-core destruction occurs with the Job or Result still live. Verify
   retirement-node destruction, the counter debit and the optional predecessor
   escrow release. Verify worker-held source backing, including the `Some`
   Job's prior CPU source, remains valid until its own actual Arc releases.
   Distinguish releasing one Arc from freeing the backing.
4. Prove no late Result can finish without live matching Pending, Context
   and identity. Show final teardown reopens no admission and transfers no
   capacity or authority. Establish no UI wait or join and no change to
   actual generation, Queue or front custody and compatible last-good truth.
5. When a surviving caller budget is claimed, independently verify its
   owner and charges through the last relevant Arc release, including the
   terminal Context interval. State separately any unproven process bound.

**Historical disposition and separate gates.** §8.14 remains inactive.
The conflict descriptions in §§8.13–8.14 record the pre-adoption wording.
The adopted final-Context clarification
resolves that wording conflict only for terminal Context accounting in
binding §§3–4. Accepted source evidence is recorded in [§4](#4-conformance-and-acceptance-closed-s1). This proposal
executes no tests and changes no Constitution text. The inactive Z/N
requested-Layout, peak and escrow receipts, numeric verification work and
priced cancellation, qualified producer source capture and schema2, and
coordinated R024/HAL370/HAL380 S1 and S2 remain separate gates.
**KOMPLEMENT 902.779.61 remains NOT ADMITTED.**

### 8.15 Verified std profile for owner-requested Arc Layout

**Proposed / Inactive / Non-binding.** JC retains authority and veto. This candidate applies
only to the proposed additive Z/N receipt. Binding §§1–5 and Legacy cost
and API semantics continue to govern. This appendix grants no adoption,
implementation or admission.

**Verified installed profile.** The inspected witness is rustc 1.99.1,
stdlib `58cf8f9c35b355199727627d07ae34a4e2846d67`,
`aarch64-apple-darwin`, 64-bit, with the default `Global` route. The charge
bounds owner-requested logical allocation Layout with checked arithmetic.
Allocator buckets, `malloc_usable_size`, physical allocation success and
process RSS are outside this metric. Use the verified target's typed size,
alignment and stride. For a fresh Vec owner request of c elements,
`V_T(c) = Layout::array::<T>(c)?.size()`, with zero charge only for proven
no-backing cases such as an empty or zero-sized-element Vec.

In this std, the private Arc header Layout H consists of two pointer-width
atomic words, size 16 and alignment 8. The slice request is
`A_T(n) = H.extend(Layout::array::<T>(n)?)?.0.pad_to_align().size()`.
For a sized wrapper use the actual value Layout in the same verified
composition. `A_u8(1) = 24` bytes. A one-byte payload plus the legacy
`HEADER` totals 17 bytes and misses the tail padding. `Arc::new` uses the
private `ArcInner` wrapper. `Arc::from(Vec)` allocates a distinct slice
backing, moves the elements, then releases the Vec backing. A nonzero
`try_reserve_exact(c)` on a fresh non-ZST Vec uses the inspected checked
`grow_exact` request. The public Vec guarantee supplies sufficient
capacity, and ordinary pushes within that capacity do not grow. The Arc
request proof comes from this std implementation. It is neither a stable cross-version
layout guarantee nor public ABI. Independently prove the actual sized
and slice constructor requests in every proposed profile.
Inspected std anchors are `library/alloc/src/sync.rs:387–445`,
`2353–2359`, `4072–4095`, `library/alloc/src/raw_vec/mod.rs:544–561`,
`769–801`, and `library/alloc/src/vec/mod.rs:371–395`, `1531–1547`.

**Pre-allocation charge.** Preserve the seven §8.13 phase live sets.
Reserve each concurrent owner request before dependent allocation,
including `V_T(c) + A_T(n)` during Vec-to-Arc freeze and old plus new
requested backings at growth or shrink that can move storage. Verify fresh
reserve and caller builder counts from their creation path. Capacity
observed afterward supplies no retroactive reservation. Price input and
original conversion, the single proof-byte and fixed-roster backings and
their conversion, packed and typed render builders, typed Arc freeze,
frame verification, frame freeze and certified wrapper, and prepared
retention. Use the roster's actual Rust Layout rather than its logical
56-byte encoding. Include the checked-frame builder alongside one active
128,192-byte Mode B magnitude payload and its sign, length, carry, cursor,
interval, decode and other owner-proved scratch where live.
[Source builders](../../src/three_d/material_api.rs#L458-L559)
· [Render and frame freeze](../../src/three_d/geometry.rs#L1567-L1650)
· [Checked-frame builder](../../src/three_d/geometry.rs#L1805-L1810)

Count proven aliases once within each accounting scope. Moved input stays
in `I_j` and the already charged caller baseline `B`. Generated copies
carry their own charge. Each `C_p` sums additional live requests and proved
reservations. Keep `P_j = H_j + max_p C_p` with the retained phase included
and each fixed lifecycle charge counted once. Check
`A_j <= P_j`, `I_j + P_j <= M` and `B + R + P_j <= M`. Preflight and
success use the same checked immutable identities and receipt. Context
independently reserves reachable candidate and prior allocations and their
Pending, in-flight, retiring, registration and Queue overlap. Price actual
BEGIN ticket, Binding, boxed charge and optional escrow Vec, FINISH data
and controlled-record Arcs, and registry capacity growth at their owner.
Phase-specific allocations stay in the phases where they can be live.
No complete numeric receipt or physical memory bound follows here.
[BEGIN custody](../../src/three_d/geometry_cpu.rs#L373-L414)
· [Pending charge](../../src/three_d/context.rs#L1951-L1963)
· [Registration wrapper](../../src/three_d/pbr.rs#L849-L876)
· [Registry capacity](../../src/three_d/context.rs#L4849-L4872)

**Foreign live inputs.** The current public descriptor carries no
owner-recorded allocation request. The inspected Global Arc deallocation
Layout can bound a valid live heap backing only when type, allocator and
build profile are independently established. It supplies no creator Vec
capacity, historical overlap or already-live caller scratch evidence.
Std `library/alloc/src/sync.rs:3544–3556` and
`library/alloc/src/alloc.rs:311–321` are the profile witness. A source-owner
receipt also needs independent authentication and a matching actual
backing identity. Refuse additive dependent allocation when the required
foreign provenance or bound is unknown. Keep the §8.5 fixed inventory and
the two flat proof backings. Untrusted producer claims supply no HAL
authority, and this model adds no per-record Arc or global owner.
[Original descriptor](../../src/three_d/material_api.rs#L299-L307)
· [Geometry descriptor](../../src/three_d/material_api.rs#L398-L410)

**Supported-profile gate.** The manifest contains macOS, iOS, Windows,
Unix and wasm target conditions. Those conditions alone establish no 3D
backend support. The repository root has neither a `rust-toolchain.toml`
nor a `rust-toolchain` file. The installed Mac std inspection establishes
allocation evidence for that build only. Metal conformance requires
separate execution evidence.
[Manifest target conditions](../../Cargo.toml#L33-L59)

HAL355 §14.2 allows S1 architectural adoption before execution evidence
exists for unclaimed backends. Implementation and backend conformance
are separate statuses. An additive Z/N conformance or support claim for
a specific backend and build requires independently reproduced
owner-requested Layout bounds for its target, toolchain, std and allocator,
plus independent evidence of controlled source and refusal, complete
proof, work and cancellation, allocation receipts and backend conformance.
Every claimed backend must pass the same semantics under HAL355 §14.1,
without backend variants, hidden cfg/env/user switches, degradation or
runtime fallback. Unproved profiles remain unverified for Z/N execution.
Browser runtime remains provisional and outside current runtime support
under HAL355 §14.3.
[HAL355 architecture and backend status](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#142-authority-implementation-and-backend-status)
· [Same native semantics](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#141-native-baseline)
· [Browser status](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#143-browser-status)

**Refusal and allocator failure.** HAL380 §2 requires typed capacity,
budget and overflow refusal before dependent allocation, consistent with
HAL355 §13.1's checked policy limits. `Vec::try_reserve_exact` can return an
allocator-reported failure. Infallible `Arc::new`, `Arc::from(Vec)`,
`Box::new` and growing registry pushes provide no stable recoverable
typed result on physical CPU OOM. A checked quota receipt establishes a
policy bound and reserves no heap. `Arc::try_new` and `Box::try_new` remain
unstable on 1.99.1, and the current Arc freeze has no stable fallible form.
HAL355 §7.4 governs the baseline Device/Queue lifetime, including
possible terminal Context after unrecoverable failure. Physical CPU OOM
handling is a separate owner decision and remains open. This candidate
selects no CPU process-fatal policy, custom allocator or unsafe path.
Std anchors are `library/alloc/src/sync.rs:598–608`, `2184–2193`,
`library/alloc/src/boxed.rs:377–380` and `library/alloc/src/alloc.rs:606–624`.
[HAL355 finite budgets](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#131-normative-budget-law)
· [Binding CPU admission](#2-selected-public-and-internal-cpu-contract-closed-s1)
· [HAL355 Device lifetime](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#74-baseline-device-lifetime)

**Open gates.** Architectural review still covers source-owner Layout
provenance, allocation and refusal semantics, the exact wrapper, arithmetic,
interval, decode, cursor and registry growth charges, numeric phase costs,
and versioned work coefficients and polling. Reproducible build-profile
receipts and backend evidence are separate conformance gates for each claim.
Qualified original Mikk capture, producer schema2, coordinated R024/HAL370/
HAL380 binding adoption and asset verification remain separate. The
final-Context escrow source conformance is closed in §4. This appendix
executes no tests and grants no S2 or native admission.
**KOMPLEMENT 902.779.61 remains NOT ADMITTED.**

### 8.16 Mode B numeric work and cancellation ceiling

**Proposed / Inactive / Non-binding.** JC retains final authority.
The bound covers one N triangle's exact B proof on the specified future
verifier revision. The installed geometry verifier is Legacy under §§1–5.
Implementation, physical CPU timing, an executable-work receipt and whole-request
acceptance require separate evidence.

**Arithmetic and custody.** Follow §§7.2–7.4 and §8.3. Bind the nine ordered
nonnegative weights to the checked packed smooth source and require exact sum
one. Compare all nine KKT sides `D·T_i ≥ S`, with equality on positive support.
Require `S < D²·2^588`, giving strict `d² < 1/256`. Nine narrow GCDs verify
the weights. Require zero remainders on the nine E_i divisions and, for
positive S, use one wide GCD and two further zero-remainder divisions.
Canonical length and byte equality to the computed reduced pair prove stored
d² reducedness without a tenth narrow GCD. Zero S uses canonical `0/1`.
Ten R43 pairs have 40 prefix bytes and at most 10,240 operand bytes. Computed
quotients may reach 9,355 and 9,291 bytes. Check their lengths first and compare
each matching operand's at most 512 bytes by extraction from its computed limbs.

Use one active B proof. The named arena in §7.4 holds 16,024 u64 magnitude
limbs (128,192 bytes), including the four guarded slots. Carry the running
weight sum beside E_i quotient and remainder. Keep S and Δ while reusing
two slots for the wide GCD copies. Keep the restored factor separately, then
reuse those slots for one quotient and remainder at a time. Signs, cursors,
headers, alignment, requested Layout and other live buffers are separate.
The witness uses bounded in-place passes without extra full-width copies,
a bigint library, replay, a quotient encoding buffer or per-triangle proof Arcs.
[Work ceilings and reduction](#73-verified-work-ceilings-and-stored-distance-reduction)
· [Arena lifetimes](#74-arena-lifetime-and-bounded-class-work)
· [R43 grammar](#89-canonical-two-arc-private-proof-grammar)

**Counted direct passes.** Each explicit typed u64/u128 load, store,
arithmetic, bit, shift, multiply, carry, relation, branch, cursor, index or
guard operation costs one abstract unit. A checked read/write costs at most
five, including the bounds relation and branch, stride and address arithmetic,
and the access. Padded reads cost at most eight and loop advance costs at most
three, including its continuation test and branch. Per position, clear costs
at most eight, copy 16, trim 10, one-bit shift 18, equal-length descending
compare 17, padded compare 23 and a direct add or borrow/subtract pass 34.

The schoolbook MAC shares the checked result address across its read and
write. Indexing, factor reads, result access, arithmetic and loop control
take at most `1+10+6+10+3=30` units. Its u128 product plus old digit and
carry is at most `2^128−1`. Clear each product result before reuse and count
its final scan and row carry. For opposite signs, compare magnitudes before
subtracting in place and trimming, within `(17+34+10)w` plus fixed guards.
Widths are those verified for each call.
Compare the signed KKT sides directly. Each restoring round shifts R, compares
it to the divisor in descending order through the low limb when needed,
conditionally subtracts, updates the quotient bit and trims. Each GCD round
checks parity, zero and length, then halves one or both even values or orders,
subtracts and halves the larger odd value. Each nonterminal round decreases the
combined bit length. Charge the shared-factor counter and trim in the round
and restore the common power of two once at the end. A shift, padded compare,
subtract and trim cost at most `(18+23+34+10)w`. The two-shift GCD path costs
at most `2(18+10)w`. Both round witnesses fit `85w+128`, including fixed
controls. The former division rate `16w+8` fails on the full-width narrow
shift, compare and subtract path.

| Lane | Selected envelope | Units |
| --- | --- | ---: |
| 138 schoolbook products | 32 × 5,163,588 MACs + 16 × (55,565 init + 55,565 scan + 46,206 row carries) + 512 × 138 | 167,822,848 |
| 93 signed add/sub calls | 64 × 40,779 positions + 512 × 93 | 2,657,472 |
| 11 ordered comparisons | 24 × 12,277 positions + 256 × 11 | 297,464 |
| Restoring division | `D(w)=128w+1024`, 331,776 rounds at 65 limbs and 149,164 at 1,162 | 25,438,915,584 |
| Binary GCD | `G(w)=192w+1024`, 73,728 rounds at 64 limbs and 149,164 at 1,170 | 34,642,412,032 |
| **Arithmetic subtotal** | | **60,252,105,400** |

**B-only setup and terminal work.** Count the bounded loop bodies and their
fixed guards as follows. Clears and copies cover the actual selected reuse
passes. The wide shift rows include output word/bit addressing, at most two
padded reads, writes and trim. Terminal matching extracts computed bytes
directly and counts each stored byte read and comparison.

| Work | Counted span and fixed charges | Units |
| --- | --- | ---: |
| R43 framing, checked spans and canonicality | 10 pairs × (2 × 64 + 32) | 1,600 |
| Nine weight decodes | 9,216 bytes × 16 + 1,152 words × 24 | 175,104 |
| 18 binary32 lifts | 18 × 64, with at most two writes per lifted coordinate | 1,152 |
| Named slot metadata | 82 × 32 + 32 | 2,656 |
| Arena, quotient/remainder, sum and T clears | 8 × (16,024 + 10,425 + 577 + 9 × 594) + 16 × 33 | 259,504 |
| D seed/copy-back and narrow/wide GCD copies | 16 × (2,880 + 3,492) + 32 × 29 | 102,880 |
| Quotient trim and exact zero remainder | 10 × (7,516 + 2,909) + 32 × 22 | 104,954 |
| Numerator and reduced result bit lengths | 13 × (64 × 4 + 32) | 3,744 |
| Threshold, Δ and common-factor shifts | 3 × (1,162 × 48 + 64) | 167,520 |
| Stored quotient equality | 1,024 bytes × 24 + 2 × 32 | 24,640 |
| Narrow terminal GCD checks | 9 × 24 | 216 |
| Outer entry/dispatch | 263 arithmetic calls × 64 | 16,832 |
| B record allowance and exit scalars | 3 × 32 | 96 |
| **Tail subtotal** | | **860,898** |

The 10,425 division clear positions are 9 × (576 + 65), (1,170 + 1,162)
and (1,162 + 1,162). The terminal checks visit 7,516 quotient positions and
2,909 remainder positions. Each E_i is trimmed before applying its 512-limb
multiplication bound. Lift exactly the 18 packed components into the named
slots. The 27 cross numerators are the 54 products and 27 differences
already charged in the arithmetic lanes. Keep other width, sign, length and
cursor checks in their priced loops and scalar groups.

**Cancellation and total.** The core is `C = 60,252,966,298`. One carried
counter counts every core operation, including scalar decode and byte
comparison, and costs at most five units per tick (load, decrement, store,
test, branch). Check after each completed 256 core operations and force checks
at B entry, after each of ten pair records, and at the common normal or early
exit. Reset only at a check. Carry the count across adjacent passes and inside
each MAC. Hold the cursor, operands, carry, borrow, quotient/remainder and GCD
state through the check without replay. Counting all core operations also bounds
each interval to 256 actual limb primitives.

The tick charge is at most 301,264,831,490. Full chunks and up to 12
forced checks give at most `floor(C/256)+12 = 235,363,161` polls.
Price each at up to 16 units for invocation, Acquire load, test, branch,
return and reset. Polls cost at most 3,765,810,576 units. Charge counter
and poll overhead separately from the core counter.

```text
W_B <= C + 5C + 16*(floor(C/256)+12) = 365,283,608,364
```

This bound fits `floor(u64::MAX / 365,283,608,364) = 50,499,786` isolated
B proofs. A future verifier revision must substantiate these exact pass,
state and charge bounds or reopen the paper proof.
[Work and cancellation](#84-work-and-cancellation-bounds)
· [Polling ledger](#812-exact-work-ledger-stored-d²-reducedness-and-polling)
· [Worker cancellation owner](../../src/three_d/geometry_cpu.rs#L41-L47)
· [Legacy prepare checkpoint](../../src/three_d/geometry_cpu.rs#L248-L267)

**Request boundary and open gates.** Before scanning the roster, bound
`t*max(W_L,W_Z,W_N)` plus source, binding, successor and other cancellation
work. After complete verified association, use the checked class partition.
Apply checked u64 arithmetic and the explicit nonzero caller
`maximum_exact_verification_work` at both gates. Overflow or excess yields
typed refusal before dependent costly proof. Charge B's counter and polls
once inside W_N, leaving W_cancel for other lanes. The isolated B bound gives
no numerical outer default or Device limit.

`W_N = W_frame + W_A + W_C + W_B` and full request work remain open.
F/U/H, A, C, normal-role and Z/Legacy proof, typed render and source binding,
both predecessor versions, owner-proved Layout and peak/escrow receipts,
qualified producer capture, producer schema2 and coordinated adoption need
separate closure. This proposal follows R025 original casters, R028's full
`1/510` normal domain and R027 last-good and Queue custody. All backend
claims require the same exact semantics with independent conformance evidence.
Binding §§3–4 retain closed final-Context source conformance. All Z/N
appendices remain inactive. **KOMPLEMENT 902.779.61 remains NOT ADMITTED.**
[Preflight ceilings](#87-pre-prepare-source-limits-and-charged-layouts)
· [Build profile and conformance](#815-verified-std-profile-for-owner-requested-arc-layout)
· [HAL355 backend status](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#142-authority-implementation-and-backend-status)

### 8.17 Private single-Arc checked initialization candidate

**Proposed / Inactive / Non-binding.** JC retains final product authority and veto. §8.15 retains its historical
no-unsafe selection. This subsequent candidate records a private unsafe
boundary for future review in principle. It grants no binding S1 adoption,
S2, or authority to write or execute unsafe code. Binding §§1–5 and the
accepted §4 final-Context conformance retain their scope.

The existing HAL CPU preparation and certificate owner would build each new
additive HAL-owned immutable array directly in one
`Arc<[MaybeUninit<T>]>` allocated by `Arc::<[T]>::new_uninit_slice(n)` for
the checked length. The current per-item checkpoints precede the four
Legacy Vec freezes, and installed std copies each Vec
into a distinct Arc allocation without a checkpoint. The direct
exact-length `Arc::from_iter` path needs every item. Fallible collection
through `Result` can use a Vec and the same bulk freeze. The selected
additive owner uses neither freeze nor unwind as normal cancellation.
The Legacy converter, reader, checked frames, public APIs and refusal
semantics remain unchanged. Both paths retain the shared numerical verifier.
[Original and certificate freezes](../../src/three_d/material_api.rs#L458-L543)
· [Typed render and checked frames](../../src/three_d/geometry.rs#L1581-L1649)
· [Worker checkpoint](../../src/three_d/geometry_cpu.rs#L248-L267)

Before dependent allocation, HAL verifies exact cardinality and source
ownership, checks length and Layout arithmetic, and proves the effective
caller byte and work quotas against the phase live sets. The creating HAL
owner verifies the request for the actual target/std allocator profile,
including element stride, alignment, Arc header and padding. Producer data
and external receipts confer no HAL allocation authority. The installed
Rust 1.99.1 macOS 64-bit Global witness in §8.15 is conditional. Each
conformance claim needs independent profile evidence. Capacity, budget and
overflow refusals stay typed. `Arc::new_uninit_slice` offers no stable
recoverable physical-OOM result. CPU physical-OOM policy remains open.
[Requested Layout and profile](#815-verified-std-profile-for-owner-requested-arc-layout)

Keep one strong owner with no `Weak` handles or escaped references during
initialization. Use safe `Arc::get_mut` and `MaybeUninit::write` on the next
checked slot. Run bounded work and cancellation checks and validate each
complete typed value before its write. Advance the initialized prefix
`i` only after the write finishes. Preserve exact source and certificate
field bits, including signed zero, and exact element order. Retain all
source age, generation and predecessor checks. Never read an unwritten
slot or publish a partial array. Require `i == n` and a final successful
cancellation check before the one privately audited `Arc::assume_init` site seals the same
backing once, without a copy or allocation. No type punning, unchecked
pointer arithmetic, additional unsafe construction primitive, public unsafe
interface, allocator hook, global owner, per-item Arc, second retained proof
graph, new scheduler or UI mutation is selected.

Cancellation or source refusal drops the unsealed Arc and returns the
existing typed error. An unwind out of initialization also drops unsealed
storage and yields no successful output. This cleanup is eligible only for
concrete element types with a compile-time check of `!needs_drop::<T>()`,
including transitive fields. Current `PbrOriginalVertex3D`,
`PbrVertexSource3D` and `PbrCheckedFrame3D` have inline value fields and no
element destructors.
`PbrTriangleCertificate3D` has the same no-destructor property and belongs
only to Legacy. No Legacy certificate array is created for additive Z/N.
A destructor-bearing type requires a separately designed and authorized
initialized-prefix guard. No panic-as-cancellation or `catch_unwind` fallback.
[Original element](../../src/three_d/geometry.rs#L568-L574)
· [Typed render element](../../src/three_d/geometry.rs#L245-L252)
· [Certificate and frame elements](../../src/three_d/geometry.rs#L644-L674)

Installed Rust source anchors are `library/alloc/src/sync.rs:1285–1291`,
`1565–1599`, `2184–2193`, `2303–2345`, `2646–2683`, `4072–4095`
and `4162–4243`, `library/core/src/mem/maybe_uninit.rs:214–244`,
`388–394` and `571–577`, and `library/core/src/iter/adapters/mod.rs:144–205`.
The stable allocation, unique access and write APIs leave the typed seal
unsafe. The seal transfers the Arc pointer and allocator in the inspected
stdlib. The wrapper and shared geometry certificate are published only
after their existing full checks.

Charge the one new Arc backing per HAL-generated role instead of a
nonexistent Vec/freeze overlap for that role. Under §8.13 charge all still-live
input Arcs, packed intermediates, other roles, partial-init state, local
scratch and fixed wrappers in their actual phases. The flat proof bytes
and fixed roster each keep their own single Arc. Use the roster's actual
Rust element stride for Layout, with 56 bytes the canonical encoded record.
Preflight and success bind the same owner-verified input identities and
requested Layout formula. Record each new backing's identity at allocation
and preserve it through seal and receipt. Enforce `A_j <= P_j`,
`I_j + P_j <= M` and `B + R + P_j <= M`. Pending, predecessor and
registration escrow remain independent Context charges. Legacy receipts
remain unchanged.
[Proof and roster](#89-canonical-two-arc-private-proof-grammar)
· [Phase and escrow charges](#813-proposed--inactive--non-binding--requested-layout-and-peakescrow-phase-obligations)

The existing worker owns bounded per-item and long scalar proof/copy
checkpoints and their priced visits. No opaque source-sized copy, hidden
conversion pass or uncharged temporary may follow the last poll.
B's 256 counted limb operations and the successor's 256 compared/copied
bytes remain separate limits. Fixed wrappers have separate fixed charges.
The caller cancellation token retains its existing ownership and clone
behavior. [Polling ledger](#812-exact-work-ledger-stored-d²-reducedness-and-polling)

Before any future conformance claim, prove accepted and refused Legacy
A/B/C/NormalOnly parity through the shared kernel and exact N/Z signed-zero
and canonical frame results. Cover U16, U32, unindexed, zero-length
and overflow cases, and cancellation at zero, one, mid-array, `n - 1`
and `n` completed writes and before sealing. Prove unique ownership and
cleanup on source refusal or unwind, with no partial publication,
uninitialized reads, double-free or retained extra backings. Verify
one-use source/predecessor association and matching preflight, success
and Context escrow receipts.
Any separate caller charge through final Arc release or beyond that
release needs independent owner and lifetime evidence. Block on any
unproved soundness or profile obligation.

F/U/H, A, C, Z, source role and whole-request work, peak/escrow receipts,
qualified producer capture, producer schema2, independent build-profile
conformance and coordinated adoption remain open. R025 original casters,
R028 full `1/510` normal proof, and R027 last-good and Queue custody
remain in force. **KOMPLEMENT 902.779.61 remains NOT ADMITTED.**
