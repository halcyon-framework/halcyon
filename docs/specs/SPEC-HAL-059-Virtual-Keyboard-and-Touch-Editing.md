# SPEC-HAL-059: Virtual Keyboard & Touch Text Editing Model

Mobile/Tablet Input, Touch Selection Handles, Auto-Scroll, Soft Keyboard Control, Long-Press Actions

Status: Draft
Author: JC
Depends on:

* **SPEC-HAL-052** (Editing Model)
* **SPEC-HAL-053** (IME Composition)
* **SPEC-HAL-054** (TextField Architecture)
* **SPEC-HAL-055** (Undo/Redo)
* **SPEC-HAL-056** (Clipboard)
* **SPEC-HAL-057** (SecureEntry Mode)
* **SPEC-HAL-058** (Word Navigation & Commands)
* Platform integration layer (Halcyon Mobile Port — future module)

Planned follow-ups:

* SPEC-HAL-060 (RichTextField / Inline Formatting)
* SPEC-HAL-061 (Halcyon Text Editing Tooling / Inspector)

---

## 1. Summary

This SPEC defines Halcyon’s **touch-based text editing model** and virtual keyboard interaction mechanics for mobile/tablet platforms:

* Soft keyboard invocation & dismissal
* Touch caret placement
* Drag handles for selection
* Long-press → word selection
* Long-press → context menu (copy/paste/select-all)
* Auto-scroll during drag-selection
* IME integration on mobile (iOS/Android)
* Keyboard overlap handling (viewport shifts to keep caret visible)
* “Tap to focus” for TextField
* Multi-line editing with finger gestures

This completes Halcyon’s editing model for mobile-first applications.

---

## 2. Motivation

Touch interaction differs fundamentally from mouse/keyboard:

* No precise pointer; tap hit-testing needed
* Selection handles required for adjusting cursor positions
* Long-press triggers word selection instead of double-click
* Virtual keyboard may cover half the screen
* Cursor must always remain visible, auto-scrolling
* IME composition is always active on mobile

This SPEC standardizes the expected UX and integrates with the existing editing stack.

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: Tap → position cursor
* R2: Long-press → select word
* R3: Drag handles → adjust selection boundaries
* R4: Soft keyboard opens when a TextField receives focus
* R5: Soft keyboard closes when focus lost
* R6: Caret auto-scroll when obscured by keyboard
* R7: Drag-selection edge scrolling
* R8: Copy/Cut/Paste context menu via long-press
* R9: IME composition supported normally
* R10: SecureEntry mode honored (no reveal)

### 3.2 Structural invariants

* I1: Cursor/selection always align to cluster boundaries
* I2: Long-press selection uses word boundary rules (SPEC-058)
* I3: Selection handles map to cluster boundaries
* I4: Keyboard overlay offset applied after layout
* I5: Multi-line TextField scrolls vertically to keep caret visible
* I6: No direct OS text widgets—Halcyon fully owns the editing surface

### 3.3 Touch invariants

* T1: Every touch event has:

    * phase (Down/Move/Up/Cancel)
    * position (x,y)
    * time
* T2: Multiple fingers allowed—but editing uses only single active finger
* T3: Touch movement must be interpreted as selection drag only if initiated on handle or inside existing selection

---

## 4. Touch Editing Pipeline

### 4.1 Tap to focus & place caret

1. On touch-down inside TextField:

    * Request focus
    * Hit-test → cluster index
    * Move cursor to new position
    * Clear selection

2. On touch-up:

    * If no drag occurred → place caret

### 4.2 Long-Press → select word

Long-press threshold: ~400–500 ms.

Pipeline:

1. Touch-down

2. After threshold without movement:

    * hit-test cluster
    * select word at cluster (SPEC-058)
    * show selection handles
    * show context menu (OS-provided or Halcyon’s)

3. IME composition must be cancelled before selecting word.

### 4.3 Long-Press inside selection → context menu

If long-press occurs inside the currently selected range:

* Do not change selection
* Show context menu:

    * Copy (unless SecureEntry)
    * Cut (unless SecureEntry)
    * Paste
    * Select All

### 4.4 Drag-selection (finger drag)

If user touches inside selection background:

* Start selection drag mode
* Move changes the **head** of selection
* Auto-scroll if dragging near top or bottom of TextField view

