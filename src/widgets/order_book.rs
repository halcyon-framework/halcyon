// SPEC-HAL-303: OrderBook Widget — Projection-Only Tabular Ladder
//
// Container widget: header + ScrollArea(OrderBookBody) + ratio bar.
// Follows the ListView tree pattern with Rc<RefCell<OrderBookState>>.
//
// INV-303-0: Projection-only. No book state ownership.

use crate::core::event::{Event, EventPhase, KeyCode};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::Color;
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::model::order_book::{BookLevel, BookSide, BookSideKind, LevelClassification};
use crate::utils::id::WidgetInternalId;
use crate::widgets::order_book_body::OrderBookBody;
use crate::widgets::scroll_area::ScrollArea;
use crossbeam_channel::Receiver;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

const UPDATE_CHANNEL_CAPACITY: usize = 4;
const EVENT_QUEUE_CAPACITY: usize = 128;

// ── Public types ──

/// Price direction for the spread indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceDirection {
    Up,
    Down,
    Unchanged,
}

/// Events emitted by the order book widget.
#[derive(Debug, Clone)]
pub enum OrderBookEvent {
    LevelClicked {
        price: f64,
        side: BookSideKind,
        quantity: f64,
        classification: LevelClassification,
    },
    LevelHoverEnter {
        price: f64,
        side: BookSideKind,
        quantity: f64,
    },
    LevelHoverLeave,
}

/// Data update for the order book widget (sent via channel).
pub enum OrderBookUpdate {
    Bids(BookSide),
    Asks(BookSide),
    Snapshot {
        bids: BookSide,
        asks: BookSide,
    },
    LastPrice {
        price: f64,
        direction: PriceDirection,
    },
}

/// Configuration for the order book display.
#[derive(Clone)]
pub struct OrderBookConfig {
    /// Format a price value. Default: "{:.2}".
    pub price_formatter: Option<Arc<dyn Fn(f64) -> String + Send + Sync>>,
    /// Format a quantity value. Default: "{:.5}".
    pub quantity_formatter: Option<Arc<dyn Fn(f64) -> String + Send + Sync>>,
    /// Format a total (cumulative) value. Default: same as quantity.
    pub total_formatter: Option<Arc<dyn Fn(f64) -> String + Send + Sync>>,
    /// Maximum visible levels per side. Default: 25.
    pub max_visible_per_side: usize,
    /// Ask side color (used for bars, text).
    pub ask_color: Color,
    /// Bid side color (used for bars, text).
    pub bid_color: Color,
    /// Font size for row text. Default: 11.0.
    pub font_size: f32,
    /// Row height in pixels. Default: 18.0.
    pub row_height: f32,
}

impl Default for OrderBookConfig {
    fn default() -> Self {
        Self {
            price_formatter: None,
            quantity_formatter: None,
            total_formatter: None,
            max_visible_per_side: 25,
            ask_color: Color::from_hex(0xF05454),
            bid_color: Color::from_hex(0x2ECC71),
            font_size: 12.0,
            row_height: 19.0,
        }
    }
}

// ── Shared state ──

pub(crate) struct OrderBookState {
    pub asks: BookSide,
    pub bids: BookSide,
    pub last_price: f64,
    pub last_direction: PriceDirection,
    pub config: OrderBookConfig,

    // Flash state
    pub new_flashes: Vec<(f64, BookSideKind, f32)>,
    pub ghosts: Vec<(f64, BookSideKind, f64, f32)>,
    pub qty_flashes: Vec<(f64, BookSideKind, f32)>,
    pub sweeps: Vec<(f64, f64, BookSideKind, f32)>,

    pub hovered_row: Option<usize>,
    pub pending_events: Vec<OrderBookEvent>,

    /// Cached from theme during layout.
    pub row_h: f32,
    pub spread_h: f32,
}

impl OrderBookState {
    fn new(config: OrderBookConfig) -> Self {
        Self {
            asks: BookSide::new(),
            bids: BookSide::new(),
            last_price: 0.0,
            last_direction: PriceDirection::Unchanged,
            config,
            new_flashes: Vec::new(),
            ghosts: Vec::new(),
            qty_flashes: Vec::new(),
            sweeps: Vec::new(),
            hovered_row: None,
            pending_events: Vec::new(),
            row_h: 0.0,
            spread_h: 0.0,
        }
    }

