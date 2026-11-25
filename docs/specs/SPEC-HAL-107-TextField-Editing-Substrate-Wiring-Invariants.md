### SPEC-HAL-107: TextField ↔ Editing Substrate Wiring Invariants

**Status:** Implemented
**Scope:** TextField widget only
**Purpose:** Lock down *how* TextField uses the already-implemented editing substrate. No new algorithms. No new abstractions. No redesign.

---

## 1. Motivation

The editing substrate (SPEC-HAL-052) is complete and correct.
TextField integration (SPEC-HAL-105) is functionally incomplete due to missing **wiring invariants**, not missing logic.

This SPEC exists to:

* Eliminate ambiguity in ownership of cursor, selection, and edits
* Prevent regression into char-based logic
* Make selection, navigation, and editing behavior mechanically enforceable

This SPEC is **not optional**. Any TextField behavior violating it is a bug.

---

## 2. Non-Goals

This SPEC explicitly does **not**:

* Redesign selection behavior
* Add IME, clipboard, or undo/redo
* Change visual styling
* Modify substrate algorithms
* Introduce new public APIs

---

## 3. Terminology

* **Anchor**: Fixed end of a selection
* **Head**: Moving end of a selection
* **Cluster**: Atomic grapheme unit as defined by substrate
* **Substrate**: `src/core/editing.rs`
* **TextField**: `src/widgets/text_field.rs`

---

## 4. Absolute Invariants (Non-Negotiable)

### 4.1 Cursor & Selection Ownership

1. **TextField MUST NOT compute cursor movement itself**
2. **TextField MUST NOT reason about UTF-8 chars**
3. **All cursor movement MUST go through substrate functions**
4. **All document mutation MUST go through substrate functions**

Any violation = SPEC breach.

---

## 5. Selection Model Invariants

### 5.1 Anchor Rules

The selection anchor is set **only** in the following cases:

* PointerDown (mouse click)
* Focus acquisition
* Explicit selection clear

The anchor:

* MUST NOT change during keyboard navigation
* MUST NOT be reset on Shift+Arrow
* MUST persist across layout rebuilds

### 5.2 Head Rules

* The head moves via substrate navigation calls
* The head is the only end that moves during selection extension
* Head position MUST always correspond to a valid cluster boundary

---

## 6. Keyboard Navigation Contract

### 6.1 Arrow Keys

| Input | Required Call                               |
| ----- | ------------------------------------------- |
| Left  | `move_left_collapse` / `move_left_select`   |
| Right | `move_right_collapse` / `move_right_select` |
| Home  | `move_home` / `move_home_select`            |
| End   | `move_end` / `move_end_select`              |

Rules:

* Shift decides `_select` vs `_collapse`
* TextField MUST NOT modify cursor/selection directly

### 6.2 Forbidden Behavior

* Clearing selection on arrow navigation
* Recomputing cursor from prefix shaping
* Adjusting anchor implicitly

---

## 7. Editing Operations Contract

### 7.1 Text Insertion

All text insertion MUST go through:

* `apply_insert_op`

Rules:

* If selection is non-empty, insertion replaces selection
* Cursor update is handled by substrate
* TextField MUST NOT post-adjust cursor

### 7.2 Deletion

All deletion MUST go through:

* `apply_edit_op(TextEditOp::Backspace)`
* `apply_edit_op(TextEditOp::Delete)`

Rules:

* Deletion is cluster-safe
* Selection deletion is handled by substrate
* TextField MUST NOT drain strings manually

---

## 8. Mouse Interaction Contract

### 8.1 Pointer Down

* Set cursor via `hit_test_x_y`
* Clear selection **unless Shift is held**
* Set anchor = cursor.pos

### 8.2 Pointer Move (Drag)

* Update selection head only
* Anchor remains unchanged
* Uses substrate cluster positions

### 8.3 Pointer Up

* Finalize selection
* No mutation after release

---

## 9. Layout Cache Invariants

