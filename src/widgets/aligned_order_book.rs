// SPEC-HAL-296 §15.2 / SPEC-HAL-303: Optional price-aligned order-book ladder.

use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::LAYER_CONTENT;
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::model::order_book::{BookLevel, BookSide, BookSideKind};
use crate::plot::market_detail::MarketDetailPresentedSnapshot;
use crate::plot::{
    LiquiditySummary, MarketDetailConfig, MarketDetailLink, MarketDetailProjection,
    MarketDetailProjector, PriceLevelLine, PriceTransform, ProjectedBookLevel, VisibleDepthScale,
};
use crate::utils::id::WidgetInternalId;
use crate::widgets::order_book::{
    OrderBookConfig, OrderBookEvent, OrderBookUpdate, format_price, format_qty,
};
use crossbeam_channel::Receiver;
use std::borrow::Cow;

const UPDATE_CHANNEL_CAPACITY: usize = 4;
const EVENT_QUEUE_CAPACITY: usize = 128;
const SUMMARY_HEIGHT: f32 = 18.0;
// INV-303-20: labels require the existing ten-logical-pixel minimum.
const MIN_LABELED_ROW_HEIGHT: f32 = 10.0;

/// Vertical presentation policy for Plot-linked order-book rows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AlignedOrderBookRowLayout {
    /// Preserve the projected bucket's exact Plot-owned Y geometry.
    #[default]
    PriceAligned,
    /// Keep exact Plot-owned price anchors while packing rows to a readable height.
    ReadableAnchored,
}

#[derive(Debug, Clone, Copy)]
struct PresentedBookRow {
    level_index: usize,
    y: f32,
    height: f32,
    anchor_y: f32,
}

#[derive(Debug, Default)]
struct PresentedBookRows {
    rows: Vec<PresentedBookRow>,
    above: LiquiditySummary,
    below: LiquiditySummary,
}

fn projected_quantity_label(config: &OrderBookConfig, level: &ProjectedBookLevel) -> String {
    format_qty(config, level.quantity)
}

fn projected_price_label(config: &OrderBookConfig, level: &ProjectedBookLevel) -> String {
    format_price(config, level.price)
}

fn add_projected_summary(summary: &mut LiquiditySummary, level: &ProjectedBookLevel) {
    summary.level_count += level.source_level_count;
    summary.quantity += level.quantity;
}

fn focused_row_window(anchors: &[f32], capacity: usize, target_y: f32) -> std::ops::Range<usize> {
    if capacity >= anchors.len() {
        return 0..anchors.len();
    }
    if capacity == 0 {
        let split = anchors.partition_point(|anchor| *anchor < target_y);
        return split..split;
    }

    let mut start = anchors.partition_point(|anchor| *anchor < target_y);
    let mut end = start;
    while end - start < capacity {
        match (start > 0, end < anchors.len()) {
            (true, true) => {
                let before = (target_y - anchors[start - 1]).abs();
                let after = (anchors[end] - target_y).abs();
                if before <= after {
                    start -= 1;
                } else {
                    end += 1;
                }
            }
            (true, false) => start -= 1,
            (false, true) => end += 1,
            (false, false) => break,
        }
    }
    start..end
}

/// Stable exact-source selection for an aligned order-book level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlignedOrderBookSelection {
    pub side: BookSideKind,
    pub price: f64,
}

/// Optional ladder projection whose Y coordinates are owned by a linked Plot.
pub struct AlignedOrderBook {
    id: WidgetInternalId,
    link: MarketDetailLink,
    config: OrderBookConfig,
    projection_config: MarketDetailConfig,
    projector: MarketDetailProjector,
    projection: MarketDetailProjection,
    depth_scale: VisibleDepthScale,
    update_rx: Option<Receiver<OrderBookUpdate>>,
    pending_events: Vec<OrderBookEvent>,
    hovered: Option<(BookSideKind, f64)>,
    selection: Option<AlignedOrderBookSelection>,
    row_layout: AlignedOrderBookRowLayout,
    wheel_price_pan: bool,
    viewport_width: f32,
    viewport_height: f32,
    cached_axis_generation: Option<u64>,
    cached_reference_generation: u64,
    projected_source_generation: u64,
}

