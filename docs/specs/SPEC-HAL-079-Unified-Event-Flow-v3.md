# **SPEC-HAL-079 — Unified Event Flow v3 (Predictable Pointer & Gesture System)**

**SPEC-HAL-079**
**Unified Event Flow v3 — Pointer, Gesture & Capture Model**
**Author:** Halcyon Architect
**Status:** Draft
**Version:** 1.0

**Depends On:**

* SPEC-HAL-009 Event Routing v2
* SPEC-HAL-033 Input Fusion Layer
* SPEC-HAL-036 Input Coalescing
* SPEC-HAL-074 Style Runtime
* SPEC-HAL-076 StyleCache v2
* SPEC-HAL-077 Style Animations
* SPEC-HAL-078 Layout Transitions
* SPEC-HAL-115 Unified Timing & Animation Clock

**Implements:**
A deterministic, Flutter-grade input system with **pointer identity**, **gesture recognizers**, **capture rules**, and **event phases** that remain stable under layout transitions, animations, and reflow.

---

# **1. Motivation**

Current Halcyon input routing is correct but primitive:

* limited capture semantics
* pointer IDs not persistent
* drag thresholds inconsistent
* gesture competitions not formalized
* no long-press / multi-tap / pan / pinch
* transitions & animations cause coordinate drift

079 introduces a modern event model:

* persistent pointer identity
* capture rules (soft/hard)
* native gesture recognizers
* correct phase ordering
* position prediction
* animation-aware hit testing
* multi-pointer support
* scroll inertia + axis-lock

All without compromising determinism.

---

# **2. Pointer Identity Model**

Every input contact becomes a **PointerId**:

```
PointerId(u64)
```

Assignment rules:

1. Unique per contact
2. Persistent until contact ends
3. Reused sequentially when safe
4. Independent of OS pointer indices
5. Multi-touch supported

This lets widgets reliably track drags, pans, presses.

---

# **3. Event Phases (Final)**

Halcyon adopts a **four-phase pipeline**:

```
1. Pre-hit-test    (global filters)
2. Hit-test        (tree walk)
3. Target routing  (capture, bubble/tunnel)
4. Post-process    (gesture recognizers)
```

### **3.1 Pre-hit-test**

Used for:

* input coalescing
* prediction
* velocity calculation
* DPI / scale correction
* dead-zone filtering
* compositor transforms

### **3.2 Hit-test**

Uses **target rect** from layout — **never interpolated** rectangles.

Supports:

* absolute logical space
* correct Z-order
* independent of layout transitions

### **3.3 Routing Phases**

```
Tunnel → Target → Bubble
```

Fully structured, consistent.

### **3.4 Post-process**

Gesture arena determines winners/losers.

---

# **4. Capture Model v3**

Two forms:

### **4.1 Soft Capture (“tentative”)**

Widget receives events first **if it continues to intersect pointer**.

Lost automatically when pointer leaves bounds.

Used for:

* buttons
* tabs
* hover interactions

### **4.2 Hard Capture (“exclusive”)**

Widget receives **all events**, regardless of pointer position.

Released only by:

* explicit release
* gesture win/loss
* pointer up

Used for:

* sliders
* drags
* text selection handles
* custom tools

### **4.3 Capture Priority**

Widgets may request capture simultaneously.

Priority rules:

1. Hard > Soft
2. Descendant > Ancestor
3. First-come, first-served

---

# **5. Gesture Arena Model**

Halcyon introduces a **gesture competition system** inspired by Flutter’s arena:

### **5.1 Recognizer Types**

```
TapRecognizer
DoubleTapRecognizer
LongPressRecognizer
PanRecognizer
HorizontalPanRecognizer
VerticalPanRecognizer
PinchRecognizer
ScrollRecognizer
FlingRecognizer
```

### **5.2 Arena Rules**

1. All recognizers receive events initially
2. Recognizers may enter states:

    * Possible
    * Accepted
    * Rejected
3. Once a recognizer **accepts**, others must reject
4. If multiple recognizers accept at same frame:

    * Spatial priority rules apply
    * If still tied → parent wins

This provides deterministic gesture resolution.

---

# **6. Motion Model**

### **6.1 Coalesced + Predicted Positions**

From SPEC-036:

```
PointerMove {
    raw: (x_raw, y_raw),
    coalesced: Vec<(x,y)>,
    predicted: Option<(x_pred,y_pred)>
}
```

079 consumes both coalesced and predicted positions.

### **6.2 Velocity Tracking**

Per pointer:

* exponential smoothing
* last N positions
* used for fling detection

### **6.3 Scroll Model**

Includes:

* inertia
* overscroll resistance
* axis-lock (auto or explicit)
* nested scroll propagation rules

---

# **7. Integration With Layout Transitions (SPEC-078)**

Critical rule:

### **Hit testing ALWAYS uses static layout geometry.**

Never interpolated.

### **Pointer movement visual effects use interpolated geometry.**

This ensures:

* no hit drift
* no pointer mismatch
* smooth visual transitions

---

# **8. Event Struct Definitions**

### **8.1 Pointer Events**

```
PointerDown { id, position, screen_position, time, button }
PointerMove { id, position, delta, velocity, predicted, time }
PointerUp   { id, position, time }
PointerCancel { id }
PointerHover { id, position }
PointerScroll { id, delta, phase }
PointerWheel { lines, amount, time }
```

### **8.2 Gesture Outputs**

```
Tap { id }
DoubleTap { id }
LongPress { id }
PanStart { id, delta }
PanUpdate { id, delta }
PanEnd { id, velocity }
PinchStart { center, scale }
PinchUpdate { center, scale }
PinchEnd { velocity }
ScrollStart
ScrollUpdate
ScrollEnd
Fling { velocity }
```

---

# **9. Multi-Pointer & Multi-Modal Support**

079 fully supports:

* multi-touch
* pen + touch simultaneous
* mouse + touch
* stylus tilt/pressure (future extension)

Inputs share the unified identity model.

---

# **10. Testing Matrix**

### **Pointer Tests**

* persistent ID
* multiple pointers
* lift + re-down
* capture transitions
* arena winner/loser arbitration

### **Gesture Tests**

* tap → pan conflicts
* horizontal vs vertical pan
* scroll vs child pan
* pinch + pan simultaneous
* long-press suppression

### **Layout Interaction**

* hit-test reliability during layout transitions
* scroll offset + pointer movement correctness
* animation-aware target routing

---

# **11. Implementation Plan**

### **Phase 1 — Pointer Identity + Event Flow**

* new PointerId allocator
* Pre-hit-test → Hit-test → Target → Post phases

### **Phase 2 — Capture Model v3**

* hard/soft capture
* release rules
* parent/child arbitration

### **Phase 3 — Gesture Arena**

* recognizer states
* arena arbitration logic

### **Phase 4 — Gesture Library**

* tap
* double-tap
* pan
* pinch
* scroll

### **Phase 5 — Integration**

* Layout transitions
* Scene graph
* Animations
* Scroll containers

### **Phase 6 — Optimization**

* SIMD velocity calculations
* arena branch elimination
* pointer ringbuffer
