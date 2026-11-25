# SPEC-HAL-361: App Frame Realization Observation

**Author:** JC
**Status:** Adopted / Binding
**Date:** 2026-09-15
**Owner:** Core / canonical application frame orchestration
**Runtime scope:** `App::run`
**Extends:** SPEC-HAL-266A, SPEC-HAL-354, SPEC-HAL-360
**Depends on:** SPEC-HAL-002, SPEC-HAL-084, SPEC-HAL-163, SPEC-HAL-295A, SPEC-HAL-359
**Adoption authority:** Delegated architectural adoption on 2026-09-15,
following the source audit.

## Pre-Flight

- Objective: associate an ordinary application's custom-render preparation with
  the exact surface text preparation and subsequent output of canonical `App::run`.
- Mandatory startup and authority entry chain completed: `AGENTS.md`,
  `PRE-FLIGHT.md`, `docs/ENGINEERING.md`, `PROJECT-ENTRY.md`, Constitution 002 and
  amendment guidelines 002A; governing 084, 163, 266A, 295A, 354, 355, 359, 360;
  relevant 018, 161, 162, 317 and 317A observation/lifecycle contracts inspected.
- Governing authority and status: this adopted core/application contract;
  inherited contracts retain their exact statuses. 018, 317 and 317A remain Draft.
- Authority change: one additive application observation contract; no renderer,
  widget-interaction, agent-channel or consumer-domain authority transfer.
- Target layer and existing state verified: `core/app.rs` canonical preparation,
  compile, render and submit/present sequence at `e946ae95`.
- Owned mechanisms and files: App configuration/context, a core receipt module,
  attempt identity in existing custom-preparation metrics, focused tests, this
  SPEC, manifest/lockfile and changelog.
- Named invariants: ATTEMPT, EXACT-SURFACE, PREPARATION, OUTPUT, PUBLICATION,
  LATEST, ABSENCE and AUTHORITY below.
- Forbidden behavior: false output/completion claims, cross-attempt association,
  private binding exposure, renderer-phase callbacks, semantic mutation and
  unbounded engine receipt history.
- Existing mechanisms inspected and reuse decision: retain HAL-360 text results,
  compositor ordering, HAL-266A participant dispatch, and existing renderer
  submission. Draft `FrameRendered` is insufficient and remains unchanged.
- Relevant external boundaries: applications retain their own candidate meaning;
  Halcyon supplies only mechanical immutable association.
- Test ownership and placement: core receipt tests, existing custom preparation
  and HAL-360 renderer tests, an external ordinary `App::run` native probe.
- Exact verification through Grip: the check action in
  `verify package halcyon test frame_realization check`, then
  `verify package halcyon test frame_realization`,
  `verify package halcyon test custom_render_preparation`,
  `verify package halcyon test complete_text_realization`, followed by
  `verify package halcyon test`. The native probe separately proves the
  canonical public consumer path. Results and limitations are recorded below.
- Completeness boundary: public immutable receipts and candidate association,
  canonical wiring, bounded retention, focused proof and an owned verified commit.
- SemVer/changelog: public capability and preparation-metrics field require
  `0.199.0-beta`, synchronized manifest/lockfile and newest changelog entry.
- Git/external effects: exact owned commit; no push, reset or clean.
- Out of scope: HAL-360's nine accepted aggregate failures, HAL-040 recovery,
  staging infrastructure, draft runtime and agent contracts.

## Halcyon Pre-Flight

- Governing SPEC and read status: authority chain above completed before edits.
- Widget policy: N/A, generic core observation and existing preparation capability.
- Interaction added and policy authority: none.
- Applicable FORBID rules: HAL-002 phase/ownership law, HAL-295A renderer privacy,
  HAL-266A semantic-mutation and target-isolation prohibitions, HAL-354 cross-phase
  mutation/retention prohibitions, HAL-360 preparation/output distinction.
- Existing framework mechanism inspected: `ScreenFrameContext`, `AppContext`,
  `Screen::flush_pending_actions`, `CustomRenderTargetMetrics`, private preparation
  bindings, `MultiTargetDrawPlan`, `render_multi`, `submit_and_present`.
