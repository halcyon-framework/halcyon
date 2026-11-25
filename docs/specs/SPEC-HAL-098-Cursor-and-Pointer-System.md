# **SPEC-HAL-098 — Cursor & Pointer System**

**SPEC-HAL-098**
**Cursor & Pointer System**
**Author:** Halcyon Architect
**Status:** PROPOSED
**Version:** 1.0

**Depends On:**

* SPEC-HAL-009 — Event Routing & Hit-Testing
* SPEC-HAL-014 — Advanced Input
* SPEC-HAL-038 — Focus Model
* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-074 — Style Runtime Engine
* SPEC-HAL-090 — Logical Grid & Compact Layout Rules

---

## **Implementation Status**

*Not implemented.*

This SPEC defines the complete cursor/pointer system for Halcyon:

* canonical cursor types
* pointer semantics
* hover detection
* hit zone geometry
* resize zones
* drag affordances
* cursor priority rules
* platform mapping
* DPI-aware cursor rasterization

The goal is to create a **precise**, **stable**, and **predictable** pointer model suitable for dense engineering UI.

---

# **1. Motivation**

Pointer interaction is one of the main sensory channels in desktop UX.
A poor cursor system introduces:

* jitter
* ambiguous hit zones
* inconsistent tool affordances
* accidental drags
* poor precision on high DPI
* mismatched platform cursors

Halcyon needs a **unified, platform-agnostic** pointer model:

* predictable
* tight
* high-precision
* zero ambiguity
* minimal lag
* consistent across all widget families

---

# **2. Cursor Types**

Halcyon supports a strict and minimal cursor set.
No decorative or excessive variants.

### **Primary**

```
arrow (default)
text (I-beam)
hand (link / interactive hover)
```

### **Resize**

```
resize_horizontal
resize_vertical
resize_diag_tlbr
resize_diag_bltr
```

### **Move / Drag**

```
move
grabbing
drag_copy
drag_link
```

### **Forbidden / Blocked**

```
not_allowed
```

Platform-specific variants are normalized; the Halcyon abstraction stays stable.

---

# **3. Hit Zones**

Hit zones define where widgets change from default arrow to an actionable cursor.

### **Standard Hit Zone Padding**

```
horizontal: 3 px
vertical: 3 px
```

For compact UI (SPEC-HAL-090):

```
2 px minimum
```

No widget may define hit zones looser than this.

---

# **4. Resize Zones**

All resizable widgets (panels, windows, split views) use unified resize zones.

### **Zone Thickness**

```
6 logical px (default)
4 logical px (compact)
```

Behavior:

* cursor switches immediately upon entry
* zone detection must be pixel-precise
* no hover hysteresis
* corner zones have priority over edges

Corner zoning order:

1. diag TL–BR
2. diag BL–TR
3. vertical
4. horizontal

Widgets may not override this precedence.

---

# **5. Hover Logic**

Hover detection rules:

* hover is triggered when pointer enters widget bounds
* unhover triggered when pointer leaves
* hover state changes must be immediate
* pointer movement must not create flicker or stalls
* hover must not block pointer events to children

Hover → cursor updates must follow these constraints:

* no delay
* no animation
* instantaneous swap
* high-DPI-aware raster selection

Halcyon explicitly **forbids** web-style “hover delay” patterns.

---

# **6. Pointer Priority Model**

When multiple candidates exist for cursor change, Halcyon resolves priority:

1. **Active drag/resize operation**
2. **Resize zone**
3. **Draggable widget**
4. **Clickable interactive widgets**
5. **Selectable text**
6. **Default arrow**

This ensures:

* no ambiguous drag vs resize behavior
* no flickering between cursor types
* consistent hit testing

---

# **7. DPI & Scaling Behavior**

Cursors must remain pixel-crisp on all DPI configurations.

### Rules:

1. cursor raster assets are loaded per-DPI level
2. vector cursors (SVG) are pre-rasterized into GPU bitmaps
3. cursor hotspot must be correctly mapped into physical pixels
4. no fractional hotspot coordinates allowed
5. cursor scaling never uses linear filtering; nearest-neighbor only

This prevents:

* blurry pointers
* misaligned hotspots
* wandering hotspots on fractional scaling

---

# **8. Drag & Drop Pointer Semantics**

Halcyon defines a stable set of drag pointer transitions:

```
drag_start → grabbing  
drag_over_drop_zone → drag_copy or drag_link  
drag_blocked → not_allowed  
drag_end → arrow
```

Rules:

* drag cursor must update immediately
* no animation
* no fade
* pointer image must remain stable even during high-frequency motion

---

# **9. Focus Interaction**

Cursors must integrate with the input/focus model:

* pointer hover does not change focus
* clicking a focusable widget sets focus
* focus rings defined in SPEC-HAL-097 do not affect cursor

Keyboard navigation must never modify cursor type.

---

# **10. Declarative API Integration**

Example usage in the style system:

```
cursor.default = arrow
cursor.on_hover = hand
cursor.on_drag = grabbing
cursor.resize.horizontal = resize_horizontal
cursor.resize.corner = resize_diag_tlbr
```

Widgets may expose higher-level properties:

```
widget.set_draggable(true)
widget.set_resizable(true)
```

Internally mapped to cursor types and hit zones.

---

# **11. Renderer Integration**

Pointer images must be loaded via the Halcyon asset loader:

* platform cursor handles (Windows, macOS, X11, Wayland)
* GPU-managed raster variants
* physical DPI backends (1x, 1.5x, 2x, 3x, 4x)

Renderer guarantees:

* instant cursor swapping
* no hitching from asset loading
* consistent hotspot alignment

---

# **12. Implementation Plan**

### Phase 1 — Cursor Abstraction Layer

Define pointer types, DPI mappings, hotspot rules.

### Phase 2 — Hit Zone & Resize Detection

Implement unified hit zone logic.

### Phase 3 — Event Routing

Integrate pointer transitions with event system.

### Phase 4 — Drag Semantics

Integrate drag cursor transitions.

### Phase 5 — Platform Adapters

Implement cursor loaders for:

* Windows
* macOS
* X11
* Wayland

### Phase 6 — QA

* high-DPI displays (150%, 175%, 200%, 250%, 300%)
* multi-monitor setups
* fractional scaling
* ultra-high-refresh cursors (144–240 Hz)
* jitter and flicker testing
