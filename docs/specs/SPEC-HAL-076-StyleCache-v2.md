# **SPEC-HAL-076 — StyleCache v2 (Hot-Path Caching for Layout & Paint)**

**SPEC-HAL-076**
**StyleCache v2 — Hot-Path Style Snapshot Caching**
**Author:** Halcyon Architect
**Status:** Draft
**Version:** 1.0

**Depends On:**

* SPEC-HAL-072 Declarative StyleSheets
* SPEC-HAL-073 Composition & Inheritance
* SPEC-HAL-074 Style Runtime Engine
* SPEC-HAL-075 SIMD Merge Engine
* SPEC-HAL-046 Widget Model v2

**Implements:**
A **multi-layer caching system** that prevents redundant style merges or recomputation during layout/paint.
Designed for extremely large UI trees (100k+ nodes) with real-time theme switching or animations.

---

# **1. Motivation**

The merge engine in SPEC-075 is fast, but not free.

Some trees won’t need full merges on every traversal:

* many widgets share identical style inputs
* many nodes are structurally stable
* variant/mixin/theme changes often affect only subtrees
* layout must run every frame — style must not

To avoid unnecessary CPU load, Halcyon needs a **tiered StyleCache** that:

* caches StyleSnapshots
* caches diffs
* caches “layout-affecting bits”
* caches final resolved color/material values
* avoids recomputing snapshots when all inputs unchanged

---

# **2. Goals**

### **Primary**

* Only compute StyleSnapshot when needed
* Minimal branching in Widget::layout / Widget::paint
* Zero-alloc hot path
* Dirty-bits propagate only where required
* Detect identical style roots to reuse snapshots

### **Secondary**

* Baseline for Style Animations (upcoming SPEC)
* Integrate with Flex/Layout engines
* Provide constant-time “should I layout?” checks

---

# **3. Architecture Overview**

```
StyleSheet + Mixin + Variant + Inherit
         ↓
   StyleInputsHash (64-bit)
         ↓
   StyleCache.lookup(hash)
         ↓             ↘ no
   cached_snapshot? ———→ merge via SPEC-075 → snapshot
         ↓
   store snapshot in LRU
         ↓
Widget.layout / Widget.paint consume snapshot
```

A single 64-bit hash identifies the full **style input state** of a widget:

```
(parent_snapshot_id,
style_node_id,
variant_id,
override_id,
theme_version)
```

If the hash is unchanged → the snapshot is reused.

---

# **4. StyleInputsHash**

### **4.1 Hash Structure**

```
struct StyleInputsHash {
    parent_snapshot_id: u32,   // from ancestor
    local_style_id: u16,       // stylesheet node
    variant_id: u16,           // active variant
    override_hash: u32,        // inline overrides
    theme_version: u16,        // global increment
}
```

### **4.2 64-bit fold**

```
fn fold(&self) -> u64
```

Used as the cache key.

---

# **5. StyleCache**

### **5.1 Structure**

```
struct StyleCache {
    lru: LruMap<u64, CachedStyleEntry>,
    capacity: usize,
}
```

### **5.2 CachedStyleEntry**

```
struct CachedStyleEntry {
    snapshot: StyleSnapshot,
    diff_mask: DiffMask,
    layout_affects: bool,
    animatable_mask: u64,
}
```

### **5.3 Behavior**

* If entry present → directly use cached snapshot
* If absent → compute via SIMD merge → insert
* Least-recently-used eviction

No lifetime leaks. No dangling pointers.

---

# **6. Dirty Bit Model**

Three dirty categories:

### **6.1 Style Dirty**

Snapshot changed.
Checked via hash mismatch or diff.

### **6.2 Layout Dirty**

Layout-affecting bits changed.
From SPEC-075 `LAYOUT_AFFECTING`.

### **6.3 Paint Dirty**

Non-layout style changed (colors, opacity, etc.)

Each propagated separately through WidgetTree.

---

# **7. Snapshot Reuse**

### **7.1 Identical style nodes share snapshots**

If two widgets have identical input hashes →
they reuse the exact same snapshot pointer.

This halves style memory on many trees.

### **7.2 Deduplication Table**

```
struct SnapshotPool {
   interning: HashMap<SnapshotKey, Arc<StyleSnapshot>>,
}
```

Allows de-duping large static UI structures.

---

# **8. Animation Integration (Prep)**

076 sets up fields needed for animation system:

* `animatable_mask` tells which properties are animatable
* future SPEC will create a live “AnimatedLayer” overlay
* diff + animatable_mask drive animation triggers

---

# **9. Theme Switching**

On theme update:

* increment global `theme_version`
* all hashes become invalid
* widgets recompute snapshot on next layout/paint

Zero complexity.
Instant, consistent theme swaps.

---

# **10. Multi-Pass Layout Engine Integration**

StyleCache provides:

```
fn layout_affects(&self) -> bool;
```

Layout engine uses:

* If `layout_affects == false` AND size unchanged → skip subtree layout
* If `true` → relayout only subtree
* If paint dirty only → skip layout

This makes layout O(changes) rather than O(n).

---

# **11. Testing Matrix**

### **Unit Tests**

* Hash correctness
* Snapshot correctness
* LRU behavior
* Layout-affecting detection

### **Integration Tests**

* Deep tree theme switch
* Variant toggles
* Live style overrides
* Whole-tree deduplication

### **Stress Tests**

* 100k widgets switching theme
* 10k animation triggers per frame
* Randomized style mutation

Goal:
**Style cache hit-rate > 95%** on stable UI.

---

# **12. Implementation Plan**

### **Phase 1 — Hashing Layer**

Implement `StyleInputsHash`, folding, equality.

### **Phase 2 — LRU Cache**

Fixed-capacity LruMap.

### **Phase 3 — CachedStyleEntry**

Snapshot + diff + masks.

### **Phase 4 — Integrate into StyleEngine**

Check cache before merge.

### **Phase 5 — Integrate Dirty Bits with WidgetTree**

Layout vs paint dirty propagation.

### **Phase 6 — Snapshot Pool (Interning)**

Deduplicate repeated snapshots.

### **Phase 7 — Benchmark + Optimize**

Requirement: < 200ns per cache lookup.
