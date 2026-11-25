# SPEC-HAL-109: TextArea ↔ Editing Substrate & Viewport Wiring Invariants

**Status:** Implemented
**Scope:** TextArea widget only
**Layer:** Widget wiring (NOT substrate, NOT policy)
**Depends on:** SPEC-HAL-052, SPEC-HAL-107, SPEC-HAL-108
**Constrains:** `src/widgets/text_area.rs`

---

## 1. Motivation

The editing substrate (SPEC-HAL-052) and editor invariants (SPEC-HAL-107) are **complete and correct**.

The current TextArea implementation is **functionally broken** due to missing and incorrect **wiring invariants**, including:

* uncontrolled vertical growth instead of clipping + scrolling
* caret and selection leaving the visible viewport
* blocked or misrouted keyboard navigation
* newline insertion failures despite substrate correctness

These failures are **not algorithmic**.
They are **ownership and wiring violations**.

This SPEC exists to **lock down exactly how TextArea must connect**:

* editing substrate
* scrolling container
* geometry constraints
* layout cache
* rendering viewport

This SPEC is **binding**.
Any TextArea behavior violating it is a bug.

---

## 2. Explicit Inheritance

TextArea **inherits all editor-level invariants** defined in:

* **SPEC-HAL-107 (TextField ↔ Editing Substrate Wiring Invariants)**

Including but not limited to:

* cursor semantics
* selection semantics
* insertion / deletion invariants
* mouse hit-testing rules
* boundary authority
* deletion modifier semantics
* “Nothing Weird Happens” guarantees

SPEC-HAL-109 **does not restate or redefine** those rules.

This SPEC defines **only what is different for TextArea**.

---

## 3. Non-Goals

This SPEC explicitly does **not**:

* redesign editor semantics
* modify substrate algorithms
* introduce new APIs
* define scrolling math
* define scrollbar rendering
* define text shaping or layout algorithms
* introduce policy (see SPEC-HAL-108)

---

## 4. Terminology

* **Viewport**: the visible clipped region of the TextArea
* **Scroll Offset**: logical offset applied to layout output
* **Content Height**: total layout height of text content
* **Geometry Policy**: as defined in SPEC-HAL-108
* **Substrate**: `src/core/editing.rs`

---

## 5. Absolute Wiring Invariants (Non-Negotiable)

1. **TextArea MUST NOT modify editor semantics**
2. **TextArea MUST NOT compute cursor or selection logic**
3. **TextArea MUST NOT reason about UTF-8**
4. **All edits and navigation MUST go through substrate APIs**
5. **Viewport, scrolling, and clipping MUST NOT affect document state**

Any violation = SPEC breach.

---

## 6. Geometry & Growth Wiring Invariants

TextArea geometry is governed by **policy**, but enforced by **wiring**.

### 6.1 Height Behavior

1. TextArea MUST respect geometry policy bounds:

    * `initial_height`
    * `min_height`
    * `max_height`
2. TextArea MUST NOT grow beyond `max_height`
3. When content height exceeds allowed height:

    * growth stops
    * overflow is redirected to scrolling (if allowed)
4. TextArea MUST NOT auto-expand indefinitely

### 6.2 Forbidden Behavior

* TextArea MUST NOT resize itself based on content without policy permission
* TextArea MUST NOT bypass clipping by expanding layout tree
* Layout output MUST NOT escape the viewport

---

## 7. Viewport & Clipping Invariants

1. TextArea MUST clip all rendered content to its viewport
2. Text outside the viewport MUST NOT be visible
3. Caret and selection rendering MUST be clipped consistently with text
4. Clipping MUST NOT mutate:

    * cursor
    * selection
    * document text

If text is visible beyond the viewport, wiring is wrong.

---

## 8. Scrolling Wiring Invariants

### 8.1 Scroll Ownership

1. Scroll offset is owned by TextArea, not substrate
2. Substrate MUST remain scroll-agnostic
3. Scroll offset MUST be applied **after layout**, never before

### 8.2 Scroll Behavior

4. When content fits viewport:

    * scroll offset MUST be zero
5. When content exceeds viewport and scrolling is allowed:

    * scroll offset MAY change
6. When scrolling is disallowed:

    * overflow MUST be clipped, not scrolled

### 8.3 Forbidden

* Scrolling MUST NOT:

    * move cursor positions
    * alter selection
    * affect boundary computation
    * affect insertion or deletion semantics

---

## 9. Caret & Selection Visibility Invariants

1. Caret MUST be renderable whenever it exists
2. If caret moves outside viewport:

    * TextArea MUST adjust scroll offset to reveal it
3. Selection head MUST be renderable
4. Anchor MAY be outside viewport, but head MUST be visible
5. Visibility adjustment MUST NOT:

    * change cursor position
    * modify selection
    * mutate document

Failure to see caret or selection = wiring bug.

---

## 10. Keyboard Navigation Wiring

1. TextArea MUST forward all navigation keys to substrate:

    * arrows
    * Home / End
    * Ctrl/Opt + arrows
2. Modifier semantics MUST match SPEC-HAL-107
3. TextArea MUST NOT block navigation due to:

    * scrolling
    * viewport clipping
    * focus handling
4. Vertical navigation MUST integrate with scrolling:

    * caret movement triggers scroll, not failure

