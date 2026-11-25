# **SPEC-HAL-070 — Halcyon Timeline Animation System (Keyframes, Curves, Graphs)**

**SPEC-HAL-070**
**Timeline Animation System**
**Author:** Halcyon Architect
**Status:** Draft / Non-authoritative exploration
**Version:** 1.0
**Controlling authority:** SPEC-HAL-021 is the sole generic animation Timeline
authority. This Draft does not authorize a second Timeline type, accumulated-`dt`
playback, keyframe/graph runtime, direct node/renderer writes, or a universal
replacement animation subsystem.

**Depends On:**

* SPEC-HAL-021 Generic Animation Timeline
* SPEC-HAL-024 Absolute Logical Space
* SPEC-HAL-034 Rendering Pass Graph
* SPEC-HAL-048 Text Layout Engine
* SPEC-HAL-066 Unified Glyph Raster Pipeline
* SPEC-HAL-068 Text Compositing Effects
* SPEC-HAL-069 Animated Text Effects

**Explores only:**
A historical fully general animation-engine proposal for:

* widget transitions
* opacity fades
* transforms
* color transitions
* text effect animations
* layout transitions
* scene-level choreography

Core features:

1. Timeline-driven keyframes
2. Easing curves
3. Declarative animation graphs
4. Interruptible animations
5. Reversible states
6. Deterministic progression
7. Zero-GC, stable-frame runtime
8. GPU-friendly parameter extraction

This proposal is not an implementation plan and does not replace the binding
SPEC-HAL-021 contract. Every sketch below is non-authoritative evidence only.

---

## **1. Motivation**

Animated UIs require a unified engine:

* button hover → fade + scale
* text focus → outline breath + cursor animation
* modal popups → opacity + translation
* transitions between screens
* toast notifications
* tab panels sliding
* keyboard focus rings

This Draft predates the binding generic Timeline decision. SPEC-HAL-021 now
defines that engine boundary; 070 records an unadopted broader design and owns no
runtime or renderer authority.

---

## **2. Goals**

### **2.1 Primary**

* Universal animation model
* Keyframes + curves
* Time-based and progress-based animations
* Node-scoped and scene-scoped timelines
* Zero drift between CPU and GPU
* Fully deterministic

### **2.2 Secondary**

* Interruptible transitions
* Reversible animations
* Highly compact instance data
* API pleasant enough for all widgets

---

## **3. Architecture Overview**

```
  Widget Triggers (events/state)
                ↓
           Animation Graph
                ↓
       TimelineInstance { t, playing, direction }
                ↓
      Keyframe Evaluator (CPU)
                ↓
    Extracted GPU Params (color, opacity, offset…)
                ↓
              Renderer
                ↓
               Frame
```

---

## **4. Core Data Model**

### **4.1 Timeline**

```
pub struct Timeline {
    pub duration: f32,
    pub tracks: Vec<Track>,
}
```

### **4.2 Track Types**

```
pub enum TrackValue {
    Float,
    Vec2,
    Vec3,
    Color,
    Transform2D,
    EffectIntensity,
}
```

### **4.3 Keyframes**

```
pub struct Keyframe<T> {
    pub time: f32,     // 0..duration
    pub value: T,
    pub curve: Easing,
}
```

### **4.4 Easing Curves**

```
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },
}
```

### **4.5 Track**

```
pub struct Track {
    pub property: AnimProperty,
    pub keyframes: Vec<Keyframe<AnimValue>>,
}
```

### **4.6 AnimProperty**

Defines **what** is animated:

```
pub enum AnimProperty {
    Opacity,
    Color,
    Position,
    Scale,
    Rotation,
    ShadowOffset,
    GlowRadius,
    TextColor,
    OutlineThickness,
}
```

### **4.7 Runtime Instance**

```
pub struct TimelineInstance {
    pub time: f32,
    pub playing: bool,
    pub direction: AnimDirection,
}
```

```
pub enum AnimDirection { Forward, Reverse }
```

---

## **5. Evaluation Logic**

### **5.1 Step 1 — Advance Time**

```
if playing:
    time += dt * speed * sign(direction)
```

### **5.2 Step 2 — Clamp or Loop**

Modes:

* Clamp
* Loop
* Ping-Pong

