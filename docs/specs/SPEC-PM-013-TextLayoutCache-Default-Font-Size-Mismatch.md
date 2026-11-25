# SPEC-PM-013: TextLayoutCache Default Font Size Mismatch

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Widget
**Applies To:** TextField, TextArea, SpinBox, Slider,
any widget using TextLayoutCache

---

## 0. Pathology Name

**TextLayoutCache Default Font Size Mismatch**

Also known as:
- "Text centering is off"
- "Text width doesn't match measurement"
- "Cache diverges from live shaping"

---

## 1. Summary of the Failure

`TextLayoutCache::default()` initializes with `font_size = 16.0`.
The theme's standard font_size is `13.0`. When any code path
calls `rebuild_layout_cache()` without first calling
`cache.set_font_size(theme_font_size)`, the cache shapes text
at 16pt while the widget paints at 13pt. The width divergence
is 16/13 = 1.23x — enough to break text alignment, centering,
and scroll offset computations.

This failure caused TextField text-centering to shift right
inside nested containers (GroupBox). The stale cache width
was 23% wider than the real text, pushing centered text off
its correct position.

---

## 2. Root Cause

`TextLayoutCache::new()` at text_layout_cache.rs:362 defaults
to `font_size = 16.0`. This is a construction default, not
the theme font_size. Public APIs that rebuild the cache
(`rebuild_layout_cache`) must explicitly set the font_size,
but this step was missing.

---

## 3. Prevention Rules

**RULE-PM-013-1: Every `rebuild_layout_cache` call MUST
be preceded by `cache.set_font_size(font_size)`.**

No exceptions. Even if the font_size "hasn't changed",
set it explicitly. The cost is zero; the risk of omission
is a 23% width error.

**RULE-PM-013-2: `rebuild_layout_cache` MUST use
`layout_width_for_policy()` for the width parameter.**

For single-line TextFields, the layout width must be
`SINGLE_LINE_LAYOUT_WIDTH` (100000), not the actual
viewport width. Using viewport width causes incorrect
line-break decisions in the cache.

---

## 4. Affected Systems

- TextField (commit 908a99f2 activated the bug)
- TextArea
- SpinBox (embedded TextField)
- Slider (embedded TextField)
- Any future widget using TextLayoutCache

---

## 5. Forbidden Patterns

FORBID-PM-013-1: Calling `rebuild_layout_cache()` without
first calling `set_font_size()`.

FORBID-PM-013-2: Passing raw `layout_width` to
`rebuild_layout_cache()` for single-line text fields.
Must use `layout_width_for_policy()`.