- Reinvention risk: reuse those owners; add neither a scheduler nor an event bus.
- Demo/example compliance: generic proof only; no Escape handling or new interaction.

## 1. Attempt Identity

**INV-361-ATTEMPT:** Canonical App orchestration mints one opaque
`AppFrameAttemptId` at the start of each render attempt before custom preparation.
Its sequence is strictly increasing and cannot wrap. It is scoped to one
application run, distinct from renderer `FrameEpoch`. Different application runs
cannot yield equal attempt identities. Identity exhaustion fails before reuse.

The ID accompanies the existing immutable per-target preparation metrics. A
consumer may retain `{attempt_id, SurfaceId}` beside its own candidate. This
neither changes the candidate nor certifies publication, Compile or output.
Noncanonical preparation has no App attempt ID; it cannot fabricate one.

A candidate associated with attempt N matches only a receipt for N and its exact
SurfaceId. N-1 and N+1 cannot certify it. Sequence gaps disclose skipped
observations; they do not identify why the consumer missed an attempt.

Renderer-wide Compile identity remains private. Direct renderer users retain
HAL-360's APIs and its same-epoch supersession checks unchanged.

**HAL385 narrow owner decision — 2026-11-17:** browser allocation qualification
adds an opaque `RendererPreparationIdentity` to the separate Compile Pending
outcome. It exposes no GPU handle, graph, pool entry or command custody. App
may publish a Pending snapshot with no text/output success, then a final snapshot
for the same attempt when that exact preparation resumes. Previously retained
snapshots stay immutable; latest-only observation and opt-in allocation remain
unchanged. Pending, NoTargets and Failed are distinct. Supersession/cancellation
cannot turn the old Pending receipt into output evidence for another attempt.

## 2. Immutable Receipt

**INV-361-EXACT-SURFACE:** Each snapshot contains its exact attempt ID and one
record per surface in the attempt's compositor order. Each record contains:

- the exact `SurfaceId`;
- an optional copy of the compiled plan's HAL-360 `SurfaceTextRealization`;
- `output_produced`;
- `submitted`; and
- `presented_called`.

The snapshot explicitly distinguishes successful Compile from no targets,
Compile failure, unchanged scene and deferred layout. A skipped or unavailable
Compile has no text result and all output flags false. A surface with a valid
empty scene may have HAL-360 Complete with zero counts.

**INV-361-PREPARATION:** A present text result is copied from the exact compiled
plan before it is consumed. Its FrameEpoch and typed Complete/Unavailable result
are preserved. It grants no persistent glyph/resource validity and no GPU or
display completion guarantee.

**INV-361-OUTPUT:** `output_produced` is true only for an actual output returned
by `render_multi` for that surface. `submitted` means the existing Queue
submission was issued for that output. `presented_called` means that output's
`SurfaceTexture::present` call was issued. Neither flag claims GPU completion or
display completion. Missing/incomplete text cannot coexist with true output
flags. A Complete surface that fails acquisition or encoding retains its
preparation result with all output flags false. Surfaces remain independent.

Headless Compile evidence has no surface output, submit or present claim. This
contract does not admit headless surface acquisition or create a new renderer.

## 3. Publication and Observation

**INV-361-PUBLICATION:** Publish only after all returned outputs have passed
through the existing submit/present path and after the frame critical section
has ended. Deferred/unchanged attempts publish their explicit skip outcome.
Consumers observe through ordinary application callbacks/build boundaries or
their read-only observer handle. Publication never invokes widgets, re-enters
layout, changes application/domain state or triggers a new frame.

**INV-361-LATEST:** Observation is opt-in through `AppConfig` and provides a
cloneable latest-snapshot observer, also readable through `AppContext`. Startup
has no receipt. Each publication atomically replaces the prior immutable
snapshot. There is no event history, queue, acknowledgement or delivery guarantee
for intermediate attempts. The engine retains only its latest snapshot; a
consumer may explicitly retain immutable copies under its own bounded ownership.

