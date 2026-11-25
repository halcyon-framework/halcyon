# SPEC-HAL-210: Slider Control and Value Exposure Invariants

**Status:** Binding  
**Layer:** Widget Contract  
**Scope:** Slider control only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208, SPEC-HAL-209

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the Slider control.

A Slider represents **continuous or stepped scalar input** with optional
**value exposure**, while preserving Halcyon’s core laws:

- strict ownership boundaries
- deterministic behavior
- no hidden state
- application-layer synchronization only

This SPEC locks down **what a Slider is allowed to do** and, more importantly,
**what it must never do**.

---

## 1. Conceptual Definition

A **Slider** is a control that:

- exposes a scalar value within a fixed domain
- supports pointer dragging and keyboard adjustment
- may visually expose its current value
- never owns application state

The Slider is **not** a data source.
It is a **value emitter only**.

---

## Invariant Class A — Value Semantics

### INV-A1: Explicit Value Domain

Each Slider has an explicit domain:

- `min`
- `max`
- optional `step`

The domain is immutable after construction.

---

### INV-A2: Single Scalar Value

The Slider represents exactly one scalar value:

- integer **or**
- floating-point

No multi-axis, no ranges, no compound values.

---

### INV-A3: No Implicit Clamping

Slider must never silently modify values:

- No auto-normalization
- No implicit rounding
- No snapping unless `step` is specified

All value constraints are explicit.

---

## Invariant Class B — Input & Commit Semantics

### INV-B1: Explicit Commit Points

Value updates occur only on:

- pointer drag movement
- keyboard adjustment

No commits on:

- hover
- focus
- animation ticks

---

### INV-B2: Drag Is Continuous Intent

During pointer drag:

- value updates continuously
- final value on release is not special

There is **no “preview” vs “commit” distinction**.

---

### INV-B3: Keyboard Parity

Keyboard interaction must mirror pointer semantics:

- Arrow keys adjust by step or default delta
- Page keys adjust by larger delta
- Disabled state blocks all adjustments

---

## Invariant Class C — Value Exposure

### INV-C1: Optional Value Label

Slider **may** expose its current value visually:

- inline label
- tooltip
- adjacent text

This exposure is **purely visual**.

---

### INV-C2: Optional Editable Value Field

If a value label is exposed, it **may** be made editable as an
optional, declarative feature.

When editing is enabled:

- entry into edit mode is explicit (e.g. double-click on the value)
- commit is explicit (Enter)
- cancel is explicit (Escape)
- focus loss reverts to the value at edit start (no implicit commit)
- typed input is parsed and clamped to the domain on commit
- parse failure reverts silently

When the slider is not in edit mode, the value field is non-interactive
and read-only. Pointer drag and keyboard adjustment continue to operate
on the slider proper, independent of the value field.

Value editing must reuse one real WidgetTree-owned TextField child and the HAL-101 section 2.4 engine-owned session. The Slider retains its own committed scalar, domain, edit-start state and existing explicit commit/cancel policy, with only source-qualified owned draft observations. It shall not introduce or own another text editing mechanism.

---

### INV-C3: Label Ownership

If a value label exists:

- it is owned by the Slider
- not a child widget
- not externally composed

Same ownership model as SPEC-HAL-208 / 209.

The host-painted value label remains non-Widget display content under this existing rule. The separate active text-entry child is a tree-owned editing role governed by section 2.4, not the optional display label. Its alignment and the numeric meaning/commit policy are unchanged. This clause does not activate the separate text-motion timing exception or admit centered P6/P7 content presentation.

The closed host adapter preserves this widget's current command, resolver/value/tag, callback and clear/reset order under HAL-101 section 2.5; ordinary standalone TextField Enter/Cancel shall not replace host policy. Runtime child-dependent API calls require the scoped owner context; no no-context peer replacement or silently queued sync is admitted.

---

## Invariant Class D — Visual & Layout Contract

### INV-D1: Geometry Includes Value Label

Layout must account for:

- track
- thumb
- value label (if enabled)

No overlap. No clipping.

---

### INV-D2: Baseline Alignment

Textual value exposure must be baseline-aligned using font metrics.

---

### INV-D3: Full-Area Hit Testing

Entire control area (track + thumb + label) is hit-testable.

---

## Invariant Class E — Animation & Time

### INV-E1: Animation Is Non-Semantic

Animation must never:

- affect value computation
- delay value emission
- introduce time-based state

---

### INV-E2: No Internal Timing

Slider must not own:

- timers
- clocks
- easing logic tied to semantics

**INV-E3: Shared Value-Entry Text Presentation Only.** Only an already active Slider text-entry region may consume HAL-101 section 8.4.6 after its ownership admission. This narrowly excepts INV-E1's time-based-state prohibition only for the existing bounded nonsemantic text/caret projection. INV-E2's prohibition of internal timers/clocks and semantic easing remains. Pointer/keyboard scalar adjustment, domain/clamp, label alignment, edit entry, immediate value emission and existing explicit commit/cancel policy are unchanged. Centered text does not become eligible for left-aligned P6/P7 presentation.

**FORBID-E3.** No delayed value, scalar interpolation through the editor channel, new host clock, changed alignment or duplicate TextField engine.

---

## Invariant Class F — Binding & Integration

### INV-F1: Application-Layer Binding Only

Slider emits values exclusively via:

- `ChangeWriter::set_*`

Exact type depends on scalar kind.

---

### INV-F2: No Model Access

Slider must never:

- read application state
- cache model values
- synchronize internally

---

### INV-F3: Exactly-Once Sync Semantics

Multiple value changes within a frame:

- are coalesced
- final value wins

Observed via `FormBinder`.

---

## Invariant Class G — Disabled & Focus Semantics

### INV-G1: Disabled Is Absolute

When disabled:

- no dragging
- no keyboard adjustment
- no value emission

---

### INV-G2: Focus Is Non-Mutating

Focus affects visuals and keyboard routing only.

---

## Invariant Class H — Determinism & Safety

### INV-H1: Deterministic Replay

Identical inputs yield identical value sequences.

---

### INV-H2: No Per-Frame Allocation

Steady-state interaction must not allocate.

---

### INV-H3: Invariant Supremacy

No invariant may be violated for UX, animation,
or platform imitation.

New behavior requires a new SPEC.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define:

- exact slider appearance
- orientation (horizontal / vertical)
- animation curves
- formatting of value text

Those are layered concerns.

---

## 3. Summary (Non-Normative)

- Slider emits a scalar value only
- Value exposure is visual, never editable
- No timing, no ownership, no model access
- Fully deterministic, binding-driven control

This SPEC makes Slider a **safe, composable, high-precision primitive**.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.
