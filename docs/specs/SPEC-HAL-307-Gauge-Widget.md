# SPEC-HAL-307: **Gauge Widget**

### *Radial Arc Meter with Zones, Needle, and Value Display*

**Author:** JC
**Status:** Draft (v2)
**Depends on:** SPEC-HAL-002 (Constitution), SPEC-HAL-278 (Radial Gauge Primitive)

---

# 1. Purpose

A radial arc meter displaying a single value within a range.
Configurable colored zones. Animated needle or fill arc.
Numeric value display. Designed for real-time, high-frequency
value updates with responsive visual feedback.

The existing `RadialGaugeNode` scene primitive and dedicated
GPU pipeline (shader, vertex type, uniform buffer) provide
the rendering foundation. The Gauge widget extends this
pipeline with: zone coloring, configurable arc sweep and
origin, needle mode, tick marks, and value easing.

**Key design decisions:**

- **Shader-rendered** — single quad, single draw call, GPU
  per-pixel. No CPU-side arc tessellation. Maximum
  responsiveness for real-time data.
- **Origin-based fill** — fill arc starts from a configurable
  `origin` value, extending toward the current value in
  either direction. Supports both traditional 0→100 gauges
  and centered ±deviation meters.
- **Full circle support** — `arc_degrees` ranges from 0 to
    360. Not restricted to semicircle or three-quarter arc.

---

# 2. Data Model

```rust
pub struct GaugeConfig {
    /// Minimum value. Default: 0.0.
    pub min: f64,
    /// Maximum value. Default: 100.0.
    pub max: f64,
    /// Fill origin value. The fill arc extends FROM this value
    /// TOWARD the current value. If value > origin, fills
    /// clockwise. If value < origin, fills counter-clockwise.
    /// Default: min (traditional gauge behavior).
    ///
    /// Example: origin=0, min=-100, max=100 creates a centered
    /// bidirectional meter. Positive values fill right from
    /// center, negative values fill left from center.
    pub origin: f64,
    /// Arc sweep in degrees. Range: (0, 360].
    /// 180 = semicircle, 270 = three-quarter, 360 = full circle.
    /// Default: 270.
    pub arc_degrees: f32,
    /// Colored zones on the arc track. Maximum 4 zones.
    /// Zones are rendered as part of the track background.
    /// Zone boundaries are in value space (not angle space).
    pub zones: Vec<GaugeZone>,
    /// Display mode.
    pub mode: GaugeMode,
    /// Show numeric value text. Default: true.
    pub show_value: bool,
    /// Value formatter. Default: "{:.0}".
    pub formatter: Option<Arc<dyn Fn(f64) -> String + Send + Sync>>,
    /// Label below the value text.
    pub label: Option<String>,
    /// Track thickness as fraction of radius. Default: 0.15.
    pub thickness: f32,
    /// Track background color (behind zones, visible where no
    /// zone covers). Default: theme border subtle.
    pub track_color: Option<Color>,
}

pub struct GaugeZone {
    /// Value where zone begins.
    pub start: f64,
    /// Value where zone ends.
    pub end: f64,
    /// Zone color.
    pub color: Color,
}

pub enum GaugeMode {
    /// Filled arc from origin to current value.
    Fill,
    /// Thin needle line pointing at current value.
    Needle,
}
```

---

# 3. Consumer API

```rust
// Traditional 0-100 gauge with zones
let gauge = Gauge::new()
    .with_config(GaugeConfig {
        min: 0.0,
        max: 100.0,
        arc_degrees: 270.0,
        zones: vec![
            GaugeZone { start: 0.0, end: 60.0, color: Color::from_hex(0x2ECC71) },
            GaugeZone { start: 60.0, end: 85.0, color: Color::from_hex(0xF39C12) },
            GaugeZone { start: 85.0, end: 100.0, color: Color::from_hex(0xE74C3C) },
        ],
        mode: GaugeMode::Needle,
        ..Default::default()
    })
    .with_value(42.0);

// Centered bidirectional meter (origin at 0)
let deviation = Gauge::new()
    .with_config(GaugeConfig {
        min: -50.0,
        max: 50.0,
        origin: 0.0,
        arc_degrees: 180.0,
        mode: GaugeMode::Fill,
        ..Default::default()
    })
    .with_value(-12.0); // fills left from center

// Full circle progress ring
let ring = Gauge::new()
    .with_config(GaugeConfig {
        min: 0.0,
        max: 1.0,
        arc_degrees: 360.0,
        mode: GaugeMode::Fill,
        ..Default::default()
    })
    .with_value(0.73);

// Update (real-time):
gauge.set_value(73.0); // eases toward target
```