**INV-361-ABSENCE:** Without an observer, the engine constructs no receipt vector
or shared observation storage. Attempt identity still accompanies canonical
custom preparation. No custom participant traversal or metrics construction is
added when the HAL-266A participant set is empty.

**INV-361-AUTHORITY:** The private target-bound presentation table, renderer
preparation identity, DrawPlan construction and surface selection remain private.
No new callback, semantic payload, persistent layout/glyph reference, domain
type, GPU handle, renderer ownership or private consumer access is introduced.
Draft agent `FrameRendered` retains its existing monitoring behavior and is not
an authority for this receipt.

**Proposed relationship to SPEC-HAL-363/364 (2026-10-05):** The paired
documents remain Proposed pending joint adoption. This annotation changes no
declared status, implemented scope or substantive law in this document.

SPEC-HAL-364 internal bounded group outcomes are separate from this opt-in,
latest-only public observer. Disabling observation does not disable internal
lifecycle reconciliation. Real AppFrameAttemptId, per-surface output/submission/
present-call meanings and post-execution publication remain unchanged. No new
receipt queue, callback, semantic re-entry, physical-display guarantee or
cross-surface atomicity is implied. Both inactive capabilities add no observer
or capture work.

## 4. Required Proof

1. Startup absence, opt-in access through AppConfig/AppContext and cloned handles.
2. Exact attempt delivered before preparation and matched only with that
   attempt/surface; later attempts and application runs cannot match old receipts.
3. Real HAL-360 Complete, typed Unavailable and empty-scene outcomes survive
   copying, with deterministic independent multi-surface records.
4. Actual canonical public `App::run` consumer observes native successful output
   only after submit/present, and incomplete text reports no output while another
   complete native surface still submits.
5. Complete preparation with no output remains Complete with all output flags
   false. Headless proof is explicitly Compile-only.
6. Compile skip/failure has no text-success claim; latest overwrite retains no
   internal history, held snapshots remain immutable, and sequence gaps are
   observable. No renderer callback into widgets or receipt-driven frame loop.
7. Existing custom-preparation and HAL-360 supersession proofs remain valid.

## 5. Implementation Evidence

Recorded on 2026-09-15 against `e946ae95`, with the `0.199.0-beta`
implementation applied, on macOS. The check action passed with zero errors and
55 warnings. Focused test results through Grip:

| Command | Result |
| --- | --- |
| `verify package halcyon test frame_realization` | 5 passed, 0 failed |
| `verify package halcyon test custom_render_preparation` | 15 passed, 0 failed |
| `verify package halcyon test complete_text_realization` | 6 passed, 0 failed |

These groups overlap. The new proofs cover startup absence, optional context
access, cloned observers, distinct application-run IDs, skipped attempt IDs,
immutable retained snapshots, release of overwritten engine history and explicit
Compile-unavailable outcomes. The retained-scene test observes two exact attempt
IDs at preparation while preserving one allocation and avoiding a second
layout/paint or visual invalidation.

The real renderer test copies Complete, typed Unavailable and empty-scene
Complete results in compositor order with their exact SurfaceId and FrameEpoch.
A subsequent Compile supersedes that plan; `render_multi` returns no outputs,
and the receipt retains its original preparation evidence with all output flags
false. This is headless Compile and pre-acquisition refusal evidence. It does
not simulate native acquisition failure, GPU completion or presentation.

### 5.1 Public native application proof

`tests/app_frame_realization_native.rs` is a separate integration executable
using public application, widget and custom-render APIs. It builds in ordinary
test runs but opens native windows only when invoked with `--run-native`.
Its ordinary `Screen::flush_pending_actions` callback reads both AppContext and
the configured observer handle, checks that they expose the same immutable
snapshot, and matches the candidate's exact preparation attempt and surface.

The final executable was built through Grip with
`verify package halcyon test app_frame_realization_native timeout 900 detach-after 1`.
That filtered invocation executes zero ordinary tests and explicitly skips the
native probe; it is build evidence only. The following command was then executed
from the repository root and exited successfully:

```sh
python3 -c 'import subprocess, sys; sys.exit(subprocess.run(["target/debug/deps/app_frame_realization_native-72c09ea454037279", "--run-native"], timeout=45).returncode)'
```

