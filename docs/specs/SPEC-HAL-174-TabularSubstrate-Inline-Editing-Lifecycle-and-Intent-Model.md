# SPEC-HAL-174: TabularSubstrate — Inline Editing Lifecycle and Intent Model

**Status:** Implemented / Closed
**Layer:** Interaction Semantics / Intent Routing (Non-Mutating)
**Audience:** Engine, Interaction, State
**Depends on:**
SPEC-HAL-009, SPEC-HAL-037, SPEC-HAL-079, SPEC-HAL-140, SPEC-HAL-144, SPEC-HAL-158
**Parent:** SPEC-HAL-164
**Consumes:** SPEC-HAL-167, SPEC-HAL-173
**Addenda:** SPEC-HAL-174A (Deferred Comparator Application During Edit)

---

## 1. Purpose

This SPEC defines the **inline editing model** for TabularSubstrate as a strictly **intent-driven, projection-only**
mechanism.

It exists to prevent:

- editors becoming structural residents
- focus and layout authority bleed
- virtualization-unsafe editor lifetimes
- data mutation inside UI code

Editing is a **mode**, not a structure.

---

## 2. Binding Invariants

**INV-174-1:** Editing is scoped to exactly one CellId at a time (RowId x ColumnId). No multi-edit.

**INV-174-2:** Edit lifecycle is explicit: Idle → Armed → Editing → Commit|Cancel → Idle. No implicit transitions.

**INV-174-3:** Edit state is owned by TabularSubstrate, not by embedded widgets.

**INV-174-4:** CellContentProjection is the only geometry source for editor placement. No direct use of CellProjection, ColumnSlot, or geometry tables.

**INV-174-5:** Scrolling, sorting, column hide, or tree collapse while editing must deterministically resolve to Cancel. No silent behavior.

**INV-174-6:** RowModel identity is never mutated by entering, updating, or exiting edit mode.

**INV-174-7:** No rendering logic inside edit state; rendering remains in paint path using existing primitives.

---

## 3. EditState Enum

```rust
pub enum EditState {
    Idle,
    Armed(EditSession),
    Editing(EditSession),
}
```

`EditSession` stores:
- `editing_cell: CellId` — the cell being edited
- `pre_edit_focus: Option<CellId>` — focus state before edit began

---

## 4. Transition Table

| From     | Event                   | To       | Side Effect                           |
|----------|-------------------------|----------|---------------------------------------|
| Idle     | RequestBeginEdit        | Editing  | Store pre-edit focus, focus → cell    |
| Editing  | RequestCommitEdit       | Idle     | Focus remains on cell, apply pending  |
| Editing  | RequestCancelEdit       | Idle     | Restore pre-edit focus, apply pending |
| Editing  | Scroll                  | Idle     | Cancel semantics (restore focus)      |
| Editing  | Sort intent             | Idle     | Cancel, then apply sort               |
| Editing  | Tree collapse           | Idle     | Cancel, then apply tree               |
| Editing  | Column hide             | Idle     | Cancel semantics                      |
| Editing  | Focus intent            | Idle     | Cancel, then apply focus              |

Armed → Editing is immediate (synchronous). Armed exists as a state for future async permission checks.

---

## 5. Event Triggers

**Idle → Editing:**
- RequestBeginEdit intent (pointer double-click, Enter key, programmatic)

**Editing → Idle (Commit):**
- RequestCommitEdit intent (Enter key, blur commit)

**Editing → Idle (Cancel):**
- RequestCancelEdit intent (Escape key)
- Scroll position change (destabilization)
- Sort intent received (destabilization)
- Tree collapse intent received (destabilization)
- Column hide (destabilization)
- Focus intent to different cell (destabilization)

---

## 6. Commit and Cancel Semantics

**Commit:**
- Edit session ends
- Focus remains on the edited cell
- Pending comparator applied (SPEC-HAL-174A)
- TabularSubstrate does not interpret payload

**Cancel:**
- Edit session ends
- Focus restored to pre-edit state
- Pending comparator applied (SPEC-HAL-174A)
- No data mutation

**Destabilization cancel:**
- Same as Cancel semantics
- Triggering operation proceeds after cancel completes

---

## 7. Authority Rules

While editing:
- Sort/Tree/Focus intents cancel the edit, then apply normally
- Resize intents pass through (do not affect edit)
- Scroll cancels the edit
- Column hide cancels the edit if editing column is hidden

---

## 8. CellContentProjection as Geometry Source

**INV-174-4:** Editor placement uses `content_projections()` exclusively.
No direct access to column slots, geometry tables, or raw cell projection rects.

---

## 9. Forbidden Behaviors

Explicitly illegal:

- mutating data directly from editor
- resizing columns from editor
- forcing row realization
- preventing virtualization
- multi-cell editing
- implicit commit on blur
- nested RequestBeginEdit

---

## 10. Scope Boundaries

This SPEC does **not** define:

- editor widget APIs
- validation rules
- undo/redo semantics
- rendering of editor content

Those are layered elsewhere.

---

## 11. Relationship to Deprecated SPECs

This SPEC replaces inline-edit assumptions from:

- SPEC-HAL-118*
- SPEC-HAL-135*

No semantics are inherited.

---

## 12. Next SPEC

SPEC-HAL-175 will define:

**Cell Focus and Navigation Semantics**

---

## 13. Closure

**This SPEC is fully implemented and closed.**

Implementation: `src/widgets/tabular_view/mod.rs` (EditState enum, lifecycle transitions, destabilization cancel).

Enforcement:

- **INV-174-1:** `EditState::Editing` carries exactly one CellId. Nested BeginEdit rejected.
- **INV-174-2:** Explicit `EditState` enum with Idle/Armed/Editing variants. All transitions via `handle_edit_intent` or `cancel_edit_for_destabilization`.
- **INV-174-3:** `edit_state: EditState` field on TabularSubstrate. No embedded widget state.
- **INV-174-4:** `editing_content_projection()` returns CellContentProjection for the editing cell.
- **INV-174-5:** `handle_intent` cancels edit before applying sort/tree/focus. `set_scroll_y` and `set_column_visible` cancel edit on destabilization.
- **INV-174-6:** No RowModel mutation in any edit lifecycle path.
- **INV-174-7:** No rendering logic in EditState or handle_edit_intent.
- **Deferred comparator:** SPEC-HAL-174A `apply_pending_comparator()` called on both commit and cancel.

Integration tests in `edit_focus_separation.rs` and `edit_lifecycle.rs`.

Future changes to edit lifecycle, multi-cell editing, or editor widget embedding require a new SPEC.

---

## 14. Cross-References

See SPEC-HAL-202 for derived composition analysis tracing edit-session behavior when editor widgets (ComboBox) are embedded in tabular consumers.
