# SPEC-HAL-096C — Geometry Token Enforcement and Migration Law

**Status:** Draft
**Layer:** Architecture / Enforcement
**Depends on:**

* SPEC-HAL-002 (Constitution and Architectural Invariants)
* SPEC-HAL-090 (Logical Grid System and Compact Layout Rules)
* SPEC-HAL-093 (Visual Appearance and Radii System)
* SPEC-HAL-094 (Component Geometry and Layout Rules)
* SPEC-HAL-096A (Style Primitives and Decoration Stack)
* SPEC-HAL-096B (Geometry Tokens and Canonical Metrics)

**Supersedes:** None
**Scope:** Widget layout, paint, hit-testing, and geometry literals
**Audience:** Engine, widget, and component authors

---

## 1. Purpose

This SPEC makes **geometry token usage mandatory** and **hardcoded geometry literals illegal** in Halcyon widget code, except for explicitly enumerated exceptions.

This document is **enforcement-only**.

It introduces:

* binding rules,
* explicit prohibitions,
* a migration law,
* and a review gate.

No new geometry tokens are defined here.
No visual behavior is changed by this SPEC.

---

## 2. Definitions

**Geometry Literal**
Any numeric value (`f32`, `i32`, etc.) that encodes:

* spacing,
* padding,
* margin,
* border thickness,
* corner radius,
* component height/width,
* hit-zone expansion,
  when used directly in layout, paint, or hit-testing logic.

**Geometry Token**
A value sourced from theme-level specs defined in:

* `Theme.spacing`
* `Theme.padding`
* `Theme.borders`
* `Theme.radii`
* component geometry tables defined by SPEC-HAL-094

**Token-Available Context**
Any code path where `Theme` is accessible (layout/paint contexts, widget runtime).

---

## 3. Absolute Prohibitions

### 3.1 New Hardcoded Geometry (Hard Law)

After adoption of this SPEC:

> **No new geometry literals may be introduced** in any widget layout, paint, or hit-testing code when a token exists or could exist.

This includes but is not limited to:

* padding values,
* internal spacing,
* border widths,
* radii,
* control heights,
* row heights,
* scrollbar dimensions.

Violations are **architectural errors**, not stylistic issues.

---

### 3.2 Silent Drift Is Forbidden

Replacing one hardcoded literal with another hardcoded literal is illegal.

Example (illegal):

```rust
// BAD: literal swap
let padding = 8.0; // replaced 10.0
```

Example (legal):

```rust
let padding = cx.theme.padding.m;
```

---

## 4. Allowed Exceptions (Explicit and Narrow)

The following cases are **temporarily permitted** and MUST be documented inline.

### 4.1 No-Theme-Access Code Paths

Code paths without access to `Theme`, such as:

* pure hit-testing helpers,
* geometry math utilities,
* constructors executed before theme binding.

**Requirement:**
Such literals must be marked with:

```rust
// UNMAPPED-GEOMETRY: <reason>
// SPEC-HAL-096C
```

---

### 4.2 Tests and Debug Harnesses

* Test-only code
* Debug visualization tools

These may use literals freely but **must not be copy-pasted into production widgets**.

---

### 4.3 Pre-SPEC Legacy Code (Temporary)

Existing literals predating this SPEC are tolerated **only until migrated** under Section 6.

No new usage is allowed.

---

## 5. Token Authority Resolution

When multiple SPECs appear relevant, authority resolves as follows:

1. **SPEC-HAL-002** — absolute invariants
2. **SPEC-HAL-094** — component geometry (heights, structural dimensions)
3. **SPEC-HAL-093** — borders, radii, visual appearance
4. **SPEC-HAL-090** — spacing, padding, grid system
5. **SPEC-HAL-096B** — token definitions and aliases

Lower layers may not override higher ones.

---

## 6. Migration Law

Migration proceeds in **three mechanical phases**.

### Phase 1 — Token Exists

If a token exists:

* **Immediate replacement is mandatory**.
* No UNMAPPED markers allowed.

Example:

```rust
// REQUIRED
let border = cx.theme.borders.b1;
```

---

### Phase 2 — Token Missing but SPEC-Defined

If the geometry is specified by an SPEC but not yet wired into `Theme`:

* Leave the literal,
* Add `UNMAPPED-GEOMETRY`,
* Reference the governing SPEC.

Example:

```rust
// UNMAPPED-GEOMETRY: Scrollbar thumb min size
// Governed by SPEC-HAL-094, token not yet wired
let min_thumb = 22.0;
```

---

### Phase 3 — No SPEC Coverage

If no SPEC defines the value:

* The literal is allowed,
* **But a new SPEC is REQUIRED** before generalization.

Ad-hoc reuse is forbidden.

---

## 7. Review Gate (Binding)

Any PR that:

* touches widget layout,
* touches widget paint,
* touches hit-testing geometry,

**MUST** satisfy all of the following:

1. No new geometry literals unless explicitly exempted.
2. All geometry tokens sourced from `Theme`.
3. Any UNMAPPED geometry clearly marked and justified.
4. PR description references:

    * `SPEC-HAL-096B`
    * `SPEC-HAL-096C`

Failure on any point is a **hard reject**.

---

## 8. Enforcement Model

This SPEC is enforced by:

* code review,
* architectural audits,
* and future static analysis tooling.

Tooling is optional.
The rule is not.

---

## 9. Non-Goals

This SPEC does **not**:

* introduce new tokens,
* redesign widgets,
* alter visuals,
* change density behavior,
* implement scaling.

It only prevents further entropy.

---

## 10. Final Law

> Geometry in Halcyon is **data**, not decoration.
> Data must be named, centralized, and enforced.
> Drift is a bug.
