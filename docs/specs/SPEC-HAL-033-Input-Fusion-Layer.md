# SPEC-HAL-033: Input Fusion Layer

**Author:** JC
**Date:** 2025-11-24
**Status:** Draft
**Depends on:**

* SPEC-HAL-004 – WGPU / Winit Integration
* SPEC-HAL-009 – Event Routing & Hit-Testing
* SPEC-HAL-014 – Advanced Input
* SPEC-HAL-024 – Coordinate Model

---

## 1. Purpose

Define a **single, canonical input layer** that:

* Normalizes all raw platform input (mouse, touchpad, keyboard, future touch)
* Generates **fused, high-level events** (drag, click, double-click, scroll, shortcuts)
* Provides a clean base for **complex keystrokes and combos** (chords, sequences)
* Guarantees ordering, timing, and coordinate consistency for the rest of Halcyon

This layer sits between **platform adapters (winit)** and **EventRouter**.

---

## 2. Goals / Non-Goals

### 2.1 Goals

1. **Canonical event stream**

    * One internal event type for all input (`FusedInputEvent`)
    * EventRouter consumes only fused events, never raw winit events

2. **Unified pointer model**

    * Mouse and touchpad treated as one logical pointer source
    * Compatible with future touch / pen

3. **Gestures generated centrally**

    * Click / double-click / triple-click
    * Press-and-hold, drag, drag-end
    * Wheel scroll as coherent “scroll pulses” (not random deltas spam)

4. **Complex keyboard support**

    * Chords: `Ctrl+Shift+P`, `Alt+Space`, etc.
    * Sequences: `Ctrl+K, Ctrl+S` style combos
    * Multi-step combos with timeouts and conflict rules
    * Same infrastructure used for **global shortcuts** and **widget-scoped shortcuts**

5. **Monotonic timing**

    * All input time based on `Instant`
    * No wall-clock or system time used for *any* decision

6. **Deterministic behavior**

    * Same physical events → same fused stream
    * Clear rules for ambiguity (e.g. double-click vs two single clicks)

### 2.2 Non-Goals

* No gesture recognition beyond basic desktop gestures:

    * No pinch/zoom/rotate yet
* No text input method editor (IME) handling beyond what winit already provides
* No remapping UI or keybinding editor UI in this SPEC (uses this layer later)

---

## 3. Architecture Overview

### 3.1 Current Stack (Simplified)

```text
winit → (ad-hoc mapping) → EventRouter → Widgets
```

Problems:

* Raw winit events leak everywhere
* Each place re-invents “double-click”, “drag”, “shortcut”
* Hard to reason about real input stream

### 3.2 Proposed Stack

```text
winit → PlatformAdapter → InputFusion → EventRouter → Widgets
```

* **PlatformAdapter**: lossless conversion from winit to `RawInputEvent`
* **InputFusion**: stateful engine producing `FusedInputEvent`
* **EventRouter**: routing only fused events into widget tree

---

## 4. Data Model

### 4.1 RawInputEvent

**Location:** `src/core/event.rs`

```rust
/// Lossless, platform-agnostic representation of incoming events.
/// Produced by PlatformAdapter. Consumed ONLY by InputFusion.
pub enum RawInputEvent {
    PointerMoved {
        time: Instant,
        device: PointerDevice,
        position: PointF,         // absolute logical window coords
        delta: Vec2F,             // logical delta since last move (same device)
    },
    PointerButton {
        time: Instant,
        device: PointerDevice,
        button: PointerButton,
        state: ButtonState,       // Pressed / Released
        position: PointF,
    },
    PointerWheel {
        time: Instant,
        device: PointerDevice,
        delta: WheelDelta,        // Line or Pixel delta, normalized to logical
        position: PointF,
    },
    Key {
        time: Instant,
        key_code: KeyCode,        // physical key
        state: KeyState,          // Down / Up
        repeat: bool,
        modifiers: Modifiers,     // ctrl/alt/shift/meta snapshot
    },
    TextInput {
        time: Instant,
        text: String,             // UTF-8 text (after IME)
    },
    FocusChanged {
        time: Instant,
        focused: bool,
    },
    // Extendable: Gamepad, touch, pen, etc.
}
```

Support structs:

```rust
pub enum PointerDevice {
    Mouse,
    Touchpad,
    // Later: Touch, Pen, etc.
}

pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    X1,
    X2,
}

pub enum ButtonState { Pressed, Released }
pub enum KeyState { Down, Up }

pub enum WheelDelta {
    Lines(f32, f32),   // x, y in "scroll lines"
    Pixels(f32, f32),  // x, y in logical pixels
}
```

### 4.2 FusedInputEvent

**Location:** `src/core/event.rs`

```rust
/// Canonical input events used throughout Halcyon.
/// EventRouter routes THESE, not RawInputEvent.
pub enum FusedInputEvent {
    // Pointer
    PointerMove {
        time: Instant,
        pos: PointF,
        delta: Vec2F,
        device: PointerDevice,
        buttons: PointerButtonsState,
    },
    PointerDown {
        time: Instant,
        pos: PointF,
        device: PointerDevice,
        button: PointerButton,
        click_count: u8, // 1 = single, 2 = double, 3 = triple
    },
    PointerUp {
        time: Instant,
        pos: PointF,
        device: PointerDevice,
        button: PointerButton,
        click_count: u8,
    },
    DragStart {
        time: Instant,
        origin: PointF,
        button: PointerButton,
    },
    DragUpdate {
        time: Instant,
        origin: PointF,
        current: PointF,
        delta: Vec2F,
        button: PointerButton,
    },
    DragEnd {
        time: Instant,
        origin: PointF,
        current: PointF,
        button: PointerButton,
        cancelled: bool,
    },
    Scroll {
        time: Instant,
        pos: PointF,
        delta: Vec2F, // logical pixels, fused from wheel events
        source: ScrollSource, // Wheel / Touchpad
    },

    // Keyboard / shortcuts
    KeyDown {
        time: Instant,
        key_code: KeyCode,
        modifiers: Modifiers,
        repeat: bool,
    },
    KeyUp {
        time: Instant,
        key_code: KeyCode,
        modifiers: Modifiers,
    },
    TextInput {
        time: Instant,
        text: String,
    },
    Shortcut {
        time: Instant,
        id: ShortcutId,
        scope: ShortcutScope, // Global / FocusedWidget / WidgetTree
    },

    // Focus, window
    FocusChanged {
        time: Instant,
        focused: bool,
    },

    // Internal synthetic events if needed
    Tick {
        time: Instant,
        dt: Duration,
    },
}
```

Shortcuts:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShortcutId(pub u32);

pub enum ShortcutScope {
    Global,
    FocusedWidget,
    WidgetTree(WidgetId),
}
```

---

## 5. InputFusion Engine

### 5.1 Responsibilities

**InputFusion** takes `RawInputEvent` stream and:

1. Maintains per-device state:

    * Pointer position, pressed buttons, active drag if any
    * Last click time/pos per button
2. Maintains keyboard state:

    * Pressed key set
    * Modifier state
    * Recent key sequence buffer (for combos)
3. Normalizes:

    * Wheel deltas → logical pixel deltas
    * Double-click and triple-click detection
    * Drag lifecycle events
4. Detects shortcuts:

    * Chords (simultaneous modifiers + key)
    * Sequences (multi-step combos)

### 5.2 API

**Location:** `src/core/event.rs` or `src/core/input_fusion.rs`

```rust
pub struct InputFusion {
    pointer_state: PointerFusionState,
    keyboard_state: KeyboardFusionState,
    scroll_state: ScrollFusionState,
    shortcut_state: ShortcutFusionState,
    last_time: Instant,
}

impl InputFusion {
    pub fn new() -> Self { /* ... */ }

    /// Feed raw event(s), get zero or more fused events.
    pub fn process_raw(
        &mut self,
        raw: RawInputEvent,
        out: &mut Vec<FusedInputEvent>,
    ) {
        // match raw { ... }
    }
}
```

The **only** place where raw → fused happens is inside `InputFusion`.

---

## 6. Pointer Fusion

### 6.1 State

```rust
struct PointerFusionState {
    position: PointF,
    last_down: Option<LastClickInfo>,
    buttons: PointerButtonsState,
    active_drag: Option<ActiveDrag>,
}

struct LastClickInfo {
    time: Instant,
    pos: PointF,
    button: PointerButton,
    count: u8,
}

