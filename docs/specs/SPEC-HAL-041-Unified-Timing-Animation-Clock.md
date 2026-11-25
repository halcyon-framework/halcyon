# **SPEC-HAL-041 — Unified Timing / Animation Clock**

**Author:** JC
**Date:** 2025-11-24
**Status:** Draft
**Scope:** Timing, Input, Animation, Frame Pacing
**Purpose:** Define a **single, global, deterministic, monotonic time source** used for *all* time-dependent systems in Halcyon: animations, throttling, input coalescing, frame pacing, double-click logic, fling/scroll inertia, chart interactions, and interpolation. Remove all ad-hoc timestamps. Prevent drift, jitter, stutter, and timing desyncs across systems.
**Controlling authority:** The single-time-source principle remains design
evidence. Binding generic Timeline playback/sampling and literal frame order are
owned by SPEC-HAL-021 and SPEC-HAL-042; their split advance/routing/Update order
supersedes every “global Animation Tick” shorthand below.

---

# **1. Problem Statement**

Right now Halcyon has no unified timing model.
Different subsystems implicitly rely on:

* `Instant::now()` in many places
* Winit timestamps for pointer events
* Internal counters for animations
* Manual deltas for scroll inertia
* Ad-hoc logic for double-click detection
* Renderer frame pacing logic separate from input timing
* No global "frame time" identity

This creates **desynchronization**, such as:

* cursor jitter when UI lags,
* animations running too fast/slow during resize stalls,
* scroll inertia stuttering due to mismatched timestamps,
* inconsistent drag interaction timing,
* "double-click" firing inconsistently at high FPS,
* chart zoom behavior becoming unstable at high DPI.

Trading UI absolutely requires **stable, unified time**.

---

# **2. Core Design Goal**

Halcyon must use:

```
ONE CLOCK
ONE TIMESTAMP PER FRAME
ONE DELTA PER FRAME
```

Everything else — all animations, all inertial scrolls, all double-click logic, all throttling, all fusion layers — derive from these values.

No subsystem calls `Instant::now()` on its own.

---

# **3. The Unified Clock Model**

The clock consists of **three values** updated every frame:

```
struct FrameTime {
    /// Monotonic timestamp for this frame
    now: Instant,

    /// Time delta between the last two frames
    dt: Duration,

    /// Frame index (monotonically increasing)
    frame_index: u64,
}
```

To obtain float seconds for animation/easing math:

```
dt_secs = frame_time.dt.as_secs_f32()
```

**Note:** There is no `dt_secs` field in `FrameTime`. Systems that need float seconds must call `.as_secs_f32()` on the `Duration`.

Only **one component** in the whole engine computes this.

That component is:

```
EngineTicker
```

The `EngineTicker` produces a fresh `FrameTime` once per frame *before* layout/paint.

---

# **4. Precise Update Sequence**

Every rendering cycle:

```
1. Winit event loop wakes the app
2. EngineTicker captures the one frame timestamp/delta/index (begin_frame)
3. Engine-owned pre-routing visual state advances
4. Input coalescing/fusion consumes the same frame time
5. Generic Timeline advances existing handles once
6. Widgets handle events and may start/cancel handles at that frame time
7. Widget update samples Timeline read-only and publishes owner state
8. Layout
9. Authorized post-layout projection and custom preparation
10. Paint
11. Render
```

Every part of the pipeline receives the **same `now` and `dt`**.

---

# **5. Death to `Instant::now()`**

The following uses are banned:

* In pointer handlers (`PointerMoveEvent`)
* In click detection
* In gesture recognition (drag, flick)
* In animations (scroll inertia, opacity, transitions)
* In chart interactions (zoom momentum)
* In diffing (change detection)
* In layout (timeouts, debouncing)

These become:

```
event.timestamp = frame_time.now
```

or

```
let dt_secs = frame_time.dt.as_secs_f32();
```

No more direct timestamp calls.

---

# **6. Clock Responsibilities**

## **6.1 Core timing for the entire UI**

* Animation progression
* Scroll momentum
* Fling physics
* Cursor blink
* Focus caret animation
* Tooltip delay
* Long-press detection
* Double-click & triple-click windows
* Continuous pointer repeat

## **6.2 Input Coalescing (SPEC-HAL-036)**

Uses `dt` to merge high-frequency events.

## **6.3 Frame Pacing (SPEC-HAL-030)**

Ensures stable frame rhythm.

## **6.4 Animation Scheduler (Action System)**

Simple, deterministic, no jitter.

---

# **7. Clock Types (Soft vs Hard)**

We define two timing domains:

### **Soft Clock (UI Clock)**

