# **SPEC-HAL-086 — Scroll & Viewport System (Unified Scroll Model v1)**

### **IMPORTANT: ScrollArea is a fixed-width container.**

It fills all available cross-axis space provided by its parent.
Children never influence ScrollArea's width.
Scrollbars always align to ScrollArea.rect.

---

**SPEC-HAL-086**
**Scroll & Viewport System (Unified Scroll Model v1)**
**Author:** Halcyon Architect
**Status:** Implemented
**Version:** 1.0

**Depends On:**

* SPEC-HAL-005 Scene Graph
* SPEC-HAL-008 Layout Engine
* SPEC-HAL-009 Event Routing v2
* SPEC-HAL-024 Unified Coordinate Model
* SPEC-HAL-033 Input Fusion Layer
* SPEC-HAL-036 Input Coalescing
* SPEC-HAL-074 Style Runtime Engine
* SPEC-HAL-075 Style Property Indexing & Layout Stability
* SPEC-HAL-076 StyleCache v2
* SPEC-HAL-079 Unified Event Flow v3
* SPEC-HAL-080 The Halcyon Interaction Engine
* SPEC-HAL-081 The Halcyon Compositor v1
* SPEC-HAL-084 RenderGraph v2
* SPEC-HAL-041 Unified Timing & Animation Clock

---

## Implementation Status

**Partial Implementation (ScrollContainer Model)**

The full unified scroll and viewport system described in this SPEC is not yet implemented.
However, a pragmatic `ScrollContainer` model provides working scroll functionality:

### Implemented (ScrollContainer v1)

- **Stage 1**: `ScrollContainer` struct with content_size, viewport_size, scroll offsets
- **Stage 2**: `LayoutResult` with viewport-aware layout protocol
- **Stage 3**: Dual-offset state machine (target vs visual offsets, exponential smoothing)
- **Stage 4**: Engine-owned pre-routing scroll visual update
  - `WidgetTree::update_scroll_animations(dt_secs)` called in frame loop
  - Runs before event routing so hit-test projections match displayed content
  - `ScrollArea` event handlers only set target offset; visual smoothing is centralized
  - This delta-based scroll subsystem is separate from SPEC-HAL-021 generic
    Timeline playback and follows the literal SPEC-HAL-042 split phase order
- **Stage 5**: ScrollableViewport rewriter in layout phase (SUPERSEDED by Stage 7)
  - `LayoutResult` extended with `viewport_size` and `scroll_offset` fields
  - `WidgetNode` stores `layout_scroll_offset` from layout result
  - `ScrollArea::layout()` reports scroll info via `LayoutResult::scroll()`
  - ~~`compute_absolute_positions()` applies scroll offset to shift children~~ (removed in Stage 7)
  - ~~Children of scroll containers have `absolute_xy` shifted by visual scroll offset~~ (removed in Stage 7)
- **Stage 6**: Scroll viewport clipping
  - `ScrollArea.clip_bounds()` returns viewport rect for clipping
  - `paint_node()` wraps scroll container children in `ClipNode` via `begin_clip()`
  - `Batcher.flatten_node()` processes `ClipNode` with `push_clip()`/`pop_clip()`
  - Primitives inside scroll containers capture `ClipState` at insertion time
  - Renderer applies scissor rect via `clip_rect_to_scissor()` during batch rendering
  - Content outside viewport is clipped at GPU level via scissor test
- **Stage 7**: ScrollNode integration into SceneGraph
  - `SceneNode::Scroll(ScrollNode)` added to scene graph
  - `ScrollNode` wraps children with translation offset
  - `SceneBuilder::begin_scroll()`/`end_scroll()` create scroll transforms
  - `paint_node()` wraps scroll container children in `ScrollNode` (inside `ClipNode`)
  - `Batcher` maintains scroll offset stack (`push_scroll`/`pop_scroll`)
  - All `add_*` methods apply `current_scroll_offset` to primitive positions
  - `compute_absolute_positions()` and `compute_absolute_paint_rects()` NO LONGER apply scroll
  - Scroll transforms are now canonical scene graph nodes, not layout hacks

### Not Yet Implemented

- Full `ScrollModel` with velocity and physics
- Nested scroll resolution
- Declarative overflow styles
- Scrollbar model separate from view

---

**Implements:**
A unified **Scroll & Viewport** system:

1. ScrollModel (offset, velocity, bounds)
2. ScrollNode in SceneGraph
3. Nested scrolling semantics
4. Momentum/inertial scroll using timing clock
5. Viewport clipping rules
6. Declarative overflow styles
7. Scrollbar model (no visuals)

---

## **1. Motivation**

Currently scrolling is:

* ad-hoc per widget
* incomplete
* not formally bound to coordinate model or compositor

But modern UI is scroll-first:

* timelines
* chat views
* lists
* nested panels

