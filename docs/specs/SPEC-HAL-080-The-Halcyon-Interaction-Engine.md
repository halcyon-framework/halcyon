# **SPEC-HAL-080 — The Halcyon Interaction Engine (HIE v1)**

**SPEC-HAL-080**
**Halcyon Interaction Engine (HIE v1)**
**Author:** Halcyon Architect
**Status:** Draft / Non-authoritative exploration
**Version:** 1.0
**Controlling authority:** SPEC-HAL-021 owns generic animation semantics and
SPEC-HAL-042 owns literal frame order. HIE may coordinate already-authorized
owners only; it cannot replace, advance, sample for, or redefine Timeline.

**Depends On:**

* SPEC-HAL-009 Event Routing v2
* SPEC-HAL-021 Generic Animation Timeline
* SPEC-HAL-033 Input Fusion Layer
* SPEC-HAL-036 Input Coalescing
* SPEC-HAL-041 Unified Timing & Animation Clock
* SPEC-HAL-042 Frame Loop & Engine Ticker
* SPEC-HAL-074 Style Runtime
* SPEC-HAL-075 Layout Stability Guarantees
* SPEC-HAL-076 StyleCache v2
* SPEC-HAL-077 Style Animations
* SPEC-HAL-078 Layout Transitions
* SPEC-HAL-079 Unified Event Flow v3
* SPEC-HAL-147 Authorized Geometry-State Interpolation

**Explores:**
A proposed internal coordinator for **events → widgets → style → animations → state → updates** without transferring the named owners into one authority:
**The Halcyon Interaction Engine (HIE v1).**

HIE may centralize orchestration only. It does not own generic animation timing,
playback, sampling, or renderer mutation.

---

# **1. Motivation**

Halcyon’s input, state, style, and animation systems are currently correct but **distributed**:

* widgets run internal logic
* animations live in separate modules
* style updates propagate inconsistently
* state changes cause indirect re-layout
* event → UI → render pipeline has no central coordinator

This creates edge cases:

* stale style during animations
* event-driven layout invalidation not synchronized
* gesture events firing before style resolution
* transitions not respecting state updates
* difficulty adding higher-level UI behaviors

**The Halcyon Interaction Engine** provides one unified “brain”.

---

# **2. Core Idea**

HIE defines a **single authoritative pipeline**:

```
Inputs → Event Flow → State Update → Style Resolution → Layout → Animation → Render
```

Every frame, HIE drives this pipeline **in strict order**, ensuring determinism.

---

# **3. The HIE Loop**

At every frame or event tick:

```
1. Consume raw inputs (from OS)
2. Pre-process (coalesce, predict, fuse)
3. Unified Event Flow v3 (SPEC-079)
4. Widget State Updates (imperative)
5. Declarative Model Updates (if any)
6. Style Resolution (SPEC-074/076)
7. Layout Pass (measure → arrange)
8. Transitions & Animations (SPEC-078/077)
9. Render Graph Build
10. GPU Submission
```

All driven by **the single Halcyon Clock** (SPEC-115).

---

# **4. Widget Lifecycle (New)**

### **4.1 Lifecycle States**

Each widget cycles through:

```
Idle → Hot → Active → Updating → Animating → Measuring → Arranging → Painting
```

**Idle:** no input
**Hot:** hover/touch start
**Active:** captured / pressed
**Updating:** state changed via events
**Animating:** any animation affecting style/layout
**Measuring/Arranging:** layout pass
**Painting:** renderer pass

### **4.2 Mandatory Lifecycle Hooks**

HIE gives widgets explicit lifecycle entry points:

```
on_pre_event()
on_event()
on_post_event()

on_state_update()
on_style_update()
on_layout_warmup()

on_measure()
on_arrange()

on_paint()
```

Nothing is optional. Everything participates.

---

# **5. Interaction State Graph**

Each widget receives a tiny, guaranteed-consistent state machine:

```
InteractionState {
    hot: bool,
    active: bool,
    focused: bool,
    disabled: bool,
    capture: CaptureMode,  // None, Soft, Hard
}
```

Updated only by HIE, never by widgets manually.

---

# **6. High-level Interaction Behaviors**

HIE allows building higher-level interactions at the engine level:

* toggles
* switches
* disclosure panels
* menus
* hover-driven popups
* focus traversal
* keyboard navigation
* shortcuts
* “hot zones”
* semantic events (“activate”, “cancel”, “submit”)

