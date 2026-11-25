# **SPEC-HAL-044: Dedicated UI Thread & Engine Isolation**

**Status:** Draft
**Author:** JC
**Depends on:**

* SPEC-HAL-042 (Frame Loop & Engine Ticker)
* SPEC-HAL-043 (Timers, Throttling, Idle Frames)

**Planned follow-ups:**

* SPEC-HAL-045 (GPU Triple-Buffer Scene Streaming)
* SPEC-HAL-046 (High-Frequency Scheduler)

---

## **1. Summary**

This SPEC defines the separation of Halcyon’s architecture into:

* **UI Thread** — owns widget tree, layout, event routing, scene construction
* **Platform Thread** — owns OS event loop (winit) and GPU device
* **Engine Channels** — high-level message passing between both

The goal is to isolate the UI engine from platform constraints, enabling:

* predictable frame timing
* non-blocking rendering
* triple-buffer scene streaming (045)
* high-frequency chart updates (046)
* prevention of OS-level stalls (macOS focus throttling, Windows compositor delays)

This SPEC does *not* implement GPU streaming nor async renderer execution — those are defined in 045. Here we simply establish the **threading model and contracts**.

---

## **2. Motivation**

Current Halcyon runs entirely inside the OS event-loop thread.
Limitations:

* Layout and heavy UI work occur inside the same thread as GPU rendering.
* Winit platforms (macOS/Windows) impose throttling when unfocused or minimized.
* GPU operations can cause pipeline stalls that block UI responsiveness.
* Future 046 scheduler would choke under GPU load.
* Dirty-tracking and layout recalculation need isolation from presentation timing.

We need a **strict split**:

* Platform thread handles window, events, swapchain, GPU.
* UI thread handles the full engine ticker (042/043).

Renderer becomes a consumer of scenes produced by the UI thread.

---

## **3. Goals**

1. Full UI execution occurs in **one dedicated thread**.
2. Platform thread becomes a **thin host** that feeds raw events and consumes rendered scenes.
3. No blocking cross-thread calls — only message passing.
4. Prepare for:

    * triple-buffer scenes (045)
    * ultra-high-frequency schedulers (046)
    * multi-window future support

---

## **4. Non-Goals**

* No GPU streaming yet (045).
* No async renderer (045).
* No multi-threaded layout.
* No parallel widget-tree traversal.
* No custom executor or async runtime.

This SPEC defines isolation, not concurrency scaling.

---

## **5. Model Overview**

### **5.1 Threads**

```
[Platform Thread]  --|send RawInputEvent|-->  [UI Thread]
[Platform Thread]  <--|recv SceneFrame|-----  [UI Thread]
```

### **5.2 Roles**

#### **Platform Thread**

* Owns OS event loop
* Owns GPU device, queue, swapchain
* Converts OS events → RawInputEvent
* Forwards them immediately (non-blocking) to UI thread
* Receives scene frames to render

#### **UI Thread**

* Runs full Engine Ticker (042) + Frame Timers (043)
* Owns widget tree, event router, input fusion
* Runs layout, scene build, and pushes scene to channel
* NEVER touches GPU or window directly

---

## **6. Engine Channels**

Two unidirectional bounded channels:

### **6.1 Input Channel**

Platform → UI

```
mpsc::Sender<RawInputEvent>
mpsc::Receiver<RawInputEvent>
```

Always non-blocking on send.
If full, drop low-priority events (pointer move), keep high-priority ones (down/up, wheel, key).

### **6.2 Scene Channel**

UI → Platform

```
mpsc::Sender<SceneFrame>
mpsc::Receiver<SceneFrame>
```

Bounded size = 2 (preparing for triple-buffer).
If full: UI overwrites the oldest frame (always discard stale scenes).

---

## **7. UI Thread Lifecycle**

UI thread runs:

```
loop {
    now = Instant::now()
    drain_input_channel()             → RawInputEvent[]
    engine.tick(now, raw_events)      → SceneFrame?
    if scene produced:
         send to platform thread
}
```

Frame loop includes:

* timers (043)
* throttling (043)
* idle frames (043)
* momentum (036)
* layout/scene build (042)

UI thread never sleeps except controlled by ticker (043's decisions).

---

## **8. Platform Thread Lifecycle**

```
loop {
    winit.poll_events()
    forward_raw_events_to_ui()
    if scene_available:
         render(scene)
    request_redraw_from_winit()
}
```

Key rule:
**Platform thread never blocks on UI thread.**
It simply pushes events and draws whatever scene is newest.

---

## **9. Frame Reason Mapping (042/043)**

UI decides frame reasons:

* Input
* Timer
* Idle
* Force
* Redraw

Platform thread does not influence these.
It only requests redraw when presenting a frame (winit constraint).

---

## **10. Integration With Scene Graph**

UI thread sends a `SceneFrame`:

```
pub struct SceneFrame {
    pub build_time: Instant,
    pub frame_id: u64,
    pub root_gpu_scene: GpuScene,   // CPU-side description
    pub stats: FrameStats,
}
```

Platform thread converts it into GPU commands.
045 introduces triple-buffering; 044 simply defines the message format.

---

## **11. Failure & Backpressure Handling**

### **11.1 Input Overflow**

Drop pointer moves first, then scroll, never drop down/up.

### **11.2 Scene Overflow**

Discard oldest scene.
UI always prioritizes fresh scenes.

### **11.3 UI Thread Panics**

Platform thread logs and exits gracefully without corrupting GPU state.

---

## **12. Testing Strategy**

1. **Thread isolation test:**

    * UI thread must run even if GPU rendering stalls.
    * Momentum, timers, idle frames continue uninterrupted.

2. **Event ordering tests:**

    * Ensure RawInputEvents remain strictly ordered.

3. **Scene freshness test:**

    * Scene channel should always return newest scene.

4. **Stress tests:**

    * Artificial 100ms GPU stalls should not pause input fusion or scheduler.

5. **Platform parity:**

    * macOS, Windows, Linux must produce identical UI-thread frame timelines.

---

## **13. Migration Plan**

1. Extract engine loop from examples.
2. Create UI thread integrated into example runtimes.
3. Introduce input/scene channels.
4. Migrate renderer to consuming scenes only.
5. Ensure layout & input no longer occur inside winit callback.
6. Prepare ground for 045 triple-buffer streaming.

---

## **14. Future Work**

Direct follow-ups:

* **SPEC-HAL-045** — Scene triple-buffering using this UI thread
* **SPEC-HAL-046** — Ultra-high-frequency chart scheduler running in UI thread
* Animated widgets, async loaders, and GPU pipelines rely on this isolation
