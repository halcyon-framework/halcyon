**SPEC-HAL-013: TextField — Full Specification (Cursor, Selection, IME, Scrolling, Validation)**
**Author:** JC
**Date:** 2025-11-17
**Status:** Draft
**Scope:** The *complete* design for Halcyon’s TextField — the most complex standard widget.
**Purpose:** Establish the exact behavior, internal state, event handling, layout rules, painting, IME flow, and validation hooks.
**Primary Use-Case:** Trading UIs (order quantity, limit price, quick hotkey entry), config forms, code/text editing primitives.

This is the most complex Tier-1 widget. This SPEC is long, strict, and exhaustive.

---

# 1. Why TextField Is Hard

A proper text field requires:

* typed text input
* deletion
* cursor positioning
* selection
* double-click word selection
* triple-click line selection (optional)
* continuous key repeat
* modifier key logic (Ctrl/Cmd)
* clipboard (cut/copy/paste)
* internal horizontal scrolling
* text measurement
* IME composition support
* focus acquisition & loss rules
* painting caret and selection overlays
* validation and input filters
* event phase correctness
* pointer capture during drag-selection

This is unavoidable. Any half-baked TextField ruins an entire UI framework.

This SPEC defines the **minimum correct TextField that never needs a rewrite**.

---

# 2. Public API

TextField is constructed declaratively:

```rust
TextField::new()
    .value(&state.username)
    .on_change(|new_value, app| app.username = new_value.to_string())
    .placeholder("email")
    .select_all_on_focus(true)
    .password_mode(false)
```

**Props:**

```rust
pub struct TextFieldProps {
    pub text: String,
    pub placeholder: Option<String>,
    pub password_mode: bool,          // show bullets
    pub select_all_on_focus: bool,
    pub max_len: Option<usize>,       // optional validation
}
```

**Callbacks:**

```rust
pub on_change: Box<dyn Fn(&str, &mut AppState)>
pub on_submit: Option<Box<dyn Fn(&str, &mut AppState)>>
pub on_cancel: Option<Box<dyn Fn(&mut AppState)>>
```

---

# 3. Internal Widget State

Every TextField instance stores private state:

```rust
pub struct TextFieldState {
    pub cursor: usize,            // byte index into text
    pub selection: Option<(usize, usize)>,  // selection range (start <= end)
    pub scroll_x: f32,            // horizontal scroll amount
    pub ime_active: bool,         // true while IME is composing
    pub ime_range: Option<(usize, usize)>,  // composition range
    pub has_focus: bool,
    pub mouse_select_origin: Option<usize>, // for drag selection
}
```

### Notes:

* Cursor & selection use **byte indices**, not graphemes.
* We operate on UTF-8 but selection/cursor movement must *visually* iterate by grapheme cluster.
* IME composition operates on a temporary text overlay.

---

# 4. Layout

TextField respects:

* padding (theme)
* border widths
* text sizes

### Layout algorithm:

1. Width: shrink-to-fit unless Row/Column allocate more.
2. Height: single-line, fixed to `theme.text.normal` + vertical padding.
3. Content width: measure full text width for scroll logic.

Return:

```rust
Size {
    width: constraints.max_width,
    height: theme.text.normal + padding*2,
}
```

---

# 5. Painting

### Layers (in order):

1. Background (rect, radius)
2. Border (focused = primary color)
3. Text (password-mode uses • for each grapheme)
4. Selection highlight (rect behind text)
5. IME composition underline (if active)
6. Caret (blinking vertical bar)

### Caret blink:

Handled by global timing tick.
State doesn’t store blink; renderer stores time.

### Scroll offset:

Text is drawn at `x = -scroll_x + padding`.

---

# 6. Event Handling

The heavy part.

Halcyon’s event phases apply:

* **Capture** phase is ignored by TextField.
* **Target** phase performs core logic.
* **Bubble** phase is ignored (except Enter for submission).

---

# 7. Pointer Events

### PointerDown

* Acquire focus.
* Compute clicked character index → move cursor.
* If shift is down → extend selection.
* Else → clear selection.
* Start drag-selection:

  ```rust
  mouse_select_origin = Some(clicked_index);
  capture pointer
  ```

### PointerMove

If pointer is captured:

* Update selection endpoint to new hovered index.
* Auto-scroll horizontally if moving outside bounds.

### PointerUp

* Release pointer capture.
* Selection remains.

### DoubleClick

Select word:

* Expand selection to nearest word boundaries.

### TripleClick (optional, later)

Select entire line.

---

# 8. Keyboard Events

TextField is the **target** widget only when focused.

### 8.1 Navigation

ArrowLeft:

* If no selection → move cursor left (prev grapheme)
* If selection → collapse cursor to start

