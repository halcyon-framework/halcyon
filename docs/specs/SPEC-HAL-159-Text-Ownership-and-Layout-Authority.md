# SPEC-HAL-XXX — Text Ownership and Layout Authority

**Status:** Draft
**Layer:** Core Architecture / Layout Authority
**Severity:** Structural
**Enforcement:** Mandatory
**Related SPECs:** SPEC-HAL-008, SPEC-HAL-010, SPEC-HAL-094, SPEC-HAL-141, SPEC-HAL-146, SPEC-HAL-158, SPEC-HAL-092, SPEC-HAL-104

---

## 1. Problem Statement

Halcyon currently permits **text rendering that is not owned by a layout participant**.
This manifests as:

* UI text rendered directly in window or scene space
* Instructional/status text in demos bypassing layout
* Baseline-positioned text appearing visually “correct” but architecturally illegal
* Repeated misdiagnosis of layout, coordinate, or scroll bugs that are in fact ownership violations

This has already caused **multiple regressions, wasted audits, and false debugging paths**.

The system lacks a **single, explicit authority rule** governing *who is allowed to place text*.

---

## 2. Scope

This SPEC governs:

* All **visible text that is part of the UI**
* All **text rendered during normal application operation**
* All **demo and example code**

This SPEC does **not** redefine text shaping, glyph metrics, or rendering pipelines.

---

## 3. Definitions

**UI Text**
Text that communicates state, instruction, labels, content, or feedback to the user.

**Layout Participant**
An entity that:

* participates in the layout engine
* owns a `Rect` produced by layout
* is subject to layout authority and invalidation rules

**Text Widget**
A widget whose primary responsibility is text presentation (e.g. Label, TextBlock, Paragraph).

**Non-UI Text**
Text rendered for debugging, diagnostics, profiling, or development tooling.

---

## 4. Core Principle

> **Text does not own space. Widgets do.**

Text may only appear on screen **as content owned by an entity that owns layout space**.

---

## 5. Binding Invariants

### INV-TEXT-OWNERSHIP-1

**All UI text MUST be owned by a layout participant.**

Rendering UI text outside a widget is **architecturally illegal**.

---

### INV-TEXT-OWNERSHIP-2

**Text MUST NOT be positioned directly in window, scene, or absolute coordinates for UI purposes.**

Baseline math is an *internal concern* of a widget, not a placement mechanism.

---

### INV-TEXT-OWNERSHIP-3

**Text rendering APIs exposed at scene or renderer level MUST NOT be used for UI text.**

They are reserved exclusively for:

* debug overlays
* diagnostics
* tooling output

---

### INV-TEXT-OWNERSHIP-4

**All demo and example UI MUST obey production layout rules.**

Demos are not exempt.
Instructional text MUST use text widgets.

---

### INV-TEXT-OWNERSHIP-5

**Text widgets MUST derive their final position exclusively from layout results.**

Any text placement derived from:

* manual offsets
* baseline anchoring relative to window
* ad-hoc coordinate math

is illegal outside a widget’s internal implementation.

---

## 6. Allowed but Restricted Behavior

### DEBUG-TEXT-ALLOWANCE-1

Non-UI text MAY be rendered without layout participation **only if**:

* explicitly marked as debug/diagnostic
* visually distinguishable from UI text
* disabled or gated in production builds

This exception MUST NOT be used by demos, examples, or tests presenting UI behavior.

---

## 7. Prohibited Patterns (Explicit)

The following patterns are **forbidden**:

* Rendering instructional text directly in window demos
* Status lines drawn via scene primitives instead of widgets
* “Temporary” text bypassing layout “just for now”
* Using baseline positioning to simulate layout
* Treating text as a lightweight visual instead of content

---

## 8. Architectural Consequences

* Any visible text implies a widget boundary
* Labels, headings, status lines, and instructions are widgets
* Text placement bugs are no longer ambiguous:

    * If text looks wrong → check widget layout, not renderer
* Layout authority is preserved and auditable

---

## 9. Enforcement Strategy

* Code review: direct scene text rendering in UI contexts is a hard rejection
* Demos failing this SPEC must be rewritten
* Future tooling MAY add runtime assertions or debug flags
* Release-grade caller enforcement runs for every emitted text node and therefore MUST inspect the
  static caller path without allocating or normalizing an owned path string per call

---

## 10. Rationale

This SPEC exists because **implicit rules failed**.

Despite extensive SPEC coverage of layout, geometry, typography, and widgets, the absence of a single, explicit ownership law allowed text to repeatedly escape architectural control.

This SPEC closes that hole permanently.

---

## 11. Non-Goals

This SPEC does NOT:

* Redesign text widgets
* Change glyph metrics or shaping
* Define typography styles
* Introduce new rendering features

It defines **authority**, not appearance.

---

## 12. Final Statement

> If text is visible to the user, it must belong to something that owns space.

Anything else is a bug.
