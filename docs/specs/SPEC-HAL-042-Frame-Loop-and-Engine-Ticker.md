# SPEC-HAL-042: Frame Loop & Engine Ticker

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

Status: Implemented baseline; 2026-08-23 binding animation amendment pending
Author: JC / JC
Depends on: SPEC-HAL-032 (Hit-Test Acceleration), SPEC-HAL-033 (Input Fusion Layer), SPEC-HAL-036 (Input Coalescing)
Planned follow-ups: SPEC-HAL-043/044/045/046
Extended by: SPEC-HAL-021 for generic Timeline advance and same-frame sampling; SPEC-HAL-354 for cadence-neutral data invalidation, no-op clean phases, retained scene fragments, and proportional renderer preparation

---

## Implementation Status

**Core frame loop**: Implemented in `src/core/app.rs` `run_ui_loop()`.

**EngineTicker**: Implemented with `begin_frame()` as sole `Instant::now()` source per SPEC-HAL-041.

**Examples**: All examples (`input_fusion_debug`, `flow_grid_*`, `shortcut_demo`) use canonical frame loop.

**Animation phases**: The live loop already advances engine-owned scroll visual
state and existing Timeline handles before event routing, then runs Widget update
after routing and before layout. SPEC-HAL-021's read-only UpdateContext sampling,
handle-state, and playback contract is binding but not yet implemented. Scroll
smoothing remains its separate subsystem and is not a Timeline playback mode.

---

## 1. Summary

This SPEC defines the **authoritative frame loop** for Halcyon and introduces the **Engine Ticker** as the single source of timing for:

- input fusion (including momentum)
- animation updates (scroll smoothing, transitions)
- layout and scene updates
- rendering and presentation
- future timers, throttling, and high-frequency subsystems

The goal is to move from ad-hoc per-example event loops to a **single, well-specified frame contract** that all platforms, examples, and later engine layers must follow.

This SPEC does **not** introduce advanced throttling, timers, or multi-threaded rendering. Those are defined in SPEC-HAL-043 through SPEC-HAL-046 and hook into the frame loop defined here.

---

## 2. Motivation

Right now, examples manually wire:

- `InputFusion::process_raw`
- `InputFusion::update_momentum`
- `InputFusion::flush`
- event routing
- layout + paint
- renderer calls

This leads to:

- subtle differences between examples
- unclear ordering between input, momentum, layout, and rendering
- difficulty introducing global frame-rate control, timers, or a separate UI thread

We need a **single, canonical frame loop** that:

1. Guarantees a stable order of operations per frame.
2. Centralizes time (now/dt) so all subsystems agree.
3. Provides clean extension points for:
   - frame throttling and idle frames (043)
   - a dedicated UI thread (044)
   - triple-buffered scene streaming (045)
   - high-frequency chart scheduler (046)

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

- R1: Every frame has a **single timestamp (`now`)** and a **delta (`dt`)** used across:
  - coalescing / momentum
  - animations
  - scheduling (later)
- R2: Input fusion is always updated in this order:
  1. process all raw input events
  2. update momentum once per frame
  3. flush coalesced events
  4. route fused events to widgets
- R3: Existing Timeline handles advance exactly once from the captured frame time before event routing. Event-started handles bind to that same time and therefore sample exact `from` in the start frame.
- R4: After event routing, Widget update samples Timeline read-only, publishes widget-owned presentation state, and reports the narrowest invalidation before layout.
- R5: Layout, post-layout transition projection, custom-render preparation, retained paint, rendering, and presentation consume the same frame's stable state in that order.
- R6: Layout and scene building run only when needed but, when they run, they use the **same `now`** as the rest of the frame.

### 3.2 Ordering invariants

Within a single frame:

1. **OS events drain** before the admitted frame consumes them.
2. EngineTicker captures the frame's sole timestamp/delta/index.
3. Engine-owned pre-routing visual state advances; all **raw → fused input** completes before event routing.
4. The generic Timeline advances exactly once before event routing.
5. **Event routing** may set targets and start/cancel handles at the captured time.
6. **Widget update** samples Timeline read-only and publishes owner state before layout.
7. **Layout** precedes post-layout geometry-transition projection.
8. Post-layout projection precedes custom-render preparation and retained scene paint.
9. Scene materialization precedes render, presentation, and frame completion.

