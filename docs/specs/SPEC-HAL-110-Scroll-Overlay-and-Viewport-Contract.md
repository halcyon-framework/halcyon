# SPEC-HAL-110

**Unified Scroll Overlay & Viewport Contract**

**Status:** Implemented
**Scope:** Scrollable widgets using overlay scrollbars
**Applies to:** TextArea (now), ListView, CodeEditor, CanvasViewport (future)
**Non-Goals:** No rendering style, no animation policy, no theming

---

## 1. Problem Statement

Scrollable widgets historically fail due to **implicit, duplicated, or divergent assumptions** between:

* layout
* painting
* hit-testing
* wheel routing
* drag handling

This SPEC formalizes a **single, invariant contract** for overlay scrollbars and viewport behavior to eliminate geometry drift, input bugs, and Flex-interaction regressions.

---

## 2. Core Definitions

### 2.1 Widget Rect

The full allocated rectangle of the widget, including padding and scrollbar overlays.

### 2.2 Text / Content Viewport

The **effective renderable content area**, derived from widget rect minus:

* padding
* scrollbar gutters (only when scrollbars are visible)

This viewport is authoritative for:

* clipping
* caret visibility
* selection painting
* scroll math

---

## 3. Gutter Reservation Invariant

### INV-G1 — Conditional Gutter Reservation

Scrollbar gutters **MUST be reserved only when the scrollbar is visible**.

* Vertical scrollbar visible → reduce viewport **width**
* Horizontal scrollbar visible → reduce viewport **height**
* Both visible → reserve both, leaving a corner exclusion zone

No scrollbar → no gutter reservation.

---

## 4. Geometry Parity Invariant

### INV-G2 — Paint = Hit-Test Geometry

Any geometry used to:

* paint scrollbar tracks/thumbs
  **MUST be bit-identical** to geometry used for:
* hit-testing
* hover detection
* drag initiation

No independent calculations allowed.

---

## 5. Corner Exclusion Invariant

### INV-C1 — Bottom-Right Corner Is Non-Interactive

When **both scrollbars are visible**, the overlapping bottom-right rectangle:

```
x ≥ widget_width  − v_scrollbar_thickness
y ≥ widget_height − h_scrollbar_thickness
```

**MUST NOT**:

* initiate drag
* respond to hover
* consume pointer events

This area is inert.

---

## 6. Drag Exclusivity Invariant

### INV-D1 — Single Active Drag

At most **one scrollbar drag** may be active at a time.

* If vertical drag active → horizontal drag initiation blocked
* If horizontal drag active → vertical drag initiation blocked

No drag stealing. No handover.

---

## 7. Wheel Routing Invariant

### INV-W1 — Consume Only When Scrollable

Wheel events **MUST be consumed only if scrolling is possible**:

* Vertical wheel → consumed only if vertical scrollbar visible
* Horizontal wheel / Shift+wheel → consumed only if horizontal scrollbar visible

Otherwise:

* return `false`
* allow parent scroll containers to handle

---

## 8. Wheel Delta Semantics

### INV-W2 — No Hidden Scaling

Wheel deltas are already in **logical pixels**.

* `scroll_by(dx, dy)` must apply deltas **directly**
* No multipliers
* No acceleration hacks

Smoothing belongs to the scroll container, not the widget.

---

## 9. Height Policy Invariant

### INV-H1 — Height Policy Modes

Scrollable widgets must support exactly one of:

1. **Fixed Height**

    * Widget height constant
    * Scrolling active immediately when content exceeds viewport

2. **Auto-Grow with Max**

    * Height grows with content until `max_height`
    * Scrolling activates once max reached

3. **Unbounded Auto-Grow**

    * Height grows with content indefinitely
    * Internal scrolling never activates

---

## 10. Flex Second-Pass Immunity

### INV-F1 — Bounded Height Stabilization

Once a widget resolves to a **bounded height**, that height:

* MUST be cached
* MUST survive subsequent layout passes
* MUST ignore `max_height = INFINITY` from Flex second pass

Flex is not allowed to retroactively change scrolling semantics.

---

## 11. Hover Semantics

### INV-H2 — Hover Mirrors Drag Geometry

Hover detection:

* Uses the same geometry as paint
* Is disabled while dragging
* Resets to normal state when pointer leaves scrollbar area

Hover must never flicker or desync.

---

## 12. Test Requirements

Any widget claiming compliance with this SPEC **MUST** have tests covering:

* gutter reservation
* corner exclusion
* drag exclusivity
* wheel consumption rules
* height policy transitions
* Flex second-pass immunity

TextArea is the reference implementation.

---

## 13. Reference Implementation

Reference implementation: TextArea (master)
Normative tests: tests/text_area_scroll_semantics.rs

---

## 14. Future Work (Explicitly Out of Scope)

