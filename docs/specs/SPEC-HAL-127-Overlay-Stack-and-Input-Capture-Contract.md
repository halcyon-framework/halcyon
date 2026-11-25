# SPEC-HAL-127

**Overlay Stack & Input Capture Contract**

**Status:** Implemented
**Scope:** Z-ordered overlays, modal layers, and exclusive input capture
**Applies to:** Dropdown popup, Menu, Tooltip, Popover, Dialog, ContextMenu, OverlayRoot
**Layer:** Input routing / Focus / Event arbitration
**Depends on:**

* SPEC-HAL-126 (Focus Traversal & Tab Order Contract)
* SPEC-HAL-110 (Unified Scroll Overlay & Viewport Contract)

**Non-Goals:**

* Visual styling or animation
* Layout of overlay contents
* Accessibility narration (SPEC-HAL-125)
* Drag-and-drop semantics (SPEC-HAL-121)

---

## 1. Problem Statement

Overlays fail when input routing is **implicit**, **distributed**, or **order-dependent**, leading to:

* clicks leaking through popups
* focus escaping modals
* scroll events reaching background widgets
* inconsistent dismissal behavior

This SPEC defines a **single authoritative overlay stack** and a **strict input-capture model**.

Any widget bypassing this contract is **incorrect by definition**.

---

## 2. Core Concepts

### 2.1 Overlay

An overlay is a widget layer that:

* is rendered above normal content
* participates in global input routing
* may restrict or capture input

Examples:

* dropdown popup
* context menu
* modal dialog
* tooltip (non-capturing)

---

### 2.2 Overlay Stack

A **global, ordered stack** of active overlays.

Properties:

* strictly ordered (bottom → top)
* topmost overlay is the **primary input owner**
* stack is managed centrally (OverlayRoot)

---

### 2.3 Input Capture

A state where:

* all pointer and keyboard input is routed exclusively to one overlay
* underlying widgets receive nothing

Capture is **explicit**, never implicit.

---

## 3. Overlay Stack Invariants

### INV-OV1 — Single Authoritative Stack

There MUST be exactly one overlay stack per window.

Overlays MUST NOT manage local or nested stacks.

---

### INV-OV2 — Total Ordering

Overlays are totally ordered by stack position.

At any time:

* exactly one overlay is topmost
* hit-testing follows stack order from top → bottom

---

### INV-OV3 — Explicit Push / Pop

Overlay lifecycle:

* overlays are pushed explicitly
* overlays are popped explicitly
* implicit destruction is FORBIDDEN

---

## 4. Input Routing Rules

### INV-OV4 — Top-Down Hit-Testing

Pointer events:

* are hit-tested from topmost overlay downward
* stop at first accepting overlay
* never “fall through” accepted overlays

---

### INV-OV5 — Keyboard Ownership

Keyboard events are delivered to:

1. topmost capturing overlay, if any
2. topmost non-capturing overlay that has focus
3. base application focus

No other routing is allowed.

---

## 5. Input Capture Semantics

### INV-OV6 — Explicit Capture Only

An overlay captures input only if:

* it explicitly requests capture
* capture is granted by OverlayRoot

Default: **no capture**.

---

### INV-OV7 — Exclusive Capture

At most **one** overlay may capture input at a time.

Capturing overlay:

* blocks all input to lower overlays
* blocks all input to base widgets

---

### INV-OV8 — Capture Release

Capture MUST be released when:

* overlay is popped
* overlay explicitly releases capture
* Escape dismisses the overlay (if configured)

Capture MUST NOT persist beyond overlay lifetime.

---

## 6. Focus Semantics

### INV-OV9 — Focus Containment

For capturing overlays:

* focus MUST be trapped inside overlay
* Tab / Shift+Tab cycle within overlay
* focus MUST NOT escape

---

### INV-OV10 — Focus Restoration

On overlay dismissal:

* focus MUST be restored to the previously focused widget
* restoration MUST be exact

No heuristic focus guessing allowed.

---

## 7. Dismissal Rules

### INV-OV11 — Outside Click Handling

Overlay MAY be configured to dismiss on outside click.

If enabled:

* outside click is consumed
* dismissal happens before any other widget sees the event

---

### INV-OV12 — Escape Key Handling

Escape key:

* dismisses topmost overlay if configured
* is consumed by overlay
* MUST NOT propagate downward

---

## 8. Scroll Interaction

### INV-OV13 — Scroll Isolation

When an overlay is active:

* scroll events are routed to the topmost overlay
* background scroll is blocked
* wheel events MUST NOT leak

Scroll behavior inside overlay follows SPEC-HAL-110.

---

## 9. Tooltip Exception

### INV-OV14 — Non-Capturing Tooltips

Tooltips:

* MUST NOT capture input
* MUST NOT accept focus
* MUST NOT block pointer events
* MUST be auto-dismissed on pointer movement or timeout

Tooltips are informational only.

---

## 10. Responsibility Boundaries

### OverlayRoot Owns:

* overlay stack
* input routing
* capture arbitration
* focus restoration

### Overlay Owns:

* request/release capture
* dismissal policy
* internal focus management

### Overlays MUST NOT:

* route input manually
* intercept events after dismissal
* manage their own stacking order

---

## 11. Forbidden Behavior

Overlays MUST NOT:

* allow input to leak to background widgets
* stack multiple capturing overlays
* steal focus without restoration
* dismiss silently without state cleanup
* depend on z-index hacks

---

## 12. Test Requirements

Implementations MUST test:

* stack ordering
* exclusive capture
* focus trapping
* focus restoration
* outside-click dismissal
* Escape handling
* scroll blocking

---

## 13. Reference Implementation

Planned components:

* `OverlayRoot`
* `OverlayHandle`
* `PopupOverlay`
* `ModalOverlay`

---

## 14. Final Note

Overlays are **authority boundaries**.

If input can escape them,
the system is already broken.
