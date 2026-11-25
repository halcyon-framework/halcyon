# SPEC-HAL-084 — RenderGraph v2 (GPU Pass Orchestration Law)

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**SPEC-HAL-084**
**RenderGraph v2 (GPU Pass Orchestration Law)**
**Author:** Halcyon Architect
**Status:** Binding — Phase Boundary Enforcement: COMPLETE
**Version:** 2.0
**Amended by:** SPEC-HAL-295A (Adopted / Binding; narrow custom offscreen authority correction to INV-RG-011, §K, and AUTH-084-3); SPEC-HAL-359 (Adopted / Binding; per-surface cross-pipeline Z execution clarification to INV-RG-010); [SPEC-HAL-360](SPEC-HAL-360-Complete-Text-Realization.md) (Adopted / Binding; complete-required text admission and narrow D2 clarification preserving Compile-only GPU writes)

**Depends On:**

* SPEC-HAL-002 Constitution and Architectural Invariants
* SPEC-HAL-004 WGPU/Winit Integration
* SPEC-HAL-005 Scene Graph
* SPEC-HAL-024 Unified Coordinate Model
* SPEC-HAL-028 Renderer Architecture v1
* SPEC-HAL-030 Presentation Policy and Swapchain Semantics
* SPEC-HAL-040 GPU Resource Lifetime & Pools
* SPEC-HAL-041 Unified Timing & Animation Clock
* SPEC-HAL-045 GPU Triple-Buffer Scene Streaming
* SPEC-HAL-081 The Halcyon Compositor v1
* SPEC-HAL-082 Multi-Window Architecture
* SPEC-HAL-083 Unified SceneGraph Diff Engine v2
* SPEC-HAL-083A SceneCache Dispatch Mandatory Wiring
* SPEC-HAL-150 Compositor Authority and Limits

---

## Implementation Status

**Partially implemented — Phase Boundaries Enforced**

The formal RenderGraph pass system is not implemented. However, the critical **Build/Compile/Execute phase separation** is enforced:

* **Build Phase** (`DrawPlanBuilder::build`): CPU-only, produces `DrawPlan`
* **Compile Phase** (`WgpuRenderer::compile`): All GPU buffer writes
* **Execute Phase** (`WgpuRenderer::render`): Command encoding only, zero writes

See **Appendix: Phase Enforcement** for Rust mechanisms that enforce these boundaries.

Further work on RenderGraph structure (pass ownership, graph topology) is deferred to SPEC-HAL-085+.

---

## SECTION A — Purpose

This SPEC defines the **binding invariants** and **authority boundaries** for the RenderGraph v2 system.

RenderGraph v2 is the **orchestration layer** for all GPU work in Halcyon. It receives compositor output, schedules GPU passes, and submits command buffers. It does not invent, interpret, or modify scene semantics.

This document establishes:

* What RenderGraph **must do**
* What RenderGraph **must never do**
* The **phase separation** that governs all GPU orchestration
* The **consumption rules** for SceneCache and compositor output
* The **resource ownership** and lifetime constraints

All implementation must conform to these invariants. Any violation is an architectural breach.

---

## SECTION B — Binding Invariants

The following invariants are **law**. They govern all RenderGraph implementation, extension, and modification. Invariants use declarative, testable language. Forbidden behaviors are explicit.

---

### INV-RG-001 — Orchestration Only

RenderGraph owns **ordering, dependency resolution, and execution**.

RenderGraph does NOT:

* invent draw data
* generate geometry
* create resources from semantics
* interpret scene meaning

RenderGraph is downstream of all authority. It executes; it does not decide.

---

### INV-RG-002 — Single Consumer of SceneCache

RenderGraph may consume SceneCache **only** during the Build phase.

**Forbidden:**

* Pass executors reading SceneCache directly
* GPU callbacks accessing SceneCache
* Any transitive read of SceneCache during Compile or Execute phases

This is enforced transitively from SPEC-HAL-083 and SPEC-HAL-083A ordering guarantees.

---

### INV-RG-003 — Phase Separation Is Absolute

RenderGraph has exactly **three phases**:

1. **Build** — Pure CPU construction, no GPU objects created
2. **Compile** — Resource resolution, logical-to-physical binding, no GPU commands
3. **Execute** — GPU command encoding only

**Invariant constraints:**

