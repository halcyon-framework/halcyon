**Filename:**
`docs/SPECs/SPEC-HAL-152-OS-Event-Ingress-and-Sanitization.md`

---

# SPEC-HAL-152: OS Event Ingress and Sanitization

**Status:** Binding
**Layer:** Platform Boundary / Input Ingress
**Scope:** Ingress, normalization, validation, and rejection of OS-originated events
**Depends on:** SPEC-HAL-033, SPEC-HAL-038, SPEC-HAL-140, SPEC-HAL-151
**Constrains:** Engine-level semantics only (no OS policy invention)

---

## SECTION A — Problem Statement

Operating systems deliver input as **untrusted, lossy, inconsistent streams**:

* reordered or coalesced events
* missing up/down pairs
* platform-specific semantics leaks
* implicit capture/focus assumptions
* device quirks surfacing as logic bugs

If OS events enter Halcyon directly, **engine invariants are violated at the boundary**.

This SPEC defines a **hard ingress layer** that sanitizes, normalizes, and validates all OS events **before** they can influence interaction state.

---

## SECTION B — Design Principles

**PRINCIPLE-1: OS input is untrusted**
All OS events are treated as hostile until proven valid.

**PRINCIPLE-2: Normalize first, route later**
No routing, capture, or focus logic runs on raw OS events.

**PRINCIPLE-3: Drop beats guess**
Invalid or ambiguous events are rejected, never “fixed”.

**PRINCIPLE-4: Determinism over fidelity**
Consistency beats platform feature parity.

**PRINCIPLE-5: One-way boundary**
OS events may enter Halcyon only through a single ingress.

---

## SECTION C — Ingress Pipeline

All OS input passes through the following stages:

1. **Receipt** — raw OS event
2. **Normalization** — convert to canonical Halcyon event form
3. **Sanitization** — validate sequences and invariants
4. **Admission** — accepted events enter the engine
5. **Rejection** — invalid events are dropped with diagnostics

Stages 2–4 are mandatory.

---

## SECTION D — Canonical Event Model

All admitted events must be representable as:

* PointerMove / PointerDown / PointerUp
* Scroll(delta)
* KeyDown / KeyUp
* TextInput
* WindowFocus / WindowBlur
* WindowResize
* DeviceChange
* RelativePointerMotion(lease_id, window_id, pointer_id, time, RelativeLogicalDelta)

`RelativePointerMotion` is the narrow additive event defined by SPEC-HAL-362.
It is canonical only after matching-lease binding, logical normalization,
finite-value and timestamp sanitization, and admission under that SPEC. A raw
host, native, or device-relative delta is not this event.

**INV-OS-CANONICAL-ONLY**
No engine code may observe raw OS events.

---

## SECTION E — Sanitization Rules

**INV-OS-SEQUENCE-VALID**
Input sequences MUST be structurally valid:

* no PointerUp without prior PointerDown
* no duplicate Down without Up
* no negative device IDs
* no cross-device conflation

**INV-OS-TEMPORAL-MONOTONIC**
Event timestamps MUST be non-decreasing per device stream.

**INV-OS-FOCUS-EXPLICIT**
Focus changes must be explicit OS events; never inferred.

---

## SECTION F — Rejection Policy

Rejected events:

* do not mutate state
* do not propagate
* are logged in debug builds

**INV-OS-NO-RECOVERY**
Sanitization MUST NOT invent synthetic events to “heal” streams.

---

## SECTION G — Capture and Focus Interaction

**INV-OS-NO-IMPLICIT-CAPTURE**
OS events MUST NOT imply capture.

**INV-OS-NO-IMPLICIT-FOCUS**
Pointer movement MUST NOT imply focus.

Capture and focus transitions are handled **only** by engine logic per SPEC-HAL-140 and SPEC-HAL-151.

---

## SECTION H — Multi-Window Routing

OS events containing window identifiers are validated:

**INV-OS-WINDOW-VALID**
Events referencing unknown or destroyed windows are dropped.

**INV-OS-WINDOW-BOUNDARY**
Events are routed only to their owning window domain.

---

## SECTION I — Platform Variance Containment

Platform-specific behavior (e.g., scroll phases, key repeat, touch quirks):

* MUST be normalized or discarded at ingress
* MUST NOT leak into engine semantics

**INV-OS-NO-PLATFORM-LEAK**
Engine behavior must be identical across platforms given identical canonical events.

---

## SECTION J — Diagnostics and Instrumentation

Ingress must expose:

* counts of dropped events
* rejection reasons
* per-device stream stats

Instrumentation MUST NOT alter behavior.

---

## SECTION K — Performance Guarantees

**INV-OS-O1**
Ingress processing MUST be constant-time per event.

**INV-OS-NO-ALLOCATION**
Ingress MUST NOT allocate on the hot path.

---

## SECTION L — Interaction with Existing SPECs

* Gesture arbitration: SPEC-HAL-033
* Focus routing: SPEC-HAL-038
* Focus vs capture: SPEC-HAL-140
* Multi-window domains: SPEC-HAL-151

SPEC-HAL-152 introduces **no new interaction behavior**, only boundary enforcement.

---

## SECTION M — Acceptance Criteria

SPEC-HAL-152 is satisfied when:

* [ ] Raw OS events never reach engine logic
* [ ] Invalid sequences are dropped deterministically
* [ ] Focus/capture are never inferred from OS input
* [ ] Cross-platform behavior is identical
* [ ] Diagnostics clearly report rejected input

---

## SECTION N — Authoritative Statements

**AUTH-152-1:** “The OS lies. The engine verifies.”
**AUTH-152-2:** “If input is ambiguous, it is invalid.”
**AUTH-152-3:** “Sanitization is not recovery.”
