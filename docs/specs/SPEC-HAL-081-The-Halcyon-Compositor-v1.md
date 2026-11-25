# **SPEC-HAL-081 — The Halcyon Compositor v1 (Surfaces, Layers, Overlays, Modal System)**

---

## STATUS: DEPRECATED (historical reference only)

All active contracts are defined in:
- **SPEC-HAL-081A** — Phase-1 Enforcement (frozen)
- **SPEC-HAL-081B** — Multi-Surface Compositor (batching, renderer, scene_key)
- **SPEC-HAL-081C** — Overlay Surface Integration (roles, provenance, lifetime)

This document is retained for historical context and design archaeology only.
Do NOT cite this SPEC for invariants, rules, or guarantees.

---

**SPEC-HAL-081**
**Halcyon Compositor v1 — Surfaces, Layers, Overlays, Modal System**
**Author:** Halcyon Architect
**Status:** DEPRECATED — Historical Reference Only
**Version:** 1.0

**Superseded By:**
- SPEC-HAL-081A (Phase-1 Enforcement)
- SPEC-HAL-081B (Multi-Surface Compositor)
- SPEC-HAL-081C (Overlay Surface Integration)

**Original Dependencies:**

* SPEC-HAL-005 Scene Graph
* SPEC-HAL-009 Event Routing v2
* SPEC-HAL-024 Unified Coordinate Model
* SPEC-HAL-033 Input Fusion Layer
* SPEC-HAL-036 Input Coalescing
* SPEC-HAL-074 Style Runtime
* SPEC-HAL-075 Layout Stability Guarantees
* SPEC-HAL-076 StyleCache v2
* SPEC-HAL-077 Style Animations
* SPEC-HAL-078 Layout Transitions
* SPEC-HAL-079 Unified Event Flow v3
* SPEC-HAL-080 Interaction Engine (HIE v1)
* SPEC-HAL-041 Unified Timing & Animation Clock

---

## Implementation Status

**DEPRECATED** — This SPEC is superseded by 081A/081B/081C.

The original v1 vision is preserved below for historical reference.

---

**Implements:**
A dedicated compositor subsystem responsible for **layering, surfaces, popups, modals, overlays, dialogs, toasts, and window-relative UI**, replacing ad-hoc layering logic with a formal multi-surface model.

---

# **1. Motivation**

Until now Halcyon has:

* a scene graph
* a layout engine
* a renderer
* an event router

…but no explicit **compositing model**.

As a result:

* popups are hacks
* modals break hit-testing
* dropdowns need manual z-index work
* overlays must fight layout
* tooltips sit in undefined coordinate space
* input routing becomes ambiguous
* focus navigation is broken in layered UI

Modern UI demands a compositor, period.

---

# **2. Compositor Responsibilities**

The Halcyon Compositor defines:

1. **Surfaces**
   Independent rendering/interaction spaces.

2. **Layers**
   Ordered collections of surfaces.

3. **Overlays**
   Surfaces that float above the main tree.

4. **Modals**
   Surfaces that block interaction behind them.

5. **Popups/Toasts/Tooltips**
   Surface categories with automatic positioning.

6. **Coordinate transforms**
   Parent surface → child surface → screen.

7. **Hit-testing across layers**
   Highest-visible wins.

8. **Render ordering**
   Layers control draw order explicitly.

9. **Focus isolation**
   Modal surfaces get their own focus scope.

10. **Adaptive layout** within surface bounds.

This formalizes everything UI frameworks normally botch.

---

# **3. Surface Model**

A **Surface** is a root of a widget subtree.

```
struct Surface {
    id: SurfaceId,
    root_widget: WidgetId,
    bounds: RectF,           // in absolute logical space
    kind: SurfaceKind,
    z_index: i32,
    focus_scope: FocusScope,
}
```

### **SurfaceKind:**

```
Main               // primary app surface
Popup              // dropdowns, context menus
Tooltip            // hover hints
Modal              // blocks lower surfaces
Overlay            // overlays above everything
Toast              // ephemeral notifications
WindowLocal        // sub-window or multi-window
CompositedIframe   // future-proof
```

---

# **4. Layer Model**

Surfaces belong to **Layers**.

Example default stack:

```
Layer 0 — Main UI
Layer 1 — Popup Layer
Layer 2 — Tooltip Layer
Layer 3 — Modal Layer
Layer 4 — Overlay Layer
Layer 5 — Toast Layer
```

Each layer:

* maintains its own surface list
* defines strict z-order
* is rendered front-to-back

A surface with higher layer always wins hit-tests.

---

# **5. Coordinate System (Critical)**

All surfaces use **absolute logical space** (SPEC-024).

Coordinate transforms:

