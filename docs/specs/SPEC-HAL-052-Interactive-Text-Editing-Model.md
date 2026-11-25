# SPEC-HAL-052: Interactive Text Editing Model

Cursor, Selection, Graphemes, Clusters, Navigation, Editing Semantics

Status: Implemented
Author: JC
Depends on:

* SPEC-HAL-007 – State Model
* SPEC-HAL-009 – Event Routing & Hit Testing
* SPEC-HAL-033/036 – Input Fusion & Coalescing
* SPEC-HAL-048 – Paragraph & Text Layout Engine
* SPEC-HAL-049 – Font Collections & Fallback Rules
* SPEC-HAL-050 – Unified Text Shaping Architecture
* SPEC-HAL-065 – Complex Text Shaping (rustybuzz)
* SPEC-HAL-090–099 – Layout Grid, Typography, Pointer, Input Precision
* SPEC-HAL-100 – Widget Interaction State Machine

Planned follow-ups:

* SPEC-HAL-053 – IME Composition Model
* SPEC-HAL-054 – TextField Architecture & Editable Paragraph Widget
* SPEC-HAL-055 – Clipboard, Undo/Redo & History Model

---

## 0. Purpose

This SPEC defines the **authoritative text editing model** for Halcyon:

* Cursor representation and movement
* Selection model and semantics
* Mouse and keyboard interaction rules
* Cluster-safe editing (insert, delete, backspace, word-delete)
* Hit-testing (pointer → caret / selection)
* Integration with shaping and layout (clusters, lines)
* Integration with pointer and interaction rules (090–100)

This is **foundational**. All future editable widgets (single-line, multi-line, code editor, rich text) must obey this model.

IME, clipboard, undo/redo, and widget-specific visuals are explicitly out of scope here, and covered in their own SPECs.

---

## 1. Definitions

1. **Text buffer**

    * Logical content of a single editable paragraph.
    * Stored as UTF-8.
    * Indexed in **byte offsets**.
    * Newlines (`\n`) separate **logical lines**.

2. **Grapheme cluster**

    * User-perceived character (base + combining marks, emoji sequences, ligatures etc.).
    * Editing moves and deletion operate in units of grapheme clusters.
    * Implementation uses:

        * `unicode-segmentation` for grapheme boundaries, and
        * rustybuzz cluster boundaries where shaping overrides naive segmentation.

3. **Cluster**

    * A **shaping cluster** from rustybuzz:

        * `glyph.cluster` is the byte offset into the original UTF-8 text.
    * One cluster maps to a grapheme or ligature decision by the shaper.
    * Editing uses the **intersection**:

        * legal edit/caret positions are cluster boundaries fixed by shaping.

4. **Caret / cursor position**

    * A position **between clusters**, represented as a **byte offset** `pos`:

        * `0 <= pos <= text.len()`
    * Caret is always at a **legal cluster boundary**.

5. **Anchor**

    * The position where the current selection started, also a byte offset `anchor`.
    * Selection direction is implicit:

        * forward if `pos >= anchor`
        * backward if `pos < anchor`.

6. **Selection range**

    * Half-open range:
      `start = min(anchor, pos)`
      `end   = max(anchor, pos)`
      Selected text = `[start, end)`.
    * If `start == end` → no selection.

7. **Logical line**

    * Substring between:

        * start of text or `\n`
        * next `\n` (exclusive) or end-of-text.
    * Triple-click, Home/End, line selection operate on **logical lines**.

8. **Visual line**

    * Result of layout + wrapping in ParagraphEngine.
    * Vertical movement and hit-testing use **visual lines**.

9. **Word**

    * Block of “word characters” surrounded by non-word characters.
    * Word characters:

        * Unicode letters and digits
        * underscore `_`
        * optionally `'` if inside letters/digits (e.g. `don't`).
    * Word boundaries are used for:

        * double-click selection
        * Ctrl/Alt+Left/Right navigation
        * Ctrl+Backspace / Ctrl+Delete word deletion.

---

## 2. Data Model (Core Types)

All of this lives in a **pure logic** module, no widgets, no platform events:

`src/core/editing.rs` (or equivalent).

### 2.1 TextBuffer abstraction

We keep the concrete implementation as `String` for now, but the core logic talks to a trait to allow ropes later:

```rust
pub trait TextBuffer {
    fn as_str(&self) -> &str;
    fn replace_range(&mut self, range: std::ops::Range<usize>, with: &str);
    fn len(&self) -> usize;

    // Optional convenience:
    fn is_empty(&self) -> bool { self.len() == 0 }
}
```

Initial implementation:

```rust
pub struct SimpleTextBuffer {
    text: String,
}
```

### 2.2 Cursor and selection

```rust
/// Byte offset at a legal cluster boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextCursor {
    pub pos: usize,
}

/// Anchor + active position, half-open [start, end).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSelection {
    pub anchor: usize,
    pub pos: usize,
}
```

Invariants:

* `0 <= pos <= text.len()`
* `0 <= anchor <= text.len()`
* Both must always be valid caret positions:

    * i.e. must lie on cluster boundaries.

Helpers:

* `fn start(&self) -> usize`
* `fn end(&self) -> usize`
* `fn is_empty(&self) -> bool`
* `fn normalized(&self) -> (usize, usize)` – returns `(start, end)`.

### 2.3 ClusterBoundary and ClusterMap

`ClusterBoundary` is the editing view over ParagraphEngine’s shaping output.

```rust
pub struct ClusterBoundary {
    pub byte_start: usize,  // inclusive
    pub byte_end: usize,    // exclusive
    pub line_index: usize,  // visual line index
    pub x_logical: f32,     // x position within line
}
```

Cluster map:

* For a single paragraph:

    * `Vec<ClusterBoundary>` in **visual order**.
    * A **sentinel** at end-of-text:

        * `byte_start == byte_end == text.len()`
        * belongs to last visual line, `x_logical` at end of last line.

Builder:

```rust
impl ClusterBoundary {
    /// Build cluster boundaries from ParagraphEngine lines.
    /// Two-pass:
    /// 1) Collect all byte_start and derive byte_end by next start or text.len()
    /// 2) Walk lines/runs/glyphs in visual order, group same-cluster glyphs,
    ///    use map to set byte_end.
    fn build_from_lines(lines: &[Line], text: &str) -> Vec<ClusterBoundary>;
}
```

Lookup:

* `fn find_boundary_at(boundaries: &[ClusterBoundary], byte_offset: usize) -> Option<usize>`

    * If `byte_offset` inside `[start, end)` or `== start` → that cluster.
    * Otherwise → cluster with nearest byte region (minimal distance).
* `fn hit_test(boundaries: &[ClusterBoundary], lines: &[Line], x: f32, y: f32) -> usize`

    * Map logical `(x,y)` to nearest cluster’s `byte_start`.

Exact algorithms were already prototyped and are acceptable; details stay in implementation, invariants are here.

### 2.4 EditingState

Central object for a single editable paragraph:

```rust
pub struct EditingState<B: TextBuffer> {
    pub buffer: B,
    pub cursor: TextCursor,
    pub selection: Option<TextSelection>,
    pub clusters: Vec<ClusterBoundary>,
    pub preferred_x: Option<f32>, // for vertical navigation
}
```

Responsibilities:

* Maintain invariants for cursor and selection.
* Keep `clusters` matching current laid-out text:

    * Rebuild required on text change, style change, or width change.
* Provide all editing and navigation operations as **pure logic**.

---

## 3. Invariants (Hard Rules)

These must always hold after any operation:

1. Cursor invariants

    * `0 <= cursor.pos <= buffer.len()`
    * `cursor.pos` is a legal caret position:

        * Either `0`, `buffer.len()`, or `byte_start` of some `ClusterBoundary`.

2. Selection invariants

    * If `selection.is_some()`:

        * Both `anchor` and `pos` obey cursor invariants.
    * Selection range is half-open:

        * `start = min(anchor, pos)`
        * `end   = max(anchor, pos)`.
    * `start` and `end` are legal caret positions.

3. Cluster alignment

    * Every legal caret position is either:

        * Start of text
        * End of text
        * Or `ClusterBoundary.byte_start`.

4. UTF-8 validity

    * After any text operation, buffer must contain valid UTF-8.
    * Insert functions reject or sanitize invalid UTF-8.

5. No partial cluster deletion

    * Deletion ranges always align with `[byte_start, byte_end)` of whole clusters.
    * No deletion splits a grapheme/shaping cluster.

6. Consistency with ParagraphEngine

    * `clusters` must be built from up-to-date layout.
    * After text mutation, editing code marks clusters “dirty”.
    * Caller is responsible for re-building clusters before the next movement/hit-test; editing functions must never use stale layout silently.

---

## 4. Mouse Interaction Semantics

This is the canonical behavior. Widgets use it via simple calls into EditingState and helpers.

### 4.1 Single click (no modifiers)

1. Hit-test pointer into paragraph-local logical coordinates `(x, y)`.
2. Use `ClusterBoundary::hit_test` to get `byte_offset`.
3. Snap to legal caret position:

    * `pos = snap_to_cluster(byte_offset)` via `find_boundary_at`.