The layout cache MUST be invalidated when **any** of the following change:

* Document content
* Cursor position
* Selection head
* Selection anchor

Selection rendering MUST use cluster boundaries from the cache.

---

## 10. Rendering Invariants

### 10.1 Selection Rendering

* Selection rectangles computed from substrate clusters only
* No prefix shaping
* No char index math

### 10.2 Caret Rendering

* Caret is rendered **only** when selection is empty
* Caret X position derived from `visual_cluster_for_pos`
* Caret visibility logic MUST NOT mutate selection

**INV-107-TA-PROJECTION.** Solely for the qualified TextArea presentation defined by HAL-109 §17, the canonical caret destination MUST continue to derive from the current editing substrate and current visual-cluster boundary. A separately qualified immutable painted-x projection MAY interpolate toward that destination. This exception changes only the painted caret x; it grants no alternative cursor, selection, hit-test, navigation, preferred-x or scroll authority. Outside that qualification, the canonical caret is painted under the existing rules.

**FORBID-107-TA-PROJECTION.** No projected coordinate may feed editing, hit testing, selection, wrapping, reveal or scroll decisions. No stale boundary, inferred character width, prefix shaping or render readback may establish a destination.


---

## 11. Explicitly Forbidden Operations

TextField MUST NOT:

* Call `String::insert_str`
* Call `String::drain`
* Iterate `char_indices`
* Compute caret X by shaping prefixes
* Mutate `cursor.pos` directly
* Mutate `selection.anchor/head` directly (except on pointer down)

---

## 12. Compliance Checklist (Mechanical)

A TextField implementation is SPEC-HAL-107 compliant **iff**:

* All navigation paths call substrate functions
* All edits call substrate functions
* Selection anchor is never implicitly reset
* Layout cache reflects selection state
* No UTF-8 logic exists in TextField

---

## 13. Relationship to Other SPECs

* **Depends on:** SPEC-HAL-052, SPEC-HAL-105
* **Constrains:** SPEC-HAL-105 implementation
* **Prepares for:** IME, clipboard, undo/redo

---

## 14. Enforcement

Any future TextField change violating this SPEC:

* MUST be rejected
* MUST be reverted
* MUST reference this SPEC in review

---

# Text Editor — Fundamental Invariants

These are **engine-level truths**.
If *any* of these break, the editor is broken, even if it “looks okay”.

---

## 0. Text Representation Invariants

1. The document text is a **linear sequence of Unicode scalar values**, stored as UTF-8.
2. Every edit produces a **single, well-defined byte sequence**.
3. Text order is **never altered implicitly**.

    * No reordering
    * No swapping
    * No visual rearrangement that does not correspond to byte order
4. Newlines (`\n`) are **ordinary characters** with semantic meaning.
5. The document always has a **stable length in bytes**.

---

## 1. Cursor Position Invariants

1. The cursor position is a **byte index** into the document.
2. The cursor position is **always valid**:

    * `0 <= cursor.pos <= text.len()`
3. The cursor is allowed to be:

    * At the start of the document
    * Between any two characters
    * At the end of the document
4. The cursor **never points into the middle of a UTF-8 codepoint**.
5. The cursor position **never depends on layout state**.
6. The cursor does **not move unless explicitly instructed**.

---

## 2. Boundary / Anchor Invariants

1. Every valid cursor position must correspond to a **legal boundary**.
2. A boundary exists:

    * Before the first character
    * After the last character
    * Between every adjacent pair of characters
3. Boundaries are **monotonic in document order**.
4. Boundary byte indices are **strictly increasing**.
5. No boundary may:

    * Go backwards
    * Be duplicated
    * Be skipped arbitrarily
6. Boundaries **do not depend on rendering order**.
7. Boundaries **do not disappear** after edits.

---

## 3. Line / Newline Invariants

1. A newline (`\n`) splits text into **logical lines**.
2. Every newline creates:

    * A line end boundary **before** `\n`
    * A line start boundary **after** `\n`
