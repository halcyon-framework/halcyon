# SPEC-HAL-002: **Halcyon Constitution & Architectural Invariants**

### *Foundational Rules for Stability, Safety, and Long-Term Evolution*

**Author:** JC
**Status:** Foundational (Immutable except for explicit amendments)
**Applies To:** Entire Halcyon ecosystem — engine, widgets, renderer, navigation, input, text pipeline, layout, extensions
**Supersedes:** SPEC-HAL-002 (Trading Requirements)

---

# 1. Purpose

This SPEC establishes the **non-negotiable architectural laws** of the Halcyon UI Engine.

It defines:

* What Halcyon **must always do**
* What Halcyon **must never do**
* The **invariants** that protect the engine from corruption, deadlocks, borrow-checker traps, and maintainability collapse
* The **anti-poison rules** guaranteeing Halcyon remains stable as it grows into a full, complex UI system
* The **execution model** that all SPECs, widgets, renderers, and features must obey

This is Halcyon’s **Constitution**.

If any SPEC or component contradicts this document, **this document wins**.

---

# 2. Foundational Principles

These five pillars define Halcyon’s permanent design identity.

### **Pillar A — Retained-Mode Ownership**

Halcyon maintains a **persistent widget tree**, owned by a single root, never fragmented across modules.

### **Pillar B — Explicit Phase Separation**

Every frame follows strict ordered phases:

1. **Process external events**
2. **Apply deferred UI mutations**
3. **Rebuild widget props**
4. **Layout**
5. **Scene build**
6. **GPU paint**

No phase may mutate data owned by another phase.

### **Pillar C — Identity-Through-ID, Not References**

All widgets, screens, nodes, and UI elements are identified through **IDs**, not Rust references.

Persistent references between UI objects are permanently forbidden.

### **Pillar D — Pure Data In, Explicit Actions Out**

The application state (`AppState`) is pure data.
Widgets receive data in read-only form and return mutations through explicit, controlled actions only.

### **Pillar E — GPU-First Rendering Model**

Scene graph must always reflect a stable, deterministic GPU command stream.
No mid-frame mutation of layout or widget tree is allowed.

### **Pillar F — Engine Sovereignty**

Halcyon is the **exclusive authority** over all semantic interpretation.
The operating system is **transport only** — it delivers events and presents frames, but owns no semantics.
No OS callback, platform signal, or external system may directly mutate focus, capture, modality, routing, layout, composition, draw order, or frame loop semantics.

#### Sovereignty Invariants

The following invariants establish Halcyon's exclusive semantic authority. These invariants **take precedence** over any conflicting SPEC text, implementation, or extension. If conflict exists, these invariants govern.

**INV-SOV-OS-TRANSPORT-ONLY**

The operating system layer MUST be treated as transport primitives only:
* Event ingress (raw input delivery)
* Surface allocation (windows, GPU contexts)
* Presentation (frame submission to display)
* Integration hooks (clipboard, IME, accessibility bridges)

The OS MUST NOT be treated as authoritative for any semantic domain.

**INV-SOV-SEMANTIC-OWNERSHIP**

Halcyon is the **exclusive owner** of the following semantic domains:

| Domain | Description |
|--------|-------------|
| Focus | Which widget/window has keyboard focus |
| Capture | Pointer capture state and lifetime |
| Modality | Modal behavior and input blocking |
| Routing | Event dispatch targets and order |
| Layout | All geometry computation (positions, sizes, constraints) |
| Composition | Surface ordering, layering, overlay management |
| Draw Order | Paint order within and across surfaces |
| Frame Loop Semantics | Frame timing, throttling, phase execution |

No external system may claim authority over these domains.

**INV-SOV-NO-OS-FALLBACK**

Halcyon MUST NOT implement "fallback" paths that delegate semantic authority to the OS:
* No "let OS handle focus" paths
* No "let OS manage capture" paths
* No "let OS decide modality" paths
* No platform-specific semantic shortcuts

All semantic decisions MUST be made within Halcyon's engine layer.

**INV-SOV-ADVISORY-SIGNALS**