* Cross-phase data access is forbidden
* A pass executor cannot mutate graph structure
* A pass executor cannot mutate resources
* Phase boundaries are hard; no "soft" transitions exist

---

### INV-RG-004 — Deterministic Topology

Given identical inputs (SceneCache + FramePlan), RenderGraph topology and pass order MUST be **bit-for-bit identical**.

**Forbidden:**

* Iteration over HashMap or HashSet without deterministic ordering
* Any source of nondeterminism in graph construction
* Timestamp-based or random ordering decisions

Topology must be reproducible for debugging and testing.

---

### INV-RG-005 — Explicit Dependency Only

A pass may execute **only** after all declared dependencies have completed.

**Forbidden:**

* Implicit ordering based on registration order
* "Well-known" pass names that imply ordering
* Hardcoded sequencing outside the dependency graph
* Hidden dependencies not declared in the pass node

All ordering must be explicit and inspectable.

---

### INV-RG-006 — No Semantic Authority

RenderGraph does NOT decide **what** is drawn.

RenderGraph decides only **when** and **where** GPU work executes.

This aligns with SPEC-HAL-150: RenderGraph is downstream of compositor authority. The compositor provides immutable render intent; RenderGraph schedules its execution.

---

### INV-RG-007 — Resource Ownership Boundary

RenderGraph owns **logical resources only**:

* Attachment descriptors
* Buffer descriptors
* Texture descriptors

Physical GPU resources are acquired via SPEC-HAL-040 pools during Compile/Execute phases.

**Forbidden:**

* RenderGraph retaining raw `wgpu::*` handles beyond a frame epoch
* RenderGraph owning GPU memory directly
* Pass executors storing GPU handles across frames

**363/364 scoped amendment:** Cross-frame physical content leases are SPEC-HAL-040-owner records,
not raw handles stashed in RenderGraph, SceneNodes or pass executors.

---

### INV-RG-008 — Resource Lifetime Is Frame-Scoped

**363/364 scoped amendment:** Frame-created graph resources remain frame-scoped by default.
A resource may persist only through an expressly assigned SPEC-HAL-040 lifetime,
including SPEC-HAL-364's named Retained Presentation Generation lifetime.
RenderGraph retains only logical external lease references for that lifetime;
physical GPU holdings and safe retirement remain with the existing renderer
resource owner. Keeping a graph descriptor or pass executor alive SHALL NOT
extend a physical resource's validity. Unassigned cross-frame retention remains
forbidden; graph-internal resources remain Tier 2 by default.

---

### INV-RG-009 — Swapchains Are Sinks, Never Sources

Swapchain images are **terminal nodes** in the graph.

**Forbidden:**

* Reading from a swapchain attachment
* Using swapchain as input to any pass
* Sharing swapchain state between windows

Multi-window graphs have multiple sinks. Each window has exactly one swapchain sink node.

---

### INV-RG-010 — Compositor Contract

RenderGraph consumes compositor output as **immutable render intent**.

**Forbidden:**

* Reordering compositor layers
* Reinterpreting z-order
* Applying geometry changes to compositor output
* Modifying blend modes or opacity not specified by compositor
* Any operation that would violate INV-COMP-NO-LIES (SPEC-HAL-150)

Violation equals compositor authority breach.

> **Adopted clarification:** SPEC-HAL-359 requires Compile to preserve admitted
> primitive Z order across pipeline-specific lowering in one immutable
> per-surface command schedule. Execute following that schedule is faithful
> consumption of compositor/render intent; grouping by pipeline in a way that
> overrides strictly different Z values is forbidden reinterpretation.

---

### INV-RG-011 — No Hidden Passes

> **Adopted amendment:** SPEC-HAL-295A supplies the binding replacement text for
> INV-RG-011, the §K “All GPU work flows through the graph” criterion, and
> AUTH-084-3.

Every renderer-owned GPU pass and every GPU submission that consumes or
mutates renderer/compositor resources MUST correspond to a RenderGraph node.
Ad-hoc encoders, helper passes, direct encoder access, and hidden submissions
in that domain remain forbidden.

A producer explicitly admitted by SPEC-HAL-295 is outside the RenderGraph
domain only while it operates exclusively on participant-owned offscreen
resources and obeys SPEC-HAL-295A. Its display result is represented in graph
topology by the ordinary `ImageNode` composition pass. This exception grants
no access to renderer-owned resources and cannot be extended by analogy.

