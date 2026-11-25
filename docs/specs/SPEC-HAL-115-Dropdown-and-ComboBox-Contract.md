# SPEC-HAL-115

**Dropdown & ComboBox Contract**

**Status:** Design — Approved
**Scope:** Popup-based single-selection controls
**Applies to:** Dropdown, ComboBox, Select
**Non-Goals:** Multi-selection (use List), generic collection semantics (SPEC-114), styling, animation

---

## 1. Problem Statement

Dropdown and ComboBox widgets routinely fail due to:

* unclear popup positioning and anchoring
* inconsistent open/close trigger behavior
* ambiguous selection commit vs cancel semantics
* broken keyboard navigation inside popups
* focus management failures on close

This SPEC defines a **strict interaction contract** for popup-based selection controls, covering the full lifecycle from trigger to selection commit.

---

## 2. Core Definitions

### 2.1 Dropdown

A **Dropdown** is a control that:

* displays a trigger element showing current selection
* opens a popup containing selectable options
* commits selection on explicit user action
* returns focus to trigger on close

---

### 2.2 ComboBox

A **ComboBox** extends Dropdown with:

* a text input field in the trigger
* filtering/search capability
* optional free-form input acceptance

---

### 2.3 Popup

The **popup** is a transient overlay containing the option list.

* exists only while dropdown is open
* positioned relative to trigger
* captures keyboard focus while open

---

## 3. Popup Anchoring

### INV-D1 — Anchor to Trigger

The popup MUST be anchored to the trigger element:

* default: below trigger, left-aligned
* fallback: above trigger if insufficient space below
* fallback: right-aligned if insufficient space left

---

### INV-D2 — Anchor Stability

Once opened, the popup anchor MUST NOT change due to:

* window scroll
* content changes
* selection changes

Re-anchoring requires close + reopen.

---

### INV-D3 — Viewport Constraint

The popup MUST NOT extend outside the window viewport.

If the option list exceeds available space, the popup MUST scroll internally.

---

## 4. Open/Close Triggers

### 4.1 Opening

### INV-O1 — Open Triggers

The dropdown opens on:

* click on trigger
* `Space` or `Enter` when trigger is focused
* `Down Arrow` when trigger is focused (optional)
* programmatic `open()`

---

### INV-O2 — Open Is Idempotent

Attempting to open an already-open dropdown is a no-op.

---

### 4.2 Closing

### INV-C1 — Close Triggers (Commit)

The dropdown closes with selection commit on:

* click on an option
* `Enter` when an option is highlighted
* programmatic `commit()`

---

### INV-C2 — Close Triggers (Cancel)

The dropdown closes without selection change on:

* `Escape`
* click outside popup
* focus loss to unrelated widget
* programmatic `cancel()`

---

### INV-C3 — Close Is Idempotent

Attempting to close an already-closed dropdown is a no-op.

---

## 5. Selection Commit Rules

### INV-S1 — Commit Changes Selection

On commit:

* selection updates to highlighted option
* `on_selection_change` event emits
* popup closes
* focus returns to trigger

---

### INV-S2 — Cancel Preserves Selection

On cancel:

* selection remains unchanged
* no event emits
* popup closes
* focus returns to trigger

---

### INV-S3 — No Implicit Commit

Hovering or highlighting an option MUST NOT commit selection.

Selection commits only on explicit action.

---

## 6. Keyboard Navigation Inside Popup

### INV-K1 — Arrow Navigation

While popup is open:

* `Down Arrow` — highlight next option
* `Up Arrow` — highlight previous option
* `Home` — highlight first option
* `End` — highlight last option

Navigation wraps at boundaries (configurable).

---

### INV-K2 — Type-Ahead (Optional)

Typing characters MAY:

* filter options (ComboBox)
* jump to first matching option (Dropdown)

Behavior must be explicitly configured.

---

### INV-K3 — Activation Keys

