# SPEC-HAL-218: DropdownMenu Control Invariants

**Status:** Binding  
**Layer:** Control / Action Surface  
**Scope:** DropdownMenu control only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-216, SPEC-HAL-217

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the DropdownMenu control.

A DropdownMenu is an **action selector**, not a value selector.

This SPEC exists to prevent DropdownMenus from drifting into:
- ComboBox-like value controls
- implicit state containers
- navigation or hierarchy authorities

DropdownMenu is **about choosing an action**, not storing a choice.

---

## 1. Conceptual Definition

A **DropdownMenu** is a control that:

- presents a compact trigger (usually a button-like affordance)
- opens a menu of **actions**
- emits **one explicit action** per user selection
- immediately closes after selection

It must **never**:
- retain selected state
- expose a persistent value
- synchronize with application models
- masquerade as a ComboBox

---

## 2. Relationship to Other Controls (Non-Overlapping)

| Control        | Purpose                         | Persistent Value |
|---------------|----------------------------------|------------------|
| ComboBox      | Select **value** from options    | Yes              |
| DropdownMenu  | Select **action** to execute     | No               |
| PopupMenu     | Display action list (surface)    | No               |
| ContextMenu   | Context-bound action list        | No               |

**DropdownMenu ≠ ComboBox.**  
If the result must be remembered, synchronized, or displayed as a value — **DropdownMenu is forbidden**.

---

## Invariant Class A — Ownership & Authority

### INV-A1: No Value Ownership

DropdownMenu does not own:
- selected values
- last chosen items
- remembered state

It emits **actions only**.

---

### INV-A2: No Model Synchronization

DropdownMenu must not:
- read application models
- write persistent state
- participate in FormBinder or value binding

---

## Invariant Class B — Action Semantics

### INV-B1: Exactly-One Action Emission

Each menu activation emits:
- **at most one action**
- synchronously
- deterministically

No multi-select.
No batching.
No chaining.

---

### INV-B2: Immediate Dismissal

After an action is chosen:
- the menu closes immediately
- no lingering state is preserved

---

### INV-B3: No Implicit Defaults

DropdownMenu must not:
- auto-select a default action
- re-emit the last action
- assume a preferred choice

Every emission is explicit.

---

## Invariant Class C — Input & Interaction

### INV-C1: Explicit Trigger

DropdownMenu opens only via:
- pointer activation
- keyboard activation (Enter / Space)

No hover-open by default.

---

### INV-C2: Keyboard Parity

Keyboard interaction must fully mirror pointer behavior:
- open
- navigate
- select
- dismiss

---

### INV-C3: Escape Cancels Silently

Pressing Escape:
- closes the menu
- emits no action

---

## Invariant Class D — Visual & Layout Contract

### INV-D1: Compact Trigger

DropdownMenu exposes a **single compact trigger**:
- typically button-like
- may include label + caret

It is not a list control.

---

### INV-D2: Menu Is a Popup Surface

The opened menu:
- uses PopupMenu infrastructure
- owns no layout outside its surface
- does not reflow surrounding UI

---

### INV-D3: No Value Display

The trigger must not display:
- the last selected action
- any “current choice” state

If such display is needed → wrong control.

---

## Invariant Class E — Menu Structure

### INV-E1: Flat or Hierarchical Actions

DropdownMenu may present:
- flat action lists
- hierarchical submenus

Hierarchy is **structural only**, not semantic state.

---

### INV-E2: Submenus Are Navigational

Submenus:
- exist only to reach actions
- do not represent categories or values

---

## Invariant Class F — Disabled & Availability Semantics

### INV-F1: Disabled Is Absolute

When disabled:
- trigger does nothing
- menu cannot open
- no action emitted

---

### INV-F2: Per-Action Availability

Individual actions may be:
- disabled
- hidden

This affects **visibility and activation only**, not structure.

---

## Invariant Class G — Determinism & Safety

### INV-G1: Deterministic Emission

Given the same:
- menu structure
- input sequence

The same action is emitted.

---

### INV-G2: No Per-Frame Allocation

Idle and steady interaction must not allocate.

---

### INV-G3: Invariant Supremacy

No invariant may be violated for:
- convenience
- platform imitation
- UX expectations

New behavior requires a new SPEC.

---

## 3. Non-Goals (Explicit)

This SPEC does **not** define:
- visual styling
- animation behavior
- menu positioning heuristics
- accessibility narration

These are layered concerns.

---

## 4. Summary (Non-Normative)

- DropdownMenu selects **actions**, never values
- No persistence, no memory, no binding
- One action → one emission → immediate close
- Clear separation from ComboBox semantics

This SPEC locks DropdownMenu as a **pure, honest action selector**.
