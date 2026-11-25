# SPEC-HAL-054: TextField Architecture & Editable Paragraph Node

Unified Editable Text Widget, Layout/Render Integration, Editing Pipeline, IME + Cursor + Selection

Status: Draft
Author: JC
Depends on:

* **SPEC-HAL-048** (Paragraph & Text Layout Engine)
* **SPEC-HAL-049** (Font Collections & Fallback Rules)
* **SPEC-HAL-050** (HarfBuzz Shaping Integration)
* **SPEC-HAL-052** (Text Editing Model: Cursor & Selection)
* **SPEC-HAL-053** (IME Composition Model)
* **SPEC-HAL-033/036** (Input Fusion & Coalescing)
* **SPEC-HAL-009** (Event Routing & Hit-testing)
* **SPEC-HAL-001/007** (State Model & Widget System)

Planned follow-ups:

* SPEC-HAL-055 (Undo/Redo Model)
* SPEC-HAL-056 (Clipboard Integration)
* SPEC-HAL-057 (Password Field + Secure Entry Mode)

---

## 1. Summary

This SPEC defines the **TextField widget architecture** for Halcyon:

* Canonical editable text widget
* Unified paragraph-based layout
* EditingState integration (cursor, selection, composition)
* Hit-testing, selection drags, shift-navigation
* Scrollable text viewport (single or multi-line)
* Focus management
* Rendering pipeline
* Event routing pipeline
* Integration with NativeFontBackend + ParagraphEngine + IME layer

With this SPEC, Halcyon gains a fully optimized, battle-ready TextField built on the entire typography stack.

---

## 2. Motivation

Halcyon’s current text functionality:

* Only supports static single-line rendering
* No editable widget
* No cursor rendering
* No selection
* No IME
* No multi-line
* No scrollable viewport
* No event routing patterns for text widgets

This SPEC defines the authoritative, production-grade implementation for `TextField`, used for:

* Input boxes
* Multi-line editors
* Search bars
* Chat boxes
* Code editors (later SPEC)
* Log viewers
* Terminal emulators (future)

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: Support single-line and multi-line modes.
* R2: Full editing model:

    * insert
    * delete
    * backspace
    * selection
    * movement
    * word-jump (optional follow-up)
* R3: IME support (SPEC-053).
* R4: Scrolling support for:

    * horizontal (single-line)
    * vertical (multi-line)
* R5: Cursor and selection visualization.
* R6: Hit test into paragraph for mouse placement.
* R7: Drag selection.
* R8: Double-click selection of clusters or words (Configurable in 055+056).
* R9: TextField integrates with Halcyon focus model.

### 3.2 Editing invariants

* I1: EditingState is single source of truth.
* I2: No direct text mutation; mutations go through EditingState methods.
* I3: Cursor/selection always aligned to cluster boundaries (SPEC-052).
* I4: Composition text shown inline when IME active (SPEC-053).

### 3.3 Layout invariants

* L1: TextField participates in layout like any other widget.
* L2: ParagraphEngine handles measurement based on constraints.
* L3: Scroll offset clamps to content size.
* L4: Line height determined by ParagraphEngine.

### 3.4 Rendering invariants

* R1: Text quads produced by ParagraphEngine used for rendering.
* R2: Cursor drawn in logical coordinates.
* R3: Selection rendered behind glyphs.
* R4: Composition decorations rendered on top of glyphs.

---

## 4. Top-Level Architecture

TextField consists of:

### 4.1 State

```
pub struct TextFieldState {
    pub editing: EditingState,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub single_line: bool,
    pub wrap: bool,
    pub focused: bool,
 }
```

### 4.2 Widget

```
pub struct TextField {
    pub style: TextStyle,
    pub padding: EdgeInsets,
}
```

### 4.3 Core pipeline

```
InputFusion → Event Routing → TextField::on_event
→ EditingState mutation → ParagraphEngine layout
→ Scene build → Renderer draws text + cursor + selection
```

### 4.4 Paragraph integration

TextField builds a **Paragraph** from `editing.text`, merged with composition text:

```
if composition exists:
    underlying text before cursor
    composition.text with special span
    underlying text after cursor
else:
    underlying text only
```

ParagraphEngine returns:

* Vec<Line>
* Vec<Run>
* Vec<ShapedGlyph>
* cluster mapping
* line metrics

This output feeds:

* hit-testing
* cursor movement
* selection rendering
* scroll calculations

---

## 5. Event Routing for TextField

### 5.1 Focus events

* On FocusIn:

    * state.focused = true
    * request IME activation (platform-specific)
* On FocusOut:

    * state.focused = false
    * cancel composition
    * clear selection

### 5.2 Keyboard events

TextField receives:

* character input
* navigation keys
* deletion keys
* IME events

Pipeline:

```
TextField::on_key_event(KeyEvent) → EditingState::apply_key
```

### 5.3 Mouse events

* MouseDown:

    * hit-test → cluster index
    * set cursor
    * clear selection
    * record drag_start
* MouseMove (button down):

    * extend selection
* MouseUp:

    * finalize selection