If Ctrl/Opt + Arrow does nothing, wiring is broken.

---

## 11. Multi-Line Editing Invariants

1. TextArea MUST allow newline insertion:

    * at start of line
    * at end of line
    * on empty lines
2. Consecutive newlines MUST be allowed
3. Enter behavior MUST obey SPEC-HAL-107 invariants
4. TextArea MUST NOT impose artificial newline limits

If substrate allows it and TextArea blocks it → bug.

---

## 12. Layout Cache Invalidation (TextArea-Specific)

Layout cache MUST be invalidated when **any** of the following change:

* document content
* cursor position
* selection head
* scroll offset
* viewport size

Stale layout with moving cursor = violation.

---

## 13. Rendering Order Invariants

1. Layout → scroll offset → clipping → rendering
2. Rendering MUST reflect document order
3. Scroll offset MUST NOT reorder content
4. Visual order MUST match byte order

If text appears reordered during scrolling, layout is wrong.

---

## 14. Explicitly Forbidden Operations

TextArea MUST NOT:

* resize itself outside geometry policy
* compute caret X/Y independently
* suppress substrate navigation
* invent scroll-based cursor movement
* silently clamp edits
* special-case newlines
* diverge from SPEC-HAL-107 semantics

---

## 15. Compliance Checklist (Mechanical)

A TextArea implementation is SPEC-HAL-109 compliant **iff**:

* SPEC-HAL-107 invariants hold unchanged
* Geometry growth is policy-bound
* Overflow is clipped or scrolled, never grown
* Caret and selection remain visible
* Keyboard navigation works identically to TextField
* Substrate behavior is untouched

---

## 16. Enforcement

Any future TextArea change violating this SPEC:

* MUST be rejected
* MUST be reverted
* MUST reference this SPEC during review

---

### Final note (intentional, not decorative)

If TextArea "mostly works" but:

* text escapes viewport
* caret disappears
* Enter stops working
* Ctrl/Opt navigation breaks

then **TextArea is broken**, regardless of demo appearance.

This SPEC exists to prevent exactly that.

---

## Stage B: Coordinate Space Consistency

### B.1 Stage Ownership

Stage B owns:

* Coordinate space definitions
* Transform equations between spaces
* Hit-test coordinate mapping
* Visual output coordinate mapping
* Clipping rect coordinate authority

Stage B does NOT own:

* Scrolling mechanics (ensure_caret_visible)
* Geometry growth policy
* Rendering order (Stage D)
* Pointer cursor UX
* Substrate logic

---

### B.2 Coordinate Space Definitions

Five coordinate spaces exist. Names are canonical.

**S_content** — Content coordinates

* Origin: top-left of laid-out text content
* Units: logical pixels
* Range: `(0, 0)` to `(content_width, content_height)`
* Owner: `TextLayoutCache` (lines, boundaries, glyphs)
* Invariant: Content coordinates are **scroll-agnostic**

**S_viewport** — Viewport coordinates

* Origin: top-left of visible text region (inside padding)
* Units: logical pixels
* Range: `(0, 0)` to `(viewport_width, viewport_height)`
* Owner: TextArea layout pass
* Invariant: Viewport size ≤ widget size minus padding

**S_layout** — Widget-local layout coordinates

* Origin: top-left of widget bounding box
* Units: logical pixels
* Range: `(0, 0)` to `(widget_width, widget_height)`
* Owner: TextArea layout pass
* Invariant: Layout coordinates are widget-relative, not scene-relative

**S_world** — Scene/window coordinates

* Origin: top-left of application window
* Units: logical pixels (scaled by DPI)
* Range: unbounded
* Owner: Renderer / scene graph
* Invariant: World = Layout + widget_rect.origin

**S_hit** — Hit-test input coordinates

* Origin: depends on event source
* For pointer events: S_world (then transformed to S_content)
* For caret placement: must map to S_content
* Owner: `handle_event`
* Invariant: Hit-test coordinates MUST be transformed to S_content before substrate calls

---

### B.3 Canonical Transforms

#### B.3.1 Vertical transforms (scrolling axis)

```
content_to_viewport_y(y_content) = y_content - scroll_y
viewport_to_content_y(y_viewport) = y_viewport + scroll_y
```

#### B.3.2 Horizontal transforms

TextArea supports horizontal scrolling for single-line mode and long lines.

```
content_to_viewport_x(x_content) = x_content - scroll_x
viewport_to_content_x(x_viewport) = x_viewport + scroll_x
```

**INV-B.H1 — Axis-symmetric horizontal scroll**

TextArea MUST apply horizontal scroll offset the same way it applies vertical scroll offset.
Hit-testing during drag-selection MUST include anticipated `scroll_x` delta the same way it does for `scroll_y`.
Paint, caret, selection, and hit-test MUST share the same `scroll_x`.

#### B.3.3 Viewport to layout

```
layout_to_viewport(p_layout) = p_layout - (padding_left, padding_top)
viewport_to_layout(p_viewport) = p_viewport + (padding_left, padding_top)
```

#### B.3.4 Layout to world

```
layout_to_world(p_layout) = p_layout + widget_rect.origin
world_to_layout(p_world) = p_world - widget_rect.origin
```

---

### B.4 Transform Invariants

**INV-B.T1 — Transform composition is associative**