4. Set:

    * `cursor.pos = pos`
    * `selection = None` (clear selection).
    * `preferred_x = Some(cluster.x_logical)`.

Outside bounds:

* Click before first cluster → `pos = 0`.
* Click after last cluster in last line → `pos = buffer.len()`.

### 4.2 Click + drag (character-wise selection)

On mouse down:

* Anchor is established:

    * `pos = snapped_offset_at_down`
    * `selection = None`

Option A (recommended): store anchor separately in widget, call editing API:

* `selection = Some(TextSelection { anchor: pos, pos })`

On mouse move while button down:

* Compute hit-test offset.
* Snap to cluster.
* Update:

    * `selection = Some(TextSelection { anchor, pos: new_pos })`
    * `cursor.pos = new_pos`.

On mouse up:

* Selection stays as-is.

No hysteresis, no thresholds: anchor at down, active at last move.

### 4.3 Double-click (word selection)

Double-click at offset `i`:

1. Snap `i` to nearest caret boundary.

2. Compute word boundaries:

    * `word_start = scan_word_start(buffer, i)`
    * `word_end = scan_word_end(buffer, i)`

3. Set:

    * `cursor.pos = word_end`
    * `selection = Some(TextSelection { anchor: word_start, pos: word_end })`

Word rules:

* Word characters: Unicode letters, digits, underscore `_`.
* Non-word characters break words.
* Optional: treat `'` between word characters as part of word (`don't`).

### 4.4 Double-click + drag (word-wise selection)

* Double-click defines initial word range `[w0_start, w0_end)`, anchor at its center.
* When dragging:

    * At pointer index `j`, compute `[w1_start, w1_end)`.

Rules:

* If `j >= (w0_start + w0_end) / 2`:

    * select `[w0_start, w1_end)`

* Else:

    * select `[w1_start, w0_end)`

Selection always expands/shrinks in full-word chunks.

### 4.5 Triple-click (line selection)

Triple-click at `(line_index, _)`:

1. Compute logical line boundaries:

    * `line_start` = byte index of first char in that logical line.
    * `line_end`   = byte index of `\n` or end-of-text.

2. Set:

    * `cursor.pos = line_end`
    * `selection = Some(TextSelection { anchor: line_start, pos: line_end })`

Question of including the newline:

* Default for Halcyon:

    * Selection **does not include** the `\n` by default.
    * SPEC-055 (clipboard/undo) may define “copy line with newline” behavior.

### 4.6 Triple-click + drag (line-wise selection)

* Triple-click defines initial line `L0`.
* Dragging up/down selects whole logical lines from `L0` to `Lk`.
* Selection:

    * From start of first selected line
    * To end of last selected line (without trailing `\n`).

---

## 5. Keyboard Movement (No Selection)

Base rule:

> Movement keys (without Shift) move the caret and **clear selection**.

After any movement without Shift:

* `selection = None`
* `anchor = cursor.pos` conceptually (if selection struct is re-used; or simply no selection).

### 5.1 Left / Right arrow

Using grapheme/cluster navigation:

* Left:

    * If `cursor.pos > 0`:

        * Move to previous legal caret position (previous cluster boundary).

* Right:

    * If `cursor.pos < buffer.len()`:

        * Move to next legal caret position.

After move:

* `selection = None`
* `preferred_x = Some(current_cluster.x_logical)` (for vertical navigation).

### 5.2 Ctrl/Alt + Left/Right (word-wise movement)

Platform mapping is outside this SPEC; semantic is the same.

* Ctrl+Left:

    * Move to start of previous word.
    * If inside a word: move to its start.
    * Else, skip backwards over non-word chars to previous word start.

* Ctrl+Right:

    * Move to end of current/next word:

        * If inside a word: move to its end.
        * If at boundary or separator: move to next word’s end.

Word scanning uses the same word-character rules as double-click.

### 5.3 Up / Down arrow

Vertical movement uses visual lines and x position:

* Maintain `preferred_x`:

    * On any horizontal movement or click: `preferred_x = cluster.x_logical`.
    * On vertical movement: if `preferred_x` is None, set it to current `cluster.x_logical`, then move.

* Up:

    * Find current cluster in `clusters`.
    * Using its `line_index` and `x_logical` (or `preferred_x`), move to cluster on previous visual line whose `x_logical` is closest to `preferred_x`.
    * If already on first line: clamp to first line.

* Down:

    * Same, but next visual line.
    * If already on last line: clamp to last line.

After vertical movement:

* `selection = None`
* `preferred_x` preserved.

### 5.4 Home / End

