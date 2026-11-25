// SPEC-HAL-296 §15.2: Optional Plot/order-book market-detail alignment.

use super::{AxisAuthority, AxisKind, PriceTransform, Probe};
use crate::core::paint::Color;
use crate::model::order_book::{BookLevel, BookSide, BookSideKind};
use crate::plot::price_level::PriceLevelLine;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MarketDetailReferenceLine {
    pub price: f64,
    pub color: Color,
    pub line_style: PriceLevelLine,
}

struct MarketDetailPresentation {
    price_axis: AxisAuthority,
    reference_line: Option<MarketDetailReferenceLine>,
    reference_generation: u64,
}

#[derive(Default)]
struct MarketDetailNavigation {
    pending_price_pan_viewports: f64,
}

const MAX_PENDING_PRICE_PAN_VIEWPORTS: f64 = 8.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct MarketDetailPresentedSnapshot {
    pub axis_generation: u64,
    pub price_min: f64,
    pub price_max: f64,
    pub reference_generation: u64,
    pub reference_line: Option<MarketDetailReferenceLine>,
}

/// Pure-data opt-in link between a price-owning Plot and aligned projections.
#[derive(Clone)]
pub struct MarketDetailLink {
    price_axis: Arc<Mutex<AxisAuthority>>,
    probe: Arc<Mutex<Probe>>,
    presentation: Arc<Mutex<MarketDetailPresentation>>,
    navigation: Arc<Mutex<MarketDetailNavigation>>,
    price_axis_kind: AxisKind,
}

impl MarketDetailLink {
    pub fn new(price_min: f64, price_max: f64) -> Self {
        Self::with_axis_kind(price_min, price_max, AxisKind::Linear)
    }

    pub fn with_axis_kind(price_min: f64, price_max: f64, price_axis_kind: AxisKind) -> Self {
        Self {
            price_axis: Arc::new(Mutex::new(AxisAuthority::new(price_min, price_max))),
            probe: Arc::new(Mutex::new(Probe::new())),
            presentation: Arc::new(Mutex::new(MarketDetailPresentation {
                price_axis: AxisAuthority::new(price_min, price_max),
                reference_line: None,
                reference_generation: 0,
            })),
            navigation: Arc::new(Mutex::new(MarketDetailNavigation::default())),
            price_axis_kind,
        }
    }

    pub fn price_range(&self) -> (f64, f64) {
        self.presentation
            .lock()
            .map(|presentation| presentation.price_axis.range())
            .unwrap_or((f64::NAN, f64::NAN))
    }

    pub fn price_axis_kind(&self) -> AxisKind {
        self.price_axis_kind
    }

    pub(crate) fn price_axis(&self) -> Arc<Mutex<AxisAuthority>> {
        self.price_axis.clone()
    }

    pub(crate) fn probe(&self) -> Arc<Mutex<Probe>> {
        self.probe.clone()
    }

    pub(crate) fn request_price_pan(&self, delta_y: f32, viewport_height: f32) -> bool {
        if !delta_y.is_finite()
            || delta_y == 0.0
            || !viewport_height.is_finite()
            || viewport_height <= 0.0
        {
            return false;
        }
        let fraction = delta_y as f64 / viewport_height as f64;
        if let Ok(mut navigation) = self.navigation.lock() {
            navigation.pending_price_pan_viewports =
                (navigation.pending_price_pan_viewports + fraction).clamp(
                    -MAX_PENDING_PRICE_PAN_VIEWPORTS,
                    MAX_PENDING_PRICE_PAN_VIEWPORTS,
                );
            true
        } else {
            false
        }
    }

    pub(crate) fn take_price_pan(&self) -> f64 {
        self.navigation
            .lock()
            .map(|mut navigation| std::mem::take(&mut navigation.pending_price_pan_viewports))
            .unwrap_or(0.0)
    }

    #[cfg(test)]
    pub(crate) fn reference_line(&self) -> Option<MarketDetailReferenceLine> {
        self.presentation
            .lock()
            .ok()
            .and_then(|presentation| presentation.reference_line)
    }

