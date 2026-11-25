# SPEC-HAL-278: **Track Widget Display Styles**

### *Unified Visual Mode System for Track-Based Widgets*

**Author:** JC
**Status:** Implemented
**Amended by:** SPEC-HAL-359 (binding cross-pipeline Z execution)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-210 (Slider Control and Value Exposure Invariants)
* SPEC-HAL-211 (ProgressBar and BusyIndicator)
* SPEC-HAL-275 (RangeSlider)

---

# 1. Purpose

Define a unified display style system for all track-based widgets. Four visual modes serve different UI contexts — from classic form controls to dense trading dashboards to radial gauges.

The value model, domain semantics, interaction invariants, and accessibility contracts are UNCHANGED across all styles. Only the rendering, layout geometry, and hit-testing paths differ.

---

# 2. Affected Widgets

| Widget | Track | Bar | Pill | Radial |
|--------|-------|-----|------|--------|
| **Slider** | Thin track + circle thumb | Square bar + edge tab | Falls back to Bar | Circular arc + angular drag |
| **RangeSlider** | Thin track + two circle thumbs | Square bar + edge tabs + center-drag | Falls back to Bar | Falls back to Track |
| **ProgressBar** | Thin track + fill | Square bar + fill | Capsule + fill | Circular ring + arc fill |
| **BusyIndicator** | Thin track + sliding segment | Square bar + sliding segment | Capsule + sliding segment | Circular ring + rotating arc |

---

