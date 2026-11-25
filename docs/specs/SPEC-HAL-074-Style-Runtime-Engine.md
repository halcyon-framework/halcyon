# **SPEC-HAL-074 — Style Runtime Engine (Cascade Resolver, Live Updates, Dirty Propagation)**

**SPEC-HAL-074**
**Style Runtime Engine — Cascade Resolver & Live Updates**
**Author:** Halcyon Architect
**Status:** Draft
**Version:** 1.0

**Depends On:**

* SPEC-HAL-046 Widget Model v2
* SPEC-HAL-071 Animation DSL
* SPEC-HAL-072 Declarative StyleSheets
* SPEC-HAL-073 Composition & Nesting Model
* SPEC-HAL-009 Event Routing
* SPEC-HAL-024 Absolute Logical Space
* SPEC-HAL-091 Adaptive Density Modes

**Implements:**
The **runtime system** that takes all declarative + inherited + variant + mixin rules and turns them into **actual effective runtime styles**, including:

* cascade resolution
* inheritance merge
* variant substitution
* animation hooks
* dirty-style propagation
* subtree invalidation
* dynamic updates (theme change, variant toggle, hover/focus/pressed/etc.)

This SPEC defines the **real engine** that makes the styling system actually work at runtime.

---

# **1. Motivation**

072 defined StyleSheet syntax.
073 defined hierarchical composition rules.

But neither defined:

* how runtime inheritance is merged
* how styles are applied to widgets
* how updates propagate
* how style changes invalidate layout/paint
* how animations are triggered
* how variants toggle
* how to compute an “effective style snapshot”

Without this engine, styling is static and unusable.

074 is mandatory.

---

# **2. Goals**

### **Primary**

* Resolve full cascade into a **StyleSnapshot**
* Every widget gets a **final immutable resolved style**
* Update propagation system:

    * theme → whole tree
    * variant → subtree
    * mixin updates → subtree
    * local override → single widget
* Runtime performance:

    * O(1) merge operations
    * SIMD-friendly style payloads
    * minimal memory churn
    * guaranteed deterministic order

### **Secondary**

* Trigger animation rules (SPEC-071)
* Allow instant theme switching
* Allow live variant toggles (e.g., dark mode)
* Dirty bounding-box propagation (layout/pixel)

---

# **3. Core Types**

### **3.1 StyleSnapshot**

A flattened, immutable struct that contains all effective properties for a widget:

```
pub struct StyleSnapshot {
    pub color_fg: Color,
    pub color_bg: Color,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub padding: EdgeInsets,
    pub margin: EdgeInsets,
    pub radius: f32,
    pub shadow: Shadow,
    pub spacing: f32,
    pub opacity: f32,
    ... // dozens more
}
```

This object is computed once per update → cached → used for layout/paint.

### **3.2 StyleContext**

From SPEC-073:

```
struct StyleContext {
    scope: &'static str,
    variant: Option<&'static str>,
    mixes: SmallVec<[StyleMixin; 4]>,
    overrides: StyleSnapshot,
    inherited: StyleSnapshot,
}
```

### **3.3 MergeResult**

```
enum MergeResult {
    Unchanged,    // No change → no invalidation
    LayoutDirty,  // Affects geometry
    PaintDirty,   // Visual but not geometric
}
```

---

# **4. Cascade Resolution Pipeline**

When a widget is about to layout/paint, or when style invalidation occurs, this pipeline runs:

```
1. Start with parent.effective_style
2. Apply mixins (in order)
3. Apply variant rules
4. Apply widget-type defaults
5. Apply overrides (from builder)
6. Produce StyleSnapshot
```

Each step produces a small vector of changed properties (SIMD mask merge).

### Determinism

The order is strict and stable.
No CSS-like ambiguity.

---

# **5. Style Dirty Flags**

A style change affects either:

### **5.1 Paint-only:**

* fg color
* bg color
* shadow
* opacity
* border.color