    pub(crate) fn set_reference_line(&self, reference: Option<MarketDetailReferenceLine>) {
        if let Ok(mut presentation) = self.presentation.lock()
            && presentation.reference_line != reference
        {
            presentation.reference_line = reference;
            presentation.reference_generation = presentation.reference_generation.wrapping_add(1);
        }
    }

    pub(crate) fn reference_generation(&self) -> u64 {
        self.presentation
            .lock()
            .map(|presentation| presentation.reference_generation)
            .unwrap_or(0)
    }

    pub(crate) fn presented_axis_snapshot(&self) -> Option<(u64, f64, f64)> {
        self.presentation.lock().ok().map(|presentation| {
            let (min, max) = presentation.price_axis.range();
            (presentation.price_axis.generation(), min, max)
        })
    }

    pub(crate) fn presented_snapshot(&self) -> Option<MarketDetailPresentedSnapshot> {
        self.presentation.lock().ok().map(|presentation| {
            let (price_min, price_max) = presentation.price_axis.range();
            MarketDetailPresentedSnapshot {
                axis_generation: presentation.price_axis.generation(),
                price_min,
                price_max,
                reference_generation: presentation.reference_generation,
                reference_line: presentation.reference_line,
            }
        })
    }

    pub(crate) fn set_presented_price_range(&self, min: f64, max: f64) {
        if let Ok(mut presentation) = self.presentation.lock() {
            presentation.price_axis.set_range(min, max);
        }
    }

