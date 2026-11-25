# SPEC-HAL-308: **Sparkline Widget**

### *Miniature Inline Chart — No Axes, No Interaction*

**Author:** JC
**Status:** Draft
**Depends on:** SPEC-HAL-002 (Constitution)

---

# 1. Purpose

A fixed-size miniature line or area chart for embedding inline
with other widgets — inside table cells, cards, labels, status
bars. Consumer provides values, widget auto-scales and renders.

Plot is the wrong tool for this. Plot carries GpuResources,
PlotRenderPool, offscreen texture allocation — infrastructure
designed for full-size interactive charts. A Sparkline pushes
a filled Path into the parent's scene graph. One tessellation,
one draw call, no render target.

---

# 2. Data Model

```rust
pub struct SparklineConfig {
    /// Widget width in logical pixels. Default: 60.0.
    pub width: f32,
    /// Widget height in logical pixels. Default: 16.0.
    pub height: f32,
    /// Line/fill color. Default: theme accent.
    pub color: Color,
    /// True = filled area below line. False = line only.
    /// Default: false (line only).
    pub fill: bool,
    /// Line thickness in logical pixels. Default: 1.5.
    /// Used for line-only mode (fill mode ignores this).
    pub line_width: f32,
}
```

---

# 3. Consumer API

```rust
// Construction
let sparkline = Sparkline::new()
    .with_config(SparklineConfig {
        width: 60.0,
        height: 16.0,
        ..Default::default()
    })
    .with_data(vec![1.0, 3.0, 2.0, 5.0, 4.0, 6.0]);

// Update
sparkline.set_data(new_values);

// Color from theme (default)
let sparkline = Sparkline::new()
    .with_data(values);
// Uses cx.theme.colors.accents.primary at paint time
```

---

# 4. Rendering

## 4.1 Y Auto-Scale

Compute `min` and `max` from the data. Map each value to a
pixel Y within the widget height:

```
pixel_y = height - ((value - min) / (max - min)) * height
```

If all values are equal, render a flat line at vertical center.
If data is empty, render nothing.

## 4.2 X Distribution

Values are evenly spaced across the widget width:

```
pixel_x = (i as f32 / (count - 1) as f32) * width
```

If only one value, render a dot at center.

## 4.3 Fill Mode

Build a Path:
- `move_to(0, height)` (bottom-left)
- `line_to(x_0, y_0)` ... `line_to(x_n, y_n)` (data points)
- `line_to(width, height)` (bottom-right)
- `close()`

Push via `cx.scene.push_path(path, color, z_index, None)`.
This is a filled polygon — the scene graph tessellates it.

## 4.4 Line Mode

Build a thin filled strip by offsetting the polyline:

```
For each segment (x0,y0)→(x1,y1):
  Compute perpendicular offset = line_width / 2
  Push 6 vertices forming a quad along the segment
```

Construct as a Path with the offset contour and push as filled.
Alternatively, if this produces visual artifacts at joins,
use Path's `build_stroke_mesh` and push the resulting mesh.

Note: `PaintSink::push_path` accepts filled paths only. For
line rendering, the widget constructs the stroke geometry
itself and submits it as a filled path. This is consistent
with how icons and other vector primitives work in Halcyon.

---

# 5. Policy

```rust
/// Sparkline has no interaction — display only.
/// No policy enum needed. This section exists to satisfy
/// the policy requirement (SPEC-HAL-002 §3.11) and explicitly
/// document that no interaction surface exists.
///
/// INV-308-POLICY: Sparkline never captures pointer, never
/// emits events, never accepts keyboard input.
```

---

# 6. Invariants

**INV-308-0:** Display only. No interaction, no events.
**INV-308-1:** Auto-scales Y to `[min, max]` of provided values.
**INV-308-2:** Fixed pixel size from config. Not coordinate-space.
**INV-308-3:** No GPU pipeline overhead. Renders into parent's
scene graph via `push_path`. No offscreen texture, no render
pool.
**INV-308-4:** Empty data = nothing rendered. Single value =
flat line at vertical center.
**INV-308-5:** Color defaults to `cx.theme.colors.accents.primary`
when not overridden in config.

---

# 7. Forbidden Behaviors

**FORBID-308-0:** Sparkline must NOT create GPU resources,
offscreen textures, or render pipelines.
**FORBID-308-1:** Sparkline must NOT accept input events or
participate in focus/capture.
**FORBID-308-2:** Sparkline must NOT allocate per-frame. Path
construction reuses the data Vec.

---

# 8. Acceptance Criteria

1. Renders a polyline from `Vec<f64>` data
2. Auto-scales Y to data range
3. Fill mode: solid area below the line
4. Line mode: thin stroked line (as filled offset path)
5. Fixed size from config (width × height)
6. Empty data renders nothing
7. Single value renders flat line
8. Color from config or theme default
9. No GPU pipeline created
10. Widget fits inline in a Flex row alongside Labels

---

# 9. Implementation Phases

**Phase 1:** Fill mode only. Sparkline struct, Widget trait,
layout returns fixed size, paint builds Path from data and
pushes as filled. Demo embedding a sparkline in a Flex row.

**Phase 2:** Line mode. Offset polyline construction or
`build_stroke_mesh`. Test both modes side by side in demo.

---

# 10. Consistency Matrix (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. Widget in tree. |
| B — Phase Separation | Preserved. Paint is pure. |
| C — ID-based Identity | Preserved. WidgetId. |
| D — Pure Data | Preserved. Vec<f64> is pure data. |
| E — GPU-First | N/A — scene graph path, not custom GPU. |
| F — Sovereignty | Preserved. No OS interaction. |

No permanent invariants weakened.