**363/364 scoped amendment:** SPEC-HAL-364 composition, isolation, mask and copy passes are
renderer-owned work and SHALL be represented in Build topology. The
SPEC-HAL-295A participant-owned exception SHALL NOT be used for renderer
snapshotting or mask/isolation passes.

---

### INV-RG-012 — Failure Visibility

Graph construction failures MUST cause **hard failure**.

Failure conditions include:

* Cycles in dependency graph
* Missing dependencies
* Invalid resource references
* Duplicate pass names
* Resource format mismatches

**Forbidden:**

* Silent fallback to degraded behavior
* Best-effort execution of partial graphs
* Swallowing errors with default values
* Continuing execution after structural failure

Failures must be loud, immediate, and debuggable.

---

### INV-RG-013 — Presentation Isolation

Present-mode policy (SPEC-HAL-030) is applied **after** graph execution.

**Forbidden:**

* RenderGraph selecting present modes
* RenderGraph controlling vsync behavior
* RenderGraph making timing decisions
* Coupling graph structure to presentation policy

RenderGraph produces command buffers. Presentation layer decides when and how to present.

---

### INV-RG-014 — Triple-Buffer Compatibility

RenderGraph consumes exactly **one immutable SceneFrame** per execution (SPEC-HAL-045).

**Invariant constraints:**

* RenderGraph MUST NOT assume synchronization with the UI thread
* RenderGraph MUST NOT block waiting for scene production
* RenderGraph MUST treat SceneFrame as frozen/read-only
* RenderGraph MUST NOT modify SceneFrame contents

This preserves SPEC-HAL-045 decoupling between UI and GPU threads.

---

### INV-RG-015 — Pass Executor Isolation

Pass executors receive a **constrained context**.

Pass executors MAY:

* Encode GPU commands
* Read from bound resources
* Write to declared output attachments

Pass executors MUST NOT:

* Access graph structure
* Modify other passes
* Access SceneCache
* Create new resources
* Modify resource bindings

Executors are sandboxed to their declared inputs/outputs.

---

### INV-RG-016 — Extension Rule

Any new behavior requires a **new SPEC** and **zero reinterpretation** of these invariants.

New behaviors include:

* Resource aliasing rules
* Async pass execution
* Subgraph merging
* Pass reordering optimizations
* Cross-frame resource persistence

Extensions must explicitly declare compatibility with all INV-RG-* invariants.

**363/364 scoped amendment:** SPEC-HAL-364 §12 is the explicit new extension for occurrence
composition and named retained content leases. It declares compatibility with
INV-RG-001/002/003/004/009: no semantic/geometry inference, no SceneCache read
beyond Build, no new phase, deterministic topology and swapchains as sinks.

---

## SECTION C — Authority Boundaries

RenderGraph exists in a strict authority hierarchy:

```
SPEC-HAL-002 (Constitution)
    ↓
SPEC-HAL-150 (Compositor Authority)
    ↓
SPEC-HAL-083/083A (SceneCache/Diff Engine)
    ↓
SPEC-HAL-084 (RenderGraph — THIS SPEC)
    ↓
SPEC-HAL-040 (GPU Resource Pools)
    ↓
SPEC-HAL-030 (Presentation Policy)
```

RenderGraph:

* Receives authority from compositor (SPEC-HAL-150)
* Consumes SceneCache under SPEC-HAL-083A ordering constraints
* Delegates resource allocation to SPEC-HAL-040 pools
* Defers presentation decisions to SPEC-HAL-030

RenderGraph MUST NOT:

* Override compositor decisions
* Bypass SceneCache consumption rules
* Manage GPU resources outside the pool system
* Control presentation timing

---

## SECTION D — Phase Contracts

### D.1 Build Phase

**Trigger:** Frame start, after SceneCache dispatch (SPEC-HAL-083A)

**Inputs:**

* SceneCache (read-only snapshot)
* FramePlan from compositor

**Outputs:**

* Complete RenderGraph topology
* All passes registered with dependencies
* All resources declared

**Constraints:**

* Pure CPU operations only
* No GPU object creation
* No command encoding
* SceneCache access permitted ONLY in this phase