* `Enter` — commit highlighted option
* `Space` — commit highlighted option (Dropdown only; in ComboBox, inserts space)
* `Tab` — commit and move focus to next widget
* `Escape` — cancel and close

---

## 7. Focus Management

### INV-F1 — Focus Moves to Popup on Open

When popup opens:

* focus moves to popup or highlighted option
* trigger retains logical association

---

### INV-F2 — Focus Returns to Trigger on Close

When popup closes (commit or cancel):

* focus MUST return to trigger
* focus MUST NOT move to unrelated widget

---

### INV-F3 — Focus Trap

While popup is open:

* `Tab` commits and closes (moves focus forward)
* `Shift+Tab` commits and closes (moves focus backward)
* focus MUST NOT escape popup via arrow keys

---

## 8. Highlight State

### INV-H1 — Highlight Is Distinct from Selection

The **highlight** indicates keyboard position within popup.

* highlight is transient (exists only while open)
* selection is persistent
* highlight MAY differ from current selection

---

### INV-H2 — Initial Highlight

On open:

* highlight initializes to current selection (if any)
* if no selection, highlight initializes to first option

---

## 9. ComboBox-Specific Rules

### INV-CB1 — Text Input Integration

ComboBox text field:

* reflects current selection when closed
* allows free typing when open
* filters option list based on input (if configured)

---

### INV-CB2 — Free-Form Acceptance (Optional)

ComboBox MAY accept values not in the option list.

If enabled:

* `Enter` on non-matching text creates new selection
* if disabled, non-matching input is rejected on commit

---

## 10. Disabled State

### INV-D4 — Disabled Is Absolute

When disabled:

* trigger MUST NOT open popup
* no keyboard interaction
* no focus acquisition

---

## 11. Programmatic Control

### INV-PROG1 — Explicit APIs

Dropdown/ComboBox MUST expose:

* `open()` / `close()`
* `commit()` / `cancel()`
* `set_selection(index)` / `get_selection()`
* `set_options(list)`

---

## 12. Responsibility Boundaries

**Dropdown/ComboBox trigger** owns:

* open/close state
* current selection display
* keyboard focus acceptance

**Popup** owns:

* option list display
* highlight state (transient)
* keyboard navigation within popup

**Dropdown/ComboBox** MUST NOT own:

* visual rendering (delegated to painter)
* popup positioning math (delegated to anchor system)
* option data (delegated to application)

**Delegated responsibilities:**

* Anchor system: popup positioning and viewport constraint
* Focus system: focus trap management
* SPEC-HAL-114: selection semantics for option list

---

## 13. Event Emission

### INV-E1 — Events

Dropdown emits:

* `on_open`
* `on_close`
* `on_selection_change(new_index)` — only on commit with changed selection

---

## 14. Forbidden Behavior

Dropdown/ComboBox MUST NOT:

* commit on hover
* commit on highlight change
* leave popup open on focus loss
* fail to return focus to trigger
* change selection on cancel
* allow multiple popups open simultaneously

---

## 15. Test Requirements

Any Dropdown/ComboBox implementation MUST include tests for:

* open via click, keyboard, programmatic
* close via commit and cancel paths
* keyboard navigation inside popup
* focus return on close
* escape cancellation
* outside-click cancellation
* disabled state
* type-ahead (if supported)

---

## 16. Reference Implementation

Planned widgets: `Dropdown`, `ComboBox`, `Select`

Related SPECs:

* SPEC-HAL-112 (Button Interaction — activation timing)
* SPEC-HAL-114 (List Selection — option list semantics)

---

## 17. Final Note

Dropdowns are **transient selection state machines** with strict focus contracts.

Any implementation that leaks focus, commits on hover, or fails to restore focus is **incorrect by definition**.

---

## Validation Checklist

- [ ] Scope is non-overlapping with other SPECs
- [ ] Ownership boundaries are explicit
- [ ] All cross-SPEC references are valid
- [ ] Forbidden behaviors are explicitly listed
- [ ] Behavior is mechanically testable