For any point P in S_content:
```
world(P) = widget_rect.origin + padding + (P.x, P.y - scroll_y)
```

**INV-B.T2 — Transforms are invertible**

Every transform has a well-defined inverse.
Round-trip through transforms MUST preserve coordinates (within floating-point epsilon).

**INV-B.T3 — Scroll offset is the ONLY dynamic transform**

All other transforms (padding, widget_rect) are fixed for a given frame.
Only `scroll_y` changes during interaction without triggering re-layout.

**INV-B.T4 — Transform application order**

Content → (apply scroll) → Viewport → (apply padding) → Layout → (apply widget_rect) → World

Reversing: World → Layout → Viewport → Content

---

### B.5 Hit-Test Mapping Invariants

**INV-B.HIT1 — Pointer events arrive in S_world**

Pointer events from the platform provide coordinates in S_world.

**INV-B.HIT2 — Hit-test requires S_content coordinates**

The substrate's `hit_test()` function operates in S_content.
Widget MUST transform pointer coordinates before calling substrate.

**INV-B.HIT3 — Hit-test transform equation**

```
content_x = world_x - widget_rect.x - padding_left
content_y = world_y - widget_rect.y - padding_top + scroll_y
```

**INV-B.HIT4 — Hit-test is valid even when clipped**

If pointer is within widget bounds but content is scrolled out of view,
hit-test MUST still map to valid S_content coordinates.
The substrate returns the nearest valid boundary.

**INV-B.HIT5 — Caret placement uses hit-test result directly**

After hit-test returns a byte position, cursor is set to that position.
No additional coordinate transforms apply to the byte position.

---

### B.6 Visual Mapping Invariants

**INV-B.VIS1 — Caret rect is computed in S_content**

Caret position → boundary lookup → `(x_content, y_content, width, height)`

**INV-B.VIS2 — Caret rendering requires S_viewport transform**

Before rendering, caret rect MUST be transformed:
```
caret_viewport_y = caret_content_y - scroll_y
```

**INV-B.VIS3 — Selection rects follow same transform**

All selection rectangles are computed in S_content, then transformed to S_viewport.

**INV-B.VIS4 — Text glyphs follow same transform**

Glyph positions from layout are in S_content.
Rendering applies scroll offset to produce S_viewport positions.

**INV-B.VIS5 — Visual order matches document order**

Transform application MUST NOT reorder visual elements.
Line N appears above Line N+1 in both S_content and S_viewport.

---

### B.7 Clipping Invariants

**INV-B.CLIP1 — Clip rect is defined in S_layout**

```
clip_rect = Rect {
    x: padding_left,
    y: padding_top,
    width: viewport_width,
    height: viewport_height,
}
```

**INV-B.CLIP2 — Clip rect bounds the viewport**

Content outside clip_rect MUST NOT be visible.

**INV-B.CLIP3 — Clipping is visual-only**

Clipping MUST NOT affect:
* Cursor byte position
* Selection anchor/head
* Document text
* Hit-test results (before clipping check)
* Boundary computation

**INV-B.CLIP4 — Caret clipping matches content clipping**

If text on line N is clipped, caret on line N MUST also be clipped.
Partial caret visibility is allowed (caret may be half-visible at edge).

**INV-B.CLIP5 — Clipping does not prevent hit-test**

Even if content is clipped, pointer events within widget bounds
MUST still produce valid hit-test results in S_content.

---

### B.8 The "Red Line" Phenomenon (Explicit)

**Observed bug:** When content_height > viewport_height and scroll_y == 0,
caret or selection drawn beyond viewport appears as a "red line" at viewport edge.

**INV-B.RED1 — Clipping is mandatory for overflow**

When `caret_content_y - scroll_y > viewport_height`:
* Caret MUST be clipped (not drawn at viewport edge)
* Selection MUST be clipped (not clamped to viewport)

**INV-B.RED2 — Hit-test remains valid during overflow**

Even when caret is clipped (not visible), clicking within the widget
MUST correctly map to S_content and place cursor at the correct byte position.

**INV-B.RED3 — Overflow state does not corrupt coordinates**

When content overflows viewport:
* S_content coordinates remain valid
* Transforms remain invertible
* Hit-test mapping remains correct
* Only VISIBILITY is affected, not POSITION

**INV-B.RED4 — Scroll adjustment is separate concern**

Ensuring caret visibility via scroll adjustment is NOT a Stage B concern.
Stage B guarantees coordinate consistency; Stage C handles scroll-to-caret.

---

### B.9 Forbidden Behaviors (Stage B)

**FORBID-B.1 — Coordinate space confusion**

Widget MUST NOT:
* Pass S_world coordinates to substrate expecting S_content
* Render S_content coordinates without scroll transform
* Apply scroll offset to hit-test input twice
* Mix coordinate spaces in a single calculation

**FORBID-B.2 — Implicit transforms**

Widget MUST NOT:
* Assume scroll_y == 0
* Assume viewport == content size
* Assume padding == 0
* Hard-code coordinate offsets

**FORBID-B.3 — Coordinate-dependent clipping**

Widget MUST NOT:
* Clamp cursor position based on viewport
* Reject hit-test based on visual clipping
* Modify byte positions based on scroll state

**FORBID-B.4 — Transform side effects**

