# SPEC-HAL-296B: **Plot Connector Primitives**

### *Lines, Bezier Curves, Styled Paths, and Force-Directed Routing*

**Author:** JC
**Status:** Draft
**Amends:** SPEC-HAL-296 §4.1 (Built-In Series), SPEC-HAL-296A (Circle Markers)

---

# 1. Purpose

This amendment adds connector primitives to the Plot system:
straight lines, Bezier curves, styled polylines, and a
force-directed routing utility. These are plot-level markers
(same pattern as PlotCircle from SPEC-HAL-296A), not data
series. They are stored on `Plot`, managed via `DataUpdate`,
and rendered in the offscreen GPU overlay pass.

---

# 2. Design Principles

**DP-296B-1: Connectors are markers, not series.**
They are ID-based, individually add/remove-able, rendered in
the overlay pass alongside PriceLevel/PlotTag/PlotCircle.
Not tied to a series index.

**DP-296B-2: World-anchored, screen-styled.**
Endpoints are world coordinates (f64). Visual properties
(width, dash, arrow size) are screen pixels (f32). Connectors
move with pan/zoom but maintain consistent visual weight.

**DP-296B-3: Composable.**
Complex visualizations are composed from simple primitives.
A trade bracket = 2 PlotLines + 3 PlotCircles. A signal chain =
n PlotBeziers + n PlotCircles. The framework doesn't know
what these mean — it draws primitives.

**DP-296B-4: Consumer owns routing decisions.**
Force-directed routing is a utility, not a mandatory path.
The consumer can place connectors with explicit coordinates
(Direct) or request obstacle-avoiding routing. The framework
provides the solver; the consumer decides when to use it.

---

# 3. Primitive Types

## 3.1 PlotLine

Straight line between two world-coordinate points.

```rust
#[derive(Debug, Clone)]
pub struct PlotLine {
    pub id: u64,
    /// Start point (world coordinates).
    pub x1: f64,
    pub y1: f64,
    /// End point (world coordinates).
    pub x2: f64,
    pub y2: f64,
    /// Visual properties.
    pub color: Color,
    pub width: f32,
    pub line_style: PlotLineStyle,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub arrow_size: f32,
    /// Optional group for batch removal.
    pub group: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlotLineStyle {
    Solid,
    Dashed { dash: f32, gap: f32 },
    Dotted { dot: f32, gap: f32 },
}

impl Default for PlotLine {
    fn default() -> Self {
        Self {
            id: 0,
            x1: 0.0, y1: 0.0,
            x2: 0.0, y2: 0.0,
            color: Color::from_srgb(1.0, 1.0, 1.0, 0.8),
            width: 1.0,
            line_style: PlotLineStyle::Solid,
            arrow_start: false,
            arrow_end: false,
            arrow_size: 8.0,
            group: None,
        }
    }
}
```

### Rendering

Each solid segment is a rotated quad (6 vertices):

```
direction = normalize(p2 - p1)
perp = (-direction.y, direction.x)
corners = p1 ± perp * half_width, p2 ± perp * half_width
```

Dashed/dotted: walk along the line at dash intervals,
emit a quad per dash segment. Same logic PriceLevel uses
for horizontal dashes, generalized to arbitrary angles.

Arrowheads: equilateral triangle at endpoint, aligned
with the line direction. 3 vertices per arrowhead.

```
arrow_tip = endpoint
arrow_base_center = endpoint - direction * arrow_size
arrow_left = arrow_base_center + perp * arrow_size * 0.5
arrow_right = arrow_base_center - perp * arrow_size * 0.5
```

All rendered via the overlay pipeline (flat-color triangles).
Same pipeline as PriceLevel lines.

## 3.2 PlotBezier

Cubic Bezier curve between two world-coordinate endpoints
with two control points.

```rust
#[derive(Debug, Clone)]
pub struct PlotBezier {
    pub id: u64,
    /// Start point (world coordinates).
    pub x1: f64,
    pub y1: f64,
    /// Control point 1 (world coordinates).
    pub cx1: f64,
    pub cy1: f64,
    /// Control point 2 (world coordinates).
    pub cx2: f64,
    pub cy2: f64,
    /// End point (world coordinates).
    pub x2: f64,
    pub y2: f64,
    /// Visual properties.
    pub color: Color,
    pub width: f32,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub arrow_size: f32,
    /// Optional group for batch removal.
    pub group: Option<String>,
}
```

### Rendering