Halcyon needs a canonical scroll model that is:

* deterministic
* nested-aware
* animation-capable

---

## **2. Goals**

### **2.1 Primary**

* Single ScrollModel for all scroll containers
* ScrollNode in SceneGraph with viewport + content rect
* Nested scroll resolution (inner vs outer)
* Momentum scrolling using unified timing clock
* Precise integration with clip stack and RenderGraph

### **2.2 Secondary**

* Declarative style properties for overflow, behavior, scrollbar presence
* Scrollbars as a view onto ScrollModel

---

## **3. ScrollModel**

```rust
pub struct ScrollModel {
    pub offset_x: f32,
    pub offset_y: f32,

    pub max_x: f32,
    pub max_y: f32,

    pub velocity_x: f32,
    pub velocity_y: f32,

    pub overscroll_x: f32,
    pub overscroll_y: f32,

    pub is_active: bool,    // currently receiving direct input
    pub is_momentum: bool,  // currently decaying
}
```

### **3.1 Rules**

* Offsets in **logical** space (SPEC-024)
* Offsets clamped to [0, max] when not overscrolling
* Velocity updated on scroll input, decays via friction model
* Overscroll is temporary and damped

---

## **4. SceneGraph Integration**

### **4.1 ScrollNode**

```rust
pub struct ScrollNode {
    pub viewport: RectF,     // where content is visible
    pub content_size: SizeF, // full content logical size
    pub model: ScrollModel,
}
```

### **4.2 Coordinate Behavior**

* viewport in parent’s logical coordinates
* content rendered as if at `-model.offset` within viewport
* hit-testing transforms pointer by +offset before testing children

### **4.3 Clip Interaction**

ScrollNode implicitly pushes:

* clip rect == viewport
* translation == (-offset_x, -offset_y)

Renderer sees:

1. push_clip(viewport)
2. translate(-offset_x, -offset_y)
3. draw children
4. pop_clip()

---

## **5. Input & Event Flow**

### **5.1 ScrollEvent**

```rust
pub enum ScrollPhase {
    Start,
    Update,
    End,
    Momentum,
}

pub struct ScrollEvent {
    pub delta_x: f32,
    pub delta_y: f32,
    pub phase: ScrollPhase,
    pub device: ScrollDevice, // Wheel, Touch, Trackpad
}
```

### **5.2 Event Routing**

* Unified Event Flow v3 (079) resolves which ScrollNode(s) receive events
* ScrollNodes handle direct deltas first
* Remaining delta may bubble to parent scrolls if a node hits bounds

---

## **6. Nested Scrolling Semantics**

### **6.1 Rule**

1. Start from the deepest ScrollNode under pointer
2. Apply delta to its ScrollModel
3. Clamp offset
4. If delta still remains and child is at bound → propagate upward
5. Repeat until delta is consumed or root reached

### **6.2 Example**

Scrolling a list inside a panel:

* inner list scrolls first
* when top/bottom reached, outer panel starts scrolling

---

## **7. Momentum & Timing**

Use `SPEC-HAL-041` timings.

### **7.1 Velocity Update**

On `ScrollPhase::Update`:

```rust
velocity_x = update_velocity(velocity_x, delta_x, dt);
velocity_y = update_velocity(velocity_y, delta_y, dt);
```

On `ScrollPhase::End`:

* `is_momentum = true` if |velocity| > threshold

### **7.2 Tick Integration**

Each frame:

```rust
if model.is_momentum {
    apply_friction(&mut model, dt);
    integrate_offset(&mut model, dt);
    clamp_and_handle_overscroll(&mut model);
}
```

If velocity < epsilon and no overscroll → `is_momentum = false`.

---

## **8. Declarative Style Integration**

### **8.1 Properties**

```text
overflow: Clip | ScrollX | ScrollY | ScrollXY
scroll-behavior: Instant | Smooth
scrollbar: None | Auto | Always
scroll-snap: None | Start | Center | End
```

### **8.2 Behavior**

* overflow determines whether ScrollNode is created for a widget
* ScrollModel size is determined from `content_size - viewport_size`
* `scroll-behavior: Smooth` uses an animation to target offset
* `scroll-snap` snaps final momentum to nearest anchor positions

---

## **9. Scrollbars (Model Only)**

### **9.1 State**

```rust
pub struct ScrollbarState {
    pub track_rect: RectF,
    pub thumb_rect: RectF,
    pub visible: bool,
}
```

Model calculated from:

* viewport size
* content size
* current offset

Scrollbar visuals are widget-level and theme-level, not part of HAL.

---

## **10. Renderer & RenderGraph Integration**

### **10.1 CPU Side**

ScrollNode → emits:

* a ClipNode (viewport)
* a TransformNode (-offset)
* nested children

### **10.2 GPU Side**

RenderGraph v2 just sees:

* clip command
* draw commands with transformed vertices

No special scroll passes; it’s all baked into transforms.

---

## **11. Testing Matrix**

**ScrollModel**

* clamping with various content sizes
* overscroll and damping
* momentum decay timing

**Nested Scroll**

* inner-only scroll
* handoff to outer scroll at bounds
* diagonally scrolling content

**Rendering**

* clipped content correctness
* fractional offsets (no visual tearing)
* scroll + layout transitions compatibility

---

## **12. Implementation Plan**

### **Phase 1 — ScrollModel Core**

* type definition
* clamping, velocity, friction

### **Phase 2 — SceneGraph ScrollNode**

* viewport + content rect
* offset-based transform
* clip integration

### **Phase 3 — Event Integration**

* ScrollEvent
* nested scroll resolution logic

### **Phase 4 — Style-Level Wiring**

* map `overflow` and friends to ScrollNode creation
* plug into layout engine for content size

### **Phase 5 — Scrollbars**

* model state computation
* example widgets for scrollbars

### **Phase 6 — Examples & Tests**

* scrolling list demo
* chat-style log view
* nested scroll playground

---

## **13. Implementation Notes**

* Scroll & Viewport are required for all serious UIs.
* This SPEC must be implemented before "fancy" scroll-linked animations or physics.
* R-series GPU work assumes ScrollNode semantics are stable.

---

## **HALCYON SCROLL SYSTEM — HARDENED RULESET (FINAL)**

### **14.1 Ownership of Layout Space**

A ScrollArea's assigned rect from its parent is the **only** spatial authority for:

* viewport size
* scrollbar anchoring
* hit regions
* scroll offset bounds

ScrollArea may **not** exceed or modify its parent-assigned rect.

---

### **14.2 Width Authority Rule (Non-negotiable)**

```
ScrollArea.width = parent.assigned_inner_width
```

This width is determined *only* by the parent's constraint propagation and padding model.

Children, content size, or child shrink-to-fit behavior MUST NOT reduce or override ScrollArea.width.

---

### **14.3 Viewport Model**

```
Viewport.width = ScrollArea.rect.width
Viewport.height = ScrollArea.rect.height
```

Viewport is a direct mirror of ScrollArea.rect.
There is no scenario where viewport dimensions differ from the assigned rect.

---

### **14.4 Scrollbar Anchoring Invariant**

Scrollbars anchor **exclusively** to scroll_area.rect:

```
scrollbar.right_edge = scroll_area.rect.right
```

Not:

* viewport rect
* inner content rect
* child rects
* padded rects of ancestors

This invariant overrides all earlier text.

---

### **14.5 Parent Padding and Margins**

ScrollArea **must respect**:

* parent.padding
* parent.margin
* parent.border

A ScrollArea may never draw scrollbars outside its assigned rect.

Parent padding reduces available width for ScrollArea.
ScrollArea fills the parent's inner width AFTER padding.
ScrollArea must NEVER draw outside the padded inner rect of its parent.

---

### **14.6 Children of ScrollArea**

Children layout rules remain intact:

* children always respect ScrollArea's own padding
* children respect their own padding/margins
* children NEVER bypass ancestor padding
* children NEVER draw into forbidden ancestor space

ScrollArea's width rule affects only ScrollArea itself, not its descendants' margin/padding rules.

---

### **14.7 Nested ScrollAreas**

Nested scroll areas inherit all rules recursively:

Outer ScrollArea
→ assigns rect to inner
→ inner fills its assigned width
→ inner scrollbars align to inner.rect, not outer rect

This mirrors native UI behavior.

---

### **14.8 Absolutely Forbidden Behaviors**

The following are explicitly outlawed:

* shrink-to-content ScrollAreas
* width inference from child content
* scrollbar anchoring to viewport instead of scroll_area.rect
* any Flex/Legend logic that reduces ScrollArea's width
* padding propagation into ScrollArea's viewport
* dynamic resize of ScrollArea width based on children

These must be removed wherever present.

---

## **15. Final Enforcement Layer (HAL-086)**

These invariants are required for a correct ScrollArea implementation:

```
scroll_area.rect.width == scroll_area.viewport.width
scroll_area.viewport.width == assigned_layout_width
scrollbar.right_edge == scroll_area.rect.right
children.max_width <= scroll_area.inner_width
```

If any of these fail at runtime, it indicates a layout bug in:

* Flex cross-axis sizing
* Legend width assignment
* Constraint propagation logic
* ScrollArea implementation

Scrollbars MUST align to scroll_area.rect, no exceptions.

---

### **15.1 Dual-Width Model Prohibition**

ScrollArea MUST NOT have two competing width sources.

Allowed source: assigned rect from parent.
Forbidden sources: viewport-size-only models, child-content-sized models.

---

See also: SPEC-HAL-110
