# **SPEC-HAL-083 — Unified SceneGraph Diff Engine v2**

**SPEC-HAL-083**
**Unified SceneGraph Diff Engine v2**
**Author:** Halcyon Architect
**Status:** Closed / Complete
**Closure Phase:** Phase-24
**Closure Date:** 2026-01-14
**Implementation State:** Fully implemented and tested
**Version:** 1.0

**Depends On:**

* SPEC-HAL-005 Scene Graph
* SPEC-HAL-024 Unified Coordinate Model
* SPEC-HAL-028 RenderPass Architecture
* SPEC-HAL-039 Scene Diff Compression
* SPEC-HAL-041 Unified Timing & Animation Clock
* SPEC-HAL-045 GPU Resource Lifetime
* SPEC-HAL-074 Style Runtime
* SPEC-HAL-075 Layout Stability Guarantees
* SPEC-HAL-076 StyleCache v2
* SPEC-HAL-077 Style Animations
* SPEC-HAL-078 Layout Transitions
* SPEC-HAL-079 Unified Event Flow v3
* SPEC-HAL-080 Interaction Engine
* SPEC-HAL-081 Style Reflow Model v2
* SPEC-HAL-082 Paint Order Model v2

**Implements:**
A fully deterministic, incremental, stable **SceneGraph diff engine** that transforms widget output frames into minimal GPU-ready deltas, guaranteeing order stability, spatial stability, and animation continuity.

---

## **1. Motivation**

The existing SceneGraph diff (v1) is too naive:

* compares nodes structurally instead of semantically
* causes redundant rerenders
* induces flicker during transitions
* fails to preserve instance identity across frames
* runs O(n) per node instead of O(1) amortized
* doesn’t integrate with StyleCache v2
* doesn’t preserve paint-order contracts from SPEC-082

Halcyon needs a deterministic, stable, compression-friendly SceneGraph diff that works at scale (thousands of nodes).

---

## **2. Goals**

### **2.1 Primary Goals**

* Stable scene identity across frames
* Deterministic minimal diffs
* Zero flicker on widget recreation
* O(1) amortized diff cost per node
* Full paint-order correctness
* Reflow-aware diffing (SPEC-081)
* Style transition compatibility
* Animation frame continuity

### **2.2 Secondary Goals**

* GPU-friendly delta structure
* Easy instance-based caching
* Seamless batching and reordering
* Compression-friendly node encoding

---

## **3. Architecture Overview**

```
Widget Tree → Layout → Paint → Raw SceneGraph
                          ↓
                   SceneGraph v2
                          ↓
                 Diff Engine v2
                          ↓
              SceneDelta (GPU-ready)
                          ↓
                 Renderer (wgpu)
```

Diff Engine v2 operates on **semantic identity**, not structural shape.

---

## **4. SceneNode Identity Model**

Every SceneNode receives a **StableSceneId**:

```
struct StableSceneId(u64);
```

Generated from:

* widget ID
* local paint index
* paint-order slot
* element subtype (Rect, Text, Image, Clip…)

Stable across frames unless the widget tree structure truly changes.

---

## **5. Node Key**

Each node produces a canonical key:

```
SceneKey {
    id: StableSceneId,
    kind: SceneKind,
    order: PaintOrderSlot,     // from SPEC-082
    bbox: RectF,               // absolute (SPEC-024)
}
```

This key drives diffing.

---

## **6. Diff Algorithm**

### **Input:**

* Previous SceneGraph
* New SceneGraph

### **Output:**

```
SceneDelta {
    inserts: Vec<SceneNode>,
    updates: Vec<SceneUpdate>,
    removes: Vec<StableSceneId>,
}
```

### **Core Rules:**

#### **6.1 Match by StableSceneId**

Not index.
Not structural position.

#### **6.2 Only diff meaningful fields**

Position
Color
Opacity
Paint-mode
Texture UVs
Glyph quads
Clip rects

Ignore fields irrelevant to rendering (like debug tags).

#### **6.3 Stability Rules**

If:

* widget exists
* but layout changed
* but identity remains

→ emit **update**, not remove/insert.

#### **6.4 Removal Rules**

Node removed only if:

* widget unmounted
* or paint-order context changed
* or SceneKind changed

---

## **7. Scene Update Compression**

SPEC-039 integrates here.

Compression applied to:

* floats (pos, size)
* clip rects
* color values
* UVs
* glyph quad groups
* style values

Using:

