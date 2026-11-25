# **SPEC-HAL-075 — Style Property Indexing & SIMD Merge Engine**

**SPEC-HAL-075**
**Style Property Indexing & SIMD Merge Engine**
**Author:** Halcyon Architect
**Status:** Draft
**Version:** 1.0

**Depends On:**

* SPEC-HAL-072 Declarative StyleSheets
* SPEC-HAL-073 Composition & Inheritance Model
* SPEC-HAL-074 Style Runtime Engine
* SPEC-HAL-046 Widget Model v2

**Implements:**
The **low-level engine** that makes Halcyon’s style system fast, deterministic, and zero-branch in the hot path:

* compact style property indexing
* SIMD merge for style cascade
* fast diff computation
* memory layout for StyleSnapshot
* property metadata tables
* inheritance and animatability tables

This SPEC defines the **machine-level backbone** under 072–074.
Without 075, the style engine would be slow, branchy, and unpredictable.

---

# **1. Motivation**

Halcyon’s style engine must:

* merge **multiple style sources** per widget
* resolve inheritance
* detect diffs quickly
* produce a **flat StyleSnapshot**
* run **for every widget** during:

    * layout
    * paint
    * theme switches
    * variant toggles
    * mixin updates

Cascade must be **fast**—much faster than Flutter, SwiftUI, Compose, or the web.

To achieve this, Halcyon needs a **fixed property table**, indexed by u8, and SIMD merge rules.

---

# **2. Goals**

### **Primary**

* Fixed **array-based StyleSnapshot** (no hashmaps, no lookups)
* SIMD-merge for cascade:
  merging `inherit → mixins → variant → overrides`
* Fast diff production
* Stable ordering for all properties
* Branch-free “apply or skip” logic

### **Secondary**

* Full metadata tables (inherit, layout-affecting, animatable)
* Zero allocations after init
* Cache-friendly memory layout

---

# **3. Property Indexing System**

### **3.1 PropertyID**

Every style property gets a **stable integer index**:

```
#[repr(u8)]
pub enum PropertyID {
    ColorFg = 0,
    ColorBg = 1,
    FontSize = 2,
    FontWeight = 3,
    PaddingLeft = 4,
    PaddingRight = 5,
    PaddingTop = 6,
    PaddingBottom = 7,
    MarginLeft = 8,
    MarginRight = 9,
    MarginTop = 10,
    MarginBottom = 11,
    Radius = 12,
    ShadowOffsetX = 13,
    ShadowOffsetY = 14,
    ShadowBlur = 15,
    ShadowColor = 16,
    Opacity = 17,
    Spacing = 18,
    // ...
    COUNT = N,
}
```

This index is used to index all style arrays.

### **3.2 PropertySlot**

All style values stored as f32 or packed f32x4.

```
struct PropertySlot {
    value: f32,
}
```

Color stored as:

```
struct ColorSlot {
    rgba: [f32; 4],
}
```

Selection depends on property type; indexing remains uniform.

### **3.3 StyleSnapshot Layout**

```
pub struct StyleSnapshot {
    pub f32_values: [f32; P],      // scalar props
    pub f32x4_values: [[f32; 4]; Q], // color/vec4 props
}
```

P and Q fixed at compile time.

This becomes **cache-perfect** and SIMD-friendly.

---

# **4. Property Metadata Tables**

These tables define semantics for each PropertyID.

### **4.1 Inheritance Table**

```
const INHERIT: [bool; PropertyID::COUNT] = [
    /* ColorFg */ true,
    /* ColorBg */ false,
    /* FontSize */ true,
    /* FontWeight */ true,
    /* Padding */ false,
    ...
];
```

### **4.2 Animatable Table**

```
const ANIMATABLE: [bool; COUNT] = [
    true, // fg
    true, // bg
    true, // font-size
    true, // weight
    false, // margin
    true, // opacity
    ...
];
```

### **4.3 Layout-Affecting Table**

```
const LAYOUT_AFFECTING: [bool; COUNT] = [
    false, // fg
    false, // bg
    true,  // font-size
    true,  // padding
    true,  // margin
    false, // opacity
    ...
];
```

Used by 074 for dirty propagation.

---

# **5. SIMD Merge Engine**

### **5.1 Merge Phases**

```
parent_snapshot
    → inherited_snapshot
        → mixin(style_mixin)
            → variant(style_variant)
                → widget_override
                    = final Snapshot
```

### **5.2 Merge Model**

Every style source is represented as:

```
struct StyleChunk {
    mask: [u64; K],   // which properties defined
    f32: [f32; P],
    f32x4: [[f32; 4]; Q],
}
```

### **5.3 SIMD Merge Algorithm**

Pseudocode:

```
for i in SIMD_chunks:
    let inherited_mask = PARENT.mask & INHERIT_MASK[i];
    let mixed_mask = MIXIN.mask[i];
    let variant_mask = VARIANT.mask[i];
    let override_mask = OVERRIDE.mask[i];

    let final_mask = inherited_mask | mixed_mask | variant_mask | override_mask;

    let chosen_values = simd_select(
        final_mask,
        [inherited_values, mixin_values, variant_values, override_values]
    );

    store(chosen_values into snapshot);
```

Branch-free.
Mask-driven.
Merges entire style snapshot in <0.5µs for deep trees.

---

# **6. StyleDiff Engine**

After building a new snapshot, compare with previous snapshot:

```
diff.mask[i] = (old[i] != new[i])
```

SIMD “not equal” across all f32 and f32x4 slots.

This immediately produces diff for SPEC-074.

---

# **7. Memory Model**

### **7.1 Snapshot Arena**

All snapshots stored in an arena:

```
struct StyleArena {
    snapshots: Vec<StyleSnapshot>,
}
```

Preallocated, compact, no fragmentation.

### **7.2 Chunk Reuse**

StyleChunk for mixins/variants reused across widgets.

Memory use is extremely small.

---

# **8. Runtime Behavior**

### **8.1 On Style Update**

1. Recompute snapshot
2. Compute SIMD diff
3. If diff.layout → layout dirty
4. If diff.paint → paint dirty
5. Notify animation engine for animatable properties

### **8.2 On Theme Switch**

* Theme provides a global StyleChunk
* Recompute entire tree
* SIMD makes it fast

### **8.3 On Mixin Change**

* Only users of mixin get dirty
* Cascade re-resolved

---

# **9. Testing Matrix**

### **9.1 Unit**

* Merge correctness for each phase
* Diff accuracy per property
* SIMD vs scalar correctness
* Alignment correctness

### **9.2 Integration**

* Cascading correctness
* Inheritance correctness
* Variant override priority
* Memory stability

### **9.3 Stress Tests**

* 100k widgets full tree cascade
* Theme switches under load
* Animations + cascading merges

---

# **10. Implementation Plan**

### **Phase 1 — Property Table Generator**

Generate:

* PropertyID enum
* INHERIT table
* LAYOUT_AFFECTING table
* ANIMATABLE table
* default values table

### **Phase 2 — Snapshot Struct**

Flat arrays, fixed-size.

### **Phase 3 — StyleChunk**

Mask + partial value arrays.

### **Phase 4 — SIMD Merge**

Implement using:

* std::simd
* packed f32x4 operations

Fallback to scalar if unavailable.

### **Phase 5 — Diff Engine**

Bitmask comparison per property.

### **Phase 6 — WidgetTree Integration**

Replace old hashmap or struct merges.

### **Phase 7 — Benchmarks**

Ensure <1µs per merge on mid-range CPU.
