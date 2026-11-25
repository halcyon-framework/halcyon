# SPEC-PM-014: Composite Widget Embedded Field Management

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Widget
**Applies To:** SpinBox, Slider, TimePicker, any widget
with embedded TextFields not in the widget tree

---

## 0. Pathology Name

**Composite Widget Embedded Field Management**

Also known as:
- "Caret stays after clicking outside"
- "Embedded text field doesn't blur"
- "Typing in spinbox doesn't work after first edit"
- "Slider value field shows wrong size"

---

## 1. Summary of the Failure

Composite widgets (SpinBox, Slider) contain embedded
TextFields that are NOT in the widget tree. These fields
must be manually managed for: cache rebuilding, keyboard
capture, focus/blur lifecycle, and pointer forwarding.

Failures observed:
1. SpinBox: clicking outside doesn't cancel edit (keyboard
   capture not requested — framework can't track ownership)
2. Slider: clicking the track during value editing activates
   the track handler (pointer events forwarded blindly,
   not gated on value field rect)
3. SpinBox: text renders at wrong size (rebuild_layout_cache
   called without correct font_size — see PM-013)

---

## 2. Root Cause

Embedded TextFields bypass the framework's automatic
lifecycle management (focus, capture, layout). The
composite widget must replicate these behaviors manually.
Missing any step creates subtle interaction bugs.

---

## 3. Prevention Rules

**RULE-PM-014-1: `enter_edit_mode()` MUST call
`cx.request_keyboard_capture()`.**

Without this, the framework doesn't know the widget
holds keyboard focus. Clicking elsewhere won't trigger
capture release → blur → cancel.

**RULE-PM-014-2: `enter_edit_mode()` MUST call
`rebuild_layout_cache()` with correct font_size and
layout_width_for_policy().**

See PM-013. The embedded TextField's cache must match
the widget's actual rendering parameters.

**RULE-PM-014-3: Pointer forwarding to embedded fields
MUST be gated on cached geometry rects.**

Never forward all pointer events blindly. Check if the
click is within the embedded field's cached rect before
forwarding. Clicks outside must fall through to the
host widget's own handlers.

**RULE-PM-014-4: `cancel_edit()` MUST clean up embedded
field state (in_capture, focused, blink).**

The embedded field's visual state (caret blinking,
selection highlighting) must be reset when editing ends.

---

## 4. Affected Systems

- SpinBox (embedded editing_field)
- Slider (embedded value_field)
- Any future composite widget with non-tree-managed fields

---

## 5. Forbidden Patterns

FORBID-PM-014-1: Entering edit mode without
`cx.request_keyboard_capture()`.

FORBID-PM-014-2: Forwarding all pointer events to
embedded fields without geometric gating.

FORBID-PM-014-3: Rebuilding embedded field cache without
setting font_size and layout_width_for_policy.
