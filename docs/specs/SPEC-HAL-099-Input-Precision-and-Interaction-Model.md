# **SPEC-HAL-099 — Input Precision & Unified Interaction Model**

**SPEC-HAL-099**
**Input Precision & Unified Interaction Model**
**Author:** Halcyon Architect
**Status:** PROPOSED
**Version:** 1.0

**Depends On:**

* SPEC-HAL-009 — Event Routing & Hit Testing
* SPEC-HAL-014 — Advanced Input
* SPEC-HAL-033 — Input Fusion Layer
* SPEC-HAL-038 — Focus Model
* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-090 — Logical Grid & Compact Layout Rules
* SPEC-HAL-097 — Motion & Micro-Animation Rules
* SPEC-HAL-098 — Cursor & Pointer System

---

## **Implementation Status**

*Not implemented.*

This SPEC defines:

* input precision rules
* pointer-down vs click semantics
* unified press/hold/drag model
* hit slop rules (strict, minimal)
* micro delays (the allowed ones)
* double-click definition
* unified selection logic for lists/trees
* button/toggle/checkbox behavior
* drag activation thresholds
* safe cancellation rules
* state machine for interactive widgets

Halcyon interaction model must be:

* deterministic
* unambiguous
* low-latency
* pixel-precise
* stable at high refresh rates
* identical across OSes

If user input interacts with Halcyon, it must feel **sharp**, **direct**, **professional**, **zero-fat**.

---

# **1. Philosophy**

Halcyon does **not** copy Web UX or mobile metaphors.
It follows desktop-grade engineering UX:

* immediate response
* no click delays
* zero ambiguity in press/move/drag
* consistent state machines
* minimal hit slop
* pixel-precise drag initiation

Input in Halcyon always prioritizes:

**accuracy > “friendliness”**
**determinism > animation**
**predictability > clever heuristics**

---

# **2. Hit Testing Precision**

Hit testing runs at layout time and has hard rules.

### **2.1 Hit Slop**

Halcyon avoids generous touch-like hit slop.

Defaults:

```
hit_slop_horizontal = 1 px
hit_slop_vertical   = 1 px
```

Compact UI:

```
0 px
```

Widgets for accessibility may explicitly opt in to larger zones, but never by default.

### **2.2 Overlapping Zones**

If two widgets overlap:

1. topmost widget wins
2. pointer-capture rules override (see below)

---

# **3. Pointer Down → Click Semantics**

Halcyon defines “click” as:

```
pointer_down inside widget bounds
pointer_up inside widget bounds
no drag threshold exceeded
no cancellation event
```

### **3.1 Pointer Down**

Executed immediately:

* widget enters “pressed” state
* motion rules from SPEC-HAL-097 apply
* no delay allowed

### **3.2 Pointer Up**

If pointer is still inside bounds:
→ Click fires.

If pointer left bounds for more than 32 ms:
→ Press is cancelled.

### **3.3 Cancellation Rules**

Press cancels when:

* ESC pressed
* pointer moves outside by > drag threshold
* widget becomes disabled
* modal overlay appears
* focus changes due to system events

Cancellation must:

* be immediate
* reset visual state
* never falsely emit click

---

# **4. Drag Threshold**

To distinguish click vs drag:

```
drag_threshold = 3 logical px
compact: 2 logical px
```

Drag begins when pointer displacement exceeds this threshold *in any direction*.

### Rules:

* threshold must not be DPI-scaled
* pointer-down coordinate stored in physical pixels
* if threshold crossed → click cancelled → drag start

Drag must feel deterministic, not heuristic.

---

# **5. Double-Click Semantics**

Double-click is defined strictly:

```
time_delta ≤ 250 ms
distance ≤ 4 px
```

If either fails → counts as two single clicks.

Double-click actions:

* list item “open”
* tree item “expand/collapse” (optional)
* text selection word/sentence

No triple-click logic is included in this SPEC.

---

# **6. Long Press (Hold) Rules**

