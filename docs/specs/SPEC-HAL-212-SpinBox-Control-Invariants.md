# SPEC-HAL-212: SpinBox Control Invariants

**Status:** Binding  
**Layer:** Widget Contract  
**Scope:** SpinBox (numeric increment / decrement control)  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208, SPEC-HAL-210

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the SpinBox control.

A SpinBox is a **numeric value editor** with explicit increment/decrement affordances.
It combines **direct value input semantics** with **step-based adjustment**, without hiding
or inventing behavior.

This SPEC exists to prevent SpinBoxes from becoming:
- mini state machines
- implicit validators
- magic numeric widgets with hidden rules

SpinBox must remain **boring, explicit, and predictable**.

---

## 1. Conceptual Definition

A SpinBox represents a **single scalar numeric value**.

It allows the user to:
- increment
- decrement
- optionally type (if a text surface is present)

It must **never**:
- guess intent
- auto-correct silently
- infer ranges
- manage formatting policies implicitly

---

## Invariant Class A — Ownership & Authority

### INV-A1: Value Is External

SpinBox does not own the authoritative value.

It emits **explicit change intents** only.
The application layer owns the value.

---

### INV-A2: No Hidden State

SpinBox must not store:
- cached numeric history
- accumulated deltas
- derived state

All visible state is derived from the current value.

---

### INV-A3: No Async Ownership

SpinBox must not:
- debounce internally
- delay commits
- animate toward values

All timing belongs outside.

**INV-A4: Tree-owned text entry.** When direct text editing is supported, SpinBox uses the stable WidgetTree-owned TextField child and qualified engine session of HAL-101 section 2.4. SpinBox owns numeric value/domain, edit-start state and existing explicit parsing/commit/cancel/step behavior, never the child's Widget or editing state. Owned draft observations are not numeric authority. Runtime force-commit and draft replacement must use the explicit qualified owner boundary before emitting their result; no silent queue or stale draft is permitted. Numeric alignment, immediate exactly-once value emission and INV-A3's prohibition of animation toward numeric values remain. The separate motion extension remains inactive until jointly adopted.

**INV-A4: Shared Value-Entry Text Presentation Only.** Only an already active SpinBox text-entry region may consume HAL-101 section 8.4.6 after its ownership admission. INV-A3 continues to prohibit debounce, delayed commits and animation toward numeric values; its timing exclusion permits only this existing engine-owned finite text/caret projection. Increment/decrement, numeric domain, parsing, alignment and exactly-once explicit emission remain unchanged. Centered text does not become eligible for left-aligned P6/P7 presentation.

**FORBID-A4.** No numeric value smoothing, timing-dependent stepping, hidden commit or duplicated TextField engine.

The closed host adapter preserves this widget's current command, resolver/value/tag, callback and clear/reset order under HAL-101 section 2.5; ordinary standalone TextField Enter/Cancel shall not replace host policy. Runtime child-dependent API calls require the scoped owner context; no no-context peer replacement or silently queued sync is admitted.

---

## Invariant Class B — Numeric Semantics

### INV-B1: Explicit Numeric Domain

SpinBox operates on a declared numeric domain:

- integer or floating-point
- optional min / max
- explicit step

No implicit defaults.

---

### INV-B2: Step Is Declarative

Increment/decrement behavior uses a **declared step**.

No acceleration.
No variable step size.
No modifier-key magic unless explicitly specified.

---

### INV-B3: No Silent Clamping

If a change would exceed bounds:

- behavior must be explicit (reject / clamp / no-op)
- default behavior must be documented

Silent correction is forbidden.

---

## Invariant Class C — Input & Commit Semantics

### INV-C1: Explicit Change Emission

SpinBox emits a change only when:

- increment/decrement is activated
- text input commits (if supported)

No emission on hover, focus, or layout.

---

### INV-C2: Exactly-Once Per Action

Each user action produces **at most one change emission**.

No cascading or repeated emission.

---

### INV-C3: No Auto-Commit Guessing

SpinBox must not infer commit points.

If text editing exists:
- commit is explicit (Enter / focus loss if defined)

---

## Invariant Class D — Visual & Layout Contract

### INV-D1: Three Logical Regions

A SpinBox consists of:
- value display region
- increment control
- decrement control

These regions must be visually and semantically distinct.

---

### INV-D2: Label Ownership (Optional)

If a label exists:
- it is owned by the SpinBox
- it is non-interactive
- clicking label does **not** change value

(Label ≠ toggle)

---

### INV-D3: Hit Regions Are Exact

Increment/decrement hit regions must be precise.

No full-row accidental activation.

---

### INV-D4: Single-owner button/value seams (2026-11-23)

A standalone SpinBox projects exactly one border at each internal seam
between the numeric TextField region and its decrement/increment controls.
The shared TextField input surface paints those internal borders; the SpinBox
paints only the two button zones' top and bottom segments and the outer left
and right edges. The complete control uses the existing layout-cached region
geometry and physical-pixel snapping. Closed button-zone borders overlapping
the TextField's own edges are forbidden.

This is a paint-ownership clarification only. Preserve the unchanged
CellIntegrated presentation, stable tree-owned editor, value/step semantics,
existing Focus/Capture, text selection, disabled behavior, and exact region
hit testing.

## Invariant Class E — Keyboard Semantics

### INV-E1: Arrow Keys Are Explicit

If arrow keys are supported:
- Up = increment
- Down = decrement

No acceleration.
No wrapping.

---

### INV-E2: Modifier Keys Are Inert

Unless explicitly specified, modifiers do nothing.

No hidden Shift / Ctrl semantics.

---

## Invariant Class F — Disabled & Focus Semantics

### INV-F1: Disabled Is Absolute

When disabled:
- no emission
- no value change
- visual state only

---

### INV-F2: Focus Does Not Change Value

Gaining or losing focus must not mutate value.

---

## Invariant Class G — Binding & Integration

### INV-G1: Application-Layer Binding

SpinBox integrates via **explicit binding / emission**, not polling.

---

### INV-G2: No Self-Synchronization

SpinBox never reads from models directly.
It reacts only to provided value props.

---

## Invariant Class H — Determinism & Safety

### INV-H1: Deterministic Behavior

Given the same input sequence and values,
SpinBox behavior is identical.

---

### INV-H2: No Per-Frame Allocation

No allocations during idle or steady interaction.

---

### INV-H3: Invariant Supremacy

No invariant may be violated for:
- “better UX”
- “platform parity”
- “expected behavior”

New behavior requires a new SPEC.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define:
- number formatting
- locale rules
- text parsing strategies
- value display precision

Those belong to higher-level abstractions.

---

## 3. Summary (Non-Normative)

- SpinBox is a **strict numeric editor**
- No magic, no inference, no guessing
- Every change is explicit
- Application owns meaning

This SPEC locks SpinBox as a **mechanical control**, not a smart one.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.
