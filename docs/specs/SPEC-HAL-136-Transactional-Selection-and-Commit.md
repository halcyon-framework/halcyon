# SPEC-HAL-136: Transactional Selection and Explicit Commit Semantics

**Status:** Design / Binding
**Layer:** Interaction semantics (widget-level, non-render)
**Scope:** `List`, `Table`, future multi-item selectors
**Depends on:** SPEC-HAL-135
**Constrains:** Selection logic, keyboard/mouse interaction only (no rendering, no layout)

---

## SECTION A — Problem Statement

SPEC-HAL-135 defines **immediate selection semantics**:

* click mutates selection
* modifiers extend or toggle
* selection is destructive by default

This model is correct for **simple scenarios**, but fails for **complex workflows**:

* accidental clicks destroy work
* keyboard navigation + selection conflicts
* bulk operations require re-selection after errors
* checkbox-driven lists fight click-to-select semantics
* there is no notion of “working set” vs “final intent”

We need a **non-destructive selection model** that:

* allows incremental construction of a selection
* survives navigation, focus, and activation
* requires **explicit user commit**
* coexists with SPEC-HAL-135 without altering it

---

## SECTION B — Design Principle

**PRINCIPLE-1: Selection can be a working set, not an action**
Selection is sometimes preparatory state, not an immediate command.

**PRINCIPLE-2: Commit must be explicit**
No implicit commit on click, blur, or activation.

**PRINCIPLE-3: No silent loss of work**
Accidental pointer actions must not destroy accumulated selection.

**PRINCIPLE-4: Orthogonal to selection mechanics**
Transactional behavior layers on top of SPEC-HAL-135, not instead of it.

---

## SECTION C — Terminology

**Working Selection**
The mutable set being edited during a transactional session.

**Committed Selection**
The finalized set emitted by an explicit commit action.

**Commit Action**
A user-triggered event that finalizes the working selection.

**Transactional Selection Mode**
A selection interaction model where selection mutation is decoupled from pointer activation.

---

## SECTION C.1 — Transaction Lifecycle

**INV-SEL-TXN-LIFE-1**
Entry into transactional mode MUST be explicit.

**INV-SEL-TXN-LIFE-2**
Termination of a transactional session MUST occur only via:

* explicit commit
* explicit cancel
* explicit mode switch

**INV-SEL-TXN-LIFE-3**
Transactional sessions MUST NOT terminate implicitly due to:

* focus loss
* activation
* navigation
* overlay open/close

**INV-SEL-TXN-LIFE-4**
Nested transactional sessions are forbidden.

**INV-SEL-TXN-LIFE-5**
At most one active transactional session may exist per widget instance.

---

## SECTION D — Selection Interaction Models

Introduce a new orthogonal axis:

```rust
enum SelectionInteractionModel {
    Immediate,        // SPEC-HAL-135
    Transactional,    // SPEC-HAL-136
}
```

This is **independent of**:

* `SelectionMode::{None, Single, Multi}`
* ownership or delegation of selection state

---

## SECTION E — Behavioral Semantics

### E.1 Immediate Mode (Reference)

Defined fully by SPEC-HAL-135.
This SPEC does not modify it.

---

### E.2 Transactional Mode (New)

#### Pointer Semantics

**INV-SEL-TXN-POINTER-1**
Plain click MUST NOT mutate working selection.

**INV-SEL-TXN-POINTER-2**
Click MAY:

* move focus
* activate item
* trigger non-selection actions

#### Keyboard Semantics

**INV-SEL-TXN-KBD-1**
Arrow keys move focus only.

**INV-SEL-TXN-KBD-2**
Space toggles focused item in working selection.

**INV-SEL-TXN-KBD-3**
Shift + arrows extends working selection range.

**INV-SEL-TXN-KBD-4**
Ctrl/Cmd modifies working selection without affecting anchor semantics.

**INV-SEL-TXN-KBD-5**
Enter MUST NOT commit unless explicitly bound as a commit action.

#### Drag Semantics

**INV-SEL-TXN-DRAG-1**
Drag selection edits working selection, not committed state.

**INV-SEL-TXN-DRAG-2**
Drag MUST NOT change persistent anchor semantics defined in SPEC-135.

---

### E.3 Transactional Range Semantics (Binding)

#### Range Addition

**INV-TRX-ADD-1**
In transactional mode:

* Shift+Drag **adds** to working selection
* Shift+Click **adds** to working selection

Selection is never replaced by range operations in transactional mode.

