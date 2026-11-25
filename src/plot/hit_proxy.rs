// SPEC-HAL-296 Phase 2b: HitProxy — Main-Thread Hit-Test Data
//
// Pure coordinate math for hit-testing, selection rects, and Y-range queries.
// No GPU types, no wgpu — lives entirely on the main thread.
//
// Data is cloned from Series at init time and never mutated.

use crate::plot::series::SeriesInteraction;
use crate::plot::series::area::AreaPoint;
use crate::plot::series::bar::BarPoint;
use crate::plot::series::candle::RenderCandle;
use crate::plot::series::circle::Circle;
use crate::plot::series::depth::{BookLevel, BookSideKind};
use crate::plot::series::histogram::HistogramPoint;
use crate::plot::series::line::LinePoint;
use crate::plot::transform::Transform;

/// Cloned series data for main-thread hit-testing.
pub enum HitProxy {
    Candle {
        candles: Vec<RenderCandle>,
    },
    Line {
        points: Vec<LinePoint>,
    },
    Bar {
        points: Vec<BarPoint>,
    },
    Area {
        points: Vec<AreaPoint>,
    },
    Histogram {
        points: Vec<HistogramPoint>,
    },
    Depth {
        bids: Vec<BookLevel>,
        asks: Vec<BookLevel>,
        tick_spacing: f64,
    },
    Circles {
        circles: Vec<Circle>,
        default_radius: f32,
    },
}

/// Cached series metadata (interaction, z_order, auto-scale flag).
pub struct SeriesMeta {
    pub interaction: SeriesInteraction,
    pub z_order: u32,
    pub contributes_to_y_autoscale: bool,
}

/// One entry per series: proxy data + metadata.
pub struct HitTestEntry {
    pub proxy: HitProxy,
    pub meta: SeriesMeta,
}

impl HitTestEntry {
    /// Hit-test a screen-space point against this series' data.
    /// Returns an opaque hit result (same semantics as Series::hit_test).
    pub fn hit_test(
        &self,
        x: f32,
        y: f32,
        transform: &Transform,
    ) -> Option<Box<dyn std::any::Any>> {
        match &self.proxy {
            HitProxy::Candle { candles } => {
                let world_x = transform.px_to_x(x);
                let world_y = transform.px_to_y(y);
                for (i, c) in candles.iter().enumerate() {
                    if world_x >= c.x_open
                        && world_x <= c.x_close
                        && world_y >= c.low
                        && world_y <= c.high
                    {
                        return Some(Box::new(i));
                    }
                }
                None
            }
            HitProxy::Line { points } => {
                if points.len() < 2 {
                    return None;
                }
                let threshold = 10.0_f32;
                let mut best_dist = f32::INFINITY;
                let mut best_idx = 0usize;
                for (i, p) in points.iter().enumerate() {
                    let px = transform.x_to_px(p.x);
                    let py = transform.y_to_px(p.y);
                    let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
                    if dist < best_dist {
                        best_dist = dist;
                        best_idx = i;
                    }
                }
                if best_dist <= threshold {
                    Some(Box::new(best_idx))
                } else {
                    None
                }
            }
            HitProxy::Bar { .. } | HitProxy::Area { .. } | HitProxy::Histogram { .. } => None,
            HitProxy::Circles {
                circles,
                default_radius,
            } => {
                let threshold = 2.0_f32;
                let mut best_effective = f32::INFINITY;
                let mut best_idx = 0usize;
                for (i, p) in circles.iter().enumerate() {
                    let px = transform.x_to_px(p.x);
                    let py = transform.y_to_px(p.y);
                    let r = p.radius.unwrap_or(*default_radius);
                    let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
                    let effective = dist - r;
                    if effective < best_effective {
                        best_effective = effective;
                        best_idx = i;
                    }
                }
                if best_effective <= threshold {
                    Some(Box::new(best_idx))
                } else {
                    None
                }
            }
            HitProxy::Depth {
                bids,
                asks,
                tick_spacing,
            } => {
                let world_y = transform.px_to_y(y);
                let world_x = transform.px_to_x(x);
                let threshold = tick_spacing * 0.6;

                let (side, levels) = if world_x < 0.0 {
                    (BookSideKind::Bid, bids)
                } else {
                    (BookSideKind::Ask, asks)
                };

                levels
                    .iter()
                    .min_by(|a, b| {
                        (a.price - world_y)
                            .abs()
                            .partial_cmp(&(b.price - world_y).abs())
                            .unwrap()
                    })
                    .filter(|l| (l.price - world_y).abs() <= threshold)
                    .map(|l| -> Box<dyn std::any::Any> {
                        Box::new((l.price, side, l.quantity, l.classification))
                    })
            }
        }
    }

