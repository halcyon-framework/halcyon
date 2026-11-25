# SPEC-HAL-177A: Deferred Intent Delivery and Single Consumption

**Status:** Binding Addendum
**Scope:** Existing private deferred intent queue on TabularSubstrate
**Extends:** SPEC-HAL-177
**Integrates with:** SPEC-HAL-174, SPEC-HAL-174A
**Supersedes:** SPEC-HAL-177 queue-entry edit gating, execution-time debug-panic/release-ignore rule, INVARIANT-QUEUE-4, and their corresponding closure claims.

## 1. Authority

The maintainer's decision of 2026-11-24 resolves the authority conflict
recorded in [C17](../guide/audits/contradictions.md#c17--spec-hal-177-deferred-queue-versus-spec-hal-174-edit-authority):
delivery mode must not change the governing intent semantics; a consumed
queue entry must not be returned for execution.

This addendum preserves the existing direct execution contract. It does not
amend edit lifecycle, comparator deferral, provider-owned sorting, row identity,
or column interaction semantics.

## 2. Admission and execution

- Private `emit_intent` remains the sole producer of `pending_intents`.
  It accepts every existing `TabularIntent` variant, including during an
  active edit. Admission does not execute the intent or cancel the edit.
- Preserve the existing admission callback, invoked once for each emission
  before pushing the intent. It is notification, not execution authority.
- `handle_intent` remains the sole execution authority for this substrate.
  It applies existing rules against the state at execution, identically
  for direct and deferred delivery. The existing HAL-174 Sort and Focus
  routes cancel active editing before applying their operation; Resize
  does not cancel editing. Preserve the flat-substrate Tree no-op and the
  separate hierarchical owner.
- No queue-specific drop, cancellation, debug panic, release ignore, or
  new intent handling is introduced by delivery mode.

## 3. Consumption

- At the existing layout boundary, `process_pending_intents` takes the
  pending batch and passes each entry in FIFO order to `handle_intent`
  once. Consumed entries must not be restored to the queue. Each queue
  entry, not each distinct value, is the unit of consumption.
- `take_intents` remains a public pure, destructive drain of the currently
  pending entries, preserving FIFO order. It neither executes them nor
  returns a history of executed entries. Entries drained by this method
  are no longer pending for layout processing.
- The existing callback remains the notification route. No second queue,
  executed-intent history, implicit replay, deduplication or coalescing
  is introduced.

## 4. Boundaries

All other HAL-177 invariants and non-goals remain binding. The private
entry remains intentional dormant infrastructure; no production caller,
visibility expansion, or new public API is required.

HAL-174A and HAL-171 retain their comparator-specific rules. In particular,
this addendum does not govern SPEC-HAL-188-backed ListView sorting: the
provider-authoritative contract in SPEC-HAL-353 section 8 remains unchanged.
Provider order invalidation is not a queued legacy Sort intent.

## 5. Verification

Exercise admission during an active edit, no early execution, FIFO and
single consumption at processing, unchanged admission callback and pure
drain, and the existing direct edit/sort/focus/resize behavior. Retain
the existing tests. No production activation or broader conformance
claim follows from these focused checks.

**Implementation verified (2026-11-24, Halcyon 0.270.3-beta).**
The existing queue processor no longer restores consumed entries. The direct
execution and admission callback paths remain unchanged. The private entry
remains dormant. The selected library tests (12) and edit lifecycle and
focus integration tests (29) passed. Native Showcase, WASM Web Showcase,
and iOS Simulator library compilation/check completed without warnings.
This verification does not qualify any new production queue caller.
