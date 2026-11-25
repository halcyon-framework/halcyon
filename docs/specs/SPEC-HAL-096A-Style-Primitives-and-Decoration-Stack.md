# SPEC-HAL-096A — Style Primitives & Decoration Stack

**Status:** Draft  
**Layer:** Theme / Visual Semantics  
**Depends on:** SPEC-HAL-096, SPEC-HAL-011  
**Affects:** All widgets with visual decoration  
**Scope:** Specification only (NO implementation)

---

## 1. Problem Statement

Halcyon widgets currently implement visual decoration (borders, focus rings, hover effects)
*implicitly* and *ad-hoc* inside widget paint logic.

Although the underlying patterns are consistent, they are:
- duplicated across widgets
- not formally named
- not centrally controlled
- impossible to reason about as a system

This SPEC formalizes **style primitives** already in use and introduces a
**Decoration Stack** to make decoration explicit, declarative, and auditable.

No new rendering capabilities are introduced.

---

## 2. Non-Goals

This SPEC explicitly does **not**:

- introduce shadows or elevation geometry
- introduce new scene primitives
- modify renderer or GPU code
- redesign widget APIs
- invent new colors or themes
- change visual appearance

This is a **naming and authority SPEC**, not a visual redesign.

---

## 3. Observed Existing Patterns (Baseline)

From code audit:

- Borders implemented via **2-rect inset technique**
- Focus implemented via **expanded rect at fixed Z layer**
- Hover implemented via **premultiplied alpha overlay**
- Depth implemented via **color only**, not geometry

These patterns are already stable and correct.
They are simply undocumented and unowned.

---

## 4. Style Primitives

### 4.1 BorderStyle

Represents a visual border using the established inset pattern.

**Semantic Definition:**
- A border is not a stroke.
- It is an *outer rect* + *inner inset rect*.

**Fields:**
- `thickness: f32`
- `color: Color`
- `radius: CornerRadius`
- `inset_radius_delta: f32`  
  (usually equals thickness)

**Rendering Contract:**
1. Draw outer rect with `color`
2. Draw inner rect inset by `thickness`
3. Inner radius = max(0, outer_radius − inset_radius_delta)

**Forbidden:**
- Widgets may not hand-roll inset borders.

---

### 4.2 FocusStyle

Represents keyboard focus indication.

**Semantic Definition:**
- Focus is a visual *aura*, not a border mutation.

**Fields:**
- `expansion: f32` (rect growth in all directions)
- `color: Color`
- `radius: CornerRadius`
- `layer: ZIndex` (must be `LAYER_FOCUS`)

**Rendering Contract:**
1. Expand base rect by `expansion`
2. Draw at `layer`
3. Must not overlap content layer

**Forbidden:**
- Focus drawn at content or background layers
- Focus replacing border color

---

### 4.3 HoverStyle

Represents pointer hover feedback.

**Semantic Definition:**
- Hover is a transient overlay, not a stateful background.

**Fields:**
- `overlay_color: Color` (premultiplied)
- `max_alpha: f32`
- `blend_mode: PremultipliedAlpha`
- `animation: HoverAnimationSpec` (existing)

**Rendering Contract:**
1. Draw overlay rect above background
2. Below focus ring
3. Alpha controlled by hover animation

**Forbidden:**
- Hover implemented via background replacement
- Non-premultiplied alpha

---

### 4.4 ElevationStyle (Declared, Empty)

**Status:** Declared but intentionally unused.

**Semantic Definition:**
- Elevation describes perceived depth.
- In Halcyon, depth is currently color-based only.

**Note:**
- No shadows exist.
- ElevationStyle exists to prevent future ad-hoc shadow hacks.

---

## 5. DecorationStack

### 5.1 Definition

A DecorationStack is an **ordered application** of style primitives.

**Order (fixed):**
1. Background
2. BorderStyle
3. HoverStyle
4. FocusStyle

Widgets may omit entries but may not reorder them.

---

### 5.2 Ownership Rules

- DecorationStack is resolved **before** widget content.
- Widgets provide:
  - geometry
  - state flags (focused, hovered, disabled)
- Styles decide **how** visuals are drawn.

---

## 6. Widget Mapping (Current State)

| Widget        | Border | Focus | Hover | Notes |
|--------------|--------|-------|-------|-------|
| Button       | ✔      | ✔     | ✔     | Fully style-driven |
| Checkbox     | ✔      | ✔     | ✖     | Checkmark is content |
| RadioButton  | ✔      | ✔     | ✖     | Selection dot is content |
| TextField    | ✔      | ✔     | ✖     | Selection & caret are content |
| TextArea     | ✔      | ✔     | ✖     | Same as TextField |
| Panel        | ✔      | ✖     | ✖     | Static decoration |
| Scrollbar    | Partial| ✖     | ✔     | Thumb hover only |
| Divider      | Border | ✖     | ✖     | Single rect |
| Label        | ✖      | ✖     | ✖     | Typography only |

---

## 7. Explicitly Widget-Owned Rendering (Non-Decorative)

The following must **never** be abstracted as styles:

- Checkbox checkmark geometry
- RadioButton selection dot
- Text selection highlights
- Caret rendering
- Scrollbar hit-test expansion

These are semantic content, not decoration.

---

## 8. Forbidden Patterns (Binding)

After adoption of this SPEC:

- Widgets must not draw:
  - borders manually
  - focus rings manually
  - hover overlays manually
- Widgets must not:
  - change border color on focus
  - encode decoration logic inline
- All decoration must flow through:
  - BorderStyle
  - FocusStyle
  - HoverStyle
  - DecorationStack

Violations are architectural errors.

---

## 9. Migration Strategy (Future SPEC)

This SPEC defines **what exists**.

A follow-up SPEC will define:
- Rust structs
- Theme integration
- Mechanical refactor steps

No code changes are permitted under this SPEC.

---

## 10. Summary

This SPEC:
- names what already exists
- centralizes visual authority
- prevents future divergence
- prepares the codebase for safe evolution

No pixels change.
No behavior changes.
Only structure and ownership become explicit.