3. Every line has:

    * A valid start boundary
    * A valid end boundary
4. Empty lines are valid lines.
5. Consecutive newlines create multiple empty lines.
6. The first line always starts at byte `0`.
7. The last line may or may not end with a newline.
8. The cursor is allowed to sit:

    * At the start of a line
    * At the end of a line
    * On the newline itself

---

## 4. Cursor Movement — Horizontal

1. Moving left:

    * Moves the cursor to the **previous boundary**
2. Moving right:

    * Moves the cursor to the **next boundary**
3. Horizontal movement:

    * Never skips characters
    * Never jumps lines implicitly
4. Moving right from the last character of a line:

    * Lands on the newline boundary (if present)
5. Moving right from end-of-document:

    * Is a no-op
6. Moving left from start-of-document:

    * Is a no-op
7. Horizontal movement clears vertical intent (preferred column).

---

## 5. Cursor Movement — Vertical

1. Vertical movement is **visual-line based**, not byte based.
2. Moving up/down:

    * Preserves horizontal intent (preferred column)
3. Preferred column:

    * Is captured on first vertical move
    * Is preserved across consecutive vertical moves
    * Is cleared by horizontal movement
4. Moving up from the first line:

    * Is a no-op or clamps to first line
5. Moving down from the last line:

    * Is a no-op or clamps to last line
6. Vertical movement never changes document order.
7. Vertical movement always lands on a valid boundary.

---

## 6. Mouse Hit-Testing & Caret Placement Invariants

### 6.1 Coordinate → Boundary Mapping

1. Mouse click resolves to **exactly one ClusterBoundary**
2. The resolved boundary **must exist** in the active boundary list
3. Caret position after click equals `boundary.byte_start`
4. Caret **never** lands between boundaries
5. Caret **never** lands inside a UTF-8 codepoint

### 6.2 Horizontal Resolution (X axis)

1. Click X position maps to the **nearest boundary by x_logical**
2. If click X is left of first boundary on line → caret = line start
3. If click X is right of last boundary on line → caret = line end
4. No midpoint heuristics unless explicitly defined (nearest wins)

### 6.3 Vertical Resolution (Y axis)

1. Click Y position resolves to **exactly one visual line**
2. Resolved line is the one whose vertical span contains Y
3. If Y is above first line → first line
4. If Y is below last line → last line
5. Caret placement never skips lines
6. Y→line resolution **must use per-line vertical bounds** from layout. Average-height mapping is forbidden.

### 6.4 Combined X/Y Resolution

1. Line is resolved **before** horizontal boundary selection
2. Horizontal boundary search is restricted to resolved line only
3. Cross-line boundary selection is forbidden

### 6.5 Newline Semantics

1. Clicking **after end of line** places caret at line end boundary (before `\n`), not on previous/next line
2. Clicking on visual area representing newline glyph (if any) resolves to newline boundary
3. Newline is treated as an ordinary boundary for hit-testing

### 6.6 Empty / Short Line Handling

1. Clicking on empty line places caret at line start
2. Clicking past end of short line clamps to last boundary
3. Clicking on a line shorter than preferred column does **not** reuse preferred_x

### 6.7 Selection Reset Rules

1. Plain click (no modifiers): collapses selection, anchor == head == caret
2. Shift-click: anchor preserved, head set to clicked caret position
3. Mouse click clears `preferred_x` (see §7.6 for authoritative semantics)

### 6.8 Stability & Determinism

1. Same click input must always resolve to same caret position
2. Resolution is deterministic given same boundaries
3. No dependence on previous cursor position

### 6.9 UTF-8 Safety

1. Mouse placement never lands inside multi-byte UTF-8 sequence
2. All resolved caret positions satisfy `is_char_boundary`

### 6.10 Boundary Authority

1. Mouse hit-testing **must not compute new positions**
2. Only existing `ClusterBoundary` entries may be used
3. Rendering geometry does not override boundary authority

