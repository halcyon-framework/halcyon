**SPEC-HAL-014: Halcyon — Advanced Input Model (Trading-Critical Gestures & High-Precision Interaction Layer)**
**Author:** JC
**Date:** 2025-11-17
**Status:** Draft
**Scope:** Defines advanced input semantics required for trading-grade components: charts, DOM/ladder, blotters, selection tools, price-level interactions, rubber-band tools, and extremely high-frequency pointer events.
**Purpose:** Extend HAL-009 (Event Routing & Hit-Testing) with domain-specific gestures and interaction guarantees essential for serious trading systems.
**Dependencies:** HAL-009 (routing), HAL-010 (widget system), HAL-011 (basic widgets), HAL-013 (TextField focus rules).

This SPEC is about **interaction richness**, not rendering, not layout.

---

# 1. Why This SPEC Exists

Trading systems require:

* **zero-miss pointer capture**
* stable drag behaviors on ultra-fast panning/zooming
* multi-level click semantics (single / double / triple)
* high-frequency event tolerance
* box-select + rubber-band tools
* context-sensitive click
* safety semantics (drag-cancel, drag-undo)
* modifier-sensitive actions (Ctrl+click = add order, Alt+click = cancel all at price)
* multi-region hit-tests *inside* one widget (price region, chart region, axis region, overlay region)
* synthetic composite gestures for charting (Marquee select, Crosshair drag, Range measurement)

Generic UI frameworks do not handle this.
Halcyon must.

---

# 2. Gesture Model: What Halcyon Supports

We define the following gestures at the **framework level**, not widget level:

### 2.1 Click Variants

* SingleClick
* DoubleClick
* TripleClick
* Click+Hold (long press)

### 2.2 Drag Variants

* DragStart
* DragMove
* DragEnd
* DragCancel

### 2.3 Rubber-Band Tools

* BoxSelectStart
* BoxSelectMove
* BoxSelectEnd (commit)
* BoxSelectCancel

### 2.4 High-Precision Composite Events

* HoverChange (enter/leave)
* PointerStable (mousemove with no button)
* DragThresholdExceeded (e.g., 3 px)
* ModifierChanged (affects ongoing drag)

### 2.5 Price/Region-specific Semantics

Inside widgets (like chart/ladder), the input model emits:

* RegionEnter / RegionLeave
* RegionClick
* RegionDragStart / Move / End

This is necessary for complex finance UI where one widget has multiple logical zones.

---

# 3. Core Architecture: Gesture Recognizers

Each widget can install **Gesture Recognizers**:

```
Widget
 └── GestureRecognizer
       ├── ClickRecognizer
       ├── DoubleClickRecognizer
       ├── DragRecognizer
       ├── BoxSelectRecognizer
       └── RegionRecognizer (chart/ladder-specific)
```

Recognizers translate raw pointer events into high-level events.

This is the power layer on top of HAL-009.

---

# 4. State Machines

Each recognizer is a **deterministic finite-state machine**.
No guessing. No heuristics.

Let's define the essential ones.

---

# 5. Click Recognizer FSM

```
Idle
 ├─ PointerDown → PendingClick
PendingClick
 ├─ PointerUp (within threshold) → SingleClick
 ├─ SecondDown (within Δt, Δd) → PendingDoubleClick
PendingDoubleClick
 ├─ PointerUp → DoubleClick
 ├─ ThirdDown (Δt, Δd) → PendingTripleClick
PendingTripleClick
 ├─ PointerUp → TripleClick
```

Parameters:

* time threshold Δt = 250 ms
* distance threshold Δd = 4 px

The recognizer guarantees reliable double-click detection even in high-frequency environments.

---

# 6. Drag Recognizer FSM

```
Idle
 ├─ PointerDown → MaybeDrag
MaybeDrag
 ├─ Move < threshold → stay
 ├─ Move > threshold → DragStart
Drag
 ├─ Move → DragMove
 ├─ PointerUp → DragEnd
 ├─ ESC/CaptureLost → DragCancel
```

Critical features:

* pointer capture is acquired at DragStart
* drag doesn’t break even if pointer leaves window
* ESC cancels any drag (standard in CAD/trading UIs)

---

# 7. Box Selection Recognizer (Rubber Band)

This powers selection on charts, blotters, DOM.

