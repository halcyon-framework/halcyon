Agreed. Below is the **same SPEC form, tone, and structural rigor**, adapted cleanly to **List selection semantics**. No fluff, no commentary — just binding law.

---

# SPEC-HAL-135: List Selection, Range, and Drag Semantics

**Status:** Design / Binding
**Layer:** Widget behavior (input → selection policy)
**Scope:** `List` widget selection only (mouse + keyboard modifiers)
**Depends on:** SPEC-HAL-052, SPEC-HAL-107, SPEC-HAL-128, SPEC-HAL-131
**Constrains:** `src/widgets/list.rs` behavior only (no engine/router refactors)

---

## SECTION A — Problem Statement

The current `List` selection behavior has **semantic ambiguity** between:

* click vs drag
* Shift+Click vs Shift+Drag
* persistent anchor vs transient drag anchor

This ambiguity has led to:

* regressions in Shift+Click range selection
* accidental coupling between drag and anchor logic
* deferred-selection hacks that guess user intent
* repeated breakage during fixes

What is missing is a **formal, non-overlapping selection model** that:

* cleanly separates gestures
* preserves established desktop UX expectations
* is immune to routing/capture phase details
* is mechanically testable

---

## SECTION B — Design Principles

**PRINCIPLE-1: Gesture separation**
Click, Shift+Click, Ctrl+Click, and Shift+Drag are distinct gestures with non-overlapping semantics.

**PRINCIPLE-2: No intent guessing**
Selection behavior must never be deferred “until we know what the user meant”.

**PRINCIPLE-3: Anchor stability**
Persistent selection anchor must survive drag gestures unchanged.

**PRINCIPLE-4: Drag is explicit**
Drag selection exists only when explicitly enabled by modifier (Shift).

**PRINCIPLE-5: Desktop orthodoxy**
Behavior must match long-established desktop list semantics (Explorer, Finder, Commander-class UIs).

---

## SECTION C — Terminology

**Persistent Anchor**
Logical index stored in list selection state.
Used by Shift+Click and keyboard range selection.

**Drag Anchor**
Transient logical index, valid only during Shift+Drag.
Never persisted beyond drag lifetime.

**Selection Set**
Current set of selected logical item indices.

**Activation**
Item activation (Enter / click-release without drag).

---

## SECTION D — Selection Gestures

### D.1 Plain Click (No Modifiers)

**Trigger**
Primary click without Shift/Ctrl/Cmd.

**Effects**

* Selection set = `{clicked}`
* Persistent anchor = `clicked`
* All previous selection cleared
* Activation allowed
* Drag must NOT start

**INV-LIST-SEL-CLICK-1**
Plain click always collapses selection to one item.

---

### D.2 Ctrl/Cmd Click (Toggle)

**Trigger**
Ctrl/Cmd + primary click.

**Effects**

* Toggle `clicked` in selection set
* Persistent anchor unchanged
* No drag
* No activation side effects

**INV-LIST-SEL-CTRL-1**
Ctrl/Cmd click never affects anchor.

---

### D.3 Shift+Click (Range Select)

**Trigger**
Shift + primary click **without dragging**.

**Effects**

* Selection set = `range(persistent_anchor … clicked)` (inclusive)
* Persistent anchor unchanged
* No drag
* No activation

**INV-LIST-SEL-SHIFT-CLICK-1**
Shift+Click must resolve immediately on PointerDown/Up, without pointer movement.

---

### D.4 Shift+Drag Start

**Trigger**
Shift + primary PointerDown.

**Effects**

* Drag anchor = `clicked`
* Drag active = true
* No selection mutation yet

**INV-LIST-SEL-DRAG-START-1**
Shift+Drag start must NOT mutate selection immediately.

---

### D.5 Shift+Drag Move

**Trigger**
PointerMove while:

* drag active
* Shift held

**Effects**

* Selection set = `range(drag_anchor … hovered)`
* Updates continuously