1. Evaluate the cubic Bezier at n intervals:

```
B(t) = (1-t)³P0 + 3(1-t)²tC1 + 3(1-t)t²C2 + t³P1
```

2. Adaptive subdivision: n = max(16, arc_length / 4.0).
   Longer curves get more segments for smooth appearance.
   Arc length estimated from control polygon:
   `|P0-C1| + |C1-C2| + |C2-P1|`.

3. Emit a rotated quad per segment (same as PlotLine segments).

4. Arrowheads: aligned with the tangent at the endpoint.
   Tangent at t=0: `B'(0) = 3(C1 - P0)`.
   Tangent at t=1: `B'(1) = 3(P1 - C2)`.

### Convenience Constructors

```rust
impl PlotBezier {
    /// Create a Bezier with auto-computed control points.
    /// `bend` controls curvature: 0 = straight, 0.3 = gentle arc,
    /// 1.0 = dramatic curve. Positive = curve above midpoint,
    /// negative = curve below.
    pub fn arc(
        id: u64,
        x1: f64, y1: f64,
        x2: f64, y2: f64,
        bend: f64,
    ) -> Self {
        let mx = (x1 + x2) / 2.0;
        let my = (y1 + y2) / 2.0;
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();
        // Perpendicular offset
        let nx = -dy / len;
        let ny = dx / len;
        let offset = len * bend;
        Self {
            id,
            x1, y1,
            cx1: mx + nx * offset * 0.5, cy1: my + ny * offset * 0.5,
            cx2: mx + nx * offset * 0.5, cy2: my + ny * offset * 0.5,
            x2, y2,
            ..Self::default()
        }
    }
}
```

## 3.3 PlotPath

Connected polyline through n world-coordinate waypoints
with uniform style. For simple annotations — trend channels,
bracket shapes, manual drawings.

```rust
#[derive(Debug, Clone)]
pub struct PlotPath {
    pub id: u64,
    pub points: Vec<(f64, f64)>,
    pub color: Color,
    pub width: f32,
    pub line_style: PlotLineStyle,
    pub closed: bool,
    pub arrow_end: bool,
    pub arrow_size: f32,
    pub group: Option<String>,
}
```

### Rendering

Iterate pairs of consecutive points. Each pair is a segment
rendered as a rotated quad (same as PlotLine). Joins between
segments use miter or bevel depending on angle:

- Angle < 150°: bevel join (extra triangle to fill the gap)
- Angle >= 150°: miter (extend quads to intersection)

`closed: true` connects last point back to first.

## 3.4 PlotStyledPath (Temporal Links)

A polyline where each segment has independent visual properties.
This is the temporal link primitive: an entity's lifecycle
traced across time and price, with color/width encoding state.

```rust
#[derive(Debug, Clone)]
pub struct PlotStyledPath {
    pub id: u64,
    pub segments: Vec<StyledSegment>,
    pub group: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StyledSegment {
    pub x: f64,
    pub y: f64,
    /// Style for the segment FROM this point TO the next.
    /// Last segment's style is unused (no next point).
    pub color: Color,
    pub width: f32,
    pub line_style: PlotLineStyle,
}
```

### Use Case: Order Lifecycle

```rust
PlotStyledPath {
    id: order_id,
    segments: vec![
        // Placed (gray, thin, dashed)
        StyledSegment {
            x: placed_time, y: placed_price,
            color: Color::GRAY, width: 1.0,
            line_style: PlotLineStyle::Dashed { dash: 4.0, gap: 2.0 },
        },
        // Partially filled (blue, medium)
        StyledSegment {
            x: partial_fill_time, y: partial_fill_price,
            color: Color::BLUE, width: 1.5,
            line_style: PlotLineStyle::Solid,
        },
        // Fully filled (green, thick)
        StyledSegment {
            x: fill_time, y: fill_price,
            color: Color::GREEN, width: 2.0,
            line_style: PlotLineStyle::Solid,
        },
        // Closed (dim, thin)
        StyledSegment {
            x: close_time, y: close_price,
            color: Color::from_srgb(0.5, 0.5, 0.5, 0.5), width: 1.0,
            line_style: PlotLineStyle::Dotted { dot: 2.0, gap: 2.0 },
        },
    ],
    group: Some("orders".into()),
}
```

### Rendering

Each pair of consecutive segments is one line segment with
the style from the first point. Same rotated-quad renderer.
The per-segment style variation creates a visually differentiated
path without multiple PlotLines.

