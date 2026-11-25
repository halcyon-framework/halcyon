# **SPEC-HAL-077 — Style Animations v1 (Time-Driven Dynamic Style Layers)**

**SPEC-HAL-077**
**Style Animations v1 — Time-Based Interpolation Layer**
**Author:** Halcyon Architect
**Status:** Draft
**Version:** 1.0

**Depends On:**

* SPEC-HAL-072 Declarative StyleSheets
* SPEC-HAL-073 Composition & Inheritance
* SPEC-HAL-074 Style Runtime Engine
* SPEC-HAL-075 SIMD Merge Engine
* SPEC-HAL-076 StyleCache v2
* SPEC-HAL-046 Widget Model v2
* SPEC-HAL-041 Unified Timing & Animation Clock

**Implements:**
A **low-cost, cache-friendly animation system** built directly into the style pipeline, with zero per-widget heap allocations and zero virtual dispatch.
Adds a new “Animated Layer” on top of the merged StyleSnapshot.

---

# **1. Motivation**

Halcyon styles are fully static today.
But modern UI requires:

* smooth change of colors
* hover/focus transitions
* fade-in/out
* size transitions
* animated layout (if permitted)
* theme transition animations
* timed effects

Animations must:

* be **zero-cost when idle**
* work with **hundreds of thousands of widgets**
* integrate with **StyleCache v2**
* avoid invalidating the whole UI tree
* not trigger unnecessary relayout

---

# **2. Core Concept: AnimatedStyleLayer**

Animations do **not** mutate the StyleSnapshot.
They overlay a **computed delta** on top of it.

```
FinalStyle = BaseSnapshot ⊕ AnimatedLayer
```

The AnimatedLayer stores:

```
struct AnimatedLayer {
    color: Option<AnimatedColor>,
    opacity: Option<AnimatedFloat>,
    transform: Option<AnimatedTransform>,
    size: Option<AnimatedFloat>,  // optional layout-affecting
    started_at: TimePoint,
    duration: f32,
    easing: EasingCurve,
    repeat: RepeatMode,
}
```

Zero allocations.
Stored inline in widget state.
Absent = animation off.

---

# **3. Property Model**

v1 supports the most common UI properties.

### **3.1 Colors**

```
AnimatedColor {
    from: RGBA,
    to: RGBA,
}
```

Interpolated in linear color space.

### **3.2 Opacity**

```
AnimatedFloat { from: f32, to: f32 }
```

### **3.3 Transform**

```
AnimatedTransform {
    from: Transform2D,
    to: Transform2D,
}
```

### **3.4 Size (Optional)**

Opt-in:
Animated size triggers layout dirty.

---

# **4. Animation Engine**

A pure function:

```
fn evaluate_anim_layer(
    anim: &AnimatedLayer,
    base: &StyleSnapshot,
    now: TimePoint,
) -> StyleSnapshot
```

Internally:

1. Compute `t = clamp((now - started_at) / duration)`
2. Apply easing curve
3. Interpolate each animated property
4. Overlay onto base snapshot

### **4.1 No Caching**

Animated values change per frame → no caching.

### **4.2 No Allocations**

Everything is stack local.

---

# **5. Widget State Extension**

Each widget gets:

```
struct WidgetStyleState {
    base_snapshot: Arc<StyleSnapshot>,
    animated_layer: Option<AnimatedLayer>,
    final_snapshot: StyleSnapshot,  // cached for this frame
}
```

`final_snapshot` reused between layout & paint in same frame.

---

# **6. Dirty Propagation**

### **6.1 Style Dirty**

Triggered when an animation attaches or ends.

### **6.2 Layout Dirty**

Only if animated properties affect layout.

### **6.3 Per-frame Dirty**

If an animation is running, the widget must repaint each frame.

Enumerator:

```
enum WidgetFrameState {
    Idle,
    NeedsRepaint,
    NeedsRelayout,
}
```

---

# **7. Declarative Syntax (SPEC-072 Integration)**

Stylesheet example:

```
Button {
    background: #222;
    color: white;

    on:hover {
        animate background 150ms ease-out;
        animate color 120ms ease-out;
    }

    on:pressed {
        animate transform 80ms ease-in;
        animate opacity 60ms linear;
    }
}
```

Compiler expands these into `AnimatedLayer` specs.

---

# **8. Easing Functions**

Built-in:

* Linear
* EaseIn
* EaseOut
* EaseInOut
* CubicBezier(x1, y1, x2, y2)
* Overshoot
* Bounce (optional)

Stored as:

```
enum EasingCurve { … }
```

---

# **9. Repeat Modes**

```
enum RepeatMode {
    Once,
    Loop,
    PingPong,
}
```

PingPong uses mirrored t.

---

# **10. Theme Switching Integration**

When theme_version changes:

* base_snapshot is invalidated
* animation continues, but now interpolates to new base
* final_snapshot recreated cleanly
* no visual jumps

This produces fluid theme transitions.

---

# **11. Rendering Pipeline Integration**

Renderer sees only:

```
final_snapshot
```

No knowledge of animation logic.

Animations never require renderer changes.

---

# **12. Testing Matrix**

### Unit Tests

* Color interpolation
* Opacity interpolation
* Easing curves correctness
* Transform lerp
* Repeat modes
* Loop vs ping-pong

### Integration Tests

* Hover fade
* Pressed scale
* Animated theme change
* Multiple overlapping animations
* No layout reflow unless needed

### Stress Tests

* 100k animated opacity nodes
* 10k color transitions simultaneously
* Rapid variant toggling

---

# **13. Implementation Plan**

### **Phase 1** — AnimatedLayer structs

Color, opacity, transform, size.

### **Phase 2** — Interpolation engine

Implement easing + lerp routines.

### **Phase 3** — WidgetState extension

Store base, anim-layer, final snapshot.

### **Phase 4** — StyleEngine integration

Overlay animated results on snapshot.

### **Phase 5** — Dirty bits propagation

Repaint-only or relayout where required.

### **Phase 6** — Declarative syntax

Compile `animate PROP DURATION EASING`.

### **Phase 7** — Benchmark

Target 100k animations @ 60fps.