**363/364 scoped amendment:** Items 6–9 are elaborated by §§6.6–6.7 for the paired contracts. This
adds no phase and moves none of input fusion, scroll advance, Timeline advance,
routing, layout, custom preparation or presentation.

### 3.3 Safety/consistency invariants

- I1: No subsystem is allowed to call `Instant::now()` for frame logic directly. All time-based decisions must use the `now` from the Engine Ticker.
- I2: `InputFusion::update_momentum` must be called at most **once per frame**.
- I3: No fused events from frame *N* may be delivered in frame *N+1* except those explicitly coalesced and flushed at frame start from the coalescing layer.
- I4: If the renderer reports the window as unfocused, the frame loop may skip GPU work but must still be able to:
  - drain events
  - update input state
  - run the pre-routing scroll-visual and Timeline advances and the pre-layout Widget update
  - resume cleanly when focus returns
- I5: Event handlers may set targets and start/cancel handles but do not advance Timeline or derive later-phase state. Widget-owned visual/animated values mutate during Update. Layout, custom preparation, and paint consume them read-only, except for the separate pure SPEC-HAL-147 post-layout geometry projection.

---

## 4. Design Overview

The frame loop is driven by an **Engine Ticker** which owns:

- the current frame time (`now`)
- the previous frame time (`last_frame_time`)
- the frame delta (`dt`)
- a **frame reason** enum describing why this frame exists (input, redraw, timer, idle, etc.)

At a high level:

```text
while running:
    // 1) Drain platform events → raw input, window events, quit signals
    drain_platform_events()

    // 2) Capture the one authoritative frame time
    tick = engine_ticker.next_frame()

    // 3) Advance engine-owned pre-routing visual state
    update_scroll_visuals(tick.dt)

    // 4) Feed raw input into InputFusion
    input_fusion.process_raw(...)

    // 5) Per-frame momentum and coalescing
    input_fusion.update_momentum(tick.now, &mut fused)
    input_fusion.flush(&mut fused)

    // 6) Advance existing generic Timeline handles exactly once
    timeline.advance(tick.now)

    // 7) Route fused events; handlers may set targets/start/cancel
    event_router.route_input_event(&mut widget_tree, fused_event)

    // 8) Sample Timeline in Widget update and publish owner state
    update_widgets(&mut widget_tree, timeline.samples())

    // 9) If dirty: layout; then post-layout projections/preparation/scene
    if frame_needs_layout_or_scene:
        layout_tree(...)
        compute_layout_transition_projection(...)
        prepare_custom_render_participants(...)
        build_scene(...)

    // 10) Render & present (if focused / visible)
    renderer.render_frame(...)

    // 11) Loop; ticker decides when the next frame is allowed
```

This SPEC locks down **what must happen and in what order**, but leaves **how we schedule the next frame** flexible for follow-up SPECs.

---

## 5. Engine Ticker API

### 5.1 Types

```rust
/// Why did we enter a frame?
pub enum FrameReason {
    /// OS requested a redraw (e.g., `RedrawRequested`)
    OsRedraw,
    /// At least one input event was processed since the last frame
    Input,
    /// A timer or animation requested a frame (SPEC-HAL-043/046)
    Timer,
    /// Periodic frame produced while idle (e.g., for subtle animations)
    Idle,
    /// Explicit request from engine code (e.g., resize, debug)
    Force,
}

/// Per-frame time snapshot used by all engine subsystems.
pub struct FrameTime {
    pub now: std::time::Instant,
    pub dt: std::time::Duration,
    pub frame_index: u64,
    pub reason: FrameReason,
}
```

### 5.2 EngineTicker responsibilities (in this SPEC)

For SPEC-042, `EngineTicker` is a **simple, local component**:

* Keeps `last_frame_time` and computes `dt` as `now - last_frame_time`.
* Determines a `FrameReason` based on:

    * whether any input has been seen since last frame
    * whether the OS requested a redraw
    * an optional `force_next_frame` flag (e.g., resize)
* Returns a `FrameTime` for each frame.

It does **not** yet:

* enforce min/max FPS
* run a timer wheel
* manage independent high-frequency clocks

That behavior is added in SPEC-HAL-043 and SPEC-HAL-046.

### 5.3 Expected minimal interface

