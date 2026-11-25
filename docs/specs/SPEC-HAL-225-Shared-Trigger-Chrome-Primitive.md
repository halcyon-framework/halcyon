# SPEC-HAL-225: Shared Trigger Chrome Primitive (Visual-Only)

**Status:** Draft (Binding upon adoption)
**Layer:** Visual Primitives / Control Chrome
**Scope:** Paint/layout helpers only (no semantics)
**Applies To:** DropdownMenu trigger, ComboBox trigger, future button-like triggers
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208

---

## 0. Purpose

Define a shared, reusable trigger "chrome" primitive that unifies visual appearance only for button-like triggers across controls.

This SPEC exists to prevent:
- semantic reuse via "shared trigger widgets"
- behavior leakage between ComboBox / DropdownMenu
- state creeping into visuals

This is chrome, not a control.

---

## 1. Conceptual Definition

TriggerChrome is a pure visual primitive that:
- paints a trigger surface (background, border, focus ring)
- paints label + optional adornments (caret, icon)
- optionally computes intrinsic geometry

TriggerChrome is not a widget.

---

## 2. Public Surface

### Inputs (all explicit)

Provided per paint call by the owning control:
- `rect: RectF`
- `label: &str` (or pre-shaped run)
- state flags (all external):
  - `enabled: bool`
  - `hovered: bool`
  - `pressed: bool`
  - `focused: bool`
  - `open: bool` (visual-only)
- adornments:
  - caret on/off
  - optional icon
- theme access via PaintContext

### Outputs

- Emits paint commands only.
- May optionally return:
  - intrinsic height
  - content rects (label bounds, caret bounds)

---

## 3. Binding Invariants

### INV-225-A1: Zero Semantics Ownership

TriggerChrome must not open, close, select, infer, or decide anything.

---

### INV-225-A2: No Input Handling

No hit-testing, no events, no focus, no keyboard logic.

---

### INV-225-A3: No Persistent State

No cached geometry, flags, or remembered values.

---

### INV-225-A4: No Cross-Control Coupling

Must not depend on ComboBox, DropdownMenu, PopupMenu, or ContextMenu code.

---

### INV-225-A5: Deterministic Rendering

Same inputs → same pixels.

---

### INV-225-A6: Allocation Discipline

No per-frame allocation introduced by TriggerChrome itself.

---

## 4. Non-Goals

TriggerChrome does not define:
- keyboard behavior
- focus policy
- disabled semantics beyond visuals
- menu anchoring

---

## 5. Acceptance Criteria

- DropdownMenu can adopt TriggerChrome without gaining ComboBox semantics.
- ComboBox can later adopt TriggerChrome without behavior change.
- Visual consistency improves without architectural coupling.
