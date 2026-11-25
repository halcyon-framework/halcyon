# SPEC-PM-005: Layout Feedback Oscillation from Scrollbar Visibility Toggling

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Engine, Widget, Layout
**Applies To:** ScrollArea, Flex, any scrollable container wrapping content with shrinkable children
**Discovered:** SPEC-267 SplitPane + ScrollArea integration testing

---

## 0. Pathology Name

**ScrollArea–Flex Constraint Feedback Oscillation**

Also known as:
- "Accordion effect on resize"
- "Content squeezes and spreads during drag"
- "Scrollbar flickers every frame"
- "Unwanted visual movement on hover"
- "Labels compress inside scroll container"

---

## 1. Summary of the Failure

This pathology occurs when **ScrollArea passes bounded height constraints to a Flex child whose children have non-zero shrink factors**. The bounded constraints cause Flex to **compress content to fit the viewport** rather than overflowing. ScrollArea never learns the content's natural (pre-shrink) size, so it cannot show scrollbars or enable scrolling for the compressed-away content.

During dynamic resize (e.g., SplitPane drag), the viewport size changes each frame. Flex redistributes shrink amounts differently per frame, producing visible content compression/expansion ("accordion"). At boundary conditions where content barely fits/doesn't fit, scrollbar visibility toggles per frame ("flicker").

This is a **constraint contract mismatch**, not a coordinate space bug. It is distinct from PM-002 (coordinate double-application) and PM-004 (geometry authoring space drift).

---

## 2. Observable Symptoms

Any of the following are red flags:

1. Content visually compresses (labels/buttons squeezed vertically) inside a ScrollArea
2. Content height oscillates during window or panel resize ("accordion")
3. Scrollbar appears and disappears rapidly during resize ("flicker")
4. Content moves/shifts visually on hover without user scroll input
5. ScrollArea shows no scrollbar despite content logically exceeding viewport
6. Expanding the panel reveals content was being compressed — shrinking it compresses again
7. Problem only manifests with bounded Flex inside ScrollArea; same Flex without ScrollArea works fine

If any of these occur, **this pathology is active**.

---

## 3. Root Cause (Architectural)

The failure is caused by a **constraint contract mismatch** between ScrollArea and Flex.

### The Contract Chain

1. **ScrollArea** receives bounded constraints from its parent (e.g., SplitPane allocates 300px height)
2. **ScrollArea** passes those bounded constraints to its child: `Constraints::new(0, viewport_width, 0, viewport_height)` (scroll_area.rs ~line 1100)
3. **Flex** (child) receives `max_height = 300`. Its children (30 labels) measure at ~600px total natural height
4. **Flex** has `shrink: 1.0` (default for all children). Shrink logic compresses all children proportionally to fit 300px
5. **Flex** reports `content_size` based on **post-shrink** child sizes — because children were laid out at compressed sizes and report compressed `content_size` back
6. **ScrollArea** sees `content_size ≈ viewport_size`, concludes content fits, shows no scrollbar

### Why Flex Reports Wrong content_size

Flex's content_size aggregation (flex.rs ~line 496) reads `cx.tree.get_content_size(child)` for each child. For simple widgets (Label, Button), `content_size = size` — whatever they were laid out at. Since Flex already shrunk them to fit, their reported content_size reflects the compressed size, not the natural size.

Flex **does** compute the natural size during its measurement pass (`total_base` variable, flex.rs ~line 269), but this value is never propagated to `content_size`. It is used only for grow/shrink distribution internally.

### The Accordion Mechanism

1. Frame N: viewport = 300px, Flex shrinks 600px → 300px, ScrollArea sees fit, no scrollbar
2. User resizes panel: viewport = 295px
3. Frame N+1: Flex shrinks 600px → 295px, different shrink distribution, content visually shifts
4. Each frame: viewport changes → Flex redistributes → visual jitter

### The Flicker Mechanism

At boundary conditions where content barely overflows:

1. Frame N: content overflows by 2px → scrollbar appears → scrollbar track/overlay changes visual state → triggers repaint
2. Frame N+1: repaint with same constraints → same overflow → but smooth scrolling animation or rounding causes 1px difference → content fits → scrollbar disappears
3. Frame N+2: no scrollbar → content overflows again → repeat

### Default FlexParams Amplification

`FlexParams::default()` has `shrink: 1.0` (layout.rs line 264). This means **every widget** placed in a Flex column inside a ScrollArea will be compressed rather than overflowing, unless the developer explicitly sets `shrink: 0.0`. This is a dangerous default in scroll contexts.

---

## 4. Why the Bug Reappears

This pathology will reappear because:

1. **Bounded constraints are intentional** — ScrollArea passes bounded height for virtualization support (INV-LAYOUT-AUTHORITY-1). List needs viewport height to compute visible slot count.
2. **Default shrink is non-zero** — Every Flex child has `shrink: 1.0` by default. Developers won't think to set `shrink: 0` for scroll contexts.
3. **Works without ScrollArea** — The same Flex layout works perfectly when not inside a ScrollArea, because the parent gives it enough space or unbounded constraints.
4. **Works at large viewport** — If the viewport is tall enough, no shrink occurs, so the bug only manifests when content exceeds viewport (exactly when you need scrolling).
5. **content_size masking** — Flex's content_size reports post-shrink values, hiding the overflow from ScrollArea. No diagnostic reveals the mismatch without tracing both measurement and distribution phases.