Coordinate transforms MUST NOT:
* Modify scroll_y
* Trigger layout
* Emit events
* Mutate any state

---

### B.10 Stage B Test Anchors

Tests for Stage B MUST verify:

1. **T-B.1**: Hit-test at viewport origin maps to content (0 + scroll_y)
2. **T-B.2**: Hit-test at viewport bottom maps to content (viewport_height + scroll_y)
3. **T-B.3**: Caret at content_y=0 with scroll_y=0 renders at viewport_y=0
4. **T-B.4**: Caret at content_y=100 with scroll_y=50 renders at viewport_y=50
5. **T-B.5**: Caret at content_y=100 with scroll_y=0, viewport_height=50 is clipped
6. **T-B.6**: Round-trip transform (content→world→content) preserves position

---

### B.11 Stage B Summary

Stage B establishes that:

* Five coordinate spaces exist with fixed definitions
* Transforms between spaces are explicit and invertible
* Hit-test coordinates map correctly regardless of scroll/clip state
* Visual output coordinates map correctly with scroll transform
* Clipping is visual-only and does not affect coordinate semantics
* The "red line" bug is a clipping enforcement failure, not a coordinate failure

---

## Stage C: Scroll-to-Caret Discipline

### C.1 Stage Ownership

Stage C owns:

* Rules for when `scroll_y` must be adjusted
* Rules for how much `scroll_y` changes
* Caret visibility guarantees
* Scroll bounds enforcement

Stage C does NOT own:

* Coordinate space definitions (Stage B)
* Transform equations (Stage B)
* Clipping mechanics (Stage B)
* Scroll bar rendering (Stage D)
* Pointer wheel handling (out of scope)
* Animation / easing (out of scope)
* ScrollContainer internals

---

### C.2 State Definitions

Stage C reads:

* `scroll_y` — current vertical scroll offset
* `viewport_height` — visible region height
* `content_height` — total laid-out content height
* `caret_content_y` — caret **top edge** in S_content (not center, not bottom)
* `caret_height` — caret height (typically line_height)

Stage C mutates:

* `scroll_y` — and ONLY scroll_y

Stage C MUST NOT mutate:

* `cursor.pos`
* `selection.anchor`
* `selection.head`
* `document` text
* `boundaries`
* Any coordinate space definition

---

### C.3 Visibility Definitions

**Caret visibility** is defined in S_viewport coordinates.

```
caret_viewport_top = caret_content_y - scroll_y
caret_viewport_bottom = caret_viewport_top + caret_height
```

**INV-C.VIS1 — Caret fully visible**

Caret is fully visible iff:
```
caret_viewport_top >= 0 AND caret_viewport_bottom <= viewport_height
```

**INV-C.VIS2 — Caret partially visible**

Caret is partially visible iff:
```
caret_viewport_bottom > 0 AND caret_viewport_top < viewport_height
```
(at least some portion is within viewport)

**INV-C.VIS3 — Caret fully clipped**

Caret is fully clipped iff:
```
caret_viewport_bottom <= 0 OR caret_viewport_top >= viewport_height
```

---

### C.4 Scroll Adjustment Rules

**INV-C.ADJ1 — Adjustment trigger**

Scroll adjustment MUST occur when caret is NOT fully visible (INV-C.VIS1 fails).

**INV-C.ADJ2 — Adjustment timing**

Scroll adjustment occurs AFTER cursor/selection mutation, NEVER before.

Sequence:
1. Substrate updates cursor position
2. Widget detects caret visibility failure
3. Widget adjusts scroll_y
4. Next paint reflects new scroll position

**INV-C.ADJ3 — Minimal scroll (caret above viewport)**

When caret is above viewport (`caret_viewport_top < 0`):
```
new_scroll_y = caret_content_y
```
This places caret at viewport top edge.

**INV-C.ADJ4 — Minimal scroll (caret below viewport)**

When caret is below viewport (`caret_viewport_bottom > viewport_height`):
```
new_scroll_y = caret_content_y + caret_height - viewport_height
```
This places caret at viewport bottom edge.

**INV-C.ADJ5 — No adjustment when visible**

When caret is fully visible (INV-C.VIS1 holds):
```
new_scroll_y = scroll_y  // unchanged
```

**INV-C.ADJ6 — Scroll direction matches movement**

* Cursor moves down → scroll_y increases (or unchanged)
* Cursor moves up → scroll_y decreases (or unchanged)

Scroll MUST NOT move opposite to cursor movement direction.

---

### C.5 Scroll Bounds Invariants

**INV-C.BOUND1 — Minimum scroll**

```
scroll_y >= 0
```

Negative scroll is forbidden.

**INV-C.BOUND2 — Maximum scroll**

```
scroll_y <= max(0, content_height - viewport_height)
```

Cannot scroll past content end.

**INV-C.BOUND3 — No scroll when content fits**

When `content_height <= viewport_height`:
```
scroll_y == 0
```

Scrolling is meaningless when all content is visible.

**INV-C.BOUND4 — Bounds enforcement after adjustment**

After any scroll adjustment:
```
scroll_y = clamp(scroll_y, 0, max(0, content_height - viewport_height))
```

---

### C.6 Idempotence Invariants

**INV-C.IDEM1 — No jitter**

Calling ensure_caret_visible twice with unchanged cursor position
MUST produce identical scroll_y values.