---

## 7. Selection Invariants

1. A selection is defined by two positions:

    * `anchor`
    * `head`
2. A collapsed selection has:

    * `anchor == head`
3. Selection order is irrelevant:

    * `start = min(anchor, head)`
    * `end = max(anchor, head)`
4. Selection boundaries are valid cursor positions.
5. Extending selection:

    * Moves `head`
    * Leaves `anchor` unchanged
6. Collapsing selection:

    * Sets both to the cursor position
7. Selection never points outside the document.

### 7.6 Mouse-Initiated Caret + Selection Semantics — Preferred X Handling

**Ownership:** `preferred_x` is **caret navigation state**. It is not owned by selection. It is not owned by mouse hit-testing. §5 (Cursor Movement — Vertical) defines its keyboard semantics. This section is authoritative for how mouse input affects `preferred_x`.

Mouse-initiated caret placement (including single click and Shift+click) **clears `preferred_x`**.

Mouse input does not establish or preserve horizontal navigation intent.
`preferred_x` is reserved exclusively for keyboard-driven vertical movement (Up/Down).

**Normative Rules:**

1. Any mouse click resets `preferred_x` to `None`
2. Shift+click resets `preferred_x` to `None`
3. Clicks on empty lines do not carry horizontal intent
4. Clicks above/below text bounds do not establish horizontal intent
5. Repeated clicks are idempotent with respect to `preferred_x`

**Rationale:**

Horizontal intent (`preferred_x`) exists to preserve the user's desired column position across vertical keyboard navigation. Mouse clicks represent explicit positional intent — the user clicked where they want the caret. There is no "memory" to preserve; the click itself is the intent.

Clearing `preferred_x` on mouse click ensures that subsequent Up/Down navigation starts fresh from the clicked position rather than jumping to a previously remembered column.

---

## 8. Text Insertion Invariants

1. Inserting text:

    * Inserts **exactly at cursor.pos**
2. Insertion preserves:

    * Prefix order
    * Inserted text order
    * Suffix order
3. Inserting at any boundary is valid.
4. Inserting at start-of-line is valid.
5. Inserting at end-of-line is valid.
6. Inserting `\n`:

    * Splits the current line
    * Creates a new line
7. After insertion:

    * Cursor advances by inserted byte length
8. No insertion:

    * Reorders existing text
    * Drops characters
    * Moves unrelated text

---

## 9. Deletion Invariants

1. Backspace deletes:

    * The character *before* the cursor
2. Delete deletes:

    * The character *after* the cursor
3. Deleting a selection:

    * Removes the entire selected range
4. Deleting a newline:

    * Merges two lines
5. After deletion:

    * Cursor lands at a valid boundary
6. Deletion never:

    * Skips characters
    * Deletes more than specified
    * Reorders text

---

## 10. Layout Invariants

1. Layout output preserves **document order**.
2. Runs are produced in **strictly increasing byte ranges**.
3. Lines are produced in **document order**.
4. No layout step may:

    * Swap runs
    * Reorder lines
    * Reattach suffix before prefix
5. Layout may **never change meaning** of text.
6. Layout must be **rebuildable deterministically** from text alone.

---

## 11. Cursor ↔ Layout Contract

1. Cursor logic depends on boundaries, not rendering.
2. Layout must produce boundaries consistent with text.
3. Cursor positions must remain valid after layout rebuild.
4. Layout must not invalidate cursor positions.
5. If layout cannot represent a cursor position, layout is wrong.

---

## 12. "Nothing Weird Happens" Invariants

These sound dumb. They are not.

1. Pressing Enter inserts a newline.
2. Pressing Enter never moves unrelated text.
3. Pressing Enter never does nothing.
4. Pressing Enter at column 0 works.
5. Pressing Enter at end of line works.
6. Pressing Enter on any line works.
7. Cursor never teleports.
8. Text never jumps lines on its own.
9. Text never appears to reorder visually.
10. If something looks impossible, it is a bug.