**363/364 scoped amendment:** Build consumes the already-selected immutable scene/cache inputs
after mandatory dispatch and declares all composition passes, dependencies and
logical resources. No widget callback, widget/member realization or producer
paint may be invoked by Compile or Execute. Renderer-owned text/resource
realization from complete admitted owned descriptors remains at its established
Compile boundary under SPEC-HAL-360/SPEC-HAL-084. The final scene-choice cutoff
precedes diff, and therefore also precedes Build; Build is not a late
scene-fallback boundary.

### D.2 Compile Phase

**Trigger:** Build phase complete

**Inputs:**

* Frozen RenderGraph topology
* Resource descriptors

**Outputs:**

* Topologically sorted pass order
* Physical resources acquired from pools
* Resource lifetime analysis complete

**Constraints:**

* Graph structure is immutable
* Resource descriptors are immutable
* GPU textures/buffers acquired but not written
* No command encoding

**363/364 scoped amendment:** Topology and descriptors freeze after Build. Compile SHALL resolve
each required source/resource/profile and either produce a complete executable
surface plan or its typed complete-unavailability outcome. Known admission or
binding failures SHALL be detected before Execute. They SHALL NOT create a
late branch, drop a required occurrence or restart scene/diff/dispatch.

### D.3 Execute Phase

**Trigger:** Compile phase complete

**Inputs:**

* Sorted pass list
* Bound physical resources
* Pass executor closures

**Outputs:**

* Command buffers ready for submission

**Constraints:**

* Graph structure is immutable
* Resource bindings are immutable
* Only pass executors may encode commands
* No new resource allocation
* No SceneCache access

**363/364 scoped amendment:** Execute consumes the frozen complete plan only, without allocating
a new target or realizing sources. A failure first learned after encoding,
submission or a present call follows SPEC-HAL-364 B-9.18: record actual operations,
stop further invalid use and retire safely; never claim rollback or physical
atomicity. This does not permit ignoring a known pre-execution failure.

---

## SECTION E — Failure Modes

All failures MUST be explicit. This section enumerates required failure behaviors.

### E.1 Graph Cycle Detected

**Behavior:** Hard panic in debug, error return in release

**Message:** Must include cycle path

### E.2 Missing Dependency

**Behavior:** Hard failure before Compile phase

**Message:** Must name missing pass and dependent pass

### E.3 Resource Not Found

**Behavior:** Hard failure

**Message:** Must name resource and requesting pass

### E.4 Format Mismatch

**Behavior:** Hard failure

**Message:** Must include expected format, actual format, and resource name

### E.5 Invalid Pass Executor Access

**Behavior:** Debug assertion failure

**Condition:** Pass executor attempts forbidden operation (graph mutation, SceneCache access, etc.)

---

## SECTION F — Architecture (Subordinate to Invariants)

The following architecture description is **subordinate** to Section B invariants. Any apparent conflict is resolved in favor of the invariants.

### F.1 High-Level Flow

```
SceneGraph + Diff (083)          Timing (041)
              ↓                        ↓
      Compositor v1 (081) aggregates layers
              ↓
     SceneCache Dispatch (083A)
              ↓
        RenderGraph v2 Build Phase
              ↓
        RenderGraph v2 Compile Phase
              ↓
        RenderGraph v2 Execute Phase
              ↓
       Swapchain presentation (030)
```

### F.2 Core Types

```rust
pub struct RenderGraph {
    pub passes: Vec<RenderPassNodeId>,
    pub resources: RenderResources,
}

pub struct RenderResources {
    pub textures: Arena<TextureHandle>,
    pub buffers: Arena<BufferHandle>,
    pub attachments: Arena<AttachmentHandle>,
}

pub struct RenderPassNode {
    pub id: RenderPassNodeId,
    pub name: &'static str,
    pub inputs: SmallVec<[ResourceRef; 4]>,
    pub outputs: SmallVec<[ResourceRef; 4]>,
    pub dependencies: SmallVec<[RenderPassNodeId; 4]>,
    pub kind: RenderPassKind,
}
```

### F.3 Pass Kinds

```rust
pub enum RenderPassKind {
    /// Classic WGPU render pass
    Graphics(RenderPassGraphics),
    /// Compute pass (for effects, SDF, etc.)
    Compute(RenderPassCompute),
    /// Transfer / copy
    Transfer(RenderPassTransfer),
}
```

---

## SECTION G — Resource Model (Subordinate to Invariants)

### G.1 Logical vs Physical

* **Logical resources:** Declared by RenderGraph (size, format, usage)
* **Physical resources:** Backed by WGPU textures/buffers via SPEC-HAL-040 pools