Driven by actual frame dt, used for all user-visible animations.

Behaves like:

* Faster FPS → smoother
* Lower FPS → dt increases → animation catches up smoothly

### **Hard Clock (Physical Time Source)**

Monotonic, but used only for:

* double-click windows
* drag-hold thresholds
* timeout filters
* input gating

Everything else stays on *Soft Clock*.

This prevents visibly inconsistent animation speeds when FPS fluctuates.

---

# **8. Consumption Rules**

All subsystems obtain timing exclusively through:

```
frame_time.now           // Instant - current frame timestamp
frame_time.dt            // Duration - time since last frame
frame_time.dt.as_secs_f32()  // f32 - float seconds for delta-driven subsystem math
```

**No widget, renderer, or subsystem is allowed to call `Instant::now()` for frame-time-relative calculations.**

The frame loop guarantees a single `dt` per frame. Delta-based subsystems may use:

```
dt_secs = frame_time.dt.as_secs_f32()
```

Generic SPEC-HAL-021 Timeline playback derives phase from absolute captured
`frame_time.now - start_time`; it never accumulates `dt`.

---

# **9. Widget Animation Prohibition**

**Widgets must not advance time-based state inside event handlers.**

The engine advances existing Timeline/engine-owned visual state before routing.
Event handlers may set targets and start/cancel handles at the captured frame
time. Widget update then samples generic Timeline state read-only and publishes
the widget-owned visual value before layout.

Layout, custom preparation, paint, and renderer phases do not advance Timeline or
derive new widget/application state.

This ensures:

* Consistent animation timing across all widgets
* No per-widget `Instant::now()` calls
* Deterministic frame behavior
* Centralized animation control

---

# **10. EngineTicker Implementation**

```
pub struct EngineTicker {
    last_frame_time: Instant,
    frame_index: u64,
}
```

### On each frame start:

```
let now = Instant::now();
let dt = now - self.last_frame_time;
self.last_frame_time = now;
self.frame_index += 1;

FrameTime { now, dt, frame_index }
```

### During entire frame:

* `frame_time` is immutable.
* No recalculation.

### Guarantees:

* Consistent values across all subsystems
* No drift
* No jitter from multi-sample timestamp calls
* One source of truth

---

# **11. Event Timestamp Model**

Winit timestamps are ignored except as fallback.

Halcyon overrides them:

```
PointerEvent.timestamp = frame_time.now
```

Same for keyboard, wheel, gesture events.

---

# **12. Required Invariants**

### Invariant 1 — Clock Updated Before Layout

Before each frame's layout run, EngineTicker must be updated exactly once.

### Invariant 2 — No Other Timing Sources Allowed

Any attempt to read Instant directly is considered a bug.

### Invariant 3 — Duration Conversions Go Through FrameTime

Never compute your own dt.

### Invariant 4 — Time Consumers Use Only Captured Frame Time

Delta-based subsystems use captured `dt`. Generic Timeline playback uses captured
absolute `now` and never accumulated delta. No private timestamp or integer
duration arithmetic enters easing math.

### Invariant 5 — Physics-Motion Code Uses Soft Clock Only

Scroll inertia must be stable across frame spikes.

### Invariant 6 — No Time Advancement in Event Handlers or Later Phases

Event handlers may choose targets and start/cancel handles. Timeline advance is
engine-owned; sampled widget-state mutation is Update-owned; layout, custom
preparation, paint, and rendering consume the result read-only.

---

# **13. Implementation Checklist**

### Core

* [x] Add `EngineTicker` in `core/app.rs`
* [x] Add `FrameTime` struct
* [x] Add `EngineTicker::begin_frame()` call at start of every frame
* [x] Expose `frame_time` to all subsystems

### Input

* [ ] Update all `Event` structs to store internal timestamps
* [ ] Replace all timestamp logic with `frame_time.now`
* [ ] Integrate with Input Coalescing (SPEC-HAL-036)

### Animation

* [ ] Implement the binding SPEC-HAL-021 pre-routing Timeline advance and
  same-frame Update sampling split
* [ ] Ensure predictable easing from absolute captured engine time

### Renderer

* [ ] Synchronize frame pacing with `frame_time.dt`

### Widgets

* [ ] TextField caret blink uses `now`
* [ ] ScrollArea time progression uses the engine-owned pre-routing scroll phase
* [ ] Tooltip delays use `now`
* [ ] Button long-press logic uses `now`

---

# **14. End**

This SPEC unifies **the entire timing system** under one deterministic, monotonic clock — eliminating jitter, stutter, desync, and timestamp fragmentation.
It is the **final pillar** that makes Halcyon's event/input/animation model coherent and production-grade.
