# SPEC-HAL-315: Read-Only Text Selection

### *Subsystem for Selecting and Copying Non-Editable Text*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-002 (Constitution)
* SPEC-HAL-047–067 (Text & Font System)
* SPEC-HAL-092 (Typography Rules)
* SPEC-HAL-103/104 (Font Metrics & Pixel Stability)

---

# 1. Purpose

Halcyon widgets such as TextBlock and Label can opt into shared read-only
text selection. TextField and TextArea own editable text with selection,
cursor, undo, and IME support via `core/editing.rs`.

This SPEC defines a reusable, policy-governed, read-only text selection
subsystem that any widget can opt into. It covers single-run and multi-run
text regions. It does NOT cover text editing, cursor rendering, IME, or
undo/redo — those remain the domain of TextField/TextArea.

---

# 2. Design Principles

**DP-1: Read-only.** No insertion, deletion, or mutation. Selection exists
solely for copy-to-clipboard. No caret, no blinking cursor, no undo.

**DP-2: Opt-in.** Widgets that want selectable text embed the selection
component and forward pointer events. Widgets that don't want it change
nothing. No global mode, no ambient behavior.

**DP-3: Policy-governed.** A `TextSelectionPolicy` enum controls when
selection is available. Default is `Disabled`. Widgets set their policy
explicitly. The subsystem enforces it.

**DP-4: Reuse existing infrastructure.** The text engine already has
`GlyphRun` (shaped glyphs with advances), `TextBackgroundSpan` (highlight
rects behind text), and `draw_with_backgrounds`. The `core/editing.rs`
module has `Selection` (anchor/head), word boundary detection, and line
boundary detection. Console has proven hit-test logic. This SPEC composes
these, not reinvents them.

**DP-5: Ephemeral rendering.** Selection highlight rects are computed at
paint time from the selection state and the current GlyphRun. No persistent
highlight geometry. Consistent with Constitution §3.9 (no persistent
references to ephemeral layout structures).

---

# 3. Scope

## 3.1 In Scope

* Hit-testing: pointer (x, y) → character index within a shaped text run
* Selection state: anchor + head byte offsets within a text string
* Selection gestures: click-to-place, drag-to-extend, double-click word,
  triple-click line/all, Shift+click extend
* Selection rendering: highlight rects behind selected text
* Clipboard: Ctrl+C / Cmd+C copies selected substring
* Multi-run selection: selection spanning multiple text runs within a
  single widget (e.g., Console entries, expansion region text blocks)
* Policy enforcement: per-widget control of when selection is active

## 3.2 Out of Scope

* Text editing (insert, delete, replace) — TextField/TextArea
* Cursor rendering (blinking caret) — TextField/TextArea
* IME composition — TextField/TextArea
* Undo/redo — TextField/TextArea
* Cross-widget selection (selecting text across multiple widgets) —
  would require a global selection coordinator; deferred
* Drag-to-move text — editing operation, not selection

---

# 4. Types

## 4.1 TextSelectionPolicy

```rust
/// Controls when read-only text selection is available.
///
/// SPEC-HAL-315 §4.1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSelectionPolicy {
    /// No selection. Text is purely visual. Default.
    Disabled,

    /// Selection available when the widget has focus or capture.
    Enabled,

    /// Selection available only in Capture mode (entered via `i` or
    /// explicit capture request). Prevents accidental selection during
    /// casual navigation.
    OnCapture,

    /// Selection available only while a modifier key is held.
    /// Shift is the conventional choice — Shift+click to start,
    /// Shift+drag to extend.
    OnModifier,
}

impl Default for TextSelectionPolicy {
    fn default() -> Self {
        Self::Disabled
    }
}
```

## 4.2 ReadOnlyTextSelection