struct ActiveDrag {
    button: PointerButton,
    origin: PointF,
    last_pos: PointF,
}
```

### 6.2 Click / Double-Click Rules

Parameters (configurable later, constants for now):

* `DOUBLE_CLICK_MAX_DELAY`: e.g. 350 ms
* `DOUBLE_CLICK_MAX_DISTANCE`: e.g. 4 logical px

Algorithm:

* On `PointerButton` with `Pressed`:

    * If:

        * same button as last click
        * `time - last.time <= DOUBLE_CLICK_MAX_DELAY`
        * `distance(pos, last.pos) <= DOUBLE_CLICK_MAX_DISTANCE`
    * Then: `click_count = last.count + 1` (clamped to 3)
    * Else: `click_count = 1`
    * Emit `FusedInputEvent::PointerDown { click_count, ... }`
* On `Released`:

    * Emit `PointerUp` with same `click_count` as last press

This gives deterministic click semantics for the whole system.

### 6.3 Drag Lifecycle

Rule:

* Drag is tied to a **button**, not just pointer moves.

Algorithm:

* On `PointerDown`:

    * Record potential drag: `ActiveDrag { button, origin = pos, last_pos = pos }`
* On `PointerMove` while button still pressed:

    * If `active_drag.is_none()` and moved beyond **drag slop** threshold (e.g. 2 px):

        * Emit `DragStart`
        * Set `active_drag`
    * If `active_drag.is_some()`:

        * Emit `DragUpdate { origin, current, delta }`
* On `PointerUp`:

    * If `active_drag` with same button:

        * Emit `DragEnd { cancelled: false }`
        * Clear `active_drag`
    * Else: just PointerUp

EventRouter will later map these to capture semantics; InputFusion only produces the chain.

### 6.4 Scroll Fusion

Normalize wheel behavior:

```rust
struct ScrollFusionState {
    // Possibly accumulate small deltas to whole-line pulses later
}
```

Rules:

* Convert `WheelDelta` to logical pixel `delta` using:

    * Line deltas × `LINE_HEIGHT_ESTIMATE` (from theme/font size)
    * Pixel deltas passed as-is
* Coalescing is optional but recommended:

    * If multiple wheel events arrive within a frame, merge into one `Scroll` event

Output is always:

```rust
FusedInputEvent::Scroll {
    time,
    pos,
    delta,
    source: ScrollSource::MouseWheel | ScrollSource::Touchpad,
}
```

---

## 7. Keyboard & Shortcuts

### 7.1 KeyboardFusionState

```rust
struct KeyboardFusionState {
    pressed_keys: HashSet<KeyCode>,
    modifiers: Modifiers,
}
```

* Updated from `RawInputEvent::Key`
* Ensures consistent view of currently pressed keys

### 7.2 Shortcut Model

We need **chords** and **sequences**.

#### Chord examples

* `Ctrl+Shift+P`
* `Alt+Enter`
* `Ctrl+K Ctrl+S` (first step is chord, then another chord as sequence)

#### Sequence examples

* `Ctrl+K`, then `Ctrl+C` within 800 ms
* `G G` type “go-to-top” combos

### 7.3 ShortcutDescriptor

**Location:** `src/core/event.rs` or separate `shortcut.rs`

```rust
pub struct ShortcutDescriptor {
    pub steps: Vec<ShortcutStep>,
    pub max_delay_between_steps: Duration,
    pub id: ShortcutId,
    pub scope: ShortcutScope,
    pub priority: u8, // tie-break when multiple match
}

pub struct ShortcutStep {
    pub required_keys: Vec<KeyCode>,  // e.g. [KeyCode::K]
    pub required_mods: Modifiers,     // e.g. CTRL|SHIFT
}
```

### 7.4 ShortcutFusionState

```rust
struct ShortcutFusionState {
    active_steps: Vec<ActiveShortcut>,
    registry: Vec<ShortcutDescriptor>,
}