**INV-C.IDEM2 — Stable when visible**

If caret is fully visible before ensure_caret_visible:
* scroll_y MUST NOT change
* No "centering" or "preferred position" logic

**INV-C.IDEM3 — Single adjustment per cursor move**

Each cursor mutation triggers at most ONE scroll adjustment.
No cascading adjustments. No re-entrancy.

---

### C.7 Independence Invariants

**INV-C.IND1 — No hit-test dependency**

Scroll adjustment logic MUST NOT:
* Call hit_test()
* Read pointer position
* Depend on mouse button state

**INV-C.IND2 — No selection dependency**

Scroll adjustment is based on CARET position only.
Selection anchor position does not affect scroll adjustment.

**INV-C.IND3 — No layout dependency**

Scroll adjustment reads layout output (boundaries, line positions)
but MUST NOT trigger layout recomputation.

**INV-C.IND4 — No rendering dependency**

Scroll adjustment MUST NOT depend on:
* Previous frame's rendered state
* Glyph atlas state
* GPU buffer state

---

### C.8 Interaction with Stage B

**INV-C.STAGE1 — Stage B transforms remain valid**

After scroll adjustment:
* All Stage B coordinate transforms remain correct
* INV-B.T1–T4 still hold
* INV-B.HIT1–HIT5 still hold

**INV-C.STAGE2 — Scroll adjustment is a transform parameter change**

Scroll adjustment changes the `scroll_y` parameter in Stage B transforms.
It does NOT change the transform equations themselves.

**INV-C.STAGE3 — Clipping rules unchanged**

After scroll adjustment:
* INV-B.CLIP1–CLIP5 still apply
* Content outside new viewport is still clipped

---

### C.9 Forbidden Behaviors (Stage C)

**FORBID-C.1 — Cursor mutation during scroll**

ensure_caret_visible MUST NOT modify cursor.pos.

**FORBID-C.2 — Selection mutation during scroll**

ensure_caret_visible MUST NOT modify selection.anchor or selection.head.

**FORBID-C.3 — Document mutation during scroll**

ensure_caret_visible MUST NOT modify document text.

**FORBID-C.4 — Over-scroll**

scroll_y MUST NOT exceed bounds defined in INV-C.BOUND1–BOUND2.

**FORBID-C.5 — Scroll before cursor move**

Widget MUST NOT adjust scroll_y in anticipation of cursor movement.
Adjustment is reactive, not predictive.

**FORBID-C.6 — Centering logic**

Widget MUST NOT center caret in viewport.
Minimal scroll only—caret at edge, not middle.

**FORBID-C.7 — Scroll on focus alone**

Gaining focus MUST NOT trigger scroll adjustment
unless caret is already outside viewport.

**FORBID-C.8 — Scroll during text input**

Text insertion may move caret, which may trigger scroll.
But the scroll adjustment is due to caret movement, not text insertion directly.
(Causation: insert → caret moves → scroll adjusts)

---

### C.10 Stage C Test Anchors

Tests for Stage C MUST verify:

1. **T-C.1**: Caret at content_y=0, scroll_y=0, viewport_height=100 → no scroll adjustment
2. **T-C.2**: Caret at content_y=150, scroll_y=0, viewport_height=100 → `scroll_y == caret_content_y + caret_height - viewport_height` (caret at bottom edge)
3. **T-C.3**: Caret at content_y=50, scroll_y=100, viewport_height=100 → `scroll_y == caret_content_y` (caret at top edge)
4. **T-C.4**: Caret moves down repeatedly → scroll_y never decreases
5. **T-C.5**: Caret moves up repeatedly → scroll_y never increases
6. **T-C.6**: content_height=50, viewport_height=100 → scroll_y always 0
7. **T-C.7**: After ensure_caret_visible, cursor.pos unchanged
8. **T-C.8**: After ensure_caret_visible, selection unchanged
9. **T-C.9**: Two calls to ensure_caret_visible with same cursor → same scroll_y (idempotence)
10. **T-C.10**: scroll_y never negative after any operation
11. **T-C.11**: scroll_y never exceeds (content_height - viewport_height) after any operation

---

### C.11 Stage C Summary

Stage C establishes that:

* Caret visibility is defined precisely in S_viewport coordinates
* Scroll adjustment occurs AFTER cursor mutation, never before
* Adjustment is minimal: caret at edge, not centered
* No adjustment when caret already visible (idempotence)
* Scroll bounds are strictly enforced (no negative, no over-scroll)
* Scroll adjustment NEVER mutates cursor, selection, or document
* Stage B coordinate guarantees remain intact after scroll adjustment

---

## Stage D: Paint Order + Clipping Enforcement

### D.1 Stage Ownership

Stage D owns:

* Paint-time transform usage (which scroll offset is used for rendering)
* Paint order of: background → selection → text → caret
* Clip stack discipline (push/pop) for viewport clipping
* Ensuring caret/selection/text all share identical scroll transform
* Eliminating "red line" artifacts by **never clamping geometry to viewport edges**

Stage D does NOT own:

* Coordinate space definitions / equations (Stage B)
* Scroll-to-caret math (Stage C)
* Hit-test mapping (Stage B)
* Scroll wheel behavior
* Layout, shaping, boundaries generation

---

### D.2 Render-time Scroll Offset Authority