    pub(crate) fn set_presented_frame(
        &self,
        min: f64,
        max: f64,
        reference: Option<MarketDetailReferenceLine>,
    ) {
        if let Ok(mut presentation) = self.presentation.lock() {
            presentation.price_axis.set_range(min, max);
            if presentation.reference_line != reference {
                presentation.reference_line = reference;
                presentation.reference_generation =
                    presentation.reference_generation.wrapping_add(1);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn set_price_range_for_test(&self, min: f64, max: f64) {
        if let Ok(mut axis) = self.price_axis.lock() {
            axis.set_range(min, max);
        }
        self.set_presented_price_range(min, max);
    }
}

/// Presentation-only aligned ladder projection policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketDetailConfig {
    /// Price grouping policy for aligned rows.
    pub grouping: MarketDetailGrouping,
    /// Minimum vertical bucket height. Output rows are bounded by viewport pixels.
    pub min_bucket_height: f32,
    /// Robust visible-depth percentile used as the horizontal scale target.
    pub depth_percentile: f32,
    /// Rate used when visible depth grows.
    pub scale_attack: f32,
    /// Rate used when visible depth falls.
    pub scale_release: f32,
}

impl Default for MarketDetailConfig {
    fn default() -> Self {
        Self {
            grouping: MarketDetailGrouping::AutoPixel,
            min_bucket_height: 10.0,
            depth_percentile: 0.90,
            scale_attack: 64.0 / 3.0,
            scale_release: 16.0 / 3.0,
        }
    }
}

/// Price grouping intent for an aligned market-detail projection.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum MarketDetailGrouping {
    /// Group only when source levels would collide at the configured pixel floor.
    #[default]
    AutoPixel,
    /// Group prices that round to the requested number of decimal places.
    DecimalPlaces(u8),
    /// Group prices on an explicit positive price-step grid.
    PriceStep(MarketDetailPriceStep),
}

/// Validated positive price step for semantic market-detail grouping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketDetailPriceStep(f64);

impl MarketDetailPriceStep {
    pub fn new(step: f64) -> Option<Self> {
        (step.is_finite() && step > 0.0).then_some(Self(step))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquiditySummary {
    pub level_count: usize,
    pub quantity: f64,
}

impl Default for LiquiditySummary {
    fn default() -> Self {
        Self {
            level_count: 0,
            quantity: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedPriceRange {
    /// Inclusive lower boundary of the deterministic presentation bucket.
    pub min: f64,
    /// Exclusive upper boundary except at the visible domain maximum.
    pub max: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedBookLevel {
    pub side: BookSideKind,
    /// Single operator-facing grouped price at the row's axis-coordinate midpoint.
    pub price: f64,
    /// Internal aggregation bounds retained for projection evidence and interaction.
    pub price_range: ProjectedPriceRange,
    pub quantity: f64,
    pub source_level_count: usize,
    pub y: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarketDetailProjection {
    pub levels: Vec<ProjectedBookLevel>,
    pub above: LiquiditySummary,
    pub below: LiquiditySummary,
}

#[derive(Default)]
struct SideIndex {
    levels: Vec<BookLevel>,
    prefix_quantity: Vec<f64>,
}

impl SideIndex {
    fn replace(&mut self, side: BookSide) -> bool {
        let mut levels = side.levels;
        levels.sort_by(|a, b| a.price.total_cmp(&b.price));
        if self.levels == levels {
            return false;
        }
        self.levels = levels;
        self.prefix_quantity.clear();
        self.prefix_quantity.reserve(self.levels.len() + 1);
        self.prefix_quantity.push(0.0);
        for level in &self.levels {
            let next =
                self.prefix_quantity.last().copied().unwrap_or(0.0) + level.quantity.max(0.0);
            self.prefix_quantity.push(next);
        }
        true
    }

    fn lower_bound(&self, price: f64) -> usize {
        self.levels.partition_point(|level| level.price < price)
    }

    fn upper_bound(&self, price: f64) -> usize {
        self.levels.partition_point(|level| level.price <= price)
    }

    fn quantity(&self, start: usize, end: usize) -> f64 {
        self.prefix_quantity[end] - self.prefix_quantity[start]
    }

    fn summary(&self, start: usize, end: usize) -> LiquiditySummary {
        LiquiditySummary {
            level_count: end.saturating_sub(start),
            quantity: self.quantity(start, end),
        }
    }

    fn nearest(&self, price: f64, min: f64, max: f64) -> Option<&BookLevel> {
        let start = self.lower_bound(min);
        let end = self.upper_bound(max);
        if start >= end {
            return None;
        }
        let insertion =
            self.levels[start..end].partition_point(|level| level.price < price) + start;
        let upper = self
            .levels
            .get(insertion)
            .filter(|level| level.price <= max);
        let lower = insertion
            .checked_sub(1)
            .and_then(|index| self.levels.get(index))
            .filter(|level| level.price >= min);
        match (lower, upper) {
            (Some(a), Some(b)) => {
                if (price - a.price).abs() <= (b.price - price).abs() {
                    Some(a)
                } else {
                    Some(b)
                }
            }
            (Some(level), None) | (None, Some(level)) => Some(level),
            (None, None) => None,
        }
    }

    fn exact(&self, price: f64) -> Option<&BookLevel> {
        let index = self.lower_bound(price);
        self.levels
            .get(index)
            .filter(|level| level.price.to_bits() == price.to_bits())
    }
}

/// Indexed exact book snapshot plus bounded viewport projection.
#[derive(Default)]
pub struct MarketDetailProjector {
    bids: SideIndex,
    asks: SideIndex,
    source_generation: u64,
    projection_count: AtomicU64,
}

impl MarketDetailProjector {
    pub fn replace_book(&mut self, bids: BookSide, asks: BookSide) -> bool {
        let changed = self.bids.replace(bids) | self.asks.replace(asks);
        if changed {
            self.source_generation = self.source_generation.wrapping_add(1);
        }
        changed
    }

    pub fn replace_bids(&mut self, bids: BookSide) -> bool {
        let changed = self.bids.replace(bids);
        if changed {
            self.source_generation = self.source_generation.wrapping_add(1);
        }
        changed
    }

    pub fn replace_asks(&mut self, asks: BookSide) -> bool {
        let changed = self.asks.replace(asks);
        if changed {
            self.source_generation = self.source_generation.wrapping_add(1);
        }
        changed
    }

    pub fn source_generation(&self) -> u64 {
        self.source_generation
    }

    pub fn projection_count(&self) -> u64 {
        self.projection_count.load(Ordering::Relaxed)
    }

    pub fn project(
        &self,
        transform: PriceTransform,
        config: MarketDetailConfig,
    ) -> MarketDetailProjection {
        self.project_with_reference_anchor(transform, config, None)
    }

    pub(crate) fn project_with_reference_anchor(
        &self,
        transform: PriceTransform,
        config: MarketDetailConfig,
        reference_price: Option<f64>,
    ) -> MarketDetailProjection {
        self.projection_count.fetch_add(1, Ordering::Relaxed);
        let mut projection = MarketDetailProjection::default();
        let Some(bucket_grid) = PriceBucketGrid::new(transform, config, reference_price) else {
            return projection;
        };
        project_side(
            &self.bids,
            BookSideKind::Bid,
            transform,
            &bucket_grid,
            &mut projection,
        );
        project_side(
            &self.asks,
            BookSideKind::Ask,
            transform,
            &bucket_grid,
            &mut projection,
        );
        projection.levels.sort_by(|a, b| {
            a.y.total_cmp(&b.y)
                .then_with(|| side_rank(a.side).cmp(&side_rank(b.side)))
        });
        projection
    }

    pub fn nearest_exact(
        &self,
        side: BookSideKind,
        price: f64,
        visible_min: f64,
        visible_max: f64,
    ) -> Option<&BookLevel> {
        match side {
            BookSideKind::Bid => &self.bids,
            BookSideKind::Ask => &self.asks,
        }
        .nearest(price, visible_min, visible_max)
    }

    pub(crate) fn exact_level(&self, side: BookSideKind, price: f64) -> Option<&BookLevel> {
        match side {
            BookSideKind::Bid => &self.bids,
            BookSideKind::Ask => &self.asks,
        }
        .exact(price)
    }
}

fn side_rank(side: BookSideKind) -> u8 {
    match side {
        BookSideKind::Bid => 0,
        BookSideKind::Ask => 1,
    }
}

fn add_summary(target: &mut LiquiditySummary, source: LiquiditySummary) {
    target.level_count += source.level_count;
    target.quantity += source.quantity;
}

#[derive(Clone, Copy)]
struct PriceBucket {
    price_low: f64,
    price_high: f64,
}

struct PriceBucketGrid {
    buckets: Vec<PriceBucket>,
}

impl PriceBucketGrid {
    fn new(
        transform: PriceTransform,
        config: MarketDetailConfig,
        reference_price: Option<f64>,
    ) -> Option<Self> {
        if transform.height_px <= 0.0 {
            return None;
        }
        let axis_min = transform.price_to_axis_coordinate(transform.min)?;
        let axis_max = transform.price_to_axis_coordinate(transform.max)?;
        let axis_span = axis_max - axis_min;
        if !axis_span.is_finite() || axis_span <= 0.0 {
            return None;
        }

        let bucket_height = if config.min_bucket_height.is_finite() {
            config.min_bucket_height.max(1.0)
        } else {
            1.0
        };
        match config.grouping {
            MarketDetailGrouping::AutoPixel => Self::automatic(
                transform,
                axis_min,
                axis_max,
                axis_span,
                bucket_height,
                reference_price,
            ),
            MarketDetailGrouping::DecimalPlaces(decimal_places) => {
                let step = 10_f64.powi(-(decimal_places as i32));
                Self::semantic(
                    transform,
                    axis_max,
                    axis_span,
                    bucket_height,
                    step,
                    reference_price,
                )
            }
            MarketDetailGrouping::PriceStep(step) => Self::semantic(
                transform,
                axis_max,
                axis_span,
                bucket_height,
                step.get(),
                reference_price,
            ),
        }
    }

    fn automatic(
        transform: PriceTransform,
        axis_min: f64,
        axis_max: f64,
        axis_span: f64,
        bucket_height: f32,
        reference_price: Option<f64>,
    ) -> Option<Self> {
        let step = axis_span * bucket_height as f64 / transform.height_px as f64;
        if !step.is_finite() || step <= 0.0 {
            return None;
        }
        let reference_axis = reference_price
            .and_then(|price| transform.price_to_axis_coordinate(price))
            .filter(|axis| axis.is_finite());
        let first_axis = reference_axis.map_or_else(
            || (axis_min / step).floor() * step,
            |reference_axis| reference_axis + ((axis_min - reference_axis) / step).floor() * step,
        );
        if !first_axis.is_finite() {
            return None;
        }

        let pixel_bound = (transform.height_px / bucket_height).ceil().max(1.0) as usize + 2;
        let bucket_count =
            (((axis_max - first_axis) / step).ceil().max(1.0) as usize).min(pixel_bound);
        let mut buckets = Vec::with_capacity(bucket_count);
        let mut axis_low = first_axis;
        for _ in 0..bucket_count {
            let axis_high = axis_low + step;
            if !axis_high.is_finite() || axis_high <= axis_low {
                break;
            }
            let Some(price_low) = transform.axis_coordinate_to_price(axis_low) else {
                axis_low = axis_high;
                continue;
            };
            let Some(price_high) = transform.axis_coordinate_to_price(axis_high) else {
                axis_low = axis_high;
                continue;
            };
            if price_high > price_low {
                buckets.push(PriceBucket {
                    price_low,
                    price_high,
                });
            }
            axis_low = axis_high;
        }
        Some(Self { buckets })
    }

    fn semantic(
        transform: PriceTransform,
        axis_max: f64,
        axis_span: f64,
        bucket_height: f32,
        base_step: f64,
        reference_price: Option<f64>,
    ) -> Option<Self> {
        if !base_step.is_finite() || base_step <= 0.0 {
            return None;
        }
        let collision_axis_span = axis_span * bucket_height as f64 / transform.height_px as f64;
        let collision_price_span = match transform.kind {
            AxisKind::Time | AxisKind::Linear => collision_axis_span,
            AxisKind::Log => {
                transform.max
                    - transform.axis_coordinate_to_price(axis_max - collision_axis_span)?
            }
        };
        if !collision_price_span.is_finite() || collision_price_span <= 0.0 {
            return None;
        }
        let ratio = collision_price_span / base_step;
        if !ratio.is_finite() {
            return None;
        }
        let nearest = ratio.round();
        let tolerance = f64::EPSILON * 16.0 * ratio.abs().max(1.0);
        let merge_count = if (ratio - nearest).abs() <= tolerance {
            nearest
        } else {
            ratio.ceil()
        }
        .max(1.0);
        let step = base_step * merge_count;
        if !step.is_finite() || step <= 0.0 {
            return None;
        }

        let mut price_low = ((transform.min / step) - 0.5).floor() * step + step * 0.5;
        if !price_low.is_finite() {
            return None;
        }
        while price_low > transform.min {
            let previous = price_low - step;
            if previous >= price_low {
                return None;
            }
            price_low = previous;
        }
        while price_low + step <= transform.min {
            let next = price_low + step;
            if next <= price_low {
                return None;
            }
            price_low = next;
        }

        let pixel_bound = (transform.height_px / bucket_height).ceil().max(1.0) as usize + 3;
        let reference_price = reference_price
            .filter(|price| price.is_finite() && *price > transform.min && *price < transform.max);
        let mut buckets = Vec::with_capacity(pixel_bound);
        for _ in 0..pixel_bound {
            let price_high = price_low + step;
            if !price_high.is_finite() || price_high <= price_low {
                break;
            }
            if let Some(reference) = reference_price
                && reference > price_low
                && reference < price_high
            {
                buckets.push(PriceBucket {
                    price_low,
                    price_high: reference,
                });
                buckets.push(PriceBucket {
                    price_low: reference,
                    price_high,
                });
            } else {
                buckets.push(PriceBucket {
                    price_low,
                    price_high,
                });
            }
            if price_high >= transform.max {
                break;
            }
            price_low = price_high;
        }
        Some(Self { buckets })
    }
}

fn project_side(
    side: &SideIndex,
    side_kind: BookSideKind,
    transform: PriceTransform,
    bucket_grid: &PriceBucketGrid,
    output: &mut MarketDetailProjection,
) {
    if side.levels.is_empty() || transform.height_px <= 0.0 {
        return;
    }

    let visible_start = side.lower_bound(transform.min);
    let visible_end = side.upper_bound(transform.max);
    add_summary(&mut output.below, side.summary(0, visible_start));
    add_summary(
        &mut output.above,
        side.summary(visible_end, side.levels.len()),
    );
    if visible_start >= visible_end {
        return;
    }

    for bucket in &bucket_grid.buckets {
        let price_low = bucket.price_low;
        let price_high = bucket.price_high;
        if price_high < transform.min || price_low > transform.max {
            continue;
        }
        let visible_low = price_low.max(transform.min);
        let visible_high = price_high.min(transform.max);
        let start = side.lower_bound(visible_low).max(visible_start);
        let end = if price_high >= transform.max {
            side.upper_bound(visible_high)
        } else {
            side.lower_bound(visible_high)
        }
        .min(visible_end);
        if start >= end {
            continue;
        }

        let Some(y_top) = transform.price_to_px(visible_high) else {
            continue;
        };
        let Some(y_bottom) = transform.price_to_px(visible_low) else {
            continue;
        };
        let height = y_bottom - y_top;
        if height <= 0.0 {
            continue;
        }

        let quantity = side.quantity(start, end);
        let Some(axis_low) = transform.price_to_axis_coordinate(price_low) else {
            continue;
        };
        let Some(axis_high) = transform.price_to_axis_coordinate(price_high) else {
            continue;
        };
        let Some(anchor_price) = transform.axis_coordinate_to_price((axis_low + axis_high) * 0.5)
        else {
            continue;
        };
        output.levels.push(ProjectedBookLevel {
            side: side_kind,
            price: anchor_price,
            price_range: ProjectedPriceRange {
                min: price_low,
                max: price_high,
            },
            quantity,
            source_level_count: end - start,
            y: y_top,
            height,
        });
    }
}

/// Smoothed horizontal visible-depth scale. It has no price-axis authority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisibleDepthScale {
    current: f64,
    target: f64,
}

impl Default for VisibleDepthScale {
    fn default() -> Self {
        Self {
            current: 1.0,
            target: 1.0,
        }
    }
}

impl VisibleDepthScale {
    pub fn current(&self) -> f64 {
        self.current.max(f64::EPSILON)
    }

    pub fn set_visible_levels(
        &mut self,
        levels: &[ProjectedBookLevel],
        config: MarketDetailConfig,
    ) {
        let mut quantities: Vec<f64> = levels
            .iter()
            .map(|level| level.quantity)
            .filter(|quantity| quantity.is_finite() && *quantity > 0.0)
            .collect();
        if quantities.is_empty() {
            self.target = 1.0;
            return;
        }
        quantities.sort_by(f64::total_cmp);
        let percentile = config.depth_percentile.clamp(0.5, 1.0);
        let index = ((quantities.len() - 1) as f32 * percentile).floor() as usize;
        let median = quantities[(quantities.len() - 1) / 2];
        self.target = quantities[index].min(median * 4.0).max(f64::EPSILON);
        if !self.current.is_finite() || self.current <= 0.0 {
            self.current = self.target;
        }
    }

    pub fn advance(&mut self, dt: f32, config: MarketDetailConfig) -> bool {
        let rate = if self.target > self.current {
            config.scale_attack
        } else {
            config.scale_release
        }
        .max(0.0);
        let alpha = 1.0 - (-rate * dt.max(0.0)).exp();
        let previous = self.current;
        self.current += (self.target - self.current) * alpha as f64;
        (self.current - previous).abs() > self.current.abs().max(1.0) * 1e-6
    }

    pub fn settled(&self) -> bool {
        (self.current - self.target).abs() <= self.current.abs().max(1.0) * 1e-4
    }
}
