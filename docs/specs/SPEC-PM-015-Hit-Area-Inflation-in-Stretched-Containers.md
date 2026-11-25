# SPEC-PM-015: Hit Area Inflation in Stretched Containers

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Widget
**Applies To:** RadioGroup, Checkbox, Switch, any control
with a clickable indicator + label inside a stretched
parent

---

## 0. Pathology Name

**Hit Area Inflation in Stretched Containers**

Also known as:
- "Clicking empty space activates a radio button"
- "Checkbox triggers from the far right"
- "Accidental data changes from misclicks"

---

## 1. Summary of the Failure

Controls with an indicator + label (RadioGroup, Checkbox)
use `layout_bounds.width` for x hit-testing. In a stretched
Flex container (e.g., inside a GroupBox), `layout_bounds.width`
is the FULL container width (~1300px). Clicking anywhere
on the same row — even 1200px to the right of the label —
activates the control.

This is dangerous for data-entry forms. Accidental clicks
change values silently.

---

## 2. Root Cause

The control's `handle_event` checks
`local_x <= self.layout_bounds.width` which includes all
the empty space after the label. The actual clickable
content (indicator + gap + label text) is much narrower.

---

## 3. Prevention Rules

**RULE-PM-015-1: Controls with indicator + label MUST
cache content width from layout.**

In `layout()`, compute:
```rust
let content_w = indicator_size + label_gap + text_width;
self.cached_content_width = content_w;
```

**RULE-PM-015-2: Hit testing MUST use cached content
width, not layout_bounds.width.**

```rust
let inside = local_x >= 0.0
    && local_x <= self.cached_content_width
    && local_y >= row_y
    && local_y < row_y + row_h;
```

---

## 4. Affected Systems

- RadioGroup (fixed)
- Checkbox (fixed)
- Switch (verify)
- Any future indicator+label control

---

## 5. Forbidden Patterns

FORBID-PM-015-1: Using `layout_bounds.width` or
`rect.width` for hit-testing in controls where the
interactive area is narrower than the layout allocation.