```
Widget local → Surface → Global (absolute logical) → Physical
```

Surface boundaries define clipping.

Popup/overlay positioning uses:

```
anchor_rect (absolute logical)
```

Transform simplicity is key:
**No surface has its own “world”; surfaces are just clipped regions in global space.**

---

# **6. Hit Testing Across Surfaces**

Hit test order:

1. iterate layers from highest to lowest
2. for each surface:

    * check bounds
    * run surface-level hit test into its widget tree
3. first non-empty hit wins

Modals additionally:

* consume hits to lower layers
* block pointer, wheel, keyboard, gestures behind them

Focus also respects layer priority.

---

# **7. Event Routing Across Surfaces**

Unified Event Flow (SPEC-079) extended:

```
OS Input
 ↓
Surface Resolution (topmost)
 ↓
Surface Hit Test
 ↓
Widget Hit Test
 ↓
Interaction Engine (HIE)
 ↓
State → Style → Layout → Render
```

If the input is inside a modal surface:

* it stays inside modal until modal closes
* all other surfaces are non-interactive

If inside popup:

* popup captures until dismissal
* outside click dismisses (if configured)

If inside tooltip:

* tooltip is passive; does not receive events
* does not block others

---

# **8. Modal System**

A modal surface:

* blocks interaction behind it
* has a mandatory dimming backdrop
* has isolated focus scope
* traps keyboard navigation
* cannot be bypassed by scroll or pointer events

API:

```
spawn_modal(surface_id, modal_params)
close_modal(surface_id)
```

---

# **9. Popups & Dropdowns**

A popup surface:

* anchored to a widget rect
* auto-adjusts position based on available screen space
* follows the anchor rect on layout changes

Popup lifetime:

```
open_popup(surface_id)
auto_dismiss_on_click_outside()
auto_dismiss_on_scroll_outside()
```

---

# **10. Tooltips**

Tooltip surfaces:

* no focus
* no interaction
* follow pointer
* style-driven delay
* fade-in/out via animation clock

Triggered via HIE semantic event:
`SemanticEvent::HoverStart` / `HoverEnd`.

---

# **11. Toasts**

Ephemeral overlay surfaces:

* stacked vertically
* auto-dismiss timers
* animations on enter/exit
* cannot receive input

---

# **12. Render Graph Integration**

Each surface defines its own **RenderNode**.

The compositor produces:

```
[Layer 0 surfaces]
↓
[Layer 1 surfaces]
↓
...
↓
[Layer N surfaces]
```

Renderer processes in order:

```
for layer in layers:
    for surface in layer:
        render(surface.render_graph)
```

The GPU sees composited output as a flat list of passes.

---

# **13. Focus System Integration**

Focus is scoped by surface:

* main layer has global scope
* popup gets local scope
* modal gets isolated scope
* tooltips/toasts ignore focus

Focus traversal respects:

```
surface priority > z_index > widget order
```

Keyboard navigation never escapes modal surface.

---

# **14. Layout Integration**

Each surface runs layout in its own clipping region:

```
layout(surface.root_widget, constraints=surface.bounds)
```

Popups re-layout on:

* anchor rect changes
* window resize
* font-size changes
* style changes

Modals and overlays do not shift main layout; they float independently.

---

# **15. Style Runtime Integration**

Style applies per-surface:

```
resolve_style(surface.root)
```

Animations and transitions (SPEC-077/078) run independently per-surface.

---

# **16. Testing Matrix**

### **Surface tests**

* bounds clipping
* coordinate correctness
* anchor positioning

### **Layer tests**

* hit-testing order
* z-order overrides

### **Modal tests**

* input blocking
* focus confinement
* dismissal

### **Popup tests**

* anchor stability
* auto-dismiss rules
* scroll/resize behavior

### **Overlay tests**

* transparency
* pointer pass-through
* renderer stacking

### **Tooltip tests**

* hover lifetime
* pointer tracking
* fade animations

---

# **17. Implementation Plan**

### **Phase 1 — Core: Surfaces + Layers**

* Surface type
* Layer registry
* coordinate transforms

### **Phase 2 — Event Routing**

* cross-surface hit test
* modal capture rules

### **Phase 3 — Rendering**

* surface-to-render graph builder
* per-layer draw ordering

### **Phase 4 — Focus System**

* focus scopes
* modal focus trap

### **Phase 5 — Popups**

* anchor rect tracking
* positioning logic
* dismiss rules

### **Phase 6 — Modals**

* backdrop
* blocking rules
* close logic

### **Phase 7 — Tooltips + Toasts**

* timers
* animations
* passive behavior

### **Phase 8 — Integration**

* full pipeline through HIE
* style + layout sync
* timing & animation clock
