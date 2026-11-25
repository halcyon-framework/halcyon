**SPEC-HAL-009: Halcyon — Event Routing & Hit-Testing**

**Author:** JC
**Date:** 2025-11-16  
**Status:** Draft  
**Scope:** Event model, coordinate spaces, hit-testing, capture/target/bubble routing, focus, and trading-grade pointer handling.  
**Primary Use-Case:** High-precision trading UIs (charts, ladders, blotters, selection tools).

---

## 1. Goals

Event system must:

- Route pointer and keyboard events **deterministically** through the widget tree.
- Support **hit-testing** on nested widgets with proper z-order.
- Support **capture/target/bubble** phases (for complex composite widgets).
- Provide a **focus model** for keyboard input and text fields.
- Support **pointer capture** for drags (e.g. chart panning, ladder interaction).
- Handle **high-frequency events** (mouse move / scroll) without allocations.
- Be compatible with:
  - chart selection / box select,
  - ladder interactions (click-and-drag on price levels),
  - docking / window dragging.

Non-goals (for this SPEC):

- Gesture recognition (pinch, swipe).
- IME/text composition details (TextField SPEC later).
- Undo/redo or command bus.

---

## 2. Coordinate Spaces

We define three coordinate spaces:

1. **Window Space**
   - Origin: top-left of the OS window.
   - Units: logical pixels (after DPI scaling).
   - Source of all pointer events from `winit`.

2. **Root UI Space**
   - Normally same as window space for a single-root app.
   - We can later add global transforms (e.g. zoomed UI) if needed.

3. **Widget Local Space**
   - Each widget has a `Rect` (from layout).
   - Pointer positions are mapped into widget’s local `(0,0)` at its top-left.

Basic convention:

- Hit-testing is done in **root UI space** using widget `Rect`s.
- When dispatching an event to a widget, we pass both:
  - global position (root), and
  - local position (relative to that widget).

---

## 3. Event Types (Refinement)

We extend/refine what exists in `event.rs` to include phases and focus.

### 3.1. High-Level Event

```rust
// src/event.rs (conceptual additions)

#[derive(Debug, Clone)]
pub enum Event {
    Pointer(PointerEvent),
    Key(KeyEvent),
    Focus(FocusEvent),
    Window(WindowEvent),
}

#[derive(Debug, Clone)]
pub enum FocusEvent {
    FocusGained,
    FocusLost,
}

#[derive(Debug, Clone)]
pub enum WindowEvent {
    Resized { width: u32, height: u32 },
    CloseRequested,
}
````

### 3.2. Pointer Event

We add **both** global and local positions to the event that reaches the widget.

```rust
#[derive(Debug, Clone)]
pub struct PointerEvent {
    pub global_pos: (f32, f32),         // root/window space
    pub local_pos: (f32, f32),          // widget-local space
    pub kind: PointerEventKind,
    pub button: Option<PointerButton>,  // for Down/Up; None for Move/Scroll
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, Copy)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}
```

We keep the existing `PointerEventKind` and `PointerButton`.

### 3.3. Key Event

We extend with modifiers and repeat flag (useful for trading hotkeys):

```rust
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key_code: KeyCode,
    pub is_down: bool,
    pub is_repeat: bool,
    pub modifiers: Modifiers,
    pub text: Option<String>, // non-empty for text input chars
}
```

---

## 4. Event Phases: Capture / Target / Bubble

We adopt a three-phase event routing model:

1. **Capture Phase**

    * Event travels from root → target’s parent, top-down.
    * Allows parents to intercept or preprocess events.

2. **Target Phase**

    * Event is delivered to the exact widget under the pointer (or focused widget for keys).

3. **Bubble Phase**

    * Event travels from target’s parent → root, bottom-up.
    * Allows parents to react after the target has processed it.

### 4.1. Widget Hook Shape

We express phases in the widget API as a single handler with a **phase parameter**, to keep the trait minimal.

```rust
// src/widget.rs (conceptual refinement)

use crate::event::{Event, EventPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    Capture,
    Target,
    Bubble,
}

pub trait Widget {
    fn layout(&mut self, cx: &mut LayoutContext, constraints: Constraints) -> Size;

    fn paint(&self, builder: &mut SceneBuilder);

