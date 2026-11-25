# SPEC-HAL-296A: **ScatterSeries — SDF Circle Rendering**

### *Amendment to SPEC-HAL-296: Plot — Coordinate-Authority Rendering System*

**Author:** JC
**Status:** Draft
**Amends:** SPEC-HAL-296 §4.1 (Built-In Series), §14 (GPU Pipeline)

---

# 1. Purpose

This amendment adds a ScatterSeries to the Plot system — GPU-rendered
circles for scatter plots, data point markers, trade markers, signal
indicators, and any visualization requiring positioned dots of varying
size and color.

---

# 2. ScatterSeries

ScatterSeries renders data points as filled or bordered circles using
an SDF (Signed Distance Field) circle shader. Each point is one quad
(6 vertices). The fragment shader evaluates `distance(frag, center)`
for smooth antialiased edges at any radius.

## 2.1 Data Type

```rust
/// A single point in a ScatterSeries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScatterPoint {
    /// World X coordinate.
    pub x: f64,
    /// World Y coordinate.
    pub y: f64,
    /// Per-point radius in pixels. None → use ScatterStyle.radius.
    pub radius: Option<f32>,
    /// Per-point fill color. None → use ScatterStyle.fill_color.
    pub fill_color: Option<Color>,
}
```

Points carry world coordinates (f64, per INV-296-5). Radius and color
are optional overrides — most points use the series default, individual
points override for emphasis (e.g., a highlighted trade, a signal with
confidence-mapped size).

## 2.2 Style

```rust
/// Visual configuration for a ScatterSeries.
#[derive(Debug, Clone)]
pub struct ScatterStyle {
    /// Default fill color for all points.
    pub fill_color: Color,
    /// Default radius in pixels.
    pub radius: f32,
    /// Border color. Alpha 0 = no border.
    pub border_color: Color,
    /// Border width in pixels. 0 = no border.
    pub border_width: f32,
}

impl Default for ScatterStyle {
    fn default() -> Self {
        Self {
            fill_color: Color::WHITE,
            radius: 4.0,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
        }
    }
}
```

## 2.3 GPU Vertex Format

```rust
/// Per-vertex data for the SDF circle shader.
///
/// Each circle is one quad (6 vertices). The vertex shader positions
/// the quad corners. The fragment shader evaluates the SDF circle.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleVertex {
    /// Quad corner position in screen pixels.
    pub position: [f32; 2],
    /// Circle center in screen pixels.
    pub center: [f32; 2],
    /// Circle radius in pixels (outer edge of fill).
    pub radius: f32,
    /// Fill color RGBA.
    pub fill_color: [f32; 4],
    /// Border color RGBA (alpha 0 = no border).
    pub border_color: [f32; 4],
    /// Border width in pixels (0 = no border).
    pub border_width: f32,
}
```

16 floats × 4 bytes = 64 bytes per vertex, 384 bytes per circle.
At 10,000 points: 3.84 MB vertex data — trivial for GPU.

## 2.4 SDF Circle Shader (WGSL)

```wgsl
struct Uniforms {
    viewport_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) fill_color: vec4<f32>,
    @location(4) border_color: vec4<f32>,
    @location(5) border_width: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) center: vec2<f32>,
    @location(1) radius: f32,
    @location(2) fill_color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_width: f32,
    @location(5) frag_pos: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let ndc_x = (in.position.x / uniforms.viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.position.y / uniforms.viewport_size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.center = in.center;
    out.radius = in.radius;
    out.fill_color = in.fill_color;
    out.border_color = in.border_color;
    out.border_width = in.border_width;
    out.frag_pos = in.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = distance(in.frag_pos, in.center);
    let outer_r = in.radius;
    let inner_r = in.radius - in.border_width;

    // Antialiased edge (1px smoothstep at outer boundary)
    let outer_alpha = 1.0 - smoothstep(outer_r - 0.5, outer_r + 0.5, dist);

    if outer_alpha <= 0.0 {
        discard;
    }

    // Border vs fill
    var color: vec4<f32>;
    if in.border_width > 0.0 && dist > inner_r - 0.5 {
        let border_blend = smoothstep(inner_r - 0.5, inner_r + 0.5, dist);
        color = mix(in.fill_color, in.border_color, border_blend);
    } else {
        color = in.fill_color;
    }

    color.a = color.a * outer_alpha;
    return color;
}
```

The shader evaluates the SDF per-fragment. The 0.5px smoothstep at
the outer edge provides hardware-quality antialiasing. The border is
a second smoothstep transition between fill and border color at
`radius - border_width`.

## 2.5 Geometry Building

For each visible point:

1. Transform world coordinates to screen pixels via `Transform`
2. Compute effective radius and colors (per-point override or style default)
3. Emit one quad: 6 vertices, center ± (radius + 1px AA margin)

The +1px margin ensures the smoothstep antialiasing has room to
blend to transparent at the outer edge. Without it, circles are
hard-clipped at the quad boundary.