By default, Home/End operate on **logical lines**, not whole document.

* Home:

    * `cursor.pos = logical_line_start(current_pos)`

* End:

    * `cursor.pos = logical_line_end(current_pos)` (before `\n` if present).

With Ctrl (or Command):

* Ctrl+Home: `cursor.pos = 0`
* Ctrl+End: `cursor.pos = buffer.len()`

After move:

* `selection = None`
* `preferred_x` updated to new `cluster.x_logical`.

---

## 6. Keyboard Selection (Shift Variants)

Core rule:

> With Shift held, **only `pos` moves**. `anchor` stays where it was when selection started.

Formal:

* If there was no selection before:

    * On first Shift + movement:

        * `anchor = cursor.pos` (old pos before movement).
        * `pos = new_cursor_pos`.

* If selection already exists:

    * Movement updates only `pos` (anchor unchanged).

Applies to:

* Shift+Left/Right – character-wise selection
* Shift+Ctrl+Left/Right – word-wise selection
* Shift+Up/Down – line-wise selection by visual navigation
* Shift+Home/End – to start/end of logical line
* Ctrl+Shift+Home/End – start/end of document

---

## 7. Delete / Backspace Semantics

### 7.1 With selection present

If `selection.is_some()` and `start != end`:

* Backspace OR Delete:

    * Remove text in `[start, end)`.
    * `cursor.pos = start`
    * `selection = None`.

This is symmetric: both keys delete the selected range.

### 7.2 Without selection

If there is no selection:

* Backspace:

    * If `cursor.pos == 0`: no-op.
    * Else:

        * Find previous cluster boundary before `cursor.pos` via `clusters`.
        * Delete `[prev.byte_start, cursor.pos)`.
        * `cursor.pos = prev.byte_start`.

* Delete:

    * If `cursor.pos == buffer.len()`: no-op.
    * Else:

        * Find cluster whose `byte_start == cursor.pos` (or nearest forward).
        * Delete `[curr.byte_start, curr.byte_end)`.
        * `cursor.pos = curr.byte_start`.

### 7.3 Ctrl + Backspace / Ctrl + Delete (word-based)

Without selection:

* Ctrl+Backspace:

    * Delete “previous word”:

        * `start = word_start_before(cursor.pos)`
        * `end   = cursor.pos`
        * Delete `[start, end)`
        * `cursor.pos = start`.

* Ctrl+Delete:

    * Delete “next word”:

        * `start = cursor.pos`
        * `end   = word_end_after(cursor.pos)`
        * Delete `[start, end)`
        * `cursor.pos = start`.

With selection:

* Same as normal backspace/delete: delete selection.

---

## 8. Breaking Out of a Selection

When there is a selection and you press Left/Right **without** Shift:

Common editor convention:

* Left:

    * Caret goes to **start** of selection:

        * `cursor.pos = start`
        * `selection = None`

* Right:

    * Caret goes to **end** of selection:

        * `cursor.pos = end`
        * `selection = None`

Up/Down/Home/End without Shift:

* Collapse selection to the **movement result** position:

    * `cursor.pos = new_position`
    * `selection = None`.

---

## 9. Hit Testing (Coordinates → Caret)

Hit-testing is defined on top of `ClusterBoundary` and ParagraphEngine lines.

Given:

* Paragraph-local coordinates `(x, y)` in logical units (after scale/device transform reversal).
* `lines: &[Line]` from ParagraphEngine.
* `boundaries: &[ClusterBoundary]`.

Algorithm:

1. Map `y` to a **visual line index**:

    * Use `line_from_y(lines, y)`:

        * Estimate line height as average of `lines[i].height`.
        * `line_index = clamp(floor(y / avg_height), 0, lines.len() - 1)`.

2. Collect all clusters on that line.

3. If none:

    * Search nearest non-empty line above/below in increasing distance.

4. On chosen line:

    * Find cluster whose `x_logical` is closest to `x`.
    * Return its `byte_start`.

5. Snap to caret using `find_boundary_at`; this guarantees legal cluster boundary.

This hit-test **always** returns a valid caret position between 0 and `text.len()`.

---

## 10. Integration with SPECs 090–100

This SPEC defines semantics; 090–100 define constraints it must obey.

* 090 – Logical Grid System

    * Caret, selection rectangles must align to the grid.
    * Visual rendering must snap to pixel grid and layout grid units.
    * Editing does not change; painting uses grid-snapped geometry.

* 091 – Adaptive Density Modes

    * Hit area, line heights can shrink/expand with density.
    * `line_from_y` uses actual line metrics from ParagraphEngine, not magic numbers.