    /// Phase-aware event handling.
    ///
    /// Return true if the event is handled and should be stopped.
    fn handle_event(&mut self, event: &Event, phase: EventPhase) -> bool;
}
```

This lets a composite widget:

* intercept during Capture (e.g. a ScrollArea),
* do core behavior at Target (e.g. button click),
* react during Bubble (e.g. parent panel reacts after child).

---

## 5. Hit-Testing Model

### 5.1. Hit Tree

We maintain a logical tree of widgets:

* Each composite widget knows its children and their `Rect`s in root coordinates (or parent-relative, but we can normalize).

Hit-test is:

1. Start at root widget.

2. Recursively traverse visible children in **reverse z-order** (top-most last child first).

3. For each child:

    * Check if point `(x, y)` is inside its bounds.
    * If inside:

        * Recurse into that child (if it has children).
        * If no deeper hit, that child is the target.

4. If no child hit, parent can be a target (if it wants).

Return value:

```rust
pub struct HitTestResult {
    pub widget_id: WidgetId,
    pub local_pos: (f32, f32),
}
```

### 5.2. Trading-Specific Considerations

* For charts:

    * Chart widget does internal hit-testing for candles/objects (this is **inside** the chart widget, separate from Halcyon core).
* For ladders:

    * Ladder widget receives pointer events with local coordinates and decides which price level was hit.

Halcyon’s hit-test stops at **widget boundaries**; domain specifics are handled inside that widget.

---

## 6. Event Routing Algorithm

### 6.1. For Pointer Events

Given a pointer event from platform (`winit`):

1. Convert physical pixels → logical pixels → `global_pos`.
2. Run **hit_test(global_pos)** on the widget tree → `HitTestResult`.
3. Build **event path**: `Vec<WidgetId>` from root → target.

Then:

#### Capture Phase

* Iterate from root → parent of target:

    * For each widget on path:

        * Compute widget-local position (if used).
        * Build `Event::Pointer` with both global/local.
        * Call `handle_event(event, EventPhase::Capture)`.
        * If handler returns `true`, **stop** (event consumed).

#### Target Phase

* Call `handle_event(event, EventPhase::Target)` on the target widget.
* If returns `true`, **stop**.

#### Bubble Phase

* Iterate from parent of target → root, in reverse:

    * For each widget:

        * Call `handle_event(event, EventPhase::Bubble)`.
        * If returns `true`, **stop**.

### 6.2. For Keyboard Events

Keyboard events go to the **focused widget**:

1. Layout / app maintain a **focused_widget: Option<WidgetId>`.
2. Focused widget receives Key event in `Target` phase directly.
3. Bubble phase can propagate upwards (e.g. global hotkeys).

No hit-testing for keys.

---

## 7. Focus Model

We define a simple focus manager.

### 7.1. FocusManager

Central component (likely owned by the app or top-level UI state):

```rust
pub struct FocusManager {
    focused: Option<WidgetId>,
}

impl FocusManager {
    pub fn focused(&self) -> Option<WidgetId> { self.focused }

    pub fn set_focus(&mut self, new_focus: Option<WidgetId>, tree: &mut WidgetTree) {
        if self.focused == new_focus {
            return;
        }

        // Send FocusLost to old
        if let Some(old_id) = self.focused {
            // locate widget and send Event::Focus(FocusLost)
            // ...
        }

        self.focused = new_focus;

        // Send FocusGained to new
        if let Some(new_id) = new_focus {
            // locate widget and send Event::Focus(FocusGained)
            // ...
        }
    }
}
```

### 7.2. Focus Acquisition Rules

* **TextField** and similar can request focus on click (Target phase).
* **Tab** key cycles focus between focusable widgets in a precomputed order.
* Only one widget has keyboard focus at a time.

Trading use:

* Hotkey binds can be global (Bubble phase at root after focused widget sees them).
* TextFields for order quantity/price still behave correctly.

---

## 8. Pointer Capture (CRITICAL for Trading)

We need pointer capture for:

* dragging chart to pan/zoom,
* drawing selection rectangles,
* dragging ladder levels,
* dragging splitters/panels.

### 8.1. CaptureManager

Global state:

```rust
pub struct PointerCapture {
    pub captured_widget: Option<WidgetId>,
    pub button: Option<PointerButton>,
}

impl PointerCapture {
    pub fn capture(&mut self, widget_id: WidgetId, button: PointerButton) {
        self.captured_widget = Some(widget_id);
        self.button = Some(button);
    }

    pub fn release(&mut self) {
        self.captured_widget = None;
        self.button = None;
    }

    pub fn is_captured_by(&self, widget_id: WidgetId) -> bool {
        self.captured_widget == Some(widget_id)
    }
}
```

### 8.2. Routing With Capture

Rules:

* If `PointerCapture.captured_widget` is **Some(id)**:

    * **All** pointer events (Move/Down/Up/Scroll) go to that widget as **Target**, skipping hit-test.
    * Bubble phase still runs up the tree from that widget.
* Capture is usually acquired on PointerDown (Target phase) and released on PointerUp.

This guarantees:

* No “fallout” when dragging fast.
* Reliable dragging behavior (charts, ladders) even when pointer leaves widget bounds.

---

## 9. Double-Click / Click Classification

Many trading widgets need precise click classification:

* single left click,
* double left click,
* right click,
* click + drag.

We specify:

* Platform layer (`winit`) provides physical events: Down/Up + timestamp.
* Halcyon maintains:

```rust
pub struct ClickTracker {
    last_click_time: Option<std::time::Instant>,
    last_click_pos: Option<(f32, f32)>,
    last_button: Option<PointerButton>,
}
```

Classification rules (configurable thresholds):

* If second `Down` event for same button occurs within Δt (e.g., 250ms) and within Δd pixels (e.g., 4px):

    * classify as `DoubleClick` event (via a higher-level synthetic event or flag in `PointerEventKind`).

We can:

* extend `PointerEventKind` with `DoubleClick`, or
* keep a separate higher level event inside widgets.

SPEC stance: **extend PointerEventKind** for clarity:

```rust
pub enum PointerEventKind {
    Down,
    Up,
    Move,
    Scroll { delta_x: f32, delta_y: f32 },
    DoubleClick,
}
```

Trading widgets (like charts) can then react to `DoubleClick` cleanly.

---

## 10. Integration With Halcyon Structures

### 10.1. WidgetTree Representation

We need some representation of the widget tree with IDs and parent/children relationships, even if not yet fully coded.

Conceptually:

```rust
pub struct WidgetNode {
    pub id: WidgetId,
    pub widget: Box<dyn Widget>,
    pub children: Vec<WidgetNodeId>,
    pub rect: Rect,
    // maybe z-order, focusable flags, etc.
}

pub struct WidgetTree {
    pub root: WidgetNodeId,
    // fast maps from WidgetId → node, etc.
}
```

Hit-testing and routing operate on this `WidgetTree`.

### 10.2. App Integration

In `App::handle_event` or in `AppRunner`:

* Platform event → Halcyon `Event`.

* If pointer:

    * use `PointerCapture` + `WidgetTree` to determine Target path.
    * dispatch through phases.

* If key:

    * use `FocusManager` to find target.
    * dispatch `Target` then `Bubble`.

---

## 11. Trading-Specific Interaction Scenarios

We explicitly design for these:

### 11.1. Chart Selection

* Chart widget:

    * Uses pointer capture for box selection.
    * On `PointerDown`:

        * Acquire capture.
        * Remember origin.
    * On `PointerMove`:

        * Update box rect.
    * On `PointerUp`:

        * Release capture.
        * Compute selection / call into backend.

Halcyon only needs to:

* route events,
* provide capture,
* ensure HitTest target is stable.

### 11.2. Ladder / Order Book

* Ladder widget uses local coords to map pointer to price level.
* Might want “click-drag to sweep levels”:

    * Captured pointer.
    * Track Move events.
    * For each Move, recompute price level and update highlight/preview.

Again: event system only needs to deliver high-frequency Move+Down/Up with capture.

### 11.3. Docking / Floating Panels

* Titlebar widget of panel:

    * On `PointerDown`, acquires capture.
    * On `Move`, updates panel’s position.
    * On `Up`, releases capture.

---

## 12. Performance Considerations

* Hit-testing is **O(depth)** for typical UI trees.
* For large trees:

    * Shortcut: maintain per-container bounding boxes to quickly discard subtrees.
* No allocations in the inner dispatch loop:

    * Reuse preallocated `Vec<WidgetId>` for the event path.
* Pointer events can be very frequent:

    * Avoid cloning heavy event objects; keep them light.
    * Avoid per-event heap allocations.

---

## 13. Implementation Checklist

When you (or CC) start implementing SPEC-HAL-009, the concrete tasks are:

1. **Extend `event.rs`:**

    * Add `FocusEvent`, `WindowEvent`, `EventPhase`, `Modifiers`.
    * Extend `PointerEvent` and `KeyEvent` with modifiers, etc.
    * Optionally extend `PointerEventKind` with `DoubleClick`.

2. **Add `EventPhase` to `Widget::handle_event`:**

    * Update trait and all existing stub widgets accordingly.

3. **Introduce `FocusManager` and `PointerCapture` structs:**

    * Owned by a top-level UI state or `AppRunner`.

4. **Define `WidgetTree` abstraction:**

    * Enough to get root, parents, and children (IDs + Rects).

5. **Implement hit-testing:**

    * Function: `fn hit_test(tree: &WidgetTree, x: f32, y: f32) -> Option<HitTestResult>`.

6. **Implement event routing in runner/platform layer:**

    * From `winit` event to `Event`.
    * For pointer: capture or hit-test → event path → capture/target/bubble.
    * For key: use `FocusManager`.

7. **Implement pointer capture logic:**

    * API on widgets: they can request capture via some callback or shared handle.

8. **Add minimal tests:**

    * Hit-testing with nested containers.
    * Capture overriding hit-testing.
    * Focus switching generates FocusLost/FocusGained.
    * DoubleClick classification.

Once this is in place, Halcyon can host **serious trading widgets** with reliable interaction semantics, and the charting/selection logic you already have conceptually can be ported into a Halcyon widget cleanly.

---

See also: SPEC-HAL-110

```
::contentReference[oaicite:0]{index=0}
```