* Scrollbar theming
* Scroll inertia curves
* Touch gesture semantics
* Scroll snapping

---

## 15. Scrollbar Responsibility Boundary

### Scrollbar Module Owns:

* Geometry computation (track, thumb, hit_rect)
* Hit expansion rules (direction, bounds)
* Corner exclusion via track_limit parameter
* Drag state machine
* Color selection based on state

### Widgets Own:

* Visibility decisions (when to show each scrollbar)
* track_limit computation (applying corner exclusion)
* Local-to-global coordinate translation
* Passing correct viewport/content dimensions to layout()

### Widgets MUST NOT:

* Compute scrollbar geometry independently
* Bypass layout() for paint or hit-test
* Apply different geometry for paint vs hit-test
* Implement custom hit expansion logic

---

---

## 16. Scroll Locality & Viewport Authority Invariants

These invariants formalize the architectural boundary between ScrollArea and its context.

### INV-SCROLL-LOCALITY

> A ScrollArea defines an isolated scroll context.
> Scrollbars, clipping, viewport size, and scroll offsets are strictly local to the ScrollArea rect.

* ScrollArea MUST NOT assume window dimensions.
* Scrollbar height/position is derived from ScrollArea's rect, not from parent/window.
* Placing ScrollArea inside a fixed-size container: scrollbar remains inside that container.
* Window resize does NOT change scrollbar geometry unless the container changes.

### INV-VIEWPORT-AUTHORITY

> ScrollArea is the sole authority over viewport_extent.
> Child widgets MUST NOT infer or override viewport dimensions.

* Viewport rect = ScrollArea layout rect.
* Content (e.g., List) receives viewport_height via constraints.max_height.
* Content MUST NOT derive viewport from window, parent tree, or external state.
* Content MUST NOT store viewport geometry as mutable runtime state.

### INV-CONTENT-OFFSET-ONCE

> Scroll offset may be applied exactly once in the render pipeline.

* Either via paint transform OR via origin shift — never both.
* ScrollArea applies translation via `scene.begin_scroll(offset)`.
* Content widgets paint at their layout-assigned positions.
* No widget may apply additional scroll compensation.

### INV-CONTENT-HEIGHT-EXACT

> content_extent.height MUST exactly equal logical content height.

* For fixed-height: content_height = item_count × item_height
* For variable-height: content_height = sum(all item heights)
* Content extent is set by content widget's LayoutResult.content_size.
* ScrollArea accepts this value without modification.

### INV-VIEWPORT-CLIP

> Content painting MUST be clipped to viewport rect.

* ScrollArea returns clip_bounds() = viewport extent.
* paint_node() wraps children in ClipNode at this rect.
* No content may paint outside the viewport rect.
* Scrollbars are painted in overlay, on top of clipped content.

---

## 17. Final Note

This SPEC exists because scrolling bugs are **structural**, not accidental.
From this point forward, any new scrollable widget that violates this contract is **incorrect by definition**.

---

## 18. Scrollbar Visibility & Gutter Semantics (Binding Contract)

This section formalizes the rules governing scrollbar visibility, gutter reservation, and the
distinction between container and viewport geometry. These rules are **architecturally binding**
and exist to prevent the class of bugs where:

* Right-edge padding exceeds left-edge padding when scrollbar is hidden
* Scrollbar is not clickable/draggable (hit testing mismatch)
* Scrollbar space is reserved unconditionally

### 18.1 Canonical Geometry Definitions

These definitions are authoritative and must be used consistently across all code paths:

| Term | Definition | Used For |
|------|------------|----------|
| **Container rect** | Full allocation from parent (e.g., `rect` in paint, `constraints.max_*` in layout) | Scrollbar positioning, hit testing, painting |
| **Viewport extent** | Scrollable content area after insets (container minus padding/root_inset) | Scroll math, content layout, clip bounds |
| **Content extent** | Total logical size of scrollable content | Max scroll calculation |
| **Gutter** | Space occupied by visible scrollbar track | Only reserved when scrollbar visible |

### 18.2 Binding Invariants

For ScrollArea overlay scroll domains, opposing-scrollbar occlusion may extend an axis that already has genuine logical overflow relative to the full parent-assigned container; it must not create overflow on an otherwise fitting axis. Determine this eligibility from the current child content extent for both ordinary final layout and retained initialization, including after content shrink. An eligible overflowing axis retains the existing enabled-opposing-scrollbar thickness allowance so its final edge remains reachable. Child constraints, the single child layout lifecycle, exactly-once settlement and first-projection continuity remain unchanged. This scoped rule supersedes unconditional opposing-axis subtraction for ScrollArea only; other widgets' gutter contracts are unchanged.

#### INV-SCROLLBAR-GUTTER

> Scrollbar track space MUST be reserved iff the scrollbar is visible in the current layout pass.