```rust
pub struct EngineTicker {
    last_frame_time: Instant,
    frame_index: u64,
    pending_reason: FrameReason, // merged reason from previous loop
}

impl EngineTicker {
    pub fn new() -> Self { ... }

    /// Called by the platform loop when something *may* require a frame.
    pub fn notify_reason(&mut self, reason: FrameReason) {
        // merges reasons; Input dominates Idle, etc.
    }

    /// Called once per frame, just before frame processing.
    pub fn begin_frame(&mut self) -> FrameTime {
        let now = Instant::now();
        let dt = now - self.last_frame_time;
        let reason = self.pending_reason.take_or_default();
        self.last_frame_time = now;
        self.frame_index += 1;
        FrameTime { now, dt, frame_index, reason }
    }
}
```

The exact merging priority of reasons is part of SPEC-043; for 042 it is enough that:

* `Input` dominates `Idle`
* `OsRedraw` dominates `Idle`
* `Force` dominates everything

### 5.4 Delta time usage

All frame-time consumers receive the same `FrameTime`. Delta-based subsystems such
as scroll smoothing may use:

```rust
dt_secs = frame_time.dt.as_secs_f32()
```

The generic Timeline instead evaluates from absolute `frame_time.now` and its
recorded start time, per SPEC-HAL-021; it does not accumulate `dt`. The frame loop
guarantees one timestamp and one `dt` per frame. No subsystem may compute its own
delta or call `Instant::now()`.

---

## 6. Canonical Frame Loop

This section defines the **exact order of operations** within a frame. All platforms and examples must follow this order logically, even if their concrete code is slightly different.

### 6.1 Event drain phase

1. Platform layer (winit) produces events:

    * `WindowEvent::CloseRequested`
    * `WindowEvent::Resized`
    * `WindowEvent::ScaleFactorChanged`
    * `WindowEvent::Focused`
    * pointer / keyboard events
    * `RedrawRequested`

2. Platform adapter converts them to Halcyon events:

    * `RawInputEvent` (for pointer/keyboard, wheel, focus)
    * `HalcyonEvent::Resize`, `HalcyonEvent::ScaleChanged`, etc.
    * engine control events (`ExitRequested`)

3. All platform events for this iteration are drained into:

    * a `Vec<RawInputEvent>`
    * a `Vec<HalcyonEvent>` for non-input engine events
    * a boolean `os_requested_redraw`

4. For every input-related event, the platform adapter also calls:

```rust
engine_ticker.notify_reason(FrameReason::Input);
```

For every OS redraw event:

```rust
engine_ticker.notify_reason(FrameReason::OsRedraw);
```

Resizes and explicit internal requests may call:

```rust
engine_ticker.notify_reason(FrameReason::Force);
```

### 6.2 Frame tick

At the start of the frame loop body:

```rust
let frame_time = engine_ticker.begin_frame();
// frame_time.now and frame_time.dt are now the authoritative time for this frame.
```

This `frame_time` object is passed (or made visible) to any subsystem that needs `now` / `dt`.

**2026-10-08 scoped accounting clarification (HAL-FRAME-BEGIN-ACCOUNTING-2026-10-08-A1):**
The delegated technical reviewer accepted this correction under JC's standing delegation. The existing
once-per-frame `EngineTicker::begin_frame`, through `begin_frame_at`, also
initializes that actual frame's existing layout-call accounting. Debug builds
reject a frame begin from an executing frame critical section, Update, Layout,
Transition or Paint before resetting accounting or changing ticker state.
Callers begin each genuinely separate frame once and pass its captured timing
to the consuming scene path. Scene and layout helpers, including
`build_scene_with_appearance`, do not begin a frame or reset its accounting;
the existing SINGLE-LAYOUT-PER-FRAME / INV-SCENE-003 guard still rejects a second
layout in one begun frame. Native App and other public ticker consumers share
this owner. This changes no frame order, cadence, time calculation or surface
scope, and does not adopt the Draft or Proposed clauses referenced elsewhere.

### 6.3 Pre-Routing Visual State and Input Fusion

Immediately after the frame tick, engine-owned scroll visual state advances once
from the captured delta:

```rust
widget_tree.update_scroll_animations(frame_time.dt.as_secs_f32());
```

This update precedes event routing so hit-test projections use the visual scroll
state displayed for the frame. It is the separate delta-based scroll subsystem,
not generic Timeline playback.

For each `raw` in drained raw input:

```rust
input_fusion.process_raw(raw, &mut fused_events);
```

Then per frame:

```rust
input_fusion.update_momentum(frame_time.now, &mut fused_events);
input_fusion.flush(&mut fused_events);
```

Notes:

* scroll visual state advances at most once per admitted frame;
* `update_momentum` **must** be called exactly once per frame, before `flush`;
* `flush` must be called before any event routing; and
* `flush` emits coalesced pointer moves, scrolls, and drag updates.

Neither scroll advancement nor InputFusion routes an event itself.

### 6.4 Generic Timeline Advance

After InputFusion and before event routing, the engine advances existing generic
Timeline handles exactly once:

```rust
timeline.advance(frame_time.now);
```

Timeline derives playback from the absolute captured time under SPEC-HAL-021.
A handle started later in this frame receives no second advance; it binds to
`frame_time.now` and its first sample is exact `from`.

### 6.5 Event Routing Phase

For each `event` in `fused_events`:

```rust
event_router.route_input_event(&mut widget_tree, event);
```

This phase may:

* update event-owned target state
* request layout or paint invalidation
* set animation targets and start/cancel Timeline handles at the captured frame time

The routing phase **must not**:

* perform layout, scene build, or GPU work directly
* advance Timeline or sample/publish state reserved for Widget update
* call `Instant::now()` or compute time deltas

It only marks state and dirty flags.

### 6.6 Widget Update Phase

After routing and before layout, Widget update receives read-only same-frame
Timeline sampling. A newly started handle samples exact `from`; an existing
handle exposes the value already derived from `frame_time.now`.

Update may change widget-owned presentation values and revisions and must report
the narrowest valid invalidation. Once Update completes, Timeline and
widget-owned inputs are immutable through layout, post-layout projection, custom
preparation, paint, and rendering.

The old phrase “Animation Tick” covered the distinct §6.3 scroll update, §6.4
Timeline advance, and this Update-owned sampling boundary. It is not a legal
ordering shorthand. Timeline itself never schedules a frame. Active Timeline or
scroll state reports animation demand to the existing EngineTicker owner.

**363/364 scoped amendment:** SPEC-HAL-363 request/control mutation, timing reservation, interruption
and outcome reconciliation SHALL occur in Update. Their output SHALL be immutable
for the rest of the frame. Event routing may emit already-authorized actions; a
presentation request SHALL NOT create a new routing or scheduling phase.

### 6.7 Layout and Scene Phase

After Widget update completes:

1. Query the widget tree and/or engine state to see if layout or scene updates are needed (dirty flags).
2. If yes:

```rust
let constraints = ...; // root constraints from window size
layout_tree(&mut widget_tree, root, constraints, &theme, text_engine);

let scene = scene_builder.build(&widget_tree, &theme, text_engine);
```

**363/364 scoped amendment:** Layout consumes stable Update/style inputs and remains independent of
presentation projection and framework focus/capture publication. After layout,
SPEC-HAL-147 and SPEC-HAL-363/364 resolve geometry eligibility and effective
presentation ownership, then derive pure visual values from Update-published
parameters. SPEC-HAL-266A custom preparation follows that projection and precedes
retained paint. Retained scene construction may complete certified-source
admission without calling a widget twice. One complete presented scene is selected
before ordinary diff, mandatory dispatch and render preparation. Later
preparation failure SHALL use the owning complete failure path, not restart that
sequence. No listed phase samples or mutates Timeline except the capabilities
expressly granted by SPEC-HAL-021/354.

### 6.8 Render and Present Phase

If the window is focused or otherwise allowed to render:

```rust
renderer.set_focused(window_is_focused);

if renderer.is_focused() {
    renderer.begin_frame(window_size, scale_factor);
    renderer.render_scene(&scene);
    renderer.end_frame();
}
```

If the window is **not** focused:

* renderer is allowed to skip GPU work entirely, but the frame loop still proceeds so that internal state (input, timers, animations) remains consistent.
* `EngineTicker` may later be integrated with throttling (043) to reduce work when unfocused.

---

## 7. Integration with Existing Systems

### 7.1 InputFusion (SPEC-HAL-036)

This SPEC formalizes the place of `InputFusion` in the frame:

1. `process_raw` is called **per event** when events are drained.
2. `update_momentum(now, out)` is called **once per frame** with `frame_time.now`.
3. `flush(out)` is called **once per frame**, **after** `update_momentum`.
4. Timeline advances before routing; all fused events route before Widget update and layout/scene.

Existing examples that manually call `update_momentum` and `flush` in their `RedrawRequested` handlers must be migrated to use the Engine Ticker driven frame loop.

### 7.2 Event routing & widget tree

