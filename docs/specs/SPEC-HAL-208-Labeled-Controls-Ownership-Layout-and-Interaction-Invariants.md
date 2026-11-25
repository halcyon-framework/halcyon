# SPEC-HAL-208: Labeled Controls — Ownership, Layout, and Interaction Invariants

**Status:** Binding  
**Layer:** Widget Contract / Interaction  
**Scope:** Checkbox, RadioButton, RadioGroup (future: Switch, Toggle)  
**Ground Truth:** SPEC-HAL-021, SPEC-HAL-141, SPEC-HAL-188A, SPEC-HAL-206, SPEC-HAL-207

---

## 0. Purpose

This SPEC defines **binding invariants** for controls that present a **binary or discrete indicator paired with textual label content**.

It formalizes how labels are:
- owned,
- laid out,
- rendered,
- hit-tested,
- and interacted with,

without introducing composite widget trees, synthetic children, or implicit event routing.

This SPEC is **not** a styling guide and **not** a layout algorithm description.
It is **law** governing authority boundaries and behavioral guarantees.

---

## 1. Definitions

**Labeled Control**  
A widget that presents:
- a primary indicator (checkbox square, radio circle),
- and an associated text label,
  as a **single semantic control**.

**Indicator**  
The visual affordance representing state (checked / selected).

**Label**  
Static, non-editable text describing the control’s meaning.

---

## Invariant Class A — Ownership & Authority

### INV-A1: Single-Widget Ownership

A labeled control **owns its label**.

The label is:
- not a separate widget,
- not a child node,
- not externally composed.

There must be **exactly one widget** responsible for:
- indicator,
- label,
- interaction semantics.

---

### INV-A2: No External Label Coupling

Application code must not be required to:
- place a `Label` widget next to a control,
- synchronize layout between independent widgets,
- forward events manually.

Any solution requiring sibling widgets for labeling is forbidden.

---

### INV-A3: No Synthetic Widget Nodes

Labels must not be implemented by:
- inserting hidden child widgets,
- proxy Label nodes,
- internal widget trees.

Text is rendered directly by the control.

---

## Invariant Class B — Layout Semantics

### INV-B1: Unified Layout Box

The layout size of a labeled control must account for:
- indicator size,
- indicator–label gap,
- label text size (measured),
- vertical alignment constraints.

The control reports a **single bounding rectangle**.

---

### INV-B2: Text-Driven Height

Control height must be:

max(indicator_height, label_line_height)

Hard-coding height to indicator size is forbidden once labels exist.

---

### INV-B3: Baseline Alignment

Label text must be baseline-aligned according to:
- existing text metric helpers,
- consistent vertical centering relative to indicator.

Pixel-centering hacks are forbidden.

---

### INV-B4: Theme-Controlled Gap

The horizontal gap between indicator and label must be:
- defined in theme geometry,
- not hard-coded in widget code.

Example (conceptual):

checkbox.label_gap
radio.label_gap

---

## Invariant Class C — Rendering

### INV-C1: Indicator First, Label Second

Paint order must be:
1. indicator (background, border, glyph),
2. label text.

This guarantees visual clarity and predictable z-order.

---

### INV-C2: No Label Styling Authority

Labeled controls may:
- render text,
- choose color variants (enabled/disabled/focused),

but must not:
- apply typography effects,
- manage fonts beyond what Button already does.

Label rendering must reuse existing text engine primitives.

---

## Invariant Class D — Interaction & Hit-Testing

### INV-D1: Unified Hit Region

The **entire labeled area** is interactive.

Clicking:
- indicator,
- label text,
- empty space between,

must produce the same semantic activation.

---

### INV-D2: No Label Event Sink

Label text must never:
- intercept pointer events,
- consume clicks independently,
- block activation.

There is no separate hit-testing for label vs indicator.

---

### INV-D3: Focus Enclosure

Where enclosure is required, focus indication (ring, glow, highlight) must cover:
- indicator,
- label,
- gap between them.

Partial focus visuals are forbidden except under the scoped default-selector ruling below.

**JC default-selector ruling — 2026-10-24:** The default presentation of Checkbox,
RadioButton/RadioGroup options and Switch MUST NOT emit an additional focus
enclosure around indicator, gap and label. Existing value indicators and existing
indicator-border/hover/press modulation remain unchanged. No substitute
navigation-focus ornament is required for this default selector presentation.
This scope supersedes the previous full-label enclosure requirement for these
controls only. It does not equate value selection with focus or change framework
focus, current internal target, navigation, activation, capture, label ownership
or unified hit geometry. Other control recipes retain applicable focus/capture
requirements.

---

## Invariant Class E — RadioGroup-Specific Rules

### INV-E1: Row as Atomic Unit

Each radio option row is a **single labeled control**.

Indicator + label form one hit-testable unit.

---

### INV-E2: Per-Row Height Determination

RadioGroup row height must be derived from:
- indicator size,
- label text metrics,

not a fixed constant.

---

### INV-E3: No Cross-Column Labeling

RadioGroup must not require:
- separate columns for indicators and labels,
- alignment tricks in application code.

All layout happens internally.

---

## Invariant Class F — Accessibility & Semantics (Non-Optional)

### INV-F1: Semantic Unity

From the system’s perspective, a labeled control is:
- one focusable entity,
- one semantic action,
- one state machine.

There is no exposed sub-structure.

---

### INV-F2: Future Accessibility Compatibility

This design must remain compatible with:
- accessibility tree projection,
- screen reader labeling,
- semantic role assignment.

Splitting indicator and label would violate this invariant.

---

## Invariant Class G — Prohibited Patterns

### INV-G1: No External Labels

Patterns like:

```rust
Row {
    Checkbox(),
    Label("Accept terms")
}
```

are forbidden for labeled controls.

⸻

INV-G2: No Widget Composition for Labels

Using:
	•	Flex,
	•	Stack,
	•	Overlay,
	•	or any layout container

to simulate labeling is forbidden.

⸻

INV-G3: No Event Forwarding Hacks

Manual forwarding such as:
	•	“on label click, toggle checkbox”
	•	“delegate pointer event”

is forbidden.

⸻

8. Migration Guidance (Non-Normative)
	•	Existing demos must be updated to use built-in labels.
	•	External Label widgets may still exist for standalone text.
	•	Checkbox / RadioGroup APIs will gain label parameters or setters.
	•	Button remains unchanged — it already owns its label correctly.

⸻

9. Summary (Non-Normative)

This SPEC establishes that:
	•	Labels are not decoration.
	•	Labels are not layout glue.
	•	Labels are part of the control.

Checkboxes and radio buttons without integrated labels are structurally incomplete controls.

This SPEC locks that door permanently.

⸻


If you want, next steps can be:
- define the exact constructor / setter API for labels,
- specify theme geometry additions,
- or write the concrete implementation task for Checkbox first, RadioGroup second.
