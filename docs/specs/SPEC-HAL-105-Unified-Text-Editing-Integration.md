# **SPEC-HAL-105: Unified Text Editing Integration**

**Status:** Design
**Author:** JC
**Depends on:**

* **SPEC-HAL-052** (Text Editing Substrate)
* **SPEC-HAL-101** (TextField Widget)
* **SPEC-HAL-048** (Paragraph & Text Layout Engine)
* **SPEC-HAL-050** (Shaping integration)
* **SPEC-HAL-102** (Typography & Theme Contract)

**Overview**

This SPEC unifies all text-editing behavior in Halcyon across:

* cursor movement
* selection logic
* word/line/paragraph operations
* hit-testing
* drag selection
* editing (insert/delete/word delete)
* click/double-click/triple-click
* caret visibility
* layout cache integration
* configuration flags

The goal is to replace ad-hoc, char-based text handling in TextField and future widgets with the canonical editing substrate defined in SPEC-HAL-052, using cluster boundaries, TextCursor, and Selection.

The SPEC defines **12 implementation stages** which must be followed in order, ensuring all existing substrate test suites pass.

---

# **Objectives**

1. Replace all character-based logic with cluster-based editing.
2. Bind TextField directly to the editing substrate (TextCursor, Selection).
3. Use TextLayoutCache for cluster boundary extraction and hit-testing.
4. Integrate OS-consistent selection, drag, and multi-click behavior.
5. Introduce unified configuration and state models for all editable widgets.
6. Ensure all editing operations pass existing tests (cluster, word, line, paragraph navigation/selection and edit ops).

---

# **Non-Objectives**

* IME composition (SPEC-HAL-053 will define this).
* Multi-line TextArea (separate SPEC).
* Autonomous formatting (syntax highlighting, etc).

---

# **Canonical Editing Stack (Summary)**

This SPEC mandates the use of the following substrate components:

* **TextCursor**

    * cluster-aligned position
    * affinity (left/right)
    * preferred_x for vertical nav

* **Selection**

    * anchor/head representation
    * empty/non-empty state

* **ClusterBoundary**, **WordBoundary**, **LineBoundary**, **ParagraphBoundary**

* Editing ops:

    * `apply_edit_op`
    * `apply_insert_op`
    * `apply_delete_word_back`
    * `apply_delete_word_forward`

* Navigation ops:

    * `move_left_collapse`, `move_right_collapse`
    * `move_*_select`
    * `move_word_left/right (+_select)`
    * `move_home/end (+_select)`
    * `move_to_paragraph_start/end`
    * `move_up/down (+_select)`

* Hit-testing ops:

    * `hit_test_x_y`
    * `hit_test_click`
    * `visual_cluster_for_pos`

TextField must become a thin layer over these primitives.

---

# **Stage Breakdown**

The implementation must occur in the 12 stages below.

Each stage is atomic and must compile + pass relevant test suites before proceeding.

---

## **Stage 1 — State Unification**

Replace TextField’s ad-hoc fields with unified state.

**Introduce:**

* `TextDocument`
* `TextCursor`
* `Selection`
* `TextFieldState`
* `TextFieldConfig`
* `TextFieldDragMode`

**TextField stores:**

```
document: TextDocument
cursor: TextCursor
selection: Selection
drag_mode: TextFieldDragMode
config: TextFieldConfig
layout_cache: TextLayoutCache
scroll_x: f32
```

Caret index fields are removed.

---

## **Stage 2 — Layout Cache Integration**

TextField no longer shapes text manually.

**Requirements:**

* Use `TextLayoutCache` for boundaries and line data.
* In `layout()`, rebuild cache on document version change.
* Cache invalidates after any text mutation.
* Extract cluster boundaries directly from cache.

---

## **Stage 3 — Canonical Hit-Testing**

Replace char-based prefix-shaping hit-test.

**Requirements:**

* Use `hit_test_x_y(boundaries, x + scroll_x, 0)`
* Use `hit_test_click(cursor, selection, boundaries, x, y)`
* Click moves cursor and resets selection unless Shift is held.

---

## **Stage 4 — Cursor Integration**

Replace character-based cursor navigation with substrate.

**Requirements:**

* `move_left_collapse`
* `move_right_collapse`
* `move_home` / `move_end`
* Maintain `preferred_x` during vertical navigation.
* Caret blink resets on all movement.

All cursor ops must use cluster boundaries.

---

## **Stage 5 — Selection Integration**

Add the anchor/head selection model.

**Requirements:**

