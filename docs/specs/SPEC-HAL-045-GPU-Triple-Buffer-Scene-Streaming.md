# **SPEC-HAL-045: GPU Triple-Buffer Scene Streaming**

**Status:** Draft
**Author:** JC
**Depends on:**

* SPEC-HAL-042 (Frame Loop & Engine Ticker)
* SPEC-HAL-043 (Timers, Throttling, Idle Frames)
* SPEC-HAL-044 (Dedicated UI Thread)

**Planned follow-ups:**

* SPEC-HAL-046 (High-Frequency Scheduler)

---

## **1. Summary**

This SPEC introduces **triple-buffered GPU scene streaming** between:

* the **UI thread** (scene producer)
* the **Platform/GPU thread** (scene consumer)

Triple-buffering prevents:

* stalls caused by GPU waiting on UI
* stalls caused by UI waiting on GPU
* missed frames during heavy scene construction
* inconsistent frame pacing when GPU is overloaded

The system guarantees:

* UI thread **always runs at full speed**, regardless of GPU
* GPU thread **always renders the freshest available scene**
* no cross-thread blocking ever occurs

This SPEC does *not* define GPU scheduling or fancy pipelines.
It only defines the **data structure**, **channels**, and **lifetime rules**.

---

## **2. Motivation**

Currently:

* UI and GPU share the same thread.
* Scene build + GPU submission occur sequentially.
* A slow GPU stalls UI.
* A slow UI starves GPU.

This destroys:

* animation smoothness
* input responsiveness
* scroll momentum stability
* consistency of the Engine Ticker (042/043)
* viability of high-frequency charting (046)

Triple-buffered streaming decouples both sides completely.

---

## **3. Goals**

1. UI thread never blocks on GPU state.
2. GPU thread never blocks waiting for scene build.
3. Always render the *newest* scene.
4. Avoid memory spikes and runaway buffering.
5. Prepare for:

    * async GPU pipeline (future)
    * high-frequency chart updates (046)
    * multi-window rendering

---

## **4. Non-Goals**

* No GPU-side multithreading.
* No parallel scene encoding.
* No automatic frame pacing (043 handles that).
* No advanced render graph.

---

## **5. Architecture**

### **5.1 Buffer model**

We maintain:

```
[Front Buffer]   — currently on-screen
[Mid Buffer]     — last submitted
[Back Buffer]    — newest produced by UI thread
```

These are **CPU-side scene frames**, not GPU textures.

At presentation time:

* GPU thread swaps Mid → Front
* UI thread writes into Back
* If Back is filled again before GPU consumes it, the older one is discarded

### **5.2 Channel**

Replace the simple bounded channel from 044 with a triple-buffer ring:

```
pub struct SceneRing {
    front: Option<SceneFrame>,
    mid:   Option<SceneFrame>,
    back:  Option<SceneFrame>,
}
```

API:

```rust
impl SceneRing {
    // UI thread
    fn push_new_scene(&mut self, scene: SceneFrame);

    // GPU thread
    fn take_latest(&mut self) -> Option<SceneFrame>;
}
```

Rules:

* Push always writes to **back**.
* If back occupied → overwrite it (discard older).
* GPU take:

    * if back available → move back → front
    * else if mid available → move mid → front
    * else use previous front

This ensures GPU always gets the freshest possible scene without blocking UI.

---

## **6. SceneFrame Contract**

SceneFrame is defined in 044:

```
pub struct SceneFrame {
    pub frame_id: u64,
    pub build_time: Instant,
    pub root_gpu_scene: GpuScene, // CPU-side commands
    pub stats: FrameStats,
}
```

**045 adds one new rule:**

`GpuScene` must be **fully CPU-ready** and require *no UI-thread access* during GPU encoding.

This means:

* all buffer descriptors
* draw lists
* clip stacks
* material instances

… must be finalized inside UI thread.

Renderer treats it as immutable.

---

## **7. GPU Thread Flow**

Each redraw request:

```
if let Some(frame) = scene_ring.take_latest() {
    renderer.encode(frame.root_gpu_scene)
}
renderer.present()
request_redraw()
```

Notes:

* Always request redraw to keep GPU pumping.
* GPU thread should never sleep longer than OS throttling rules allow.
* UI thread’s ticker (042) drives scene production, not winit.

---

## **8. UI Thread Flow**

At end of every engine tick:

```
if scene_changed {
    let scene = build_scene(...)
    scene_ring.push_new_scene(scene)
}
```

UI thread does not know when GPU consumes the scene.
It only produces.

---

## **9. Backpressure & Overload Safety**

### Case 1: UI faster than GPU

UI overwrites back buffer; GPU uses freshest scene.
No mismatch.

### Case 2: GPU faster than UI

GPU keeps rendering the last scene.
Frame pacing is handled by 043.

### Case 3: Extreme load

Triple-buffer structure prevents both memory leak and starvation.

---

## **10. Multisampling, HDR, Swapchain**

Triple-buffering is CPU-side only.
Swapchain remains double-buffered or triple-buffered depending on platform.

045 does not change GPU format; only scene delivery.

---

## **11. Testing Strategy**

1. **Stall GPU artificially (100–200 ms).**
   UI momentum + animations must continue smooth.

2. **Stall UI artificially.**
   GPU must keep drawing old scene without blocking.

3. **Hammer renderer with 3000 frames/sec UI production.**
   GPU must always render the freshest.

4. **Recreate swapchain during resize.**
   Scene ring must remain stable across reallocation.

5. **Stress test multiple windows (future).**

---

## **12. Migration Plan**

1. Introduce `SceneRing`.

2. Update renderer to pull from it.

3. Move all scene production exclusively into UI thread.

4. Remove direct drawing from examples.

5. Example runtime updates:

    * platform thread renders continuously
    * UI thread ticks independently

6. Prepare integration point for 046 scheduler.

---

## **13. Future Work**

* GPU encoding parallelization
* Compressed scene deltas
* Direct GPU-side scene graph
* Multi-window scenes
* High-frequency chart lanes (046)