    fn detect_changes(&mut self, side: BookSideKind, old: &[(f64, f64)], new: &[BookLevel]) {
        let new_set: Vec<(f64, f64)> = new.iter().map(|l| (l.price, l.quantity)).collect();
        let side_count = old.len().max(new.len()).max(1);

        // Consumed: in old, not in new
        let mut consumed = Vec::new();
        for &(price, qty) in old {
            if !new_set.iter().any(|&(p, _)| (p - price).abs() < 1e-12) {
                consumed.push((price, qty));
            }
        }
        let consumed_is_bulk = consumed.len() > side_count * 3 / 10;
        if !consumed_is_bulk {
            for &(price, qty) in &consumed {
                if !self
                    .ghosts
                    .iter()
                    .any(|(p, s, _, _)| (p - price).abs() < 1e-12 && *s == side)
                {
                    self.ghosts.push((price, side, qty, 1.0));
                }
            }
            if consumed.len() >= 2 {
                let min_p = consumed.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
                let max_p = consumed
                    .iter()
                    .map(|c| c.0)
                    .fold(f64::NEG_INFINITY, f64::max);
                self.sweeps.push((min_p, max_p, side, 1.0));
            }
        }

        // New: in new, not in old
        let mut new_levels = Vec::new();
        for &(price, _) in &new_set {
            if !old.iter().any(|&(p, _)| (p - price).abs() < 1e-12) {
                new_levels.push(price);
            }
        }
        let new_is_bulk = new_levels.len() > side_count * 3 / 10;
        if !new_is_bulk {
            for price in &new_levels {
                if !self
                    .new_flashes
                    .iter()
                    .any(|(p, s, _)| (p - *price).abs() < 1e-12 && *s == side)
                {
                    self.new_flashes.push((*price, side, 1.0));
                }
            }
        }

        // Quantity changes: threshold + dedup + bulk guard
        let mut qty_changes = Vec::new();
        let mut total_matched = 0usize;
        for &(price, new_qty) in &new_set {
            if let Some(&(_, old_qty)) = old.iter().find(|&&(p, _)| (p - price).abs() < 1e-12) {
                total_matched += 1;
                let rel = if old_qty.abs() > 1e-15 {
                    ((new_qty - old_qty) / old_qty).abs()
                } else {
                    0.0
                };
                if rel > 0.05 || (new_qty - old_qty).abs() > 0.01 {
                    qty_changes.push(price);
                }
            }
        }
        let qty_is_bulk = total_matched > 0 && qty_changes.len() > total_matched * 3 / 10;
        if !qty_is_bulk {
            for price in qty_changes {
                if let Some(existing) = self
                    .qty_flashes
                    .iter_mut()
                    .find(|(p, s, _)| (p - price).abs() < 1e-12 && *s == side)
                {
                    existing.2 = 1.0;
                } else {
                    self.qty_flashes.push((price, side, 1.0));
                }
            }
        }
    }

    fn decay_flashes(&mut self, dt: f32) {
        let decay_new = 1.0 / 0.15;
        let decay_ghost = 1.0 / 0.20;
        let decay_qty = 1.0 / 0.12;

        for f in &mut self.new_flashes {
            f.2 -= dt * decay_new;
        }
        self.new_flashes.retain(|f| f.2 > 0.0);

        for g in &mut self.ghosts {
            g.3 -= dt * decay_ghost;
        }
        self.ghosts.retain(|g| g.3 > 0.0);

        for f in &mut self.qty_flashes {
            f.2 -= dt * decay_qty;
        }
        self.qty_flashes.retain(|f| f.2 > 0.0);

        for s in &mut self.sweeps {
            s.3 -= dt * decay_ghost;
        }
        self.sweeps.retain(|s| s.3 > 0.0);
    }

    pub(crate) fn has_active_flashes(&self) -> bool {
        !self.new_flashes.is_empty()
            || !self.ghosts.is_empty()
            || !self.qty_flashes.is_empty()
            || !self.sweeps.is_empty()
    }

    pub(crate) fn push_event(&mut self, event: OrderBookEvent) {
        if self.pending_events.len() >= EVENT_QUEUE_CAPACITY {
            let overflow = self.pending_events.len() + 1 - EVENT_QUEUE_CAPACITY;
            self.pending_events.drain(..overflow);
        }
        self.pending_events.push(event);
    }
}

// ── Helper functions (used by both container and body) ──

pub(crate) fn find_flash(
    flashes: &[(f64, BookSideKind, f32)],
    price: f64,
    side: BookSideKind,
) -> f32 {
    flashes
        .iter()
        .find(|(p, s, _)| (p - price).abs() < 1e-12 && *s == side)
        .map(|(_, _, age)| *age)
        .unwrap_or(0.0)
}

pub(crate) fn format_price(config: &OrderBookConfig, price: f64) -> String {
    if let Some(ref fmt) = config.price_formatter {
        fmt(price)
    } else {
        format!("{:.2}", price)
    }
}

