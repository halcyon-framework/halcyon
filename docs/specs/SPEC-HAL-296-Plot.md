# SPEC-HAL-296: **Plot — Coordinate-Authority Rendering System**

### *GPU-Native Data Visualization With Shared Axis Synchronization*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-001 (Core Architecture)
* SPEC-HAL-295 (Custom Render Pass Extension Point)

**Related SPECs (Plot ecosystem):**
* SPEC-HAL-297 (Plot Tool System)
* SPEC-HAL-298 (Widget Spotlight — design only, implementation deferred)
* SPEC-HAL-299 (Plot Price Levels, Order Markers & Tags — Implemented)
* SPEC-HAL-300 (Depth Bar Series — Draft)
* SPEC-HAL-301 (Heatmap Series — Draft)
* SPEC-HAL-302 (Indicator Series Types — Draft)
* SPEC-HAL-303 (Order Book Widget — Draft)

---

# 1. Purpose

This SPEC defines the Plot system — Halcyon's first-class module for
rendering data in coordinate space. Plot is why Halcyon exists.

A Plot is a coordinate-authority rendering surface. It owns a 2D
coordinate system (X × Y), renders data series through its own GPU
pipeline via an offscreen texture, supports pan/zoom/interaction in
its coordinate space, and can share axes with other Plots for
synchronized views.

A candlestick chart is a Plot where X = time, Y = price, and the series
type is `CandleSeries`. An equity curve is a Plot where X = time,
Y = PnL, and the series type is `LineSeries`. A depth heatmap is a Plot
where X = time, Y = price, Z = liquidity mapped to color. An order book
is a Plot where X = quantity, Y = price, and the series type is
`DepthBarSeries`.

---

# 2. Module Location

```
halcyon/
  src/
    plot/                ← peer to core/, widgets/, renderer/
      mod.rs             ← Plot widget, module root
      axis.rs            ← Axis types and configuration
      axis_authority.rs  ← shared axis synchronization (any axis)
      probe.rs           ← shared coordinate cursor (cross-widget)
      view_state.rs      ← viewport state (coordinate ranges)
      transform.rs       ← world ↔ screen mapping
      render_pass.rs     ← CustomRenderPass impl (GPU pipeline owner)
      interaction.rs     ← pan, zoom, crosshair, selection
      hit_test.rs        ← coordinate-space object identity
      series/            ← pluggable data visualizations
        mod.rs
        candle.rs        ← CandleSeries (instanced OHLC rendering)
        line.rs          ← LineSeries (moving averages, indicators)
        bar.rs           ← BarSeries (volume, histograms)
        rect.rs          ← RectSeries (regions, zones, session backgrounds)
        heatmap.rs       ← HeatmapSeries (depth heatmap, liquidity)
        depth_bar.rs     ← DepthBarSeries (order book horizontal bars)
      geometry/          ← GPU vertex/instance builders
        mod.rs
        candle_geometry.rs
        line_geometry.rs
        bar_geometry.rs
        rect_geometry.rs
        heatmap_geometry.rs
        depth_bar_geometry.rs
      snapshot.rs         ← FrameSnapshot, RenderCandle
      annotation.rs       ← world-anchored interactive elements
      shaders/
        candle_body.wgsl
        overlay.wgsl
```

Plot is NOT inside `widgets/`. It is a domain module — a peer to `core`,
`renderer`, `widgets`. The `Plot` struct implements `Widget`, but the
module contains far more than a single widget.

---

# 3. Core Types

## 3.1 Axis

```rust
/// Axis type configuration. Determines coordinate interpretation,
/// formatting, and interaction behavior.
pub enum AxisKind {
    /// Continuous time axis. Values are i64 microseconds since UNIX epoch.
    /// Pan/zoom operates in time. Labels format as datetime.
    Time,

    /// Linear numeric axis. Values are f64.
    /// Pan/zoom operates linearly. Labels format as numbers.
    Linear,

    /// Logarithmic numeric axis. Values are f64 stored in world space
    /// and mapped logarithmically during Transform. Domain must be positive.
    Log,
}

/// Axis configuration for one dimension of a Plot.
pub struct Axis {
    pub kind: AxisKind,
    /// Label formatting callback (value → display string).
    pub formatter: Option<Box<dyn Fn(f64) -> String>>,
    /// Whether axis labels are visible.
    pub show_labels: bool,
    /// Whether gridlines are drawn for this axis.
    pub show_grid: bool,
}
```

## 3.2 ViewState

Carried forward from the original chart renderer. Pure data — the "camera."

```rust
/// Viewport state in world-space coordinates.
/// The single source of truth for what the Plot displays.
pub struct ViewState {
    /// X-axis range (time_min/time_max for time axis, or arbitrary f64).
    pub x_min: f64,
    pub x_max: f64,

    /// Y-axis range (price, volume, indicator value, etc.).
    pub y_min: f64,
    pub y_max: f64,

    /// Runtime auto-follow latch (scroll X to track latest data point).
    pub follow_latest: bool,
}
```

**INV-296-VS1:** ViewState uses f64 for ALL coordinate values. No f32
until after Transform. This preserves precision for molecular-scale
prices (PEPE at 0.00000001) and massive prices (BTC at 100,000).

**INV-296-VS2:** Interaction modifies ViewState only. Transform is
derived from ViewState each frame. Interaction never mutates Transform.

## 3.3 Transform

Bidirectional mapping between world and screen coordinates.

```rust
/// Ephemeral transform computed from ViewState + viewport pixel size.
/// Created fresh each frame. Never mutated directly.
pub struct Transform {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub x_kind: AxisKind,
    pub y_kind: AxisKind,
    pub width_px: f32,
    pub height_px: f32,
}

impl Transform {
    pub fn from_view_state(vs: &ViewState, w: f32, h: f32) -> Self;
    pub fn from_view_state_with_axes(
        vs: &ViewState,
        w: f32,
        h: f32,
        x_kind: AxisKind,
        y_kind: AxisKind,
    ) -> Self;

    /// World X → screen pixel. f64 math until final f32 cast.
    pub fn x_to_px(&self, x: f64) -> f32;

    /// World Y → screen pixel. Inverted (Y=0 at top). f64 until f32.
    pub fn y_to_px(&self, y: f64) -> f32;

    /// Screen pixel → world X. Returns f64.
    pub fn px_to_x(&self, px: f32) -> f64;

    /// Screen pixel → world Y. Returns f64.
    pub fn px_to_y(&self, py: f32) -> f64;

    /// Checked variants return None for invalid domains, including
    /// non-positive values on logarithmic axes.
    pub fn x_to_px_checked(&self, x: f64) -> Option<f32>;
    pub fn y_to_px_checked(&self, y: f64) -> Option<f32>;
    pub fn px_to_x_checked(&self, px: f32) -> Option<f64>;
    pub fn px_to_y_checked(&self, py: f32) -> Option<f64>;
}
```

**INV-296-TR1:** Transform normalizes relative distances, not absolute
values. `y_to_px` computes `(y - y_min) / (y_max - y_min)` in f64,
then multiplies by height_px, then casts to f32. This prevents collapse
on molecular-scale prices.

For `AxisKind::Log`, Transform normalizes `log10(value)` relative to
`log10(min)..log10(max)`. Non-positive log-domain values are invalid and
must not emit geometry.

**INV-296-TR2:** Transform is ephemeral. Created from ViewState each
frame. Never stored across frames. Never mutated.

## 3.4 AxisAuthority

The axis synchronization primitive. Pure data, shared via Arc. Either
axis (X or Y) can be shared independently. A candlestick chart and an
order book share their Y-axis (price). Multiple charts share their
X-axis (time).

```rust
/// Shared axis state. Multiple Plots reading the same AxisAuthority
/// stay synchronized on that axis — pan one, all pan. Zoom one, all zoom.
///
/// Not limited to time or X. A chart and an order book can share a
/// Y-axis AxisAuthority (price), while each has independent X axes.
pub struct AxisAuthority {
    /// Current axis range.
    min: f64,
    max: f64,

    /// Generation counter. Incremented on every mutation.
    /// Plots compare against their cached generation to detect changes.
    generation: u64,
}

impl AxisAuthority {
    pub fn new(min: f64, max: f64) -> Self;
    pub fn range(&self) -> (f64, f64);
    pub fn generation(&self) -> u64;
    pub fn set_range(&mut self, min: f64, max: f64);
    pub fn pan(&mut self, delta: f64);
    pub fn zoom(&mut self, factor: f64, center: f64);
}
```

Shared via `Arc<Mutex<AxisAuthority>>` (or lock-free if contention
becomes an issue). Plots can live in different containers, different
panels, different tabs, different windows. They synchronize through
the shared authority, not through widget relationships.

**INV-296-AA1:** AxisAuthority is pure data. No widget references.
No callbacks. No event subscriptions. Plots poll the generation counter.

**INV-296-AA2:** A Plot with a shared X-axis AxisAuthority reads
x_min/x_max from the authority. Pan/zoom on the X axis goes through
the authority. The Y axis is either local or shared via a separate
AxisAuthority.

**INV-296-AA3:** A Plot with a shared Y-axis AxisAuthority reads
y_min/y_max from the authority. Pan/zoom on the Y axis goes through
the authority. The X axis is either local or shared via a separate
AxisAuthority.

**INV-296-AA4:** A Plot without any shared AxisAuthority owns its
ViewState entirely — both axes are local.

**INV-296-AA5:** X and Y axis authorities are independent. A Plot
can share X only, Y only, both, or neither.

## 3.5 Probe

The cross-widget coordinate cursor. When a user hovers over one Plot
(or any widget that writes to a Probe), all other Plots and widgets
reading the same Probe can react — rendering highlights, crosshair
projections, or visual feedback at the probed coordinate.