**INV-D.S1 — Render uses visual scroll only**

All rendering that depends on scroll uses:

* `scroll_y_visual = self.state.scroll.borrow().visual_offset().1`

This includes:

* Text glyph positions
* Selection rectangles
* Caret rectangle

**FORBID-D.S1 — Target scroll used for rendering is forbidden**

Using `target_offset().1` for any paint-time transform is forbidden.

**INV-D.S2 — Logic uses target scroll only**

All non-render logic uses:

* `scroll_y_target = self.state.scroll.borrow().target_offset().1`

This includes:

* ensure_caret_visible (Stage C)
* hit-test mapping (Stage B)
* any post-mutation decisions

**FORBID-D.S2 — Visual scroll used for logic is forbidden**

Using `visual_offset().1` for hit-test or ensure_caret_visible is forbidden.

---

### D.3 Paint Order Invariants

**INV-D.O1 — Paint order is fixed**

Within the text viewport clip:

1. Background (viewport fill, if any)
2. Selection (all selection rects)
3. Text glyphs
4. Caret

**FORBID-D.O1 — Caret under text is forbidden**

Caret must be painted after text, always.

**FORBID-D.O2 — Selection over caret is forbidden**

Selection must be painted before caret, always.

---

### D.4 Clip Rect Authority

**INV-D.CLIP1 — Clip rect is defined in S_layout and equals viewport**

The clip rect is:

* Origin: `(padding_left, padding_top)` in S_layout
* Size: `(viewport_width, viewport_height)` in S_layout

**INV-D.CLIP2 — Clip stack is balanced**

Every `push_clip(clip_rect)` MUST have exactly one matching `pop_clip()` on all code paths.

**INV-D.CLIP3 — Clip encloses selection + text + caret**

Selection, text, and caret must be emitted while the viewport clip is active.

**FORBID-D.CLIP1 — No partial clipping hacks**

Forbidden:

* Clamping caret_y to `[0, viewport_height]`
* Drawing a "proxy caret" at edge when caret is off-screen
* "Fixing" overflow by changing geometry instead of clipping

That specific behavior is the red-line bug generator.

---

### D.5 Paint-space Mapping Invariants

Definitions:

* `p_content` is in **S_content**
* `p_viewport` is in **S_viewport**
* `p_layout` is in **S_layout**

Let:

* `scroll_y_visual` be the ONLY dynamic render-time parameter

**INV-D.XFORM1 — Content → viewport transform**

* `x_viewport = x_content`
* `y_viewport = y_content - scroll_y_visual`

**INV-D.XFORM2 — Viewport → layout transform**

* `x_layout = x_viewport + padding_left`
* `y_layout = y_viewport + padding_top`

**INV-D.XFORM3 — Identical transform for caret/selection/text**

Caret, selection rects, and glyph positions MUST all use the exact same mapping chain.

**FORBID-D.XFORM1 — Double scroll is forbidden**

Applying scroll in both geometry generation and paint emission is forbidden.

**FORBID-D.XFORM2 — Mixed scroll values are forbidden**

In a single paint pass, all three (text/selection/caret) must use the same `scroll_y_visual`.

---

### D.6 Caret Paint Discipline

**INV-D.CARET1 — Caret geometry is computed in S_content**

Caret rect is derived from boundaries / line metrics in S_content.

**INV-D.CARET2 — Caret is transformed then clipped, never clamped**

Caret rect becomes S_layout via INV-D.XFORM1–2, then clip applies.

**FORBID-D.CARET1 — Edge caret is forbidden**

If caret is fully outside viewport (above or below), Stage D must emit either:

* nothing (fully clipped), or
* a rect that gets clipped naturally

It must never "snap" to the viewport edge.

---

### D.7 Selection Paint Discipline

**INV-D.SEL1 — Selection rects are computed in S_content**

Selection rectangles are generated in S_content (line-based segments).

**INV-D.SEL2 — Selection rects are transformed then clipped**

Transform to S_layout using the same mapping as caret/text, then clip applies.

**FORBID-D.SEL1 — Viewport-based selection clamping is forbidden**

Selection must not be truncated by altering its geometry. Only the clip may truncate visibility.

---

### D.8 Text Paint Discipline

**INV-D.TEXT1 — Glyph positions are in S_content**

Text layout output positions are treated as S_content.

**INV-D.TEXT2 — Glyph emission uses render-time visual scroll**

All glyph positions are transformed using `scroll_y_visual` before emission.

---

### D.9 Stage D Forbidden Behaviors

**FORBID-D.1 — Any "red line" fallback**

No drawing at viewport edge as a substitute for offscreen geometry.

**FORBID-D.2 — Paint-time mutation**

Paint must not mutate:

* scroll offsets
* cursor / selection
* document
* layout_cache

**FORBID-D.3 — Clip affects semantics**

Clip must not affect:

* cursor.pos
* selection ranges
* hit-test output
* ensure_caret_visible decisions

---

### D.10 Stage D Test Anchors

These tests require **paint output inspection** (scene command stream / clip stack / emitted rects).

1. **T-D.1 Clip present**
   Painting TextArea emits a viewport `push_clip` with rect exactly `(padding_left, padding_top, viewport_w, viewport_h)`.

2. **T-D.2 Clip balanced**
   For every paint call, clip stack depth ends where it started.