Event routing already exists as:

```rust
event_router.route_input_event(&mut widget_tree, fused);
```

The SPEC requires that:

* Routing happens only after all fusion for the frame is complete.
* Routing sets dirty flags and targets and may start/cancel Timeline handles instead of doing layout/paint directly.
* Routing does not advance Timeline or publish state reserved for Widget update.

### 7.3 ScrollContainer and Pre-Routing Visual Update

SPEC-HAL-086 introduces the dual-offset scroll model:

* `scroll_offset_target`: logical position set by event handlers
* `scroll_offset_visual`: smoothed position used for rendering

The engine-owned pre-routing visual-update phase calls
`scroll.update_visual(dt_secs)` to interpolate visual toward target. This is
scroll-subsystem behavior, not generic Timeline repetition.

**Widget event handlers only modify target offset:**

```rust
// In ScrollArea::handle_event
let (cur_x, cur_y) = self.scroll.target_offset();
self.scroll.set_target_offset((cur_x - delta_x, cur_y - delta_y));
// Do NOT call update_visual() here — the pre-routing engine phase handles it
```

**Rendering uses visual offset:**

```rust
fn scroll_offset(&self) -> Option<(f32, f32)> {
    Some(self.scroll.visual_offset())
}
```

### 7.4 Renderer & WgpuRenderer focus fix

The earlier Metal focus throttling fix introduced:

* `WgpuRenderer::set_focused(bool)`
* `WgpuRenderer::is_focused()` and early return in `render()` when unfocused

This SPEC simply requires that:

* The **render phase** respects these flags.
* The frame loop never blocks on GPU if `is_focused() == false`.

---

## 8. Interaction with Follow-Up SPECs

This SPEC intentionally stops short of advanced behavior. The later SPECs plug into the same skeleton:

* **SPEC-HAL-043: Frame Timers, Throttling, Idle Frames**

    * Extends `EngineTicker` with:

        * timer wheel or scheduled callbacks
        * min/max FPS logic
        * explicit idle frame scheduling
    * Adds new `FrameReason::Timer` and `FrameReason::Idle` semantics.
* **SPEC-HAL-044: Separate UI Thread**

    * Moves this frame loop into a dedicated UI/engine thread.
    * Defines how platform events are forwarded and how shutdown works.
* **SPEC-HAL-045: GPU Triple-Buffer Scene Streaming**

    * Splits the render phase so scene build happens on the UI thread and GPU submission on a render thread.
    * Reuses the same `FrameTime` but changes where the scene is consumed.
* **SPEC-HAL-046: Unified High-Frequency Scheduler (for charts)**

    * Adds high-frequency sub-tick scheduling inside the same frame envelope.
    * Uses `frame_time.dt` and timers from 043.

The key point: all of these features build on the **same frame order** defined here. None of them are allowed to reorder:

`platform event drain → frame time → pre-routing scroll visual advance → input fusion → Timeline advance → routing → Widget update sampling → layout → post-layout transition → custom preparation → retained scene → render → present`.

---

## 9. Testing Strategy

### 9.1 Unit-level

* Basic `EngineTicker` tests:

    * `dt` non-zero, monotonically advancing
    * `FrameReason` merge rules
* InputFusion conformance:

    * Verify that one call to `update_momentum` + `flush` per frame yields the same events as the existing tests assume.
* ScrollContainer visual update:

    * Verify `update_visual(dt_secs)` correctly interpolates toward target
    * Verify event handlers modify only target, not visual
* Generic Timeline ordering:

    * Verify exactly one pre-routing advance from the captured timestamp
    * Verify event-started handles sample exact `from` in same-frame Widget update
    * Verify existing handles sample current frame time and later phases read only

### 9.2 Integration-level

New example: `examples/frame_loop_debug.rs` (later):

* Logs each frame as:

    * `now`, `dt`, `reason`
    * counts of raw and fused events
    * flags: `did_timeline_advance`, `did_widget_update`, `did_layout`, `did_scene`, `did_render`

Manual testing:

* rapid scrolling with and without focus
* click / double-click / drag correctness with coalescing on
* alt-tab / cmd-tab focus loss and recovery
* resize behavior (scene and layout recomputed exactly when needed)
* scroll smoothing behavior (visual catches up to target over multiple frames)

---

## 10. Migration Notes

