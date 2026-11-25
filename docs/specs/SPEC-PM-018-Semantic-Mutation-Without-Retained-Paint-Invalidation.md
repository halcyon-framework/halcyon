# SPEC-PM-018: Semantic Mutation Without Retained Paint Invalidation

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Engine, Widget, Interaction
**Applies To:** Owner callbacks and action paths that change state consumed by retained paint

---

## 0. Pathology Name

**Semantic Mutation Without Retained Paint Invalidation**

Also known as **Lifecycle Callback Invalidation Blindness**: state changes
immediately, but its visual projection updates only after the next unrelated
handled input.

## 1. Observed Incident

On 2026-10-06, RadioGroup keyboard activation through Space, i, or Enter changed
`selected` immediately and emitted the selection delta, but the selected radio
remained visually stale until a later Up/Down input.

`EventRouter::activate_focused_widget` invokes `on_keyboard_activate` directly.
That path does not apply the paint invalidation performed after a handled event
by `dispatch_to_widget_with_pointer_origin`. RadioGroup's callback committed the
reducer result and called only `request_continuous_frames()`.

The later handled arrow event dirtied the node, so paint finally projected the
selection that had already changed. The interaction policy and reducer timing
were correct; the changed paint dependency had not been invalidated.

The initial enforcement audit confirmed the same failure class in the keyboard
activation callbacks of Checkbox, SegmentedControl, DropdownMenu, Breadcrumbs,
and the static animation fallbacks of Switch and DisclosureSectionHeader. These
six sibling instances were corrected with owner-local paint invalidation,
preserving navigation intent, no-op branches, and successful animation guarantees.

## 2. Structural Cause and False Fix

A callback or action path bypasses normal handled-event dispatch, while its
owner assumes that successful state mutation or frame demand also dirties its
retained fragment. The retained scene correctly reuses the clean fragment,
which still represents the old state.

`request_continuous_frames()` is a false fix for this failure: it schedules work
but does not invalidate a changed paint dependency. More scheduled frames can
reuse the same stale fragment. Update or animation demand has the same boundary;
it cannot replace the invalidation required by the changed projection.

## 3. Why the Failure Reappears

Tests that assert only reducer state or callbacks miss retained paint dirtiness.
Later handled input hides the missing invalidation by eventually repainting the
already-mutated state. Lifecycle callbacks resemble normal input handlers but
do not necessarily inherit their caller's invalidation guarantees.

## 4. Hard Prevention Rules (Binding)

### PM-018-R1: The Mutation Path Owns Its Invalidation Obligation

Any owner callback or action path that changes paint-consumed semantic or
presentation state MUST emit the narrowest corresponding invalidation before
that changed dependency can be reused in the next admitted scene phase.

A caller/framework guarantee that already invalidates the exact changed
dependency satisfies this obligation. This rule does not require redundant
invalidation where that guarantee exists. A direct lifecycle callback MUST NOT
assume the guarantee of a handled-event path it does not traverse.

### PM-018-R2: Scheduling Is Never Invalidation

Frame, update, and animation demand MUST NOT substitute for dirtying a changed
dependency. A static selection change requires paint invalidation. Continuing
animation demand is justified only by an independently active visual effect.

### PM-018-R3: Preserve the Narrowest Actual Change

A paint-only state change MUST NOT invalidate layout or unrelated fragments.
An unchanged visual result MUST remain paint-clean unless another dependency
actually changed. Compare the owner result rather than repainting every action.

For this RadioGroup incident, `on_semantic_event` reports a selection delta only
when selection changes. `on_keyboard_activate` uses that result to call
`EventContext::invalidate_paint()` and does not request continuous frames for
the static selection commit.

## 5. Affected Systems and Forbidden Patterns

This failure class applies to retained widgets whose lifecycle callbacks,
explicit activation actions, or other owner paths mutate paint-consumed state
outside normal handled-event dispatch.

Forbidden patterns:

- changing paint-consumed state and relying only on frame/update demand;
- assuming a direct callback receives handled-event automatic dirtiness;
- using layout invalidation or whole-tree repaint for a local paint-only delta;
- relying on later unrelated input to make an already-committed state visible.

## 6. Regression Proof

The RadioGroup regression
`radio_group_input_keyboard_activation_invalidates_paint_on_selection_change`
in `tests/radio_group_input_tests.rs` begins with a mounted, paint-clean node and
activates it through `EventRouter::activate_focused_widget`. Selection changes
and the owning node becomes paint-dirty immediately, without another input or
layout invalidation. Repeating activation with unchanged selection remains
paint-clean and emits no second selection callback.

## 7. Authority and Incident References

- [SPEC-PM-000](SPEC-PM-000-Pathology-and-Mistake-Prevention-Series.md): the repeated failure class requires permanent prevention law.
- [SPEC-HAL-354 §5](SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md#5-required-dependency-invalidation): selection consumed by paint invalidates its dependency; update demand does not dirty retained fragments.
- [SPEC-HAL-328 §§11.2, 15](SPEC-HAL-328-Keyboard-Authority-Focus-and-Capture.md): RadioGroup explicit activation and owner responsibility for internal-state invalidation.
- [RadioGroup activation callback](../../src/widgets/radio_group_widget.rs) and [regression target](../../tests/radio_group_input_tests.rs): the 2026-10-06 incident and owner-local correction.