---

#### Anchor Cleanup

**INV-TRX-ANCHOR-1**
After DragEnd in transactional mode:

* `anchor_id == None`
* `range_anchor_id == None`

All anchors MUST be cleared when a range operation completes.

---

#### Anchor Establishment

**INV-TRX-ANCHOR-2**
In transactional mode:

* Shift+Click MUST NOT fall back to focus
* Anchor MUST be established only via explicit user action
* If no anchor exists, Shift+Click uses the clicked index as anchor (degenerate range of length 1)

---

#### Range Finalization (Cross-Reference)

**INV-RANGE-FINALIZE-1** (defined in SPEC-HAL-135)

After completing any range selection (Shift+Drag):

* Range session ends
* Drag anchor cleared
* Focused/active item MUST be cleared
* Selection preserved

DragEnd clears focus, preserves selection. This invariant applies in both immediate and transactional modes.

---

## SECTION F — Commit Semantics

### F.1 Commit Action

Commit is explicit and user-driven.

Possible commit triggers (non-exclusive):

* Enter
* Button / command
* Context action
* Programmatic call

**INV-SEL-TXN-COMMIT-1**
Without an explicit commit, working selection has no external effect.

**INV-SEL-TXN-COMMIT-2**
Commit produces a snapshot of the working selection.

**INV-SEL-TXN-COMMIT-3**
Post-commit behavior MUST be explicitly defined by the caller:

* clear working selection
* keep selection
* transition widget state

### F.2 State Coexistence

**INV-SEL-TXN-STATE-1**
Working selection and committed selection are distinct state containers.

**INV-SEL-TXN-STATE-2**
Committed selection is immutable during an active transactional session.

**INV-SEL-TXN-STATE-3**
Commit atomically replaces committed selection with a snapshot of working selection.

**INV-SEL-TXN-STATE-4**
Cancel restores working selection to exactly match last committed selection.

### F.3 Ordering Rules

**INV-SEL-TXN-ORDER-1**
Activation is distinct from commit.

**INV-SEL-TXN-ORDER-2**
Commit is distinct from activation.

**INV-SEL-TXN-ORDER-3**
No implicit coupling exists between activation and commit.

**INV-SEL-TXN-ORDER-4**
Any key binding that triggers both activation and commit MUST be explicitly defined by the widget configuration.

---

## SECTION G — Cancellation Semantics

**INV-SEL-TXN-CANCEL-1**
Transactional selection MUST be cancellable.

Cancel effects:

* discard working selection
* restore last committed state (if any)
* leave focus unchanged

---

## SECTION H — Relationship to Checkboxes and Item Controls

**INV-SEL-TXN-CONTROL-1**
Interactive child controls (e.g. checkbox) MAY map to selection toggling.

**INV-SEL-TXN-CONTROL-2**
Control activation MUST NOT implicitly commit selection.

**INV-SEL-TXN-CONTROL-3**
Control-driven selection edits MUST obey the same transactional rules as keyboard and drag.

Checkboxes become **editors of the working set**, not commit triggers.

---

## SECTION I — Forbidden Behaviors

**FORBID-SEL-TXN-IMPLICIT-COMMIT**
Blur, focus loss, click, or activation MUST NOT commit.

**FORBID-SEL-TXN-DESTRUCTIVE-CLICK**
Click MUST NOT clear working selection.

**FORBID-SEL-TXN-HIDDEN-STATE**
Transactional state must be observable and queryable.

---

## SECTION J — API Surface (Minimal)

No new widget APIs required beyond:

* querying working selection
* commit / cancel hooks
* interaction model configuration

Selection ownership remains unchanged.

---

## SECTION K — Acceptance Criteria

### K.1 Performance Guard

**INV-SEL-TXN-PERF-1**
Transactional selection MUST NOT trigger layout, paint, or widget tree rebuilds.

### K.2 Completion Criteria

SPEC-HAL-136 is satisfied when:

* [ ] Selection can be built incrementally without loss
* [ ] Click navigation does not destroy selection
* [ ] Space toggles focused items
* [ ] Commit is explicit and observable
* [ ] Cancel restores clean state
* [ ] SPEC-HAL-135 behavior remains untouched

---

## SECTION L — Authoritative Statements

**AUTH-136-1:**
“Transactional selection treats selection as state, not as action.”

**AUTH-136-2:**
“Commit must be explicit, never inferred.”

**AUTH-136-3:**
“Accidental input must not destroy intentional work.”