---

Good. You’re right: those need to be **operationally explicit**, not slogan-level.
I’ll rewrite **section 11 only**, but in the same *engine-grade* style as the rest: concrete, testable, no interpretation room.

Below is a **fully expanded, unambiguous version** of the “Nothing Weird Happens” invariants.

---

# 12. "Nothing Weird Happens" — **Expanded Operational Invariants**

These are **user-visible guarantees** that must fall directly out of engine behavior.
Each one must be testable *without rendering*, purely via text + cursor state.

---

## 12.1 Enter Key — Core Semantics

1. **Pressing Enter inserts exactly one `\n` character**

    * At the current cursor position
    * No other characters are inserted or removed

2. **The document after Enter is exactly**:

   ```
   prefix + "\n" + suffix
   ```

   where:

    * `prefix = text[0 .. cursor.pos]`
    * `suffix = text[cursor.pos .. end]`

3. **The relative order of all existing characters is preserved**

    * Every character in `prefix` stays before `\n`
    * Every character in `suffix` stays after `\n`
    * No character changes position relative to any other existing character

---

## 12.2 Cursor State After Enter

4. **After pressing Enter, the cursor position is**:

   ```
   old_cursor_pos + 1
   ```

   (byte length of `"\n"`)

5. **The cursor ends up immediately *after* the inserted newline**

    * The cursor is positioned at the start of the new line
    * The cursor is *before* any text that was previously to the right

6. **The cursor does not depend on layout or rendering state**

    * Its new position is derived purely from text mutation

---

## 12.3 Enter at Specific Positions

7. **Pressing Enter at column 0 (start of a line)**:

    * Inserts a newline *before* the current line’s content
    * Produces an empty line above
    * Moves the cursor to the start of the newly created empty line

   Example:

   ```
   "|Hello"
   Enter →
   "\n|Hello"
   ```

8. **Pressing Enter in the middle of a line**:

    * Splits the line into two logical lines
    * Left part stays on the original line
    * Right part moves to the next line
    * Cursor moves to the beginning of the second line

   Example:

   ```
   "Hel|lo"
   Enter →
   "Hel\n|lo"
   ```

9. **Pressing Enter at end of a line**:

    * Inserts a newline after the line
    * Produces an empty line below
    * Cursor moves to the start of the empty line

   Example:

   ```
   "Hello|"
   Enter →
   "Hello\n|"
   ```

10. **Pressing Enter at end of document**:

    * Appends a newline
    * Cursor moves to the new final position

---

## 12.4 Enter with Existing Newlines

11. **Pressing Enter on any line works identically**

    * First line
    * Middle line
    * Last line
    * Empty line

12. **Repeated Enter presses produce multiple consecutive newlines**

    * Each Enter inserts exactly one `\n`
    * No collapsing or deduplication occurs

13. **Empty lines are preserved**

    * Inserting Enter on an empty line creates another empty line

---

## 12.5 Things That Must NEVER Happen on Enter

14. **Pressing Enter never moves unrelated text**

    * Text before the cursor never moves
    * Text after the cursor never jumps to a different line except by being shifted *after* the inserted newline

15. **Pressing Enter never reorders text**

    * No suffix text may appear above prefix text
    * No line swapping
    * No visual order that contradicts byte order

16. **Pressing Enter never does nothing**

    * If Enter is accepted, the document must change

17. **Pressing Enter never depends on cursor column**

    * Column 0 is valid
    * Last column is valid
    * Any column is valid

18. **Pressing Enter never depends on layout height or viewport**

    * Auto-height, scrolling, clipping must not affect insertion

---

## 12.6 Cursor Stability / "No Teleportation"

19. **The cursor never teleports**

    * Cursor movement after Enter is exactly `+1 byte`
    * No jumps to start or end of document

20. **The cursor never jumps to a different logical line except as a direct result of the newline insertion**

