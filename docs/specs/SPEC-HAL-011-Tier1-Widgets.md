# **SPEC-HAL-011 — Tier-1 Widgets**

**SPEC-HAL-011**
**Tier-1 Widgets**
**Author:** Halcyon Architect
**Status:** Implemented
**Version:** 2.0

**Depends On:**

* SPEC-HAL-000 Glossary & Naming
* SPEC-HAL-001 Core Architecture
* SPEC-HAL-005 Scene Graph
* SPEC-HAL-006 Text & Font System
* SPEC-HAL-007 State Model
* SPEC-HAL-008 Layout Engine
* SPEC-HAL-009 Event Routing & Hit-Testing
* SPEC-HAL-015 Caching & Diffing
* SPEC-HAL-018 App Framework
* SPEC-HAL-020 Action System
* SPEC-HAL-024 Coordinate Model
* SPEC-HAL-026 Layout Invalidation
* SPEC-HAL-033 Input Fusion Layer
* SPEC-HAL-036 Input Coalescing
* SPEC-HAL-038 Focus Model
* SPEC-HAL-040 GPU Resource Lifetime
* SPEC-HAL-041 Timing & Animation Clock
* SPEC-HAL-045 Triple-Buffer Scene Streaming
* SPEC-HAL-086 Scroll & Viewport System
* SPEC-HAL-087 Widget Identity Model

---

## Implementation Status

**Fully implemented**

The entire Tier-1 surface exists in the codebase:

* FlexRow / FlexColumn
* Container
* Panel
* Label
* Button
* Divider
* ScrollArea
* FixedSpacer
* FlexSpacer
* FlowGrid (Tier-2 wrapper on Flex)

Legacy Row / Column / Spacer have been removed from the codebase.

---

**Implements:**

A minimal, complete UI widget set providing:

1. Linear layout in both directions
2. Box-model containers
3. Panels (background, border, titles)
4. Text labels
5. Buttons with interaction states
6. Dividers
7. Scrollable viewports (full SPEC-086 integration)
8. Responsive grids (Flex-based, Tier-2)

This SPEC defines the canonical Tier-1 API and deprecates the pre-Flex Row/Column/Spacer widgets.

---

## **1. Motivation**

A retained-mode UI engine requires a minimal “first layer” of widgets:

* layout primitives
* content primitives
* interaction primitives
* viewport primitives

Higher-level components (menus, trees, list views, inspectors, code editors) must all be buildable out of these.

The old Tier-1 SPEC was written **before Flex existed**, making it obsolete.
The current engine uses **Flex as the sole layout model**.
This SPEC updates the Tier-1 specification to match the real system.

---

## **2. Goals**

### **2.1 Primary Goals**

* Define **canonical Tier-1 widget API** aligned with the real engine
* Use **Flex** as the universal layout primitive
* Standardize Container, Panel, Label, Button, Divider, ScrollArea
* Remove ambiguity around Row / Column / Spacer
* Integrate widget identity (SPEC-087)
* Integrate scroll system (SPEC-086)

### **2.2 Secondary Goals**

* Ensure all Tier-1 widgets are **theme-driven**
* Ensure pure layout widgets do not perform event logic
* Ensure event handlers do not run timing or animations directly
* Ensure all widgets behave predictably in diffing + caching

---

## **3. Architecture Overview**

```
  Widget (impl)
        ↓
  Layout Pass (Flex / Box Model)
        ↓
  Scene Graph (rects, text nodes, overlay layers)
        ↓
  Renderer (triple buffer)
        ↓
  GPU (wgpu pipelines)
```

All Tier-1 widgets participate in:

* layout pass
* scene build
* event routing
* identity model
* diffing + caching
* scroll & viewport model if applicable

---

## **4. Tier-1 Widget Set**

The canonical set:

1. **FlexRow / FlexColumn** — linear layout (wrap/no-wrap, grow/shrink/basis)
2. **Container** — padding, background, border, radius
3. **Panel** — decorated container (margin, padding, border, title)
4. **Label** — static text
5. **Button** — pointer-driven interaction widget
6. **Divider** — horizontal/vertical rule
7. **ScrollArea** — viewport + scrollbars (SPEC-086)
8. **FixedSpacer** — fixed-size spacing element
9. **FlexSpacer** — flexible spacing element (expands with FlexParams)

Tier-2:

* **FlowGrid** — responsive grid wrapper on Flex

Legacy Row / Column / Spacer have been **removed**. Use FlexRow/FlexColumn with FixedSpacer/FlexSpacer instead.

---

## **5. Layout Model**

### **5.1 FlexRow / FlexColumn**

Flex is the only layout engine in Tier-1.

```
axis: Horizontal / Vertical  
grow / shrink / basis per child  
spacing  
padding  
align_items  
justify_content  
wrap or no-wrap  
```

Supports:

* single-line layouts
* multi-line responsive layouts
* shrink-to-fit
* stretch alignment
* main-axis distribution modes:

    * Start / End / Center
    * SpaceBetween
    * SpaceAround
    * SpaceEvenly

### **5.2 FlexParams**

```
grow: share of free space
shrink: shrink weight
basis: preferred main-size
```

### **5.3 Spacer Widgets**

Two dedicated spacer widgets replace the legacy `Spacer`:

**FixedSpacer** — Fixed-size spacing:
```rust
// Vertical spacer (100px) in a FlexColumn
tree.add_child(column, Box::new(FixedSpacer::vertical(100.0)), None, 0);

// Horizontal spacer (50px) in a FlexRow
tree.add_child(row, Box::new(FixedSpacer::horizontal(50.0)), None, 0);
```

**FlexSpacer** — Flexible spacing (absorbs remaining space):
```rust
// Flexible spacer between two buttons
let spacer = tree.add_child(row, Box::new(FlexSpacer::new()), None, 0);
tree.set_flex(spacer, FlexParams::flexible(1.0));
```

Both widgets:
* Have no children
* Paint nothing
* Are not hit-testable
* Are simpler than the legacy `Spacer` widget

### **5.4 FlowGrid (Tier-2)**

A preset FlexRow with:

* wrap enabled
* children given `basis = min_item_width`
* children `grow = 1.0, shrink = 1.0`

Used for responsive tile/grid layouts.

---

## **6. Core Containers**

### **6.1 Container**

Provides:

* background
* padding
* border
* radius
* single child (or subtree node)
* hit-test transparency unless styled otherwise

Uses `shrink_constraints` for inner layout.

### **6.2 Panel**

Extends Container with:

* margins
* stronger border semantics
* optional header/title region
* boxed-panel visual patterns

---

## **7. Content Widgets**

### **7.1 Label**

Static text widget.

* measured via text engine
* shaped via HarfBuzz / native backends
* painted through unified glyph pipeline
* no event handling
* hit-test transparent

Supports:

* color
* font size
* font face (via text engine)
* optional max-width wrapping (via parent constraints / SPEC-048)

---

## **8. Interactive Widgets**

### **8.1 Button**

Has:

* child subtree
* padding
* hover / pressed states
* pointer capture
* FnMut() callback (later replaced by action system)

State transitions:

```
Normal → Hovered → Pressed → (click)
```

Paint uses theme colors based on state.

Buttons do not:

* run timers
* run animations directly
* perform layout logic on events

All visual changes happen through state and the theme.

---

## **9. Decorative Widgets**

### **9.1 Divider**

Horizontal or vertical rule:

* fixed thickness
* max available size in the other axis
* single rect push

No events.

---

## **10. Scroll Widgets**

### **10.1 ScrollArea**

Tier-1 viewport with full SPEC-086 integration.

Responsibilities:

* layout child with loose constraints
* compute content size
* maintain scroll offsets via ScrollContainer
* translate child content
* clip to viewport rect
* paint scrollbars (overlay scene nodes)
* pointer capture for scrollbar dragging
* scroll wheel / drag / touch / trackpad input

ScrollArea is the foundational scrolling primitive.

---

## **11. Event Model**

Tier-1 widgets follow:

* SPEC-009 routing
* SPEC-033 fusion
* SPEC-036 coalescing
* SPEC-038 focus rules
* SPEC-087 widget identity

Widgets receive fused, routed events that already account for:

* pointer capture
* scroll deltas
* hover states
* focus direction
* coordinate transforms

### **11.1 Event Handler Restrictions**

Event handlers **must not**:

* create timers
* read wall-clock time
* perform animations directly
* trigger layout in loops

They may request:

* layout invalidation
* repaint invalidation
* focus changes
* pointer capture

Timing is handled by SPEC-041.

---

## **12. Theming**

All Tier-1 widgets use theme-provided:

* background colors
* border colors
* text colors
* button visual states
* divider color

No hard-coded colors.

---

## **13. Implementation Plan**

### **Phase 1 — Flex Engine**

* implement Flex (row/column)
* implement wrap / no-wrap
* implement grow / shrink / basis
* implement alignment & justification

### **Phase 2 — Containers**

* Container
* Panel

### **Phase 3 — Content**

* Label

### **Phase 4 — Interaction**

* Button
* event-driven state machine

### **Phase 5 — Viewport**

* ScrollArea (SPEC-086)

### **Phase 6 — Tier-2**

* FlowGrid

---

## **14. Implementation Notes**

### **14.1 Legacy Cleanup**

Row / Column / Spacer have been **deleted** from the codebase:

* Row → FlexRow (Flex::row())
* Column → FlexColumn (Flex::column())
* Spacer::fixed(N) → FixedSpacer::vertical(N) or FixedSpacer::horizontal(N)
* Spacer::flex() → FlexSpacer::new() with tree.set_flex(node, FlexParams::flexible(1.0))

All examples and tests have been migrated to use Flex-based layouts with proper spacer widgets.