Logical resources are graph-internal. Physical resources are pool-managed.

### G.2 Attachments

```rust
pub struct AttachmentDesc {
    pub size: AttachmentSize,
    pub format: TextureFormat,
    pub samples: u32,
    pub clear_color: Option<Color>,
    pub load_op: LoadOp,
    pub store_op: StoreOp,
}

pub enum AttachmentSize {
    Absolute { width: u32, height: u32 },
    SwapchainRelative { factor_x: f32, factor_y: f32 },
}
```

### G.3 Lifetime Tier Assignment

Per SPEC-HAL-040:

| Resource Type | Default Tier | Notes |
|---------------|--------------|-------|
| Internal attachments | Tier 2 (frame-bound) | Discarded each frame |
| Swapchain attachment | Tier 1 (surface-bound) | Managed by presentation layer |
| Staging buffers | Tier 2 (frame-bound) | Pool-recycled |

---

## SECTION H — Multi-Window Support (Subordinate to Invariants)

Each window has:

* Its own swapchain attachment (INV-RG-009)
* Its own root subgraph
* No shared swapchain state

Implementation options (both valid):

* One aggregated RenderGraph with multiple sinks
* One graph per window, executed sequentially

Requirements:

* Deterministic order per window (INV-RG-004)
* Correct resource aliasing per window
* No cross-window resource sharing without explicit declaration

---

## SECTION I — Debugging Requirements

RenderGraph MUST support:

* Textual dump of all passes and resources
* Dependency graph visualization
* Per-pass resource binding inspection
* Failure message with full context

Optional:

* Debug UI overlay
* Per-pass GPU timing

Debugging capabilities are required for invariant verification.

---

## SECTION J — Testing Requirements

### J.1 Graph Invariant Tests

* Cycle detection MUST trigger hard failure
* Topological ordering MUST be deterministic across runs
* Resource lifetime correctness MUST be verified
* Missing dependency MUST trigger hard failure

### J.2 Rendering Tests

* Identical inputs MUST produce identical outputs
* Multi-window rendering MUST present correct content per window
* Resize MUST not leak resources or corrupt graph

### J.3 Integration Tests

* SceneCache consumption MUST occur only in Build phase
* Compositor output MUST be unmodified by RenderGraph
* Presentation MUST occur after Execute phase

---

## SECTION K — Acceptance Criteria

SPEC-HAL-084 is satisfied when:

* [ ] All INV-RG-* invariants are mechanically enforceable
* [ ] Phase separation is architecturally enforced
* [ ] SceneCache consumption rules are respected
* [ ] Compositor authority is never violated
* [ ] Resource lifetimes follow SPEC-HAL-040 tiers
* [ ] Failures are loud and explicit
* [ ] Graph topology is deterministic
* [ ] All renderer-owned GPU work flows through the graph; admitted SPEC-HAL-295 offscreen production satisfies the isolated producer contract in SPEC-HAL-295A and enters renderer execution only through Image composition.

---

## SECTION L — Authoritative Statements

**AUTH-084-1:** "RenderGraph schedules; it does not decide."

**AUTH-084-2:** "Phases are walls, not guidelines."

**AUTH-084-3:** "Every renderer-owned GPU command has a graph node. An admitted offscreen producer has no renderer-owned command and is represented at composition by its `ImageNode`."

**AUTH-084-4:** "Identical inputs, identical outputs, always."

**AUTH-084-5:** "Failure is information; silence is corruption."

---

## Appendix: Phase Enforcement (Rust Implementation)

Phase boundaries are enforced at the **type level** and through **debug assertions**, not just convention.

### Enforcement Mechanisms

#### 1. Visibility (`pub(crate)`)

Both `DrawPlan` and `CompiledDrawPlan` have all fields marked `pub(crate)`:

```rust
pub struct DrawPlan {
    pub(crate) primitives: Vec<FlatPrimitive>,
    pub(crate) batches: Vec<DrawBatch>,
    // ... other fields
}
```

**Effect:** External code cannot construct these types via struct literals. Only the designated phase owner can create instances.

**Compiler error for external construction:**
```
error[E0451]: fields `primitives`, `batches`, ... of struct `DrawPlan` are private
```

#### 2. Move Semantics (No Clone)

`CompiledDrawPlan` does **not** implement `Clone`:

```rust
pub struct CompiledDrawPlan {
    pub(crate) surface_output: wgpu::SurfaceTexture, // Not Clone
    // ...
}
```

**Effect:** `render()` takes `CompiledDrawPlan` by value (move), consuming it. The same compiled plan cannot be rendered twice.

```rust
pub fn render(&mut self, compiled: CompiledDrawPlan) -> RenderOutput
//                                 ^^^^^^^^^^^^^^^^ moved, not borrowed
```

#### 3. No Public Constructors

Neither type exposes `new()` or `Default`:

* `DrawPlan`: Only constructible by `DrawPlanBuilder::build()`
* `CompiledDrawPlan`: Only constructible by `WgpuRenderer::compile()`

#### 4. Debug Assertions (Phase Ordering)

In debug builds, runtime assertions verify:

```rust
#[cfg(debug_assertions)]
debug_assert!(
    self.last_compile_epoch.get() == Some(current_epoch),
    "SPEC-HAL-084 VIOLATION: render() called without compile()"
);
debug_assert!(
    !self.compile_consumed.get(),
    "SPEC-HAL-084 VIOLATION: render() called twice for same compile()"
);
```

### GPU Write Isolation

All GPU writes (`queue.write_buffer`, `create_buffer`, etc.) occur **only** in `compile()`.

`render()` contains:
* Command encoder creation
* Pipeline/bind group binding
* Draw calls
* **Zero** buffer writes

This is enforced by code structure and documented with explicit comments:

```rust
// SPEC-HAL-084: EXECUTE PHASE — COMMAND ENCODING ONLY
// INV-COMPILE-003: This function performs ZERO buffer writes.
```

### Summary Table

| Mechanism | Prevents |
|-----------|----------|
| `pub(crate)` fields | External construction |
| No `Clone` on `CompiledDrawPlan` | Reuse across frames |
| Move semantics in `render()` | Double consumption |
| Debug assertions | Wrong phase ordering |
| Code structure | GPU writes in execute phase |

## Adopted HAL-388 S1A scoped Build failure contract (2026-11-13)

**Binding, narrowly scoped.** `DrawPlanBuilder<MultiSurfaceContract>::build_multi` and its ordinary presentation-binding companion now have typed CPU Build results `Result<DrawPlan<MultiSurfaceContract>,DrawPlanBuildError>`. The first variant is `UnsupportedLocalAmbientGlowClip { surface_id, node_key, rounded_clip_depth }` under binding HAL-388 §8. A source-local glow admission may be made before an ancestor adds a rounded clip. The complete composed SceneGraph, not the local PaintSink, is the source of actual clip admission. During the Build flattening of the full per-surface retained composition, an effective nonzero glow under any positive-radius rounded ancestor MUST refuse the DrawPlan before any Compile/Execute operation. The existing ShapePipeline is scissor-only for this variant; it cannot silently claim an inherited rounded mask. The full clip stack, including rounded ancestors obscured by a later rectangular entry, is authoritative for this check. HAL-084 INV-RG-003/012 and HAL-012 §11 still govern. Existing WgpuRenderer Compile's `wgpu::SurfaceError` stays reserved for actual GPU/surface failures; do not encode a material admission refusal in it. Production App callers return/report typed Build failure without panicking, presenting a partial graph or substituting another material. Previously working no-glow Build/Compile/Execute behavior is retained. This extension admits no GPU pass, compositor semantics, shader binding or new general refusal service.

## Adopted analytic-surface Build admission S1 (2026-11-13)

HAL-386 section 8 extends the already implemented Result-returning DrawPlanBuilder; it does not add another error bus, Build API or phase. Add these DrawPlanBuildError variants: UnsupportedAnalyticSurfaceClip { surface_id: SurfaceId, node_key: Option<SceneNodeKey>, rounded_clip_depth: usize }; InvalidAnalyticSurfaceClip with the same fields; InvalidAnalyticSurfaceTarget { surface_id: SurfaceId, node_key: Option<SceneNodeKey> }; AnalyticSurfaceCapacityExceeded { surface_id: SurfaceId, limit: usize, requested: usize }.

