# SPEC-HAL-302: **Indicator Series Types**

### *AreaSeries, HistogramSeries, and Indicator Convenience Builders*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-296 (Plot — Coordinate-Authority Rendering System)

---

# 1. Purpose

This SPEC defines additional Series types needed for technical
indicator visualization: filled areas between two lines (Bollinger
Bands, Ichimoku Cloud, channels), positive/negative histogram bars
(MACD histogram, momentum), and convenience builders that compose
multiple Series into complete indicator overlays.

---

# 2. Problem

Current Series types (Candle, Line, Bar) cover candlesticks, moving
averages, and volume. They don't cover:

- **Filled bands** — Bollinger Bands need a shaded area between
  upper and lower lines. Ichimoku Cloud needs two filled regions
  with color-coded direction. Channel overlays need upper/lower
  boundary fills.

- **Centered histograms** — MACD histogram has positive bars
  (green, above zero) and negative bars (red, below zero). The
  existing BarSeries has a fixed baseline and doesn't color-code
  by sign.

- **Composed indicators** — MACD requires 3 sub-series (histogram
    + signal line + MACD line). Bollinger requires 3 (upper line +
      lower line + filled area). RSI requires 1 line + 2 horizontal
      reference lines (30, 70). These compositions are boilerplate.

---

# 3. AreaSeries

Fills the region between two Y-value curves at each X point. Used
for bands, channels, clouds.

```rust
/// A pair of Y values at a given X coordinate.
#[derive(Debug, Clone, Copy)]
pub struct AreaPoint {
    pub x: f64,
    pub y_upper: f64,
    pub y_lower: f64,
}

pub struct AreaStyle {
    /// Fill color RGBA. Typically semi-transparent.
    pub fill_color: [f32; 4],
    /// Optional upper boundary line color. None = no line.
    pub upper_line_color: Option<[f32; 4]>,
    /// Optional lower boundary line color. None = no line.
    pub lower_line_color: Option<[f32; 4]>,
    /// Boundary line width in pixels.
    pub line_width: f32,
}

pub struct AreaSeries {
    points: Vec<AreaPoint>,
    style: AreaStyle,
    z_order: u32,
    // GPU state
}
```

## 3.1 Rendering

The fill is rendered as a triangle strip between upper and lower
curves. For each consecutive pair of X points, four vertices form
two triangles:

```
upper[i] ─── upper[i+1]
    │  ╲          │
    │    ╲        │
lower[i] ─── lower[i+1]
```

The fill uses a solid color (semi-transparent). Boundary lines
are optional LineSeries-style quads rendered on top.

Z-order defaults to 5 (behind candles, behind EMAs). The fill
should be a background element.

## 3.2 Consumer API

```rust
let bb = AreaSeries::new()
    .with_style(AreaStyle {
        fill_color: [0.3, 0.5, 1.0, 0.15],
        upper_line_color: Some([0.3, 0.5, 1.0, 0.4]),
        lower_line_color: Some([0.3, 0.5, 1.0, 0.4]),
        line_width: 1.0,
    })
    .with_z_order(5);

// Consumer computes upper/lower from domain logic:
let bb_points: Vec<AreaPoint> = candles.iter().map(|c| {
    AreaPoint {
        x: (c.x_open + c.x_close) / 2.0,
        y_upper: sma + 2.0 * stddev,
        y_lower: sma - 2.0 * stddev,
    }
}).collect();

let plot = Plot::new()
    .add_series(Box::new(bb))
    .with_candles(candles);
```

---

# 4. HistogramSeries

Vertical bars centered on a zero line, colored by sign. Used for
MACD histogram, momentum oscillators, volume delta.

```rust
/// A single histogram bar.
#[derive(Debug, Clone, Copy)]
pub struct HistogramPoint {
    pub x: f64,
    pub y: f64,  // Positive = above zero, negative = below
}

pub struct HistogramStyle {
    /// Color for positive bars (above zero).
    pub positive_color: [f32; 4],
    /// Color for negative bars (below zero).
    pub negative_color: [f32; 4],
    /// Bar width as fraction of interval. Default 0.8.
    pub width_fraction: f32,
}

pub struct HistogramSeries {
    points: Vec<HistogramPoint>,
    style: HistogramStyle,
    z_order: u32,
    // GPU state
}
```

