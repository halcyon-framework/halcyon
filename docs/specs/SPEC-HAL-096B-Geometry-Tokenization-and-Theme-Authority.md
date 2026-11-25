# SPEC-HAL-096B — Geometry Tokenization, Literal Boundaries, and Theme Authority

**Status:** Draft  
**Author:** JC  
**Layer:** Core / Design System / Theme–Layout Boundary  
**Supersedes:** None  
**Depends on:**
- SPEC-HAL-002
- SPEC-HAL-008
- SPEC-HAL-010
- SPEC-HAL-011
- SPEC-HAL-012
- SPEC-HAL-090
- SPEC-HAL-091
- SPEC-HAL-092 / SPEC-HAL-092A
- SPEC-HAL-093 / SPEC-HAL-093A
- SPEC-HAL-094
- SPEC-HAL-096A
- SPEC-HAL-141
- SPEC-HAL-145

---

## 1. Purpose

This SPEC defines **where geometry values MUST be tokenized**, **where literals are forbidden**, and **where literals are explicitly allowed or required**.

It resolves long-standing ambiguity between:
- layout geometry,
- visual decoration,
- interaction hit-testing,
- and theme-driven styling.

This document is **normative**. Violations are architectural errors.

---

## 2. Core Principle

> **Geometry has ownership. Tokens do not replace ownership.**

Not all numbers are equal.

A value may be:
- **semantic** → must be tokenized
- **visual** → must come from theme
- **structural** → must remain literal
- **interaction-bound** → must remain literal
- **constructor-bound** → cannot access theme

This SPEC classifies them.

---

## 3. Authority Layers (Non-Negotiable)

From SPEC-HAL-002 and SPEC-HAL-141:

1. **Layout Engine (HAL-008)**  
   Owns constraints, flow, size resolution.

2. **Widget Runtime (HAL-010 / HAL-011)**  
   Owns widget identity and lifecycle.

3. **Theme (HAL-012, 090, 093, 094, 096A)**  
   Owns *semantic visual intent*, not layout math.

4. **Interaction / Hit-Testing (HAL-009, HAL-099)**  
   Owns precision and spatial thresholds.

Tokens may **never** cross these authority boundaries implicitly.

---

## 4. Mandatory Tokenization (MUST)

The following **must use tokens** and **must not hardcode literals**.

### 4.1 Spacing & Padding

**Authority:** SPEC-HAL-090

Applies to:
- spacing between sibling widgets
- internal padding of containers
- section separation

Tokens:
- `spacing.{xs,s,m,l,xl,x2l}`
- `padding.{xs,s,m,l}`

Rule:
> Widgets MUST NOT hardcode spacing/padding values in layout or paint paths when theme access is available.

**Exception:** `Panel.padding` is instance-owned — see §4.4.

---

### 4.2 Borders & Radii (Visual)

**Authority:** SPEC-HAL-093

Applies to:
- border thickness
- corner radii
- decorative outlines

Tokens:
- `borders.{b0,b1,b2}`
- `radii.{r0,xs,s,m,l}`

Rule:
> Visual borders and radii MUST come from theme tokens.

**Exception:** `Panel.border` is instance-owned — see §4.4.

---

### 4.3 Component Heights (When Defined)

**Authority:** SPEC-HAL-094

Applies to:
- buttons
- inputs
- list rows
- standard controls **only if specified**

Rule:
> If SPEC-HAL-094 defines a component height, it MUST be used.
> If not defined, literals are allowed but MUST be flagged as UNMAPPED.

---

### 4.4 Container Padding & Border (Instance-Owned) — Panel

**Authority:** SPEC-HAL-010 (widget instance ownership)

Applies to:
- `Panel.padding` / `Panel.border`

The `Panel` container's `padding` and `border` are **owned by the instance** — set via
`with_padding` / `with_border`, defaulting to the constructor constants
(`Padding::all(8.0)`, `Borders::new(1.0, Color::TRANSPARENT)`). `Panel::layout` reads
`self.padding` / `self.border` directly and **does not** sync them from theme tokens.

Rule:
> `Panel` padding and border are instance-owned and MUST NOT be theme-synced in `layout()`.
> A consumer MUST be able to zero them (e.g. `with_padding(0.0)` for a full-bleed background
> root); a theme-forced inset that cannot be overridden is forbidden.

This is a deliberate, named exception to §4.1 and §4.2, tracked in code as
`INV-GEOM-SYNC-PANEL` (`src/widgets/panel.rs`). `Panel.margin` (already instance-owned) and
`Panel.radius` are unaffected by this entry.

---

## 5. Explicitly Forbidden Tokenization (MUST NOT)

### 5.1 Typography Substitution

**Authority:** SPEC-HAL-092

Rule:
> Typography tokens are NOT geometry tokens.

Examples (FORBIDDEN):
- using `theme.typography.font_size` as padding
- deriving row height from font size
- replacing literal text size with unrelated typography role

Typography roles are semantic and **not interchangeable**.

---

### 5.2 Layout Constraint Math

**Authority:** SPEC-HAL-008

Examples:
- flex distribution math
- constraint resolution
- overflow clipping math

Rule:
> Layout math MUST remain literal and algorithmic.
> Tokens must not influence constraint resolution.

---

### 5.3 Hit-Testing Thresholds

**Authority:** SPEC-HAL-009 / HAL-099

Examples:
- pointer tolerance
- caret margins
- resize hit zones

Rule:
> Interaction geometry MUST remain literal unless explicitly lifted into an SPEC.

---

## 6. Constructor Boundary Rule

**Authority:** SPEC-HAL-010

Facts:
- Constructors do NOT have theme access.
- Theme is runtime-contextual.

Rule:
> Geometry values used in constructors MAY remain literal.
> Such literals MUST NOT be silently replaced.
> They MUST be flagged for future refactor via runtime initialization if required.

---

## 7. Allowed Transitional State

This SPEC explicitly allows **temporary mixed state**:

- Tokens exist
- Some widgets still use literals
- No visual drift is introduced

Conditions:
- Numeric equivalence MUST be preserved
- All unmapped literals MUST be documented
- No behavioral or layout change is allowed

---

## 8. Prohibited Behaviors

The following are **architectural violations**:

- Introducing new hardcoded spacing where a token exists
- Replacing literals without SPEC authority
- Rebinding typography roles to geometry
- Introducing density scaling without SPEC-HAL-091 activation
- Theme-driven layout reflow
- Silent visual drift

---

## 9. Enforcement

Violations of this SPEC:
- block refactors
- invalidate PRs
- require rollback

This SPEC does **not** mandate immediate cleanup.
It mandates **correct direction** and **clear boundaries**.

---

## 10. Summary

- Tokens are **semantic**, not universal
- Geometry has ownership
- Literals are sometimes correct
- Theme is not layout
- Typography is not geometry
- Constructors are special
- Hit-testing is sacred

This SPEC locks the rules so future work stops oscillating.

---

**End of SPEC-HAL-096B**
