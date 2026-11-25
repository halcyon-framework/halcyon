# SPEC-HAL-120

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**Tree Expand / Collapse Animation Semantics**

**Status:** Design — Approved  
**Scope:** Optional visual animation for tree expansion state changes  
**Applies to:** Tree, TreeItem (non-virtualized and virtualized)  
**Depends on:** SPEC-HAL-116, SPEC-HAL-119  
**Non-Goals:** Interaction timing, input handling, layout authority, virtualization mechanics

---

## 1. Purpose

Tree expand/collapse animations improve **spatial continuity** and **user comprehension**
but are a common source of subtle bugs when they leak into:

- layout
- hit-testing
- focus
- selection
- scroll anchoring

This SPEC defines **strictly visual, non-authoritative animation semantics**
for Tree expand/collapse transitions.

Animations are **optional** and **purely decorative**.

---

## 2. Core Principle

> **Expansion state is instantaneous. Animation is a visualization of that state change — not the change itself.**

State changes happen immediately. Animation reflects them asynchronously.

---

## 3. Core Invariants

### INV-A1 — State Before Animation

On expand or collapse:

- Tree expansion state MUST update immediately
- Visible-node ordering MUST update immediately
- Selection and focus rules MUST execute immediately (per SPEC-HAL-116)

Animation MAY start afterward, but MUST NOT delay or alter state transitions.

---

### INV-A2 — Animation Is Non-Interactive

During animation:

- Hit-testing uses **post-state geometry**
- Selection logic uses **post-state node visibility**
- Keyboard navigation follows **post-state visible order**
- Scroll computations use **post-state content size**

No transitional geometry is authoritative.

---

### INV-A3 — No Geometry Ownership

Animations:

- MUST NOT influence layout calculations
- MUST NOT alter item rects
- MUST NOT change indentation
- MUST NOT introduce intermediate hit zones

All geometry is owned by layout and Tree logic.

---

### INV-A4 — Input Immunity

Animations MUST NOT:

- intercept pointer events
- change cursor shape
- delay focus changes
- block keyboard input
- suppress scroll events

Input is processed normally throughout animation.

---

## 4. Expand Animation Semantics

### INV-E1 — Expand Visualization

On expand:

- Descendant nodes become logically visible immediately
- Animation MAY visually reveal nodes (e.g. fade, slide, clip)
- Nodes MUST be hit-testable immediately, even if mid-animation

---

## 5. Collapse Animation Semantics

### INV-C1 — Collapse Visualization

On collapse:

- Descendant nodes become logically hidden immediately
- Selection relocation rules apply immediately (SPEC-HAL-116)
- Animation MAY visually hide nodes after state change

Hidden nodes MUST NOT be focusable or selectable during animation.

---

## 6. Scroll Stability

### INV-S1 — Scroll Anchor Preservation

Expand/collapse animations:

- MUST NOT change scroll offset implicitly
- MUST NOT auto-scroll to reveal new nodes
- MUST preserve scroll anchor chosen by scroll container

Any scroll adjustment requires explicit user input.

---

## 7. Virtualization Compatibility

### INV-V1 — Virtual-Safe Animation

Animations:

- MUST operate on realized items only
- MUST tolerate item recycling
- MUST NOT assume continuous presence of child widgets

VirtualizedTreeView MUST be able to disable animations entirely.

**363/364 scoped amendment:** A realized Tree/member paint source MAY be captured under
SPEC-HAL-363/364 using its existing stable owner-qualified member address and
an owned immutable paint-value closure. This is not an ordinary SPEC-HAL-078
transition of a recycled widget rect. It SHALL NOT query unrealized geometry,
force offscreen realization, retain a RowProjection or create a WidgetId.
Recycling invalidates current bindings; already-certified historical generations
remain only under their independent bounded lifetime.

---

## 8. Configuration

### INV-CFG1 — Explicit Enablement

Tree expand/collapse animations:

- MUST be disabled by default
- MUST require explicit enable flag
- MUST support complete disable at runtime

---

## 9. Forbidden Behavior

Animations MUST NOT:

- delay state mutation
- encode interaction state
- imply clickability
- affect accessibility semantics
- depend on frame timing for correctness

Any such dependency is invalid.

---

## 10. Responsibility Boundaries

### Tree Owns:

- expansion state
- visible node order
- selection and focus rules

### Animation System Owns:

- visual interpolation
- timing curves
- clipping or masking
- fade / slide effects

Animation system MUST NOT mutate Tree state.

**363/364 scoped amendment:** Tree retains expansion, visible order, focus/selection and semantic
lifetime. SPEC-HAL-363 coordinates the single Timeline parameter and group
lifetime; SPEC-HAL-364 owns occurrences, coverage and execution admission.
Visual masks never become Tree hit/scroll geometry. SPEC-HAL-078A/078B ordinary
virtualization/topology denials remain; the pair supplies only an explicitly
admitted member-source route. Section 8 default-off, explicit enablement and
runtime disable remain unchanged.

---

## 11. Test Requirements

Any implementation claiming compliance MUST test:

- state changes occur before animation
- hit-testing correctness during animation
- focus stability during animation
- selection invariants during animation
- scroll stability during animation
- virtualization compatibility

---

## 12. Reference Implementation

Planned approach:

- Tree painter hook after state update
- Optional animation layer wrapping TreeItem visuals
- Disable flag respected by virtualized trees

---

## 13. Future Work (Explicitly Out of Scope)

- Physics-based animation
- Per-node animation customization
- Animated structural guides
- Framework motion-policy handling for SPEC-HAL-363/364 follows
  SPEC-HAL-363 §4's explicit policy boundary. Tree-specific preference UI,
  platform mapping and unadopted timing defaults are not supplied here.

---

## 14. Final Note

Tree animations exist to **support cognition**, not interaction.

If disabling animation changes behavior,
the implementation is **architecturally incorrect**.
