# SPEC-HAL-121

**Tree Drag-and-Drop Reordering Contract**

**Status:** Design — Approved  
**Scope:** Structural reordering of tree nodes via pointer drag-and-drop  
**Applies to:** Tree, TreeItem (non-virtualized); VirtualizedTreeView (extension)  
**Depends on:** SPEC-HAL-116 (Tree & TreeItem Interaction Model), SPEC-HAL-110 (Scroll Overlay & Viewport)  
**Non-Goals:** Visual styling, animations, auto-layout heuristics, cross-widget drag-and-drop

---

## 1. Problem Statement

Tree drag-and-drop is a **high-risk interaction** that frequently corrupts state due to:

* ambiguous drop targets
* implicit reparenting rules
* accidental selection mutation
* focus loss during drag
* scroll interference during drag

This SPEC defines a **strict, invariant-based contract** for tree node reordering that prevents structural ambiguity and guarantees deterministic outcomes.

If an implementation violates this contract, it is **incorrect by definition**.

---

## 2. Core Concepts

### 2.1 Dragged Node

The **logical node** being moved.  
Identified by stable `NodeId`.

* Dragged node MAY represent a subtree.
* Dragged node identity MUST NOT change during drag.

### 2.2 Drop Target

A **resolved structural destination**, not a pixel location.

Drop target is one of:

1. **Before(NodeId)**
2. **After(NodeId)**
3. **Inside(NodeId)**

No other targets are permitted.

---

## 3. Drag Initiation Invariants

### INV-DND1 — Explicit Drag Start

A drag MUST start only when:

* primary pointer button is pressed
* pointer moves beyond drag threshold
* press originated on a TreeItem content rect

Expand/collapse hit zones MUST NOT initiate drag.

---

### INV-DND2 — Selection Freeze

On drag start:

* selection set is **frozen**
* focus is **frozen**
* no selection mutation allowed until drag completes or cancels

Dragging MUST NOT implicitly select additional nodes.

---

## 4. Valid Drag Scope

### INV-DND3 — Self-Containment Prohibition

A node MUST NOT be dropped:

* into itself
* into any of its descendants

Such drop targets are **invalid** and MUST be rejected.

---

### INV-DND4 — Root Constraints

Root-level constraints:

* dropping before root MAY be forbidden (configurable)
* dropping inside root MUST be explicit
* root identity MUST remain stable

---

## 5. Drop Target Resolution

### INV-DND5 — Single Authoritative Target

At any time during drag:

* at most **one** drop target may be active
* drop target MUST be fully resolved (Before / After / Inside)

Ambiguous targets are FORBIDDEN.

---

### INV-DND6 — Geometry Is Advisory Only

Visual indicators (insertion lines, highlights):

* are advisory
* MUST map to a valid structural target
* MUST NOT introduce new semantics

Behavior is driven by structure, not pixels.

---

## 6. Scroll Interaction During Drag

### INV-DND7 — Scroll Isolation

While dragging:

* vertical auto-scroll MAY occur near viewport edges
* horizontal scroll MUST NOT auto-trigger
* scroll MUST NOT change drop semantics

Scroll follows SPEC-HAL-110.

---

## 7. Drop Commit Semantics

### INV-DND8 — Atomic Commit

On drop:

* reordering MUST be atomic
* either fully applied or not applied at all
* no intermediate states allowed

If drop target invalid → no-op.

---

### INV-DND9 — Event Emission

On successful drop:

* emit `on_reorder { node, old_parent, new_parent, index }`
* emit exactly once

Programmatic reorders MAY suppress events (`silent: bool`).

---

## 8. Cancellation Semantics

### INV-DND10 — Cancellation Is Lossless

If drag is cancelled:

* tree structure MUST remain unchanged
* selection and focus MUST be restored exactly
* no events emitted

Escape key MUST cancel drag.

---

## 9. Keyboard Interaction (Optional)

### INV-DND11 — Keyboard Reordering (Optional)

If implemented:

* explicit modifier required (e.g. Ctrl+Arrow)
* MUST reuse same structural rules as pointer drag
* MUST emit same events

No implicit keyboard reordering allowed.

---

## 10. Visual Feedback Contract

### INV-DND12 — Non-Interactive Indicators

Drag indicators:

* MUST NOT participate in hit-testing
* MUST NOT steal focus
* MUST NOT alter layout

Indicators are strictly visual.

---

## 11. Responsibility Boundaries

### Tree Owns:

* structural validation
* drop target resolution
* invariant enforcement
* event emission

### TreeItem Owns:

* drag start eligibility
* visual feedback hooks

### Tree MUST NOT:

* compute layout during drag
* mutate selection implicitly
* infer semantics from visuals

---

## 12. Forbidden Behavior

Tree drag-and-drop MUST NOT:

* auto-expand nodes on hover
* reorder on hover
* change selection during drag
* allow partial subtree moves
* emit multiple reorder events
* depend on pixel-perfect cursor position

---

## 13. Test Requirements

Any implementation MUST include tests for:

* self-drop rejection
* descendant-drop rejection
* atomic commit
* cancellation restoration
* scroll during drag
* event emission correctness

---

## 14. Reference Implementation

Planned widget: `Tree` (non-virtualized)  
Future extension: `VirtualizedTreeView` (SPEC-HAL-118)

---

## 15. Future Work (Out of Scope)

* Cross-tree drag-and-drop
* Dragging external data
* Animated reparenting
* Accessibility drag narration

---

## 16. Final Note

Tree drag-and-drop is **structural surgery**, not a gesture.

Any implementation that treats it as a visual trick rather than a constrained state transition is broken — even if it “looks fine”.
