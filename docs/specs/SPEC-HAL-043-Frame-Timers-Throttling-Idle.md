# **SPEC-HAL-043: Frame Timers, Throttling & Idle Frames**

**Status:** Draft
**Author:** JC
**Depends on:** SPEC-HAL-042 (Frame Loop & Engine Ticker)
**Constrained by:** SPEC-HAL-021 (Generic Animation Timeline). Frame timers own
callback scheduling only; they cannot emulate `Loop`/`Alternate` playback by
restarting `Once`, advance Timeline, or create animation-owned frame cadence.
**Planned follow-ups:** SPEC-HAL-044/045/046

---

## **1. Summary**

This SPEC introduces three fundamental timing mechanisms that extend the frame loop defined in SPEC-HAL-042:

1. **Frame Timers**
   One-shot or repeating callbacks executed at the start of a frame.

2. **Frame Throttling**
   Engine-controlled min/max FPS boundaries, independent of OS event scheduling.

3. **Idle Frames**
   A mechanism for the engine to wake itself even when no input or OS events occur.

Collectively, these give the engine a **deterministic internal heartbeat**, enabling smooth animations, reduced CPU load, and predictable update cadence—critical for 044/045/046.

This SPEC does **not** introduce multithreading, GPU triple-buffering, or high-frequency schedulers; those are layered on top of these timing primitives.

---

## **2. Motivation**

The frame loop (042) defines *ordering* and *responsibilities* per frame, but does not define *when* frames occur.

Current limitations:

* Redraws depend solely on OS-driven events.
* No concept of "run a frame every X ms".
* No internal timers for widgets or engine tasks.
* No min/max FPS control.
* No idle wakeups → momentum, animations, scheduler (046) cannot run smoothly.

This SPEC introduces:

* A unified timing source.
* A registry of timers (sleep until next timer or next frame).
* Frame throttling and idle behaviors.
* Deterministic firing order of timed callbacks.

This is required for:

* UI thread isolation (044)
* GPU streaming loop (045)
* High-frequency chart scheduler (046)
* Smooth momentum (036)
* Any future animation subsystem

---

## **3. Goals**

1. **Deterministic timing** inside the engine.
2. **Stable firing contract** for timers before each frame.
3. **Throttling** through a min/max interval:

    * Lower bound avoids over-rendering.
    * Upper bound avoids stalling.
4. **Idle frames** to drive momentum, animations, and background tasks.
5. **Zero ambiguity** about when and why a frame is produced.

---

## **4. Non-Goals**

* No new threading model (044).
* No GPU-side buffering (045).
* No chart scheduler (046).
* No animation engine.
* No OS-level timers—engine timers live entirely inside the ticker.

---

## **5. Design**

### **5.1 Terminology**

* **FrameTimer:** A scheduled callback executed at the beginning of a frame.
* **Throttle:** Rules that prevent too-frequent or too-slow frame emission.
* **Idle Frame:** A frame produced with no external events.

All timing originates from the ticker’s monotonic `Instant`.

---

## **6. Engine Ticker Extensions**

SPEC-HAL-042 introduced:

```
tick(now):
    drain_os_events()
    update_input(now)
    flush_input()
    route_events()
    layout/scene/render
```

SPEC-HAL-043 extends this:

```
tick(now):
    run_due_timers(now)
    decide_if_frame_allowed(now)     <-- throttling
    maybe_sleep_until_next_event()
    (if frame allowed)
        update_input(now)
        flush_input()
        route_events()
        run_idle_callbacks_if_needed()
        layout/scene/render
```

Timing becomes an explicit decision layer, not implicit behavior.

---

## **7. Frame Timers**

### **7.1 Features**

* One-shot or repeating.
* Stored in a min-heap by fire-time.
* Fired **before input fusion** and layout.
* Fire order is stable and deterministic.
* Callbacks run inside the engine thread (single-threaded for now).

