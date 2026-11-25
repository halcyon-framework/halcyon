# SPEC-PM-022: Procedural Material Generation, Eligibility, and Lifetime Conflation

**Author:** JC
**Status:** Proposed pathology record pending Core Architecture review
**Date:** 2026-10-29
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Prevention
**Owner:** Core Architecture
**Audience:** Engine, Renderer, Resource, Widget
**Scope:** Existing admitted procedural Button material production, current-frame publication and generation retirement
**Authority:** Subordinate to SPEC-HAL-002/002A. No supersession, constitutional amendment, new execution admission or owner-SPEC status change.

## 0. Pathology

Treating evidence from one resource-lifecycle stage as proof of another:
generated content as current target eligibility, queued production as admitted
Ready, admission as complete current output, or settled sampling as settled
producer writes.

These are distinct facts. Their conflation caused unnecessary production,
missing Button faces during resize, and an incomplete retirement predicate.
The incidents below are separate causal manifestations of this one
evidence-substitution class.

This record preserves lessons from adopted HAL-012, HAL-266A and HAL-295A.
Its Proposed status does not suspend those binding contracts. It creates no
new producer, cache, phase, capability or default style.

## 1. Incident reconstruction

### 1.1 Window resize with unchanged texture contents

The historical restored-source experiment held F=(20,20,84,30), viewport,
material basis and effective ratio fixed, changing only window extents.
Its light/dark, 1x/2x cases recorded 84 GPU readbacks and 32 immediate
missing-face resize frames. Each case recorded nine successful preparations
including cold preparation; that is not nine observed admissions. The trace
was not a complete registration/submission ledger.

Complete target key K changed although the producer's local pixel inputs did
not. Staged replacement left a frame without a current material Image.
A previous experiment removing window extent from K equality was withdrawn:
equal content cannot make unequal target snapshots equal.

The adopted correction separates immutable content basis C and original
generation admission from fresh eligibility for complete current K.
Scoped unchanged-content reuse retains the original admitted generation,
while authenticating current placement, visibility, attempt and registrar state.

Historical source-bound evidence at revision `793d02a713c8cc6fd41a18ae68f09d12833a7c16`
recorded the missing-face defect, not a demonstrated repair.

### 1.2 Current placement cannot be inherited from original production

The documented source audit established that face size, ratio, finite local
effects and source/endpoint inputs generate texture contents; absolute face
origin determines destination coverage. Current viewport and composition
coverage are not thereby texture-generation inputs.

Relaxing key comparison while retaining the original stored destination would
paint valid content at an old location. Fresh current eligibility must carry
the corresponding current destination and UV mapping while retaining original
generation admission unchanged.

Historical source inspection identified reuse of the stale destination as an
unsafe shortcut. No inspected receipt proves a separate stale-generation
GPU-sampling incident; this record does not infer one.

### 1.3 Real face-size replacement under custody backpressure

Changing face size changes generated content and requires a new complete
Rest/Pressed pair. The later audit found that preparing, admitted and retiring
records all counted toward the existing two-record owner bound. With both
records retained, preparation deferred. App could preserve the active request
with no current Image; Button omitted Surface while other roles/content continued.

Earlier registration alone could not solve that capacity hold. The adopted
correction couples genuine same-attempt admission for the narrow already-admitted
explicit-request F-size replacement with target-local dependent-output Pending.
A blocked replacement produces no fresh target scene; caption-only or stale
material cannot be submitted as successful current replacement output.

