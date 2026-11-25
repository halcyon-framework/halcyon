````markdown
**SPEC-HAL-008: Halcyon Layout Engine & Flex Containers**

### **IMPORTANT: ScrollArea is a fixed-width container.**

It fills all available cross-axis space provided by its parent.
Children never influence ScrollArea's width.
Scrollbars always align to ScrollArea.rect.

---

**Author:** JC
**Date:** 2025-11-16
**Status:** Draft
**Scope:** Layout model, layout pass, `Row` / `Column` containers, alignment/spacing, and performance constraints.
**Primary Use-Case:** Trading-grade dense UIs (panels, ladders, blotters, forms, dashboards).

---

## 1. Goals

Layout engine must:

- Be **deterministic** and **retained-mode**.
- Handle **high-density** UIs (many widgets, nested panels).
- Support **Row / Column** flex layouts as first-class primitives.
- Be expressive enough for:
  - multi-panel screens,
  - toolbars, status bars,
  - tables/grids built from rows/columns,
  - sidebars, split layouts, etc.
- Run in **< 1 ms** for ~2000 widgets on a modern CPU.
- Be easy to reason about: no hidden magic, no constraint solver hell.

Non-goals (for this SPEC):

- Full grid/fractional layout.
- Constraint solver (à la Cassowary).
- Fancy animations / layout transitions.

Those can come later on top of this.

---

## 2. Layout Model Overview

Each widget:

- Receives **constraints** `[min_width, max_width] × [min_height, max_height]`.
- Returns a concrete **Size { width, height }` satisfying those constraints.
- Stores its **final position** (x, y) in its own local coordinate space (or via a layout tree).

**SPECIAL CASE: ScrollArea cross-axis rule**
When a ScrollArea appears as a child in a Column, its cross-axis constraint (width) is ALWAYS parent.max_width_after_padding.

Flex must NOT compute ScrollArea width from:
– max_child_cross
– intrinsic child widths
– shrink-to-fit heuristics

Layout pass is **top-down**:

1. Parent computes constraints for children.
2. Children compute sizes.
3. Parent chooses how to position children (x/y offsets).

We will start with one layout algorithm: **Flex layout** (horizontal or vertical), implemented via:

- `Row` – horizontal flex container.
- `Column` – vertical flex container.

---

## 3. Core Types Additions (`layout.rs`)

We extend `layout.rs` with rect/position and flex enums.

```rust
// src/layout.rs (additions)

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMain {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignCross {
    Start,
    Center,
    End,
    Stretch,
}
````

Later we can extend this module with DPI info, etc.

---

## 4. Widget Layout Contract (Refinement)

We refine the `Widget` trait to make layout responsibilities explicit.

```rust
// src/widget.rs (refinement idea, not mandatory change right now)

pub trait Widget {
    /// Layout within given constraints.
    ///
    /// Returns the size this widget wants to occupy.
    fn layout(&mut self, cx: &mut LayoutContext, constraints: Constraints) -> Size;

    /// Paint at previously established position.
    ///
    /// The widget is responsible for knowing where it is in its parent's
    /// coordinate space, or the parent sets transforms before painting.
    fn paint(&self, builder: &mut SceneBuilder);

    /// Handle input/event (already defined).
    fn handle_event(&mut self, event: &Event);
}
```

**Position storage** can be:

* either inside each widget (`self.bounds: Rect`),
* or in a separate layout tree structure.

For now, simplest: each container stores child `Rect`s; leaf widgets know their local (0,0) coordinate and are painted relative to the container offset.

---

## 5. Flex Container Abstraction

We define a common internal helper struct to describe items in a flex container.

### 5.1. FlexItemConfig

Configuration for a child inside a flex container:

```rust
// src/layout.rs or a new flex module

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexItemConfig {
    pub flex_grow: f32,   // >0 = can expand along main axis
    pub flex_shrink: f32, // >0 = can shrink if needed
    pub basis: Option<f32>, // preferred size along main axis (in px), None = auto
}
```

### 5.2. Child Slot

In the container widget (`Row` / `Column`) we will keep:

```rust
pub struct ChildSlot<W: Widget> {
    pub widget: W,
    pub config: FlexItemConfig,
    pub rect: Rect,  // assigned during layout
}
```

For now this is just conceptual; SPEC-HAL-009 or the implementation step will define exact generics/trait object story.

---

## 6. Flex Layout Algorithm (Deterministic)

We adopt a simplified, predictable flexbox-like algorithm.

### 6.1. Definitions

Let:

* `axis` = `Horizontal` for `Row`, `Vertical` for `Column`.
* `main_size` = width for `Row`, height for `Column`.
* `cross_size` = height for `Row`, width for `Column`.

Input:

* Container constraints: `Constraints`.
* Children: list of `(Widget, FlexItemConfig)`.

### 6.2. Steps (Pseudocode, Horizontal Version)

1. **Determine available main/cross ranges**

```text
available_main_min  = constraints.min_width
available_main_max  = constraints.max_width
available_cross_min = constraints.min_height
available_cross_max = constraints.max_height
```

2. **Measure intrinsic size for each child**

For each child:

* Build child constraints:

    * Main axis: `min=0, max=available_main_max` (soft).
    * Cross axis: `min=0, max=available_cross_max` (soft).

* Call `child.layout(child_constraints)` to get `child_size`.

3. **Initial main axis size**

Sum of intrinsic sizes:

```text
intrinsic_main_sum = Σ child_size.main
```

4. **Distribute free space or compression**

Let `target_main` be chosen in `[available_main_min, available_main_max]`.

* Compute `free_space = target_main - intrinsic_main_sum`.

Case A: `free_space > 0` (expand)

* Compute `total_grow = Σ flex_grow`.

* If `total_grow > 0`:

  Each child with `flex_grow > 0` gets:

  ```text
  extra_main_child = free_space * (flex_grow_child / total_grow)
  final_main_child = intrinsic_main_child + extra_main_child
  ```

* Else (no grow):

    * `final_main_child = intrinsic_main_child` for all.

Case B: `free_space < 0` (shrink)

* Compute `total_shrink = Σ flex_shrink`.

* If `total_shrink > 0`:

  Each child:

  ```text
  shrink_amount = |free_space| * (flex_shrink_child / total_shrink)
  final_main_child = max(0, intrinsic_main_child - shrink_amount)
  ```

* Else (no shrink):

    * Overflow: container size may exceed target; acceptable as long as we document.

Case C: `free_space == 0`:

* `final_main_child = intrinsic_main_child`.

5. **Cross Axis Size**

* Compute `cross_needed = max(child_size.cross)`.
* Clamp to `available_cross_min..=available_cross_max`.
* If `AlignCross::Stretch`, children with no explicit cross sizing are stretched in cross axis to `cross_needed`.

6. **Positioning (Main Axis Alignment)**

Compute starting offset and inter-item spacing based on `AlignMain`:

* `Start`:

  ```text
  offset = 0
  spacing = 0
  ```

* `Center`:

  ```text
  leftover = target_main - Σ final_main_child
  offset = leftover / 2
  spacing = 0
  ```

* `End`:

  ```text
  leftover = target_main - Σ final_main_child
  offset = leftover
  spacing = 0
  ```

* `SpaceBetween` (N > 1):

  ```text
  leftover = target_main - Σ final_main_child
  spacing = leftover / (N - 1)
  offset = 0
  ```

* `SpaceAround`, `SpaceEvenly`: analogous.

7. **Assign Rects**

For `Row`:

```text
x = offset
for child in children:
    rect.x = x
    rect.y = cross_position(child_align)
    rect.width  = final_main_child
    rect.height = final_cross_child
    x += final_main_child + spacing
```

For `Column` same but swapped axes.

8. **Return Container Size**

Container size:

```text
width  = target_main  (Row) or computed cross span (Column)
height = cross_needed (Row) or target_main (Column)
```

Clamped to incoming constraints.

---

## 7. Row and Column Widgets (API Sketch)

These will live in `src/widgets/row.rs`, `src/widgets/column.rs` later, but we define the *shape* now.

### 7.1. Builder API

```rust
// src/widgets/row.rs (conceptual)

use crate::{
    layout::{Axis, AlignMain, AlignCross, Constraints, LayoutContext, Rect, Size},
    scene::SceneBuilder,
    widget::Widget,
};

pub struct Row {
    pub children: Vec<Box<dyn Widget>>,
    pub align_main: AlignMain,
    pub align_cross: AlignCross,
    pub spacing: f32,
    pub padding: f32, // symmetrical for now
    pub rect: Rect,   // assigned during layout
    // child rects stored separately or inside child widgets
}

impl Row {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            align_main: AlignMain::Start,
            align_cross: AlignCross::Start,
            spacing: 0.0,
            padding: 0.0,
            rect: Rect::ZERO,
        }
    }

    pub fn child(mut self, child: Box<dyn Widget>) -> Self {
        self.children.push(child);
        self
    }

    pub fn align_main(mut self, align: AlignMain) -> Self {
        self.align_main = align;
        self
    }

    pub fn align_cross(mut self, align: AlignCross) -> Self {
        self.align_cross = align;
        self
    }

    pub fn spacing(mut self, value: f32) -> Self {
        self.spacing = value;
        self
    }

    pub fn padding(mut self, value: f32) -> Self {
        self.padding = value;
        self
    }
}
```

### 7.2. Widget Implementation (High-Level)

```rust
impl Widget for Row {
    fn layout(&mut self, cx: &mut LayoutContext, constraints: Constraints) -> Size {
        // 1. Derive inner constraints by subtracting padding.
        // 2. For each child:
        //    - compute child constraints
        //    - call child.layout(cx, child_constraints)
        //    - store intrinsic sizes
        // 3. Run flex algorithm (Section 6).
        // 4. Assign child rects (x,y,width,height).
        // 5. Set self.rect to container rect.
        // 6. Return container Size.
        todo!()
    }

    fn paint(&self, builder: &mut SceneBuilder) {
        // Optionally draw background/border via theme,
        // then paint children (builder scoped with transform if needed).
        for child in &self.children {
            child.paint(builder);
        }
    }

    fn handle_event(&mut self, event: &crate::event::Event) {
        // Later: use hit-testing + event routing logic from SPEC-HAL-009.
        // For now, naive broadcast or no-op.
        let _ = event;
    }
}
```

`Column` is same but `Axis::Vertical`.

---

## 8. Performance Constraints (Trading Context)

Halcyon must handle layouts like:

* Left panel: market tree / watchlist.
* Center: 1–4 charts, each with overlay widgets.
* Right: order entry, ladder, blotter.
* Bottom: logs, diagnostics.

This implies:

* **Widget count**: easily 500–2000 in complex layouts.
* **Layout passes per second**: at least once per frame (30–120 Hz).
* **Layout time budget**: ≤ 1 ms typical.

So:

* No heap allocations in the inner loop for flex.
* Reuse `Vec`s for child measurement overhead.
* Avoid dynamic dispatch in hottest paths where possible (later optimization).
* Use `f32` for geometry, `f64` only where absolutely needed (e.g. chart world space, not UI layout).

---

## 9. Integration Points with Future SPECs

### 9.1. SPEC-HAL-009 (Event Routing & Hit-Testing)

Layout and event routing must align on coordinate systems:

* Each widget’s local `Rect` is used in hit testing.
* Containers will be responsible for mapping global pointer coords into child local coords.

This SPEC establishes that **every widget that participates in layout must have well-defined bounds**.

### 9.2. SPEC-HAL-004 (Renderer Integration)

Renderer will consume `Scene` primitives in **absolute coordinates** within the window.

Layout defines where widgets are.
Painting uses those positions to generate primitives.

---

## 10. First Implementation Targets from This SPEC

When you start coding this SPEC, the concrete TODO list is:

1. Extend `layout.rs` with:

    * `Rect`
    * `Axis`
    * `AlignMain`
    * `AlignCross`

2. Decide where to store per-widget `Rect`:

    * simplest: in each container + maybe root.

3. Implement **Row**:

    * builder API as above,
    * `layout()` implementing Section 6,
    * `paint()` iterating children.

4. Implement **Column** (reuse same algorithm with swapped axes).

5. Create a tiny test harness (even without wgpu) to:

    * Instantiate a `Row`/`Column` with 3–5 dummy widgets,
    * Give them fake intrinsic sizes,
    * Assert final layout positions against expected values.

6. Add unit tests:

    * `AlignMain::Start/Center/End/SpaceBetween`,
    * `AlignCross::Start/Center/End/Stretch`,
    * `flex_grow` and `flex_shrink` sanity tests.

That's the layout backbone you need to start building real screens later, including trading layouts and eventually the chart widget.
Everything else (events, renderer, docking, etc.) will sit on top of this.

---

## **ScrollArea Width Propagation Invariants (HAL-008 Extension)**

1. Parent passes **max_width** to children.
2. ScrollArea receives parent.max_width_after_padding as its max_width.
3. ScrollArea returns EXACTLY this width in LayoutResult.size.width.
4. No container (Legend, Flex, Grid, FlowGrid) may clamp, reduce, or infer a smaller width for ScrollArea.

**Flex::column correction note:**
Flex::column must NEVER use measured child widths to infer container width if the child is ScrollArea.
ScrollArea is always treated as "stretch to full width" regardless of alignment.

---

See also: SPEC-HAL-110