OS signals (window focus events, activation events, etc.) are **advisory inputs** that Halcyon MAY:
* Accept and project into engine state
* Ignore entirely
* Transform before projection
* Defer to a later frame
* Reinterpret based on engine context

The engine is never obligated to honor OS signals verbatim.

#### Sovereignty Prohibitions

**FORBID-SOV-AUTHORITY-BLEED**

Direct platform callbacks MUST NOT mutate semantic state.

Forbidden patterns:
* OS focus callback directly setting `focused_widget`
* OS activation callback directly modifying capture state
* Platform event handler directly mutating routing tables
* Window manager callback directly changing modal stack

All semantic mutations MUST be performed by engine-owned mechanisms, not directly inside platform callbacks.

**FORBID-SOV-IMPLICIT-SEMANTICS**

Halcyon MUST NOT infer semantic intent from OS window state beyond explicitly projected signals.

Forbidden patterns:
* Assuming "window active = all widgets focusable"
* Assuming "window minimized = pause all interaction"
* Assuming "OS modal flag = engine modal behavior"
* Deriving capture state from OS cursor confinement

Semantic state MUST be explicitly computed from Halcyon's own state model.

#### Sovereignty Precedence and Cross-References

**Precedence Statement**

If any SPEC, implementation, or extension implies OS semantic authority over focus, capture, modality, routing, layout, composition, draw order, or frame loop semantics, that implication is **invalid** and MUST be corrected to comply with this Constitution.

**Dependent Constraints**

The sovereignty invariants are enforced by and constrain:

* **SPEC-HAL-150** — Compositor Authority and Limits: compositor is projection-only, receives authorized geometry, does not own semantics
* **SPEC-HAL-151** — Interaction Domains: focus/capture/routing are Halcyon-scoped, not OS-scoped
* **SPEC-HAL-139** — OS Mediation / Capability Gating: Halcyon requests capabilities; OS mediates access but does not own semantic interpretation
* **SPEC-HAL-030** — Presentation Policy: visual presentation is separate from engine semantics; presentation follows engine authority
* **SPEC-HAL-081B / 081C** — Surface Stack / Overlays: surface ordering is compositor-internal, not OS-managed

**Authority Bleed Examples (Non-Normative)**

The following are examples of authority bleed violations that MUST be prevented:

1. **Focus bleed**: OS `WM_SETFOCUS` directly calling `focus_manager.set_focus(widget_id)` — violates FORBID-SOV-AUTHORITY-BLEED
2. **Capture bleed**: OS `WM_CAPTURECHANGED` directly releasing engine capture state — violates INV-SOV-SEMANTIC-OWNERSHIP
3. **Modal bleed**: OS dialog callback directly pushing to engine modal stack — violates FORBID-SOV-IMPLICIT-SEMANTICS
4. **Routing bleed**: Platform-specific input handler bypassing EventRouter — violates INV-SOV-NO-OS-FALLBACK

---

# 3. Permanent Prohibitions ("Poisons")

These are the actions that **must never occur** anywhere in Halcyon.
Each one is paired with its required antidote in Section 4.

### 3.1 **No Direct References Between UI Elements**

Forbidden:

* Parent pointers
* Child pointers
* Sibling pointers
* Persistent `&mut` or `&` references stored across frames
* Cyclical coupling between widgets or screens

All relationships must be expressed through **stable IDs**.

---

### 3.2 **No Mutation During Active Borrow**

During any:

* layout pass
* event routing pass
* scene building pass
* rendering preparation pass

Mutation of WidgetTree, AppState, layout caches, or navigation stack is forbidden.

All mutations must be deferred.

---

### 3.3 **No Async Mutation of UI State**

No thread, async task, background operation, or external system may mutate:

* AppState
* Widget state
* WidgetTree
* Navigation stack

All cross-thread messages must enter UI via a synchronized **pending events queue**.

---

### 3.4 **No UI Object Owning Another UI Object Outside the Tree**

No widget may:

* Own another widget directly
* Borrow another widget directly
* Reference child state directly
* Store callbacks capturing references to the tree or state