---

## 5. Selection Handles

### 5.1 Two handles

* Left handle = anchor boundary
* Right handle = head boundary

Both are draggable:

```
struct SelectionHandle {
    position: (x,y),
    cluster_index: usize,
    active: bool,
}
```

### 5.2 Drag behavior

Dragging handle:

* hit-test pointer → cluster index
* update selection.anchor or selection.head
* auto-scroll near edges
* cursor placed at selection.head

### 5.3 Handle layout

Handles rendered as:

* Small circle or vertical pill
* Offset slightly below line baseline
* Scaled with DPI

Handles appear only when selection exists.

---

## 6. Virtual Keyboard Lifecycle

### 6.1 Opening keyboard

Triggered when TextField receives focus.

Halcyon must:

* Inform platform layer: “soft keyboard required”
* Platform reports keyboard height
* Halcyon adjusts layout of viewport accordingly

### 6.2 Closing keyboard

Keyboard dismissed on:

* FocusOut
* User tapping outside editable control
* Platform “hide keyboard” action

### 6.3 Keyboard Overlap Handling

When keyboard appears:

1. Halcyon gets keyboard height (px)
2. TextField’s visible area is shrunk by this height
3. Auto-scroll ensures caret is visible

---

## 7. Viewport Auto-Scroll

### 7.1 Scroll on caret move

If caret.y is below visible bottom (adjusted for keyboard):

```
scroll_y = caret.y - visible_height + line_height
```

If caret.y is above visible top:

```
scroll_y = caret.y - padding
```

### 7.2 Scroll on drag-selection

When dragging near top/bottom:

* Gradually increase scroll speed
* Clamp scroll values

### 7.3 Scroll on paste

After paste, caret moved to end of pasted text → scroll to visible.

---

## 8. IME on Mobile

Mobile always uses IME for typing.

### 8.1 IME composition pipeline

Same as SPEC-053:

* pre-edit visible (masked if secure mode)
* caret inside composition
* commit inserts real text

### 8.2 IME candidate bar

Usually drawn by OS above keyboard.
Halcyon does not draw it.

### 8.3 Cursor → IME sync

Platform must receive caret rect in screen coordinates:

```
TextField::composition_caret_rect()
```

Used for candidate windows.

---

## 9. Clipboard Interaction

Same as SPEC-056, but available via:

* Long-press context menu
* OS-provided action sheets

### SecureEntry mode:

* Copy = disabled
* Cut = disabled
* Paste = enabled
* Context menu adapts automatically

---

## 10. Word Selection in Touch Mode

Instead of double-click, touch provides:

* Long-press → select word
* Drag handles → extend

Word boundaries are identical to SPEC-058.

---

## 11. Integration With EditingState

EditingState remains the source of truth.

Touch events call:

* `move_cursor`
* `select_word_at`
* `select_line_at`
* `extend_selection_to`
* `insert` (via IME commit)
* `delete` (via IME/soft-keyboard backspace)

No forked behavior.

---

## 12. Rendering Rules for Touch UX

* Selection highlight thicker on mobile
* Handles drawn on GPU layer
* Touch hit-test area larger than visible handle
* Caret blink slower (500 ms ON / 500 ms OFF)
* Optional magnifier/zoom-lens on long-press-hold (future)

---

## 13. Platform Integration Layer

Halcyon needs a platform abstraction:

```
trait VirtualKeyboardHost {
    fn show_keyboard(&self);
    fn hide_keyboard(&self);
    fn set_keyboard_avoid_area(&self, height: f32);
    fn ime_set_cursor_rect(&self, rect: RectF);
}
```

Implementations:

* Android: InputMethodManager
* iOS/iPadOS: UIResponder, UIInputView
* Desktop: no-op

---

## 14. Testing Strategy

* Tap-to-focus on all letters
* Touch selection drag + auto-scroll
* Long-press select word
* Long-press open context menu
* IME commit during selection
* Keyboard open/close transitions
* SecureEntry masking with touch
* Android vs iOS behavior consistency
* Scroll jitter edge-case tests

---

## 15. Security Considerations

* SecureEntry forbids copy/cut
* Underlying text never exposed to GPU
* Magnifier (future) must mask underlying characters
* Keyboard-drawn suggestions do not reveal secure text