The complete per-surface composed traversal admits at most one positive-rounded ancestor for the new analytic family, preserving its original scroll-projected rect/radii independently of the rectangular intersection. A second positive-rounded ancestor, invalid clip, invalid actual target/derived geometry or more than 512 visible analytic occurrences in the complete DrawPlan yields the corresponding typed failure before publishing a complete plan or calling Compile. The same retained local Arc must be requalified after parent clip changes. The exact supported domain and first failure evidence are specified in HAL-386 section 8.3. Existing no-analytic scenes, HAL-388 refusal and successful Build results are unchanged.

Compile may allocate/prepare the new pipeline's own bounded buffers through existing renderer resource custody. Execute consumes only its complete frozen commands in existing passes. Compile/SurfaceError is not repurposed for source appearance or clip failures. There is no extra RenderGraph pass, partial-graph success, fallback material, cross-surface source dependency or widget callback in Compile/Execute.

## Adopted current-backdrop graph sequence (HAL-385 section 8, 2026-11-13)

HAL-385 section 8 narrowly admits renderer-owned prefix, snapshot-copy, horizontal/vertical filter, replacement and terminal-transfer nodes for a participating base target. The completed HAL-359 command schedule remains immutable. Every source is a current same-target lower-order prefix on a nonswapchain texture. The real output attachment is a terminal sink only. No participant-owned offscreen exception or ad hoc hidden renderer pass is applicable.

Build topology is CPU-only at the existing renderer topology-build seam. Each producing prefix/replacement has a unique logical AttachmentHandle version; explicit read/dependency edges prove last-reader-before-overwrite before version handles may alias physical accumulation storage. Ordered attachment Load is allowed; simultaneous sampled/copy source use and overwrite is not. The existing graph's final-writer dependency algorithm is preserved, not fed one multiply-written source handle. Pass names remain unique bounded static names. Execute receives declared bindings, immutable ranges and constrained resources, never Device/Queue/pool or presentation authority.

HAL-385 defines exact typed Build and RendererCompileError refusals, budgets, source completeness, current preparation identity and output custody. All affected Compile callers migrate in the same unit; genuine old surface errors retain their original handling inside RendererCompileError::Surface. Other no-frost surfaces keep the original Main/Overlay graph and outputs. Missing mandatory prefix content, unsupported companion graphs or invalid topology must not be converted into apparent successful frosting or a partial executable frame.

## Adopted HAL385 S2 bounded sampling-domain extension (2026-11-14)

HAL385 section11 adds only qualified optical data and extended finite regions to the existing current-prefix graph. The filter output domain Q includes the exact bounded edge-displacement reach and bilinear support; snapshot S includes Q plus the complete Gaussian kernel. Final replacement remains V. The existing unique logical accumulator versions, copy/filter/replacement/terminal sequence, declared resources, dependencies, no-feedback proof and swapchain-sink law remain binding. A second graph service, hidden pass, historical/cross-surface source or weakened failure handling is not admitted.

HAL040's existing bounded pool and HAL385 section9's original-command custody are unchanged. Storage matching and resource admission must account for the actual new Q/S extents and1344-byte records before execution. The unchanged256MiB/768MiB ceilings include every charged resource; overflow refuses explicitly. Existing ordinary no-frost graphs and caller failure semantics remain intact. The new controls do not imply any implementation or pixel PASS.
## 2026-11-18 coordinated scoped RenderGraph admission: HAL-395 [S1 Binding]

Accept the first material-local HDR-source/SDR-Bloom graph of HAL-395 sections 8.2-8.3 and 9.1-9.7 only. Original immutable Scene Build owns material/source admission; AFTER HAL-359 Compile produces the final executable per-SurfaceId command schedule, the existing Compile-owned CPU topology-build seam declares one closed acyclic sequence. At the original material's proven command, execute its original AnalyticSurface draw or completed HAL-385 Frosted replacement, then exactly BloomSource S, BloomHorizontal H, BloomVertical G, BloomComposite into original current-target accumulator A, then resume remaining higher-Z original commands and transfer once to the ordinary swapchain sink. Every renderer-owned stage is a declared graph node, has unique logical resource writer identity, and binds physical backing through existing resource ownership. No destination texture sample, cross-surface read, same-view feedback, ad-hoc submission, new UI phase or compositor semantic decision is admitted. The existing HAL-385 source-prefix, resource/version, companion-graph and current-output rules remain binding. First profile is at most one Bloom per SurfaceId, with exact original-source and Z admission and typed failure. Absence of Bloom retains the existing graph unchanged; no physical HDR output is admitted. This S1 graph decision does not claim executable GPU evidence.