These are implemented centrally and exposed to widgets.

---

# **7. Semantic Event Model (New)**

HIE separates:

### **Pointer events:**

low-level (down, move, up, wheel)

### **Widget events:**

gesture-level (tap, long-press, drag)

### **Semantic events:**

high-level (activate, open, close, toggle, submit)

Pipeline:

```
Pointer → Gesture → Semantic → WidgetState
```

For example:

```
Tap on Button → SemanticEvent::Activate → ButtonState::Pressed
```

---

# **8. State Reactor (Declarative Update Layer)**

Halcyon widgets can be declarative without a separate DSL.

HIE v1 adds **StateReactors**, small closures subscribed to widget IDs:

```
subscribe(widget_id, |old_state, new_state| {
    update_model();
});
```

Triggered after state change but before style recalculation.

Not a replacement for signals; a low-level foundational mechanism.

---

# **9. Unified Style Update Phase**

In v1.0:

* style was applied lazily
* animations sometimes applied stale style
* transitions applied mid-layout

HIE fixes this:

```
State changes → StyleUpdateQueue → StyleCache v2 → LayoutInvalidateQueue
```

Explicit phases:

1. collect changed widgets
2. resolve style for each
3. determine if layout must re-run
4. propagate layout invalidation
5. queue animation transitions

No style recalculation mid-render.

---

# **10. Layout Integration**

Layout runs only when needed:

```
if (any widget needs re-layout):
    run layout pass
```

HIE tracks causality:

```
interaction event → state change → style → layout → render
```

No redundant passes.

---

# **11. Animation Integration**

HIE coordinates animation systems:

* style transitions (SPEC-077)
* layout transitions (SPEC-078)
* spring/curve animations (timing subsystem)
* gesture-driven kinetic scrolling

Rules:

1. Engine-owned scroll visual state and existing generic Timeline handles advance
   before event routing under SPEC-HAL-042.
2. Event routing may set targets and start/cancel handles; Widget update samples
   SPEC-HAL-021 Timeline state read-only and publishes owner state before layout.
3. Layout remains canonical geometry authority; SPEC-HAL-147 geometry-transition
   projection runs after layout and cannot write canonical state.
4. Gesture/scroll physics remain separate from generic Timeline playback while
   consuming the same captured engine timebase.

This avoids tearing, gaps, and drift.

---

# **12. Focus & Keyboard Model**

HIE adds:

* tab-index system
* directional focus navigation
* programmatic focus requests
* selection ring visuals (style-driven)
* focus persistence rules

Keyboard events become part of semantic events:

```
Enter → Activate  
Space → Activate  
Esc → Cancel  
Tab → FocusNext
```

---

# **13. Compositor Integration (Future)**

HIE prepares Halcyon for:

* modal layers
* popups
* toasts
* tooltips
* overlay windows
* focus guards
* subtrees with independent coordinate systems

All routed through HIE.

---

# **14. Testing Matrix**

### **Lifecycle Tests**

* proper transition ordering
* widget entering/leaving states
* style recalculation consistency

### **Gesture Integration**

* unified gesture → semantic event chain
* capture priority
* arena winner selection

### **Style & Layout**

* style updates queued correctly
* no mid-pass invalidation
* animation + layout synchronization

### **Focus Tests**

* tab traversal
* directional navigation
* popup focus confinement

### **Regression**

* no stale state
* no jitter from event scheduling
* no race between animations and layout

---

# **15. Implementation Plan**

### **Phase 1 — Engine Core**

* HIE driver loop
* phase system
* event → state → style → layout → render order

### **Phase 2 — Lifecycle Model**

* lifecycle hooks
* state graph
* widget registration

### **Phase 3 — Semantic Event Layer**

* Tap → Activate
* Keyboard → Activate/Cancel
* initial semantic event set

### **Phase 4 — StateReactors**

* dependency tracking
* dirty-flag propagation

### **Phase 5 — Integrated Style Updates**

* StyleCache v2 binding
* invalidation model

### **Phase 6 — Layout Integration**

* layout warmup
* causality tracking
* transition-aware hit testing

### **Phase 7 — Animation Integration**

* unified Halcyon Clock
* transition → animation → layout sequencing

### **Phase 8 — Focus System**

* focus ring
* navigation
* directional heuristics

### **Phase 9 — Expansion**

* overlays
* modals
* window-based input routing
