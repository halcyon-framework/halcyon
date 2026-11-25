# SPEC-HAL-275: **RangeSlider — Structural Contract and Invariants**

### *Dual-Handle Min/Max Value Range Selection*

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-210 (Slider Control and Value Exposure Invariants)

---

# 1. Purpose

Define the structural contract for a dual-handle slider that selects a value range `[low, high]` within a bounded domain `[min, max]`. Extends the single-value Slider (SPEC-HAL-210) with a second handle and range-fill semantics.

---

# 2. Value Type

```rust
#[derive(Clone, Copy, PartialEq)]
pub struct RangeValue {
    pub low: f32,
    pub high: f32,
}
```

### Invariants

**INV-275-V1: Ordering**
`low <= high` MUST hold at all times. The widget enforces this — handles may not cross.

**INV-275-V2: Domain Bounds**
`min <= low <= high <= max`. Values are clamped to the domain on set.

**INV-275-V3: Step Quantization**
If `step` is set (non-zero), both `low` and `high` snap to the nearest `step` boundary. `step` divides the domain evenly — values that don't land on a step boundary are rounded to the nearest one.

---

# 3. Handle Identity

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RangeHandle {
    Low,
    High,
}
```

**INV-275-H1: Independent Drag**
Each handle is dragged independently. Dragging one does not move the other.

**INV-275-H2: No Crossing**
When dragging the low handle, it clamps at the high handle's position (and vice versa). The handles may meet (low == high) but never cross.

**INV-275-H3: Hit Disambiguation**
When handles overlap (low == high), pointer-down selects the handle whose movement direction matches the drag direction. If ambiguous (exact center hit), the low handle takes priority.

---

# 4. Widget State

```rust
pub struct RangeSliderState {
    /// Domain bounds.
    min: f32,
    max: f32,
    /// Current committed range.
    value: RangeValue,
    /// Optional step quantization.
    step: Option<f32>,
    /// Which handle is being dragged, if any.
    active_handle: Option<RangeHandle>,
    /// Which handle has keyboard focus.
    focused_handle: Option<RangeHandle>,
    /// Orientation.
    orientation: Orientation,
}
```

---

# 5. Interaction Invariants

**INV-275-I1: Pointer Drag**
Pointer-down on a handle captures it. Pointer-move updates the handle's value. Pointer-up releases capture. The non-dragged handle does not move.

**INV-275-I2: Track Click**
Clicking the track between handles does nothing (ambiguous intent). Clicking the track outside the range moves the nearest handle to the click position.

**INV-275-I3: Keyboard**
When a handle has keyboard focus, Left/Down decrements and Right/Up increments by one step (or 1% of range if no step). Tab moves focus between the two handles.

**INV-275-I4: Continuous Update**
Value updates are emitted continuously during drag (every pointer-move), not only on release. The consumer receives the working value on each update.

---

# 6. Layout

Shares `SliderGeometry` from SPEC-HAL-210. Both handles use the same thumb diameter. The track between handles renders with accent fill; track outside the range renders with the default track color.

**INV-275-L1: Minimum Range**
No minimum range is enforced at this SPEC level (low == high is valid). An optional `min_range: f32` can be added as a future extension.

---

# 7. Rendering

**INV-275-R1: Track Fill**
The track between `low` and `high` renders with `accents.primary`. Track outside the range renders with `surfaces.level2`.

**INV-275-R2: Handle Rendering**
Both handles render identically to the single Slider's thumb (SPEC-HAL-210): circle with border, hover/pressed states.

**Scoped visual decision — JC, 2026-10-24 (binding for this paragraph):** In Track
style and its Radial fallback, both round thumbs use the existing 1.25× active
diameter while the enabled widget holds current framework keyboard Capture or
either thumb is being dragged. Pointer release keeps both enlarged only while
keyboard Capture remains; focus alone uses the resting diameter. Enlargement is
centered decoration and changes no allocation, hit geometry, values or input
ownership. The current keyboard or dragged handle retains its existing color and
Z emphasis; Navigation still identifies the current keyboard target. Bar/Pill
handle treatment is unchanged. This adopts only this visual decision under
SPEC-HAL-328 §12, not the remaining Draft contract.

**INV-275-R3: Active Handle On Top**
The currently dragged or focused handle renders above the other handle (higher z-index) to prevent visual occlusion.

**INV-275-R4: Label**
Optional value labels showing `low` and `high` above/below their respective handles.

---

# 8. Acceptance Criteria

- `low <= high` maintained at all times.
- Handles drag independently.
- Handles cannot cross.
- Track between handles has accent fill.
- Step quantization snaps both handles.
- Keyboard moves focused handle by step.
- Overlapping handles are disambiguated.

---

# 9. Non-Goals

- Multi-range (more than two handles).
- Non-linear scale (log, exponential).
- Vertical orientation in this SPEC (horizontal only; vertical is a future extension via `Orientation`).