---

# 4. Force-Directed Routing

## 4.1 Problem

Dense annotations — 20+ connectors between markers on a busy
chart — create visual clutter. Straight lines cross candles,
overlap each other, become unreadable.

## 4.2 Approach

Force-directed routing is a utility that the consumer calls
explicitly. It takes a set of connectors and the current
chart state, and produces routed PlotPaths that avoid
collisions.

```rust
pub struct PlotRouter {
    /// Repulsion strength from chart elements.
    pub element_repulsion: f32,
    /// Repulsion strength between connectors.
    pub connector_repulsion: f32,
    /// Number of solver iterations.
    pub iterations: u32,
    /// Minimum clearance from obstacles (pixels).
    pub clearance: f32,
}

impl PlotRouter {
    /// Route a set of connections, avoiding chart obstacles.
    ///
    /// `obstacles`: bounding rects of chart elements in screen pixels
    /// (candle bodies, markers, existing labels).
    ///
    /// Returns: one PlotPath per connection, with waypoints
    /// computed to minimize crossings and overlap.
    pub fn route(
        &self,
        connections: &[(ScreenPoint, ScreenPoint)],
        obstacles: &[RectF],
    ) -> Vec<Vec<(f32, f32)>> {
        // ...
    }
}
```

## 4.3 Algorithm

1. **Initialize:** For each connection, start with a straight
   line. Insert midpoints at regular intervals (every 40px).

2. **Repulsion pass:** For each midpoint, compute forces:
    - Repulsion from obstacles: F = clearance / distance²,
      direction away from nearest obstacle edge.
    - Repulsion from other connector midpoints:
      F = connector_repulsion / distance², direction away.
    - Attraction toward original straight-line position:
      F = 0.1 × displacement (spring force).

3. **Apply forces:** Move each midpoint by the net force
   vector, clamped to max displacement per iteration.

4. **Repeat** for `iterations` steps (default: 50).

5. **Smooth:** Apply Catmull-Rom interpolation through the
   final waypoints for smooth curves (optional).

6. **Return** waypoints per connection. Consumer converts to
   PlotPath or PlotBezier for rendering.

## 4.4 Integration Pattern

The router runs on the consumer's thread, not inside
Halcyon's render loop. The consumer:

1. Collects obstacle rects from the chart
   (via `plot.visible_element_rects()`)
2. Defines connections (world coords → screen coords via Transform)
3. Calls `PlotRouter::route()`
4. Converts waypoints back to world coords
5. Creates PlotPath markers via `DataUpdate::AddPath`
6. Re-routes when the view changes (pan/zoom)

The framework provides the router as a utility. The consumer
decides when and how to use it.

## 4.5 Obstacle Query API

Plot exposes the obstacle map for the router:

```rust
impl Plot {
    /// Screen-space bounding rects of visible chart elements.
    /// Includes: candle bodies, volume bars, circle markers,
    /// price level tags, freestanding tags, region borders.
    /// Excludes: grid lines, crosshair, axis labels.
    pub fn visible_element_rects(&self) -> Vec<RectF> {
        // Query from hit_entries + price_levels + tags + circles
    }
}
```

---

# 5. DataUpdate Variants

```rust
    AddLine(PlotLine),
    RemoveLine(u64),
    AddBezier(PlotBezier),
    RemoveBezier(u64),
    AddPath(PlotPath),
    RemovePath(u64),
    AddStyledPath(PlotStyledPath),
    RemoveStyledPath(u64),
```

---

# 6. Plot API Methods

For each type (Line, Bezier, Path, StyledPath):

```rust
    pub fn with_line(mut self, line: PlotLine) -> Self;
    pub fn add_line(&mut self, line: PlotLine);
    pub fn remove_line(&mut self, id: u64);
    pub fn remove_line_group(&mut self, group: &str);
    pub fn clear_lines(&mut self);

    // Same pattern for bezier, path, styled_path
```

---

# 7. Rendering Integration

All connector primitives render via the overlay pipeline
(flat-color triangles, same as PriceLevel lines). They use
the existing `OverlayVertex` format and `overlay_pipeline`.

### Vertex Building

`build_connector_vertices()` runs alongside
`build_level_vertices()` and `build_circle_vertices()` in
`do_render()`. Returns `Vec<OverlayVertex>`.

The connector vertices go into RenderParams and are rendered
in the PlotRenderer after price levels and before circles.