* 092 – Typography Rules

    * Minimum font size, line spacing, default fonts.
    * Editing model unaffected; cluster map and caret just use actual metrics.

* 093 – Visual Appearance & Radii

    * Selection highlight, caret thickness, rounding of selection corners.
    * Visual only; selection ranges controlled by this SPEC.

* 094 – Component Geometry & Layout

    * TextField internal padding, margins, scrollable content area.
    * Hit-test map `(pointer_x, pointer_y)` into paragraph-local `(x,y)` before applying this SPEC.

* 095 – Iconography & Pixel Grid

    * Not directly relevant; icons in text editors follow general grid rules.

* 096 – Color System & Semantic Palette

    * Caret color, selection colors come from semantic palette; this SPEC only requires:

        * caret must be visibly distinct from selection
        * selection must honor contrast requirements.

* 097 – Motion & Micro-Animation

    * Cursor blink, smooth scroll, selection fade; decoupled from selection semantics.

* 098 – Cursor & Pointer System

    * Widget must switch pointer cursor appropriately:

        * text I-beam over editable areas
        * default arrow outside
    * This SPEC defines where the text area is editable; 098 defines cursor shapes.

* 099 – Input Precision & Interaction Model

    * Interaction thresholds (minimum drag distance, multi-click timing).
    * This SPEC assumes the higher layer already resolves:

        * single vs double vs triple click
        * drag start vs click
        * then calls the corresponding semantic operations.

* 100 – Widget Interaction State Machine

    * Focused vs unfocused vs disabled state gating.
    * Editing operations are only active in the **Focused + Enabled** state.
    * Read-only mode uses same caret/hit-test semantics but disallows mutations.

---

## 11. Implementation Plan (Stages for Claude)

This is the concrete sequence you give Claude. Each stage must compile and keep tests green.

### Stage 0 – Skeleton

* Add `TextBuffer` trait and `SimpleTextBuffer`.
* Add `TextCursor`, `TextSelection`, `ClusterBoundary`.
* Implement basic invariants and helpers (start, end, is_empty, normalized).

### Stage 1 – Cluster Map

* Integrate with ParagraphEngine’s `Line` and glyph runs.
* Implement `ClusterBoundary::build_from_lines(lines, text)` 2-pass algorithm.
* Implement `find_boundary_at` and `hit_test` as defined above.
* Unit tests:

    * simple ASCII
    * combining accents
    * emoji/ZWJ clusters
    * multiple visual lines.

### Stage 2 – EditingState core

* Add `EditingState<B: TextBuffer>` with:

    * `new(initial: impl Into<String>)`
    * `rebuild_clusters(lines: &[Line], text: &str)` or similar glue.
* Basic navigation:

    * `move_left`, `move_right`
    * `move_to_start`, `move_to_end`
* Selection struct present but not yet used.

### Stage 3 – Selection and Keyboard Movement

* Implement:

    * `move_left()`, `move_right()` without Shift → clear selection.
    * `move_left_select()`, `move_right_select()` (Shift versions) → anchor semantics.
    * `move_up`, `move_down` using visual lines and `preferred_x`.
    * `move_up_select`, `move_down_select`.
    * `move_home`, `move_end`, `move_home_select`, `move_end_select`.
    * Word-wise movement: `move_word_left`, `move_word_right`, and Shift variants.

* Unit tests for all keyboard movement and selection scenarios (ASCII + emoji + combining).

### Stage 4 – Deletion and Insert

* Implement:

    * `insert_text(&mut self, s: &str)`
    * `backspace(&mut self)`
    * `delete_forward(&mut self)`
    * `delete_word_back(&mut self)`
    * `delete_word_forward(&mut self)`

* Respect selection rules:

    * If selection exists, both Backspace and Delete delete the selection.

* Enforce cluster boundaries when deleting.

* Tests:

    * Delete behavior with/without selection.
    * Word delete behavior.
    * No partial cluster deletion.

### Stage 5 – Mouse helpers

* Implement pure functions or methods for:

    * `click_at(x, y)`
    * `drag_to(x, y)` (character-wise)
    * `double_click_at(x, y)` (word selection)
    * `double_click_drag_to(x, y)` (word-wise)
    * `triple_click_at(line_index)`
    * `triple_click_drag_to(line_index)`

* These functions accept pre-computed hit-test info (cluster/line indices); they don’t know about pixels directly.

### Stage 6 – Integration (SPEC-054)

* SPEC-054 will wire this into a TextField widget using:

    * Fused key events → editing ops
    * Fused text input events → `insert_text`
    * Pointer events → click/double/triple/drag helpers
    * Layout results → cluster map rebuilds.
