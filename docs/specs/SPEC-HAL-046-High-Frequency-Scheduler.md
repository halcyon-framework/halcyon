# **SPEC-HAL-046: Unified High-Frequency Scheduler (Charts, Animations, Low-Latency Systems)**

**Status:** Implemented
**Author:** JC
**Depends on:**

* SPEC-HAL-042 (Frame Loop & Engine Ticker)
* SPEC-HAL-043 (Timers, Throttling, Idle Frames)
* SPEC-HAL-044 (Dedicated UI Thread)
* SPEC-HAL-045 (Triple-Buffer Scene Streaming)

**Planned follow-ups:**
None — this is the top of the timing chain.

---

## **1. Summary**

This SPEC introduces the **Unified High-Frequency Scheduler (UHFS)** — a subsystem running *inside the UI Thread* that provides deterministic, low-jitter scheduling for:

* real-time trading charts
* animated widgets
* physics-like kinetic systems (scroll momentum, inertial zoom)
* algorithmic triggers requiring sub-frame precision
* sub-refresh-rate updates (120/144/240/360 Hz) independent of UI refresh rate
* stable “virtual time” for simulation loops

Unlike timers (043), UHFS runs at **high frequency**, **fixed timestep**, and **independently** of frame rendering.

This SPEC completes Halcyon’s timing architecture.

---

## **2. Motivation**

Charts, especially high-density trading charts, need:

* **fixed-step updates** (e.g., 120 Hz or 240 Hz internal simulation)
* **low jitter**, even when frame rate fluctuates
* **deterministic dt** independent of refresh rate
* **wall-clock decoupling** for backtesting, playback, and accelerations
* **smooth interactions even under rendering load**

Problems in current architecture (before 046):

1. Frame time fluctuates → chart updates become uneven.
2. Rendering stalls → simulation becomes irregular or stops.
3. UI thread load or GPU load → input-driven signals (like momentum) get jittery.
4. No concept of **fixed timestep** simulation.

UHFS solves all these.

---

## **3. Goals**

1. Provide a **fixed-rate simulation loop** independent of frame rendering.
2. Run entirely in the **UI thread**, guaranteeing ordering relative to input.
3. Integrate tightly with:

    * Frame Loop (042)
    * Throttling & Idle (043)
4. Feed:

    * Chart lanes
    * Animated widgets
    * Inertial systems
5. Never block UI or GPU.
6. Expose clean API for widgets/subsystems.

---

## **4. Non-Goals**

* No parallel simulation
* No GPU compute
* No physics engine
* No separate simulation thread

This SPEC is conservative, deterministic, and predictable.

---

## **5. Architecture**

### **5.1 Scheduler Placement**

Runs inside UI thread, before layout/scene build:

```
EngineTick {
    now = Instant::now()
    frame_time = ticker.begin_frame(now)
    scheduler.run(&frame_time)  // <-- SPEC-046
    input_fusion.process_frame()
    route_events()
    layout/scene/update
    push_scene_to_ring()
}
```

UHFS MUST run immediately after frame timing is computed (`begin_frame`) and
BEFORE input fusion and layout. This guarantees:

* deterministic virtual-time progression
* stable simulation even if frame rate jitters
* input fusion does not retroactively affect UHFS steps

### **5.2 Fixed Timestep**

Define:

```
SCHED_HZ = configurable (default 120Hz)
FIXED_DT = 1.0 / SCHED_HZ (in seconds)
```

Scheduler maintains:

```
accumulator += (now - last_tick)
while accumulator >= FIXED_DT:
    run all scheduled jobs with dt=FIXED_DT
    accumulator -= FIXED_DT
```

This ensures:

* deterministic updates
* no frame skipping of simulation
* no dependence on rendering speed

### **5.3 Virtual Time**

Expose a `VirtualTime` counter:

```
pub struct VirtualTime(f64); // seconds
```

Incremented by FIXED_DT each simulation tick.