Historical failure analysis established the capacity hold and its distinct
writer-completion boundary; see
[HAL-266A §4](SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md#4-boundary-ordering),
lines 311–336. Initial optional activation and out-of-scope changes retain
staged A/B admission.

### 1.4 Sampling settlement omitted producer-write settlement

The same audit found that the original producer/writer tracker reached
generation custody, but retirement consulted CPU/sampling settlement without
consulting the producer's writer-completion predicate. A registered generation
can be never sampled and still have unsettled producer writes.

Removing the staging completion wait without preserving this separate
obligation would make retirement incomplete. Writer submit return establishes
ordering and submitted-pending state, not GPU completion. The adopted correction
retains the same authenticated writer record through reservation, admission and
retirement and requires independent writer, CPU and sampling settlement.

Evidence: the historical writer-completion audit, reinforced by
[HAL-295A §3.3](SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md#33-publication-boundary),
lines 200–240. The established defect is the missing retirement check.
No corruption or device-loss event is inferred.

### 1.5 Equal resource key after a superseded Compile (2026-11-25)

In the controlled Safari resize probe, a replacement was genuinely admitted
for attempt 5, but a focus event superseded Compile before its first output.
The successor retained equal resource K. Staging skipped eligibility renewal,
while the active-size Pending gate correctly required the successor's exact
attempt. The trace recorded 185 actual NoTargets receipts at 574 CSS pixels
and 1148 backing pixels. The writer completion and prior generation retirement
were observed; neither was the missing eligibility predicate.

The correction applies the existing immutable-content and registered-generation
certification when the current active-size gate applies, even at equal K.
The successor receives its own full current metrics and preparation nonce.
The extended existing staging regression preserves the generation and producer
count and rejects the cancelled attempt. The corrected controlled Safari run
has no NoTargets and samples current faces at 768, 574, 390 and 320 CSS pixels.
The controlled interruption delays the genuine WebGPU error-scope Promise;
this evidence does not establish all consumer or platform behavior.

This is another manifestation of INV-PM-022-001 and INV-PM-022-003, not a new
rule. Initial optional A/B activation, producer admission and generation
custody remain governed by the existing adopted contracts. This record retains
its Proposed status.

## 2. Distinct facts

| Fact | Meaning | Insufficient to prove |
| --- | --- | --- |
| C | Exact immutable pixel-generation inputs, including source/ABI, face size, ratio, local effects and endpoint storage/mapping | Current K, admission, G or safe retirement |
| K | Complete current owner/mount, SurfaceId, material basis and immutable target-metrics eligibility | Equality with an older snapshot after a target change |
| G | Actual ImageBindingIdentity: texture_id plus registration generation, issued by the registrar | Current attempt eligibility, completion or lifetime protection by itself |
| Current publication | Authenticated eligibility and matching destination/UV for the consuming preparation | Replay after cancellation or A→B→A merely because K repeats |
| Ready | Complete genuine pair admission and protected current immutable-resource evidence at cutoff | Actual output, GPU completion or native display |
| Custody | Independent writer, CPU-use and sampling-use obligations for the exact generation | Settlement of another obligation |

Missing eligibility is not a fabricated producer failure or binding revocation.
Pending target output is not a third material status.

## 3. Why it reappears

A texture ID, matching pixels, available binding, queued request or passing
diagnostic looks locally sufficient. The missing distinction appears across
preparation, registration, retained Paint, Compile, submission and retirement.
Stable geometry hides missing replacement; readback waits change cadence;
empty sampling custody hides never-sampled writer work.

Eventual material presence, request state and producer counts alone cannot
prove immediate complete output or safe retirement. Additional frames do not
repair a missing publication or custody predicate.

## 4. Prevention invariants derived from adopted owners

### INV-PM-022-001 — Content, key, generation and attempt remain distinct

Full current K retains all fields and equality requirements. Unchanged C may
qualify an already admitted immutable G only under HAL-266A's scoped exception
and fresh resource-owner evidence. Current F including origin, viewport,
physical coverage, clipping and endpoint mapping remain current. Cancelled or
obsolete evidence cannot be replayed when K repeats.

Derivation: HAL-012 §7.1, lines 282–329; HAL-266A §4, lines 359–472;
HAL-295A §3.3, lines 164–198 and 229–240.

### INV-PM-022-002 — Genuine admission precedes dependent commitment

Queued production or source availability is not Ready. Every relevant writer
submit returns on the same baseline queue before swap/publication; the sampled
registered front remains immutable. Genuine registrar admission, exact G,
live owner/mount, full current K and consuming-attempt evidence precede the
existing atomic pre-Paint commit.

Initial optional activation retains staged A/B order. Only the adopted
already-admitted explicit-request F-size replacement admits a fresh complete
pair in the same attempt. It grants no additional source/profile/state/scale,
runtime, caller or default capability.

Derivation: HAL-266A §4, lines 288–357; HAL-295A §3.3, lines 128–162,
222–227, and INV-295A-008, lines 376–403.

### INV-PM-022-003 — Scoped Pending preserves complete output truth

Only within that active-size exception, lawful preparation/admission or
certified retirement/capacity pending at cutoff defers the target's dependent
Paint and fresh output. It cannot publish stale material or caption-only
replacement success. Ordinary input/Update and authorized resource progress continue.

Withdrawal, supersession, unmount, inactivity and genuine owner-supplied failure
retain their adopted gate-ending rules. Genuine Unavailable retains its actual
cause and existing optional material omission with content/focus semantics.
This creates no global readiness barrier, new status or required-resource API.

Derivation: HAL-266A §4, lines 316–326; HAL-012 §7.1, lines 311–328.

### INV-PM-022-004 — Retirement requires independent exact obligations

The original authenticated writer record survives admission and replacement.
Unregister/destruction requires settled submitted writes, no outstanding writer
able to submit (or terminal no-submit cancellation proof), no CPU use able to
submit, and observed completion of all actual sampling submissions. Evidence
covers the actual resources, generation and baseline device/queue.

Submit return, callback registration, elapsed frames, encoding, eligibility
expiry, CPU cancellation or an earlier/unrelated sampling witness cannot
supply missing completion. Never-sampled and submission-uncertain work remain
protected. Uncertainty retains uncertified resources under existing failure
law; unregister precedes final drop. Protected generations cannot be overwritten
or re-registered to satisfy replacement.

Derivation: HAL-295A §3.3, lines 190–255; INV-295A-008, lines 376–403.

### INV-PM-022-005 — Evidence reports its actual stage

Request acceptance, preparation, admission, current commitment, produced output,
submission, completion and native display remain separate claims. A passing
observation that records missing faces is defect evidence. Source-inspected
assertions are not executed-test results. Headless readback does not establish
native continuity or visual acceptance.

Derivation: HAL-012 implementation-admission clauses, lines 21–32 and 682–688;
HAL-266A §9 (Contract and Proof Obligations);
[HAL-309 §2](SPEC-HAL-309-Documentation-System.md#2-authority-model), lines 48–64.
The later G1 stage-truth adoption is not proof that its implementation ran.

## 5. Forbidden patterns

- Omitting/normalizing target fields to equate old and current K.
- Treating C, pixels, texture ID, availability or copied credentials as current
  Ready; relabelling original admission or reviving a revoked G.
- Reusing original destination/clip in place of certified current placement.
- Treating queued/unadmitted production or obsolete-K completion as current admission.
- Extending the same-attempt active-size exception to initial activation or
  another unadmitted capability.
- Submitting caption-only/stale success for scoped pending replacement, or
  converting genuine Unavailable into Pending to conceal its cause.
- Solving the two-record bound with unbounded generations, early unregister,
  overwrite, forced drop, per-frame blocking GPU Wait or invented completion.
- Substituting writer, CPU or sampling evidence for another obligation.
- Splicing late results into assembled scenes, adding a second Update/layout,
  or preparing/polling resources during Paint.
- Claiming native acceptance, universal-default readiness or general performance
  from these scoped diagnostics.

The cited owners' limits remain controlling. Proposed HAL-363/364 and separate
universal-default work remain inactive.

## 6. Regression evidence and limits

The following existing tests were source-inspected, not executed in this
documentation-only task.

| Owner and exact test | Boundary asserted |
| --- | --- |
| [V3 GPU owner](../../src/core/app/sdf_button_v3_gpu_tests.rs), `canonical_button_sdf_v3_resize_resource_trace`, lines 1905–2612 | Unchanged content retains G under fresh unequal full K; real size change gets a new pair; old compiled work retains custody |
| Same owner, `canonical_button_sdf_natural_tooltip_row_resize`, 3611–4551 | Real wrapperless Flex/TooltipHost/child-caption Button with Apply/Rollback peers and current produced-frame face evidence |
| Same owner, `canonical_button_sdf_active_size_pending_preserves_generation_custody`, 5340–5822 | Genuine replacement before Paint; two held records produce no target scene/no extra work; input continues; latest size/revision resumes; independent retirement predicates |
| [Staging owner](../../src/renderer/wgpu_renderer/sdf_material_staging.rs), `sdf_window_eligibility_keeps_full_key_and_content_boundaries`, 1068–1117 | Unequal full K for extent/origin/clip changes, separate source content and current coverage |
| [Reservation owner](../../src/renderer/wgpu_renderer/sdf_image_reservation.rs), `sdf_window_eligibility_requires_fresh_publication_and_original_admission`, from 1345 | Fresh preparation provenance and original authenticated generation |
| Same owner, `sdf_reservation_authenticates_front_and_preserves_generations`, 1062–1275 | Stale-key rejection, protected drain, prohibited re-registration and relabelling |
| [Custody owner](../../src/renderer/sdf_image_generation_custody.rs), `sdf_replacement_writer_cpu_and_late_sampling_retire_independently`, 787–999 | Writer, CPU and actual late sampling independently block retirement |
| [Producer owner](../../src/renderer/custom_render_pass.rs), `sdf_replacement_uncertified_writer_cannot_publish_or_complete`, 850–884 | Uncertain submission proves neither Ready nor safe retirement |

After `ws halcyon`, exact Grip reproduction commands are:

- `verify package halcyon test canonical_button_sdf_v3_resize_resource_trace timeout 600 detach-after 5`
- `verify package halcyon test canonical_button_sdf_natural_tooltip_row_resize timeout 600 detach-after 5`
- `verify package halcyon test canonical_button_sdf_active_size_pending_preserves_generation_custody timeout 600 detach-after 5`
- `verify package halcyon test sdf_window_eligibility_ timeout 600 detach-after 5`
- `verify package halcyon test sdf_reservation_authenticates_front_and_preserves_generations timeout 600 detach-after 5`
- `verify package halcyon test sdf_replacement_ timeout 600 detach-after 5`

Run separately and wait for each terminal using the exact returned job commands.
A detached handle proves admission only; this list claims no passing result.

The 32 missing-face frames are hash-pinned documentary history. The raw
historical trace was not available as a regular file through Grip at this audit.
No corrected trace was re-read or matched to the current source hashes.

GPU fixtures cover selected light/dark 1x/2x cases. Readback waits change cadence;
Pending attempts produce no screenshot. These tests do not establish native
window-drag continuity, dynamic/fractional-scale or disabled-state coverage,
native visual acceptance, or universal-default readiness.

## 7. Source bindings and current enforcement

Inspection date: 2026-10-29. Observed HEAD: `52a638bd`; working tree was already
dirty. Complete-file SHA-256 identifies the inspected source, not HEAD alone:

| Source | SHA-256 |
| --- | --- |
| `src/core/app.rs` | `1cae73c35b6255c3c97159f343a5b0f49819030dba1cd7f3ffca2b6803857913` |
| `src/core/app/sdf_button_v3_gpu_tests.rs` | `b7ebc26f34d3680f611c250b2b78b5d41fea1a2368d7128615bc22bafca5908a` |
| `src/renderer/wgpu_renderer/sdf_material_staging.rs` | `35969865c5982b98f84181af84717dee26eb47a33194a6e650d3bb0932127bed` |
| `src/renderer/wgpu_renderer/sdf_image_reservation.rs` | `6d4fe789b0578d335dbf7432648a9cc01f7a91032273834e815d469bf8e35468` |
| `src/renderer/sdf_image_generation_custody.rs` | `58cb5bf9d46b5d180794e37155c899533a0f0070490052589d625ad1431f8ee9` |
| Content/target packet in §1.1 | `76c044622fe37d7c9e6222e177342907268ddf5947bb77a35201357b31921248` |
| Current-placement packet in §1.2 | `5d92d6d33288f0e4e01414076dc6803d21f7aef5fe50cdbaffc6ccdea3749878` |
| Active-size packet in §1.3 | `f6bc9795425cdb8a92e1a1c7eebc88a3d78cbb08fa7a881b242db30b82781e5c` |
| `SPEC-HAL-012-Theming.md` | `c4ada1058ba3923fe09148a041043e07fb893fe666b9658a2e405b926ded73e3` |
| `SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md` | `d9cf0098ad19a2338acb562187ecb079cc0051f3aa2c83c5894ccc7f6ede6232` |
| `SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md` | `fe115aa0e61225ac8e5e2f3162604f4c36b3072fc7640103c049f4646a357ec4` |

Current source corroborates the correction's structure:
App 2761–2783 orders preparation, genuine replacement admission, Pending,
commit and frozen Paint input; staging `paint_mapping` 485–513 consumes
certified current placement, and admission 854–927 checks exact preparation;
reservation 518–640 authenticates/registers G and transfers the original
producer; custody `retire_if_ready` 179–235 checks independent obligations.
These are source findings, not execution or whole-contract acceptance.

PROJECT-ENTRY lines 713–742 separately records G1 normative adoption with
implementation/conformance NOT EXECUTED. It is not an SDF regression receipt.

## 8. Existing PM coverage

The entire PM-000–021 corpus was read before assigning 022. No existing entry
owns this resource-evidence failure class.

- [PM-006](SPEC-PM-006-Rendering-Pre-Flight-Checklist.md),
  [PM-007](SPEC-PM-007-Clip-Hash-Ordering-Hazard.md) and
  [PM-011](SPEC-PM-011-Overlay-Text-Rendering-Pipeline-Mismatch.md) own Z/pipeline/clip ordering.
- [PM-004](SPEC-PM-004-Geometry-Authoring-Space-Contract-Drift.md) owns transform
  contracts; [PM-005](SPEC-PM-005-ScrollArea-Flex-Constraint-Feedback-Oscillation.md)
  owns constraint/overflow feedback.
- [PM-009](SPEC-PM-009-Dual-Source-Geometry-Drift.md) and
  [PM-010](SPEC-PM-010-Incremental-Cache-Fragility.md) own virtualization geometry/cache classes.
- [PM-012](SPEC-PM-012-Flex-Cross-Axis-Rect-Propagation-Failure.md) records a
  different TimePicker layout/paint width mismatch with unresolved cause.
  This record neither identifies nor resolves that cause.
- [PM-018](SPEC-PM-018-Semantic-Mutation-Without-Retained-Paint-Invalidation.md)
  owns mutation without retained invalidation, independently of resource custody.
- [PM-019](SPEC-PM-019-Terminal-Transition-Entries-as-False-Geometry-Effects.md)
  owns terminal Timeline entries treated as active effects;
  [PM-020](SPEC-PM-020-Focus-Only-Caret-Animation-Demand.md) and
  [PM-021](SPEC-PM-021-Telemetry-Polling-Is-Not-Animation.md) own distinct demand
  evidence failures. Their Proposed status is unchanged.

[PM-000](SPEC-PM-000-Pathology-and-Mistake-Prevention-Series.md) requires this
failure memory. HAL-012/266A/295A retain design/execution authority. This record
changes no functionality, resource bound, API, manifest, lockfile, version,
changelog or unrelated state.
