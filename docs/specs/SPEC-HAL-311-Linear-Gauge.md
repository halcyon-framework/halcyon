# SPEC-HAL-311: Linear Gauge

**Author:** JC
**Status:** Draft
**Depends on:** SPEC-HAL-307 (Gauge — shared types)

---

## 1. Purpose

A horizontal (or vertical) bar gauge. Same value model
as the radial gauge (SPEC-HAL-307): min/max range, zones,
fill styles, ticks, origin for bidirectional fill. Linear
geometry instead of arc geometry.

Use cases: VU meters, signal strength bars, temperature
bars, battery indicators, radio scanner displays,
horizontal level meters, any bar-style measurement.

---

## 2. Shared Types (from SPEC-HAL-307)

Reuse without duplication:

- **GaugeZone** — `{ start: f64, end: f64, color: Color }`
- **GaugeFillStyle** — Solid, ZoneColored, ValueLerp,
  IntensityRamp, IntensityRampReverse
- **GaugeTickMarks** — major_divisions, minor_per_major,
  show_labels, major_length, minor_length
- **GaugeMode** — Fill, Needle (needle = thin marker at
  value position instead of filled bar)

---

## 3. Config

```rust
pub struct LinearGaugeConfig {
    /// Value range.
    pub min: f64,
    pub max: f64,
    /// Fill origin. Fill extends FROM origin TOWARD value.
    /// Default: min (fill from left).
    pub origin: f64,
    /// Bar orientation. Default: Horizontal.
    pub orientation: LinearGaugeOrientation,
    /// Colored zones. Maximum 4. Reuses GaugeZone.
    pub zones: Vec<GaugeZone>,
    /// Display mode. Default: Fill.
    pub mode: GaugeMode,
    /// Fill style. Default: Solid.
    pub fill_style: GaugeFillStyle,
    /// Show numeric value text. Default: true.
    pub show_value: bool,
    /// Value formatter. Default: "{:.0}".
    pub formatter: Option<Arc<dyn Fn(f64) -> String>>,
    /// Label text (below or beside the bar). Default: None.
    pub label: Option<String>,
    /// Bar thickness in logical pixels. Default: 12.0.
    pub bar_height: f32,
    /// Track (unfilled) color. None = theme border subtle.
    pub track_color: Option<Color>,
    /// Fill color. None = theme accent primary.
    pub fill_color: Option<Color>,
    /// Tick marks. Default: none.
    pub tick_marks: GaugeTickMarks,
    /// Show pointer line at value position. Default: false.
    pub show_pointer: bool,
    /// Corner radius for bar ends. Default: bar_height / 2
    /// (fully rounded capsule). Set 0 for square ends.
    pub corner_radius: Option<f32>,
}

pub enum LinearGaugeOrientation {
    Horizontal,
    Vertical,
}
```

---

## 4. Widget

```rust
pub struct LinearGauge {
    id: WidgetInternalId,
    config: LinearGaugeConfig,
    value: f64,
}
```

Builder:

```rust
LinearGauge::new(config)
    .with_value(50.0)
```

---

## 5. Policy

**Display-only.** LinearGauge is a projection widget.
Consumer sets the value. No input events. No keyboard.
No pointer interaction beyond optional tooltip hover.

FORBID-311-1: No keyboard handlers.
FORBID-311-2: No pointer interaction that changes value.
FORBID-311-3: No internal state beyond current value.

---

## 6. Rendering

### Scene Graph Path

Render using existing scene primitives:
- `push_rounded_rect` for track background
- `push_rounded_rect` for fill bar (clipped to value)
- `push_rect` for tick marks
- Text engine for value label and tick labels

No custom shader needed. The radial gauge needed a
shader because arc math is complex. Linear bars are
just rectangles — scene graph primitives handle this.

### Fill calculation

```
normalized = (value - min) / (max - min)
origin_t = (origin - min) / (max - min)

// Fill bar extends from origin_t to normalized
fill_start = min(origin_t, normalized)
fill_end = max(origin_t, normalized)

// In pixels (horizontal):
bar_x = rect.x + fill_start * rect.width
bar_w = (fill_end - fill_start) * rect.width
```

### Zone rendering

Each zone maps to a colored rect within the bar track.
Zone positions are in value space (not normalized):

```
zone_start_px = ((zone.start - min) / (max - min)) * rect.width
zone_end_px = ((zone.end - min) / (max - min)) * rect.width
```

Zones paint behind the fill bar (z = LAYER_CONTENT).
Fill bar paints on top (z = LAYER_CONTENT + 1).

### Tick marks

Perpendicular lines at computed positions along the bar.
Major ticks: full `major_length` extending beyond the bar.
Minor ticks: smaller `minor_length`. Labels at major
ticks via text engine.

---

## 7. Layout

Height = bar_height + tick overhang + label height + padding.
Width = fills available width from parent constraints.

For vertical orientation: swap width/height roles.

---

## 8. Invariants

INV-311-1: LinearGauge is projection-only. Value set by
consumer, never by the widget.

INV-311-2: Rendering uses scene graph primitives only.
No custom GPU pipeline.

INV-311-3: Shared types (GaugeZone, GaugeFillStyle,
GaugeTickMarks, GaugeMode) are defined once in gauge.rs
and reused. No duplication.

INV-311-4: Zone rendering is paint-time projection.
LinearGauge does not own zone state or compute zone
boundaries from value — zones are static config.

---

## 9. Demo

Horizontal gauges driven by the same JumpGenerator +
SpringDamper pattern from the radial gauge demo.
Multiple gauges showing different configurations:
- Simple fill bar
- Zone-colored (green/yellow/red)
- Bidirectional (origin at center)
- With tick marks and labels
