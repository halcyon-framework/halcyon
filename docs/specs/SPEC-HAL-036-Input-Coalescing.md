# **SPEC-HAL-036 — High-Frequency Input Coalescing (Pointer & Scroll Events)**

**Author:** JC
**Date:** 2025-11-24
**Status:** Draft
**Category:** Performance / Event System
**Target:** Event Router, Pointer Engine, Scroll Systems, Chart Interactors

---

# **1. Purpose**

Modern mice generate **1000–8000 Hz** pointer deltas.
Touchpads generate **continuous scroll streams**.
Winit forwards all of it.

Halcyon cannot feed every single event into:

* layout,
* paint,
* hit-testing,
* widget logic,

…because it produces stutter, layout spam, and CPU spikes.

This SPEC introduces **mandatory coalescing** for high-frequency input:
pointer movement and scroll wheel events.

Goal:

> **Reduce 1000–8000 raw events → 60–240 coalesced events per second**
> while preserving *precision*, *intention*, and *latency*.

---

# **2. Design Overview**

Coalescing must happen **before** event routing.

Pipeline:

```
Raw Winit Events
    ↓
Input Fusion Layer (new)
    ↓
Coalescer (per-event-stream)
    ↓
Event Router (existing HAL-009)
    ↓
Widgets
```

Two independent coalescers:

1. **Pointer Coalescer**
2. **Scroll Coalescer**

Keyboard is untouched.

---

# **3. Pointer Coalescing**

## 3.1. Principle

Only the **latest** motion event in a frame matters.

If 120 motion events arrive before next redraw:

* keep only the *last* position
* keep motion delta totals
* preserve velocity for kinetic interactions

## 3.2. State

```
struct PointerCoalescer {
    pending: bool,
    last_position: Point,
    delta_accum: Vec2,
    last_timestamp: Instant,
}
```

## 3.3. Emit Rule

On each OS event: store/update state, but do not dispatch.
On `RedrawRequested`:

```
if pending:
    emit CoalescedPointerMove {
        pos: last_position,
        delta: delta_accum,
        timestamp: last_timestamp
    }
```

Then reset state.

Latency: 0–1 frame (4–16ms).
Massive CPU savings.

---

# **4. Scroll Coalescing**

Scroll is different:
scroll deltas are **additive**, not absolute.

We coalesce deltas:

```
smooth_scroll_accum += delta
```

Emit only once per frame.

Result:
smooth kinetic scrolling feels identical, but we eliminate stutter.

---

# **5. Event Types**

New synthetic events:

```
Event::PointerMoveCoalesced { pos, delta, time }
Event::ScrollCoalesced { delta, time }
```

Raw Winit motion/scroll events are **not** delivered to widgets.

PointerDown, PointerUp, Click, WheelButtonDown remain raw.

---

# **6. Integration Into Existing HAL-009 Event Routing**

Input Fusion Layer sits above router.

`event_router.rs` changes:

* Instead of directly routing Winit pointer/scroll events,
  route **only** coalesced events.

Hit-testing, capture, bubble phases remain exactly the same.

No widget changes.

---

# **7. Frame Boundary & Timing**

Coalesced event emission is tied to:

* `WindowEvent::RedrawRequested`
* OR Halcyon’s frame ticker (when animations exist under HAL-115)

This guarantees consistent behavior even if OS does not issue redraws.

---

# **8. Edge Cases**

### PointerDown must flush coalescer

If pointer is moving rapidly and user clicks:

1. Flush pending motion event immediately
2. Deliver PointerDown
3. Continue coalescing normally

### Drag operations

Coalescing works perfectly:

* only latest drag position matters
* smoother than raw events

### Charts (extremely sensitive)

We allow optional “low-latency mode”:

```
coalesce = false
```

For future high-frequency trading charts.

---

# **9. Performance Impact**

Measured assumptions:

| Raw Input        | Coalesced       | CPU Savings     |
| ---------------- | --------------- | --------------- |
| 1200 Hz pointer  | 240 FPS         | ~80% reduction  |
| 6000 Hz pointer  | 240 FPS         | ~96% reduction  |
| Smooth scrolling | 400 Hz → 240 Hz | no kinetic loss |

This aligns with known patterns from:

* Chromium compositor
* Flutter engine
* Wayland input pipeline
* NVIDIA Reflex-style low-latency filtering

---

# **10. Implementation Checklist (Claude Code)**

### **1. Create module**

`src/core/input_fusion.rs`

### **2. Implement:**

* `PointerCoalescer`
* `ScrollCoalescer`
* `InputFusionLayer`

### **3. Modify window.rs / event loop:**

* intercept motion/scroll
* pass to fusion layer instead of router

### **4. Modify renderer frame loop:**

On each frame:

* ask fusion layer for coalesced events
* feed to event router

### **5. Update Event enum**

Add:

```
PointerMoveCoalesced
ScrollCoalesced
```

### **6. Preserve raw click events**

---

# **11. Final Notes**

This SPEC is **required** for:

* charts,
* scroll areas,
* selection drag,
* docking,
* high-DPI pointer devices,
* touchpads,
* pen devices (Wacom, Apple Pencil equivalents).

Without it, Halcyon will stutter under load.