* Scrollbars render ONLY when content exceeds viewport (`content > viewport`)
* NO unconditional space reservation in layout
* Content gets full viewport width/height when scrollbar hidden
* Scrollbar appears when content overflows; no layout jump needed
* Left/right content padding MUST be identical when scrollbar is hidden
* Padding asymmetry permitted ONLY when scrollbar is actually rendered

#### INV-SCROLLBAR-CONTAINER-SPACE

> Scrollbar geometry, hit-testing, and painting MUST use container rect, never viewport extent.

All scrollbar-related code paths must use consistent geometry:

| Operation | Geometry Source |
|-----------|-----------------|
| `paint_scrollbars()` | `rect` (container) |
| `handle_scrollbar_down()` | `container_size` field |
| `update_scrollbar_hover()` | `container_size` field |
| `ScrollbarV::layout()` / `ScrollbarH::layout()` | `widget_rect` parameter (container) |

The `viewport` parameter in `scrollbar.layout()` is used ONLY for `max_scroll` calculation,
not for positioning.

#### INV-VIEWPORT-CONTENT-ONLY

> `viewport_extent` represents scrollable content area only.
> It MUST NOT be used for scrollbar layout or hit-testing.

`viewport_extent` is authoritative for:
* Scroll domain bounds (`max_scroll = content - viewport`)
* Content widget constraints
* Clip bounds origin/size

`viewport_extent` MUST NOT be used for:
* Scrollbar track positioning
* Scrollbar hit rect dimensions
* Scrollbar thumb placement

### 18.3 Implementation Notes

The bug this section prevents:

```
// WRONG: Using viewport for scrollbar geometry
let local_rect = RectF::new(0.0, 0.0, viewport.width, viewport.height);
let layout = scrollbar.layout(local_rect, ...);

// Hit testing fails because:
// - paint uses rect (container) → scrollbar at x=490
// - hit test uses viewport → expects scrollbar at x=470
// Result: scrollbar visible but not clickable
```

The fix:

```
// CORRECT: Using container for scrollbar geometry
let local_rect = RectF::new(0.0, 0.0, container.width, container.height);
let layout = scrollbar.layout(local_rect, ...);

// All paths now agree on scrollbar position
```

### 18.4 Forbidden Patterns

The following patterns are **explicitly prohibited**:

1. **Reserving scrollbar space unconditionally**
   ```rust
   // FORBIDDEN: Always subtracting scrollbar width from content
   let content_width = viewport.width - scrollbar_thickness;
   ```

2. **Mixing viewport and container geometry**
   ```rust
   // FORBIDDEN: Using viewport for scrollbar hit rect
   let hit_rect = RectF::new(viewport.width - thickness, ...);
   // While paint uses container
   let paint_rect = RectF::new(rect.width - thickness, ...);
   ```

3. **Layout-time scrollbar assumptions**
   ```rust
   // FORBIDDEN: Assuming scrollbar will be visible during layout
   let available_width = container.width - scrollbar_thickness; // unconditional
   ```

4. **Deriving scrollbar position from content size**
   ```rust
   // FORBIDDEN: Scrollbar position based on content
   let scrollbar_x = content.width; // Wrong! Use container.width
   ```

### 18.5 Historical Context

This contract exists because the following bug occurred:

1. `clip_bounds()` returned `(0, 0, viewport.width, viewport.height)`
2. Content was positioned at `(root_inset, root_inset)`
3. Result: left edge had `root_inset` padding, right edge had `root_inset + clipping` = asymmetric

Additionally:

1. `handle_scrollbar_down()` used `viewport.width` for hit testing
2. `paint_scrollbars()` used `rect.width` for rendering
3. Result: scrollbar rendered at X=490 but hit test expected X=470 = not clickable

The invariants in this section ensure these bugs cannot recur.

### 18.6 Opt-in ScrollArea scrollbar reveal

`ScrollArea::with_scrollbar_reveal_policy` selects the existing presentation
by default. `AutoHideOnScrollOrRailProximity` starts with the scrollbars
hidden. Actual scrolling, an uncaptured pointer within a canonical overflowing
rail's expanded hit rectangle, and scrollbar dragging reveal the scrollbar.
Rail hover and scrollbar drag hold full opacity. After they end or scrolling
stops, the scrollbar holds for 1.5 seconds and fades to zero over 0.3 seconds.

The canonical container geometry, viewport and corner exclusions remain fixed
through the fade. The opted-in rail uses the settled mounted allocation for
paint and input, including when the configured viewport is narrower.
A hidden rail may be observed for reveal. It has no overlay
hit region and cannot intercept a press. At positive opacity the existing
overlay publishes the rail for normal scrollbar interaction. Paint and
retained hit projection use the same rail, ancestor scroll and clip.

---