# 3. Style Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackDisplayStyle {
    #[default]
    Track,
    Bar,
    Pill,
    Radial,
}
```

Lives in `src/core/paint.rs`.

### Style Availability by Widget

| Style | Slider | RangeSlider | ProgressBar | BusyIndicator |
|-------|--------|-------------|-------------|---------------|
| Track | ✓ native | ✓ native | ✓ native | ✓ native |
| Bar | ✓ native | ✓ native + center-drag | ✓ native | ✓ native |
| Pill | → falls back to Bar | → falls back to Bar | ✓ native (capsule) | ✓ native (capsule) |
| Radial | ✓ native (rotary knob) | → falls back to Track | ✓ native (ring gauge) | ✓ native (spinning arc) |

---

# 4. Invariants

### INV-278-A1: Style Is Construction-Time

Display style is set at construction or via builder. It does NOT change at runtime.

### INV-278-A2: Same Value Semantics

All styles produce identical values. Callbacks fire identically.

### INV-278-A3: Same Domain

`SliderDomain` (min, max, step) and anchor snap points are identical across all styles. Anchors do not apply to Radial.

### INV-278-A4: Owner-Correct Pipeline and Strict Z

Each visual element uses the pipeline that owns its geometry and appearance.
Every required overlap relation uses strictly different Z values. SPEC-HAL-359
executes those values across pipeline families; equal-Z pipeline or clip order
remains non-semantic under SPEC-PM-007.

| Style | Pipeline | Primitives |
|-------|----------|-----------|
| Track | Mixed (DecorationStack) | Existing owner-correct behavior |
| Bar | Rect | `push_rect`, `push_border_rect` |
| Pill | Path | `push_rounded_rect`, `push_border_rounded_rect` |
| Radial | Radial gauge | `push_radial_gauge` |

The former single-pipeline workaround is retired. Mixing pipelines is valid
when each primitive is owner-correct and every visual front/back relation has a
strict Z allocation.

---

# 5. Theme Geometry

### TrackBarGeometry (shared by Bar and Pill)

```rust
pub struct TrackBarGeometry {
    pub bar_height: f32,
    pub handle_hit_width: f32,
    pub handle_visual_width: f32,
    pub radius: f32,
}
```

| Density | bar_height | handle_hit_width | handle_visual_width | radius |
|---------|-----------|-----------------|-------------------|--------|
| Compact | 16.0 | 8.0 | 3.0 | 2.0 |
| Regular | 20.0 | 10.0 | 3.0 | 2.0 |
| Comfortable | 24.0 | 12.0 | 4.0 | 3.0 |

### Radial Layout

Radial widgets are **square**: width = height = `bar_height * 2.0`.

---

# 6. Style Rendering Rules

## 6.1 Bar Style

Square corners. All `push_rect` / `push_border_rect`.

Z-layer ordering: z=1 background, z=2 border, z=3 fill, z=4 anchor ticks + handle backgrounds, z=5 handle detail.

**Handles (Slider):** Vertical edge tab — 4px wide, extends 3px above/below bar. `borders.strong` at rest.

**Handles (RangeSlider):** Two vertical edge tabs. Three animated hover channels (70ms EaseOut): low edge, high edge, center. Center hover lifts fill luminance. Edge hover brightens handle.

**Center-drag (RangeSlider):** Click inside fill → drag entire range. Gap preserved. No anchor snap during center-drag. Grab offset preserved (no range jump).

## 6.2 Pill Style

Fully rounded ends (radius = `bar_height * 0.5`). All `push_rounded_rect` / `push_border_rounded_rect`. Only available on ProgressBar and BusyIndicator. Slider and RangeSlider fall back to Bar.

## 6.3 Radial Style

Dedicated `radial_gauge` shader pipeline. Single quad, fragment shader computes arc via `atan2`.

Shader params: `fill` [0,1], `thickness` (ring width), `track_color`, `fill_color`. Arc starts at 12 o'clock, proceeds clockwise. Anti-aliased ring edges.

Slider Radial: angular drag via `atan2(-dx, dy)`. RangeSlider: not supported (falls back to Track).

---

# 7. Hit-Testing

- **Bar/Pill edge handles:** hit zone = `handle_hit_width / 2` each side
- **RangeSlider center-drag:** area between edge handles; edge priority over center when overlapping
- **Outside range click:** moves nearest handle
- **Radial:** angular mapping within bounding square

---

# 8. Center-Drag Semantics (RangeSlider Bar)

- Gap preserved (`high - low` constant)
- Edge clamped at domain boundaries
- No anchor snap during center-drag
- Grab offset stored at drag start

---

# 9. Builder API

```rust
pub fn with_display_style(mut self, style: TrackDisplayStyle) -> Self
```

Default: `TrackDisplayStyle::Track`.

---

# 10. Pipeline Architecture

### 10.1 Radial Gauge Pipeline

| Component | File |
|-----------|------|
| Shader | `src/renderer/shaders/radial_gauge.wgsl` |
| Pipeline ID | `PipelineId::RadialGauge` in `batcher.rs` |
| Buffer builder | `build_radial_gauge_buffers_from_surface_primitives` in `wgpu_renderer.rs` |
| Scene primitive | `SceneNode::RadialGauge` |
| PaintSink method | `push_radial_gauge()` |

### 10.2 Pipeline Trap Documentation

Bugs encountered and documented for future reference:

1. **Batcher offset pollution** — dedicated pipelines need own vertex/index offset counters
2. **`compile_multi` no-op arms** — prevent rect offset adjustment on dedicated pipelines
3. **Globals struct mismatch** — shader struct must match exact byte layout (80 bytes)
4. **DPI scale_factor** — buffer builders must account for scale (or use logical coords with view_proj)
5. **Cross-pipeline z-ordering (retired defect)** — SPEC-HAL-359 now executes strict Z globally within each surface
6. **Overlay pass ordering (retired defect)** — every primitive family routes by its own overlay-range Z
7. **Coordinate space** — view_proj expects logical coordinates; physical-space params cause invisible rendering

---

# 11. Acceptance Criteria

- Track style identical to pre-SPEC behavior
- Bar style: square bar with edge tabs, center-drag on RangeSlider
- Pill style: capsule shape on ProgressBar and BusyIndicator
- Radial style: circular arc gauge with angular drag on Slider
- No cross-pipeline z-ordering issues
- All styles respect density modes

---

# 12. Non-Goals

- Vertical orientation
- Custom handle shapes
- Animated style transitions
- RangeSlider Radial
