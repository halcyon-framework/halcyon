# **SPEC-R-008 — GPU Composition Graph v1 (Multi-Pass, Ordering, Subpass Fusion)**

**Author:** Halcyon Architect
**Status:** Retired — No current consumer. Composition handled by existing scene graph and batcher.
**Version:** 1.0

**Depends On:**

* SPEC-R-001 Clip Stack Rework
* SPEC-R-002 GPU Clip Shaders & Rounded Masks
* SPEC-R-003 GPU Path Rendering Primer
* SPEC-R-004 GPU Path Renderer (MSAA Vector Pipeline v1)
* SPEC-R-005 GPU Distance Field Renderer (SDF Pipeline v2)
* SPEC-R-006 GPU Vector Stroke Engine
* SPEC-R-007 GPU Blend Engine v1
* SPEC-HAL-024 Unified Coordinate Model
* SPEC-HAL-066 Unified Glyph Raster Pipeline

---

## Implementation Status

**Not implemented**

GPU composition graph is not implemented.

---

**Implements:**
A unified **Composition Graph** layer on top of the renderer, controlling all render passes, ordering, inter-pass dependencies, clip inheritance, and optimization.

Today Halcyon has ad-hoc passes executed in a single linear sequence.
R-008 introduces a **formal graph** that orchestrates:

1. Path fills (MSAA)
2. Strokes
3. SDF shading
4. Text quads
5. Blending
6. Clipping
7. Backdrop reads
8. Subpass fusion opportunities

This replaces scattered manual ordering logic, unlocks future renderer optimizations, and prevents subtle ordering bugs.

---

## **1. Motivation**

The current renderer behaves like a **static pipeline**:

```
path → stroke → sdf → text → overlay → composite
```

This is rigid and breaks down for:

* nested isolated-render layers
* backdrop-filter widgets
* animated blur or color transforms
* custom widgets that insert render passes
* future WebGPU subpasses or Vulkan render passes
* multi-target outputs (e.g., picking)

A formal **Composition Graph** solves this by:

* modeling render operations as nodes
* modeling dependencies as edges
* allowing flexible insertion of new nodes
* enabling subpass fusion (GPU optimization)
* producing a deterministic, validated execution schedule

---

## **2. Goals**

### **2.1 Primary Goals**

* Deterministic multi-pass execution
* CPU-side graph construction per frame
* GPU-side execution of scheduled passes
* Guaranteed correct ordering (clip → draw → blend → composite)
* Subpass fusion optimization (merge compatible passes)
* Backdrop read management

### **2.2 Secondary Goals**

* Explicit intermediate textures
* Support offscreen isolation
* Automatic lifetime management of render targets

### **2.3 Non-Goals**

* Full Vulkan-style render pass API
* Tiling or deferred rendering
* HDR pipeline (future SPEC)

---

## **3. Composition Graph Model**

The graph is a **DAG**, rebuilt each frame.

### **3.1 Node Kinds**

```
enum PassKind {
    PathFill,
    PathStroke,
    SdfFill,
    Text,
    ClipMask,
    Blend,
    BackdropRead,
    CompositeFinal,
}
```

### **3.2 Node Structure**

```
struct PassNode {
    id: PassId,
    kind: PassKind,
    inputs: Vec<TextureId>,
    outputs: Vec<TextureId>,
    deps: Vec<PassId>,
}
```

### **3.3 Texture Resources**

```
enum TextureKind {
    LinearRgba16f,
    SdfR8,
    Msaa4x,
    Depth8,
    Intermediate,
}
```

Textures are allocated per frame according to pass requirements.

### **3.4 Rules**

* ClipMask must run before any pass that uses its mask
* BackdropRead must occur before Blend
* CompositeFinal must consume last linear target
* Text must run after path fills/strokes but before blend
* Isolation boundaries allocate new intermediate textures

---

## **4. Subpass Fusion**

A major goal: merging multiple passes into the same GPU render pass.

Example fusion:

```
PathFill + Stroke → fused
PathFill + Text   → fused if clip identical
Text + SDF        → incompatible (different pipelines)
```

Fusion rules:

### **4.1 Compatible If**

* Same render target
* Same clip mask
* Same blend mode
* Same sample count (MSAA)
* No backdrop reads between them

### **4.2 Benefits**

* Reduces render pass transitions
* Minimizes GPU pipeline swaps
* Reduces command buffer overhead
* Improves batching

---

## **5. Isolation Layers**

Widgets (like modals, blur panes) request isolated rendering:

```
Isolate {
    children render into offscreen texture
    then composite into parent
}
```

Composition graph supports:

* isolated texture
* background-copy node (backdrop read)
* composite/blend node
* clip inheritance

---

## **6. Backdrop Reads**

Needed for:

* blur
* color filters
* opacity groups

R-008 defines:

```
PassKind::BackdropRead
```

It produces a linear RGBA16F snapshot of the parent’s target.

Constraints:

* Must occur *before* any blending that requires it
* Can only read stable textures
* Cannot read from MSAA or SDF textures (must resolve first)

---

## **7. Execution Schedule**

### **7.1 Steps**

1. Build nodes
2. Resolve dependencies
3. Topological sort
4. Perform subpass fusion
5. Allocate textures
6. Emit GPU command buffer
7. Submit to device

### **7.2 Validation Rules**

The scheduler validates:

* no cycles
* no missing dependencies
* no read-after-write hazards
* correct clip ordering
* correct isolation

---

## **8. Integration with Existing Renderer**

### **8.1 Current vs New Model**

**Current:**

```
path pass
stroke pass
sdf pass
text pass
final blend
present
```

**New:**

```
graph.build(scene)
graph.schedule()
graph.execute()
present
```

### **8.2 Shader Constraints**

Each pass must specify:

* its pipeline key
* input textures
* output target
* blend mode (R-007)
* clip mask texture if any

---

## **9. Testing Matrix**

### **9.1 Structural Tests**

* cycles detected correctly
* no missing dependencies
* deterministic ordering

### **9.2 Visual Tests**

* transparency layers
* rounded clips + text
* blur + text + stroke
* nested isolates
* modal + backdrop blur
* MSAA strokes over SDF backgrounds

### **9.3 Stress Tests**

* 20+ isolated widgets
* rapid animations
* 4K resolution resizing
* blend-mode stress (multiply, screen, etc.)

---

## **10. Implementation Plan**

### **Phase 1 — Graph Infra**

* DAG nodes + edges
* Texture registry
* Pass registration

### **Phase 2 — Scheduler**

* Topological sort
* Constraint solver
* Subpass fusion

### **Phase 3 — GPU Integration**

* Bind groups per pass kind
* Render target allocator
* Linear-light intermediate

### **Phase 4 — Migration**

* Convert each renderer pass into a node
* Remove old hardcoded pass chain

### **Phase 5 — Validation**

* Compare vs legacy renderer
* Enable new path on feature flag
* Make new pipeline default

---

## **11. Implementation Notes**

* Composition Graph unlocks future optimization SPECs
* Subpass fusion is a major win for Halcyon’s batching model
* Isolation layers become trivial
* Backdrop filters become first-class
* This makes Halcyon’s renderer structurally comparable to Flutter/Skia’s RenderGraph, but with deterministic ordering and explicit resource semantics