## 4.1 Rendering

Similar to BarSeries but with baseline always at 0.0 and
color determined by sign. Positive bars grow upward from zero,
negative bars grow downward.

---

# 5. Convenience Builders

These compose multiple Series into complete indicators. They
live on `Plot` as builder methods.

```rust
impl Plot {
    /// Bollinger Bands: filled area + middle SMA line.
    /// `period` = SMA lookback, `deviations` = band width
    /// (typically 2.0).
    pub fn with_bollinger_bands(
        self,
        candles: &[RenderCandle],
        period: usize,
        deviations: f64,
        fill_color: [f32; 4],
    ) -> Self;

    /// MACD: histogram + signal + MACD line.
    /// `fast` = fast EMA period (typically 12),
    /// `slow` = slow EMA period (typically 26),
    /// `signal` = signal EMA period (typically 9).
    ///
    /// NOTE: MACD is usually in a SEPARATE subplot (different Y
    /// axis). This builder adds the 3 series — the consumer is
    /// responsible for putting them in a separate Plot with
    /// appropriate Y range.
    pub fn with_macd(
        self,
        candles: &[RenderCandle],
        fast: usize,
        slow: usize,
        signal: usize,
    ) -> Self;

    /// RSI: single line + reference lines at 30 and 70.
    /// Like MACD, usually in a separate subplot with Y range [0, 100].
    pub fn with_rsi(
        self,
        candles: &[RenderCandle],
        period: usize,
        color: [f32; 4],
    ) -> Self;

    /// Ichimoku Cloud: two filled areas (Senkou Span A/B) +
    /// Tenkan-sen, Kijun-sen, Chikou Span lines.
    pub fn with_ichimoku(
        self,
        candles: &[RenderCandle],
    ) -> Self;
}
```

## 5.1 Design Note: Indicators vs. Series

The convenience builders compute indicator values from candle data
and create the appropriate Series. The computation is simple
(EMA, SMA, stddev) and runs once at build time. Live updates
recompute via DataUpdate — the consumer sends new candle data and
the builder-created series update accordingly.

This is deliberate: Plot is a rendering system, not an analytics
engine. The convenience builders are sugar for common patterns.
Complex indicators (custom strategies, machine learning signals)
are computed externally and fed as LineSeries/BarSeries/AreaSeries
data.

---

# 6. DataUpdate Extensions

```rust
pub enum DataUpdate {
    // ... existing variants ...

    /// Replace all area points.
    ReplaceAreaPoints {
        series_index: usize,
        points: Vec<AreaPoint>,
    },
    /// Update the last area point.
    UpdateLastAreaPoint {
        series_index: usize,
        point: AreaPoint,
    },
    /// Replace all histogram points.
    ReplaceHistogramPoints {
        series_index: usize,
        points: Vec<HistogramPoint>,
    },
}
```

---

# 7. Invariants

**INV-302-1:** AreaSeries fill is rendered as a triangle strip in
the overlay pipeline. No new shaders for v1.

**INV-302-2:** HistogramSeries baseline is always 0.0. Consumer
offsets data if a different baseline is needed.

**INV-302-3:** Convenience builders compute indicator values at
build time. They do not create persistent computation pipelines.
Live recomputation is the consumer's responsibility.

**INV-302-4:** MACD, RSI, and other oscillators are typically in
separate subplots. The convenience builders add Series — the
consumer manages Plot layout.

---

# 8. Acceptance Criteria

1. AreaSeries renders filled region between two curves
2. Area fill pans/zooms correctly
3. Optional boundary lines render on top of fill
4. HistogramSeries renders positive bars above zero, negative below
5. Histogram colors match sign (positive = green, negative = red)
6. with_bollinger_bands produces correct fill + middle line
7. with_macd produces histogram + signal + MACD line
8. with_rsi produces line + reference levels at 30 and 70
9. Live updates via DataUpdate work for Area and Histogram
10. Combined rendering (area behind candles) layers correctly

---

# 9. Non-Goals

- Built-in indicator computation engine (consumer computes)
- Strategy backtesting visualization (consumer builds with Series)
- Real-time streaming indicator recomputation (consumer manages)
- Stochastic, Williams %R, etc. — these are just LineSeries in
  a bounded Plot; no new Series type needed
