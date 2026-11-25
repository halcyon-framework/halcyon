# SPEC-HAL-306: **Pie / Donut Chart Widget**

### *Radial Segment Chart with Labels and Optional Center Value*

**Author:** JC
**Status:** Draft
**Depends on:** SPEC-HAL-002 (Constitution)

---

# 1. Purpose

Standalone radial chart widget. Not a Plot series — Plot is
Cartesian. Renders arc segments proportional to consumer-provided
values. Optional donut hole with center text.

**Projection-only.** Consumer provides slices with values,
labels, and colors. Widget computes angles and renders. No data
manipulation — no merging, no filtering, no aggregation.

---

# 2. Data Model

```rust
pub struct PieSlice {
    /// Segment value. Widget normalizes to proportional angles.
    pub value: f64,
    /// Label text (for external labels or tooltip).
    pub label: String,
    /// Segment fill color.
    pub color: Color,
}

pub struct PieChartConfig {
    /// Inner radius as fraction of outer radius.
    /// 0.0 = solid pie. 0.5+ = donut.
    pub inner_radius_fraction: f32,
    /// Text shown inside the donut hole (large). None = no center text.
    pub center_value: Option<String>,
    /// Smaller text below center_value. None = no sublabel.
    pub center_label: Option<String>,
    /// Gap between segments in degrees. Default: 1.0.
    pub gap_degrees: f32,
    /// Start angle in degrees. 0 = 12 o'clock, clockwise. Default: 0.
    pub start_angle: f32,
    /// Whether to show percentage labels outside segments.
    pub show_labels: bool,
    /// Font size for external labels. Default: 11.0.
    pub label_font_size: f32,
}
```

---

# 3. Rendering

Each segment is a filled Path:

```
move_to(center)
line_to(arc_start_point)
cubic_to (arc approximation — 4 cubic beziers per 90° arc)
line_to(center)
close()
```

For donut mode, the segment is a ring sector: outer arc
forward, inner arc backward, closed. Two concentric arc
contours connected by radial lines, forming a closed ring
segment.

The Path infrastructure supports cubic bezier approximation
of arcs. Standard technique: each 90° arc segment approximated
by one cubic bezier with control point offset `k = 0.5522847498`.

## 3.1 Label Placement

External labels (when `show_labels` is true):

- For each segment, compute the midpoint angle.
- Place label at `radius * 1.3` from center at that angle.
- If labels overlap, skip the smaller segment's label.

Leader lines: thin line from segment outer edge to label
position. Rendered via `push_rect` (1px rotated) or a 2-point
Path.

## 3.2 Center Text

Rendered inside the donut hole. `center_value` in large font
(font_size * 2), `center_label` below it in small font. Both
centered horizontally and vertically within the hole.

---

# 4. Interaction

## 4.1 Hover

Hit-test by computing the pointer's angle and distance from
center:

```
dx = pointer_x - center_x
dy = pointer_y - center_y
distance = sqrt(dx² + dy²)
angle = atan2(dx, -dy)  // 0 = top, clockwise
```

If `inner_radius <= distance <= outer_radius` AND angle falls
within a segment's arc range → that segment is hovered.

Hovered segment: lighten fill color by 20% via `color.lighten(0.2)`.

## 4.2 Events

```rust
pub enum PieChartEvent {
    SliceClicked { index: usize },
    SliceHoverEnter { index: usize },
    SliceHoverLeave,
}
```

Consumer polls via `pie.take_events()`.

---

# 5. Policy

```rust
pub enum PieChartPolicy {
    /// Display only. No hover, no click.
    Static,
    /// Hover highlights + click events.
    Interactive,
}
```

Default: `Interactive`.

---

# 6. Invariants

**INV-306-0:** Projection-only. Consumer provides slices.
Widget does not merge, filter, aggregate, or reorder slices.
**INV-306-1:** Segment angles sum to 360° minus total gap.
Each segment's angle is proportional to its value.
**INV-306-2:** Zero-value slices render as zero-width (invisible).
**INV-306-3:** Hit-testing uses angle + distance formula.
**INV-306-4:** Color defaults: if consumer provides no color,
widget assigns from a built-in palette (8-color cycle).

---

# 7. Forbidden Behaviors

**FORBID-306-0:** Widget must NOT merge small slices into
"Other". Consumer decides what to merge.
**FORBID-306-1:** Widget must NOT sort or reorder slices.
Render order = consumer order.
**FORBID-306-2:** Widget must NOT create offscreen textures
or GPU pipelines. Renders via Path into the scene graph.

---

# 8. Acceptance Criteria

1. Renders proportional arc segments from `Vec<PieSlice>`
2. Donut mode: inner hole with center text
3. Pie mode: solid fill (inner_radius_fraction = 0)
4. Gap between segments configurable
5. Hover highlights correct segment
6. Click emits SliceClicked
7. External labels with leader lines (when enabled)
8. Empty slices = nothing rendered
9. Single slice = full circle
10. Static policy disables all interaction
11. Center text renders in donut hole

---

# 9. Implementation Phases

**Phase 1:** Solid pie, no interaction. PieChart struct, Widget
trait, Path-based arc segments, layout returns square. Demo.

**Phase 2:** Donut mode. Ring sector paths. Center text.

**Phase 3:** Interaction. Hit-test by angle + distance. Hover
highlight. Click events. Policy enum.

**Phase 4:** Labels. External label placement, leader lines,
overlap avoidance.

---

# 10. Consistency Matrix (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. Widget in tree. |
| B — Phase Separation | Preserved. Paint is pure. |
| C — ID-based Identity | Preserved. WidgetId. |
| D — Pure Data | Preserved. Vec<PieSlice> is pure data. |
| E — GPU-First | N/A — scene graph path rendering. |
| F — Sovereignty | Preserved. No OS interaction. |
