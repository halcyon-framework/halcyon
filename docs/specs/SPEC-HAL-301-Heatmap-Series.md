# SPEC-HAL-301: **Heatmap Series**

### *Grid-Based Color-Mapped Data Visualization for Plot*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-296 (Plot — Coordinate-Authority Rendering System)

---

# 1. Purpose

This SPEC defines HeatmapSeries — a Series type that renders a 2D
grid of colored cells where X and Y are spatial coordinates and
color encodes a third dimension (Z value). Used for liquidity
heatmaps, order flow footprint charts, volume profiles over time,
and any visualization where a continuous value field maps to color.

---

# 2. Problem

Candle, line, and bar series represent 1D data (one value per X
position). Trading applications need 2D data surfaces: liquidity
at each (time, price) cell, order flow imbalance at each
(time, price) bucket, or volume traded at each price level over
time. These require a dense grid of cells, each with an independent
color derived from a scalar value.

---

# 3. HeatmapSeries

```rust
/// Color mapping function: maps a normalized [0, 1] value to RGBA.
pub type ColorMap = Box<dyn Fn(f32) -> [f32; 4] + Send + Sync>;

/// Grid data for heatmap rendering.
#[derive(Debug, Clone)]
pub struct HeatmapData {
    /// Grid values in row-major order. values[row * cols + col].
    /// NaN = transparent (no data).
    pub values: Vec<f32>,
    /// Number of columns (X divisions).
    pub cols: usize,
    /// Number of rows (Y divisions).
    pub rows: usize,
    /// World X range covered by the grid.
    pub x_min: f64,
    pub x_max: f64,
    /// World Y range covered by the grid.
    pub y_min: f64,
    pub y_max: f64,
    /// Value range for normalization. Values outside this range
    /// are clamped. Consumer sets this to control contrast.
    pub v_min: f32,
    pub v_max: f32,
}

pub struct HeatmapSeries {
    data: Option<HeatmapData>,
    color_map: ColorMap,
    z_order: u32,
    opacity: f32,
    // GPU state: instanced quads or texture
}
```

## 3.1 Rendering Approaches

Two viable approaches:

**Instanced quads (simpler, chosen for v1):**
Each grid cell is one instanced quad with per-instance color.
For a 500×200 grid, that's 100K instances — well within GPU budget.
Uses the existing overlay pipeline with Vertex { position, color }.
No new shaders needed.

**Texture-based (future, for very large grids):**
Upload values as a 2D texture, colormap as a 1D texture. Fragment
shader samples both. Handles 4K×4K grids efficiently. Requires a
new shader.

## 3.2 Built-in Color Maps

```rust
impl HeatmapSeries {
    /// Black → Blue → Cyan → Green → Yellow → Red → White
    pub fn colormap_thermal() -> ColorMap;

    /// Black → Purple → Red → Orange → Yellow → White
    pub fn colormap_magma() -> ColorMap;

    /// Transparent → Blue → White (for liquidity)
    pub fn colormap_liquidity() -> ColorMap;

    /// Green → White → Red (for imbalance/divergence)
    pub fn colormap_diverging() -> ColorMap;

    /// Custom color map from a closure.
    pub fn colormap_custom(f: impl Fn(f32) -> [f32; 4] + Send + Sync + 'static) -> ColorMap;
}
```

## 3.3 Consumer API

```rust
let heatmap = HeatmapSeries::new()
    .with_colormap(HeatmapSeries::colormap_thermal())
    .with_z_order(5)   // behind candles
    .with_opacity(0.6);

let plot = Plot::new()
    .with_gpu_resources(gpu)
    .add_series(Box::new(heatmap))
    .with_candles(candles);

// Live updates — replace entire grid per time slice:
data_tx.send(DataUpdate::HeatmapGrid {
    series_index: 0,
    data: current_heatmap,
}).unwrap();
```

## 3.4 Convenience Builder

```rust
impl Plot {
    /// Add a heatmap layer with a given colormap.
    pub fn with_heatmap(self, colormap: ColorMap) -> Self;
}
```

---

# 4. Footprint Chart Specialization

A footprint chart is a heatmap where each cell shows bid/ask
volume imbalance. The consumer structures this as HeatmapData
where:
- Each row = one price tick
- Each column = one time bar (candle period)
- Value = bid_volume - ask_volume (positive = buying, negative = selling)
- ColorMap = diverging (green for buying, red for selling)

The HeatmapSeries doesn't know about footprints. The consumer
maps domain data to HeatmapData.

---

# 5. DataUpdate Extension

```rust
pub enum DataUpdate {
    // ... existing variants ...

    /// Replace the entire heatmap grid.
    HeatmapGrid {
        series_index: usize,
        data: HeatmapData,
    },

    /// Append a column to the right edge (streaming mode).
    /// Values length must equal data.rows.
    HeatmapAppendColumn {
        series_index: usize,
        x_min: f64,
        x_max: f64,
        values: Vec<f32>,
    },
}
```

---

# 6. Hit-Test

Heatmap hit-test returns the cell coordinates and value:

```rust
pub struct HeatmapHit {
    pub col: usize,
    pub row: usize,
    pub x: f64,
    pub y: f64,
    pub value: f32,
}
```

The consumer can use this to show tooltip content (e.g., "Price
101.50, Time 14:32, Volume 1,234").

---

# 7. Invariants

**INV-301-1:** HeatmapData is owned by HeatmapSeries. Consumer
replaces atomically. No cell-level partial updates (column
append is the streaming granularity).

**INV-301-2:** NaN values render as transparent (no quad emitted).
This allows sparse grids without wasting GPU bandwidth.

**INV-301-3:** Color normalization uses consumer-provided v_min/v_max.
The Series does not auto-range. Consumer controls contrast.

**INV-301-4:** HeatmapSeries z_order defaults to 5 (behind candles
at 100, behind lines at 10-30). Consumer can override.

---

# 8. Acceptance Criteria

1. HeatmapSeries renders colored grid cells in the overlay pass
2. Cells pan/zoom correctly with the chart
3. NaN cells are transparent
4. Built-in colormaps produce correct gradients
5. Custom colormap works via closure
6. Live updates via DataUpdate::HeatmapGrid
7. Column append mode works for streaming
8. Hit-test returns cell coordinates and value
9. Opacity control works
10. Combined with CandleSeries (heatmap behind candles)

---

# 9. Non-Goals

- 3D surface rendering (2D color grid only)
- Automatic time aggregation (consumer provides pre-bucketed data)
- Built-in footprint logic (consumer maps domain → HeatmapData)
- Cell text labels (use PlotTag for specific annotations)
- Bilinear interpolation between cells (pixel-exact cells only)