```rust
fn build_vertices(&self, transform: &Transform) -> Vec<CircleVertex> {
    let mut verts = Vec::with_capacity(self.points.len() * 6);

    for p in &self.points {
        let px = transform.x_to_px(p.x);
        let py = transform.y_to_px(p.y);
        let r = p.radius.unwrap_or(self.style.radius);
        let fill = p.fill_color.unwrap_or(self.style.fill_color).to_array();
        let border = self.style.border_color.to_array();
        let bw = self.style.border_width;

        // Quad extends radius + 1px AA margin from center
        let margin = r + 1.0;
        let x0 = px - margin;
        let y0 = py - margin;
        let x1 = px + margin;
        let y1 = py + margin;

        let base = |pos: [f32; 2]| CircleVertex {
            position: pos,
            center: [px, py],
            radius: r,
            fill_color: fill,
            border_color: border,
            border_width: bw,
        };

        // Two triangles: TL-TR-BL, BL-TR-BR
        verts.push(base([x0, y0]));
        verts.push(base([x1, y0]));
        verts.push(base([x0, y1]));
        verts.push(base([x0, y1]));
        verts.push(base([x1, y0]));
        verts.push(base([x1, y1]));
    }

    verts
}
```

## 2.6 Viewport Culling

Points outside the viewport's world-coordinate range are skipped during
geometry building. A margin of `max_radius / scale` in world coordinates
ensures edge-straddling points are included. Same pattern as LineSeries
visible-range filtering.

## 2.7 Hit Testing

Distance from click position to point center vs radius. Trivial:

```rust
fn hit_test(&self, x: f32, y: f32, transform: &Transform) -> Option<Box<dyn Any>> {
    let threshold = 2.0; // extra px tolerance beyond radius
    let mut best_dist = f32::INFINITY;
    let mut best_idx = 0usize;

    for (i, p) in self.points.iter().enumerate() {
        let px = transform.x_to_px(p.x);
        let py = transform.y_to_px(p.y);
        let r = p.radius.unwrap_or(self.style.radius);
        let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
        let effective = dist - r; // negative = inside circle
        if effective < best_dist {
            best_dist = effective;
            best_idx = i;
        }
    }

    if best_dist <= threshold {
        Some(Box::new(best_idx))
    } else {
        None
    }
}
```

## 2.8 Integration

### SeriesData variant

```rust
pub enum SeriesData {
    Candles(Vec<RenderCandle>),
    Lines(Vec<LinePoint>),
    Bars(Vec<BarPoint>),
    BookUpdate(BookUpdateKind),
    Scatter(Vec<ScatterPoint>),  // new
}
```

### Builder method

```rust
impl Plot {
    pub fn with_scatter(self, style: ScatterStyle) -> Self { ... }
}
```

### HitProxy variant

```rust
pub enum HitProxy {
    Line { points: Vec<LinePoint> },
    Scatter { points: Vec<ScatterPoint> },  // new
    // ...
}
```

---

# 3. Invariants

**INV-296-SC1:** ScatterSeries uses SDF circle rendering. No triangle-fan
polygon approximation. One quad per point, fragment shader evaluation.

**INV-296-SC2:** Per-point radius and color overrides are optional.
Style defaults apply when `None`. No per-point allocation.

**INV-296-SC3:** World coordinates are f64 until after Transform
(inherits INV-296-5). Screen-space radius is f32 (pixel units).

**INV-296-SC4:** Viewport culling skips off-screen points during
geometry building (inherits INV-296-GPU1).

**INV-296-SC5:** Hit testing accounts for per-point radius. A click
inside any circle counts as a hit on that point.

---

# 4. Forbidden Behaviors

**FORBID-296-SC1:** ScatterSeries MUST NOT compute radius from data
values. Radius is visual — set by style or per-point override. If the
consumer wants data-mapped sizing (bubble chart), the consumer computes
radii and sets them on ScatterPoint. ScatterSeries is a renderer.

---

# 5. File Location

```
src/plot/series/scatter.rs
```

New file alongside `line.rs`, `bar.rs`, `candle.rs`. Re-exported from
`src/plot/series/mod.rs`.

---

# 6. Acceptance Criteria

1. ScatterSeries renders 10,000 points at 120fps
2. Circles have smooth antialiased edges at all radii (2px to 20px)
3. Border renders correctly (1px border on 4px radius circle)
4. Per-point color override produces distinct colors in a single series
5. Per-point radius override produces variable-sized circles
6. Hit-test correctly identifies clicked points
7. Viewport culling excludes off-screen points from vertex buffer
8. `ScatterStyle::default()` produces 4px white filled circles
9. `SeriesData::Scatter` updates points via `apply_data_update`
10. ScatterSeries composites correctly with CandleSeries and LineSeries (z-order)
11. Pan/zoom updates circle positions correctly (world-anchored, screen-sized)

---

# 7. Amendments to SPEC-HAL-296

## §4.1 Built-In Series — add entry:

```rust
/// Scatter series (markers, trade dots, signal indicators).
/// SDF circle rendering — one quad per point, fragment-shader antialiasing.
pub struct ScatterSeries { ... }
```

## §14 GPU Pipeline — add paragraph:

**circle.wgsl** — SDF circle rendering for ScatterSeries.

- Vertex shader: positions quad corners in NDC, passes center/radius/colors
  as varyings.
- Fragment shader: evaluates `distance(frag_pos, center)` against radius.
  Smooth 1px antialiasing at outer edge. Optional border via second
  smoothstep at `radius - border_width`. Discard fragments outside circle.

## §19 Acceptance Criteria — add items 29-31:

29. ScatterSeries renders filled circles at data points
30. ScatterSeries renders bordered circles when border_width > 0
31. Per-point radius and color overrides produce variable markers
