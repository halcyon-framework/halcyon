# SPEC-HAL-119

**Tree Visual Affordances & Structural Guides**

**Status:** Design — Approved  
**Scope:** Visual-only tree structure aids  
**Applies to:** Tree, TreeItem (non-virtualized and virtualized)  
**Depends on:** SPEC-HAL-116  
**Non-Goals:** Interaction semantics, hit-testing, layout ownership, virtualization logic

---

## 1. Purpose

Tree widgets benefit significantly from **visual structure cues** that communicate hierarchy,
ancestry, and grouping.

This SPEC formalizes **purely visual affordances** such as:

- tracing lines
- dotted or solid guides
- junction / elbow indicators
- ancestry continuation lines

without allowing them to interfere with **interaction, geometry, or state**.

This SPEC exists to **permit rich tree visuals without reopening behavioral ambiguity**.

---

## 2. Definitions

### 2.1 Structural Guide

A **Structural Guide** is a visual element rendered to convey hierarchy or ancestry,
including:

- vertical continuation lines
- horizontal elbow connectors
- depth indicators
- guide dots or ticks

Structural guides are **decorative only**.

---

## 3. Core Invariants

### INV-VIS1 — Decoration Only

Structural guides:

- MUST NOT participate in hit-testing
- MUST NOT receive hover state
- MUST NOT receive focus
- MUST NOT consume pointer events
- MUST NOT affect keyboard navigation
- MUST NOT alter selection behavior

They are inert by definition.

---

### INV-VIS2 — Geometry Independence

Structural guides:

- MUST NOT influence layout
- MUST NOT affect indentation calculation
- MUST NOT shift content rects
- MUST NOT modify item height or width

All interactive geometry is defined exclusively by SPEC-HAL-116.

---

### INV-VIS3 — Z-Ordering

Structural guides:

- MUST render **behind** interactive elements
- MUST NOT occlude selection, focus, or expand/collapse indicators
- MUST NOT overlap hit zones in a way that causes ambiguity

---

### INV-VIS4 — State Independence

Structural guides:

- MUST NOT change appearance based on selection or focus
- MAY change appearance based on expansion state (ancestor visibility)
- MUST NOT animate in response to interaction unless explicitly enabled

---

## 4. Styling & Theming

### INV-T1 — Style Delegation

The following are **theming concerns**, not behavioral concerns:

- line style (solid, dashed, dotted)
- thickness
- color
- opacity
- anti-aliasing

Disabling all structural guides MUST NOT affect tree behavior.

---

## 5. Expansion Awareness

### INV-E1 — Visibility Coupling

Structural guides MAY reflect expansion state:

- guides for collapsed descendants MAY be hidden
- guides for visible ancestry MAY be continuous

This MUST be derived solely from Tree expansion state defined in SPEC-HAL-116.

---

## 6. Virtualization Compatibility

### INV-VIRT1 — Virtual-Safe Rendering

Structural guides:

- MUST be computable from logical tree state
- MUST NOT rely on realized widget neighbors
- MUST be derivable per-item without access to off-screen nodes

This ensures compatibility with SPEC-HAL-118.

---

## 7. Forbidden Behavior

Structural guides MUST NOT:

- initiate expand/collapse
- respond to clicks
- alter cursor shape
- imply interactivity
- encode selection state
- encode focus state

Any such behavior is a violation.

---

## 8. Responsibility Boundaries

### Tree Owns:

- logical hierarchy
- expansion state
- visible node order

### TreeItem Owns:

- content rect rendering
- expand/collapse affordance rendering

### Structural Guide Renderer Owns:

- guide geometry computation
- guide rendering
- style application

Structural guide renderers MUST NOT access or mutate Tree state.

---

## 9. Test Requirements

Any implementation claiming compliance MUST test:

- guides do not receive hit events
- guides do not affect selection
- guides do not affect layout
- guides remain correct under expand/collapse
- guides render correctly with virtualization enabled

---

## 10. Reference Implementation

Planned integration:

- Tree painter extension
- Shared guide computation utility
- Optional enable flag on Tree

---

## 11. Future Work (Explicitly Out of Scope)

- Animated guide transitions
- Git-style branch graphs
- Drag-and-drop reparenting visuals
- Inline tree diff indicators

These require separate SPECs.

---

## 12. Final Note

Structural guides exist to **help the eye**, not the input system.

If a tree works differently when guides are enabled,
the implementation is **incorrect by definition**.