* delta-of-delta
* run-length groups
* uniform-value collapse
* shared-style tables (from SPEC-076)

---

## **8. GPU-Ready Delta Model**

The delta is split into:

```
DeltaRect       → rect batch
DeltaText       → text batch
DeltaImage      → image batch
DeltaClip       → clip batch
DeltaComposite  → blend batch
```

Each is tightly packed with GPU-ready structs aligned to 16 bytes.

---

## **9. Handling Animations (SPEC-041)**

Animations require special handling:

* If value is animated → diff stores **target**, not intermediate
* Renderer samples via unified animation clock
* No re-diff during animation frames
* Only structural changes trigger deltas

Result: extremely cheap animation frames.

---

## **10. Style Integration**

From SPEC-074/076/077:

* StyleCache v2 provides semantic style keys
* Diff engine compares style keys instead of raw style fields
* Guarantees stable identity even if style colors animate

---

## **11. Layout Integration**

SPEC-075/081 ensure:

* coordinates deterministic
* no layout jitter
* diff engine sees only true semantic changes
* stable transitions during wrapping, font fallback, or reflow

---

## **12. Implementation Plan**

### **Phase 1 — Identity & Keying**

* Implement StableSceneId
* Integrate into all SceneNodes
* PaintContext assigns identity slots

### **Phase 2 — Diff Core**

* Build lookup tables
* Implement match/update/remove logic
* Add compression hooks

### **Phase 3 — GPU Delta Encoding**

* Define DeltaX structs
* Add alignment + packing rules
* Implement instanced encoding

### **Phase 4 — Renderer Integration**

* Modify wgpu renderer to consume SceneDelta
* Add per-pass batching

### **Phase 5 — Optimizations**

* Deduplicate style tables
* Implement advanced compression
* Add GPU reuse paths

### **Phase 6 — Testing**

* Stress test with 10k nodes
* Rapid reflow
* Rapid style changes
* Rapid scene rebuild
* Directionality (RTL/LTR)
* Animated text & layout

---

## **13. Status**

**Closed / Complete** — All phases through Phase-24 are implemented and tested.

---

## **14. Closure Summary**

SPEC-HAL-083 is **frozen**. All implementation phases are complete:

* **Phase-1 through Phase-17**: Core diff engine, identity plumbing, consumer pipeline, dirty tracking
* **Phase-18**: Cache↔Scene consistency enforcement (debug assertions)
* **Phase-19**: Typed handle system with generation tracking. Debug generation metadata is live-entry-bounded: removing or clearing an entry removes its generation record, while a cache-global monotonic generation source prevents a later reuse of the same key from revalidating a stale handle.
* **Phase-20**: Cache entry ownership split (CpuOnly vs GpuBound)
* **Phase-21**: Cache mutation surface lockdown
* **Phase-22**: Explicit cache lifecycle phases (Build / Dispatch / Frozen)
* **Phase-23**: Read-only dispatch guard
* **Phase-24**: Stable read snapshot guarantee

Key invariants enforced:

* **Lifecycle enforcement**: Build phase allows mutations; Dispatch/Frozen phases are read-only
* **Identity plumbing**: All scene nodes carry stable keys; handles are typed and generation-checked
* **Bounded debug identity state**: Generation tracking may retain metadata only for live cache entries; historical removed keys never accumulate tombstones during rapid scene rebuilds
* **Mutation surface lockdown**: All mutation APIs assert correct phase and ownership
* **Stable read snapshot**: During Dispatch/Frozen, entry_count and dirty_count are immutable
* **Debug vs release split**: All assertions are debug-only; release builds have zero overhead

---

## **15. Non-Goals (Post-Closure)**

The following are explicitly **out of scope** for SPEC-HAL-083:

* Renderer/GPU integration — belongs in RenderGraph SPECs
* GPU resource binding or buffer management
* Advanced compression or batching optimizations
* Cache eviction or memory pressure policies
* Any extensions or modifications to this SPEC

SPEC-HAL-083 is a stable foundation. All future work must happen in new SPECs.

---

## **16. Follow-Up SPECs**

SPEC-HAL-083 is consumed by:

* **SPEC-HAL-084** (RenderGraph v2) — consumes SceneCache during dispatch
* Future GPU-side cache SPECs — will bind GPU resources to cache entries

**Ownership rule**: SPEC-HAL-083 must not be modified by consuming SPECs. Any required changes to the diff engine or cache model must be proposed as new SPECs with explicit dependency chains.