struct ActiveShortcut {
    descriptor_index: usize,
    current_step: usize,
    last_step_time: Instant,
}
```

### 7.5 Shortcut Matching Rules

1. On every **KeyDown**:

    * Update `KeyboardFusionState`
    * Construct current chord:

        * `current_keys = pressed_keys`
        * `current_mods = modifiers`
    * For each `ShortcutDescriptor`:

        * If it can **start** at this chord (step 0 matches):

            * Insert a new `ActiveShortcut` with `current_step = 0`
        * For existing `ActiveShortcut`s:

            * If `current_step`’s `required_keys / required_mods` match current chord:

                * If this is **final step**:

                    * If `time - last_step_time <= max_delay_between_steps`:

                        * Emit `FusedInputEvent::Shortcut { id, scope }`
                        * Remove this active shortcut
                * Else:

                    * Advance `current_step`, update `last_step_time`
            * Else if `time - last_step_time > max_delay_between_steps`:

                * Drop this active shortcut

2. Conflict resolution:

    * If multiple descriptors match at once, pick:

        * Higher `priority` first
        * If tie: more `steps`
        * If tie: deterministic order by `id`

3. KeyDown / KeyUp events are **still delivered** even when shortcuts fire
   (we can later add “consume” semantics per scope if needed).

---

## 8. Time & Ordering Guarantees

* All `RawInputEvent` have `time: Instant` from one monotonic source.
* `InputFusion` enforces:

    * `event.time >= last_time`
    * If platform gives out-of-order timestamps, clamp to `last_time`.
* Output `FusedInputEvent` sequence is **strictly ordered** by time.
* For events with same timestamp, preserve input order from the platform adapter.

This ensures reproducibility and makes future input recording/replay possible.

---

## 9. Integration Points

### 9.1 Platform Adapter (winit)

**Location:** `src/platform/window.rs` or `winit.rs`

* Convert `winit::event::Event` → `RawInputEvent`
* No logic beyond:

    * Coordinate conversion to logical space
    * Mapping of winit’s enums into our enums

### 9.2 App Event Loop

**Location:** `src/core/app.rs` or `main.rs`

* Create one `InputFusion` instance per app:

```rust
let mut input_fusion = InputFusion::new();
let mut fused_events = Vec::new();
```

* On each winit event:

    * Map to `RawInputEvent` via PlatformAdapter
    * Feed into `input_fusion.process_raw(raw, &mut fused_events);`
    * For each `fused` in `fused_events`:

        * Pass into `EventRouter` (existing system from SPEC-HAL-009)
    * Clear `fused_events` for next loop iteration

### 9.3 EventRouter

* No longer sees `RawInputEvent` or winit types
* Only processes `FusedInputEvent`
* Pointer/drag/scroll/shortcut semantics are already computed

---

## 10. Invariants & Safety

1. **No raw platform types outside adapter**

    * `winit::event::Event` never touches widgets or router.

2. **Monotonic input time**

    * `FusedInputEvent.time` strictly non-decreasing.

3. **Stable gestures**

    * Every `DragStart` has a matching `DragEnd` (unless app terminates).
    * Click counts are consistent for corresponding down/up pairs.

4. **Deterministic shortcuts**

    * Given same input series, same `Shortcut` events are emitted.
    * Time & distance thresholds fixed and centralized.

5. **Zero allocations in hot path (future target)**

    * Initial implementation can use `Vec`s, but design MUST allow pooling / fixed buffers later.

6. **INV-RESIZE-SIGNAL-1: Resize events delivered to Screen**

    * Every window resize MUST be delivered to the active Screen as a `FusedInputEvent::Resize`.
    * Layout constraint updates alone are insufficient for stateful widgets that depend on viewport dimensions.
    * Screens are responsible for updating List, ScrollArea, and other viewport-dependent widgets when resize events are received.

---

## 11. Implementation Checklist

For Claude Code later:

1. **Add core types**

    * `RawInputEvent`, `FusedInputEvent`, and all enums/structs in `src/core/event.rs`

2. **Create `InputFusion`**

    * New module `src/core/input_fusion.rs`
    * Holds `PointerFusionState`, `KeyboardFusionState`, `ScrollFusionState`, `ShortcutFusionState`
    * Implements `process_raw()`

3. **Implement pointer fusion**

    * Position tracking
    * Click counts, double-click logic
    * Drag lifecycle + thresholds
    * Scroll normalization

4. **Implement keyboard & shortcut fusion**

    * Pressed key tracking
    * Shortcut registry and matching state machine
    * Emit `FusedInputEvent::Shortcut` when matches fire

5. **Wire platform adapter**

    * Map winit events → `RawInputEvent`
    * Use `InputFusion` to emit `FusedInputEvent`

6. **Update EventRouter**

    * Switch from old event type to `FusedInputEvent`
    * Keep existing routing semantics, just new event structure

7. **Add examples**

    * `examples/input_fusion_debug.rs` to log fused events
    * `examples/shortcut_demo.rs` to show chords and sequences

---

This SPEC gives you a clean, unified input layer and a solid foundation for all the complex key combos and gesture behavior you’ll need later in the trading UI.
