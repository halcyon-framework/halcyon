# SPEC-HAL-055: Undo/Redo System for Text Editing

Transactional Editing, History Compression, Cursor/Selection Snapshotting, Integration With TextField & IME

Status: Draft
Author: JC
Depends on:

* **SPEC-HAL-052** (Interactive Text Editing Model)
* **SPEC-HAL-053** (IME Composition Model)
* **SPEC-HAL-054** (TextField Architecture)
* **SPEC-HAL-048** (Paragraph & Text Layout Engine)
* **SPEC-HAL-042** (Engine Ticker, Time Model)

Planned follow-ups:

* SPEC-HAL-056 (Clipboard Integration)
* SPEC-HAL-057 (Secure Entry / PasswordField Mode)
* SPEC-HAL-058 (Command Palette / Command-Based Editing Ops)

---

## 1. Summary

This SPEC introduces Halcyon’s **Undo/Redo model** for text editing:

* Transaction-based history
* Merge rules (typing bursts collapse into one undo step)
* Cursor & selection snapshotting
* IME-safe behavior
* Multi-line aware
* Efficient for large documents
* Low memory overhead
* Works uniformly for TextField and future rich editors (CodeEditor)

Undo/Redo forms the backbone of any interactive editor and must be deterministic, cluster-safe, and fully compatible with the editing architecture defined in SPEC 052–054.

---

## 2. Motivation

The current (nonexistent) undo mechanism must evolve into a consistent system that:

* Never corrupts UTF-8
* Reverses composition changes correctly
* Ensures typing bursts undo as atomic operations
* Tracks cursor+selection accurately
* Performs efficiently even on long paragraphs
* Allows integration with future widgets (CodeEditor)

Without this, Halcyon can’t support:

* Normal text editing usage
* Undo after IME composition
* Word deletions
* Multi-line editing
* Rich editing commands

This SPEC defines the authoritative Undo/Redo engine.

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: Every text-changing operation must create or merge into an undo transaction.
* R2: Redo operates only after an undo; any new edit clears redo stack.
* R3: History spans:

    * text snapshot
    * cursor position
    * selection state
* R4: IME composition does not pollute undo history until commit.
* R5: Undo/Redo must be O(1) or amortized O(1) for cursor/selection changes, and O(n) for text diff where necessary.

### 3.2 Invariants

* I1: Undo boundaries always align with cluster boundaries (SPEC-052).
* I2: Text integrity preserved across undo/redo.
* I3: Composition state is cleared on undo/redo.
* I4: Typing bursts merged automatically based on time threshold.
* I5: Undo cannot cross IME composition boundaries.

### 3.3 Constraints

* C1: History depth configurable, default unlimited.
* C2: Large edits (paste, delete selection) are atomic operations.
* C3: History must not leak memory; text buffers reference-counted.

---

## 4. Data Model

### 4.1 EditTransaction

```
pub struct EditTransaction {
    pub before_text: String,
    pub after_text: String,

    pub before_cursor: usize,
    pub after_cursor: usize,

    pub before_selection: Option<TextSelection>,
    pub after_selection: Option<TextSelection>,

    pub timestamp: u64, // EngineTicker time
}
```

### 4.2 UndoHistory

```
pub struct UndoHistory {
    pub undo_stack: Vec<EditTransaction>,
    pub redo_stack: Vec<EditTransaction>,
    pub merge_window_ms: u64,
}
```

### 4.3 EditingState integration

```
pub struct EditingState {
    pub text: String,
    pub cursor: TextCursor,
    pub selection: Option<TextSelection>,
    pub composition: Option<CompositionState>,
    pub history: UndoHistory,
}
```

---

## 5. Transaction Lifecycle

Undo/Redo is based on **before/after snapshots** around text mutations.

Pipeline:

```
EditingState::begin_transaction()
EditingState::apply_operation()
EditingState::end_transaction()
```

Transactions may be merged depending on rules.

---

## 6. Transaction Creation Rules

### 6.1 When a transaction begins

A new transaction begins when:

* inserting text
* deleting clusters
* deleting selection
* replacing selection
* IME commit occurs