impl AlignedOrderBook {
    pub fn new(link: MarketDetailLink) -> Self {
        let cached_reference_generation = link.reference_generation();
        Self {
            id: WidgetInternalId::new(),
            link,
            config: OrderBookConfig::default(),
            projection_config: MarketDetailConfig::default(),
            projector: MarketDetailProjector::default(),
            projection: MarketDetailProjection::default(),
            depth_scale: VisibleDepthScale::default(),
            update_rx: None,
            pending_events: Vec::new(),
            hovered: None,
            selection: None,
            row_layout: AlignedOrderBookRowLayout::default(),
            wheel_price_pan: true,
            viewport_width: 0.0,
            viewport_height: 0.0,
            cached_axis_generation: None,
            cached_reference_generation,
            projected_source_generation: 0,
        }
    }

    pub fn with_config(mut self, config: OrderBookConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_projection_config(mut self, config: MarketDetailConfig) -> Self {
        self.projection_config = config;
        self
    }

    /// Select the vertical presentation policy for aligned rows.
    pub fn with_row_layout(mut self, layout: AlignedOrderBookRowLayout) -> Self {
        self.row_layout = layout;
        self
    }

    /// Enable or disable Plot-owned vertical price panning from wheel input.
    pub fn with_wheel_price_pan(mut self, enabled: bool) -> Self {
        self.wheel_price_pan = enabled;
        self
    }

    pub fn with_book(mut self, bids: BookSide, asks: BookSide) -> Self {
        self.set_book(bids, asks);
        self
    }

    pub fn with_bids(mut self, bids: BookSide) -> Self {
        self.set_bids(bids);
        self
    }

    pub fn with_asks(mut self, asks: BookSide) -> Self {
        self.set_asks(asks);
        self
    }

    pub fn with_update_channel(mut self) -> (Self, crossbeam_channel::Sender<OrderBookUpdate>) {
        let (tx, rx) = crossbeam_channel::bounded(UPDATE_CHANNEL_CAPACITY);
        self.update_rx = Some(rx);
        (self, tx)
    }

    pub fn set_book(&mut self, bids: BookSide, asks: BookSide) {
        self.projector.replace_book(bids, asks);
    }

    pub fn set_bids(&mut self, bids: BookSide) {
        self.projector.replace_bids(bids);
    }

    pub fn set_asks(&mut self, asks: BookSide) {
        self.projector.replace_asks(asks);
    }

    pub fn take_events(&mut self) -> Vec<OrderBookEvent> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn selection(&self) -> Option<AlignedOrderBookSelection> {
        self.selection
    }

    pub fn set_selection(&mut self, selection: Option<AlignedOrderBookSelection>) {
        self.selection = selection;
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn projection(&self) -> &MarketDetailProjection {
        &self.projection
    }

    pub fn projection_count(&self) -> u64 {
        self.projector.projection_count()
    }

    fn push_event(&mut self, event: OrderBookEvent) {
        if self.pending_events.len() >= EVENT_QUEUE_CAPACITY {
            let overflow = self.pending_events.len() + 1 - EVENT_QUEUE_CAPACITY;
            self.pending_events.drain(..overflow);
        }
        self.pending_events.push(event);
    }

    fn axis_snapshot(&self) -> Option<(u64, f64, f64)> {
        self.link.presented_axis_snapshot()
    }

    fn presented_snapshot(&self) -> Option<MarketDetailPresentedSnapshot> {
        self.link.presented_snapshot()
    }

    pub(crate) fn observes_market_projection(&self, link: &MarketDetailLink) -> bool {
        std::sync::Arc::ptr_eq(&self.link.price_axis(), &link.price_axis())
    }

    fn effective_projection_config(&self) -> MarketDetailConfig {
        let mut config = self.projection_config;
        if self.row_layout == AlignedOrderBookRowLayout::ReadableAnchored {
            config.min_bucket_height = 1.0;
        }
        config
    }

    fn project_snapshot(&self, snapshot: MarketDetailPresentedSnapshot) -> MarketDetailProjection {
        self.projector.project_with_reference_anchor(
            PriceTransform::new(
                snapshot.price_min,
                snapshot.price_max,
                self.link.price_axis_kind(),
                self.viewport_height,
            ),
            self.effective_projection_config(),
            snapshot.reference_line.map(|reference| reference.price),
        )
    }

    fn reproject_if_needed(&mut self, force: bool) -> bool {
        if self.viewport_height <= 0.0 {
            return false;
        }
        let Some(snapshot) = self.presented_snapshot() else {
            return false;
        };
        let source_generation = self.projector.source_generation();
        if !force
            && self.cached_axis_generation == Some(snapshot.axis_generation)
            && self.cached_reference_generation == snapshot.reference_generation
            && self.projected_source_generation == source_generation
        {
            return false;
        }

        self.projection = self.project_snapshot(snapshot);
        self.depth_scale
            .set_visible_levels(&self.projection.levels, self.effective_projection_config());
        self.cached_axis_generation = Some(snapshot.axis_generation);
        self.cached_reference_generation = snapshot.reference_generation;
        self.projected_source_generation = source_generation;
        true
    }

    fn projection_for_presented_frame(
        &self,
        snapshot: Option<MarketDetailPresentedSnapshot>,
    ) -> Cow<'_, MarketDetailProjection> {
        let Some(snapshot) = snapshot else {
            return Cow::Borrowed(&self.projection);
        };
        let source_generation = self.projector.source_generation();
        if self.cached_axis_generation == Some(snapshot.axis_generation)
            && self.cached_reference_generation == snapshot.reference_generation
            && self.projected_source_generation == source_generation
        {
            return Cow::Borrowed(&self.projection);
        }

        Cow::Owned(self.project_snapshot(snapshot))
    }

    fn presented_rows(
        &self,
        projection: &MarketDetailProjection,
        snapshot: Option<MarketDetailPresentedSnapshot>,
    ) -> PresentedBookRows {
        if self.row_layout == AlignedOrderBookRowLayout::PriceAligned {
            return PresentedBookRows {
                rows: projection
                    .levels
                    .iter()
                    .enumerate()
                    .map(|(level_index, level)| PresentedBookRow {
                        level_index,
                        y: level.y,
                        height: level.height,
                        anchor_y: level.y + level.height * 0.5,
                    })
                    .collect(),
                above: projection.above,
                below: projection.below,
            };
        }

        self.readable_anchored_rows(projection, snapshot)
    }

    fn readable_anchored_rows(
        &self,
        projection: &MarketDetailProjection,
        snapshot: Option<MarketDetailPresentedSnapshot>,
    ) -> PresentedBookRows {
        let height = self.viewport_height.max(0.0);
        let configured_height = if self.config.row_height.is_finite() {
            self.config.row_height
        } else {
            MIN_LABELED_ROW_HEIGHT
        };
        let row_height = configured_height.max(MIN_LABELED_ROW_HEIGHT);
        let candidates: Vec<usize> = projection
            .levels
            .iter()
            .enumerate()
            .filter(|(_, level)| level.y + level.height > 0.0 && level.y < height)
            .map(|(index, _)| index)
            .collect();
        if candidates.is_empty() {
            return PresentedBookRows {
                above: projection.above,
                below: projection.below,
                ..PresentedBookRows::default()
            };
        }

        let anchors: Vec<f32> = candidates
            .iter()
            .map(|index| {
                let level = &projection.levels[*index];
                level.y + level.height * 0.5
            })
            .collect();
        let target_y = snapshot
            .and_then(|snapshot| {
                snapshot
                    .reference_line
                    .and_then(|reference| self.reference_line_y(reference.price, height, snapshot))
            })
            .unwrap_or(height * 0.5);

        let mut capacity = ((height / row_height).floor() as usize).min(candidates.len());
        let mut window = focused_row_window(&anchors, capacity, target_y);
        for _ in 0..3 {
            let has_above = projection.above.level_count > 0 || window.start > 0;
            let has_below = projection.below.level_count > 0 || window.end < candidates.len();
            let reserved = (has_above as u8 as f32 + has_below as u8 as f32) * SUMMARY_HEIGHT;
            let next_capacity = (((height - reserved).max(0.0) / row_height).floor() as usize)
                .min(candidates.len());
            if next_capacity == capacity {
                break;
            }
            capacity = next_capacity;
            window = focused_row_window(&anchors, capacity, target_y);
        }
        window = focused_row_window(&anchors, capacity, target_y);

        let mut above = projection.above;
        for index in &candidates[..window.start] {
            add_projected_summary(&mut above, &projection.levels[*index]);
        }
        let mut below = projection.below;
        for index in &candidates[window.end..] {
            add_projected_summary(&mut below, &projection.levels[*index]);
        }

        let top = if above.level_count > 0 {
            SUMMARY_HEIGHT
        } else {
            0.0
        };
        let bottom = if below.level_count > 0 {
            SUMMARY_HEIGHT
        } else {
            0.0
        };
        let selected = &candidates[window.clone()];
        if selected.is_empty() {
            return PresentedBookRows {
                rows: Vec::new(),
                above,
                below,
            };
        }

        let min_center = top + row_height * 0.5;
        let max_center = height - bottom - row_height * 0.5;
        let mut centers = Vec::with_capacity(selected.len());
        for index in selected {
            let level = &projection.levels[*index];
            let desired = (level.y + level.height * 0.5).clamp(min_center, max_center);
            let center = centers
                .last()
                .map_or(desired, |previous| desired.max(*previous + row_height));
            centers.push(center);
        }
        if let Some(last) = centers.last_mut() {
            *last = last.min(max_center);
        }
        for index in (0..centers.len().saturating_sub(1)).rev() {
            centers[index] = centers[index].min(centers[index + 1] - row_height);
        }

        let rows = selected
            .iter()
            .zip(centers)
            .map(|(level_index, center)| {
                let level = &projection.levels[*level_index];
                PresentedBookRow {
                    level_index: *level_index,
                    y: center - row_height * 0.5,
                    height: row_height,
                    anchor_y: level.y + level.height * 0.5,
                }
            })
            .collect();

        PresentedBookRows { rows, above, below }
    }

    fn presented_row_level_at(&self, x: f32, y: f32) -> Option<(BookSideKind, BookLevel)> {
        if self.viewport_width <= 0.0 || self.viewport_height <= 0.0 {
            return None;
        }
        let snapshot = self.presented_snapshot()?;
        let side = if x < self.viewport_width * 0.5 {
            BookSideKind::Bid
        } else {
            BookSideKind::Ask
        };
        let projection = self.projection_for_presented_frame(Some(snapshot));
        let rows = self.presented_rows(&projection, Some(snapshot));
        let row = rows.rows.iter().find(|row| {
            projection.levels[row.level_index].side == side && y >= row.y && y < row.y + row.height
        })?;
        let projected = &projection.levels[row.level_index];
        let fraction = ((y - row.y) / row.height).clamp(0.0, 1.0);
        let source_y = projected.y + projected.height * fraction;
        let transform = PriceTransform::new(
            snapshot.price_min,
            snapshot.price_max,
            self.link.price_axis_kind(),
            self.viewport_height,
        );
        let price = transform.px_to_price(source_y)?;
        let min = projected.price_range.min.max(snapshot.price_min);
        let max = projected.price_range.max.min(snapshot.price_max);
        self.projector
            .nearest_exact(side, price, min, max)
            .cloned()
            .map(|level| (side, level))
    }

    fn exact_level_at(&self, x: f32, y: f32) -> Option<(BookSideKind, BookLevel)> {
        if self.row_layout == AlignedOrderBookRowLayout::ReadableAnchored {
            return self.presented_row_level_at(x, y);
        }
        if self.viewport_width <= 0.0 || self.viewport_height <= 0.0 {
            return None;
        }
        let (_, min, max) = self.axis_snapshot()?;
        let transform =
            PriceTransform::new(min, max, self.link.price_axis_kind(), self.viewport_height);
        let price = transform.px_to_price(y)?;
        let side = if x < self.viewport_width * 0.5 {
            BookSideKind::Bid
        } else {
            BookSideKind::Ask
        };
        self.projector
            .nearest_exact(side, price, min, max)
            .cloned()
            .map(|level| (side, level))
    }

    fn hover_level_at(&self, x: f32, y: f32) -> Option<(BookSideKind, BookLevel)> {
        self.presented_row_level_at(x, y)
    }

    fn set_hover(&mut self, next: Option<(BookSideKind, BookLevel)>) -> bool {
        let next_key = next.as_ref().map(|(side, level)| (*side, level.price));
        if self.hovered == next_key {
            return false;
        }

        if let Some((side, level)) = next {
            self.hovered = Some((side, level.price));
            self.push_event(OrderBookEvent::LevelHoverEnter {
                price: level.price,
                side,
                quantity: level.quantity,
            });
        } else {
            let had_hover = self.hovered.take().is_some();
            if had_hover {
                self.push_event(OrderBookEvent::LevelHoverLeave);
            }
        }
        true
    }

    fn projected_level_for_exact<'a>(
        &self,
        projection: &'a MarketDetailProjection,
        side: BookSideKind,
        price: f64,
    ) -> Option<&'a ProjectedBookLevel> {
        self.projector.exact_level(side, price)?;
        projection
            .levels
            .iter()
            .filter(|level| level.side == side)
            .find(|level| price >= level.price_range.min && price < level.price_range.max)
            .or_else(|| {
                projection
                    .levels
                    .iter()
                    .filter(|level| level.side == side)
                    .find(|level| price >= level.price_range.min && price <= level.price_range.max)
            })
    }

    fn selected_projected_level<'a>(
        &self,
        projection: &'a MarketDetailProjection,
    ) -> Option<&'a ProjectedBookLevel> {
        let selection = self.selection?;
        self.projected_level_for_exact(projection, selection.side, selection.price)
    }

    fn hovered_projected_level<'a>(
        &self,
        projection: &'a MarketDetailProjection,
    ) -> Option<&'a ProjectedBookLevel> {
        let (side, price) = self.hovered?;
        self.projected_level_for_exact(projection, side, price)
    }

    fn reference_line_y(
        &self,
        price: f64,
        height: f32,
        snapshot: MarketDetailPresentedSnapshot,
    ) -> Option<f32> {
        PriceTransform::new(
            snapshot.price_min,
            snapshot.price_max,
            self.link.price_axis_kind(),
            height,
        )
        .price_to_px(price)
    }

    fn paint_reference_line(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        snapshot: Option<MarketDetailPresentedSnapshot>,
    ) {
        let Some(snapshot) = snapshot else {
            return;
        };
        let Some(reference) = snapshot.reference_line else {
            return;
        };
        let Some(y) = self.reference_line_y(reference.price, rect.height, snapshot) else {
            return;
        };
        let unit = cx.one_px();
        let thickness = unit;
        let top = rect.y + y - thickness * 0.5;
        let right = rect.x + rect.width;
        let mut paint_segment = |start: f32, end: f32| {
            cx.scene.push_rect(
                RectF::new(start, top, (end - start).max(0.0), thickness),
                reference.color,
                LAYER_CONTENT + 3,
                None,
            );
        };

        match reference.line_style {
            PriceLevelLine::Solid => paint_segment(rect.x, right),
            PriceLevelLine::Dashed | PriceLevelLine::Dotted => {
                let (mark, gap) = match reference.line_style {
                    PriceLevelLine::Dashed => (8.0 * unit, 4.0 * unit),
                    PriceLevelLine::Dotted => (2.0 * unit, 3.0 * unit),
                    PriceLevelLine::Solid => unreachable!(),
                };
                let mut x = rect.x;
                while x < right {
                    paint_segment(x, (x + mark).min(right));
                    x += mark + gap;
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn project_viewport_for_test(&mut self, width: f32, height: f32) {
        let size_changed = self.viewport_width != width || self.viewport_height != height;
        self.viewport_width = width;
        self.viewport_height = height;
        self.reproject_if_needed(size_changed);
    }

    #[cfg(test)]
    pub(crate) fn poll_projection_for_test(&mut self) {
        self.reproject_if_needed(false);
    }

    #[cfg(test)]
    pub(crate) fn config_for_test(&self) -> &OrderBookConfig {
        &self.config
    }

    #[cfg(test)]
    pub(crate) fn projection_config_for_test(&self) -> MarketDetailConfig {
        self.projection_config
    }

    #[cfg(test)]
    pub(crate) fn row_layout_for_test(&self) -> AlignedOrderBookRowLayout {
        self.row_layout
    }

    #[cfg(test)]
    pub(crate) fn source_generation_for_test(&self) -> u64 {
        self.projector.source_generation()
    }

    #[cfg(test)]
    pub(crate) fn quantity_label_for_test(&self, level: &ProjectedBookLevel) -> String {
        projected_quantity_label(&self.config, level)
    }

    #[cfg(test)]
    pub(crate) fn price_label_for_test(&self, level: &ProjectedBookLevel) -> String {
        projected_price_label(&self.config, level)
    }

    #[cfg(test)]
    pub(crate) fn reference_line_y_for_test(&self, height: f32) -> Option<f32> {
        let snapshot = self.presented_snapshot()?;
        let reference = snapshot.reference_line?;
        self.reference_line_y(reference.price, height, snapshot)
    }

    #[cfg(test)]
    pub(crate) fn presented_projection_for_test(&self) -> MarketDetailProjection {
        self.projection_for_presented_frame(self.presented_snapshot())
            .into_owned()
    }

    #[cfg(test)]
    pub(crate) fn presented_rows_for_test(&self) -> Vec<(ProjectedBookLevel, f32, f32, f32)> {
        let snapshot = self.presented_snapshot();
        let projection = self.projection_for_presented_frame(snapshot);
        self.presented_rows(&projection, snapshot)
            .rows
            .into_iter()
            .map(|row| {
                (
                    projection.levels[row.level_index].clone(),
                    row.y,
                    row.height,
                    row.anchor_y,
                )
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn presented_summaries_for_test(&self) -> (LiquiditySummary, LiquiditySummary) {
        let snapshot = self.presented_snapshot();
        let projection = self.projection_for_presented_frame(snapshot);
        let rows = self.presented_rows(&projection, snapshot);
        (rows.above, rows.below)
    }

    #[cfg(test)]
    pub(crate) fn hover_at_for_test(&mut self, x: f32, y: f32) -> bool {
        let next = self.hover_level_at(x, y);
        self.set_hover(next)
    }

    #[cfg(test)]
    pub(crate) fn hovered_projected_level_for_test(&self) -> Option<ProjectedBookLevel> {
        let projection = self.projection_for_presented_frame(self.presented_snapshot());
        self.hovered_projected_level(&projection).cloned()
    }

    #[cfg(test)]
    pub(crate) fn select_at_for_test(
        &mut self,
        x: f32,
        y: f32,
    ) -> Option<AlignedOrderBookSelection> {
        let (side, level) = self.exact_level_at(x, y)?;
        let selection = AlignedOrderBookSelection {
            side,
            price: level.price,
        };
        self.selection = Some(selection);
        Some(selection)
    }

    #[cfg(test)]
    pub(crate) fn selected_projected_level_for_test(&self) -> Option<ProjectedBookLevel> {
        self.selected_projected_level(&self.projection).cloned()
    }

    #[cfg(test)]
    pub(crate) fn queue_price_pan_for_test(&self, delta_y: f32) -> bool {
        self.link.request_price_pan(delta_y, self.viewport_height)
    }
}

impl Widget for AlignedOrderBook {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let width = constraints.max_width;
        let height = constraints.max_height;
        let size_changed = self.viewport_width != width || self.viewport_height != height;
        self.viewport_width = width;
        self.viewport_height = height;
        self.reproject_if_needed(size_changed);
        LayoutResult::fit(Size::new(width, height))
    }

    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        let mut next_bids = None;
        let mut next_asks = None;
        if let Some(rx) = self.update_rx.as_ref() {
            while let Ok(update) = rx.try_recv() {
                match update {
                    OrderBookUpdate::Bids(bids) => next_bids = Some(bids),
                    OrderBookUpdate::Asks(asks) => next_asks = Some(asks),
                    OrderBookUpdate::Snapshot { bids, asks } => {
                        next_bids = Some(bids);
                        next_asks = Some(asks);
                    }
                    OrderBookUpdate::LastPrice { .. } => {}
                }
            }
        }
        match (next_bids, next_asks) {
            (Some(bids), Some(asks)) => {
                self.projector.replace_book(bids, asks);
            }
            (Some(bids), None) => {
                self.projector.replace_bids(bids);
            }
            (None, Some(asks)) => {
                self.projector.replace_asks(asks);
            }
            (None, None) => {}
        }
        let projection_changed = self.reproject_if_needed(false);
        let scale_changed = self.depth_scale.advance(dt, self.projection_config);
        if self.viewport_height <= 0.0 {
            self.cached_reference_generation = self.link.reference_generation();
        }
        if projection_changed || scale_changed {
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        self.update_rx.is_some()
            || self.cached_reference_generation != self.link.reference_generation()
    }

    fn needs_animation(&self) -> bool {
        !self.depth_scale.settled()
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene
            .push_rect(rect, cx.theme.colors.surfaces.content, LAYER_CONTENT, None);

        let center_x = rect.x + rect.width * 0.5;
        cx.scene.push_rect(
            RectF::new(center_x, rect.y, 1.0, rect.height),
            cx.theme.colors.borders.subtle,
            LAYER_CONTENT + 1,
            None,
        );

        let half_width = (rect.width * 0.5 - 4.0).max(0.0);
        let scale = self.depth_scale.current();
        let label_height = match self.row_layout {
            AlignedOrderBookRowLayout::PriceAligned => self.projection_config.min_bucket_height,
            AlignedOrderBookRowLayout::ReadableAnchored => self.config.row_height,
        };
        let font_size = self.config.font_size.min(label_height - 2.0);
        let metrics = cx.text_engine.font_metrics(font_size.max(8.0));
        let text_color = cx.theme.colors.typography.primary;
        let secondary = cx.theme.colors.typography.secondary;
        let presented_snapshot = self.presented_snapshot();
        let projection = self.projection_for_presented_frame(presented_snapshot);
        let presented_rows = self.presented_rows(&projection, presented_snapshot);
        let selected_range = self
            .selected_projected_level(&projection)
            .map(|level| (level.side, level.price_range.clone()));
        let hovered_range = self
            .hovered_projected_level(&projection)
            .map(|level| (level.side, level.price_range.clone()));

        for row in &presented_rows.rows {
            let level = &projection.levels[row.level_index];
            let y = rect.y + row.y;
            let height = row.height.min((rect.y + rect.height - y).max(0.0));
            if height <= 0.0 {
                continue;
            }
            let fraction = (level.quantity / scale).clamp(0.0, 1.0) as f32;
            let bar_width = half_width * fraction;
            let color = match level.side {
                BookSideKind::Bid => self.config.bid_color,
                BookSideKind::Ask => self.config.ask_color,
            };
            let bar_x = match level.side {
                BookSideKind::Bid => center_x - bar_width,
                BookSideKind::Ask => center_x,
            };
            let is_selected = selected_range
                .as_ref()
                .is_some_and(|(side, range)| *side == level.side && *range == level.price_range);
            let is_hovered = hovered_range
                .as_ref()
                .is_some_and(|(side, range)| *side == level.side && *range == level.price_range);
            if is_hovered || is_selected {
                let highlight_x = match level.side {
                    BookSideKind::Bid => rect.x,
                    BookSideKind::Ask => center_x,
                };
                cx.scene.push_rect(
                    RectF::new(highlight_x, y, half_width, height),
                    if is_selected {
                        cx.theme.colors.list.row_selected
                    } else {
                        cx.theme.colors.list.row_hover
                    },
                    LAYER_CONTENT + 1,
                    None,
                );
            }
            if is_selected {
                let indicator_x = match level.side {
                    BookSideKind::Bid => rect.x,
                    BookSideKind::Ask => rect.x + rect.width - 2.0,
                };
                cx.scene.push_rect(
                    RectF::new(indicator_x, y, 2.0, height),
                    cx.theme.colors.list.selection_indicator,
                    LAYER_CONTENT + 3,
                    None,
                );
            }
            cx.scene.push_rect(
                RectF::new(bar_x, y, bar_width, height),
                color.with_alpha(0.20 + fraction * 0.45),
                LAYER_CONTENT + 2,
                None,
            );

            if self.row_layout == AlignedOrderBookRowLayout::ReadableAnchored {
                let unit = cx.one_px();
                let anchor_y = rect.y + row.anchor_y;
                let row_center = y + height * 0.5;
                let rail_x = match level.side {
                    BookSideKind::Bid => center_x - 2.0 * unit,
                    BookSideKind::Ask => center_x + unit,
                };
                let connector_top = anchor_y.min(row_center);
                let connector_height = (anchor_y - row_center).abs().max(unit);
                if connector_height > unit {
                    cx.scene.push_rect(
                        RectF::new(rail_x, connector_top, unit, connector_height),
                        color.with_alpha(0.55),
                        LAYER_CONTENT + 3,
                        None,
                    );
                }
                let tick_x = match level.side {
                    BookSideKind::Bid => center_x - 5.0 * unit,
                    BookSideKind::Ask => center_x,
                };
                cx.scene.push_rect(
                    RectF::new(tick_x, anchor_y - unit * 0.5, 5.0 * unit, unit),
                    color.with_alpha(0.72),
                    LAYER_CONTENT + 3,
                    None,
                );
            }

            if height >= MIN_LABELED_ROW_HEIGHT {
                let text_y = y + (height - (metrics.ascent + metrics.descent)) * 0.5;
                let price = projected_price_label(&self.config, level);
                let quantity = projected_quantity_label(&self.config, level);
                let (price_width, _) = cx.text_engine.measure(&price, font_size.max(8.0));
                let price_run = cx.text_engine.shape(&price, font_size.max(8.0));
                let quantity_run = cx.text_engine.shape(&quantity, font_size.max(8.0));
                let (quantity_width, _) = cx.text_engine.measure(&quantity, font_size.max(8.0));
                let price_x = match level.side {
                    BookSideKind::Bid => center_x - price_width - 4.0,
                    BookSideKind::Ask => center_x + 4.0,
                };
                let quantity_x = match level.side {
                    BookSideKind::Bid => rect.x + 4.0,
                    BookSideKind::Ask => rect.x + rect.width - quantity_width - 4.0,
                };
                if price_width + 8.0 <= half_width {
                    cx.text_engine.draw(
                        &price_run,
                        price_x,
                        text_y,
                        color,
                        LAYER_CONTENT + 4,
                        cx.scene,
                    );
                }
                if price_width + quantity_width + 12.0 <= half_width {
                    cx.text_engine.draw(
                        &quantity_run,
                        quantity_x,
                        text_y,
                        text_color,
                        LAYER_CONTENT + 4,
                        cx.scene,
                    );
                }
            }
        }

        self.paint_reference_line(cx, rect, presented_snapshot);

        let summaries = [
            (&presented_rows.above, rect.y, "above"),
            (
                &presented_rows.below,
                rect.y + rect.height - SUMMARY_HEIGHT,
                "below",
            ),
        ];
        for (summary, y, label) in summaries {
            if summary.level_count == 0 {
                continue;
            }
            cx.scene.push_rect(
                RectF::new(rect.x, y, rect.width, SUMMARY_HEIGHT),
                cx.theme.colors.surfaces.level2.with_alpha(0.94),
                LAYER_CONTENT + 10,
                None,
            );
            let text = format!(
                "{} {} · {}",
                summary.level_count,
                label,
                format_qty(&self.config, summary.quantity)
            );
            let run = cx.text_engine.shape(&text, self.config.font_size);
            cx.text_engine.draw(
                &run,
                rect.x + 4.0,
                y + 2.0,
                secondary,
                LAYER_CONTENT + 11,
                cx.scene,
            );
        }
    }

    fn handle_event(&mut self, _cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        let Event::Pointer(pointer) = event else {
            return false;
        };
        match pointer.kind {
            PointerEventKind::Move => {
                let (x, y) = pointer.local_pos;
                self.set_hover(self.hover_level_at(x, y))
            }
            PointerEventKind::Down => {
                if pointer.button != Some(PointerButton::Primary) {
                    return false;
                }
                let (x, y) = pointer.local_pos;
                if let Some((side, level)) = self.exact_level_at(x, y) {
                    self.selection = Some(AlignedOrderBookSelection {
                        side,
                        price: level.price,
                    });
                    self.push_event(OrderBookEvent::LevelClicked {
                        price: level.price,
                        side,
                        quantity: level.quantity,
                        classification: level.classification,
                    });
                    true
                } else {
                    false
                }
            }
            PointerEventKind::Scroll {
                delta_x: _,
                delta_y,
            } if self.wheel_price_pan => self.link.request_price_pan(delta_y, self.viewport_height),
            _ => false,
        }
    }

    fn on_hover_leave(&mut self, _cx: &mut EventContext) {
        self.set_hover(None);
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn wants_scroll(&self) -> bool {
        self.wheel_price_pan
    }

    fn debug_type_name(&self) -> &'static str {
        "AlignedOrderBook"
    }
}

impl WidgetDowncast for AlignedOrderBook {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