3. **T-D.3 Caret clipped below**
   With `scroll_y_visual = 0`, caret on a line whose `caret_viewport_top >= viewport_height` results in **no visible caret geometry** (i.e., caret rect is emitted but clipped away, or not emitted). No edge caret.

4. **T-D.4 Caret clipped above**
   With `scroll_y_visual` large, caret whose `caret_viewport_bottom <= 0` yields no visible caret. No edge caret.

5. **T-D.5 Selection clipped**
   Selection spanning multiple lines beyond viewport yields emitted selection rects that are clipped by viewport, with no geometry clamping.

6. **T-D.6 Shared transform**
   For the same cursor position, the relative y-delta between glyph baseline, selection rect, and caret uses the identical `scroll_y_visual` (detect by comparing emitted y positions).

7. **T-D.7 Visual vs target separation**
   If `visual_offset().1 != target_offset().1`, paint uses the visual value (detect by emitted caret y; it must match `caret_content_y - scroll_y_visual`, not target).

---

### D.11 Minimal Required Test Harness (Stage D)

To make T-D.* mechanically testable, the engine must expose, under `#[cfg(any(test, feature="test-harness"))]`, one read-only path to inspect the paint output for TextArea, for example:

* a way to retrieve the generated `Scene` / command list for a widget paint, including:

  * clip push/pop commands and rects
  * emitted caret rect draw(s)
  * emitted selection rect draw(s)
  * emitted text draw(s) with positions (or at least bounding rects)

No mutation. Pure reads only.

---

### D.12 Stage D Summary

Stage D establishes that:

* Rendering uses `visual_offset()` exclusively; logic uses `target_offset()` exclusively
* Paint order is fixed: background → selection → text → caret (within clip)
* Clip stack is balanced: every begin_clip has exactly one end_clip
* Caret, selection, and text all use identical scroll transform
* No geometry clamping—only GPU clipping truncates visibility
* No "red line" edge caret artifacts under any circumstances
* Paint is read-only: no mutation of scroll, cursor, selection, or document

---

## Stage E: Pointer Hit-Test ↔ Caret Placement Consistency

### E.1 Stage Ownership

Stage E owns:

* Pointer hit-test → byte position mapping
* Coordinate transform from S_local to S_content for hit-testing
* Caret placement from pointer events
* Interaction between hit-test and scroll offset

Stage E does NOT own:

* Paint-time transforms (Stage D)
* Scroll-to-caret logic (Stage C)
* Coordinate space definitions (Stage B)
* Keyboard navigation

---

### E.2 Hit-Test Coordinate Transform

**INV-E.XFORM1 — S_local → S_content transform**

For pointer hit-testing, the coordinate transform is:

* `x_content = x_local - padding_left`
* `y_content = y_local - padding_top + scroll_y_target`

Where `scroll_y_target = target_offset().1` (NOT visual_offset).

**INV-E.XFORM2 — Target scroll authority**

Hit-test coordinate transform MUST use `target_offset()` exclusively.

This ensures logical consistency: the user clicks at a logical position,
and the caret moves to that logical position, regardless of visual animation state.

**FORBID-E.XFORM1 — Visual offset in hit-test is forbidden**

Using `visual_offset()` for hit-test coordinate transform is forbidden.

If visual_offset differs from target_offset (animation in progress), the
hit-test must still use target_offset to maintain logical consistency.

---

### E.3 Hit-Test Output

**INV-E.HIT1 — Hit-test returns byte position**

The hit_test function maps S_content coordinates to a byte position.

**INV-E.HIT2 — No viewport clamping**

Hit-test must NOT clamp its output based on viewport bounds.

* y < 0 in S_content → maps to first line (not clamped to viewport top)
* y > content_height → maps to last line (not clamped to viewport bottom)
* Returned byte position is always valid within document bounds

**INV-E.HIT3 — Per-line vertical bounds**

Hit-test uses per-line height for y → line mapping, not fixed slot height.

---

### E.4 Caret Placement from Pointer

**INV-E.PLACE1 — Direct byte position assignment**

After hit-test, cursor.pos is set directly to the returned byte position:

* `cursor.pos = hit_pos`
* No geometry correction
* No viewport-based adjustment

**INV-E.PLACE2 — Selection update**

Selection anchor/head are updated based on modifier state:

* Plain click: `anchor = head = hit_pos` (collapse)
* Shift-click: `anchor` preserved, `head = hit_pos` (extend)

**INV-E.PLACE3 — No ensure_caret_visible for pointer events**

Pointer-initiated caret placement does NOT trigger ensure_caret_visible.

Rationale: The user clicked where they can see. If they click at the edge
and the caret ends up partially clipped, that's acceptable—they clicked there.

---

### E.5 Drag Behavior

**INV-E.DRAG1 — Drag uses same transform**

During drag (PointerEventKind::Move with active drag mode), the same
S_local → S_content transform applies.

**INV-E.DRAG2 — Drag updates cursor continuously**

Each drag move updates cursor.pos and selection.head to the hit position.

**FORBID-E.DRAG1 — No auto-scroll during drag**

Stage E does not own auto-scroll behavior. If auto-scroll is needed during
drag-to-select, it must be implemented separately and documented.

---

### E.6 Stage E Forbidden Behaviors

**FORBID-E.1 — Visual offset leakage**