A transaction does NOT begin for:

* cursor movement
* selection movement
* composition updates (IME pre-edit)

### 6.2 Before snapshot

When first mutation happens:

* store before_text
* store before_cursor
* store before_selection
* store timestamp

### 6.3 After snapshot

When transaction ends:

* store after_text
* after_cursor
* after_selection

---

## 7. Transaction Merging Rules (Typing Burst Collapsing)

Typing should be undoable as one step.

Example:

```
"h" → "he" → "hel" → "hell" → "hello"
```

Undo once → empty string.

### 7.1 Merge conditions

Two transactions A and B merge if:

* Both are insertions
* Both operate at contiguous positions
* Both within merge_window_ms (default 1000 ms)
* No cursor movement occurs between A and B
* No selection change occurs between A and B
* No line break inserted between A and B (optional configurable)
* Not separated by an IME event

### 7.2 Deletion merging

Backspaces merge if:

* Both operations delete a cluster adjacent to previous deleted cluster
* Same time window
* Cursor didn’t move between deletes

### 7.3 Fallback

If conditions fail → start new transaction.

---

## 8. Redo Model

Redo stack only exists after undo operations.

Rules:

* Any new edit clears redo stack.
* Undo pops the last transaction from undo_stack and applies its “before” snapshot.
* Redo pops from redo_stack and applies “after” snapshot.
* Cursor & selection also restored.

---

## 9. IME-Safe Rules

### 9.1 IME pre-edit

Composition updates do NOT produce undo history entries.

During composition:

* editing.text remains unchanged
* composition.text is ephemeral

### 9.2 IME commit

Commit creates a single atomic transaction:

* before = underlying text before commit
* after = text with IME commit inserted
* cursor = position after commit

### 9.3 IME cancel

Cancel does not modify underlying text → no history entry.

---

## 10. Multi-line Undo/Redo Behavior

Undo must:

* Restore multi-line cursor position (line + x offset)
* Restore selection spanning multiple lines
* Trigger re-layout through ParagraphEngine
* Ensure cluster positions still valid:

    * If text changed drastically, cursor moves to nearest available cluster

Redo repeats all the same rules.

---

## 11. Algorithmic Detail

### 11.1 Applying undo transaction

```
editing.text = tx.before_text
editing.cursor.position = tx.before_cursor
editing.selection = tx.before_selection
```

### 11.2 Applying redo transaction

```
editing.text = tx.after_text
editing.cursor.position = tx.after_cursor
editing.selection = tx.after_selection
```

### 11.3 Clearing redo on new edits

```
history.redo_stack.clear()
```

### 11.4 Memory efficiency

* Strings may use Arc<Rope> later (future SPEC).
* Simple String copies fine for most inputs (<10 KB).

---

## 12. Integration With EditingState API

EditingState exposes helper functions:

```
fn begin_edit(&mut self)
fn end_edit(&mut self)
fn record_before(&mut self)
fn record_after(&mut self)
fn merge_top(&mut self)
```

These are called internally by:

* insert()
* delete()
* backspace()
* commit_composition()

The widget author doesn’t need to handle them explicitly.

---

## 13. Integration With TextField

TextField triggers editing operations via EditingState.
EditingState handles:

* history
* cluster-safe operations
* composition
* cursor navigation

TextField only triggers:

* editing.begin_edit()
* editing.apply_operation()
* editing.end_edit()

---

## 14. Testing Strategy

* Typing bursts merged correctly.
* Multi-line edits reversible.
* Multi-glyph cluster deletions.
* Emoji cluster undo.
* IME sequences undo as one commit.
* Selection replacement undo/redo.
* Undo after drag-selection deletion.
* Undo at document boundaries.

---

## 15. Performance Notes

* Transactions small; usually only last ~50–300 chars.
* Merge logic amortized constant time.
* Large deletes (Ctrl+A → Delete) stored once; efficient.

---

## 16. Security Considerations

* Undo must always restore valid UTF-8.
* No partially-applied operations allowed.
* Composition text must be cleared after undo/redo.