---

# 4. Rendering

## 4.1 Arc Geometry

The gauge arc spans `arc_degrees` centered at the bottom.
For 270°: the gap is 90° at the bottom (135° to 225° missing).
The arc runs from `start_angle = 135°` to `end_angle = 405°`
(clockwise from 12 o'clock).

For 360°: full circle, no gap. start_angle = 0° (top).

For 180°: semicircle, gap is 180° at the bottom.
Arc from 180° to 360° (left to right across the top).

Value maps linearly to angle:

```
t = (value - min) / (max - min)
angle = start_angle + t * arc_degrees
```

Origin maps the same way:

```
t_origin = (origin - min) / (max - min)
origin_angle = start_angle + t_origin * arc_degrees
```

## 4.2 Zone Rendering (Shader-Based)

Zones are rendered by extending the existing `radial_gauge`
GPU shader pipeline. Each zone maps a value range to an
angle range and a color. The fragment shader checks which
zone the current pixel's angle falls within and applies that
zone's color instead of the track color.

### Uniform Extension

The `RadialGaugeParamsUniform` is extended with zone data
and arc control fields. Current struct is 64 bytes in a
256-byte aligned slot, leaving 192 bytes free.

New fields (added after existing `_pad`):

```rust
/// Arc start angle in radians (0 = top, clockwise).
start_angle: f32,
/// Arc sweep in radians.
sweep_angle: f32,
/// Origin fill position [0.0, 1.0] within the arc.
origin_fill: f32,
/// Number of active zones (0-4).
zone_count: u32,
/// Mode: 0 = fill, 1 = needle.
mode: u32,
/// Padding to align zone array.
_pad2: [f32; 3],
// Zone array: 4 × (start_t: f32, end_t: f32, color: [f32; 4]) = 4 × 24 = 96 bytes
zone0_range: [f32; 2],
zone0_color: [f32; 4],
zone1_range: [f32; 2],
zone1_color: [f32; 4],
zone2_range: [f32; 2],
zone2_color: [f32; 4],
zone3_range: [f32; 2],
zone3_color: [f32; 4],
```

Total: 64 (existing) + 20 (new scalars) + 96 (zones) = 180
bytes. Fits within the 256-byte slot.

Zone `start_t` and `end_t` are normalized [0.0, 1.0] within
the arc sweep (not raw angles). The shader maps pixel angle
to normalized position and checks each zone.

### Shader Changes

The fragment shader gains:

1. **Arc bounds** — pixels outside `[start_angle, start_angle + sweep_angle]` are transparent (supports partial arcs).
2. **Zone lookup** — for each pixel in the arc, check zone boundaries. If inside a zone, use zone color. Otherwise, use track color.
3. **Fill from origin** — fill arc runs from `origin_fill` to `fill` (current value normalized). If `fill < origin_fill`, the arc fills counter-clockwise from origin.
4. **Needle mode** — thin angular wedge at the fill position instead of filled arc.

### Why Shader, Not Path

- **Real-time performance** — single draw call, GPU-computed.
  No CPU tessellation per zone per frame. Critical for gauges
  updating at high frequency.
- **Anti-aliased edges** — smoothstep AA is built into the
  shader. Path-based arcs would need explicit AA.
- **Pipeline already exists** — RadialGaugeNode, vertex
  buffers, uniform management, bind groups are all wired.
  Only the shader and uniform struct need extension.

## 4.3 Needle Mode

A thin angular wedge centered at the current value's angle,
rendered in the fragment shader. The shader checks if the
pixel's angle is within ±needle_half_width of the value
angle and within the ring radius. Color: `fill_color`.

For a physical needle appearance (triangle from center to
edge), a separate Path-based triangle overlay may be added
in a later phase if the shader wedge is insufficient.

## 4.4 Fill Mode

Fill arc runs from `origin_angle` to the current value's
angle. The shader compares pixel angle against origin and
value positions:

- If `value >= origin`: pixel is filled if its angle is
  between origin_angle and value_angle (clockwise).
- If `value < origin`: pixel is filled if its angle is
  between value_angle and origin_angle (counter-clockwise
  from origin).

Fill color: the zone color at the current value position,
or `fill_color` if no zone covers the value.

## 4.5 Value Easing

`display_value: f64` eases toward `target_value: f64` via
exponential decay in `update()`:

```rust
let alpha = 1.0 - (-8.0 * dt).exp();
self.display_value += (self.target_value - self.display_value) * alpha;
```

For real-time gauges receiving high-frequency updates, the
easing rate constant (8.0) provides responsive tracking
while smoothing jitter. Consumer may adjust via
`easing_rate: f32` in config (future extension).

## 4.6 Value Text

Formatted value centered below the arc (or inside the arc
for 270°+ sweeps). Uses `formatter` or default `"{:.0}"`.
Label text below the value in smaller font.

For 360° (full ring): value text centered inside the ring.
For 180° (semicircle): value text centered below the arc.
For 270° (three-quarter): value text centered in the gap.

---

# 5. Policy

```rust
pub enum GaugePolicy {
    /// Display only. Consumer sets value programmatically.
    Display,
    /// Interactive: user can drag the needle to set value.
    /// Emits GaugeEvent::ValueChanged.
    Input,
}
```

Default: `Display`.

---

# 6. Interaction

In `Input` policy mode:
- Pointer down on the arc → capture → drag rotates the needle.
- Emits `GaugeEvent::ValueChanged { value: f64 }`.
- Value clamped to `[min, max]`.

In `Display` mode: no interaction. Hover shows tooltip with
exact value.

```rust
pub enum GaugeEvent {
    ValueChanged { value: f64 },
}
```

---

# 7. Invariants

**INV-307-0:** Projection-only in Display mode. Consumer sets value.
**INV-307-1:** Value clamped to `[min, max]` at render time.
**INV-307-2:** Display value eases toward target (not instant snap).
**INV-307-3:** Zones tile the arc without overlap or gap.
**INV-307-4:** Widget layout is square (width = height).
**INV-307-5:** Origin is clamped to `[min, max]`.
**INV-307-6:** Maximum 4 zones per gauge (shader uniform constraint).
**INV-307-7:** arc_degrees must be in range (0, 360].
**INV-307-8:** Rendering uses the existing RadialGauge GPU pipeline
(extended). No new pipelines, no offscreen textures.

---

# 8. Forbidden Behaviors

**FORBID-307-0:** Gauge must NOT create new GPU pipelines or
offscreen textures. Extends the existing RadialGauge pipeline.
**FORBID-307-1:** Gauge must NOT modify consumer data. Value is
set externally, display easing is internal.
**FORBID-307-2:** In Display mode, gauge must NOT accept input
or emit events (except tooltip hover).
**FORBID-307-3:** Gauge must NOT use Path-based arc tessellation
for zones or fill. Zones and fill are shader-computed for
real-time performance.

---

# 9. Acceptance Criteria

1. Renders arc track with up to 4 colored zones
2. Needle mode: angular wedge at value angle
3. Fill mode: filled arc from origin to value
4. Fill works bidirectionally when value crosses origin
5. Value eases smoothly toward target
6. Formatted value text centered appropriately for arc type
7. Label text below value
8. 180°, 270°, and 360° arc sweeps render correctly
9. Input mode: drag needle emits ValueChanged
10. Display mode: no input accepted
11. Zones tile correctly with no gaps/overlap
12. Origin at center of range: fill extends both directions
13. Real-time value updates at 60+ fps without jank

---

# 10. Implementation Phases

**Phase 1:** Extend RadialGauge shader and uniform with
start_angle, sweep_angle, zone support (max 4 zones),
origin_fill. Extend RadialGaugeParamsUniform and
RadialGaugeNode. Add SceneGraph::push_radial_gauge method.
Gauge widget struct (Widget trait), Display mode only,
Fill mode, value easing. Demo with zones and real-time
value updates.

**Phase 2:** Needle mode (shader angular wedge). Tick
marks along arc (minor shader extension or Path overlay).

**Phase 3:** Input mode. Drag interaction, GaugePolicy,
GaugeEvent. Pointer angle computation for drag-to-value.

**Phase 4:** Visual polish — gradient fills within zones,
glow effects on needle, configurable easing rate.

---

# 11. Consistency Matrix (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. Widget in tree. |
| B — Phase Separation | Preserved. Paint is pure. Easing in update(). |
| C — ID-based Identity | Preserved. WidgetId. |
| D — Pure Data | Preserved. Config + value are pure data. |
| E — GPU-First | **Strengthened.** Shader-rendered, single draw call. |
| F — Sovereignty | Preserved. No OS interaction. |