```rust
/// Shared coordinate probe. Any widget can write to it (on hover).
/// Any widget can read it and render accordingly.
///
/// Examples:
/// - Hover order book at price 105.2 → chart shows horizontal line at 105.2
/// - Hover chart at (time T, price P) → order book highlights level nearest P
/// - Position manager highlights an open order → chart shows marker at that price
pub struct Probe {
    /// World X coordinate of the probe. None = no X probe active.
    pub x: Option<f64>,

    /// World Y coordinate of the probe. None = no Y probe active.
    pub y: Option<f64>,

    /// Which widget wrote the probe (so it can skip rendering its own
    /// feedback — the chart already has its own crosshair, it doesn't
    /// need to also render the probe it just wrote).
    pub source: Option<WidgetInternalId>,

    /// Generation counter for change detection.
    pub generation: u64,
}

impl Probe {
    pub fn new() -> Self;
    pub fn set(&mut self, x: Option<f64>, y: Option<f64>, source: WidgetInternalId);
    pub fn clear(&mut self);
    pub fn generation(&self) -> u64;
}
```

Shared via `Arc<Mutex<Probe>>`. Same constitutional pattern as
AxisAuthority — pure data, no widget references, no callbacks.

**INV-296-PB1:** Probe is pure data. No widget references beyond the
source ID (which is used only for self-suppression, not for lookup or
communication).

**INV-296-PB2:** The source widget skips rendering probe feedback for
itself. A chart that wrote the probe at price P already renders its own
crosshair at P — it does not also render the probe highlight.

**INV-296-PB3:** Probe consumers render feedback appropriate to their
domain. A chart renders a horizontal/vertical line. An order book
highlights the nearest price level. A position widget highlights
matching orders. Each consumer decides how to visualize the probe.

**INV-296-PB4:** Probe coordinates are in shared world space. If a
chart and order book share a Y-axis AxisAuthority, the Probe's Y value
is in that shared coordinate space — both interpret it identically.

---

# 4. Series Trait

A Series is a pluggable data visualization within a Plot. It knows
how to render its data type in the coordinate space the Plot provides.

```rust
/// A data visualization within a Plot.
///
/// Implementations produce GPU geometry from their data model,
/// using the Plot's Transform for coordinate mapping.
pub trait Series: Send + Sync {
    /// Prepare GPU resources for this frame.
    ///
    /// Called during the compile phase with Device/Queue access.
    /// Build geometry, upload buffers, update uniforms.
    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        transform: &Transform,
        viewport: PhysicalRect,
    );

    /// Render into the offscreen texture.
    ///
    /// Called inside the Plot's render pass. Set pipeline, draw.
    fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        viewport: PhysicalRect,
    );

    /// Initialize GPU pipelines and persistent resources.
    ///
    /// Called once when the series is added to a Plot.
    fn init(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    );

    /// Z-order within the Plot's render pass.
    /// Lower values render first (behind higher values).
    fn z_order(&self) -> u32;

    /// Hit-test a screen-space point against this series' data.
    /// Returns an opaque hit result for the series to interpret.
    fn hit_test(
        &self,
        x: f32,
        y: f32,
        transform: &Transform,
    ) -> Option<Box<dyn std::any::Any>>;

    /// Declare this series' interaction capability.
    /// Default: None (display-only).
    fn interaction(&self) -> SeriesInteraction {
        SeriesInteraction::None
    }
}
```

**INV-296-S1:** Series are rendered in z_order within a single render
pass. Wicks (z=0) → bodies (z=1) → overlays (z=2) → crosshair (z=10).
A CandleSeries might internally use multiple z-orders for wicks vs bodies.

**INV-296-S2:** Series own their GPU pipelines and buffers. The Plot
does not know about candle instances, wick vertices, or indicator lines.
It calls `prepare()` then `render()`.

## 4.1 Built-In Series

```rust
/// OHLC candlestick rendering. Instanced draw with in-shader borders.
/// Uses candle_body.wgsl + overlay.wgsl from the original chart renderer.
pub struct CandleSeries { ... }

/// Line series (moving averages, indicators, equity curves).
/// GPU line strip rendering.
pub struct LineSeries { ... }

/// Bar series (volume, histograms).
/// Vertical bars anchored to X axis.
pub struct BarSeries { ... }

/// Rectangle series (regions, zones, session backgrounds, supply/demand).
/// Filled or outlined rectangles in world coordinates.
pub struct RectSeries { ... }

/// Heatmap series (depth heatmap, liquidity visualization).
/// X = time, Y = price, Z = intensity mapped to color gradient.
/// Renders as colored rectangles beneath candles (low z-order).
pub struct HeatmapSeries { ... }

/// Depth bar series (order book visualization).
/// Horizontal bars representing bid/ask quantity at each price level.
/// Used in order book Plots where X = quantity, Y = price.
pub struct DepthBarSeries { ... }
```

`CandleSeries` carries forward the entire GPU pipeline of the original chart renderer:
`CandleInstance` struct, `GeometryBuilder`, viewport culling, instanced
draw (6 vertices × N instances), `candle_body.wgsl` for in-shader
borders/selection/hover, `overlay.wgsl` for wicks/crosshair/box selection.

`HeatmapSeries` renders as a grid of colored rectangles. Each cell
represents a (time_bucket, price_level) with an intensity value mapped
through a `ColorGradient` (cold → hot). Rendered at low z-order so
candles paint over. Data source: historical order book snapshots bucketed
by time.

`DepthBarSeries` renders horizontal bars for order book visualization.
Bids extend left from a center line, asks extend right (or vice versa).
Color-coded by side. Each bar represents one price level's quantity.
Data source: live order book depth.

---

# 5. Offscreen Texture Rendering

Each Plot renders its series to an **offscreen wgpu texture**, not into
Halcyon's main render pass. Halcyon displays the texture as an image quad.

```
Plot::prepare()
    ↓ for each series: series.prepare(device, queue, transform, viewport)
    ↓
Plot creates own CommandEncoder
    ↓
Plot::render()
    ↓ begin_render_pass (on offscreen texture)
    ↓ for each series (z-sorted): series.render(&mut render_pass, viewport)
    ↓ end_render_pass
    ↓
queue.submit(encoder.finish())
    ↓
Halcyon's frame
    ↓ Plot::paint() calls cx.scene.push_image(texture_id, rect)
    ↓ Halcyon renders it as a regular image quad
```

**INV-296-OFF1:** The Plot's GPU work is independent of Halcyon's frame
loop. The Plot can render at 120fps while the UI idles. Halcyon displays
the latest texture.

**INV-296-OFF2:** The Plot creates its own `CommandEncoder` and submits
its own command buffer via the shared `Arc<Queue>`. No interference with
Halcyon's rendering.

**INV-296-OFF3:** Double buffering. Two offscreen textures. Plot writes
to back, Halcyon reads from front. Swap on completion. No synchronization
stalls.

**INV-296-OFF4:** The offscreen texture is registered via
`WgpuRenderer::register_image_texture()` (already exists). The widget
paints it via `PaintSink::push_image()` (already exists on
SceneBuilder). No new rendering infrastructure in Halcyon.

---

# 6. Plot Widget

Plot implements `Widget`. It participates in layout, paint, and event
routing like any other widget.

```rust
pub struct Plot {
    id: WidgetInternalId,

    // Coordinate system
    x_axis: Axis,
    y_axis: Axis,
    view_state: ViewState,

    // Shared axis authorities (either, both, or neither)
    shared_x: Option<Arc<Mutex<AxisAuthority>>>,
    shared_y: Option<Arc<Mutex<AxisAuthority>>>,
    cached_x_generation: u64,
    cached_y_generation: u64,

    // Cross-widget coordinate probe
    probe: Option<Arc<Mutex<Probe>>>,
    cached_probe_generation: u64,

    // Data visualization
    series: Vec<Box<dyn Series>>,

    // GPU resources
    render_pass_id: Option<CustomRenderPassId>,
    offscreen_texture_id: Option<u64>,
    offscreen_front: Option<wgpu::Texture>,
    offscreen_back: Option<wgpu::Texture>,

    // Interaction
    interaction: PlotInteraction,

    // Layout cache
    last_rect: RectF,
    last_physical_rect: PhysicalRect,

    // Widget state
    focused: bool,
    in_capture: bool,
}
```

### layout

Reports desired size. Accepts whatever constraints the parent gives.

### paint

```rust
fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
    // 1. Push the offscreen texture as an image quad
    if let Some(tex_id) = self.offscreen_texture_id {
        cx.scene.push_image(rect, tex_id, /* uv_rect */ ..., LAYER_CONTENT);
    }

    // 2. Paint probe feedback (if probe active and source is not self)
    self.paint_probe_feedback(cx, rect);

    // 3. Paint axis labels, gridlines via regular PaintSink
    self.paint_axes(cx, rect);

    // 4. Paint crosshair labels via PaintSink (screen-anchored)
    self.paint_crosshair_labels(cx, rect);
}
```

Halcyon composites the image quad with the axis labels, probe feedback,
and any child widgets (annotations, legends) that sit above.

### handle_event

```rust
fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
    // Transform pointer events from local coords to world coords
    // Dispatch to PlotInteraction for pan/zoom/crosshair/selection
    // PlotInteraction mutates ViewState (and AxisAuthority if shared)
    // Write to Probe on hover (world coords + self.id as source)
    // Series handle hit-testing for hover/selection
}
```

### Animation / Continuous Rendering

```rust
fn needs_animation(&self) -> bool {
    // True when:
    // - Live data feed active (chart updates continuously)
    // - Pan/zoom inertia animation in progress
    // - Crosshair visible (continuous tracking)
    // - Probe active from external source (need to render feedback)
    self.interaction.needs_continuous_frames()
        || self.has_live_data_source()
        || self.has_active_external_probe()
}
```

For truly independent frame rates, the Plot can spawn its own render
thread that loops at 120fps, writes to the back texture, and swaps.
The Halcyon widget just displays whatever texture is current.

---

# 7. PlotInteraction

Interaction state and handlers, carried forward from the original chart renderer's
`InteractionState`, adapted to `Widget::handle_event`.