pub(crate) fn format_qty(config: &OrderBookConfig, qty: f64) -> String {
    if let Some(ref fmt) = config.quantity_formatter {
        fmt(qty)
    } else {
        format!("{:.5}", qty)
    }
}

pub(crate) fn format_total(config: &OrderBookConfig, total: f64) -> String {
    if let Some(ref fmt) = config.total_formatter {
        fmt(total)
    } else if let Some(ref fmt) = config.quantity_formatter {
        fmt(total)
    } else if total >= 1000.0 {
        format!("{:.2}K", total / 1000.0)
    } else {
        format!("{:.2}", total)
    }
}

// ── OrderBook container widget ──

/// SPEC-HAL-303: Tabular order book ladder widget.
///
/// Tree: OrderBook → ScrollArea → OrderBookBody.
/// INV-303-0: Projection-only. Does not own book state.
pub struct OrderBook {
    id: WidgetInternalId,
    state: Rc<RefCell<OrderBookState>>,
    update_rx: Option<Receiver<OrderBookUpdate>>,
    /// ScrollArea child NodeId (for programmatic scroll).
    scroll_area_id: Option<NodeId>,
    /// Manual scroll flag.
    manual_scroll: bool,
    /// Pending center-on-spread request (applied in layout).
    pending_center: bool,
    /// First data load triggers initial center.
    needs_initial_center: bool,
    /// Last body height (for centering calculation).
    last_body_h: f32,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            state: Rc::new(RefCell::new(
                OrderBookState::new(OrderBookConfig::default()),
            )),
            update_rx: None,
            scroll_area_id: None,
            manual_scroll: false,
            pending_center: false,
            needs_initial_center: true,
            last_body_h: 0.0,
        }
    }

    pub fn with_config(self, config: OrderBookConfig) -> Self {
        self.state.borrow_mut().config = config;
        self
    }

    /// Create the body widget. Shares state with this container via Rc.
    pub fn create_body(&self) -> OrderBookBody {
        OrderBookBody::new(self.state.clone())
    }

    pub fn with_update_channel(mut self) -> (Self, crossbeam_channel::Sender<OrderBookUpdate>) {
        let (tx, rx) = crossbeam_channel::bounded(UPDATE_CHANNEL_CAPACITY);
        self.update_rx = Some(rx);
        (self, tx)
    }

    /// Set the ScrollArea NodeId for programmatic scroll control.
    pub fn set_scroll_area(&mut self, id: NodeId) {
        self.scroll_area_id = Some(id);
    }

    pub fn set_asks(&mut self, asks: BookSide) {
        let mut state = self.state.borrow_mut();
        let old: Vec<(f64, f64)> = state
            .asks
            .levels
            .iter()
            .map(|l| (l.price, l.quantity))
            .collect();
        state.detect_changes(BookSideKind::Ask, &old, &asks.levels);
        state.asks = asks;
    }

    pub fn set_bids(&mut self, bids: BookSide) {
        let mut state = self.state.borrow_mut();
        let old: Vec<(f64, f64)> = state
            .bids
            .levels
            .iter()
            .map(|l| (l.price, l.quantity))
            .collect();
        state.detect_changes(BookSideKind::Bid, &old, &bids.levels);
        state.bids = bids;
    }

    pub fn set_book(&mut self, bids: BookSide, asks: BookSide) {
        let mut state = self.state.borrow_mut();
        if !state.bids.levels.is_empty() || !state.asks.levels.is_empty() {
            let old_bids: Vec<(f64, f64)> = state
                .bids
                .levels
                .iter()
                .map(|l| (l.price, l.quantity))
                .collect();
            let old_asks: Vec<(f64, f64)> = state
                .asks
                .levels
                .iter()
                .map(|l| (l.price, l.quantity))
                .collect();
            state.detect_changes(BookSideKind::Bid, &old_bids, &bids.levels);
            state.detect_changes(BookSideKind::Ask, &old_asks, &asks.levels);
        }
        state.bids = bids;
        state.asks = asks;
    }

    pub fn set_last_price(&mut self, price: f64, direction: PriceDirection) {
        let mut state = self.state.borrow_mut();
        state.last_price = price;
        state.last_direction = direction;
    }

    pub fn with_asks(mut self, asks: BookSide) -> Self {
        self.set_asks(asks);
        self
    }
    pub fn with_bids(mut self, bids: BookSide) -> Self {
        self.set_bids(bids);
        self
    }
    pub fn with_last_price(mut self, price: f64, direction: PriceDirection) -> Self {
        self.set_last_price(price, direction);
        self
    }

    pub fn take_events(&mut self) -> Vec<OrderBookEvent> {
        std::mem::take(&mut self.state.borrow_mut().pending_events)
    }

    pub fn mark_sweep(&mut self, min_price: f64, max_price: f64, side: BookSideKind) {
        self.state
            .borrow_mut()
            .sweeps
            .push((min_price, max_price, side, 1.0));
    }
}