---

## 6. EditingState Operations

TextField delegates editing to EditingState (SPEC-052):

### 6.1 insertion

```
editing.insert(text)
```

### 6.2 deletion

```
editing.backspace()
editing.delete()
```

### 6.3 movement

Directional navigation:

```
editing.move_left()
editing.move_right()
editing.move_up(lines)
editing.move_down(lines)
```

### 6.4 selection

```
editing.extend_left(shift)
editing.extend_right(shift)
editing.extend_up(shift)
editing.extend_down(shift)
```

### 6.5 composition

```
editing.begin_composition()
editing.update_composition(text, caret_pos)
editing.commit_composition(text)
editing.cancel_composition()
```

---

## 7. TextField Rendering Specification

### 7.1 Background and borders

TextField may draw:

* background
* border
* focus ring (optional per style)

### 7.2 Selection highlight

Rendered as a set of quads:

```
RectF for each line between cluster positions
```

Color configurable.

### 7.3 Text glyphs

Renderer receives:

* shaped glyphs with physical coordinates
* per-run style information
* selection and composition highlighting

### 7.4 Composition decoration

For composition spans:

* Underline
* Highlight
* Dot-underline
* Possibly color override

Drawn after glyphs but before caret.

### 7.5 Caret rendering

Caret is a vertical bar at cursor.x for current line:

```
RectF {
    x = cursor_x
    y = line_top
    width = caret_thickness
    height = line_height
}
```

Blinking handled at widget/renderer level using EngineTicker (SPEC-042).

---

## 8. Scroll Model

Text may exceed TextField bounds.

### 8.1 Horizontal scroll (single-line)

If `single_line == true`:

```
if cursor_x > viewport_right
    scroll_x = cursor_x - viewport_width + padding.right

if cursor_x < viewport_left
    scroll_x = cursor_x - padding.left
```

### 8.2 Vertical scroll (multi-line)

```
if cursor_y > viewport_bottom
    scroll_y = cursor_y - viewport_height + line_height

if cursor_y < viewport_top
    scroll_y = cursor_y - line_height
```

### 8.3 Scroll clamping

```
scroll_x = clamp(scroll_x, 0, max_scroll_x)
scroll_y = clamp(scroll_y, 0, max_scroll_y)
```

### 8.4 Mouse wheel scrolling

TextField supports vertical wheel for multi-line; horizontal disabled by default.

---

## 9. Focus Management

TextField must:

* Request focus on click
* Release focus on external click
* Emit `FocusChanged` state events

Focus drives:

* caret blinking
* IME activation
* key event routing priority
* selection behavior

---

## 10. Integration With Layout Engine

### 10.1 Measurement

TextField measures via ParagraphEngine:

```
TextField::measure(constraints):
    limit width → constraints.max_width
    generate paragraph
    run ParagraphEngine.measure()
    height = sum line heights + padding
    return Size(width_used, height)
```

### 10.2 Arrangement

```
TextField::arrange(rect):
    content_rect = rect - padding
    store content_rect
```

### 10.3 Text overflow

ParagraphEngine produces content height > widget height → vertical scroll.

---

## 11. Integration With Renderer

Renderer receives:

* scene nodes:

    * background quad
    * selection quads
    * composition decorations
    * glyph quads
    * caret quad

All position calculations use logical coords.

---

## 12. Observable Widget API

TextField provides:

* `set_text(String)`
* `get_text() -> &str`
* `set_cursor(usize)`
* `set_selection(Range)`
* `clear()`
* `select_all()`
* `append_text(&str)`
* `scroll_to_cursor()`

Two modes:

```
TextField::single_line()
TextField::multi_line().wrapping(true/false)
```

---

## 13. Migration Plan

### Phase 0 — Foundation

Implement EditingState, CompositionState, cluster maps, ParagraphEngine integration.

### Phase 1 — Basic single-line TextField

Cursor, typing, backspace, left/right navigation.

### Phase 2 — Multi-line TextField

Up/down nav, scroll model, hit-testing, selection drags.

### Phase 3 — IME integration

Enable pre-edit text, composition decorations, commit/cancel.

### Phase 4 — Rendering polish

Caret blinking, selection rectangles, wrapping support.

### Phase 5 — Stabilization & API exposure

Public TextField widget ready for production.

---

## 14. Testing Strategy

* Cursor movement across multi-line paragraphs.
* Selection drag across lines.
* IME composition sequences (JP/CN/KR).
* RTL lines inside multi-line paragraphs.
* Scrolling behavior with long content.
* Composition styling and caret placement.
* Hit-testing accuracy at varying DPI.

---

## 15. Performance Notes

* Shaping per edit is unavoidable; use shaping cache.
* ParagraphEngine is optimized; small edits produce localized changes.
* Rendering is quad-heavy but GPU efficient.
* IME updates are small; fast re-layout needed.

---

## 16. Security Considerations

* UTF-8 validated on inserts.
* Composition only affects temporary state.
* Scroll values clamped; avoid overflow.
* Large text handled gracefully (avoid unbounded allocations).