```rust
pub struct PlotInteraction {
    /// Current mouse position in local widget coords.
    mouse_pos: Option<(f32, f32)>,

    /// Crosshair world coordinates (computed from mouse_pos + Transform).
    crosshair_world: Option<(f64, f64)>,

    /// Active interaction mode.
    mode: InteractionMode,

    /// Pan state.
    drag_anchor: Option<(f32, f32)>,
    drag_view_anchor: Option<(f64, f64, f64, f64)>,

    /// Box selection state.
    selection_session: Option<SelectionSession>,

    /// Selected objects.
    selection: Vec<HitObject>,
}

pub enum InteractionMode {
    Navigate,      // pan/zoom/crosshair (default)
    SelectSingle,  // click to toggle individual objects
    SelectBox,     // drag to define a selection region
    Annotate,      // ML tagging, drawing, labeling
}
```

**INV-296-IA1:** Interaction modifies ViewState (and AxisAuthority if
shared). Never modifies Transform directly. Transform is derived.

**INV-296-IA2:** Box selection uses a session model (preview → commit).
Mouse release does NOT commit. Enter commits, Escape cancels. Prevents
accidental selection loss.

**INV-296-IA3:** Interaction modes are explicit (UI button or hotkey).
No modifier-key modes. Accidental modifier release cannot destroy
selection.

**INV-296-IA4:** On pointer move, the Plot writes to its shared Probe
(if one is configured) with the current world coordinates and its own
WidgetInternalId as source. On pointer leave, the Plot clears the Probe.

## 7.0.1 SPEC-HAL-343 Spatial Policy Boundary

Plot does not delegate chart interaction authority to `SpatialInteractionPolicy`.
Pan, zoom, crosshair, probe, explicit interaction modes, box selection, series
drag, axes, and chart tools remain Plot authority because they mutate
`ViewState`, `AxisAuthority`, Plot-local interaction state, or tool output.
Plot may consume shared spatial projection, hit-region, visual, route, and
overlay placement policies for non-axis annotations and HUD placement, but the
spatial layer never executes chart mutations.

## 7.1 Selection Output Model

Plot provides two selection primitives. Neither stores "selected"
state on GPU data. Both emit pure coordinate data to the consumer.

### 7.1.1 Range Selection

A drag gesture that captures a world-coordinate rectangle.

```rust
/// Emitted when the user completes a range selection.
pub struct RangeSelection {
    /// World-space bounds of the selected region.
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}
```

The consumer decides meaning: ML training window, backtest period,
annotation region, data export range, price alert zone, session
marker, or anything else. Plot draws the visual overlay (same box
rendering as box zoom). On commit, Plot emits the `RangeSelection`
and clears the visual.

### 7.1.2 Object Selection

A click or proximity hit on a specific data point within a Series.

```rust
/// Emitted when the user clicks on or near a data point.
pub struct ObjectHit {
    /// Index of the series within the Plot's series list.
    pub series_index: usize,
    /// Series-specific hit data. CandleSeries returns candle index.
    /// LineSeries returns point index + distance. Consumer downcasts.
    pub hit: Box<dyn std::any::Any>,
    /// World coordinates of the hit point.
    pub world_x: f64,
    pub world_y: f64,
}
```

The `hit_test` method on Series already returns `Option<Box<dyn Any>>`.
Plot iterates series in reverse z-order (topmost first), hit-tests
each, and emits the first hit as an `ObjectHit`.

### 7.1.3 Multi-Selection

Shift+click accumulates `ObjectHit` entries into a selection set.
Click without Shift replaces the set. The consumer reads the current
selection set and feeds visual changes back through `body_color` /
`border_color` at `RenderCandle` construction time.

**INV-296-SEL1:** Plot never stores "selected" state on GPU
primitives. Selection is consumer-owned state. The consumer decides
what "selected" looks like by setting colors when building instances.

**INV-296-SEL2:** Range and object selection are separate modes.
A range selection does not select individual objects. An object
click does not create a range. The consumer can combine them in
its own state if desired.

## 7.2 Series Interaction — Draggable Data

Series can opt into direct manipulation. The consumer marks a series
as interactive. The user grabs a point (or the line between points)
and drags it. Plot emits the new world coordinates. The consumer
decides what to do with them.

### 7.2.1 Interaction Contract

```rust
/// Opt-in interaction capability for a Series.
pub enum SeriesInteraction {
    /// No interaction — data is display-only (default).
    None,

    /// Points can be clicked/selected but not moved.
    Selectable,

    /// Points can be grabbed and dragged to new positions.
    /// Plot emits (series_index, point_index, new_world_x, new_world_y).
    Draggable,
}
```

Series declares its interaction level. Plot checks this before
routing drag events to a series vs panning the viewport.

### 7.2.2 Drag Event Flow

```
User grabs a point on LineSeries (MA line)
    | Plot::handle_event detects Down on a hit-testable, Draggable series
    | Plot enters SeriesDrag mode (NOT pan mode)
    |
User drags to new position
    | Plot::handle_event Move events
    | Plot computes new world coordinates via Transform
    | Plot emits SeriesDragEvent { series_index, point_index, new_x, new_y }
    |
Consumer receives event
    | Consumer recomputes: "find the MA period that passes through this Y at this X"
    | Consumer updates its LineSeries data
    | Next frame: LineSeries::prepare() rebuilds geometry from updated data
    |
User releases
    | Plot exits SeriesDrag mode
    | Plot emits SeriesDragEnd
```

### 7.2.3 Drag Disambiguation

Left-drag on background -> pan viewport (existing behavior).
Left-drag on a Draggable series point -> drag the point.

Disambiguation: Plot hit-tests all series on Down. If a Draggable
series is hit, enter SeriesDrag mode. If no hit, enter pan mode.
This is determined on Down — once the mode is set, Move events
follow that mode until Up.

### 7.2.4 Consumer Use Cases

**Manual MA fitting:** User drags the MA line to a target position.
Consumer solves: "what period makes the MA pass through price P at
time T?" The MA recalculates and the line snaps to the new fit.

**Multi-MA comparison:** User selects multiple MA lines (Shift+click),
drags the group. Consumer adjusts all periods proportionally.

**Support/resistance drawing:** User drags a horizontal line to a
price level. Consumer stores it as a persistent annotation.

**Order placement:** User drags a marker to a price. Consumer creates
a limit order at that price.

### 7.2.5 Separation Principle

**INV-296-DRAG1:** Plot provides drag coordinates. The consumer owns
the math. Plot never interprets what a drag "means" — it doesn't
know what an MA is, what a Fibonacci level is, or what an order is.
It knows "the user moved point 42 from (x0,y0) to (x1,y1)."

**INV-296-DRAG2:** Plot never mutates Series data directly during
drag. It emits events. The consumer mutates its data model and
feeds updated data to the Series. This preserves unidirectional
data flow (Constitution Pillar D).

**INV-296-DRAG3:** Visual feedback during drag is immediate. Plot
can show a "ghost" position (dotted line or semi-transparent point
at the drag target) without modifying the Series data. The actual
data update happens after the consumer processes the event.

## 7.3 Selection Output API

The consumer receives selection and drag events through callbacks
or polling — the same pattern as box zoom and interaction today.

```rust
/// Selection/interaction event emitted by Plot.
pub enum PlotSelectionEvent {
    /// Range selected (drag-select completed).
    RangeSelected(RangeSelection),

    /// Object clicked/selected.
    ObjectSelected(ObjectHit),

    /// Object deselected (click on empty area, or Escape).
    SelectionCleared,

    /// Series point drag in progress.
    SeriesDrag {
        series_index: usize,
        point_index: usize,
        world_x: f64,
        world_y: f64,
    },

    /// Series point drag completed.
    SeriesDragEnd {
        series_index: usize,
        point_index: usize,
        world_x: f64,
        world_y: f64,
    },
}
```

Polling API:

```rust
impl Plot {
    /// Get pending selection events since last call.
    /// Events are consumed (drained) on read.
    pub fn take_selection_events(&mut self) -> Vec<PlotSelectionEvent>;

    /// Get the current selection set (object selections).
    pub fn selection(&self) -> &[ObjectHit];

    /// Get the current range selection (if active).
    pub fn range_selection(&self) -> Option<&RangeSelection>;

    /// Clear all selections.
    pub fn clear_selection(&mut self);
}
```

Callback API (builder pattern):

```rust
Plot::new()
    .on_selection(|event: &PlotSelectionEvent| {
        match event {
            PlotSelectionEvent::ObjectSelected(hit) => { ... }
            PlotSelectionEvent::SeriesDrag { .. } => { ... }
            _ => {}
        }
    })
```

**INV-296-SEL3:** Selection events are consumed via polling
(`take_selection_events`) or callbacks. They are never stored
across frames. Constitutional — ephemeral event data (Pillar B).

**INV-296-SEL4:** The callback, if set, fires during
`Widget::handle_event` — the constitutional mutation phase.
Not during layout or paint.

---

# 8. World-Anchored Annotations

Annotations are interactive elements positioned in world coordinates.
The Plot converts world coords to screen coords each frame using the
current Transform, then positions the annotation widget accordingly.

```rust
pub struct WorldAnchor {
    pub x: f64,    // world X (time or other)
    pub y: f64,    // world Y (price or other)
}

impl Plot {
    /// Add a world-anchored child widget.
    /// The Plot positions it at the screen location corresponding
    /// to the anchor's world coordinates each frame.
    pub fn add_annotation(
        &mut self,
        widget: Box<dyn Widget>,
        anchor: WorldAnchor,
    );
}
```

Annotations are regular Halcyon widgets — they can have tooltips, handle
clicks, contain buttons. The Plot just positions them. They render above
the offscreen texture (higher z-layer).

Examples: order markers at (time, price), Fibonacci level handles,
horizontal price alerts with cancel buttons, trade entry/exit markers.

**INV-296-AN1:** Annotations that scroll out of the viewport are hidden
(not painted, not hit-testable). The Plot manages visibility based on
the current Transform.

**INV-296-AN2:** Annotations are screen-anchored after Transform. They
do not participate in the offscreen texture rendering. They are regular
widgets in Halcyon's scene graph.

---

# 9. Stacked Projections

A common layout: price chart on top, volume below, RSI below that.
All sharing the same X axis.

