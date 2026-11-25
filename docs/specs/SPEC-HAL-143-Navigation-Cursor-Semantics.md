# SPEC-HAL-143: Navigation Cursor — Engine-Level Semantic Target

**Status:** Binding
**Layer:** Input & Interaction Model
**Scope:** Definition and governance of the navigation cursor as a first-class concept
**Depends on:** SPEC-HAL-038, SPEC-HAL-107, SPEC-HAL-135, SPEC-HAL-140, SPEC-HAL-142
**Constrains:** Engine-level semantics only (no widget policy invention)

---

## SECTION A — Problem Statement

Halcyon currently relies on **focus** to serve multiple roles:

* keyboard navigation
* selection anchoring
* activation targeting
* accessibility reference

This overloading causes:

* focus becoming sticky or ambiguous
* range extension bugs (already observed)
* keyboard behavior differing per widget
* inability to reason about “where the user is” independent of visuals
* hidden coupling between selection, focus, and activation

Modern interaction models require a **stable, explicit navigation target**
that is **not focus**, **not selection**, and **not visual state**.

This SPEC introduces the **Navigation Cursor** as that concept.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Navigation is not focus**
Navigation describes *where movement happens*, not authority.

**PRINCIPLE-2: One cursor, engine-wide**
At any moment, there is at most one navigation cursor per navigation context.

**PRINCIPLE-3: Cursor is logical, not visual**
The cursor may be rendered, but rendering is optional.

**PRINCIPLE-4: Cursor precedes selection**
Selection operations reference the cursor, never implicit focus.

**PRINCIPLE-5: Cursor is explicit state**
No implicit cursor inference is allowed.

---

## SECTION C — Definition: Navigation Cursor

The **Navigation Cursor** is an engine-level state representing:

> “The logical item index / node / position that keyboard navigation operates on.”

It is independent from:

* focus
* selection
* activation
* hover
* capture

---

## SECTION D — Cursor State Contract

**INV-CURSOR-SINGLETON**
At most one navigation cursor exists per navigation scope.

**INV-CURSOR-EXPLICIT**
Cursor changes MUST occur only via explicit navigation actions.

**INV-CURSOR-STABLE**
Cursor position MUST NOT change due to visual or layout changes.

---

## SECTION E — Cursor vs Focus

Focus and cursor are distinct.

**INV-CURSOR-NE-FOCUS**
Cursor movement MUST NOT imply focus change.

**INV-FOCUS-NE-CURSOR**
Focus changes MUST NOT imply cursor movement unless explicitly specified.

Widgets may *align* focus and cursor, but alignment is a policy, not a rule.

---

## SECTION F — Keyboard Navigation Semantics

Keyboard navigation operates on the cursor.

* Arrow keys move the cursor
* Home / End reposition the cursor
* PageUp / PageDown move cursor by structural units

**INV-KBD-MOVES-CURSOR**
Keyboard navigation MUST move the cursor, not selection.

---

## SECTION G — Cursor and Selection

Selection uses the cursor as reference.

**INV-SELECTION-REF-CURSOR**
All keyboard-driven selection operations MUST reference the cursor.

**INV-RANGE-ANCHOR-CURSOR**
Range anchors originate from cursor position, not focus or visuals.

---

## SECTION H — Cursor and Activation

Activation targets the cursor.

**INV-ACTIVATE-CURSOR-TARGET**
Activation commands (Enter, Space) act on the cursor target.

Focus may be granted *after* activation, but not before.

---

## SECTION I — Cursor and Pointer Interaction

Pointer interaction may reposition the cursor.

**INV-POINTER-SETS-CURSOR**
Explicit pointer actions MAY set the cursor.

**INV-HOVER-NO-CURSOR**
Hover MUST NOT move the cursor.

---

## SECTION J — Visual Representation

Cursor visualization is optional.

**INV-CURSOR-NO-RENDER-DEPENDENCY**
Cursor existence MUST NOT depend on rendering.

**INV-RENDER-NO-CURSOR-LOGIC**
Rendering the cursor MUST NOT affect logic.

---

## SECTION K — Testing and Determinism

**INV-CURSOR-TESTABLE**
Cursor behavior MUST be fully testable headlessly.

**INV-CURSOR-DETERMINISTIC**
Given identical inputs, cursor evolution is deterministic.

---

## SECTION L — Forbidden Behaviors

The following are forbidden:

* using focus as cursor
* inferring cursor from selection
* moving cursor during paint/layout
* widget-local cursor hacks
* multiple implicit cursors

---

## SECTION M — Interaction with Existing SPECs

* Focus routing — SPEC-HAL-038
* Selection invariants — SPEC-HAL-107, SPEC-HAL-135
* Focus vs capture — SPEC-HAL-140
* Visual separation — SPEC-HAL-142

SPEC-HAL-143 **introduces a new engine state**: the navigation cursor.

---

## SECTION N — Acceptance Criteria

SPEC-HAL-143 is satisfied when:

* [ ] Cursor exists as explicit engine state
* [ ] Keyboard navigation moves cursor only
* [ ] Selection references cursor
* [ ] Focus is no longer overloaded
* [ ] Range bugs caused by focus collapse disappear

---

## SECTION O — Authoritative Statements

**AUTH-143-1:** “Navigation needs a spine.”
**AUTH-143-2:** “Focus is not where you are.”
**AUTH-143-3:** “Selection follows the cursor, not the eye.”
