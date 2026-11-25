# SPEC-HAL-138: Confirmation and Validation Pipeline

**Status:** Design / Binding
**Layer:** Interaction Control Flow (engine-owned)
**Scope:** Explicit confirmation, cancellation, and validation of user-driven intents
**Depends on:** SPEC-HAL-127, SPEC-HAL-128, SPEC-HAL-137
**Constrains:** Overlay system, input routing, action dispatch (no widget-local confirmation logic)

---

## SECTION A — Problem Statement

Halcyon currently supports:

* selection (single, multi, transactional)
* activation
* overlays and input capture
* keyboard and pointer interaction arbitration

What is missing is a **first-class, engine-defined confirmation and validation pipeline** that:

* allows users to stage changes
* optionally validate them
* explicitly confirm, cancel, or abort
* works uniformly across widgets and interaction types
* does **not** leak confirmation logic into widgets

Without this:

* complex selections are fragile (misclick = loss)
* destructive actions lack structural safety
* confirmation is reinvented ad-hoc
* overlays exist but lack governing semantics

---

## SECTION B — Design Principles

**PRINCIPLE-1: Explicit phases, never implicit**
Confirmation must be structurally visible in the pipeline.

**PRINCIPLE-2: Engine-owned, widget-agnostic**
Widgets emit intents; they never own confirmation logic.

**PRINCIPLE-3: Validation is optional and pluggable**
Validation may exist, may not exist, and may be async.

**PRINCIPLE-4: Overlays are presentation, not control**
Overlays visualize state; they do not decide outcomes.

**PRINCIPLE-5: Abort is always possible**
User must be able to exit without committing.

---

## SECTION C — Terminology

**Intent**
A user-generated action request (activate, commit selection, delete, apply).

**Pending Intent**
An intent that has entered the confirmation pipeline but is not yet resolved.

**Validation Step**
Optional logic that inspects intent state and returns allow / deny / modify.

**Confirmation Step**
Explicit user acknowledgment (yes / no / cancel / custom).

**Resolution**
Final outcome: committed, cancelled, or rejected.

---

## SECTION D — Pipeline Phases

### D.1 Intent Emission

Widgets emit an intent into the engine pipeline.

Example intents:

* CommitSelection
* ApplyChanges
* DeleteItems
* ExecuteAction

**INV-CONFIRM-EMIT-1**
Widgets must not show confirmation UI directly.

---

### D.2 Validation Phase (Optional)

Zero or more validators may run.

Validators may:

* accept
* reject
* request user clarification
* transform intent metadata

Validators may be synchronous or asynchronous.

**INV-CONFIRM-VALIDATE-1**
Validation must not mutate widget state directly.

**INV-CONFIRM-VALIDATE-2**
Validation failure must not partially commit.

---

### D.3 Confirmation Phase (Optional)

If required, the engine enters confirmation mode.

Characteristics:

* input capture enforced
* overlay presented (modal or non-modal)
* user explicitly chooses outcome

Supported outcomes:

* Confirm
* Cancel
* Abort
* Custom action (engine-defined)

**INV-CONFIRM-ASK-1**
Confirmation must be explicit user action.

**FORBID-CONFIRM-AUTO-YES**
No implicit confirmation via focus loss, blur, or timeout.

---

### D.4 Resolution Phase

Engine resolves the intent:

* Commit → intent executes
* Cancel → intent discarded
* Abort → pipeline exits without side effects

**INV-CONFIRM-RESOLVE-1**
Resolution is atomic.

**INV-CONFIRM-RESOLVE-2**
No widget state changes before resolution.

---

## SECTION E — Overlay Contract

Overlays used for confirmation must obey:

* SPEC-HAL-127 (overlay stack)
* SPEC-HAL-128 (input capture)

**INV-CONFIRM-OVERLAY-1**
Confirmation overlays capture input exclusively.

**INV-CONFIRM-OVERLAY-2**
Closing overlay without explicit choice equals Cancel.

---

## SECTION F — Integration with Transactional Selection

Transactional selection (SPEC-HAL-136) integrates as follows:

* selection phase → build candidate set
* commit intent emitted
* confirmation pipeline governs final commit

**INV-CONFIRM-SELECT-1**
Selection staging is independent of confirmation.

**INV-CONFIRM-SELECT-2**
Cancellation restores pre-transaction state.

---

## SECTION G — Failure Semantics

### G.1 Validation Failure

* intent rejected
* overlay may display error
* no state committed

### G.2 User Abort

* intent discarded
* transactional state rolled back
* no side effects

### G.3 Engine Abort (panic, teardown)

* pipeline terminates
* no partial commit allowed

---

## SECTION H — Performance Constraints

**INV-CONFIRM-PERF-1**
Confirmation pipeline must not trigger layout or realization.

**INV-CONFIRM-PERF-2**
No polling or per-frame overhead when inactive.

**INV-CONFIRM-PERF-3**
Overlay creation is amortized and reused where possible.

---

## SECTION I — API Surface (Conceptual)

Engine-owned abstractions only:

* PendingIntent
* IntentValidator
* ConfirmationRequest
* ConfirmationResult

Widgets only emit intents.

**FORBID-CONFIRM-WIDGET-LOGIC**
Widgets must not branch behavior based on confirmation.

---

## SECTION J — Relationship to OS-Level Integration

This pipeline is a prerequisite for deeper OS integration:

* system dialogs
* native confirmations
* secure prompts
* permission flows

OS integration may substitute confirmation UI, but not pipeline semantics.

---

## SECTION K — Acceptance Criteria

SPEC-HAL-138 is satisfied when:

* [ ] Any widget intent can opt into confirmation
* [ ] Validation can be inserted without widget changes
* [ ] Cancel always restores prior state
* [ ] Overlays obey capture and stacking contracts
* [ ] No widget implements confirmation logic
* [ ] No performance regressions observed under heavy UI load

---

## SECTION L — Authoritative Statements

**AUTH-138-1:** “Confirmation is a pipeline, not a popup.”
**AUTH-138-2:** “Validation precedes permission.”
**AUTH-138-3:** “Nothing commits without explicit resolution.”