1. **Examples**

    * Replace per-example calls to:

        * `InputFusion::update_momentum(Instant::now(), ...)`
        * `InputFusion::flush(...)`
    * with calls driven by the shared frame loop using `EngineTicker::begin_frame()`.

2. **Time usage**

    * Any subsystem currently using `Instant::now()` directly inside the frame loop must instead take `FrameTime` or `now/dt` from the ticker.
    * Delta-based subsystems use `frame_time.dt`; generic Timeline playback uses
      absolute `frame_time.now` and never accumulates delta.

3. **Scroll animations**

    * Widgets must not call `scroll.update_visual()` in event handlers.
    * The pre-routing engine phase owns scroll visual advance; Widget event
      handlers own targets only.

4. **Future changes**

    * Once 043–046 are implemented, the core loop will remain the same; only the ticker's internals and how frames are scheduled will change.
    * The contract defined in Sections 6.1–6.8 must stay stable unless a future SPEC explicitly revises it.

---

## 11. Virtualization Frame Pipeline (Added 2025-12-23)

This section formalizes the frame pipeline ordering required for correct
virtualization behavior. This was discovered during the virtualization
black gap fix.

### 11.1 Extended Frame Pipeline

The full frame pipeline for virtualized content is:

```
0. pre_layout_sync_scroll_to_lists()       — sync scroll offset BEFORE layout
1. layout_tree()                           — compute widget geometry
2. post_layout_populate_input_projections()— build input projection snapshots
3. post_layout_resolve_scroll_anchors()    — resolve scroll anchors
4. post_layout_sync_scroll_to_lists()      — sync for anchor changes
5. paint_tree()                            — build scene graph
```

### 11.2 Invariants

**INV-PIPELINE-PRE-SYNC**

Pre-layout sync (step 0) handles scroll input (wheel, drag).
Ensures layout sees current scroll_y, preventing 1-frame lag.

**INV-PIPELINE-POST-SYNC**

Post-layout sync (step 4) handles anchor resolution effects.
Scroll anchors may change offset, requiring re-sync.

**INV-PIPELINE-ORDER**

The ordering is fixed. Reordering any step is forbidden.
Each step depends on outputs from previous steps.

**INV-PIPELINE-DIRTY**

If visible_range changes during pre-layout sync, the List MUST be
marked layout-dirty so layout_tree() will process it.

### 11.3 Cross-Reference

See: SPEC-HAL-131A Section L.4 (canonical definition)
See: SPEC-HAL-134 Section 21.4 (virtualization-specific ordering)

### 11.4 Paint-Layout Epoch Invariants (Added 2025-12-23)

**INV-PAINT-REQUIRES-LAYOUT-EPOCH**

A node may not be painted unless it was laid out in the current layout epoch.

- Enforcement: `paint_node()` asserts `node.layout_epoch == current_layout_epoch()` (debug builds)
- Location: `src/core/app.rs` paint_node function
- Violation symptom: Widget painted with stale geometry from previous frame

**INV-FIRST-LAYOUT-MANDATORY**

Layout may not be skipped for nodes that have never been laid out.

- New nodes are initialized with `dirty_layout = true` (WidgetNode::new)
- The skip optimization only activates if `dirty_layout == false`
- This ensures first-frame layout is always performed

**INV-LAYOUT-SKIP-GUARDED**

Layout skip optimization is only legal if:
1. Node has valid geometry (`cached_size`, `last_constraints` are set)
2. Node was laid out at least once (`dirty_layout` was cleared at least once)
3. Constraints have not changed

Violation: Skipping layout when these conditions are not met causes stale geometry during paint.

### 11.5 Paint Data Derivation Invariants (Added 2025-12-23)

**INV-PAINT-NO-DATA-DERIVATION**

Paint phase MUST NOT derive data indices from layout geometry.

- Widget geometry (`rect.x`, `rect.y`, `size`) is viewport-relative
- Data identity (logical index, item ID) MUST come from external state
- Virtualization state (visible_range.start) is authoritative for base_index

Forbidden pattern:
```rust
// WRONG: deriving logical_index from geometry
let logical_index = (rect.y / ITEM_HEIGHT).round() as usize;
```

Required pattern:
```rust
// CORRECT: deriving logical_index from virtualization state
let logical_index = base_index + slot_index;
```

See: SPEC-HAL-134 Section 21.6

---

This SPEC becomes the **ground truth** for how a Halcyon frame is produced. Every platform adapter, example, and future scheduling feature must follow this order and timing model.
