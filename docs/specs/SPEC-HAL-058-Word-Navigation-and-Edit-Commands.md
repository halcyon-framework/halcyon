# SPEC-HAL-058: Word Navigation, Word Deletion & Granular Text Commands

Double-Click Selection, Ctrl+Arrow Movement, Ctrl+Backspace/Delete, Home/End, Select-All

Status: Draft
Author: JC
Depends on:

* **SPEC-HAL-052** (Editing Model)
* **SPEC-HAL-054** (TextField Architecture)
* **SPEC-HAL-055** (Undo/Redo System)
* **SPEC-HAL-056** (Clipboard)
* **SPEC-HAL-057** (SecureEntry Mode)
* **SPEC-HAL-048** (Paragraph & Line Layout)

Planned follow-ups:

* SPEC-HAL-059 (Touch / Virtual Keyboard Navigation)
* SPEC-HAL-060 (RichTextField / Inline Formatting)

---

## 1. Summary

This SPEC defines advanced text editing commands familiar from modern editors:

* Ctrl+← / Ctrl+→ — jump by “word”
* Ctrl+Backspace — delete previous word
* Ctrl+Delete — delete next word
* Double-click — select word under cursor
* Triple-click — select entire line
* Home / End — start/end of line
* Ctrl+Home / Ctrl+End — beginning/end of entire text
* Shift+ variants for selection extension
* Cluster-safe, Unicode-aware behavior

This SPEC creates a **complete high-level text command layer**, making Halcyon usable for serious editing beyond single-character operations.

---

## 2. Motivation

Current text model supports cluster-level movement (SPEC-052), which is correct but inconvenient for users:

* Jumping letter-by-letter is slow
* Deleting words requires repeated Backspace
* Selecting words manually is annoying
* Modern apps rely on Ctrl+Arrow, Ctrl+Backspace, double-click

This SPEC adds a high-level semantic layer without disturbing:

* underlying cluster model
* IME rules
* undo/redo
* masked SecureEntry

This is the layer that makes the TextField **feel native**.

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: Word boundaries determined by Unicode categories, not ASCII assumptions.
* R2: Cluster-safe movement (never land inside a combined cluster).
* R3: Multi-line aware (Home/End per-line, Ctrl+Home/End global).
* R4: Deleting words compresses into single undo transaction.
* R5: Double-click selects the “word” under pointer.
* R6: Triple-click selects full line.
* R7: SecureEntry: masked but semantics identical.
* R8: IME: must cancel composition before word commands.

### 3.2 Invariants

* I1: Word boundaries always occur between clusters.
* I2: Movement always snaps to cluster boundaries.
* I3: Deletion merges clusters inside the word.
* I4: Undo/Redo does not reveal text in SecureEntry mode.
* I5: Word boundaries MUST NOT use regex per frame; cached classification only.

---

## 4. Word Boundary Model

### 4.1 Classification

Each Unicode scalar receives one of the following classes:

* **Letter** (alphabetic categories)
* **Number** (Nd, Nl, No)
* **Symbol** (Sm, Sc, Sk, So)
* **Whitespace** (Zs, tabs, spaces)
* **Punctuation** (Pc, Pd, Ps, Pe, Pi, Pf, Po)
* **Other** (everything else)

### 4.2 Boundary rules (summary)

Word boundaries occur:

* between whitespace and non-whitespace
* between letter and non-letter
* between number and non-number
* between symbol and non-symbol
* inside punctuation sequences (punct = punct) — treated as one or word-dependent
* Emoji are treated as “letter-like”, i.e. belong to same word if consecutive

### 4.3 Cluster alignment

Boundary always aligned to cluster boundaries:

```
cluster[i].byte_end → boundary
cluster[i+1].byte_start → boundary
```

### 4.4 Example

Text: `Hello, 世界 123!!! foo_bar`

Word segments:

* `Hello`
* `,`
* `世界`
* `123`
* `!!!`
* `foo_bar`

---

## 5. Word Navigation Commands

### 5.1 Move to previous word (Ctrl+←)

Steps:

1. Cancel composition if active.
2. Find cluster before cursor.
3. Walk left until reaching first boundary where:

    * class changes OR whitespace begins
4. Cursor snaps to boundary.

### 5.2 Move to next word (Ctrl+→)

Mirror of previous:

* Walk right until boundary
* Snap cursor to boundary

### 5.3 Selection variants

Shift+Ctrl+←/→:

* anchor stays fixed
* head moves using the movement rule
* selection updated accordingly

---

## 6. Word Deletion Commands

### 6.1 Ctrl+Backspace — delete previous word

1. Cancel IME.
2. Determine previous word boundary.
3. Begin undo transaction.
4. Delete full cluster range.
5. End undo transaction.

### 6.2 Ctrl+Delete — delete next word

Same logic, but walk forward.

### 6.3 SecureEntry effects

Do NOT reveal underlying word when deleting.
Undo restores underlying text but still masked.

---

## 7. Double-Click Selection (Mouse)

### 7.1 On double-click on cluster:

1. Determine cluster index under pointer.
2. Identify full word range:

    * expand left until boundary
    * expand right until boundary
3. Set selection = [start_of_word, end_of_word]
4. Cursor = end_of_word (consistent with common editors)

### 7.2 Triple-click

Triple-click selects the entire line:

1. Determine line under pointer.
2. Select from line.start_cluster → line.end_cluster.
3. Cursor = end_cluster.

---

## 8. Home / End Behavior

### 8.1 Home (no Ctrl)

Move cursor to start of current line.

Shift+Home extends selection.

### 8.2 End (no Ctrl)

Move cursor to end of current line.

Shift+End extends selection.

### 8.3 Ctrl+Home

Move to beginning of entire text.

### 8.4 Ctrl+End

Move to end of entire text.

Shift variants extend selection accordingly.

---

## 9. Integration With EditingState

Extend EditingState with functions:

```
move_word_left()
move_word_right()
delete_word_left()
delete_word_right()
select_word_at(pos)
select_line_at(pos)
move_home()
move_end()
move_global_home()
move_global_end()
```

EditingState handles:

* cluster alignment
* merging into undo transactions
* IME cancellation
* SecureEntry adaptation

TextField simply calls these functions.

---

## 10. Hit-Testing Considerations

Double-click logic performed after hit-testing cluster under pointer:

```
pos = hit_test(x, y)
editing.select_word_at(pos)
```

Triple-click:

```
line = paragraph.line_index_at(y)
editing.select_line_at(line)
```

---

## 11. Undo/Redo Integration

### Word deletions

Ctrl+Backspace/Delete produce **one** undo transaction.
Multiple calls spaced <`merge_window_ms` do NOT merge (word deletes stay separate).

### Double-click selection

Does NOT create undo entries.

### Triple-click

Does NOT create undo entries.

### Home/End

Do NOT create undo entries.

---

## 12. SecureEntry Special Rules

* Word boundaries computed from underlying text.
* Cursor/selection based on underlying text.
* Display uses masked clusters.
* Double-click selects masked-width range.
* Word deletion does not reveal deleted characters.

Clipboard remains disabled for copy/cut.

---

## 13. Performance Notes

* Word boundary classification cached per paragraph slice.
* Unicode categories precomputed via lookup tables.
* Word navigation O(N) worst-case but short in typical usage.
* Double-click selection O(N) around the clicked index.

---

## 14. Testing Strategy

* ASCII words
* Unicode words (`世界`, `مرحبا`)
* Mixed punctuation
* Emoji word sequences
* RTL word navigation
* Multi-line Home/End
* SecureEntry word delete
* Double-click word selection
* Triple-click line selection
* Undo/Redo around all commands
* IME interactions