Only the WidgetTree may own widgets.

---

### 3.5 **No Cyclical Lifetimes**

Halcyon must never create scenarios where:

* A depends on B
* B depends on A

in a way that forces shared mutable access.

This applies to:

* widgets
* screens
* scene nodes
* input handlers
* transition controllers
* layout engines
* render pipelines

---

### 3.6 **No Cross-Phase Leaking of Borrows**

A borrow taken during layout must not survive into paint.

A borrow taken during event routing must not survive into rebuild.

All borrows must be ephemeral to a single phase.

---

### 3.7 **No Direct UI Mutation From Callbacks**

Widget callbacks must **never** mutate other widgets or the tree directly.

They may:

* mutate their own internal state
* mutate AppState
* enqueue navigation or tree mutations

They may not:

* navigate directly
* reparent nodes directly
* modify sibling widgets
* modify neighbors in layout

---

### 3.8 **No Instant Reparenting or Refactoring Mid-Frame**

Structural operations such as:

* insert widget
* remove widget
* reparent widget
* move subtree

must only occur **between frames**, at the beginning of the rebuild pass.

---

### 3.9 **No Persistent Access to Ephemeral Layout Structures**

Paragraph layout, text runs, glyph clusters, and computed geometry are **ephemeral**, and must never be stored outside the frame they were generated.

Selections, carets, and editing must rely on:

* indices
* ranges
* offsets

Never references.

---

### 3.10 **No Hidden Global State**

No singletons controlling:

* layout
* theme
* events
* input
* scene
* fps state

All must be explicit or passed through context.

---

### 3.11 **No Interpretive Intent Resolution**

Widgets MUST NOT reinterpret, normalize, infer, or reconcile user intent.

User intent is defined solely by explicit user actions.
The widget's role is to surface options and accept commands — not to decide
which command the user "meant".

Forbidden behaviors include (but are not limited to):

- Auto-matching typed input to existing values at commit time
- Case normalization or equivalence collapsing without explicit user action
- Silent substitution of a selected value for typed input
- Choosing between multiple intent sources based on heuristics
- "Smart defaults" applied at the moment of commit

If multiple intent sources exist (e.g. typed input and list selection),
the widget MUST require an explicit user action to disambiguate.

If intent is ambiguous, the widget MUST block or defer.
If intent is explicit, the widget MUST obey verbatim.

---

# 4. Required Antidotes (Permanent Remediation Rules)

For every poison listed above, here is the enforced antidote.

These are not recommendations.
They are constitution-level rules.

---

## 4.1 IDs Instead of References

All components use:

* `WidgetId`
* `NodeId`
* `ScreenId`
* `FontId`
* `GlyphId`

References (`&T`, `&mut T`) must never persist outside the frame in which they were created.

---

## 4.2 Deferred Mutation Queue

Halcyon must define a queue for:

* UI mutations
* navigation events
* screen pushes/pops
* tree restructuring
* focus change
* selection change

These are only executed during **Phase 2** (Deferred Mutations).

---

## 4.3 Controlled AppState Mutations

Mutations to AppState must occur exclusively through:

```
cx.update(|state: &mut AppState| { ... })
```

No widget may hold a mutable reference to AppState outside this closure.

---

## 4.4 Navigation via Explicit Events

Screens communicate via:

* `NavEvent::Push(ScreenId)`
* `NavEvent::Pop`
* `NavEvent::PopTo(usize)`
* `NavEvent::Replace(ScreenId)`
* `NavEvent::CustomPayload(...)`

Screens **never** store references to each other.
Screens **never** mutate the navigation stack directly.

The navigation manager applies changes **after** event routing.

---

## 4.5 Animation & Transition Isolation

Transitions may modify:

* an **animation state buffer**
* not the WidgetTree directly
* not layout directly
* not paint buffers during traversal

Rendering uses a snapshot of the animation buffer.

**Authority Phase Separation (SPEC-HAL-145):**

* Layout, Transition, and Paint are **mutually exclusive authority phases**
* Transition interpolation is projection-only and non-authoritative
* See SPEC-HAL-141/145 for enforcement details