* Shift+Left/Right → `move_*_select`
* Selection clears on click unless Shift is held.
* Cursor only visible if `selection.is_empty()`
* All selection invariants must hold.

---

## **Stage 6 — Word Navigation & Selection**

Implement canonical word operations.

**Requirements:**

* Ctrl+Left/Right
* Ctrl+Shift+Left/Right
* Double-click → word select via `double_click_selection`
* Word-drag via `word_drag_update`

Cluster boundaries cannot be bypassed.

---

## **Stage 7 — Line Navigation & Selection**

Even for single-line TextField, implement substrate semantics.

**Requirements:**

* Up/Down navigation uses cluster boundaries (clamped to single line).
* Shift+Up/Shift+Down → expand selection vertically.
* Triple-click → whole line.
* Line-drag via `line_drag_update`.

---

## **Stage 8 — Paragraph Navigation & Selection**

Paragraph ops are required for consistency and future multi-line widgets.

**Requirements:**

* Ctrl+Home / Ctrl+End
* Paragraph selection ops
* Triple-click+drag → paragraph select
* All via substrate boundary lookup.

---

## **Stage 9 — Editing Operations**

Replace all manual string manipulation.

**Requirements:**

* Backspace → `apply_edit_op(TextEditOp::Backspace)`
* Delete → `apply_edit_op(TextEditOp::Delete)`
* Ctrl+Backspace → `apply_delete_word_back`
* Ctrl+Delete → `apply_delete_word_forward`
* Insert → `apply_insert_op`

All mutations must:

* Update document
* Invalidate layout cache
* Maintain cursor + selection invariants
* Reset caret blink

---

## **Stage 10 — Drag Interaction Model**

Native pointer-based selection behavior.

**Requirements:**

* `cx.click_count()` distinguishes single/double/triple click.
* `cx.capture_pointer()` held during drag.
* Drag updates selection based on active drag mode:

    * character
    * word
    * line
    * paragraph
* Release capture on PointerUp.

Pointer interactions must match OS behavior.

---

## **Stage 11 — Selection & Caret Painting**

Integrate selection visuals.

**Requirements:**

* Use `visual_cluster_for_pos` to compute selection rectangles.
* Draw selection before text, after background.
* Draw caret only when selection is empty.
* Obey theme colors and typography radii/thickness.

Password mode hides text but still obeys selection geometry.

---

## **Stage 12 — Advanced Features & Configuration**

Finalize all TextFieldConfig behaviors.

**Implement:**

* `commit_on_enter` → emit action (no newline)
* `max_len_bytes` → clamp insertions
* `select_all_on_focus`
* `is_read_only` → block mutations
* `is_enabled` → block focus + events

This completes the integration.

---

# **Data Structures**

### **TextFieldConfig**

```
pub struct TextFieldConfig {
    pub placeholder: Option<String>,
    pub is_password: bool,
    pub select_all_on_focus: bool,
    pub commit_on_enter: bool,
    pub max_len_bytes: Option<usize>,
    pub is_read_only: bool,
    pub is_enabled: bool,
}
```

### **TextFieldDragMode**

```
pub enum TextFieldDragMode {
    None,
    Character { anchor: usize },
    Word { initial: WordBoundary },
    Line { anchor_line: usize },
    Paragraph { anchor_para: (usize, usize) },
}
```

### **TextFieldState**

```
pub struct TextFieldState {
    pub document: TextDocument,
    pub cursor: TextCursor,
    pub selection: Selection,
    pub layout_cache: TextLayoutCache,
    pub scroll_x: f32,
    pub drag_mode: TextFieldDragMode,
}
```

---

# **Acceptance Requirements**

The implementation is complete when:

1. All existing text editing tests pass:

    * cluster navigation
    * hit-testing
    * word operations
    * line operations
    * paragraph navigation
    * selection tests
    * edit ops tests
    * insert ops tests

2. TextField’s behavior matches:

    * SPEC-HAL-101
    * native OS text fields
    * SPEC-HAL-052 invariants

3. No code path performs char-based navigation.

4. No code path performs manual shaping for hit-testing.

5. No mutation bypasses `TextDocument` or substrate ops.

6. No incorrect cluster positions are produced by UI events.

---

# **Completion Criteria**

* TextField becomes fully compliant with universal editing semantics.
* All selection modes behave as per substrate—character, word, line, paragraph.
* Mouse + keyboard editing matches expectations of modern UI platforms.
* All operations are cluster-correct, glyph-safe, and bidirectionally valid.
* The widget becomes structurally ready for IME integration (SPEC-053).