```rust
/// A vertical stack of Plots sharing an AxisAuthority on X.
/// Manages subplot sizing, shared crosshair, and unified time axis.
pub struct PlotStack {
    plots: Vec<Plot>,
    x_authority: Arc<Mutex<AxisAuthority>>,
    /// Shared probe across all plots in the stack.
    probe: Arc<Mutex<Probe>>,
    /// Height ratios (e.g. [0.6, 0.25, 0.15] for price/volume/RSI).
    ratios: Vec<f32>,
    /// Resize handles between plots.
    dividers: Vec<f32>,
}
```

`PlotStack` is a convenience container. It's NOT required. You can
achieve the same result with a Flex column of individual Plots sharing
an AxisAuthority and Probe. PlotStack just handles the divider resize
handles and unified crosshair.

**INV-296-PS1:** Crosshair vertical line is shared across all plots in
a stack (same X position, via shared Probe). Horizontal line is per-plot
(each has its own Y axis).

**INV-296-PS2:** PlotStack is a Widget. It owns its child Plots.
It implements layout by distributing vertical space according to ratios.

---

# 10. Cross-Widget Coordinate Probing

The Probe enables interaction feedback across any widgets that share
world-coordinate space — not just Plots. Any widget that can interpret
world coordinates can read a Probe and render feedback.

## 10.1 Probe Flow

```
User hovers Order Book at price level 105.2
    ↓
OrderBook Plot writes Probe { x: None, y: Some(105.2), source: ob.id }
    ↓
Chart Plot reads Probe on next frame
    ↓ source != self.id → render horizontal highlight line at y=105.2
    ↓ (chart already renders its own crosshair separately)
    ↓
Volume Plot reads Probe on next frame
    ↓ y is not meaningful for volume → ignores y
    ↓ x is None → no vertical highlight
    ↓
Position Widget reads Probe
    ↓ highlights open orders near price 105.2
```

```
User hovers Chart at (time T, price P)
    ↓
Chart Plot writes Probe { x: Some(T), y: Some(P), source: chart.id }
    ↓
OrderBook Plot reads Probe
    ↓ source != self.id → highlights nearest level to P
    ↓
Indicator Panel reads Probe
    ↓ highlights indicator value at time T
    ↓
Alert Widget reads Probe
    ↓ shows proximity to nearest alert at price P
```

## 10.2 Non-Plot Consumers

The Probe is not limited to Plot widgets. Any Halcyon widget can read
a shared Probe and react:

- **Position manager widget** — highlights rows matching the probed price
- **Alert list widget** — highlights alerts near the probed price
- **Trade entry form** — pre-fills price from the probed value
- **Indicator panel** — shows value at the probed time
- **Order ticket** — snaps to the probed price level

These are regular widgets that hold `Arc<Mutex<Probe>>` and read it
during paint. No special infrastructure. No event subscriptions. Just
polling pure data.

## 10.3 Multiple Probes

Different probe instances can exist for different purposes:

- A **primary probe** shared across the main chart and order book
  (hover-driven, cleared on pointer leave)
- A **selection probe** written by a selection tool (persists until
  explicitly cleared)
