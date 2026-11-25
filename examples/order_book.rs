// Order Book Demo — tabular bid/ask ladder with live mock data

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::model::order_book::{BookLevel, BookSide, LevelClassification};
use halcyon::prelude::*;
use halcyon::widgets::order_book::{OrderBook, OrderBookConfig, OrderBookUpdate, PriceDirection};
use halcyon::widgets::scroll_area::ScrollArea;
use std::sync::Arc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(AppConfig::new("Order Book Demo").with_size(400.0, 800.0));
    app.run(OrderBookScreen::new());
}

fn spawn_book_ticker(
    tx: crossbeam_channel::Sender<OrderBookUpdate>,
    mid_price: f64,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("book-ticker".into())
        .spawn(move || {
            let tick = 0.01;
            let mut rng = 99u64;
            let mut price = mid_price;

            // Build initial stable book
            let mut bid_qtys = Vec::new();
            let mut ask_qtys = Vec::new();
            for i in 0..30 {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                bid_qtys.push(((rng >> 33) as f64) / (u32::MAX as f64) * 0.7 + 0.01);
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                ask_qtys.push(((rng >> 33) as f64) / (u32::MAX as f64) * 0.7 + 0.01);
                let _ = i;
            }

            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Random walk the mid price (snap to tick grid)
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let r = ((rng >> 33) as f64) / (u32::MAX as f64);
                let direction = if r < 0.10 {
                    price += tick;
                    PriceDirection::Up
                } else if r < 0.20 {
                    price -= tick;
                    PriceDirection::Down
                } else {
                    PriceDirection::Unchanged
                };

                // Tweak only ~3-5 random levels per side
                for _ in 0..4 {
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let idx = ((rng >> 33) as usize) % 30;
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let delta = (((rng >> 33) as f64) / (u32::MAX as f64) - 0.5) * 0.15;
                    bid_qtys[idx] = (bid_qtys[idx] + delta).max(0.005);

                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let idx = ((rng >> 33) as usize) % 30;
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let delta = (((rng >> 33) as f64) / (u32::MAX as f64) - 0.5) * 0.15;
                    ask_qtys[idx] = (ask_qtys[idx] + delta).max(0.005);
                }

                let bids: Vec<BookLevel> = (0..30)
                    .map(|i| BookLevel {
                        price: price - tick * (i + 1) as f64,
                        quantity: bid_qtys[i],
                        cumulative: 0.0,
                        order_count: None,
                        classification: LevelClassification::Normal,
                        metadata: Vec::new(),
                    })
                    .collect();
                let asks: Vec<BookLevel> = (0..30)
                    .map(|i| BookLevel {
                        price: price + tick * (i + 1) as f64,
                        quantity: ask_qtys[i],
                        cumulative: 0.0,
                        order_count: None,
                        classification: LevelClassification::Normal,
                        metadata: Vec::new(),
                    })
                    .collect();

                if tx
                    .send(OrderBookUpdate::Snapshot {
                        bids: BookSide { levels: bids },
                        asks: BookSide { levels: asks },
                    })
                    .is_err()
                {
                    break;
                }

                if tx
                    .send(OrderBookUpdate::LastPrice { price, direction })
                    .is_err()
                {
                    break;
                }
            }
        })
        .expect("failed to spawn book ticker")
}

struct OrderBookScreen {
    needs_rebuild: bool,
    placeholder: Label,
    ticker_handle: Option<std::thread::JoinHandle<()>>,
}

impl OrderBookScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            ticker_handle: None,
        }
    }
}

impl Screen for OrderBookScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let mid = 66_422.14;
        let tick = 0.01;

        // Generate initial mock levels
        let mut bids = Vec::new();
        let mut rng = 42u64;
        for i in 0..30 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let r = ((rng >> 33) as f64) / (u32::MAX as f64);
            let price = mid - tick * (i + 1) as f64;
            let qty = 0.01 + r * 0.7;
            bids.push(BookLevel {
                price,
                quantity: qty,
                cumulative: 0.0,
                order_count: None,
                classification: LevelClassification::Normal,
                metadata: Vec::new(),
            });
        }

        let mut asks = Vec::new();
        for i in 0..30 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let r = ((rng >> 33) as f64) / (u32::MAX as f64);
            let price = mid + tick * (i + 1) as f64;
            let qty = 0.01 + r * 0.7;
            asks.push(BookLevel {
                price,
                quantity: qty,
                cumulative: 0.0,
                order_count: None,
                classification: LevelClassification::Normal,
                metadata: Vec::new(),
            });
        }

        let config = OrderBookConfig {
            price_formatter: Some(Arc::new(|p| format!("{:.2}", p))),
            quantity_formatter: Some(Arc::new(|q| format!("{:.5}", q))),
            max_visible_per_side: 25,
            ..Default::default()
        };

        let order_book = OrderBook::new()
            .with_config(config)
            .with_bids(BookSide { levels: bids })
            .with_asks(BookSide { levels: asks })
            .with_last_price(mid, PriceDirection::Up);

        let body = order_book.create_body();
        let (order_book, update_tx) = order_book.with_update_channel();

        if self.ticker_handle.is_none() {
            self.ticker_handle = Some(spawn_book_ticker(update_tx, mid));
        }

        // Wire tree: OrderBook → ScrollArea → OrderBookBody
        let ob_id = cx.mount_child(Box::new(order_book));

        let scroll_area = ScrollArea::new_unwired().with_scrollbars(false);
        let sa_id = cx.tree.add_child(ob_id, Box::new(scroll_area), None, 0);

        let body_id = cx.tree.add_child(sa_id, Box::new(body), None, 0);

        // Wire ScrollArea content
        if let Some(node) = cx.tree.node_mut(sa_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }

        // Tell OrderBook about ScrollArea for auto-center
        if let Some(node) = cx.tree.node_mut(ob_id) {
            if let Some(ob) = node.widget_mut().as_any_mut().downcast_mut::<OrderBook>() {
                ob.set_scroll_area(sa_id);
            }
        }

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