**Monotonicity Guarantee:**

Virtual time MUST be strictly monotonic and MUST advance exactly by `FIXED_DT`
per simulation step. Implementations MUST NOT use real frame dt to advance
simulation time.

Widgets and chart systems use virtual time for:

* interpolation
* animation curves
* playback
* stable durations

Virtual time is not tied to wall-clock time.

---

## **6. Scheduler Jobs**

Two job classes:

### **6.1 High-Frequency Jobs (HFJ)**

Run every simulation tick (120–240 Hz).

Used for:

* chart feed integration
* audio-like interpolation
* crosshair inertia
* multi-touch smoothing
* advanced scroll momentum (future)

### **6.2 Interpolated Jobs (IJ)**

Expose values that UI uses for rendering interpolation between simulation ticks.

Used for:

* animated transitions
* smooth panning/zoom
* value easing functions

---

## **7. API**

### **7.1 Registering Jobs**

```rust
let id = scheduler.register_hfj(|cx: &mut HFContext| {
    // dt = fixed step
});
```

### **7.2 Removing**

```rust
scheduler.remove(id)
```

### **7.3 HFContext**

```
pub struct HFContext<'a> {
    pub dt: f32,
    pub virtual_time: f64,
    pub events: &'a mut EventBus,   // for signaling widgets
    pub signals: &'a SchedulerSignals,
}
```

### **7.4 EventBus Allocation**

> **NOTE:** The event bus MAY use a dynamically sized buffer (e.g., `Vec`) during the
> initial SPEC-046 implementation. Future revisions (e.g., SPEC-05x) SHOULD move to
> a fixed-capacity or preallocated buffer to guarantee zero-allocation behavior
> in the scheduling hot path.

---

## **8. Integration with Scene Rendering**

Scheduler runs *before* layout & scene:

1. Simulation updates produce new internal state
2. Layout reads that state
3. Scene reads the post-layout state
4. Scene pushed to ring (045)
5. GPU renders asynchronously

This preserves causal ordering:

```
input → simulation → layout → scene → GPU
```

---

## **9. Overload Handling**

The scheduler MUST enforce an upper bound on catch-up simulation steps.
Implementations SHOULD expose a `max_catchup_steps` parameter (default: 4).

When the accumulator corresponds to more than `max_catchup_steps` fixed steps,
only `max_catchup_steps` are executed and the remainder are counted as dropped
steps. This preserves UI responsiveness under load while maintaining a stable
simulation rate.

Behavior:

* Compute pending steps: `steps = floor(accumulator / FIXED_DT)`
* If `steps > max_catchup_steps`:
  * `dropped_steps += steps - max_catchup_steps`
  * `steps = max_catchup_steps`
* Execute exactly `steps` simulation ticks
* Emit `dropped_steps` counter for diagnostics

Charts must handle dropped steps gracefully (e.g., interpolate or skip).

---

## **10. Testing Strategy**

1. **Determinism test**

    * identical input yields identical virtual time and internal state.

2. **Overload test**

    * simulate 80ms pause; confirm scheduler drops steps but preserves stability.

3. **Chart jitter test**

    * large random frame times → chart remains smooth.

4. **Ordering test**

    * ensure scheduler always runs before layout/render.

5. **Momentum consistency test**

    * SPEC-036 momentum inputs should be stable under GPU load.

---

## **11. Migration Plan**

1. Introduce scheduler struct with accumulator logic.
2. Expose public registration APIs.
3. Integrate into `EngineTicker::tick()` before layout.
4. Modify chart widgets to use virtual time/hfj.
5. Validate against real chart data.
6. Prepare for multi-lane schedulers (future).

---

## **12. Future Work**

* Multi-lane multi-Hz schedulers (60Hz UI + 240Hz chart)
* GPU compute lanes (far future)
* Replay/recording of fixed-step time
* Dynamic Hz based on chart complexity