impl Widget for OrderBook {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width;
        let h = constraints.max_height;

        {
            let mut state = self.state.borrow_mut();
            state.row_h = state.config.row_height;
            state.spread_h = state.config.row_height * 1.1;
        }

        self.last_body_h = h;

        // ScrollArea gets the full widget area
        if !children.is_empty() {
            let body_constraints = Constraints::new(w, w, h, h);
            _cx.layout_child(children[0], body_constraints);
            _cx.set_child_rect(children[0], RectF::new(0.0, 0.0, w, h));
        }

        // Center on spread if requested
        if self.pending_center {
            self.pending_center = false;
            if let Some(sa_id) = self.scroll_area_id {
                let state = self.state.borrow();
                let ask_count = state.asks.levels.len() as f32;
                let spread_y = ask_count * state.row_h;
                let target = (spread_y - h / 2.0).max(0.0);
                drop(state);
                if let Some(node) = _cx.tree.node_mut(sa_id) {
                    if let Some(sa) = node.widget.as_any_mut().downcast_mut::<ScrollArea>() {
                        sa.scroll_to_y(target);
                    }
                }
            }
        }

        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene
            .push_rect(rect, cx.theme.colors.surfaces.content, 0, None);
    }

    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        let mut next_bids = None;
        let mut next_asks = None;
        let mut next_last_price = None;

        if let Some(rx) = self.update_rx.as_ref() {
            while let Ok(update) = rx.try_recv() {
                match update {
                    OrderBookUpdate::Bids(bids) => next_bids = Some(bids),
                    OrderBookUpdate::Asks(asks) => next_asks = Some(asks),
                    OrderBookUpdate::Snapshot { bids, asks } => {
                        next_bids = Some(bids);
                        next_asks = Some(asks);
                    }
                    OrderBookUpdate::LastPrice { price, direction } => {
                        next_last_price = Some((price, direction));
                    }
                }
            }
        }

        let (bids_changed, asks_changed, topology_changed, last_price_changed) = {
            let state = self.state.borrow();
            let bids_changed = next_bids.as_ref().is_some_and(|bids| bids != &state.bids);
            let asks_changed = next_asks.as_ref().is_some_and(|asks| asks != &state.asks);
            let topology_changed = next_bids
                .as_ref()
                .is_some_and(|bids| bids.levels.len() != state.bids.levels.len())
                || next_asks
                    .as_ref()
                    .is_some_and(|asks| asks.levels.len() != state.asks.levels.len());
            let last_price_changed = next_last_price.is_some_and(|(price, direction)| {
                price != state.last_price || direction != state.last_direction
            });
            (
                bids_changed,
                asks_changed,
                topology_changed,
                last_price_changed,
            )
        };

        if !bids_changed {
            next_bids = None;
        }
        if !asks_changed {
            next_asks = None;
        }
        if !last_price_changed {
            next_last_price = None;
        }

        match (next_bids, next_asks) {
            (Some(bids), Some(asks)) => self.set_book(bids, asks),
            (Some(bids), None) => self.set_bids(bids),
            (None, Some(asks)) => self.set_asks(asks),
            (None, None) => {}
        }

        if let Some((price, direction)) = next_last_price {
            self.set_last_price(price, direction);
        }

        // Initial center on first data
        if self.needs_initial_center && !self.state.borrow().asks.levels.is_empty() {
            self.pending_center = true;
            self.needs_initial_center = false;
        }

        let had_active_flashes = self.state.borrow().has_active_flashes();
        self.state.borrow_mut().decay_flashes(dt);
        let flashes_finished = had_active_flashes && !self.state.borrow().has_active_flashes();

        if topology_changed {
            cx.invalidate_layout();
        } else if bids_changed || asks_changed || last_price_changed || flashes_finished {
            cx.invalidate_paint_subtree();
        }
    }

    fn needs_update(&self) -> bool {
        self.update_rx.is_some()
    }

    fn handle_event(&mut self, _cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        match event {
            Event::Key(ke) if ke.is_down && !ke.is_repeat => match ke.key_code {
                KeyCode::Space => {
                    self.manual_scroll = false;
                    self.pending_center = true;
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn wants_scroll(&self) -> bool {
        true
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "OrderBook"
    }
}

impl WidgetDowncast for OrderBook {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}
