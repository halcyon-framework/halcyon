# SPEC-HAL-144: Intent Routing Contracts — Provenance and Authority

**Status:** Binding
**Layer:** Input & Interaction Model
**Scope:** Formal routing, ownership, and authority of user intent
**Depends on:** SPEC-HAL-033, SPEC-HAL-038, SPEC-HAL-140, SPEC-HAL-143
**Constrains:** Engine-level semantics only (no widget policy invention)

---

## SECTION A — Problem Statement

Halcyon already distinguishes **events**, **gestures**, and **actions**, but lacks a
formal, binding model answering a critical question:

> *Who is allowed to emit which intent, and why?*

Without explicit intent routing contracts, systems degrade into:

* scroll happening “somewhere”
* selection mutating unexpectedly
* activation triggered by the wrong source
* keyboard and pointer paths collapsing into one
* bugs that appear only after refactors

This SPEC defines **intent as a first-class, governed signal** with explicit provenance,
authority, and routing rules.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Intent has provenance**
Every intent must declare *where it came from*.

**PRINCIPLE-2: Intent is not an event**
Events are raw; intent is interpreted.

**PRINCIPLE-3: Authority precedes routing**
An intent without authority is invalid.

**PRINCIPLE-4: No silent fallthrough**
If an intent is rejected, it stops.

**PRINCIPLE-5: One intent, one owner**
Intent may not be “handled by whoever wants it”.

---

## SECTION C — Intent Taxonomy

Halcyon recognizes the following **engine-level intent classes**:

1. Navigation Intent
2. Selection Intent
3. Activation Intent
4. Scroll Intent
5. Edit Intent

New intent classes require an SPEC.

---

## SECTION D — Intent Provenance

Every intent MUST declare its origin.

**INV-INTENT-PROVENANCE-1**
Each intent MUST specify exactly one source:

* Pointer
* Keyboard
* Programmatic
* Accessibility

**INV-INTENT-NO-AMBIGUITY**
An intent MUST NOT have multiple or inferred sources.

---

## SECTION E — Intent Authority

Intent emission requires authority.

**INV-INTENT-AUTHORITY-1**
An intent MUST be authorized before routing.

Authorization depends on:

* focus
* capture
* navigation cursor
* interaction model

If authorization fails, intent is discarded.

---

## SECTION F — Intent Routing

Routing determines *where* intent is delivered.

**INV-INTENT-ROUTE-EXPLICIT**
Routing MUST be explicit and deterministic.

**INV-INTENT-NO-BUBBLE**
Intents MUST NOT bubble implicitly.

**INV-INTENT-NO-FALLBACK**
There is no default handler.

---

## SECTION G — Domain-Specific Rules

### Navigation Intent

* Source: Keyboard / Programmatic
* Target: Navigation Cursor

**INV-NAV-INTENT-CURSOR**
Navigation intent MUST operate on the cursor only.

---

### Selection Intent

* Source: Keyboard / Pointer
* Target: Selection model

**INV-SEL-INTENT-AUTH**
Selection intent requires selection authority.

---

### Activation Intent

* Source: Keyboard / Pointer
* Target: Cursor target

**INV-ACT-INTENT-EXPLICIT**
Activation MUST be explicit (Enter, click).

---

### Scroll Intent

* Source: Pointer / Keyboard
* Target: Captured scroll container

**INV-SCROLL-INTENT-CAPTURE**
Scroll intent requires scroll capture.

---

### Edit Intent

* Source: Keyboard / Programmatic
* Target: Editing substrate

**INV-EDIT-INTENT-FOCUS**
Edit intent requires editing focus.

---

## SECTION H — Intent Rejection Semantics

Rejected intents are **terminal**.

**INV-INTENT-REJECT-STOP**
Rejected intents MUST NOT be rerouted.

**INV-INTENT-REJECT-OBSERVABLE**
Rejection MUST be observable in debug / tracing.

---

## SECTION I — Forbidden Behaviors

The following are forbidden:

* routing intent without authority
* inferring intent source
* fallback routing
* widget-local intent interception
* mixing intent classes

---

## SECTION J — Interaction with Existing SPECs

* Gesture arbitration — SPEC-HAL-033
* Focus routing — SPEC-HAL-038
* Focus vs capture — SPEC-HAL-140
* Navigation cursor — SPEC-HAL-143

SPEC-HAL-144 introduces **no new mechanics**, only binding contracts.

---

## SECTION K — Performance and Determinism

**INV-INTENT-O(1)**
Intent authorization and routing MUST be constant-time.

**INV-INTENT-DETERMINISTIC**
Identical inputs MUST produce identical intent routing.

---

## SECTION L — Acceptance Criteria

SPEC-HAL-144 is satisfied when:

* [ ] Every intent has explicit provenance
* [ ] Unauthorized intents are rejected
* [ ] No implicit bubbling exists
* [ ] Scroll, selection, activation paths are disjoint
* [ ] Debug traces show intent lifecycle clearly

---

## SECTION M — Authoritative Statements

**AUTH-144-1:** “Events happen. Intent is chosen.”
**AUTH-144-2:** “Authority decides before delivery.”
**AUTH-144-3:** “No intent without ownership.”