```
Idle
 ├─ PointerDown + Modifier == SHIFT → BoxSelectStart
BoxSelecting
 ├─ PointerMove → BoxSelectMove
 ├─ PointerUp → BoxSelectEnd (commit)
 ├─ ESC → BoxSelectCancel
```

**Domain logic is NOT in Halcyon**—only gesture detection.

Widgets receive:

```
BoxSelectStart { origin }
BoxSelectMove { current }
BoxSelectEnd { final }
BoxSelectCancel
```

---

# 8. Region Recognizer (Multi-zone Hit-Testing)

One widget (chart) can have:

* plotting area
* price axis
* time axis
* overlays
* tools

This recognizer allows widgets to subdivide their local coordinate space into **named regions**:

Example:

```
enum ChartRegion {
    PriceAxis,
    TimeAxis,
    CandlesArea,
    IndicatorsOverlay,
    OrdersOverlay,
}
```

Widget declares:

```rust
fn hit_test_region(&self, local_pos: (f32, f32)) -> ChartRegion;
```

Framework emits:

* RegionEnter(prev, new)
* RegionLeave(prev, new)
* RegionClick(region)
* RegionDragStart(region)

This is vital for high-precision interactions like dragging order levels on a ladder.

---

# 9. Modifier Semantics (Trading-Critical)

Modifiers must be delivered instantly and independently:

```
Modifiers {
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
}
```

Why:

* Shift+Drag → Box select
* Alt+Click → Close all orders
* Ctrl+Click → multi-select
* Ctrl+Shift+Drag → custom range tool

Modifiers may change *while* dragging:

* Halcyon must emit ModifierChanged events.

---

# 10. Synthetic High-Level Events

Halcyon turns raw events into meaning:

**Example for charts:**

* RangeMeasureStart
* RangeMeasureDelta
* RangeMeasureEnd

These are widget-specific but fed by:
Drag + Modifier + region classification.

Framework exposes:

```
HighLevelEvent::BoxSelectStart
HighLevelEvent::RubberBandMove
HighLevelEvent::PinchZoomStart (future)
HighLevelEvent::DragCancel
HighLevelEvent::ModifierChanged
```

Widgets opt-in.

---

# 11. Cancel Semantics (Safety)

During drag or box-select, ESC must cancel:

* pointer capture released
* widget receives DragCancel or BoxSelectCancel
* no commit happens

This mimics professional tools.

Also:

* Losing focus
* Losing capture
* Window losing mouse grab

All must cancel appropriately.

---

# 12. Trading-Specific Guarantees

Everything in this SPEC is shaped by real trading UIs:

### 12.1 Zero-miss drag

Even at 120–240 FPS, drag events must not drop.

### 12.2 Zero-hysteresis gestures

Movement threshold for drag is deterministic.

### 12.3 Low-latency path

Gesture recognizers must run:

* without heap allocations
* without dynamic dispatch
* in O(1) per event

### 12.4 No ambiguity

Double-click must never accidentally trigger drag.
Drag must never accidentally trigger double-click.
Thresholds are strict.

### 12.5 Hard snap-to-zone

Region recognition inside a widget must always be correct.

---

# 13. Implementation Checklist for Claude Code

### 13.1 Extend Event Model

* Add HighLevelEvent enum
* Add synthetic gesture events
* Add ModifierChanged event

### 13.2 Implement Recognizers

* ClickRecognizer
* DragRecognizer
* BoxSelectRecognizer
* RegionRecognizer

### 13.3 Integrate with Event Routing (HAL-009)

* Recognizers wrap raw pointer events
* Emitted gestures propagate like normal events
* PointerCapture respected throughout

### 13.4 Add ESC cancellation logic

* In runner→ cancel active gestures
* Emit DragCancel / BoxSelectCancel

### 13.5 Add region API for widgets

* Widgets declare region boundaries
* HitTest passes region-specific events

### 13.6 Add triple-click detection

(Used by chart tools for segment selection)

### 13.7 Update examples

Add a demo showing:

* click
* double-click
* drag rectangle
* rubber band
* region highlight

---

# 14. Outcome

After SPEC-HAL-013 is implemented:

Halcyon becomes:

* trading-ready
* chart-ready
* ladder-ready
* tool-ready
* selection-ready
* gesture-rich

This SPEC gives Halcyon the *precision interactivity* layer required for any real trading interface.

Charts, DOM, blotters, position tables, and range tools will behave exactly like professional terminals.