Using visual_offset() anywhere in the hit-test → caret placement path is forbidden.

**FORBID-E.2 — Viewport-based byte clamping**

Clamping the returned byte position based on viewport visibility is forbidden.

**FORBID-E.3 — Geometry correction**

Adjusting cursor.pos after assignment based on any geometric consideration is forbidden.

**FORBID-E.4 — Implicit scroll during click**

A single click must not implicitly trigger scroll adjustment.
Scroll adjustment is only triggered by keyboard navigation (Stage C).

---

### E.7 Stage E Test Anchors

1. **T-E.1 Click in viewport**
   Click at (x, y) within viewport → cursor.pos matches hit_test output exactly.

2. **T-E.2 Click with scroll**
   With scroll_y=100, click at y_local=50 → hit-test receives y_content=50-padding+100.

3. **T-E.3 Visual vs target during click**
   If visual_offset=50, target_offset=100, click uses target (100) for hit-test.

4. **T-E.4 No scroll after click**
   After pointer down, scroll_y remains unchanged (no ensure_caret_visible).

5. **T-E.5 Drag maintains transform**
   During drag, each move event uses the same coordinate transform.

---

### E.8 Stage E Summary

Stage E establishes that:

* Pointer hit-test uses target_offset exclusively (no visual_offset leakage)
* Coordinate transform S_local → S_content is well-defined and consistent
* Byte position from hit-test is applied directly to cursor.pos
* No viewport-based clamping or geometry correction
* Pointer events do not trigger ensure_caret_visible
* Click behavior is logically consistent regardless of scroll animation state

---

## 17. Qualified same-line visual caret projection

**INV-109-MOTION-SCOPE.** This presentation is optional and disabled by default. Its only interpolated operation is an actually accepted, non-repeat, unmodified Left, Right, Home or End navigation that changes canonical x, preserves a collapsed selection, preserves the document revision and remains on the same current visual line. The affected visual-line source, excluding its terminating newline, MUST be printable ASCII with unambiguous current cluster attribution. All logical operations remain immediate.

**INV-109-MOTION-SPACE.** Eligibility requires the enabled current concrete mounted instance, genuine current Focus and keyboard Capture, active window and interaction domain, no active composition, fresh current cache/source binding; unchanged visual-line identity, font metrics, scale, wrapping width, viewport, clipping, scrollbar gutters and auto-height origin; settled existing horizontal and vertical scroll presentation; and both complete endpoint caret rectangles inside the actual text viewport. The existing layout cache owns destination x/y/height. The existing ScrollContainer and reveal rules remain authoritative.

**INV-109-MOTION-TIME.** One eligible leg uses the existing engine Timeline, Once, 65 ms and EaseOut. A concrete mounted TextArea owns at most one pending destination and one active handle. Retargeting starts from its currently qualified displayed x and replaces the previous leg; it creates no queue. Equal or rejected ordinary navigation does not start, restart or cancel an otherwise qualified leg.

**INV-109-MOTION-PHASE.** Event handling may record bounded owned intent. Only ordinary engine Update starts, samples, cancels and publishes the immutable projection. Paint is read-only. A later Layout or current Paint qualification failure suppresses an obsolete projection immediately; the next lawful Update cancels its handle. No new clock, phase, second Update or layout mutation is admitted.

**INV-109-MOTION-SNAP.** Accepted content mutation, IME activity, noncollapsed selection, pointer placement or drag, verified repeat, modified navigation, vertical/page/cross-line navigation, scrolling, drag autoscroll, geometry/cache/source change, Focus, Capture, window/domain or availability loss and concrete-instance retirement are canonical snap boundaries. A snap dominates every navigation destination in the same pre-Update batch. A new leg requires later separately qualified navigation after reconciliation. Restoration never revives an old leg.

**INV-109-MOTION-PAINT.** As a scoped exception to B.VIS1, D.5 and D.CARET1, only the qualified content-space caret x may use the published projection. Canonical y, width and height remain unchanged. The existing content-to-viewport transform, actual viewport clip, paint order and caret layer remain in force; text and selection are unchanged. No viewport-edge proxy, clamping or mixed scroll mapping is permitted.

**INV-109-MOTION-LIFETIME.** The existing concrete-mount-qualified animation owner retires predecessor handles on replacement/removal, preserves ownership across same-instance layout take/restore and cannot cancel a successor's handle. New motion demand exists only while an eligible leg is unsettled. Disabled, non-Capture, unavailable and otherwise ineligible paths perform no motion allocation, additional shaping or motion-source scan. Existing blink and ScrollContainer demand remain independent.

**FORBID-109-MOTION-EXPANSION.** This subsection does not admit content reveal/conceal, animated text or selection, transient prior text, TextField P6/P7/P8 reuse, substring reshaping, new text APIs, renderer changes, multiline range animation, inferred TextInput repeat or changes to another widget.


Adopted 2026-10-30 by the supervising technical reviewer under JC's explicit TextArea motion decision; JC retains final ownership and veto. The coordinated HAL-107 §10.2 and HAL-108 §13.3 additions and this section are Binding within this exact optional presentation scope. HAL-002/002A, TextField content-presentation scope, unrelated SPEC statuses, G1 and PM-016 are unchanged. Normative adoption does not establish compilation, executed conformance or native visual acceptance.