**INV-LIST-SEL-DRAG-MOVE-1**
Drag selection range is always computed from drag anchor, not persistent anchor.

---

### D.6 Shift+Drag End

**Trigger**
Primary PointerUp after Shift+Drag.

**Effects**

* Drag active = false
* Drag anchor cleared
* **Focused/active item MUST be cleared**
* Selection remains
* No activation

**INV-LIST-SEL-DRAG-END-1**
Drag completion MUST NOT modify persistent anchor.

**INV-RANGE-FINALIZE-1**
After completing any range selection (Shift+Drag):

* Range session ends
* Drag anchor cleared
* Focused/active item MUST be cleared
* Selection preserved

Shift+Drag defines a **range gesture**, not a persistent anchor.
Completion terminates all implicit extension sources.

**INV-RANGE-LIFECYCLE-1**
Range lifecycle is centralized in `finalize_range_gesture()`:

* Called on: DragEnd (Shift+Drag completion)
* Effects:
  * `end_range_session()` clears: `range_session_active`, `range_anchor_id`, `range_base_selection`
  * `focused_id = None` (prevents keyboard-initiated implicit extension)
  * In transactional mode: `anchor_id = None` (prevents Shift+Click fallback)

After `finalize_range_gesture()`, implicit extension is impossible.
Subsequent range gestures must establish their own anchors.

---

## SECTION E — Drag Constraints

**INV-LIST-SEL-DRAG-SHIFT-ONLY**
Drag selection MUST NOT occur without Shift.

**INV-LIST-SEL-NO-DEFER**
Deferred-selection flags or retroactive resolution are forbidden.

**INV-LIST-SEL-NO-ANCHOR-MUTATION**
Persistent anchor must never be modified during drag.

---

## SECTION F — Activation Semantics

**INV-LIST-ACTIVATE-1**
Activation is allowed only if:

* no drag occurred
* no range extension occurred

**INV-LIST-ACTIVATE-2**
Shift-based gestures never activate items.

---

## SECTION G — Keyboard Interaction (Out of Scope but Constrained)

This SPEC does not redefine keyboard selection, but constrains compatibility.

**INV-LIST-KEY-COMPAT-1**
Persistent anchor semantics must remain compatible with existing keyboard range logic.

---

## SECTION H — Forbidden Behaviors

**FORBID-LIST-DEFERRED-SELECTION**
No flags that “wait to see” whether a click becomes a drag.

**FORBID-LIST-ANCHOR-OVERLOAD**
One anchor must not serve both persistent and drag purposes.

**FORBID-LIST-PHASE-DEPENDENCE**
Selection semantics must not depend on event phase (Capture vs Target).

---

## SECTION I — Implementation Constraints

* No engine/router refactors
* No Scroll/Virtualization coupling
* Changes confined to `List` widget logic
* Explicit fields required:

    * `persistent_anchor`
    * `drag_anchor`
    * `drag_active`

---

## SECTION J — Acceptance Criteria

SPEC-HAL-135 is satisfied when:

* [ ] Plain click selects one item and clears others
* [ ] Ctrl/Cmd click toggles without anchor change
* [ ] Shift+Click selects inclusive range immediately
* [ ] Shift+Drag starts range from drag start, not old anchor
* [ ] Drag without Shift does nothing
* [ ] Drag never mutates persistent anchor
* [ ] Releasing mouse after drag does not activate
* [ ] All behaviors survive repeated interaction sequences

---

## SECTION K — Authoritative Statements

**AUTH-135-1**
“Shift+Click and Shift+Drag share a modifier, not a semantic.”

**AUTH-135-2**
“Drag selection is a gesture, not a delayed click.”

**AUTH-135-3**
“Anchors are state, not heuristics.”

---

If you want, next steps are cleanly separable:

* derive **test invariants** directly from Section D (table-driven)
* produce a **Claude task** that literally enumerates violations of this SPEC
* audit Claude’s last patch against each INV one by one

But this SPEC is now the lock.
