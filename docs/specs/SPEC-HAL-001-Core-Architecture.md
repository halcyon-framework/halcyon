# SPEC-HAL-001: **Core Architecture**

### *Definitive Specification of the Halcyon UI Engine*

**Author:** JC
**Status:** Foundational
**Depends on:**

* **SPEC-HAL-002** (Constitution & Architectural Invariants)
* **SPEC-HAL-002A** (Amendment Guidelines)
* SPEC-HAL-018 (App/Screen framework)
* SPEC-HAL-007 (State Model)
* SPEC-HAL-009 (Event Routing)
* SPEC-HAL-024/026 (Coordinate & Layout Invalidation Models)
* SPEC-HAL-047–067 (Text & Font System)
* SPEC-HAL-092 (Unified Typography Rules)
* SPEC-HAL-103/104 (Font Metrics & Pixel Stability)

---

# 1. Purpose

This SPEC defines the **core architectural model** of the Halcyon UI Engine.

It documents:

* The structural identity of Halcyon
* How apps are organized
* How screens and navigation function
* How the widget tree is formed
* How state flows through the system
* How phases are orchestrated
* How safety and determinism are enforced
* How rendering is derived from pure state

This SPEC is the **central authoritative overview** that all later SPECs refine.

---

# 2. Architectural Overview

Halcyon is a **retained-mode, GPU-first UI framework** designed for:

* correctness
* determinism
* architectural safety
* extremely high rendering stability
* high-performance text systems
* complex application flows

The architecture is built around **Constitutional invariants** (SPEC-HAL-002) that prevent all known failure modes of Rust UI engines.

At the highest level, Halcyon is composed of:

1. **App** (root controller, owns state and navigation)
2. **Screens** (top-level UI containers)
3. **WidgetTree** (retained-mode UI structure)
4. **Phased engine** (rebuild → layout → scene → paint)
5. **Scene graph + renderer**
6. **Event routing & input fusion**
7. **Font, text, and typography pipelines**

This document describes how these parts interrelate.

---

# 2.5 Sovereignty Boundary (Binding)

Halcyon operates under a strict **sovereignty model** that defines the boundary between OS/platform primitives and engine-owned semantics.

### 2.5.1 OS Role: Transport Only

The operating system and platform layer MUST be treated as **transport primitives only**:

* **Event ingress** — OS delivers raw input events (pointer, keyboard, touch, focus signals)
* **Surface allocation** — OS provides drawable surfaces (windows, GPU contexts)
* **Presentation** — OS presents rendered frames to display
* **Integration hooks** — OS provides clipboard, IME, accessibility bridges

The OS role is **non-authoritative**. OS signals are advisory inputs that Halcyon MAY ignore, transform, defer, or reinterpret.

### 2.5.2 Halcyon Role: Exclusive Semantic Authority

Halcyon is the **exclusive owner** of all semantic interpretation within the engine. The following domains are Halcyon-owned:

| Semantic Domain | Authority |
|-----------------|-----------|
| **Focus** | Halcyon determines which widget/window has focus |
| **Capture** | Halcyon owns pointer capture state and lifetime |
| **Modality** | Halcyon defines modal behavior and input blocking |
| **Routing** | Halcyon decides event dispatch targets and order |
| **Layout** | Halcyon computes all geometry (positions, sizes, constraints) |
| **Composition** | Halcyon orders surfaces, layers, and overlays |
| **Draw Order** | Halcyon determines paint order within and across surfaces |
| **Frame Loop Semantics** | Halcyon controls frame timing, throttling, and phase execution |

No OS callback, platform signal, or external system may directly mutate these semantic domains.

### 2.5.3 Precedence

If any SPEC, implementation, or extension implies OS semantic authority over any domain listed above, that implication is **invalid** under SPEC-HAL-002 (Constitution).

### 2.5.4 Dependent SPECs

The sovereignty boundary constrains and is enforced by:

* **SPEC-HAL-150** — Compositor authority limits (compositor is projection-only)
* **SPEC-HAL-151** — Interaction domains (focus scoped to Halcyon, not OS)
* **SPEC-HAL-139** — OS mediation / capability gating (Halcyon requests capabilities; OS mediates but does not own semantics)
* **SPEC-HAL-030** — Presentation policy (visual presentation separate from engine semantics)
* **SPEC-HAL-081B / 081C** — Surface stack / overlays (compositor-internal, not OS-managed)

---

# 3. The App

The **App** is the root of the entire UI system.

Responsibilities:

* Own the global `AppState`
* Maintain the `NavigationStack`
* Execute all phase orchestration
* Accept OS events
* Schedule deferred mutations
* Host global services (fonts, theme, time, etc.)
* Provide a stable root widget (AppRoot)
* Trigger frame rendering

The App never stores widget references.
It stores:

* IDs
* State
* Screen stack
* Global environment

---

# 4. Screens

A **Screen** is a top-level view.
Only one screen is active at a time, but the stack may contain many.

Screens:

* Are pure factories: `(state) -> WidgetTree`
* Do not store references to other screens
* Communicate solely via navigation events
* May mutate AppState during callbacks
* Are rebuilt whenever navigated to

Navigation is entirely **event-driven**, per SPEC-HAL-018 and Constitution.

Screens must obey:

* no persistent references to parents
* no mutation outside callbacks
* no direct access to other screens
* no cross-tree references

Screens render via a root widget just like any other UI node.

---

# 5. WidgetTree (Retained-Mode UI Structure)

The WidgetTree is Halcyon’s persistent UI structure.

### Properties:

* Owned exclusively by the App
* Indexed by stable `WidgetId`
* Nodes store widget instances and widget-local state
* Rebuilt partially or fully every frame depending on dirtiness
* Used for:

  * event routing
  * layout
  * scene building

This tree is **not a DOM**.
There are no pointers — only IDs.

Conforms to constitutional rules:

* No parent references inside nodes
* No sibling references
* No persistent borrows across frames
* All relationships via integer IDs

Widget instances are purely retained-mode objects with:

* props
* internal state
* children
* identity key

---

# 6. State Model

Halcyon uses a **unidirectional state flow**:

### 6.1 Global State (`AppState`)

* Application-level data
* Shared across screens
* Mutated through explicit `cx.update(...)` calls
* Never mutated implicitly

### 6.2 Widget Local State

* Lives inside the WidgetTree
* Identified by `WidgetId`
* Survives rebuilds
* Immutable during layout and paint
* Only mutated during designated state update phase

### 6.3 Ephemeral State (layout/text/etc.)

* Recreated each frame
* Not stored persistently
* Not referenced by widgets
* Not accessible outside their phase

This model ensures deterministic updates and safe memory behavior.

---

# 7. Event System & Input Fusion

Events flow:

1. OS input events arrive
2. InputFusion normalizes them
3. EventRouter dispatches them to widgets using:

  * hit-testing
  * focus model
  * routing rules
4. Widget callbacks may:

  * update local state
  * update AppState
  * emit navigation events
  * request tree mutations

**Direct tree mutation is forbidden** outside the mutation queue.

Event-handling must be:

* synchronous
* pure except for controlled mutations
* order-deterministic

---

# 8. Navigation Model

Navigation is a **stack**:

```
ScreenStack = Vec<ScreenInstance>
```

Screens push, pop, or replace via **NavEvents**, not direct calls.

Screens themselves:

* never hold references to one another
* are rebuilt fresh from AppState when navigated to
* cannot mutate navigation during layout or paint

Valid actions:

* `Push(ScreenId)`
* `Pop`
* `PopTo(index)`
* `Replace(ScreenId)`
* `Custom(...)`

Navigation events are applied only during Deferred Mutation phase.

---

# 9. The Frame Phases

Every frame is organized strictly as defined in SPEC-HAL-002:

### **Phase 1 — Intake**

Raw OS events captured.

### **Phase 2 — Deferred Mutations**

All requested mutations applied:

* navigation
* widget tree edits
* internal state transitions
* async results applied safely

### **Phase 3 — Rebuild**

Widgets rebuilt from:

* AppState
* old retained children
* props

Rebuild is deterministic and identity-based.

### **Phase 4 — Layout**

Layout engine computes:

* sizes
* positions
* constraints
* baselines
* scroll ranges

No mutation allowed.

### **Phase 5 — Scene Build**

WidgetTree generates:

* draw commands
* text runs
* vector shapes
* quad lists

Scene graph is immutable once built.

### **Phase 6 — GPU Render**

Renderer consumes scene graph.
No tree access allowed.

---

# 10. Renderer Overview

Rendering follows these rules:

* scene graph is immutable per-frame
* text quads follow pixel-stability rules (104)
* batching by brush/material
* GPU passes deterministic
* shader inputs never mutate widget state
* renderer never accesses AppState directly

The renderer must always operate on a **frozen snapshot**.

---

# 11. Text & Typography Pipeline (Foundational Summary)

Halcyon’s text engine integrates:

* HarfBuzz shaping (SPEC-050)
* Paragraph layout engine (SPEC-048)
* Font management (047–051)
* Pixel-stable metrics (103 & 104)
* Unified typography rules (092)
* Glyph atlas + subpixel model (066–067)
* MacOS/CoreText and system fonts (047, 103)

All text layout structures are **ephemeral**.
All formatting is **pure**.
No widget stores references to paragraph internals.

Quad construction follows:

* physical pixel rounding
* atlas offset baking
* subpixel binning when required
* strict coordinate normalization

---

# 12. Identity Model

Every widget instance has:

* a stable `WidgetId`
* internal retained state
* optional identity key for reusing state across rebuilds
* deterministic parent-child order

Identity must remain stable:

* across rebuilds
* across screen transitions
* across async updates

Widget IDs must never be reused within a single rebuild cycle.

---

# 13. Coordination with Constitution (SPEC-HAL-002)

SPEC-HAL-001 must always be interpreted in compliance with SPEC-HAL-002:

* No cross-widget references
* No mutation during traversal
* Deferred mutation only
* Navigation must be event-driven
* Layout and paint must be pure
* Ephemeral types must not escape their phases
* WidgetTree is the sole owner

Any ambiguity must be resolved in favor of the Constitution.

---

# 14. Extensibility and Future Systems

Halcyon must remain compatible with:

* portals
* popups
* overlays
* composition layers
* docking systems
* multi-window output
* embedded surfaces
* 3D backdrops
* accessibility nodes

All extensions must obey constitutional invariants:

* ID-based identity
* clean phase separation
* deferred mutation
* ephemeral layout products
* snapshot-based rendering

No extension may weaken SPEC-HAL-002.

---

# 15. Conclusion

SPEC-HAL-001 defines the **complete, modern core architecture** of Halcyon:

* App as conductor
* Screens as pure factories
* WidgetTree as retained state owner
* Unidirectional state flow
* Event-driven navigation
* Pure functional layout
* Immutable scene graph
* GPU-first rendering model
* Text pipeline designed for absolute pixel stability
* Constitution guarding architectural safety

This SPEC forms the stable foundation for the entire system.
