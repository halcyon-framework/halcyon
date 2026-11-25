# SPEC-HAL-274: **SegmentedControl — Structural Contract and Invariants**

### *Mutually Exclusive Inline Option Set*

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)

---

# 1. Purpose

Define the structural contract for a horizontal set of mutually exclusive option buttons rendered as a single connected visual unit. Common for mode switching (e.g., Day / Week / Month, List / Grid / Card).

This is a small-surface widget. No new pipeline, no new shader. Standard rect + text rendering.

---

# 2. Value Type

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SegmentId(pub u32);
```

**INV-274-V1: Stable Identity**
`SegmentId` is user-assigned and stable across rebuilds. The widget does not generate or reassign IDs.

**INV-274-V2: Single Selection**
Exactly one segment is always selected. There is no deselected state and no multi-select variant at this SPEC level.

**INV-274-V3: Minimum Segment Count**
The widget requires a minimum of 2 segments. Construction with fewer than 2 is a logic error (debug panic).

---

# 3. Segment Definition

```rust
pub struct Segment {
    pub id: SegmentId,
    pub label: String,
    pub icon: Option<IconSpec>,
    pub enabled: bool,
}
```

**INV-274-S1: Label Required**
Every segment MUST have a non-empty label. Icon is optional and renders to the left of the label.

**INV-274-S2: Disabled Segments**
A disabled segment is visually muted and does not respond to interaction. A disabled segment cannot be selected. If the currently selected segment becomes disabled, the selection does NOT automatically move — the segment remains selected but visually flagged.

---

# 4. Widget State

```rust
pub struct SegmentedControlState {
    segments: Vec<Segment>,
    selected: SegmentId,
    focused_index: Option<usize>,
    hover_index: Option<usize>,
}
```

---

# 5. Interaction Invariants

**INV-274-I1: Click Activation**
Clicking an enabled segment selects it immediately. No confirmation step.

**INV-274-I2: Keyboard Navigation**
Left/Right arrows move the focused highlight between segments. Home/End jump to first/last enabled segment. The focused segment is NOT selected until the user presses Enter or Space.

**INV-274-I3: No Deselection**
There is no action that deselects all segments. One is always selected.

**INV-274-I4: Focus vs Selection**
Focus (keyboard highlight) and selection (active segment) are independent states. Focus can be on a different segment than the selected one.

---

# 6. Layout

All geometry values from theme:

```rust
pub struct SegmentedControlGeometry {
    pub height: f32,
    pub min_segment_width: f32,
    pub padding_x: f32,
    pub radius: f32,
    pub separator_width: f32,
}
```

**INV-274-L1: Connected Rendering**
Segments render as a single connected bar. The first segment has rounded left corners, the last has rounded right corners, interior segments have no rounding. Vertical separators (1px) render between segments.

**INV-274-L2: Equal Width Option**
By default, each segment sizes to fit its content. An optional `equal_width: bool` flag forces all segments to the width of the widest segment. This intrinsic option is unchanged; the separate explicit fill-width mode below may expand equal segments within a finite offered width.

**INV-274-L3: Total Width**
The widget's logical total width is the sum of segment widths + separator widths. The default remains intrinsically sized, without stretch. Only the explicit fill-width mode below may expand that logical width to a finite offered width.

## Scoped binding layout addendum — JC approval, 2026-10-24

JC approves only this opt-in sizing extension and the corresponding L2/L3
qualification. All other previously unadopted clauses retain their Draft status.
The Constitution and existing input, selection, focus and invalidation owners
are unchanged.

- `with_fill_width(bool)` is a separate construction-time option, defaulting to
  false. `with_equal_width` keeps its existing intrinsic meaning.
- When fill is enabled and the offered maximum width is finite, all segments use
  an equal width that fills that offer after accounting for every separator.
  Each segment still reserves at least the widest measured label plus its theme
  padding and the theme minimum segment width.
- If that minimum equal-width bar exceeds the offered width, Layout preserves
  those segment minima and reports the full logical overflow in `content_size`
  alongside the constrained allocation. Labels are not compressed to pretend the
  bar fits. Existing parent clipping and overflow policy remain authoritative.
- An unbounded maximum width retains the existing intrinsic sizing, including
  the independently selected equal-width option; it creates no infinite extent.
- Layout publishes one width/offset cache for painting, pointer hit-testing,
  hover, separators and keyboard focus. Relayout recomputes that geometry without
  changing selection, focus, enabled state, activation or stable identity.
- Focused proof must cover intrinsic and legacy equal widths, finite equal fill
  including separators, hit boundaries, narrow logical overflow and the actual
  constrained GroupBox/Flex/TooltipHost placement. Existing keyboard activation
  proof remains required. Native visual observation is a separate claim.

---

# 7. Rendering

**INV-274-R1: Selected Segment**
The selected segment renders with `controls.background` fill and `controls.text` label color.

**INV-274-R2: Unselected Segments**
Unselected segments render with transparent fill and `typography.secondary` label color.

**INV-274-R3: Hover State**
Hovering an unselected segment renders a subtle background tint (`surfaces.level2`).

**INV-274-R4: Focus Ring**
When the widget has keyboard focus, the focused segment shows a focus ring per the framework-automatic focus ring system.

**INV-274-R5: Outer Border**
A 1px border (`borders.default`) wraps the entire connected bar.

---

# 8. Acceptance Criteria

- Exactly one segment is always selected.
- Click selects immediately.
- Keyboard navigation moves focus independently of selection.
- Enter/Space on focused segment selects it.
- Connected rendering with rounded end corners and interior separators.
- Disabled segments are visually muted and non-interactive.
- Minimum 2 segments enforced.

---

# 9. Non-Goals

- Multi-select (use a different widget — checkbox group or TagInput).
- Vertical orientation (horizontal only at this SPEC level).
- Overflow/scroll when too many segments (constrain via segment count).
- Icon-only segments (label is always required).
