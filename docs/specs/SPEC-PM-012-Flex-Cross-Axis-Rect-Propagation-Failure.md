# SPEC-PM-012: Flex Cross-Axis Rect Propagation Failure

**Status:** Active (root cause unresolved)
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Engine, Widget, Layout
**Applies To:** All widgets inside Flex containers with
non-Stretch alignment

---

## 0. Pathology Name

**Flex Cross-Axis Rect Propagation Failure**

Also known as:
- "Widget stretches despite Alignment::Start"
- "Layout returns X but paint receives Y"
- "node.rect is correct but absolute_paint_rect is wrong"

---

## 1. Summary of the Failure

In a Flex column with `Alignment::Start`, a child widget's
layout correctly returns its intrinsic width (e.g., 54px).
The Flex correctly computes `item_cross = it.cross = 54`
and calls `set_child_rect(child, {w: 54})`. Diagnostic
traps on `set_rect` and `restore_widget` confirm `node.rect`
is NEVER written with the parent width (84.7px). Yet
`compute_absolute_paint_rects` reads 84.7 for the child,
and paint receives 84.7.

This failure manifested in TimePicker inside a Flex column
with a wider Label sibling ("Schedule Time" at ~84.7px).
The TimePicker painted at the Label's width despite
Start alignment.

---

## 2. Root Cause

Unknown. Code analysis traces every `set_rect` and
`restore_widget` call — all write 54.0. Yet
`compute_absolute_paint_rects` reads 84.7. The overwrite
occurs through a code path not covered by the two known
rect-mutation functions, or through a stale cache,
identity mismatch, or multi-pass interaction not yet
identified.

---

## 3. Prevention Rules (Workaround)

**RULE-PM-012-1: Fixed-width widgets MUST enforce their
own width in paint.**

Widgets that know their exact intrinsic width (computed
in layout and cached) must override `rect.width` at the
top of paint:
```rust
fn paint(&self, cx: &mut PaintContext, rect: RectF, ...) {
    let rect = RectF::new(
        rect.x, rect.y,
        self.cached_width.get(),
        rect.height,
    );
    // ... use corrected rect
}
```

This is not a hack — it is self-defense against framework
geometry errors. The widget is the authority for its own
intrinsic dimensions.

**RULE-PM-012-2: When debugging layout-vs-paint width
mismatch, check the SIBLING widths.**

The bug only manifests when a wider sibling sets a larger
`cross_size_final`. If no sibling is wider, the parent
Flex width equals the widget's width and the bug is invisible.
Removing the sibling confirms the cause.

---

## 4. Affected Systems

- TimePicker in Flex column with wider Label sibling
- Potentially ANY fixed-width widget in a Start-aligned
  Flex column with a wider sibling

---

## 5. Forbidden Patterns

FORBID-PM-012-1: Trusting `rect.width` in paint for
fixed-width widgets inside Flex containers. Always use
the widget's own cached intrinsic width.

---

## 6. Future Resolution

The root cause must be found and fixed in the Flex or
layout framework. When resolved, RULE-PM-012-1 becomes
defensive (kept for safety) rather than required.
Candidate areas: Flex second-pass rect assignment,
layout_child skip optimization interaction with
set_child_rect, compute_absolute_paint_rects reading
stale rects.