### **5.3 Step 3 — Evaluate Each Track**

Find keyframes `kf1` and `kf2` around current time.

Compute:

```
t_norm = (time - kf1.time) / (kf2.time - kf1.time)
p = apply_easing(kf2.curve, t_norm)
value = lerp(kf1.value, kf2.value, p)
```

### **5.4 Step 4 — Write To Node**

Examples:

* Position → node.layout.position_offset
* Scale → renderer instance scale
* Color → effect system
* OutlineThickness → text compositor
* ShadowOffset → text compositor

---

## **6. Animation Graphs**

### **6.1 Purpose**

Widgets must express animation logic in **state-machine form**:

```
Idle → Hover → Pressed → Released → Idle
```

### **6.2 Structure**

```
pub struct AnimationGraph {
    pub nodes: Vec<StateNode>,
    pub transitions: Vec<Transition>,
}
```

### **6.3 StateNode**

```
pub struct StateNode {
    pub id: StateId,
    pub timeline: Timeline,
}
```

### **6.4 Transition**

```
pub struct Transition {
    pub from: StateId,
    pub to: StateId,
    pub trigger: Trigger,
    pub direction: AnimDirection,
}
```

### **6.5 Trigger Types**

```
HoverEnter
HoverLeave
FocusGained
FocusLost
ClickDown
ClickUp
ValueChanged
Custom(&'static str)
```

---

## **7. Widget Integration**

### **7.1 Example: Button**

States:

* Idle
* Hover
* Pressed

Animations:

* Hover: opacity fade + scale up
* Pressed: scale down + glow

### **7.2 Example API**

```
button.animate(|a| {
    a.state("idle")
        .timeline(t! {
            opacity: 1.0 => 1.0;
            scale: 1.0 => 1.0;
        });

    a.state("hover")
        .timeline(t! {
            opacity: 1.0 => 1.0;
            scale: 1.0 => 1.05 @ EaseOut;
        });

    a.transition("idle", "hover", Trigger::HoverEnter);
    a.transition("hover", "idle", Trigger::HoverLeave);
});
```

---

## **8. Renderer Integration**

### **8.1 GPU Instance Data**

Each frame, widgets write:

```
instance.opacity
instance.color
instance.scale
instance.offset
instance.rotation
instance.effect_params
```

### **8.2 Text Node Integration**

Text inherits all base transforms plus effect animations.

070 + 068 + 069 unify text motion + appearance + FX.

---

## **9. Performance Considerations**

### **9.1 CPU Costs**

Per frame:

* Δt processing O(N timelines)
* Keyframe lookup (binary search)
* Lerp + curve evaluation

### **9.2 GPU Costs**

Zero extra cost except writing updated instance buffers.

### **9.3 Memory**

Typical UI: < 200 timelines, trivial.

---

## **10. Testing Matrix**

### **10.1 Determinism**

* Same input → same frame results
* No floating drift

### **10.2 Performance**

* 10,000 active keyframes → stable 60 FPS

### **10.3 Behavioral**

* Interrupting an animation
* Reversing direction
* Rapid state switching

### **10.4 Widget-Level**

* Button hover smooth
* Modal fade
* Focus transitions
* Notice pop animations

---

## **11. Implementation Plan**

### **Phase 1 — Core Engine**

* Timeline struct
* Track / Keyframe
* Evaluator
* Runtime instance

### **Phase 2 — Graph Engine**

* Graph nodes
* Transitions
* Triggers

### **Phase 3 — Widget Integration Layer**

* Mapping widget states → timeline

### **Phase 4 — Renderer Integration**

* Write transforms into instance buffers
* Integrate color/opacity

### **Phase 5 — Text Compositor Integration**

* Outline thickness
* Glow radius
* Shadow offsets

### **Phase 6 — Examples & Tests**

* Button
* Modal
* TextField focus
* Toast slide-in
* Menu transitions

---

## **12. Implementation Notes**

### **12.1 Stability**

* All times stored as f32
* No cumulative error
* No inconsistent state transitions

### **12.2 Future Work**

SPEC-HAL-071:

* **Declarative animation language**
* Visual timeline editor (future IDE)
* Multi-track hierarchical sequences
* Shared timebases and synced scenes