ArrowRight:

* Mirror of left.

Ctrl+ArrowLeft/Right:

* Jump by word.

Home:

* Cursor = 0.

End:

* Cursor = text.len().

Shift+…:

* Extend selection.

### 8.2 Editing

Backspace:

* If selection → delete selection
* Else delete previous grapheme

Delete:

* Similar but forward.

Ctrl+Backspace / Ctrl+Delete:

* Delete word backward/forward.

### 8.3 Text Input (Printable Characters)

Handled through KeyEvent.text:

* Insert at cursor.
* Replace selection if exists.
* Update cursor.
* Re-clamp scroll_x.
* Fire on_change callback.

### 8.4 Enter

* If on_submit is set:

    * Call on_submit(&text, app)
* Else:

    * No special behavior.

### 8.5 Escape

* If on_cancel:

    * Call on_cancel(app)
* Else:

    * Clear selection, remove focus.

### 8.6 Tab

* Bubble to FocusManager (switch focus).
* TextField does not consume Tab.

---

# 9. IME Composition (Critical)

IME composition is required for:

* Chinese
* Japanese
* Korean
* Various accented Latin input
* Linux + ibus
* macOS input methods

### Composition flow:

#### Event: `StartComposition`

* `ime_active = true`
* `ime_range = (cursor, cursor)`

#### Event: `UpdateComposition(text)`

* For display:

    * Overlay composition string on top of existing text.
* For selection:

    * Show underline.

#### Event: `EndComposition(text)`

* Replace ime_range with final text.
* Move cursor to end of inserted text.
* Clear ime_active.

#### Event: `CancelComposition`

* Discard any composition text.
* Restore cursor to ime_range.start.
* Clear ime_active.

### Painting IME composition:

* Text is blended as if temporarily inserted.
* Underline is drawn (theme.text.ime_underline_color).

---

# 10. Scrolling

TextField scrolls horizontally automatically.

### Rules:

1. If caret moves left of visible area → reduce scroll_x.
2. If caret moves right of visible area → increase scroll_x.
3. Scrolling does not exceed content width.

### Measuring text:

Use font subsystem (SPEC-HAL-006) for:

```rust
measure_text(&str, size) -> f32
```

### Scroll drag with middle mouse (optional later).

---

# 11. Selection Behavior

Selection is a simple tuple: `(start, end)`.

### Mouse selection:

* Origin = where mouse went down.
* New index = where mouse is now.
* selection = (min, max).

### Shift+Arrow:

* If selection is None:

    * selection = (cursor_before, cursor_after)
* Else:

    * extend selection.

### Double-click:

* Word select.

### Painting:

* Selection is painted BEFORE text, as a translucent rectangle.

---

# 12. Clipboard (Later SPEC)

TextField needs:

* Copy
* Cut
* Paste

But clipboard implementation depends on OS integration (Winit).
We will add this after core TextField is working.

For now: internal hooks only.

---

# 13. Validation

TextField exposes:

```rust
on_validate: Option<fn(&str) -> bool>
```

### Validation timing:

* After every input & after composition ends.
* If invalid → reject change (do not commit to state).
* Still allow editing; just don’t push invalid value through on_change.

Used for numeric fields in trading UI:

* Price
* Quantity
* Tick size alignment

---

# 14. Performance Requirements

* No dynamic allocations during paint/layout.
* Cursor positioning via cached glyph measurements.
* Text measuring caches offsets per grapheme.
* Scroll logic O(1).
* Selection O(1).
* Event handler must not allocate except when replacing text (string ops).

---

# 15. Implementation Checklist for Claude Code

### 15.1 State

* Add TextFieldState exactly as described.

### 15.2 Props

* Add TextFieldProps + builder-style API.

### 15.3 Rendering

* Background
* Border (focused)
* Selection rectangles
* IME overlay
* Caret
* Text (with scroll_x)

### 15.4 Selection & Cursor logic

* Grapheme iteration
* Word boundaries
* Byte-index safety
* Position-to-character mapping

### 15.5 Event Handling

* PointerDown, Move, Up
* Capture behavior
* Double-click
* Keyboard navigation
* Editing keys
* IME events
* Enter/Escape/Tab behavior

### 15.6 Layout

* Based on theme.text.normal + padding.

### 15.7 Validation

* Integrate on_change & on_validate.

### 15.8 Focus Integration

* Request focus on click
* Draw focus border

---

# 16. Result

When SPEC-HAL-011 is implemented, Halcyon will have:

* a trading-grade TextField
* precise cursor & selection mechanics
* full IME support
* robust event model
* no corner cases
* no React-style re-renders
* no egui limitations
* no XAML binding hell

This is the **final boss** of standard widgets.