```rust
/// Read-only text selection state for a single text region.
///
/// SPEC-HAL-315 §4.2
///
/// Reuses `core::editing::Selection` (anchor/head byte offsets).
/// The widget supplies the current source text for clipboard extraction.
///
/// INV-315-1: anchor and head are UTF-8 boundaries in the stored text.
pub struct ReadOnlyTextSelection {
    /// Current selection range (may be empty: anchor == head).
    selection: Selection,

    /// The text content this selection refers to.
    /// Stored for clipboard extraction. Updated by the widget
    /// whenever the underlying text changes.
    text: String,

    /// Active policy.
    policy: TextSelectionPolicy,

    /// Whether selection is currently active (policy conditions met).
    active: bool,

    /// Drag state for pointer gestures.
    drag: SelectionDrag,
}
```

## 4.3 SelectionDrag

```rust
/// Pointer drag state for selection gestures.
///
/// SPEC-HAL-315 §4.3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionDrag {
    /// No drag in progress.
    Idle,
    /// Character-level drag (single click + drag).
    Character,
    /// Word-level drag (double-click + drag).
    Word { anchor_start: usize, anchor_end: usize },
    /// Line-level drag (triple-click + drag).
    Line,
}
```

---

# 5. Bound Run Geometry

## 5.1 Hit-Test: Pixel → Byte Offset

The free functions in `core::text_selection` read the current bound
`GlyphRun`.

```rust
pub fn hit_test_run(run: &GlyphRun, target_x: f32) -> usize;
```

With source clusters available, the hit uses the nearest visual cluster
edge and its direction to return the corresponding logical byte offset.

## 5.2 Reverse Map: Byte Offset → Pixel X

```rust
pub fn x_for_offset(run: &GlyphRun, byte_offset: usize) -> f32;
```

The result is relative to the run origin and uses the bound cluster's
direction when available.

## 5.3 Selection Rects

```rust
pub fn selection_spans(
    run: &GlyphRun,
    start: usize,
    end: usize,
    line_height: f32,
    y_offset: f32,
    color: Color,
) -> Vec<TextBackgroundSpan>;
```

The function covers the selected clusters with one or more background
spans in visual order. It uses the caller's line height and offset.

---

# 6. ReadOnlyTextSelection API

## 6.1 Construction

```rust
impl ReadOnlyTextSelection {
    /// Create with the given policy and initial text.
    pub fn new(policy: TextSelectionPolicy, text: String) -> Self;

    /// Update the text content and clear the selection if its bytes change.
    pub fn set_text(&mut self, text: String);

    /// Set policy. Clears selection if new policy disallows it.
    pub fn set_policy(&mut self, policy: TextSelectionPolicy);
}
```

## 6.2 Gesture Handling

The widget calls these from its `handle_event`:

```rust
impl ReadOnlyTextSelection {
    /// Pointer down. Begins selection if policy allows.
    /// `byte_offset` is the hit-tested position from §5.1.
    /// `click_count`: 1 = character, 2 = word, 3 = line/all.
    /// `shift`: true if Shift held (extend from anchor).
    ///
    /// Returns true if the gesture was accepted.
    pub fn pointer_down(
        &mut self,
        byte_offset: usize,
        click_count: u32,
        shift: bool,
        in_capture: bool,
    ) -> bool;

    /// Pointer move during drag. Extends selection.
    /// `byte_offset` is the current hit-tested position.
    pub fn pointer_move(&mut self, byte_offset: usize);

    /// Pointer up. Finalizes drag.
    pub fn pointer_up(&mut self);

    /// Select all text. Typically bound to Ctrl+A.
    pub fn select_all(&mut self);

    /// Clear selection (collapse to empty).
    pub fn clear(&mut self);
}
```

## 6.3 Query

```rust
impl ReadOnlyTextSelection {
    /// Whether there is a non-empty selection.
    pub fn has_selection(&self) -> bool;

    /// Get the selected text substring.
    pub fn selected_text(&self) -> &str;

    /// Get the selection range (start, end) in byte offsets.
    pub fn range(&self) -> (usize, usize);

    /// Return the selected substring for the caller's clipboard write.
    pub fn copy_to_clipboard(&self) -> Option<String>;
}
```

---

# 7. Widget Integration Pattern

A widget that wants selectable text follows this pattern:

```rust
struct MyWidget {
    // ... existing fields ...
    text_selection: ReadOnlyTextSelection,
}

impl Widget for MyWidget {
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let run = cx.text_engine.shape(&self.text, font_size);
        let text_x = rect.x + padding;
        let text_y = rect.y + ...;

        let (start, end) = self.text_selection.range();
        if start < end {
            let spans = selection_spans(
                &run, start, end, run.height, 0.0,
                cx.theme.colors.selection.background,
            );
            cx.text_engine.draw_with_backgrounds(
                &run, text_x, text_y, text_color, spans, text_z, cx.scene,
            );
        } else {
            cx.text_engine.draw(&run, text_x, text_y, text_color, text_z, cx.scene);
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if let Event::Pointer(pe) = event {
            match pe.kind {
                PointerEventKind::Down => {
                    // Hit-test against the current bound run.
                    let local_x = pe.local_pos.0 - padding;
                    let run = ...; // cached or re-shaped
                    let offset = hit_test_run(&run, local_x);
                    let shift = pe.modifiers.shift;
                    let click_count = ...; // from PointerDown/DoubleClick/TripleClick
                    return self.text_selection.pointer_down(
                        offset, click_count, shift, self.in_capture, shift,
                    );
                }
                PointerEventKind::Move if self.text_selection.is_dragging() => {
                    let offset = hit_test_run(&run, local_x);
                    self.text_selection.pointer_move(offset);
                    return true;
                }
                PointerEventKind::Up => {
                    self.text_selection.pointer_up();
                    return true;
                }
                _ => {}
            }
        }
        // Cmd+C on macOS, Ctrl+C on other platforms.
        if let Event::Text(te) = event {
            let copy_modifier = if cfg!(target_os = "macos") {
                te.modifiers.meta
            } else {
                te.modifiers.ctrl
            };
            if phase == EventPhase::Target && copy_modifier && matches!(te.text.as_str(), "c" | "C")
            {
                if let Some(text) = self.text_selection.copy_to_clipboard() {
                    cx.clipboard_set_text(&text);
                    return true;
                }
            }
        }
        false
    }
}
```

The widget owns the text content and maps positions using current layout
and bound runs. `ReadOnlyTextSelection` owns selection state, policy
enforcement, and gesture interpretation.

---

# 8. Invariants

**INV-315-1:** `selection.anchor` and `selection.head` lie on UTF-8 scalar
boundaries in the current text. See §14 for gesture offset normalization.

**INV-315-2:** Selection state is never modified during layout or paint
phases. Gesture handling occurs in event phase only. Paint reads selection
state immutably.

**INV-315-3:** Policy is enforced at gesture entry (`pointer_down`). If
the policy does not permit selection in the current state, the gesture
is rejected (returns false). No partial selection can start under a
disallowed policy.

**INV-315-4:** A change to the source bytes via `set_text` clears the
selection and ends the drag. Replacing identical bytes preserves both.

**INV-315-5:** The subsystem does not store references to GlyphRun,
TextEngine, or PaintSink. All rendering queries accept these as
parameters. Consistent with Constitution §3.9.

---

# 9. Forbidden Behaviors

**FORBID-315-1:** No text mutation. `ReadOnlyTextSelection` must not
insert, delete, or modify text content. It reads and selects only.

**FORBID-315-2:** No cursor rendering. No blinking caret, no insertion
point indicator. If the selection is empty, nothing is visible.

**FORBID-315-3:** No IME integration. Read-only selection does not
accept text input events for composition.

**FORBID-315-4:** No cross-widget selection. Selection is scoped to a
single widget instance. No global selection manager, no linked
selection across widget boundaries.

**FORBID-315-5:** No ambient selection. Text is not selectable by
default. The widget must explicitly create a `ReadOnlyTextSelection`
and forward events to it. Labels, tree nodes, cell text, etc. remain
non-selectable unless the consumer widget opts in with an appropriate
policy.

---

# 10. Console Migration Path

Console currently has its own selection infrastructure:
`ConsoleTextPosition`, `ConsoleTextSelection`, `byte_offset_at_x`,
`x_offset_for_byte`, `find_word_boundaries`, `extract_selection_text`.