→ only redraw the node

### **5.2 Layout-affecting:**

* padding
* margin
* radius (affects clipping)
* font-size
* spacing
* border.width
* min/max sizes

→ must re-layout widget and children

### **5.3 Transform-affecting:**

* position
* offset
* alignment

→ must recalc transforms

074 defines classification tables for **every style property**.

---

# **6. Dirty Propagation Rules**

When a widget becomes dirty:

### **6.1 PaintDirty**

Propagate upward until first **clip boundary**
(because clipping defines the repaint area).

### **6.2 LayoutDirty**

Propagate upward until:

* a widget with fixed-size layout (e.g., `Flex { fixed_height }`)
* root

Then back down for layout of subtree.

### **6.3 FullStyleDirty**

Triggered by:

* theme change
* global variant change
* scope change
* mixin mutation

→ entire tree is recomputed.

---

# **7. Live Updates & Hot-Swapping**

074 supports:

### **7.1 Theme Switch**

Theme is a special global scope.

```
cx.scope = "dark" → whole tree invalidated
```

### **7.2 Variant Toggle**

Widget-local:

```
button.set_variant("primary")
```

Subtree recalculated.

### **7.3 Mixin Change**

Mixins can be hot-swapped at runtime.

```
StyleSystem.update_mixin("rounded", new_definition)
```

Every user of that mixin is invalidated.

### **7.4 Override Change**

```
button.set_style(|s| s.font_size = 22.0)
```

Only that widget is dirty.

---

# **8. Animation Integration (SPEC-071)**

The style engine produces **diffs**, animation engine consumes diffs.

Example:

Changing:

```
opacity: 0.3 → 1.0
```

Triggers:

```
AnimationSystem.animate_property(widget_id, "opacity", from=0.3, to=1.0)
```

Animation modifies only PaintDirty fields.

Layout-affecting properties are not animated unless marked “animatable layout” (rare).

---

# **9. Internal Data Structures**

074 uses:

### **9.1 StylePool**

Arena allocator for snapshots.

### **9.2 Compact Diff**

Bitmask marking changed properties.

```
struct StyleDiff {
    mask: u64,
    values: SmallVec<[f32; 32]>,
}
```

### **9.3 StyleCache**

One per widget:

```
struct StyleNode {
    cx: StyleContext,
    snapshot: StyleSnapshot,
    diff: StyleDiff,
}
```

---

# **10. Integration With WidgetTree**

During the traversal:

### **Layout pass:**

1. Propagate style context
2. Resolve snapshot
3. Mark dirty types
4. If LayoutDirty → recalc layout
5. Pass snapshot to widget

### **Paint pass:**

1. Use already-computed snapshot
2. No allocation
3. Paint uses only read-only style data

---

# **11. Testing Matrix**

### **Style Cascade Tests**

* Inheritance correctness
* Mixins compute correct cascades
* Variant overrides
* Widget defaults applied last

### **Dirty Propagation Tests**

* PaintDirty stops at clip boundary
* LayoutDirty hits proper container
* Multi-toggle stress tests

### **Animation Tests**

* Opacity animated smoothly
* Background-color animation
* Shadow-radius interpolation

### **Live Updates Tests**

* Theme switch on large trees
* Variant propagation
* Mixin hot-swap

---

# **12. Implementation Plan**

### **Phase 1 — Core Types**

* StyleSnapshot
* StyleDiff
* property tables
* inheritance flags

### **Phase 2 — Context Propagation**

Implement from SPEC-073.

### **Phase 3 — Cascade Resolver**

Merge pipeline with SIMD masks.

### **Phase 4 — Dirty Propagation**

Add dirty flags to WidgetTree.

### **Phase 5 — Runtime Updates**

Theme, variants, mixins, overrides.

### **Phase 6 — Widget Integration**

Update Panel, Button, TextBlock primitives to use resolved snapshots.