    /// Return the screen-space rect [x1, y1, x2, y2] of a selected object.
    pub fn selection_rect(
        &self,
        hit: &dyn std::any::Any,
        transform: &Transform,
    ) -> Option<[f32; 4]> {
        match &self.proxy {
            HitProxy::Candle { candles } => {
                let idx = hit.downcast_ref::<usize>()?;
                let c = candles.get(*idx)?;
                if c.x_close < transform.x_min || c.x_open > transform.x_max {
                    return None;
                }
                let x1 = transform.x_to_px(c.x_open);
                let x2 = transform.x_to_px(c.x_close);
                let y_top = transform.y_to_px(c.high);
                let y_bot = transform.y_to_px(c.low);
                Some([x1, y_top, x2, y_bot])
            }
            HitProxy::Line { points } => {
                let idx = hit.downcast_ref::<usize>()?;
                let p = points.get(*idx)?;
                let px = transform.x_to_px(p.x);
                let py = transform.y_to_px(p.y);
                let r = 8.0_f32;
                Some([px - r, py - r, px + r, py + r])
            }
            HitProxy::Bar { .. }
            | HitProxy::Area { .. }
            | HitProxy::Histogram { .. }
            | HitProxy::Depth { .. } => None,
            HitProxy::Circles {
                circles,
                default_radius,
            } => {
                let idx = hit.downcast_ref::<usize>()?;
                let p = circles.get(*idx)?;
                let px = transform.x_to_px(p.x);
                let py = transform.y_to_px(p.y);
                let r = p.radius.unwrap_or(*default_radius) + 2.0;
                Some([px - r, py - r, px + r, py + r])
            }
        }
    }

    /// Find all objects within a world-coordinate range.
    pub fn objects_in_range(
        &self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
    ) -> Vec<Box<dyn std::any::Any>> {
        match &self.proxy {
            HitProxy::Candle { candles } => {
                let mut results: Vec<Box<dyn std::any::Any>> = Vec::new();
                for (i, c) in candles.iter().enumerate() {
                    if c.x_close >= x_min && c.x_open <= x_max && c.high >= y_min && c.low <= y_max
                    {
                        results.push(Box::new(i));
                    }
                }
                results
            }
            HitProxy::Line { .. } => Vec::new(),
            HitProxy::Bar { .. }
            | HitProxy::Area { .. }
            | HitProxy::Histogram { .. }
            | HitProxy::Depth { .. } => Vec::new(),
            HitProxy::Circles { .. } => Vec::new(),
        }
    }

    /// Query the Y range of data for a given X range (for auto-scale).
    pub fn data_y_range(&self, x_min: f64, x_max: f64) -> Option<(f64, f64)> {
        match &self.proxy {
            HitProxy::Candle { candles } => {
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                let mut found = false;
                for c in candles {
                    if c.x_close >= x_min && c.x_open <= x_max {
                        lo = lo.min(c.low);
                        hi = hi.max(c.high);
                        found = true;
                    }
                }
                if found { Some((lo, hi)) } else { None }
            }
            HitProxy::Line { points } => {
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                let mut found = false;
                for p in points {
                    if p.x >= x_min && p.x <= x_max {
                        lo = lo.min(p.y);
                        hi = hi.max(p.y);
                        found = true;
                    }
                }
                if found { Some((lo, hi)) } else { None }
            }
            HitProxy::Bar { points } => {
                // Mirror BarSeries::data_y_range logic
                let mut lo = 0.0_f64; // baseline
                let mut hi = 0.0_f64;
                let mut found = false;
                for p in points {
                    // Approximate: treat each bar as a point at p.x
                    if p.x >= x_min && p.x <= x_max {
                        lo = lo.min(p.y);
                        hi = hi.max(p.y);
                        found = true;
                    }
                }
                if found { Some((lo, hi)) } else { None }
            }
            HitProxy::Area { points } => {
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                let mut found = false;
                for p in points {
                    if p.x >= x_min && p.x <= x_max {
                        lo = lo.min(p.y_lower);
                        hi = hi.max(p.y_upper);
                        found = true;
                    }
                }
                if found { Some((lo, hi)) } else { None }
            }
            HitProxy::Histogram { points } => {
                let mut lo = 0.0_f64;
                let mut hi = 0.0_f64;
                for p in points {
                    if p.x >= x_min && p.x <= x_max {
                        lo = lo.min(p.y);
                        hi = hi.max(p.y);
                    }
                }
                if lo <= hi { Some((lo, hi)) } else { None }
            }
            HitProxy::Depth { bids, asks, .. } => {
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                for l in bids.iter().chain(asks.iter()) {
                    lo = lo.min(l.price);
                    hi = hi.max(l.price);
                }
                if lo <= hi { Some((lo, hi)) } else { None }
            }
            HitProxy::Circles { circles, .. } => {
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                let mut found = false;
                for p in circles {
                    if p.x >= x_min && p.x <= x_max {
                        lo = lo.min(p.y);
                        hi = hi.max(p.y);
                        found = true;
                    }
                }
                if found { Some((lo, hi)) } else { None }
            }
        }
    }
}