After this SPEC is implemented, Console should be migrated to use the
shared subsystem. This is a separate task, not part of this SPEC's
implementation.

Migration steps:
1. Replace `ConsoleTextSelection` with `ReadOnlyTextSelection`
2. Replace `byte_offset_at_x` calls with `core::text_selection::hit_test_run`
3. Replace `x_offset_for_byte` calls with `core::text_selection::x_for_offset`
4. Replace `find_word_boundaries` calls with `core::editing::find_word_at`
5. Remove Console-local selection types and free functions

---

# 11. Implementation Phases

**Phase 1 — Core subsystem:**
`src/core/text_selection.rs` (or `mod.rs`). Types: `TextSelectionPolicy`,
`ReadOnlyTextSelection`, `SelectionDrag`. API: pointer_down/move/up,
select_all, clear, has_selection, selected_text, range.

**Phase 2 — Text engine extensions:**
`hit_test_run`, `x_for_offset`, `selection_spans` on `TextEngine` trait.
Extract from Console's proven implementations.

**Phase 3 — Widget integration:**
Opt Label or expansion region into selectable text as the first consumer.
Validate the integration pattern works end-to-end.

**Phase 4 — Console migration:**
Replace Console's ad-hoc selection with the shared subsystem.

---

# 12. Conclusion

This SPEC introduces read-only text selection as a composable, policy-
governed subsystem. It reuses existing infrastructure (Selection type,
GlyphRun, TextBackgroundSpan, draw_with_backgrounds, word boundaries)
and extracts proven hit-test logic from Console into the text engine
trait. Widgets opt in explicitly. No ambient behavior, no global state,
no editing capability. Selection exists for one purpose: letting users
copy text they can see.

## 13. Scoped native Markdown read-only selection adoption - 2026-11-13

**Status: ADOPTED / ACTIVE only for HAL390 section14.** The rest of this Draft
retains its status. Reuse the actual ReadOnlyTextSelection/Selection owner for
ONE MarkdownView's decoded reading text. The view maps actual bound clusters,
blocks, cells and code-tab visual segments into that one logical string.
No cross-widget/global coordinator, rich editor, cut/paste, IME or undo is
admitted. Default remains Disabled; the consumer explicitly selects Enabled.

The view supplies offsets from its actual cluster geometry. The state
holder enforces the UTF-8 safety rules in §14. The existing routed interaction
owner changes selection. Same-mounted-document resize retains logical
anchor/head. A different document or actual remount resets selection.

Cmd/Ctrl+C and Cmd/Ctrl+A follow the current `Event::Text` route.
Cmd/Ctrl+Shift+C on this eligible view
copies its entire exact Markdown source. Ordinary selection copy returns
decoded reading text with no soft-wrap breaks or geometry markers. Both use
the existing EventContext clipboard write; no implicit clipboard read.
HAL390 section14 controls these precise copy meanings and phase boundaries.

## 14. Scoped TextBlock line positions and UTF-8 selection safety

**Status: ACTIVE for this correction.** Sections outside this correction
and the existing HAL390 adoption retain their prior status.

- Paragraph layout supplies a logical source insertion position for every
  visual line from the same wrap and hard-break computation that produces
  its line box. A zero-run line terminated by LF maps to that LF's byte
  position. An empty line after a trailing LF maps to the end of the source.
- TextBlock reads that position for an empty line in its current layout.
  On nonempty lines, TextBlock uses the bound run's cluster hit testing.
  Rewrapping updates the line positions and preserves selection offsets
  into unchanged source.
- ReadOnlyTextSelection bounds offered gesture offsets to the source and
  floors interior UTF-8 offsets to the preceding scalar boundary. Widget hit
  testing supplies the actual cluster boundaries. Changed source bytes clear
  stale offsets and cancel the drag before any slice or highlight is read.
- The mounted TextBlock handles selection and copy through ordinary routed
  pointer events and the focused platform copy chord. It copies the exact
  selected source substring, including original LF bytes across blank lines.
  Soft wrapping introduces no copy characters. Paint reads selection for
  text highlights. EventLog row selection and structured row formatting
  remain with the parent under HAL312 and HAL313.
