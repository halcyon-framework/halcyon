# SPEC-HAL-092B — Font Roles and Semantic Typography Authority

**Status:** Draft

Scoped relationship (Author JC, delegated technical A1 acceptance, 2026-10-10):
HAL-048's accepted dated addition permits immutable product typography requests
on ordinary Label/Button, resolved solely by the framework's registered-face
owner. The role-only restrictions below do not prohibit that mechanism. This
does not adopt this Draft's role taxonomy or document-wide enforcement. The
addition's implementation remains pending source proof and independent review.
**Layer:** Core Architecture / Typography Semantics
**Severity:** Structural
**Enforcement:** Mandatory
**Related SPECs:** SPEC-HAL-006, SPEC-HAL-048, SPEC-HAL-049, SPEC-HAL-050, SPEC-HAL-092, SPEC-HAL-092A, SPEC-HAL-104,
SPEC-HAL-159

---

## 1. Problem Statement

Halcyon currently treats font selection as a **purely mechanical concern** (loading, shaping, rasterization) and lacks
an explicit **semantic contract** governing *why* a font is used.

This manifests as:

* Text alignment failures in Lists and Trees with no column model
* Visual drift between items that are logically aligned
* Repeated misdiagnosis of layout, advance, or shaping bugs
* Implicit assumptions about monospacing leaking into widgets
* Platform-specific inconsistencies (Linux vs macOS/Windows)

The system has **no way to express that certain text requires advance predictability** as an architectural guarantee.

As a result, layout correctness currently depends on **accidental font choice**, which is unacceptable.

---

## 2. Scope

This SPEC governs:

* Semantic classification of UI text by **intended usage**
* Binding between typography semantics and layout expectations
* Widget-level guarantees regarding glyph advance behavior

This SPEC does **not** redefine:

* Font loading mechanisms
* Text shaping or rasterization
* Glyph atlas behavior
* Theme styling or font discovery

---

## 3. Definitions

**Font Role**
A semantic classification describing *how text is expected to behave*, not how it looks.

**Advance Predictability**
The property that glyph advances are equal or layout-stable across runs.

**Semantic Typography Binding**
The process of resolving a font based on *text purpose*, not widget preference.

**Layout-Critical Text**
Text whose visual correctness depends on predictable horizontal advance.

---

## 4. Core Principle

> **Typography semantics are a layout concern, not a styling concern.**

Widgets and layout systems must be able to rely on **explicit guarantees** about text behavior.

---

## 5. Font Roles (Canonical Set)

The system defines the following **mandatory font roles**:

```text
Ui          — General UI text (proportional)
Mono        — Alignment-critical text
Code        — Source code, logs, diagnostics
Numeric     — Digit-aligned numeric displays
Decorative  — Headings, branding, non-layout-critical text
```

Font roles are **closed and authoritative**.
Ad-hoc roles are forbidden.

---

## 6. Binding Invariants

### INV-TYPO-ROLE-1

**All UI text MUST be associated with exactly one Font Role.**

Text without an explicit role is architecturally illegal.

---

### INV-TYPO-ROLE-2

**Widgets MUST declare required Font Roles, not concrete fonts.**

Widgets are forbidden from selecting font families directly.

---

### INV-TYPO-ROLE-3

**Layout MAY assume advance predictability only for FontRole::Mono, Code, or Numeric.**

Any alignment logic relying on equal advances is illegal for `FontRole::Ui` or `Decorative`.

---

### INV-TYPO-ROLE-4

**Font resolution MUST occur before shaping and MUST be role-consistent across the pipeline.**

Shaper and rasterizer MUST operate on the same resolved font for a given role.

---

### INV-TYPO-ROLE-5

**Implicit fallback across Font Roles is forbidden.**

A failure to resolve a role-bound font is an error, not a fallback opportunity.

---

## 7. Widget-Level Guarantees

Widgets implicitly assert layout contracts via their required roles.

Examples (non-exhaustive):

| Widget     | Required Role  |
|------------|----------------|
| Label      | Ui             |
| Button     | Ui             |
| List       | Mono           |
| Tree       | Mono           |
| Table      | Mono / Numeric |
| LogView    | Code           |
| CodeEditor | Code           |

Widgets consuming `FontRole::Mono` are **entitled** to assume:

* Stable glyph advance
* Visually aligned items without column models

---

## 8. Prohibited Patterns (Explicit)

The following are **forbidden**:

* Assuming monospacing without declaring `FontRole::Mono`
* Fixing alignment via per-item padding or offsets
* Using proportional fonts for layout-critical widgets
* Treating font choice as a cosmetic decision in widgets
* Platform-specific font hacks to “make it look right”

---

## 9. Architectural Consequences

* Typography becomes auditable at the semantic level
* Layout bugs are no longer ambiguous:

    * Misalignment → role violation
    * Drift → font binding error
* Widgets become deterministic across platforms
* Font changes cannot silently invalidate layout assumptions

---

## 10. Enforcement Strategy

* Widget APIs MUST require a font role declaration
* Text engine MUST reject unbound or mismatched roles
* Code review: direct font selection in widgets is a hard rejection
* Future tooling MAY add runtime assertions in debug builds

---

## 11. Rationale

This SPEC exists because **implicit typography rules failed**.

Despite correct shaping, correct metrics, and correct rasterization, the absence of semantic font roles allowed layout
correctness to depend on coincidence.

This SPEC makes typography **explicit, enforceable, and architecturally visible**.

---

## 12. Non-Goals

This SPEC does NOT:

* Introduce new widgets
* Define default font families
* Specify platform font choices
* Solve font fallback or localization

It defines **authority and guarantees**, not aesthetics.

---

## 13. Final Statement

> If layout depends on text behavior, that behavior must be declared.

Anything else is accidental correctness.

