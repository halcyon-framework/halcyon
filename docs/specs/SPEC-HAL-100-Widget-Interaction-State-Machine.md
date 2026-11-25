# **SPEC-HAL-100 — Unified Widget Interaction State Machine**

**SPEC-HAL-100**
**Unified Widget Interaction State Machine**
**Author:** Halcyon Architect
**Status:** PROPOSED
**Version:** 1.0

**Depends On:**

* SPEC-HAL-009 — Event Routing & Hit Testing
* SPEC-HAL-031 — Reactive Signals
* SPEC-HAL-033 — Input Fusion Layer
* SPEC-HAL-038 — Focus Model
* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-097 — Motion & Micro-Animation Rules
* SPEC-HAL-098 — Cursor & Pointer System
* SPEC-HAL-099 — Input Precision & Interaction Model

---

## **Implementation Status**

*Not implemented.*

This SPEC defines the **canonical interaction state machine (ISM)** for all widgets in Halcyon.
It standardizes transitions between:

* Idle
* Hover
* Pressed
* Active (for toggles or open states)
* Focused
* Disabled

This is the internal engine-level model, not a suggestion.
Every interactive widget must obey this state machine to guarantee consistency across the entire UI.

---

# **1. Motivation**

Modern UI frameworks often allow each widget to define its own interaction logic.
This leads to:

* inconsistent hover/press timing
* mismatched visual cues
* unpredictable focus behavior
* drag-start ambiguity
* divergent “active” semantics
* subtle bugs in high-DPI / high-Hz environments

Halcyon avoids this entirely by implementing **a universal interaction state engine**.

This yields:

* coherent behavior
* predictable motion
* reusable animation logic
* simpler styling
* fewer widget-level edge cases
* cleaner event routing

The ISM becomes the backbone of Halcyon’s interaction architecture.

---

# **2. The Canonical States**

Each widget can be in exactly one of the following mutually exclusive states:

### **IDLE**

No interaction. No hover. No press.

### **HOVER**

Pointer entered widget bounds.

### **PRESSED**

Pointer is down inside widget bounds and drag threshold not exceeded.

### **ACTIVE**

The widget toggled “on” (for toggle buttons, checkboxes, etc.)
or “open” (for popovers, dropdowns).

### **FOCUSED**

Widget has keyboard focus.

### **DISABLED**

Widget cannot be interacted with.

**Note:** Active and Focused may overlap, but all others are exclusive.

---

# **3. State Transition Diagram**

A simplified representation:

```
IDLE → HOVER → PRESSED → (click) → IDLE
             ↘ (drag threshold reached) → DRAG_CAPTURE (ends outside this SPEC)
```

Toggle widget:

```
IDLE → HOVER → PRESSED → ACTIVE → IDLE
```

Focus interaction:

```
IDLE → FOCUSED → IDLE
```

Disabled overrides all:

```
any_state → DISABLED
```

Re-enabled:

```
DISABLED → IDLE
```

The full graph is deterministic and contains no ambiguous edges.

---

# **4. State Transition Rules**

### **4.1 Entry/Exit Conditions**

**IDLE → HOVER**

* pointer enters bounds
* widget is not disabled

**HOVER → IDLE**

* pointer leaves bounds
* widget becomes disabled

**HOVER → PRESSED**

* pointer-down inside bounds

**PRESSED → IDLE**

* pointer-up outside bounds
* cancellation event

**PRESSED → ACTIVE**

* pointer-up inside bounds
* widget is toggleable

**PRESSED → DRAG_CAPTURE**

* pointer movement exceeds drag threshold (SPEC-HAL-099)

**ACTIVE → IDLE**

* pointer-down + pointer-up cycle
* or external deactivation

**IDLE/HOVER/PRESSED/ACTIVE → DISABLED**

* widget disabled
* all transitions cancel

**Any → FOCUSED**

* widget receives keyboard focus

**FOCUSED → IDLE/HOVER/etc.**

* widget loses keyboard focus
* new state determined by pointer position

---

# **5. Visual Responses per State**

Each state produces predictable styling hooks.

### **IDLE**

* default styling
* no motion

### **HOVER**

* use hover motion rules from SPEC-HAL-097
* opacity +5–8%
* border tone shift
* optional minimal shadow

### **PRESSED**

* pressed surface darken (8–12%)
* optical offset 1–2 px downward
* cursor stays arrow unless widget is draggable

### **ACTIVE**

* persistent active background
* accent-tinted icon/text
* optional slight elevation

### **FOCUSED**

* focus ring (border)
* no layout shift
* no hover override

### **DISABLED**

* opacity reduced
* muted text/icon
* cursor = default arrow
* all interactions suppressed

---

# **6. Event Routing Integration**

ISM integrates directly with event routing (SPEC-HAL-009):

### Pointer Events

* pointer-enter → HOVER
* pointer-leave → IDLE
* pointer-down → PRESSED
* pointer-up → IDLE or ACTIVE
* pointer-move → drag transition check

### Keyboard Events

* TAB / Shift+TAB → FOCUSED transition
* ENTER / SPACE → emulate click in focused state
* ESC → cancel PRESSED

### Focus Engine

Focus cannot override PRESSED or DRAG_CAPTURE but can override HOVER.
Focus is independent of hover.

---

# **7. Declarative Style Integration**

Widgets expose state-specific style tokens:

```
style.hover.background
style.hover.border
style.hover.shadow

style.pressed.background
style.pressed.offset_y

style.active.background
style.active.border

style.focus.border
style.focus.glow

style.disabled.opacity
style.disabled.cursor
```

The ISM determines which set applies.

---

# **8. State Machine for Common Widgets**

## **Buttons**

* click = PRESSED → IDLE
* toggle = PRESSED → ACTIVE

## **Checkboxes / Switches**

* click toggles ACTIVE
* hover does not modify check state

## **List Items**

* click selects (not active)
* double-click activates

## **Tree Items**

* click selects
* double-click expands/collapses
* active state not used here

## **Popovers / Menus**

* ACTIVE = open
* click outside or ESC = IDLE

## **Text Fields**

* FOCUSED shows cursor
* PRESSED positions caret
* ACTIVE not used

---

# **9. Timing Requirements**

State transitions must meet the following performance thresholds:

* Visual response must begin ≤ **20 ms** after state change
* Hover detection must update every frame
* Press transitions must be instantaneous
* Active state changes must not animate background scale
* No transition may drop frames at 144–240 Hz

---

# **10. Implementation Plan**

### Phase 1 — ISM Core

Implement the canonical state machine struct:

```
enum InteractionState { Idle, Hover, Pressed, Active, Focused, Disabled }
```

Transition reducer:

```
fn reduce(current, event) -> next_state
```

### Phase 2 — Widget Integration Layer

Add ISM to all interactive widget base classes.

### Phase 3 — Backend Wiring

Link ISM with:

* event routing
* pointer system
* motion engine
* focus engine
* style system

### Phase 4 — Visual Feedback Mapping

Map each state to motion + styling tokens.

### Phase 5 — QA

Test:

* click precision
* drag start jitter
* hover stability
* active toggles
* cross-DPI correctness
* high-Hz pointer stability
