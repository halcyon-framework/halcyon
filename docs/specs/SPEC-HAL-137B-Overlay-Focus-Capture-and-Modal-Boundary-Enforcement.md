# SPEC-HAL-137B: Overlay Focus, Capture, and Modal Boundary Enforcement

**Status:** Binding / Enforcement  
**Layer:** Engine (input routing, focus, capture)  
**Scope:** Runtime overlay behavior (modal correctness)  
**Depends on:** SPEC-HAL-127, SPEC-HAL-128, SPEC-HAL-137  
**Constrains:** `OverlayRoot`, `EventRouter`, `FocusManager`, pointer capture subsystem

---

## SECTION A — Purpose

SPEC-HAL-137 defined the **structural model** of overlays and introduced hardening rules.  
This SPEC **locks runtime enforcement** of the most critical invariants that were previously specified but not fully implemented.

This document is **not additive**.  
It converts existing expectations into **mandatory, testable, engine-enforced rules**.

---

## SECTION B — Scope Clarification

SPEC-HAL-137B addresses **only** the following classes of failure:

1. Focus loss or leakage across modal boundaries
2. Pointer capture survival across modal activation
3. Inconsistent keyboard vs pointer authority
4. Undefined restoration behavior after modal dismissal

It does **not** introduce:
- new overlay kinds
- new widgets
- new scheduling phases
- new APIs

---

## SECTION C — Terminology

**Active Modal**  
The topmost `OverlayEntry` whose `OverlayKind` is `Modal`.

**Modal Boundary**  
The subtree rooted at the modal’s mounted node.

**Pre-Modal Focus**  
The focused widget immediately before a modal becomes active.

**Foreign Widget**  
Any widget not contained within the active modal boundary.

---

## SECTION D — Focus Restoration Contract (P0)

### D.1 Capture Pre-Modal Focus

**INV-OVL-FOCUS-RESTORE-1**  
When a modal overlay is pushed, the engine MUST record the currently focused widget (if any) as the modal’s `focus_restore_target`.

**INV-OVL-FOCUS-RESTORE-2**  
The recorded focus target MUST be captured exactly once, at modal activation time.

---

### D.2 Restore on Modal Dismissal

**INV-OVL-FOCUS-RESTORE-3**  
When a modal overlay is popped, the engine MUST attempt to restore focus to the recorded `focus_restore_target`.

**INV-OVL-FOCUS-RESTORE-4**  
If the recorded target no longer exists or is no longer focusable, focus MUST be cleared (not reassigned heuristically).

**INV-OVL-FOCUS-RESTORE-5**  
Focus restoration MUST NOT trigger layout, realization, or focus traversal.

---

## SECTION E — Pointer Capture Cancellation (P0)

### E.1 Modal Supremacy Over Capture

**INV-OVL-CAPTURE-CANCEL-1**  
When a modal overlay becomes active, **all pointer capture sessions MUST be cancelled immediately**.

This includes:
- SPEC-HAL-128 `PointerCaptureManager`
- any legacy pointer capture paths

---

### E.2 Ordering Guarantee

**INV-OVL-CAPTURE-CANCEL-2**  
Capture cancellation MUST occur before:
- any modal input routing
- any focus changes
- any gesture arbitration

---

### E.3 Post-Modal State

**INV-OVL-CAPTURE-CANCEL-3**  
After cancellation, no widget outside the modal boundary may receive pointer events until the modal is dismissed.

---

## SECTION F — Modal Focus Trap Enforcement (P0)

### F.1 Pointer-Based Focus Assignment

**INV-OVL-FOCUS-TRAP-POINTER-1**  
While a modal is active, pointer-driven focus changes MUST be rejected if the target widget is a foreign widget.

**INV-OVL-FOCUS-TRAP-POINTER-2**  
Rejected focus changes MUST NOT:
- clear existing modal focus
- propagate fallback focus
- emit warnings or retries

---

### F.2 Keyboard and Pointer Parity

**INV-OVL-FOCUS-TRAP-PARITY-1**  
Keyboard and pointer input MUST obey identical modal focus boundaries.

No input modality is privileged.

---

## SECTION G — Escape and Gesture Interaction (Clarification)

This SPEC does not redefine Escape semantics but **clarifies ordering**:

**INV-OVL-ESCAPE-ORDER-1**  
Escape MUST cancel active pointer/gesture sessions before modal dismissal logic executes.

This preserves SPEC-HAL-128 guarantees.

---

## SECTION H — Forbidden Behaviors

**FORBID-OVL-FOCUS-HEURISTIC**  
The engine MUST NOT guess or synthesize a new focus target on modal dismissal.

**FORBID-OVL-CAPTURE-SURVIVAL**  
Pointer capture surviving modal activation is forbidden under all circumstances.

**FORBID-OVL-FOCUS-LEAK**  
Focus assignment outside an active modal boundary is forbidden.

---

## SECTION I — Test Requirements

SPEC-HAL-137B is not satisfied unless all of the following are test-proven:

1. Focus is restored to the exact pre-modal widget after dismissal
2. Pointer capture is cancelled on modal open
3. Pointer cannot focus widgets outside modal
4. Focus is cleared if restore target is invalid
5. Escape cancels gesture before modal close

---

## SECTION J — Performance Guarantees

All enforcement in this SPEC is:

- O(1) per modal transition
- free of layout or realization
- purely state-based

No per-frame cost is introduced.

---

## SECTION K — Authoritative Statements

**AUTH-137B-1**  
“Modal supremacy is absolute: focus, capture, and input are subordinate.”

**AUTH-137B-2**  
“Focus restoration is deterministic or absent — never inferred.”

**AUTH-137B-3**  
“If pointer and keyboard disagree, the system is broken.”

---

## SECTION L — Relationship to Future SPECs

SPEC-HAL-137B is a **hard prerequisite** for:

- SPEC-HAL-138 (Confirmation & Validation Pipeline)
- SPEC-HAL-139 (OS Mediation & System Integration)

Those SPECs assume modal correctness as a given.
