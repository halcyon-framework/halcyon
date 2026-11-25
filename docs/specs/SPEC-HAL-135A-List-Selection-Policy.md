## SPEC-HAL-135A — List Selection Policy

**Status:** Draft / Binding
**Scope:** List widget — selection *meaning and outcomes*
**Layer:** Policy (NOT mechanics, NOT event routing)
**Depends on:** SPEC-HAL-135
**Constrains:** `list_state.rs`, selection-related logic only

---

## 1. Purpose

This SPEC defines **what list selection means**, independently of *how* it is produced.

It is authoritative for:

* selection contents
* anchor meaning
* interaction with focus
* interaction with activation
* modifier semantics (Shift / Ctrl / Cmd)
* empty-space behavior

It intentionally does **not** describe pointer or keyboard mechanics.

---

## 2. Core Concepts

### 2.1 Selection Set

* The selection is a **set of logical item IDs**
* Order is irrelevant
* Empty set is valid

### 2.2 Persistent Anchor

* The anchor is a **single logical item ID**
* Used **only** as the reference point for range extension
* The anchor is **stable** unless explicitly replaced by a plain click

---

## 3. Global Invariants

### INV-LIST-POL-1 — Selection is authoritative state

Selection state lives exclusively in `ListState`.
Visual state, hover, drag state, or focus must not be treated as selection.

---

### INV-LIST-POL-2 — Anchor is not selection

The anchor:

* MAY point to an unselected item
* MUST NOT imply selection by itself
* MUST NOT be mutated implicitly

---

### INV-LIST-POL-3 — Single vs Multi mode

* `SelectionMode::Single`:

    * Selection size ≤ 1
    * Any operation that would produce a range collapses to the terminal item
* `SelectionMode::Multi`:

    * Full semantics apply

---

## 4. Click Semantics (Outcomes Only)

### D.1 Plain Click (no modifiers)

**Result:**

* Selection = `{clicked_item}`
* Anchor = `clicked_item`
* Focus = `clicked_item`
* Activation = **allowed**

---

### D.2 Ctrl/Cmd + Click

**Result:**

* Toggle `clicked_item` in selection
* Anchor = **unchanged**
* Focus = `clicked_item`
* Activation = **not allowed**

---

### D.3 Shift + Click

**Precondition:**

* Anchor exists

    * If not, anchor defaults to focused item

**Result:**

* Selection = range(anchor … clicked_item)
* Anchor = **unchanged**
* Focus = `clicked_item`
* Activation = **not allowed**

---

## 5. Drag Semantics (Policy-Level)

(Concrete mechanics defined in SPEC-HAL-135)

### D.4 Shift + Drag

**Result:**

* Selection = range(drag_anchor … current_item)
* Persistent anchor = **unchanged**
* Focus follows drag end
* Activation = **never allowed**

---

### INV-LIST-POL-DRAG-1 — Drag never mutates anchor

No drag operation may change the persistent anchor.

---

## 6. Activation Rules

### INV-LIST-POL-ACT-1 — Activation requires non-range gesture

Activation is allowed **only if**:

* No range extension occurred
* No Shift modifier was involved
* No drag occurred

---

### INV-LIST-POL-ACT-2 — Shift-based gestures never activate

Any gesture involving Shift **suppresses activation**, regardless of outcome.

---

## 7. Empty Space Behavior

### D.5 Click on empty space

**Result:**

* Selection cleared
* Anchor cleared
* Focus cleared
* Activation = N/A

(This is policy; routing mechanics are out of scope.)

---

## 8. Forbidden Behaviors

### FORBID-LIST-POL-1 — Implicit anchor mutation

Anchor must never change due to:

* Ctrl/Cmd click
* Drag
* Focus movement
* Selection clearing
* Keyboard navigation (covered later in 135B)

---

### FORBID-LIST-POL-2 — Selection without explicit rule

Every selection mutation must correspond to one of:

* D.1
* D.2
* D.3
* D.4
* D.5

No ad-hoc mutations.

---

### FORBID-LIST-POL-3 — Activation as side effect

Selection changes must not trigger activation implicitly.

---

## 9. Relationship to Other SPECs

* **SPEC-HAL-135**
  Defines *how* pointer gestures produce selection changes.

* **SPEC-HAL-135B (planned)**
  Will define keyboard-driven selection and anchor behavior.

---

## 10. Lock Clause

```
SPEC-HAL-135A LOCK:
This SPEC defines the authoritative meaning of list selection.
No selection behavior may be changed without updating this document.
```