Render order in the offscreen pass:
1. Grid
2. Series (candles, lines, bars, circles)
3. Selection highlight
4. Region backgrounds
5. Price level lines + tag backgrounds
6. **Connector lines + arrows** (new)
7. **Circle markers**
8. Tool overlay
9. Crosshair
10. Box selection

Connectors render before circles so circle markers appear
on top of connector endpoints.

---

# 8. Hit Testing

## 8.1 Line/Path Hit Testing

Distance from click point to nearest segment. For each
segment (p1, p2):

```
t = clamp(dot(click - p1, p2 - p1) / |p2 - p1|², 0, 1)
closest = p1 + t * (p2 - p1)
dist = |click - closest|
```

Hit if `dist <= width/2 + threshold`.

## 8.2 Bezier Hit Testing

Evaluate curve at n sample points. Find nearest sample
to click. If distance < threshold, hit. Coarse but
sufficient — Bezier curves are visual connectors, not
precision targets.

## 8.3 Events

```rust
pub enum PlotConnectorEvent {
    Clicked { id: u64, connector_type: ConnectorType },
    HoverEnter { id: u64, connector_type: ConnectorType },
    HoverLeave { id: u64, connector_type: ConnectorType },
}

pub enum ConnectorType {
    Line,
    Bezier,
    Path,
    StyledPath,
}
```

Hover tracking: same pattern as PlotCircle (§Part 4 of
SPEC-HAL-296A circle marker task).

---

# 9. Invariants

**INV-296B-1:** Connector endpoints are world coordinates (f64).
Visual properties (width, dash, arrow) are screen pixels (f32).
Connectors move with pan/zoom but maintain consistent visual weight.

**INV-296B-2:** All connector types are ID-based plot-level markers.
They do not participate in y-autoscale. They are visual annotations,
not data.

**INV-296B-3:** Force-directed routing is consumer-initiated.
The framework provides the solver as a utility. Routing never
runs automatically inside the render loop.

**INV-296B-4:** PlotStyledPath segments carry independent styles.
No interpolation between segment styles. Each segment renders
with its own color/width/dash.

**INV-296B-5:** Bezier adaptive subdivision guarantees smooth
curves. Minimum 16 segments, scaled by arc length.

---

# 10. Forbidden Behaviors

**FORBID-296B-1:** Connectors MUST NOT participate in y-autoscale.
They are overlays, not data. Adding a connector must not shift
the chart's vertical range.

**FORBID-296B-2:** PlotRouter MUST NOT run inside the render loop.
It is a consumer-side utility. If the consumer calls it per-frame,
that is a consumer performance bug.

**FORBID-296B-3:** Connectors MUST NOT reference series data.
A connector between "candle 5" and "candle 10" is expressed as
world coordinates, not series indices. The consumer resolves
data positions to coordinates. Halcyon draws.

---

# 11. Implementation Phases

**Phase 1:** PlotLine — straight lines with solid/dashed/dotted,
arrowheads. Rotated quad renderer. Hit testing. Hover/click events.
DataUpdate + builder + API.

**Phase 2:** PlotBezier — cubic Bezier with adaptive subdivision.
`arc()` convenience constructor. Arrowheads aligned to tangent.

**Phase 3:** PlotPath — uniform-style polyline. Join rendering.
Closed paths.

**Phase 4:** PlotStyledPath — per-segment styled polyline.
Temporal link visualization.

**Phase 5:** PlotRouter — force-directed routing utility.
Obstacle query API. Consumer integration pattern.

---

# 12. Acceptance Criteria

1. PlotLine renders solid/dashed/dotted between two world points
2. PlotLine arrowheads render at correct angle
3. PlotLine hit-test detects clicks within width + threshold
4. PlotBezier renders smooth curve with antialiased quad strip
5. PlotBezier `arc()` produces symmetric curve with bend parameter
6. PlotBezier arrowhead aligns with tangent at endpoint
7. PlotPath renders connected polyline with clean joins
8. PlotPath `closed: true` closes the shape
9. PlotStyledPath renders segments with independent color/width
10. PlotStyledPath correctly visualizes an order lifecycle (gray → blue → green → dim)
11. PlotRouter produces obstacle-avoiding paths for 20 connections in < 10ms
12. PlotRouter results are stable (small view changes → small path changes)
13. All connectors move correctly with pan/zoom
14. All connectors support group-based batch removal
15. Hover/click events fire correctly for all connector types
16. Connectors do not affect y-autoscale
