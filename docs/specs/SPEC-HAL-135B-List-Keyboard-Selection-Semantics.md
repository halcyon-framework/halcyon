## SPEC-HAL-135B — List Keyboard Selection Semantics

**Status:** Draft / Binding
**Scope:** List widget — keyboard-driven selection and anchor behavior
**Layer:** Policy + Semantics (NOT event routing, NOT key mapping)
**Depends on:** SPEC-HAL-135, SPEC-HAL-135A
**Constrains:** `list_state.rs`, keyboard handling paths in `list.rs`

---

## 1. Purpose

This SPEC defines **what happens to selection, anchor, focus, and activation when the list is operated via keyboard**.

It answers:

* how arrows move focus
* how Shift / Ctrl modify selection
* how anchor is created and preserved
* when activation is allowed
* how keyboard and pointer semantics remain consistent

This SPEC is authoritative for keyboard behavior.

---

## 2. Core Principles

### INV-LIST-KBD-1 — Keyboard ≠ Pointer

Keyboard navigation **never simulates pointer clicks**.
Outcomes are defined explicitly.

---

### INV-LIST-KBD-2 — Focus drives keyboard intent

Keyboard actions operate relative to:

* focused item (primary)
* persistent anchor (secondary, only for Shift)

---

### INV-LIST-KBD-3 — Anchor rules are identical to pointer semantics

Anchor behavior must match SPEC-HAL-135A:

* Only plain selection establishes anchor
* Range extension uses anchor
* Anchor never mutates implicitly

---

## 3. Focus Navigation (No Selection)

### K.1 Arrow Up / Arrow Down (no modifiers)

**Result:**

* Focus moves by ±1 item
* Selection MUST NOT change
* Anchor MUST NOT change
* Range anchors MUST NOT be established or reused
* Activation = no

---

### K.2 Page Up / Page Down / Home / End (no modifiers)

**Result:**

* Focus jumps accordingly
* Selection MUST NOT change
* Anchor MUST NOT change
* Range anchors MUST NOT be established or reused
* Activation = no

---

## 4. Selection via Keyboard

### K.3 Space (no modifiers)

**Result:**

* Equivalent to **Ctrl/Cmd + Click** on focused item
* Toggle focused item in selection
* Anchor unchanged
* Activation = no

---

### K.4 Enter (no modifiers)

**Precondition:**

* Exactly one item selected OR focused item exists

**Result:**

* Activation of focused item
* Selection unchanged
* Anchor unchanged

---

## 5. Range Selection (Shift)

### K.5 Shift + Arrow Up / Down

**Precondition:**

* Anchor exists

    * If not, anchor is initialized to focused item *before movement*

**Result:**

* Focus moves by ±1
* Selection = range(anchor … focused_item)
* Anchor unchanged
* Activation = no

---

### K.6 Shift + Page / Home / End

Same as K.5, but focus jumps instead of stepping.

---

### INV-LIST-KBD-SHIFT-1 — Shift always implies range semantics

Shift **never toggles**, never activates, never collapses selection.

---

## 6. Modifier Combinations

### K.7 Ctrl/Cmd + Arrow

**Result:**

* Focus moves
* Selection unchanged
* Anchor unchanged
* Activation = no

(This allows navigating without disturbing selection.)

---

### K.8 Ctrl/Cmd + Shift + Arrow

**Result:**

* Same as Shift + Arrow
* Ctrl/Cmd has no effect in range mode

---

## 7. Mode Constraints

### INV-LIST-KBD-MODE-1 — Single selection mode

In `SelectionMode::Single`:

* All range selections collapse to `{focused_item}`
* Anchor is updated only on plain selection
* Shift behaves as navigation + replace

---

## 8. Empty Selection Cases

### K.9 No selection, focus exists

* Shift + Arrow initializes anchor at focus
* Space toggles focused item on
* Enter activates focused item

---

### K.10 No focus

* Arrow keys establish focus on first/last item
* No selection mutation occurs

---

## 9. Forbidden Behaviors

### FORBID-LIST-KBD-1 — Arrow keys must not select implicitly

Selection changes require **explicit** Space or Shift.

---

### FORBID-LIST-KBD-2 — Anchor mutation via navigation

Focus movement alone must never affect anchor.

---

### FORBID-LIST-KBD-3 — Activation via Shift

Any Shift-modified keyboard gesture suppresses activation.

---

### FORBID-LIST-IMPLICIT-EXTEND-1 — Focus as implicit anchor after range completion

Focus MUST NOT be used as implicit anchor after range completion.

After any range selection completes via **Shift+Drag (DragEnd)**:

* Focus is cleared (per INV-RANGE-FINALIZE-1)
* Subsequent Shift+Click MUST NOT fall back to focus
* Anchor only via explicit user action

---

## 10. Consistency Guarantees

### INV-LIST-KBD-CONSISTENCY-1

For any state reachable via pointer gestures, an equivalent state must be reachable via keyboard gestures, and vice versa (excluding drag-only affordances).

---

## 11. Lock Clause

```
SPEC-HAL-135B LOCK:
This SPEC defines all keyboard-driven selection semantics for List.
Keyboard behavior must not diverge from pointer semantics.
Changes require SPEC updates.
```