### **14.2 Performance**

* Flex is O(n)
* Container/Panel negligible
* Button negligible
* ScrollArea cost dominated by overscroll/scrollbar logic
* Label cost dominated by shaping (cached)

### **14.3 Future Work**

Tier-2 and Tier-3 components:

* ListView
* TreeView
* Menu / Toolbar
* TextField (multi-SPEC system)
* Splitters / Tabs
* Windows / Layers

These depend on the Tier-1 foundation.

---

## **15. Flex Layout Invariants**

These are the **laws** that Flex must follow. If any invariant breaks, Flex becomes unpredictable.

### **15.1 Measurement Invariants**

1. Children are measured with **loose constraints** (min=0 for both axes)
2. Children must return **natural size** unless explicitly constrained
3. Measurement must **never** depend on sibling results
4. Spacer (FlexSpacer) must report **0** during natural measurement pass

### **15.2 Distribution Invariants**

1. `final_main >= 0` for all children
2. `Sum(final_main + spacing) ≈ available_main` (within tolerance)
3. Grow distributes **free** space proportionally to grow factors
4. Shrink distributes **deficit** proportionally to `base * shrink`

### **15.3 Alignment Invariants**

1. Cross-axis positioning must respect: Start / Center / End / Stretch
2. Tight cross constraints override natural sizes
3. Children measured with loose constraints → natural sizes preserved → alignment works
4. Stretch alignment applies tight cross-axis constraints during second-pass relayout

### **15.4 Layout Tree Invariants**

1. Second-pass relayout **must not** modify main-axis size
2. `set_child_rect` called exactly once per child per pass
3. `relayout_child_with_constraints` uses tight constraints from assigned rect

### **15.5 Wrap Invariants**

1. A line ends when `total_base + spacing > max_main`
2. Each line has independent distribution
3. Cross size of each line is recomputed **after** second-pass relayout
4. Lines use spacing between them

---

## **16. Cross-Axis Alignment Semantics**

### **16.1 How Alignment Works**

1. Children measured with loose constraints → report natural size
2. `max_child_cross` computed from natural sizes
3. `cross_size_final = max(max_child_cross, cross_min_constraint).min(cross_max_constraint)`
4. For each child, `origin_cross` computed based on alignment:
   * Start: `origin_cross = 0`
   * Center: `origin_cross = (cross_size_final - item_cross) / 2`
   * End: `origin_cross = cross_size_final - item_cross`
   * Stretch: `origin_cross = 0`, item stretches to fill

### **16.2 Stretch Alignment**

Stretch is special:
* First pass sets `item_cross = cross_size_final`
* Second-pass relayout uses **tight** cross-axis constraints
* Child layout receives constraints where `min_height = max_height = cross_size_final`

---

## **17. Multi-Pass Layout Algorithm**

Flex performs layout in multiple passes:

### **Pass 1: Natural Measurement**
```
for each child:
    measure with loose constraints (min=0, max=parent_max)
    record base_main = basis > 0 ? basis : measured_main
    record cross = measured_cross
```

### **Pass 2: Distribution**
```
total_base = sum(base_main) + spacing
if total_base <= available_main:
    distribute free space to items with grow > 0
else:
    distribute deficit to items with shrink > 0
```

### **Pass 3: Rect Assignment**
```
for each child:
    compute origin_cross based on alignment
    set_child_rect(child, rect)
```

### **Pass 4: Second-Pass Relayout**
```
for each child:
    constraints = tight(rect.width) × (stretch ? tight(rect.height) : loose)
    relayout_child_with_constraints(child, constraints)
```

### **Pass 5: Final Positioning**
```
update positions if children changed size during second pass
(mainly affects wrap mode where line heights may change)
```

---

## **18. Spacer Semantics**

### **18.1 FlexSpacer Behavior**

* **Natural measurement pass (loose constraints):**
  - `constraints.min_width = 0` → returns 0
  - FlexSpacer reports zero size, allowing flex distribution to compute its share

* **Second-pass relayout (tight constraints):**
  - `constraints.min_width = distributed_width` → returns distributed_width
  - FlexSpacer yields the exact distributed size

### **18.2 FixedSpacer Behavior**

* Always returns its fixed size, clamped to constraints
* Does not participate in flex grow/shrink distribution
* Maintains exact dimensions regardless of container size

---

## **19. FlowGrid Built on Flex**

FlowGrid is **not** an independent layout engine. It is a configuration preset:

> "FlowGrid uses FlexWrap + basis sizing; it is a configuration, not an engine."

```rust
FlowGrid::new(min_item_width) =
    Flex::row()
        .with_wrap()
        .with_align_items(Alignment::Stretch)
        .with_spacing(spacing)

// Each child gets:
FlexParams {
    grow: 1.0,
    shrink: 1.0,
    basis: min_item_width,
}
```

FlowGrid requires `min_item_width > 0` for proper wrap behavior (enforced by debug assertion).