| Candidate | Attempt sequence | Primary result and output | Second surface |
| --- | ---: | --- | --- |
| 0 | 1 | Complete; output/submit/present-call true | Complete; all three true |
| 1 | 2 | MissingGlyph; output/submit/present-call false | Complete; all three true |
| 2 | 3 | Complete; output/submit/present-call true | Complete; all three true |

The probe retains the first snapshot and verifies that later attempts cannot
match it and do not change it. The canonical publication assertion requires
the frame critical section to have ended. The probe certifies submit/present
calls for these native attempts; it does not certify GPU/display completion,
other platforms or HAL-040 acquisition-backpressure recovery.

### 5.2 Broader verification and inherited limits

`verify package halcyon test timeout 900 detach-after 1` completed with **2024
passed and nine failed** in the library. The aggregate remains red and the
library failure prevented subsequent integration-test execution in that run.
The failed names and reasons match the inherited record in
SPEC-HAL-360 sections 6.1 and 6.2: four SPEC088 layout-epoch fixtures, two
ListView paint-phase fixtures, two market-detail animation expectations and
the staging high-water assertion. The staging comparison was 45600 versus
32832 in this run. No new failed test was reported.

The HAL-360 source-based debt classification and its missing clean executable
baseline remain unchanged. Neither this native consumer proof nor the focused
receipt tests establish HAL-040 stabilization/recovery. The failing fixtures,
their guard/state owners and staging lifecycle are unchanged.

## 6. Required-text refusal safeguard - 2026-11-13

**Status: Selected / Binding; implementation and native acceptance pending.**
Authority: JC explicitly requested an early safeguard using Halcyon's existing
poison-pill/contract-guard patterns while correcting a consumer application's stale native frame.
This addition preserves HAL002/002A, HAL360 complete-required admission and
sections 1-5 above. It adds no font fallback or degraded rendering policy.

The native consumer-reproduction evidence contains terminal TextBlock mutations followed by
MissingGlyph(primitive 14, glyph 0), with no output/submission/present. The
capture overflow prevents complete-run acceptance but does not invalidate the
individual recorded refusal. A later SceneUnchanged receipt is not recovery.

### 6.1 Normal-path owner and behavior

INV-361-TEXT-FAULT-EXPLICIT: canonical App::run must report the FIRST actual
HAL360 Unavailable outcome for each surface on the ordinary execution path,
even without an AppFrameObserver, tracing subscriber or diagnostic switch.
Use the original compiled SurfaceTextRealization and actual attempt identity;
do not reshape, infer a cause, invent an outcome or invoke a consumer callback.

Use one private App-run-owned guard beside frame orchestration. It keeps at most
one unresolved fault per currently live surface, allocated only on failure:
first failed attempt/FrameEpoch, SurfaceId and the original typed failure.
No strings, source text, glyph runs, GPU handles, widget references, global
state, receipt history, new thread, timer, retry or redraw demand are stored.

INV-361-TEXT-FAULT-RETAINED: skipped/SceneUnchanged/LayoutDeferred attempts and
repeated identical refusals do not clear the unresolved fault. A changed typed
failure replaces its diagnostic signature. Recovery is recorded only after
Complete preparation AND successful original submit/present calls for that
same live surface, or retirement removes that surface. This diagnostic state
grants no resource validity and never authorizes reuse of an earlier frame.

Emit one bounded, content-free stderr diagnostic on first refusal, on a changed
failure signature and on actual recovery. Include the invariant name, exact
attempt/surface/epoch and primitive/glyph indices/reason. Emit after leaving the
frame critical section. Never print every frame or pointer event; identical
repeats need no formatted output or accumulated log buffer. Healthy frames
create no guard history or diagnostic allocation. Existing optional latest-only
observer semantics and INV-361-ABSENCE remain unchanged: this private fault
state is not an always-on receipt vector or shared observer.