### **7.2 API Sketch**

```rust
TimerId schedule(Duration delay, TimerCallback cb);
TimerId schedule_repeating(Duration interval, TimerCallback cb);
fn cancel(TimerId id);
```

### **7.3 Execution Order**

At the start of a frame:

1. Pop all timers with `time <= now`.
2. Execute callbacks immediately.
3. Reinsert repeating timers with new fire time.

Timers always run before processing input, ensuring predictable causality.

---

## **8. Frame Throttling**

Ticker receives min/max FPS configuration:

```rust
pub struct FrameLimits {
    pub min_frame_interval: Duration, // Never run faster than X ms
    pub max_frame_interval: Duration, // Wake up at least every Y ms
}
```

### **8.1 Rules**

* If last frame was too recent → **skip** frame.
* If last frame was too old → **force** frame (idle wake).
* OS events always override throttling (input forces a frame).
* Continuous visual/update demand uses the current display refresh interval when the platform reports it; the fallback is
  120 Hz.
* A due deadline advances from its prior scheduled phase by the required whole number of intervals. Ordinary wake latency
  never becomes part of the cadence, and a multi-period stall skips elapsed intervals without emitting catch-up frames.
* Moving a window to a display with a different reported refresh rate resets the active deadline against the new interval.

### **8.2 Purpose**

* Avoid 1000+ FPS runaway loops on some platforms.
* Avoid stalls where no frames occur for long periods.
* Provide stable timing for future animations/schedulers.

---

## **9. Idle Frames**

Idle frames are engine-driven frames required even when:

* No OS events
* No timers due
* No input
* No layout invalidation

They power:

* scroll momentum continuation (036)
* animations (future)
* high-frequency scheduler (046)
* recovery from OS-level throttling

### **9.1 Trigger Rules**

Idle frame is generated if:

* No input events,
* No timers firing,
* `now - last_frame_time >= max_frame_interval`.

Idle frames always run **update_momentum**, route zero events, then perform layout+render.

---

## **10. Frame Decision Logic**

Pseudocode:

```rust
fn maybe_issue_frame(now):
    if should_force_frame(now):           // max-frame-interval passed
        return FrameReason::Force

    if has_input_events():                // OS-driven
        return FrameReason::Input

    if timers_due(now):                   // scheduled
        return FrameReason::Timer

    if now - last_frame < min_interval:   // throttling
        return FrameReason::Skip

    // no events; idle frame
    return FrameReason::Idle
```

This gives the engine a deterministic reason for each frame.

---

## **11. Interaction With Input Fusion (036)**

* `update_momentum(now)` happens for **every allowed frame**, including `Idle`.
* When throttling skips a frame, momentum does not step.
* Idle frames guarantee momentum progresses smoothly even with low input frequency.
* Frame timers can schedule synthetic events that propagate normally through event routing.

---

## **12. Testing Strategy**

1. Unit tests:

    * Timers fire at correct times.
    * Repeating timers reschedule correctly.
    * Cancel works.
    * Throttling enforces min/max intervals.
    * Idle frames occur exactly as defined.

2. Integration tests:

    * Momentum continues during throttled or idle periods.
    * Timer-triggered layout changes cause the next frame to render correctly.

3. Platform sanity:

    * All examples must use the ticker and remove any manual sleeps/delays.

---

## **13. Migration Plan**

1. Introduce FrameTimer and throttle logic inside the core ticker.
2. Update examples to remove explicit redraw requests where unnecessary.
3. Ensure `RedrawRequested` triggers only a *request*, not forced rendering.
4. Replace ad-hoc timers (if any) with engine timers.
5. Verify momentum and idle frames behave identically across OSes.

---

## **14. Future Work**

This SPEC prepares the ground for:

* **044:** a separate UI thread
* **045:** triple-buffer GPU scene streaming
* **046:** unified high-frequency scheduler

Timer infrastructure and throttling rules directly power all three.