---

## 5. Critical Clarification

> PM-002, PM-004, and PM-005 are distinct failure classes.

- **PM-002** = coordinate transform applied by multiple hierarchical layers (parent + child both subtract scroll). Layer responsibility problem.
- **PM-004** = inconsistent vertex authoring space within the same pipeline stage. Producer-consumer contract mismatch.
- **PM-005** = constraint contract mismatch between ScrollArea and Flex. ScrollArea expects overflow reporting; Flex compresses instead of overflowing.

PM-002 and PM-004 produce "2× scroll" symptoms. PM-005 produces "accordion/flicker" symptoms. The diagnostic differentiator: if content visually compresses during resize and scrollbar is absent despite overflow, check PM-005.

---

## 6. Hard Prevention Rules (Binding)

### PM-005-R1: ScrollArea Must Not Bound Non-Virtualizing Children on the Scroll Axis

When ScrollArea's child does NOT require bounded viewport constraints (i.e., `needs_viewport_constraint() == false`), ScrollArea MUST pass `f32::INFINITY` as `max_height` (for vertical scroll) or `max_width` (for horizontal scroll). This allows Flex and other layout widgets to overflow naturally instead of shrink-compressing content.

Virtualizing widgets (List, ListView) declare `needs_viewport_constraint() == true` and receive bounded constraints per INV-LAYOUT-AUTHORITY-1.

---

### PM-005-R2: content_size Must Never Be Masked by Shrink

A widget's `content_size` MUST reflect its **natural** (unconstrained) extent, not the constrained/shrunk extent. Shrink is a visual compression — it does not reduce the logical content size that scroll containers need. (Secondary defense; primary fix is PM-005-R1.)

---

### PM-005-R3: Child Declares Constraint Needs, Not Parent Guesses

The decision of bounded vs. unbounded constraints MUST be driven by the child widget's declaration (`needs_viewport_constraint()`), not by ScrollArea guessing based on widget type checks, names, or heuristics.

---

## 7. Fix Strategy

### Primary Fix (ScrollArea + Widget Trait)

Reporting natural size in `content_size` alone does NOT fix the visual problem — Flex still receives bounded constraints and visually compresses children. The scrollbar appears but over squished content.

The real fix: **ScrollArea must pass INFINITY on the scroll axis for non-virtualizing children.**

**Problem**: Virtualizing widgets (List, ListView) need bounded height for slot count computation. Passing INFINITY breaks them.

**Solution**: Add `needs_viewport_constraint() -> bool` to the Widget trait. Default `false`. List and ListView override to `true`. ScrollArea queries the child via WidgetTree proxy before constructing constraints:

```rust
let needs_bounded = cx.tree.needs_viewport_constraint(content);
let max_h = if self.enable_scrollbar_v && !needs_bounded {
    f32::INFINITY
} else {
    content_viewport_size.height
};
```

Flex with finite children + INFINITY max_height → no shrink occurs → content overflows naturally → finite content_size returned → ScrollArea sees overflow → scrollbar appears. The downstream `content_size.is_finite()` assertion holds because all child widgets are finite.

Width remains bounded per SPEC-HAL-086 Stage 8 (child must report content_size.width).

### Secondary Fix (Flex content_size — already applied)

`.max(total_base)` in Flex content_size aggregation ensures the measurement-pass natural size is always reported regardless of shrink. This is correct as a secondary defense but does not fix the visual compression alone.

---

## 8. Files Affected

- `src/core/widget/mod.rs` — Add `needs_viewport_constraint()` to Widget trait (default false)
- `src/core/widget_tree.rs` — Add proxy method `needs_viewport_constraint(NodeId) -> bool`
- `src/widgets/list.rs` — Override `needs_viewport_constraint() -> true`
- `src/widgets/list_view/list_view.rs` — Override `needs_viewport_constraint() -> true`
- `src/widgets/scroll_area.rs` — Query trait to choose bounded vs. INFINITY height constraints
- `src/widgets/flex.rs` — `.max(total_base)` in content_size (already applied, secondary defense)

---

## 9. Relationship to Existing SPECs

- **SPEC-HAL-086 Stage 8**: ScrollArea constraint model. The bounded height constraint is intentional for virtualization. PM-005 fix preserves this contract — Flex still receives bounded constraints, but reports natural size via content_size.
- **SPEC-HAL-267**: SplitPane. Not the cause — SplitPane correctly passes constraints to children. PM-005 manifests whenever ScrollArea wraps a Flex, regardless of parent widget.
- **INV-LAYOUT-AUTHORITY-1**: "Layout constraints are authoritative." PM-005 does not violate this — constraints still govern the rendered size. content_size is a separate reporting channel for scroll extent.