Halcyon rarely uses long-press in desktop context; if used:

```
hold_delay = 420 ms
hold_move_tolerance = 2 px
```

Hold triggers before drag, unless drag threshold reached.
Hold is only used for:

* alternative context menus
* inspector multi-value editing
* extended-range sliders

---

# **7. Pointer Capture Model**

Pointer capture allows widgets to “own” the pointer during interactions.

### Capture begins:

* on drag start
* on resize start
* on scrollbar grab
* on slider grab
* on text selection drag

### Capture ends:

* on pointer up
* on cancellation
* on window focus loss

Captured widget receives all pointer events regardless of pointer position.

---

# **8. Unified Button Interaction Model**

This applies to all:

* push buttons
* toggle buttons
* icon-only buttons
* segmented controls
* toolbar buttons

### Button States:

```
idle → hover → pressed → active/selected → disabled
```

### Rules:

* pointer-down sets “pressed” immediately
* pointer-up inside bounds emits click
* pointer-up outside cancels
* pressed visual must react within ≤ 20 ms
* hover must not introduce layout shift

Toggle buttons:

```
click toggles state after release
```

Icon-only buttons must follow identical rules without exceptions.

---

# **9. List & Tree Interaction Model**

Halcyon lists and trees use deterministic selection behavior.

### **9.1 Single Click**

* selects row
* moves focus to list
* does **not** trigger edit/open

### **9.2 Double Click**

* activates row (open)
* optional expand/collapse in trees

### **9.3 Modifier Keys**

* Shift-click → range selection
* Ctrl-click (Cmd on macOS) → toggle individual row

### **9.4 Hover**

Hover does not alter selection.
Hover only affects visual highlighting and tooltips.

### **9.5 Drag Select (if enabled)**

Rows become selected as the pointer passes through them.

---

# **10. Slider, Scrollbar, and Drag Controls**

All drag-based controls follow the drag threshold rules.

### **Scrollbar**

* click on trough → page jump
* drag on thumb → capture + continuous movement
* release → snap to final position

### **Slider**

* pointer-down on thumb → immediate capture
* drag → continuous update
* release → commit value

### **Split Views (Resizable Panels)**

* hit zone = resize zone from SPEC-HAL-098
* pointer-down → pressed
* drag → resize
* pointer-up → release

No inertia.
No smoothing.
No physics.

---

# **11. Text Field Interaction Rules**

Text fields obey:

* cursor positioning on pointer-down
* selection start on drag threshold exceed
* double-click → select word
* triple-click (optional future SPEC) → select line
* drag-outside → auto-scroll

Cursor movement must not lag behind pointer motion.

---

# **12. Keyboard Interaction Parity**

Pointer events must not interfere with keyboard-driven interactions.

Rules:

* keyboard focus remains independent
* pointer hover does not steal focus
* pointer click sets focus, not hover

Pointer and keyboard must share a unified state machine.

---

# **13. Declarative API**

Widgets expose interaction behaviors:

```
interact.click.enabled = true
interact.drag.threshold = 3px
interact.double_click.enabled = true
interact.hover.enabled = true
interact.long_press.enabled = false
interact.long_press.delay = 420ms
```

Lists:

```
list.selection.multiple = true
list.selection.range = true
list.selection.activate_on = double_click
```

Resizables:

```
resize.enabled = true
resize.zone = 6px
```

---

# **14. Implementation Plan**

### Phase 1 — Core State Machines

Implement unified interaction states (idle/hover/pressed/active).

### Phase 2 — Drag & Capture

Implement drag-threshold logic and pointer capture.

### Phase 3 — Click vs Drag Resolution

Implement robust down→up semantics.

### Phase 4 — List/Tree Interaction Module

Unified selection model.

### Phase 5 — Controls Integration

Buttons, sliders, scrollbars, split views, text fields.

### Phase 6 — Platform QA

Test against:

* macOS
* Windows
* Wayland
* X11

With various refresh rates and DPI settings.