---

## 4.6 WidgetTree as the Single Owner

The widget tree:

* owns all widget instances
* owns their internal state
* assigns stable IDs
* rebases state across rebuilds
* forbids double ownership

No external module may store widget instances.

---

## 4.7 Layout and Paint Are Pure Functions

Given:

* WidgetTree snapshot
* AppState snapshot
* theme snapshot
* screen stack

Layout and paint must behave deterministically and must not mutate shared data.

---

## 4.8 Text Pipeline Ephemerality

Text layout (048, 050, 104):

* must be rebuilt every time text changes
* must not persist cross-frame
* may be cached only via stable keys, not references
* selection/caret logic must rely on indices only

---

## 4.9 All Structural Changes Must Respect Identity

When widgets move:

* identity must be tracked by keys or stable assignment
* state must carry over
* no implicit dynamic remapping

Rebuild must resolve identity deterministically.

---

## 4.10 Rendering Snapshot Isolation

During `paint` → `scene build`:

* No widget state may mutate
* Renderer uses frozen geometry
* All scene graph nodes are immutable once built for the frame

GPU submission is read-only.

---

## 4.11 Explicit Commit Authority

All commits MUST originate from an explicit user command:

- Pointer activation (click / tap)
- Keyboard activation (Enter, Confirm)
- Programmatic command routed through Context

Widgets MAY assist intent formation (highlighting, filtering, suggestions),
but assistance MUST remain reversible.

Commits are irreversible and must never be inferred.

---

# 5. Frame Execution Model (Immutable)

Every frame must follow **exactly** this order:

```
Phase 1: OS Event Intake
Phase 2: Deferred Mutations Applied
Phase 3: Rebuild Widgets from AppState
Phase 4: Layout Pass
Phase 5: Scene Graph Build
Phase 6: GPU Render Pass
```

### Forbidden:

* Mutating during layout
* Mutating during paint
* Mutating navigation during event routing
* Mutating AppState outside explicit update callbacks

---

# 6. Navigation & Multi-Screen Rules

Navigation stack is governed by:

* Pure data
* Event dispatch
* Rebuild-from-data pattern

Screen A **must never** hold references to Screen B.
Whenever B needs to go back to A, it issues:

```
cx.nav().request(NavEvent::PopTo(step))
```

Or:

```
cx.nav().request(NavEvent::Push(new_screen))
```

Screens are rebuilt fresh from AppState.

---

# 7. Text Editing & Document Rules

All advanced text interaction must respect:

* no references to text runs
* no references to glyph clusters
* selections use indices
* IME composition uses ephemeral data
* all shaping results are frame-local
* baselines and glyph quads are pixel-stable via SPEC-HAL-104

---

# 8. Render Pipeline Invariants

The renderer must guarantee:

* stable ordering
* batching by material
* no mid-draw mutation
* scene graph immutability within a frame
* pixel-snapped text as per SPEC-HAL-104
* **never** read from WidgetTree during GPU submission

---

# 9. Allowed Extensions (Future-Proofing)

Any future component must obey:

* Screens
* Navigation
* Widgets
* Animations
* Layers
* Effects
* Text pipelines
* Theming
* Accessibility
* External render surfaces

All must route through **Context**, **AppState**, **IDs**, and **deferred mutation**.

No extension may introduce back-pointers or cross-phase mutation.

---

# 10. Violations and Amendment Process

Any SPEC or implementation that conflicts with this Constitution:

* Must be corrected or amended.
* Changes to this document require an **explicit constitutional amendment**.
* Amendments require reviewing all dependent SPECs for consistency.

This ensures Halcyon never drifts into architectural decay.

---

# 11. Conclusion

This SPEC is Halcyon’s spine.

It ensures:

* Total borrow-checker safety
* Long-term maintainability
* Arbitrary future features
* Robustness under extreme complexity
* Stability under high performance load
* Predictability under complex navigation
* Safety under async data flows

This is the **permanent governing document** of the Halcyon UI Engine.
