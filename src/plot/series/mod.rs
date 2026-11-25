// SPEC-HAL-296: Series Trait
//
// Pluggable data visualization within a Plot. Implementations own their
// GPU pipelines and buffers. The Plot calls prepare() then render().

pub mod area;
pub mod bar;
pub mod candle;
pub mod circle;
pub mod depth;
pub mod histogram;
pub mod line;

pub use area::{AreaPoint, AreaSeries, AreaStyle};
pub use bar::{BarPoint, BarSeries, BarStyle};
pub use candle::{CandleSeries, RenderCandle};
pub use circle::{Circle, CircleSeries, CircleStyle};
pub use depth::{
    BookEvent, BookLevel, BookSide, BookSideKind, ClassificationStyle, DepthBarSeries,
    DepthBarStyle, LevelClassification,
};
pub use histogram::{HistogramPoint, HistogramSeries, HistogramStyle};
pub use line::{LinePoint, LineSeries, LineStyle};

/// Data payload for live series updates.
pub enum SeriesData {
    Candles(Vec<candle::RenderCandle>),
    Lines(Vec<line::LinePoint>),
    Bars(Vec<bar::BarPoint>),
    Circles(Vec<circle::Circle>),
    BookUpdate(BookUpdateKind),
}

/// Order book update types.
pub enum BookUpdateKind {
    Bids(depth::BookSide),
    Asks(depth::BookSide),
    Snapshot {
        bids: depth::BookSide,
        asks: depth::BookSide,
    },
}

use super::PlotColorScheme;
use super::transform::Transform;

/// SPEC-HAL-296 §7.2: Opt-in interaction capability for a Series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesInteraction {
    /// No interaction — data is display-only (default).
    None,
    /// Points can be clicked/selected but not moved.
    Selectable,
    /// Points can be selected AND dragged to new positions.
    /// Plot emits SeriesDrag events. Consumer owns the math.
    Draggable,
}

/// SPEC-HAL-296: Rectangle in physical pixels (post-scale-factor).
///
/// Used for viewport uniforms, GPU coordinate mapping, and offscreen
/// texture sizing. Distinct from RectF (logical pixels).
#[derive(Debug, Clone, Copy)]
pub struct PhysicalRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// SPEC-HAL-296: A data visualization within a Plot.
///
/// Implementations produce GPU geometry from their data model, using
/// the Plot's Transform for coordinate mapping.
///
/// INV-296-S1: Series are rendered in z_order within the render pass.
/// INV-296-S2: Series own their GPU pipelines and buffers.
pub trait Series: Send + Sync {
    /// One-time initialization. Create pipelines, bind group layouts,
    /// and persistent buffers.
    ///
    /// Called once when the series is added to a Plot.
    fn init(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat);

    /// Per-frame preparation. Build geometry, upload buffers, update uniforms.
    ///
    /// Called during the compile phase with Device/Queue access.
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
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, viewport: PhysicalRect);

    /// Z-order within the Plot's render pass.
    /// Lower values render first (behind higher values).
    fn z_order(&self) -> u32;

    /// Resolve theme-managed default colors before preparation.
    ///
    /// Explicit series styles and per-point colors remain authoritative.
    fn apply_color_scheme(&mut self, _scheme: &PlotColorScheme) {}

    /// Hit-test a screen-space point against this series' data.
    /// Returns an opaque hit result for the series to interpret.
    fn hit_test(&self, x: f32, y: f32, transform: &Transform) -> Option<Box<dyn std::any::Any>>;

    /// Query the Y range of visible data for a given X range.
    ///
    /// Used for auto-scale Y. Returns (y_min, y_max) of data that
    /// overlaps the given X range. Returns None if no data in range.
    fn data_y_range(&self, x_min: f64, x_max: f64) -> Option<(f64, f64)> {
        let _ = (x_min, x_max);
        None
    }

    /// Whether this series contributes to auto-scale Y.
    ///
    /// Default: true. Set to false for overlay series (e.g. volume bars
    /// behind candles) that live in a different value domain and should
    /// not influence the Y axis range.
    fn contributes_to_y_autoscale(&self) -> bool {
        true
    }

    /// SPEC-HAL-296 §7.2: Declare this series' interaction capability.
    /// Default: None (display-only).
    fn interaction(&self) -> SeriesInteraction {
        SeriesInteraction::None
    }

    /// Return the screen-space rect [x1, y1, x2, y2] of a selected
    /// object, given its hit data from hit_test().
    ///
    /// Used by Plot to draw selection highlight overlays.
    /// Returns None if the hit data is unrecognized or the object
    /// is not visible.
    fn selection_rect(&self, hit: &dyn std::any::Any, transform: &Transform) -> Option<[f32; 4]> {
        let _ = (hit, transform);
        None
    }

    /// Find all objects within a world-coordinate range.
    ///
    /// Used for box selection. Returns a Vec of hit data (same type
    /// as hit_test returns) for all objects that overlap the given
    /// world-coordinate rectangle.
    ///
    /// Default: empty (no range selection support).
    fn objects_in_range(
        &self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
    ) -> Vec<Box<dyn std::any::Any>> {
        let _ = (x_min, x_max, y_min, y_max);
        Vec::new()
    }

    /// Return a HitProxy snapshot of this series' data for main-thread
    /// hit-testing. Called once at init time.
    fn hit_proxy(&self) -> crate::plot::hit_proxy::HitProxy;

    /// Apply a data update. Returns true if the data type matches.
    fn apply_data(&mut self, data: &SeriesData) -> bool {
        let _ = data;
        false
    }

    /// Replace the last data element. Implementation downcasts from Any.
    fn update_last(&mut self, _data: Box<dyn std::any::Any>) {}

    /// Append one data element. Implementation downcasts from Any.
    fn append_data(&mut self, _data: Box<dyn std::any::Any>) {}
}