- A **data probe** written by an external signal (e.g., "latest trade
  at this price")

Each is an independent `Arc<Mutex<Probe>>`. Widgets choose which probes
to read by construction.

---

# 11. Shared Primitives Summary

Three shared primitives, all the same constitutional pattern:

| Primitive | What it synchronizes | Example |
|-----------|---------------------|---------|
| `AxisAuthority` | Coordinate ranges (pan/zoom) | Chart + order book share Y (price); chart + volume share X (time) |
| `Probe` | Cursor position across widgets | Hover chart → order book highlights level; hover order book → chart shows line |
| `Arc<Vec<T>>` | External data (positions, alerts, orders) | Position manager publishes open orders → chart renders markers |

All same shape: `Arc<Mutex<T>>` where T is pure data. Any widget reads.
Any widget writes. No cross-widget references. No callbacks. No event
subscriptions. Constitutional (Pillar C: ID-based identity, Pillar D:
pure data in, explicit actions out).

---

# 12. Data Flow

```
Trading Engine (external consumer)
    ↓ produces Candle objects (mutable, engine-side)
    ↓
SnapshotBuilder (engine-side, dedicated thread)
    ↓ converts Candle → RenderCandle (f64 prices, i64 μs timestamps)
    ↓ packages into immutable FrameSnapshot
    ↓ publishes via Arc/ArcSwap (zero-lock reads)
    ↓
Plot Widget (Halcyon-side)
    ↓ reads latest FrameSnapshot (pull model, never blocks)
    ↓ CandleSeries receives FrameSnapshot
    ↓ GeometryBuilder builds CandleInstance[] with viewport culling
    ↓ Transform maps f64 world coords → f32 screen pixels
    ↓ GPU: instanced draw to offscreen texture
    ↓
Halcyon Frame
    ↓ Plot::paint() → push_image(offscreen_texture)
    ↓ Composited with axis labels, probe feedback, annotations, other widgets
    ↓
Screen
```

**INV-296-DF1:** SnapshotBuilder lives engine-side. Renderer never calls
engine. Renderer reads snapshots atomically.

**INV-296-DF2:** FrameSnapshot.candles MUST be sorted by ascending X
(t_open for time axis). This is a strict invariant. Selection by index
depends on stable ordering.

**INV-296-DF3:** f64 prices until after Transform. No f32 downcast
before normalization. This prevents collapse on PEPE (0.00000001) and
jitter on BTC (100,000).

---

# 13. Precision Guarantees

These rules are carried forward from the original chart renderer's design. They are
non-negotiable.

**INV-296-PR1:** All price/value coordinates are f64 in ViewState,
Transform, RenderCandle, and FrameSnapshot. f32 only in GPU vertex data
(after Transform).

**INV-296-PR2:** Transform normalizes relative distances:
`(value - min) / (max - min)`. Never draws absolute values.
PEPE at 0.00000002 with min=0.00000001 normalizes to 0.5 — middle of
screen. No collapse.

**INV-296-PR3:** Viewport padding is proportional to span:
`min -= span * 0.05; max += span * 0.05`. Never fixed-value padding.

**INV-296-PR4:** Candles carry raw prices, never rounded. Rounding
destroys molecular-scale assets.

**INV-296-PR5:** Clipping occurs after Transform, in screen space.
Never clip in world space (kills tiny prices).

**INV-296-PR6:** Screen space (f32 pixels) is fine after normalization.
GPU receives f32 — correct for all assets because coordinates are
already normalized.

**INV-296-PR7:** Default axis tick labels derive decimal precision from the
actual tick interval, not from the absolute magnitude of the value. Adjacent
large prices remain textually distinct when the visible step is fractional.
Explicit consumer formatters remain authoritative.

---

# 14. GPU Pipeline (CandleSeries Specifics)

CandleSeries uses the proven original chart renderer pipeline:

**candle_body.wgsl** — instanced rendering with in-shader borders.

- Vertex shader: generates quad from `CandleInstance` data (body_rect).
  Converts screen pixels to NDC via viewport uniform.
- Fragment shader: renders body color, static border, hover border,
  selection outline — all computed from UV distance to edges.
  No CPU geometry needed for borders.

**overlay.wgsl** — non-instanced rendering for wicks, crosshair,
box selection, overlay lines.

- Simple passthrough: pixel coords → NDC, vertex color → output.

**CandleInstance** (64 bytes, bytemuck::Pod):
```rust
pub struct CandleInstance {
    pub body_rect: [f32; 4],    // x1, y1, x2, y2 screen pixels
    pub wick_x: f32,            // wick center x in pixels
    pub wick_top: f32,          // wick top y in pixels
    pub wick_bottom: f32,       // wick bottom y in pixels
    pub body_color: [f32; 4],   // candle body RGBA
    pub border_color: [f32; 4], // border RGBA (alpha 0 = no border)
    pub border_px: f32,         // border width in pixels (0 = no border)
}
```

CandleInstance is pure rendering geometry. Selection state (highlight,
outline) is handled by the overlay system — Plot draws selection rects
as separate vertices in the crosshair/overlay pipeline, not as per-instance
GPU attributes. This keeps the instance struct minimal and the shader
simple (5 vertex attributes, 5 varyings, 2-branch fragment shader).

**Draw order within the render pass:**

1. Grid lines (overlay pipeline)
2. Series in z_order (bars → candles → lines)
3. Selection highlight (overlay pipeline, cyan fill + border)
4. Synced crosshair (overlay pipeline, from shared Probe)
5. Local crosshair (overlay pipeline)
6. Box selection rectangle (overlay pipeline)

**INV-296-GPU1:** Viewport culling on CPU before GPU upload. Only
candles within the viewport's X range are sent to the GPU.

**INV-296-GPU2:** Buffer growth uses 1.5× factor to minimize
reallocations. Persistent buffers reused across frames.

**INV-296-GPU3:** MAX_VISIBLE_CANDLES = 2,000,000. Beyond this,
downsample by step.

---

# 15. Chart Synchronization and OrderBook Integration

## 15.1 Shared Time Axis (Multiple Charts)

Multiple Plots sharing an X-axis `AxisAuthority`:

```rust
let time_axis = Arc::new(Mutex::new(AxisAuthority::new(t_min, t_max)));

// Price chart (main panel)
let price_plot = Plot::new()
    .x_axis(Axis::time())
    .y_axis(Axis::linear())
    .with_shared_x(time_axis.clone())
    .add_series(CandleSeries::new(btc_snapshot))
    .add_series(LineSeries::new("SMA 20", sma_data));

// Volume chart (below price, same panel)
let volume_plot = Plot::new()
    .x_axis(Axis::time())
    .y_axis(Axis::linear())
    .with_shared_x(time_axis.clone())
    .add_series(BarSeries::new(volume_data));

// ETH chart (completely different panel/tab/window)
let eth_plot = Plot::new()
    .x_axis(Axis::time())
    .y_axis(Axis::linear())
    .with_shared_x(time_axis.clone())  // same authority!
    .add_series(CandleSeries::new(eth_snapshot));
```

Pan on BTC → `AxisAuthority::pan()` → all three plots pick up the new
X range → all re-render. Price axes are independent. Only time is linked.

## 15.2 Optional Market-Detail Alignment (Chart + OrderBook)

Plot/order-book integration is optional. A standalone `Plot` and a standalone
`OrderBook` retain independent coordinate and interaction authority.

When a consumer explicitly creates a `MarketDetailLink`, the candlestick Plot
owns the visible price domain. The aligned order-book ladder reads the last
presented snapshot of that Plot-owned Y-axis authority, not worker target state
that has not reached the screen. Both surfaces derive their ephemeral vertical
transform from the same presented range, axis kind, and viewport height, so
identical prices map to identical Y positions. The link also carries a shared
`Probe`; it is pure data and contains no widget references or callbacks.

A consumer may additionally nominate one existing Plot `PriceLevel` as the
linked market-detail reference. Plot remains the sole owner of that level and
renders and publishes its authoritative target price, theme-or-explicit color,
and line style as pure presentation data. The nominated reference bypasses
ordinary `PriceLevel` easing so it shares the live candle's update timeline
instead of lagging behind it. After the worker completes and swaps the render
target internally, Plot's UI-thread presentation latch exposes the new front
texture, presented Y-domain, and linked reference together before sibling paint.
No part of that frame is published independently or ahead of the visible Plot.
`AlignedOrderBook` paints that same semantic line at the shared price coordinate
through its right edge. No reference is published or painted unless the consumer
opts in.

Sibling update order cannot split aligned row geometry from that presentation
boundary. If Plot publishes a completed frame after `AlignedOrderBook` has already
run its update for the application frame, the ladder detects the newer presented
axis generation during paint and performs one bounded projection from its existing
exact source index. Its row Y positions, clipping summaries, and reference therefore
use the same presented Plot transform in that paint. The next update adopts the
same projection into the normal cache. This fallback does not interpolate book
prices or alter the independently smoothed horizontal depth scale.

```rust
let market_detail = MarketDetailLink::new(p_min, p_max);

// Candlestick chart: owns visible time and price domains.
let chart = Plot::new()
    .x_axis(Axis::time())
    .with_market_detail_link(market_detail.clone())
    .with_market_detail_reference_level(LAST_PRICE_LEVEL_ID)
    .with_price_level(last_price_level)
    .add_series(CandleSeries::new(candle_source));

// Order book: reads price from the Plot link; quantity scaling stays local.
let orderbook = AlignedOrderBook::new(market_detail)
    .with_book(bids, asks);
```

Pan price on the chart (vertical pan) → `price_axis` authority updates →
order book picks up the new Y range → both scroll together. The chart's
X axis (time) and the order book's horizontal depth scale are completely
independent. Horizontal time pan/zoom can affect the ladder only if Plot
auto-ranging intentionally changes the visible Y domain.

The linked composition does not change Plot navigation gestures or install an
X-axis authority. Primary-drag on Plot background pans time only and preserves
the normal Y auto-range policy. Space+primary-drag performs free X/Y pan and
publishes the resulting Plot-owned Y range to the aligned ladder.

Wheel input over `AlignedOrderBook` emits a bounded, coalesced vertical
price-pan intent through `MarketDetailLink`. The intent preserves the wheel's
logical-pixel displacement relative to the ladder viewport. Plot consumes it
in `update()`, mutates its own `ViewState`, disables Y auto-range, and publishes
the resulting price domain through the ordinary Plot-owned presentation
boundary. Because this gesture changes only the Y domain, it preserves the
independent X-axis `follow_latest` latch. The ladder never writes
`AxisAuthority`. Consumers may disable this gesture with
`AlignedOrderBook::with_wheel_price_pan(false)`, in which case the ladder does
not claim wheel routing.

Ordinary aligned-book hover remains local: the projected row under the pointer
uses the active theme's row-hover affordance and emits exact
`LevelHoverEnter`/`LevelHoverLeave` events, but does not write the shared
`Probe` or draw a horizontal guide. Empty price intervals do not resolve to a
distant nearest level. A consumer that explicitly wants cross-widget hover
reflection wires those events to a Probe or PriceLevel.

Primary click resolves the nearest exact visible source level on the clicked
bid/ask side, records a presentation-only `AlignedOrderBookSelection { side,
price }`, and still emits `LevelClicked` for application meaning. Selection
highlights the corresponding visible grouped row while retaining exact source
identity. If that exact level disappears, selection does not attach to a
nearby price. Empty-price or tick-grid selection is not inferred; it requires
explicit consumer price-grid authority outside this composition.

Levels outside the visible price domain are clipped. The aligned ladder reports
bounded aggregate quantity/count summaries above and below the viewport.
Aligned mode projects source levels into deterministic grouped-price buckets
according to the public `MarketDetailGrouping` policy. The default
`AutoPixel` policy is collision protection only: its interval is exactly the
configured minimum logical-pixel height in the active Plot axis transform,
whose default is the ten-logical-pixel minimum at which an aligned row can
carry its price and quantity labels. It never rounds that interval upward for
round-number aesthetics and never infers a product tick size. The grouping
floor and the label-bearing paint threshold therefore cannot diverge into
permanently unlabeled hairlines as the Plot zoom changes.

`DecimalPlaces(n)` is explicit semantic grouping on a globally stable decimal
price grid: source prices that round to `n` decimal places share a bin.
`PriceStep(MarketDetailPriceStep)` supplies the corresponding positive finite
price interval directly. If either semantic interval would be smaller than the
configured pixel floor, projection merges the minimum integer number of
adjacent semantic bins required to satisfy that floor; it does not substitute
a nice number. Price-label formatting remains separately consumer-controlled
through `OrderBookConfig`.

Without a nominated linked reference, automatic bucket boundaries remain
anchored to global multiples in the active Plot axis coordinate. With a
reference, `AutoPixel` anchors that collision grid to the completed presented
reference price. Semantic grids remain globally anchored; if their one
intersecting bin straddles the reference, that bin alone is split at the exact
reference. The reference and grouping grid are captured from one
`MarketDetailLink` presentation snapshot, so the nearest grouped rows
terminate at the line instead of straddling it. A changed reference generation
intentionally changes one bounded projection; an unchanged generation remains
a no-op.

Each projected row exposes one operator-facing grouped price at its
axis-coordinate midpoint and renders that single formatted price. Its lower
and upper aggregation bounds remain typed projection evidence for internal
grouping and exact-source interaction; they are not displayed as a range.
Source order or quantity cannot move a group while its interval and anchor are
unchanged. Zoom intentionally regroups when the exact automatic collision
interval or minimum semantic-bin merge count changes, and a linked reference
change intentionally changes the reference-adjacent boundary.
This prevents sparse near-mid snapshots from producing overlapping rows while
keeping the ladder visually attached to the shared market reference. Source
`BookSide` data is never rewritten, and hover/click resolves the nearest exact
source level.
The horizontal depth scale is separately smoothed from a robust visible-depth
target so one transient outlier does not rescale the price coordinate or cause
frame-to-frame flicker. Its default exponential attack/release rates are `64/3`
and `16/3`; consumer-provided `MarketDetailConfig` rates remain authoritative.

Projected bucket quantity labels show the already-aggregated bucket quantity.
The source-level count remains projection evidence and exact-interaction
metadata; it is not appended to the visible quantity as a multiplicity.

## 15.3 Full Trading Layout Example

```rust
let time_axis = Arc::new(Mutex::new(AxisAuthority::new(t_min, t_max)));
let price_axis = Arc::new(Mutex::new(AxisAuthority::new(p_min, p_max)));
let probe = Arc::new(Mutex::new(Probe::new()));

// Main chart (SplitPane left, top)
let chart = Plot::new()
    .with_shared_x(time_axis.clone())
    .with_shared_y(price_axis.clone())
    .with_probe(probe.clone())
    .add_series(CandleSeries::new(candle_source))
    .add_series(HeatmapSeries::new(heatmap_source))
    .add_series(LineSeries::new("SMA 20", sma_source))
    .add_series(RectSeries::new(session_backgrounds));

// Volume panel (SplitPane left, bottom — PlotStack with chart)
let volume = Plot::new()
    .with_shared_x(time_axis.clone())
    .with_probe(probe.clone())
    .add_series(BarSeries::new(volume_source));

// Order book (SplitPane right)
let orderbook = Plot::new()
    .with_shared_y(price_axis.clone())
    .with_probe(probe.clone())
    .add_series(DepthBarSeries::new(depth_source));

// All three: chart + volume share time. Chart + orderbook share price.
// All three share probe. Pan anywhere, everything stays synchronized.
// Hover anywhere, everything shows feedback.
```

Plots in different containers (SplitPane, TabBar, separate windows)
synchronize purely through the shared Arc data. No widget relationships.
No cross-widget references. Constitutional.

---

# 16. Builder API

```rust
Plot::new()
    .x_axis(Axis::time())
    .y_axis(Axis::linear().with_formatter(price_format))
    .with_shared_x(time_authority.clone())
    .with_shared_y(price_authority.clone())
    .with_probe(probe.clone())
    .with_view_state(ViewState::new(t_min, t_max, p_min, p_max))
    .add_series(CandleSeries::new(snapshot_source))
    .add_series(LineSeries::new("SMA 20", sma_source))
    .add_series(HeatmapSeries::new(heatmap_source))
    .add_annotation(OrderMarker::new(order), WorldAnchor { x: t, y: price })
    .with_crosshair(true)
    .with_grid(true)
    .on_selection(|objects| { ... })
```

---

# 17. What Halcyon Needs From SPEC-HAL-295

Plot uses SPEC-HAL-295's `CustomRenderPass` for GPU pipeline ownership.
Specifically, the offscreen texture variant:

- Plot registers its offscreen texture via `register_image_texture()`
- Plot creates its own `CommandEncoder`, runs its series' render passes
  on the offscreen texture, submits via `Arc<Queue>`
- Plot's Widget::paint() calls `push_image(texture_id, rect)`
- Halcyon composites the image quad with the rest of the UI

This means SPEC-HAL-295 needs the offscreen texture model, NOT the
shared-render-pass model. The SPEC should be revised accordingly before
implementation.

---

# 18. Invariants

**INV-296-1:** Plot is a Widget. It participates in layout, paint, and
event routing through standard Widget lifecycle.

**INV-296-2:** Plot renders to an offscreen texture. Halcyon's main
render pass never sees candle pipelines, instance buffers, or custom
shaders. It sees an image quad.

**INV-296-3:** Series are pluggable via trait. Adding a new visualization
type does not modify Plot, Transform, ViewState, or interaction.

**INV-296-4:** AxisAuthority is pure data shared via Arc. No widget
references. No callbacks. Constitutional (Pillar C, Pillar D).

**INV-296-5:** f64 precision until after Transform. No f32 for world
coordinates. Non-negotiable.

**INV-296-6:** Interaction modifies ViewState. Transform is derived.
Never mutate Transform directly.

**INV-296-7:** Annotations are standard Halcyon widgets positioned by
the Plot's coordinate system. They render above the offscreen texture,
not inside it.

**INV-296-8:** Plot can render at an independent frame rate from
Halcyon's UI. The offscreen texture model decouples the two.

**INV-296-9:** Probe is pure data shared via Arc. No widget references
beyond source ID for self-suppression. No callbacks. Constitutional.

**INV-296-10:** AxisAuthority supports sharing on either axis
independently. X and Y authorities are separate instances. A Plot can
share X only, Y only, both, or neither.

**INV-296-11:** Market-detail alignment is explicit and optional. The Plot is
the sole writer of the linked visible price domain; an aligned order book is a
read-only projection of that domain. Standalone Plot and OrderBook behavior is
unchanged.

**INV-296-12:** Linked price projection and horizontal depth scaling are
separate authorities. Time-axis changes do not directly invalidate or rescale
book depth.

**INV-296-13:** Re-publishing an unchanged linked axis range is a no-op. Rapid
update/zoom cycles must not create an invalidation or layout feedback loop.

**INV-296-14:** Aligned-ladder wheel navigation is pure-data intent. Plot is
the sole consumer and sole writer of the resulting Y domain. Pending input is
bounded and coalesced to O(1) state per link.

**INV-296-15:** Ordinary aligned-ladder hover does not automatically publish a
shared Probe. Cross-widget hover reflection requires explicit consumer wiring.

**INV-296-16:** Aligned-ladder selection stores exact source `(side, price)`
identity only. It never infers a market tick, synthesizes a price, or silently
reattaches to a neighboring source level.

**INV-296-17:** Empty intervals in the authoritative visible book remain empty
price-coordinate space. Aligned projection does not stretch rows or fill gaps
merely to occupy the viewport.

**INV-296-18:** Default aligned grouping is exact pixel-collision protection,
not numeric beautification. Explicit decimal-place or price-step grouping uses
a global semantic price grid and may merge only the minimum adjacent bins
needed for the configured pixel floor. A linked reference remains an exact
boundary without transferring price-grid authority to Halcyon.

**INV-296-19:** Plot label paint may request exact text metrics, but unchanged
frames with the same font identity, size, and scale reuse the text engine's
bounded metrics cache. Plot paint must not reconstruct a native font per label
or per frame.

**INV-296-SEL1:** Plot never stores "selected" state on GPU primitives.
Selection is consumer-owned. Visual appearance of selected objects is
controlled by the consumer through body_color/border_color.

**INV-296-SEL2:** Range and object selection are separate modes.

**INV-296-SEL3:** Selection events are ephemeral — consumed via
polling or callbacks, never stored across frames.

**INV-296-SEL4:** Selection callbacks fire during handle_event
(mutation phase), never during layout or paint.

**INV-296-DRAG1:** Plot provides drag coordinates. Consumer owns
the math. Plot never interprets drag semantics.

**INV-296-DRAG2:** Plot never mutates Series data during drag.
Events are emitted; consumer mutates its own data model.

**INV-296-DRAG3:** Visual drag feedback (ghost position) does not
modify Series data. It is overlay-only.

---

# 19. Acceptance Criteria

1. A CandleSeries renders 1M candles at 120fps in the offscreen texture
2. Plot is correctly clipped to its layout rect
3. Other widgets (buttons, labels) render correctly around the Plot
4. candle_body.wgsl runs with instanced rendering (in-shader borders)
5. Wicks, crosshair, overlays render via overlay.wgsl
6. Axis labels render via Halcyon's text engine (PaintSink)
7. Pan/zoom works through Widget::handle_event → ViewState mutation
8. Two Plots sharing X-axis AxisAuthority synchronize pan/zoom
9. Chart + OrderBook sharing Y-axis AxisAuthority synchronize price
10. Hover chart → OrderBook highlights level via shared Probe
11. Consumer-wired OrderBook hover can project through a shared Probe or PriceLevel
12. World-anchored annotations move with pan/zoom
13. Annotations outside viewport are hidden
14. PlotStack distributes vertical space and shares crosshair
15. HeatmapSeries renders colored cells under candles
16. DepthBarSeries renders horizontal bid/ask bars
17. PEPE (0.00000001) renders correctly — no collapse, no jitter
18. BTC (100,000) renders correctly — no precision loss
19. No performance regression for windows without Plots
20. Non-Plot widgets can read Probe and render feedback
21. Click on a candle selects it (ObjectHit with candle index)
22. Shift+click accumulates multiple candle selections
23. Drag-select a region emits RangeSelection with world coordinates
24. Draggable LineSeries point can be grabbed and repositioned
25. Drag on background pans (not confused with series drag)
26. Consumer receives PlotSelectionEvent during handle_event phase
27. Selection visual appearance is controlled by consumer colors
28. Escape clears all selections
29. Explicit market-detail composition maps identical prices to identical Y pixels
30. Vertical Plot pan/zoom updates the aligned ladder without ladder-owned Y mutation
31. Horizontal Plot zoom leaves the ladder unchanged unless Plot auto-range changes Y
32. Dense visible book levels produce bounded pixel buckets with exact source hover
33. Offscreen liquidity is represented by bounded above/below summaries
34. Repeated unchanged axis publication does not advance generation or trigger repaint
35. A nominated linked reference is a grouped-row boundary, and each grouped row
    displays one price rather than its internal aggregation range
36. Fractional large-price axis ticks remain distinct under the default formatter
37. Wheel input over an enabled aligned ladder pans only the Plot-owned Y domain
38. Ordinary aligned hover emits exact events without publishing a shared Probe
39. Aligned click selection retains exact source identity without tick inference
40. Missing visible liquidity remains an honest price-coordinate gap
41. Default grouping does not enlarge collision buckets for round-number aesthetics
42. Explicit decimal-place and fixed-price-step grouping retain stable semantic bins,
    with only minimum integer merging for pixel safety
43. Repeated unchanged Plot paint resolves one native metrics entry per exact font
    identity, size, and scale rather than recreating fonts per label

---

# 20. Non-Goals (Future Work)

- 3D visualization (depth heatmap as Z → color is in-scope; true 3D is not)
- Compute shaders for data processing
- WebGL/WebGPU target (desktop-first; possible later)
- Built-in technical analysis calculations (convenience builders in SPEC-302; engine provides raw data)
- Drag-to-reorder series z-order within a Plot (z_order is set by consumer, not interactive)
- Real-time ticker/streaming text overlay (use annotation widgets)
- Log scale (deferred — linear first, per the original chart renderer's precision rules)
- Order book rendering (→ SPEC-300)
- Heatmap/depth visualization (→ SPEC-301)

---

# 21. Consistency Matrix (SPEC-HAL-002A Required)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. Plot is a Widget. Series owned by Plot. |
| B — Phase Separation | Preserved. prepare/render follow compile/execute phases. |
| C — ID-based Identity | Preserved. AxisAuthority and Probe are pure data via Arc, no widget refs. |
| D — Pure Data | Preserved. ViewState, Transform, FrameSnapshot, Probe are pure data. |
| E — GPU-First | Strengthened. Instanced GPU rendering for domain data. |
| F — Sovereignty | Preserved. Plot renders to its own texture. Halcyon orchestrates display. |

No permanent invariants weakened.

---

# 22. Threaded Plot Rendering

## 22.1 Problem

`render_to_texture()` runs on the main thread during `Widget::update()`.
For 500K candles, geometry building alone takes measurable time. With
4 charts (quad demo), this is 4× the cost per frame. With 20+ small
charts in a dashboard, the main thread is dominated by chart rendering
while UI responsiveness suffers.

## 22.2 Architecture: Plot/PlotRenderer Split

The widget splits into two halves:

**Plot (main thread)** — the Widget. Handles events, updates ViewState,
paints the image quad, manages selection state, runs hit-testing.
Lightweight. Never blocks.

**PlotRenderer (worker thread)** — the GPU producer. Owns Series,
pipelines, offscreen textures. Receives render commands. Produces
completed frames. Never touches Widget state.

```
Main thread                          Worker thread
┌─────────────┐                      ┌──────────────────┐
│    Plot      │     RenderCmd       │  PlotRenderer    │
│  (Widget)    │────────────────────→│                  │
│              │                     │  Series[]        │
│  ViewState   │     texture_id      │  Pipelines       │
│  Selection   │←────────────────────│  Textures (×2)   │
│  Events      │   (atomic swap)     │  Overlay buffers │
│  HitIndex    │                     │                  │
└─────────────┘                      └──────────────────┘
```

## 22.3 Render Command

Plot sends a snapshot each frame. Cheap — no allocations, no GPU state.

```rust
/// Everything the worker needs to produce one frame.
/// All values are POD or pre-computed. No references.
struct RenderCmd {
    /// Viewport state (5 f64 values).
    view_state: ViewState,

    /// Texture dimensions (physical pixels).
    width: u32,
    height: u32,

    /// Crosshair position in physical pixels. None = hidden.
    crosshair: Option<(f32, f32)>,

    /// Box selection in physical pixels. None = hidden.
    box_selection: Option<(f32, f32, f32, f32)>,

    /// Pre-computed selection highlight rects in physical pixels.
    /// Computed on main thread from selection_rect() calls.
    /// Worker just draws these — no hit data needed.
    selection_rects: Vec<[f32; 4]>,

    /// Generation counter. Worker skips if same as last rendered.
    generation: u64,
}
```

## 22.4 Double-Buffered Textures

Two offscreen textures per PlotRenderer: front and back.

- Worker renders to back texture, then swaps (atomic index)
- Main thread always reads front texture via `push_image(front_id)`
- No tearing, no synchronization stalls
- One frame of latency (acceptable — 120fps = 8ms)

```rust
struct DoubleBuffer {
    textures: [wgpu::Texture; 2],
    texture_ids: [u64; 2],
    /// 0 or 1. Swapped atomically after worker completes a frame.
    front: AtomicUsize,
}

impl DoubleBuffer {
    fn front_id(&self) -> u64 {
        self.texture_ids[self.front.load(Ordering::Acquire)]
    }
    fn back_texture(&self) -> &wgpu::Texture {
        let back = 1 - self.front.load(Ordering::Acquire);
        &self.textures[back]
    }
    fn swap(&self) {
        let old = self.front.load(Ordering::Acquire);
        self.front.store(1 - old, Ordering::Release);
    }
}
```

Both textures registered with Halcyon's renderer at init time.
Plot::paint() uses `front_id()` which may alternate between frames.

## 22.5 Series Data Sharing

Series data (candles, points, bars) must be readable on the main
thread for hit-testing, data_y_range, and objects_in_range, while
also readable on the worker for geometry building.

**Solution: Read-only data snapshot on main thread.**

The consumer calls `plot.set_candles(data)` which:
1. Updates a local `Arc<Vec<RenderCandle>>` on Plot (for hit-testing)
2. Sends the new data to the worker via channel

The worker owns the actual Series objects. It receives data updates
and calls `series.set_candles()` on its copy. No shared mutable
state. No RwLock contention.

Hit-testing on the main thread uses the local Arc snapshot — same
data, no GPU state needed. CandleSeries::hit_test is pure coordinate
math over the candle Vec.

```rust
impl Plot {
    /// Consumer API: update candle data.
    /// Updates local hit-test snapshot AND sends to worker.
    pub fn set_candles(&mut self, series_index: usize, candles: Vec<RenderCandle>) {
        // Local snapshot for hit-testing
        self.hit_data[series_index] = Arc::new(candles.clone());
        // Send to worker
        self.render_tx.send(WorkerMsg::UpdateData {
            series_index,
            data: SeriesData::Candles(candles),
        }).ok();
        // Bump generation so worker re-renders
        self.generation += 1;
    }
}
```

## 22.6 PlotRenderPool

Shared across all Plots in the application. Owns the worker threads.

```rust
pub struct PlotRenderPool {
    workers: Vec<JoinHandle<()>>,
    job_tx: crossbeam::channel::Sender<RenderJob>,
}

struct RenderJob {
    renderer_id: u64,
    cmd: RenderCmd,
    /// Channel to signal completion (texture swapped).
    done_tx: oneshot::Sender<()>,
}
```

**Pool sizing:**

```rust
fn pool_size(plot_count: usize) -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    // Reserve 2 cores: main thread + GPU driver
    let available = cpus.saturating_sub(2).max(1);
    // Cap at 4 workers — diminishing returns beyond this
    available.min(4).min(plot_count)
}
```

| Plots | CPUs | Pool size |
|-------|------|-----------|
| 1     | 8    | 1         |
| 4     | 8    | 4         |
| 20    | 8    | 4         |
| 4     | 4    | 2         |
| 1     | 2    | 1         |

Workers use work-stealing: any worker can render any Plot. No affinity.
This naturally balances load when some charts are idle (off-screen,
unchanged data, occluded by other UI).

## 22.7 Dirty Tracking

Workers skip rendering when nothing changed. Each Plot maintains
a generation counter incremented on:

- ViewState change (pan, zoom, axis authority sync)
- Data update (set_candles, set_points)
- Selection change (object_selection modified)
- Crosshair movement
- Resize

Worker compares its last-rendered generation against the command's
generation. If equal, skip. This means idle charts consume zero
worker time — critical for 20+ chart dashboards where most charts
are static at any given moment.

## 22.8 Lifecycle

```
1. Plot::new()
   → PlotRenderer created but not started
   → No worker thread yet

2. Plot::init_gpu()
   → PlotRenderer initializes pipelines, double-buffer textures
   → Registers both textures with Halcyon renderer
   → PlotRenderer moved to pool (spawns or reuses worker)

3. Plot::update() (every frame)
   → Build RenderCmd from current state
   → Send to worker via channel (non-blocking)
   → Worker picks up, renders, swaps

4. Plot::paint() (every frame)
   → push_image(double_buffer.front_id())
   → May show previous frame if worker hasn't finished — acceptable

5. Plot dropped
   → Worker notified via channel close
   → Worker cleans up GPU resources
   → Thread returns to pool
```

## 22.9 Hit-Testing Without Series Access

Plot keeps lightweight hit-test data on the main thread:

```rust
/// Per-series hit-test data. Kept in sync with worker's Series.
enum HitTestData {
    Candles(Arc<Vec<RenderCandle>>),
    Lines(Arc<Vec<LinePoint>>),
    Bars(Arc<Vec<BarPoint>>),
    None, // Non-interactive series
}
```

Hit-test methods (hit_test, objects_in_range, data_y_range,
selection_rect) operate on this local data. No cross-thread
communication. No latency. Same algorithms as current Series
implementations, just operating on the snapshot.

## 22.10 Migration Path

The threading architecture is additive — it doesn't break the
current synchronous path.

**Phase 1: PlotRenderer extraction.** Move all GPU state and
render_to_texture logic into a separate struct. Plot holds a
PlotRenderer by value. Everything still runs on main thread.
This is pure refactoring — no threading yet.

**Phase 2: Channel + worker.** PlotRenderer moves to a worker
thread. Plot communicates via channels. Double-buffer textures.
Hit-test data snapshots.

**Phase 3: Pool.** Shared PlotRenderPool. Dynamic sizing. Dirty
tracking. Work stealing.

Each phase is independently testable. Phase 1 has zero behavioral
change. Phase 2 adds one frame of latency. Phase 3 is optimization.

## 22.11 Invariants

**INV-296-THR1:** Plot::paint() never blocks on worker completion.
It reads the latest completed texture. If the worker hasn't finished,
the previous frame is displayed.

**INV-296-THR2:** Hit-testing operates on main-thread data snapshots.
No cross-thread calls during handle_event.

**INV-296-THR3:** Series data updates are ordered. If the consumer
calls set_candles() twice, the worker sees both updates in order.

**INV-296-THR4:** Worker threads do not touch Widget state,
WidgetTree, AppState, or any Halcyon-owned structures. They operate
exclusively on PlotRenderer-owned GPU resources.

**INV-296-THR5:** Pool size is bounded. Never more than
`min(cpu_count - 2, 4)` worker threads regardless of Plot count.

**INV-296-THR6:** Idle Plots (unchanged generation) consume zero
worker time. Only dirty Plots are rendered.

## 22.12 Acceptance Criteria

29. render_to_texture logic fully contained in PlotRenderer struct
30. PlotRenderer is Send (can be moved to worker thread)
31. Double-buffered textures show no tearing or visual glitch
32. Hit-testing works identically to synchronous path
33. 4-chart quad demo runs with render pool
34. 20-chart dashboard demo maintains 60fps UI responsiveness
35. CPU utilization shows work distributed across cores
36. Idle charts show zero worker CPU usage
37. Data updates (set_candles) propagate to worker correctly
38. Plot drop cleans up worker resources without leak

## 22.13 Phase 3: PlotRenderPool

Shared thread pool across all Plots. `min(cpus - 2, 4)` workers
consume from one MPMC queue (crossbeam_channel::unbounded). Any
worker renders any Plot.

```rust
pub struct PlotRenderPool {
    job_tx: crossbeam_channel::Sender<RenderJob>,
    _workers: Vec<std::thread::JoinHandle<()>>,
}
```

Consumer API:

```rust
let pool = Arc::new(PlotRenderPool::new());
let plot = Plot::new()
    .with_render_pool(pool.clone())
    // ...
```

If no pool is provided, Plot auto-creates one. Shared pools
are recommended for dashboard layouts (20 charts, 4 workers).

## 22.14 Generation-Based Stale Skipping

Each render job carries a submission generation and the renderer series
generation from which its overlay parameters were built. Workers skip a job when
its submission is not newer than the last completed render or when shared series
data advanced after those parameters were captured. A worker therefore cannot
combine a newer candle series with older PriceLevel or market-detail overlays.

## 22.15 Batch-Drain Worker Loop

Workers drain ALL pending jobs on wake, deduplicate by renderer
identity (`Arc::as_ptr`), and render only the latest generation
per unique renderer. During fast pan with 20 charts, stale jobs
are filtered in microseconds without touching any mutex.

## 22.16 Crossbeam MPMC

`std::sync::mpsc` replaced with `crossbeam_channel::unbounded()`.
Enables the shared pool architecture (multiple workers consuming
from one queue).

## 22.17 Resize Bypass

Resize bypasses the pool — locks the renderer directly on the
main thread. Texture registration must be synchronous.

## 22.18 Invariants (Phase 3)

**INV-296-POOL1:** Pool workers never hold a renderer lock while
receiving from the channel. Drain first, lock second.

**INV-296-POOL2:** Batch-drain deduplication uses Arc pointer
identity. No semantic comparison of RenderParams.

**INV-296-POOL3:** Pool auto-creates if not provided. Consumer
is not required to manage pool lifecycle.

**INV-296-POOL4:** A completed Plot frame uses series data and overlay parameters
from the same series generation. Its front texture registration, rendered
ViewState, presented market-detail Y-domain, and linked reference are committed
together by Plot's UI-thread presentation latch.

**INV-296-POOL5:** While a completed Plot frame awaits UI-thread presentation,
the worker cannot render another frame into either member of its double buffer.

## 22.19 Acceptance Criteria (Phase 3)

48. PlotRenderPool distributes work across N workers
49. 20-chart dashboard runs with 4 pool workers (not 20 threads)
50. Batch-drain skips stale jobs during fast pan
51. Generation counter prevents rendering outdated frames
52. Resize works correctly through pool bypass
53. A live candle and its nominated PriceLevel cannot render from different data generations
54. The linked market-detail reference and Y-domain publish in the same application frame as their matching Plot front texture
55. A presentation-pending Plot frame cannot be overwritten or independently exposed

---

# 23. Live Data Updates

## 23.1 Problem

Series data is set once at construction via `set_candles()`,
`set_points()`, etc. Trading applications need to update the
working candle on every price tick, append new candles when
periods close, and update indicator values continuously.

## 23.2 DataUpdate Channel

Plot provides a thread-safe channel for live data updates.
The consumer holds a `Sender<DataUpdate>`, background threads
push updates, and Plot drains the receiver in `Widget::update()`.

```rust
pub enum DataUpdate {
    UpdateLastCandle { series_index: usize, candle: RenderCandle },
    AppendCandle { series_index: usize, candle: RenderCandle },
    ReplaceCandles { series_index: usize, candles: Vec<RenderCandle> },
    UpdateLastLinePoint { series_index: usize, point: LinePoint },
    AppendLinePoint { series_index: usize, point: LinePoint },
    ReplaceLinePoints { series_index: usize, points: Vec<LinePoint> },
    UpdateLastBarPoint { series_index: usize, point: BarPoint },
    ReplaceBarPoints { series_index: usize, points: Vec<BarPoint> },
}
```

Consumer API:

```rust
let (plot, data_tx) = Plot::new()
    .with_gpu_resources(gpu)
    .with_data_channel();

// Background thread pushes updates:
data_tx.send(DataUpdate::UpdateLastCandle {
    series_index: 1,
    candle: updated_candle,
}).unwrap();
```

## 23.3 Update Path

`Widget::update()` drains the channel via `try_recv()`. Each
update flows to both the main-thread hit proxy AND the worker's
Series (via `Arc<Mutex<PlotRenderer>>` lock):

1. Hit proxy updated (main thread — for hit-testing, auto_scale_y)
2. Renderer's Series updated (lock, swap data, unlock)
3. Render generation bumped → next pool job renders new data

## 23.4 Efficient Single-Element Updates

`update_last()` and `append_data()` on the Series trait enable
single-element operations without full Vec clones. The working
candle is the last element in the Vec — no separate struct.

## 23.5 Auto-Follow

`ViewState.follow_latest: bool` — when true, the viewport X
range retains its current span and scrolls to keep the latest candle at a
stable live edge with 5% of the span reserved as right-side breathing room.

`with_data_channel()` arms `follow_latest` by default, including when the
consumer supplied a `ViewState` whose ordinary non-live default is `false`.
`with_initial_follow_latest(false)` is the explicit construction-time opt-out
for a live plot that must initially preserve a historical/manual X range. The
override is order-independent with `with_view_state()` and
`with_data_channel()`.

Disabled automatically by any manual X-domain change (time pan, normal
pointer-scroll zoom, keyboard zoom, space+scroll, or box zoom). Y-only price
pan/zoom does not disable the independent time-follow latch. The ordinary
`PointerEventKind::Scroll` zoom path clears follow mode before later
`DataUpdate` channel drains can apply auto-follow.

A completed horizontal pointer pan re-arms follow only when it deliberately
returns the latest candle to the live-edge band: the current right-side gap must
be within 5% of the canonical 5% right padding, meaning the latest candle
occupies the final 10% of the viewport. Re-arming snaps to the exact canonical
padding before the next live update, preventing a delayed jump. A pointer pan
that leaves the latest candle outside that bounded band remains manual.
Space+scroll remains a discrete manual command and does not self-cancel by
re-arming on the same tick. Explicit reset
(`handle_reset_view()` through double-click, double-Space, or Home key) also
re-arms and uses the same placement.

## 23.6 Soft Right Bound

When live data arrives, `data_bounds.x_max` is dynamically set
to `latest_x + span * 0.33`, allowing the user to peek 1/3
viewport ahead of the latest data without scrolling into
empty space.

## 23.7 Invariants

**INV-296-LIVE1:** DataUpdate channel is crossbeam unbounded.
Consumer can send from any thread.

**INV-296-LIVE2:** Plot drains all pending updates in
Widget::update() before rendering. No partial application.

**INV-296-LIVE3:** A live channel arms auto-follow by default unless the
consumer explicitly opts out with `with_initial_follow_latest(false)`.
Construction order does not change that decision.

**INV-296-LIVE4:** Auto-follow is disabled by every manual X-domain change,
including ordinary pointer-scroll zoom, before a later live-channel update can
move the viewport. Y-only price navigation preserves it. A completed horizontal
pointer pan may re-arm only inside the bounded live-edge band; otherwise
re-arming requires explicit reset (double-click, double-Space, or Home key).

**INV-296-LIVE5:** The working candle is the last element in
the Vec. No separate working candle struct.

## 23.8 Acceptance Criteria

39. DataUpdate channel receives updates from background thread
40. UpdateLastCandle updates both hit proxy and renderer Series
41. AppendCandle grows both hit proxy and renderer Series
42. Auto-follow scrolls to latest candle each frame
43. Pan disables auto-follow
44. Zoom disables auto-follow
45. Double-click re-enables auto-follow
46. Home key re-enables auto-follow
47. Soft right bound prevents scrolling past data + 33%
48. Live-channel construction arms auto-follow independent of builder order
49. Explicit initial follow opt-out preserves a consumer-provided historical view
50. Y-only aligned-book price navigation preserves X auto-follow
51. A completed time-pointer pan re-arms only when the latest candle returns to the live-edge band

---

# 24. Hit Proxy System

## 24.1 Problem

With threaded rendering, the main thread no longer has direct
access to Series data (owned by PlotRenderer on the worker).
Hit-testing, selection rects, auto_scale_y, and objects_in_range
are pure coordinate math that must run on the main thread.

## 24.2 Architecture

Each Series produces a lightweight data snapshot (HitProxy) at
init time. Plot stores these as `Vec<HitTestEntry>` on the main
thread. All interaction queries go through hit entries, never
through the renderer.

```rust
pub enum HitProxy {
    Candle { candles: Vec<RenderCandle> },
    Line { points: Vec<LinePoint> },
    Bar { points: Vec<BarPoint> },
}

pub struct SeriesMeta {
    pub interaction: SeriesInteraction,
    pub z_order: u32,
    pub contributes_to_y_autoscale: bool,
}

pub struct HitTestEntry {
    pub proxy: HitProxy,
    pub meta: SeriesMeta,
}
```

HitTestEntry provides: `hit_test()`, `selection_rect()`,
`objects_in_range()`, `data_y_range()` — all pure coordinate
math, no GPU types.

## 24.3 Series Trait Method

```rust
fn hit_proxy(&self) -> HitProxy;
```

Each Series type implements this by cloning its data Vec. Called
once at init, updated on live data changes.

## 24.4 Invariants

**INV-296-HP1:** Hit proxies live on the main thread. They are
never sent to or accessed by the worker thread.

**INV-296-HP2:** Hit proxy data is updated atomically with
renderer Series data on every DataUpdate.

**INV-296-HP3:** All handle_event interaction (hit-test,
selection, drag) goes through hit_entries, never through the
renderer.

---

# 25. Keyboard Interaction

## 25.1 Capture Mode

Plot implements `capture_eligible() → true`. Clicking a Plot
enters Capture mode (orange border). In Capture mode, Plot
owns all keyboard input.

## 25.2 Two-Tier Escape

`on_capture_escape()` implements:
1. If selection exists → clear it, stay in Capture (return true)
2. If no selection → return false (framework releases Capture)

## 25.3 Keyboard Zoom

`=` or `+` zooms in (factor 1.2 at viewport center).
`-` or `_` zooms out (factor 0.8 at viewport center).

Handled via `Event::Text(TextInputEvent)` because character
keys arrive as TextInput on macOS, not KeyDown.

## 25.4 Space+Scroll Pan

`space_held: bool` tracked on Plot. Space+scroll converts
scroll delta_y into horizontal pan (5% of X range per tick)
instead of zoom.

## 25.5 Volume Toggle

`v` key toggles visibility of BarSeries at z_order 50.
Hidden series skipped in both prepare() and render() via
`RenderParams.hidden_series: HashSet<usize>`.

## 25.6 View Reset

Home key and double-click call `handle_reset_view()` — scrolls
to the latest data with the ordinary live-edge padding and re-enables
auto-follow. Double-Space
(within 300ms) also triggers reset.

## 25.7 Invariants

**INV-296-KB1:** All keyboard shortcuts fire in Capture mode
only. In Navigation mode, keys go to the framework (Tab, arrows,
Escape for focus management).

**INV-296-KB2:** Any manual X-domain change (time pan, X/Y zoom, box zoom,
keyboard zoom, space+scroll) sets `follow_latest = false`. Y-only price
navigation does not. Completion of a horizontal pointer pan may re-arm only
through the bounded live-edge rule in §23.5.

**INV-296-KB3:** `space_held` is reset on `on_capture_leave`
to prevent stuck state.

---

# 26. Theme Authority

Plot-owned visual defaults resolve from the active Halcyon `Theme` during
`Widget::update()`. The theme owns the background/plot surface, grid, axes and
labels, crosshairs, selection and tool affordances, annotation defaults, and the
default colors of built-in candle, bar/volume, line, marker, area, histogram,
and depth series.

`Plot::with_color_scheme`, explicit series styles, per-point colors, candle
body overrides, explicit annotation colors, and custom tool overlay colors
remain consumer authority. A runtime theme switch dirties the widget tree,
updates theme-bound Plot colors before layout, increments Plot render
generation, and submits the new resolved palette to the offscreen renderer.

**INV-296-THEME1:** A Plot without an explicit color scheme follows the active
Halcyon theme and preserves dark/light semantic parity.

**INV-296-THEME2:** Explicit consumer color/style overrides are never replaced
by a theme switch.

**INV-296-THEME3:** Theme changes reach renderer-owned built-in Series before
their next `prepare()` call and cause the Plot offscreen texture to be
regenerated.

**INV-296-THEME4:** Tool-produced persistent `AnnotationData` remains
consumer-owned under SPEC-HAL-297. Plot themes the built-in gesture preview;
the consumer themes or explicitly styles the persisted projection it creates.