INV-361-TEXT-FAULT-NO-FALSE-OUTPUT: preserve the existing runtime assertion
pattern in AppFrameRecorder::output_produced/submitted_and_presented. Make
attempt-local enforcement independent of optional observation at the actual
output/submission boundary: an output for a surface refused in that same
Compile is a programming invariant violation and must fail immediately.
Expected MissingGlyph/other typed Unavailable alone is NOT a panic condition.
Other complete surfaces continue normally. No whole-process termination,
Unicode stripping, fallback substitution or relaxed HAL360 admission is selected.

### 6.2 Proof and original product boundary

Extend the existing frame-realization and complete-text-realization tests.
A genuine failed text Compile must cause one first-fault diagnostic without an
observer; unchanged attempts and identical repeated failures retain that fault
without log spam; actual same-surface Complete+submit/present clears it.
A different healthy surface remains independent. A deliberately contradictory
same-attempt success must trigger the named invariant guard. No test may
manufacture a production model result or classify unavailable text as Complete.

The safeguard is not itself the glyph repair. For the original consumer failure,
identify the first failing source cluster and the actual admitted shaping face
before selecting a text/font behavior correction. Bounded private diagnosis may
record only that cluster's scalar values (maximum eight), byte range, mechanical
producer identity and actual font binding; never prompt/answer bodies. Use a
minimal nonpersonal reproducer through the same TextBlock/Compile path.
Preserve all original Unicode bytes. An unsupported-font policy is not an
invitation to activate Draft font fallback. Root-policy changes remain with
the supervising architectural owner; existing accepted behavior stays binding.

Acceptance returns to the original application reproduction with two genuine
sequential questions, the original resident model configuration and unchanged
rendering. Validate matching native receipts and app-only pixels. A useful
first-fault record or a headless test alone does not close the still-open physical
product failure.

### 6.3 Exact source attribution without a parallel provenance system - 2026-11-13

The accepted HAL048 diagnostic reuse permits the existing crate-private
`glyph_source_ranges` verifier at the first current MissingGlyph boundary.
BackendTextEngine must supply the actual admitted shaper for the failed
GlyphRun binding. All existing equality checks must pass before using the
failed glyph index's returned byte range. This diagnostic qualification never
changes the original HAL360 outcome and is distinct from ordinary first-fault
reporting, which must not reshape.

Capture current cluster/font evidence directly before SurfaceTextDemand::gather
discards its failing local run. No public FlatPrimitive/TextNode schema change,
global provenance registry or persistent text/glyph buffer is selected.
A widget or concrete mount may be named only through a direct same-attempt
owner witness; matching text, numerical indices or another process cannot
supply that association. Lack of an old producer map is not a reason to stop
the already-selected safeguard or current first-failure capture.

## 7. Missing-cluster containment observation - 2026-11-13

**Status: SELECTED / BINDING, coordinated with HAL360 section7.** JC selected
framework-owned handling without requiring an application error callback. The existing permanent
guard is retained; its text-fault state must distinguish output refusal from a
renderable but explicitly Degraded missing-cluster result.

A current Degraded outcome records one unresolved MissingGlyph signature and
its missing_cluster_count, and is eligible for ordinary output only after its
real marker and remaining content preparation pass. It is NOT Complete,
NOT Unavailable and NOT recovery. Preserve the actual variant in observer
receipts; do not fabricate a successful text result. Extend the normal first/
changed diagnostic with the actual disposition and count, never source text.

INV-361-TEXT-FAULT-NO-FALSE-OUTPUT continues to forbid output/submission for
Unavailable in the same Compile. It must also reject a claimed Degraded output
without its actual prepared markers. It must not reject an honestly qualified
HAL360 section7 Degraded surface merely because the text fault is unresolved.
Skipped/deferred/unchanged frames retain fault state. Only Complete plus actual
same-surface submit/present, or retirement, closes it; marker presentation is
not silent recovery. No repeated identical diagnostics or extra frame demand.

This expressly reconciles section6's earlier MissingGlyph all-surface-refusal
assumption; all strict failures other than the new qualified missing-cluster
case keep their original behavior. Preserve observer absence and allocation
bounds. Add direct guard/receipt tests for Degraded presentation and later real
Complete recovery, alongside existing contradiction tests.