21. **The cursor never lands on an invalid boundary**

    * After Enter, cursor position must correspond to a valid boundary in rebuilt layout

---

## 12.7 Visual Consistency (Derived, Not Primary)

22. **Visual order must reflect document order**

    * If the visual result looks reordered, the engine is wrong

23. **If a user sees text “jump upward” or “fall downward” unrelated to the cursor, it is a bug**

24. **If pressing Enter appears to move text somewhere unexpected, the engine violated invariants above**

---

## 12.8 Absolute Rule

25. **If something looks impossible or confusing, the engine is wrong — not the user**

---

## Additional Fundamental Invariants (Addendum)

### A. Document / Byte Semantics

1. Text is never implicitly normalized (no NFC/NFD, no newline normalization).
2. Each edit is atomic; no intermediate document states are observable.
3. Document byte length changes exactly by the inserted/deleted byte count.
4. No operation may implicitly modify text outside its explicit range.

---

### B. Cursor Stability

5. Cursor position is valid **before and after** every operation.
6. Cursor position is unchanged by no-op actions.
7. One user action produces at most one cursor movement.
8. Cursor position never changes during layout rebuilds.

---

### C. Boundary Guarantees

9. Boundary generation is deterministic for identical text.
10. Boundary count changes **only** when text changes.
11. Every valid cursor position maps to exactly one boundary.
12. Boundaries fully cover `[0 .. text.len()]` with no gaps.

---

### D. Line / Newline Semantics

13. A newline belongs to exactly one logical line (terminates the previous).
14. Line start is distinct from the previous line’s newline boundary.
15. Every line has representable start and end boundaries.
16. Soft wrapping never creates or removes logical lines.

---

### E. Enter Edge Guarantees

17. Enter at column 0 does not consume or move existing characters.
18. Enter at end of line advances cursor by exactly one byte.
19. Enter behavior is independent of selection direction.
20. Enter on an empty line creates exactly one empty line.

---

### F. Selection Stability

21. Selection state is invariant under layout rebuilds.
22. Collapsed selection is semantically identical to no selection.
23. Selection deletion is exactly equivalent to deleting `[start, end)`.

---

### G. Layout Order Constraints

24. Flattened layout runs must have strictly increasing byte ranges.
25. Layout recursion must preserve document order.
26. Layout output must be idempotent for unchanged text.
27. Layout must not reorder, rotate, or interleave byte ranges.

---

### H. Enforcement

28. Any invariant violation is a bug.
29. Silent correction of invariant violations is forbidden.
30. Unit tests are authoritative over demo behavior.

---

## 13. Deletion Modifier Semantics

1. **Shift does NOT change deletion semantics**
   * Shift+Backspace behaves identically to Backspace
   * Shift+Delete behaves identically to Delete
   * Shift MUST NOT extend deletion range

2. **Ctrl/Opt modifies granularity only**
   * Ctrl+Backspace / Opt+Backspace: deletes previous word/cluster per substrate rules
   * Ctrl+Delete / Opt+Delete: deletes next word/cluster per substrate rules
   * No other semantic changes

3. **Ctrl/Opt + Shift has no additional semantics**
   * Ctrl+Shift+Backspace behaves identically to Ctrl+Backspace
   * Ctrl+Shift+Delete behaves identically to Ctrl+Delete
   * Opt+Shift+Backspace behaves identically to Opt+Backspace
   * Opt+Shift+Delete behaves identically to Opt+Delete

4. **Range deletion is selection-driven only**
   * Deleting a range of text MUST use selection (anchor ≠ head)
   * Modifier keys MUST NOT create implicit selection for deletion
   * Workflow: Shift+Home/End to select, then Backspace/Delete

5. **Implementation guidance**
   * If platform/driver delivers Shift+Delete or Ctrl+Shift+Delete, implementation MAY:
     * Ignore Shift (treat as Ctrl+Delete)
     * Treat as unhandled
   * Implementation MUST NOT invent extended deletion semantics
