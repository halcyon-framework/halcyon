use halcyon::core::app::ScreenFrameContext;
use halcyon::core::layout::FlexParams;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::NodeId;
use halcyon::model::order_book::{BookLevel, BookSide, LevelClassification};
use halcyon::prelude::*;
use halcyon::widgets::order_book::{OrderBook, OrderBookConfig, OrderBookUpdate, PriceDirection};
use halcyon::widgets::scroll_area::ScrollArea;
use std::sync::Arc;

use super::*;

// Background mock feed: random-walks the mid price and per-level quantities and
// pushes Snapshot + LastPrice updates every 8ms. Detached — it self-terminates
// when the OrderBook (its receiver) is dropped on tab switch.
//
// vs examples/order_book.rs: that ticker used `rng >> 33` (31 bits) over
// `u32::MAX` (32 bits), so the unit float maxed at ~0.5; in the delta
// `(r - 0.5) * 0.15` that forced delta <= 0 and every quantity decayed to the
// 0.005 floor, flatlining the book. `rng >> 32` makes r span [0, 1) so the delta
// is symmetric and the feed stays live indefinitely.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_book_ticker(
    tx: crossbeam_channel::Sender<OrderBookUpdate>,
    mid_price: f64,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("showcase-book-ticker".into())
        .spawn(move || {
            let tick = 0.01;
            let mut rng = 99u64;
            let mut price = mid_price;

            let mut bid_qtys = Vec::new();
            let mut ask_qtys = Vec::new();
            for _ in 0..30 {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                bid_qtys.push(((rng >> 32) as f64) / (u32::MAX as f64) * 0.7 + 0.01);
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                ask_qtys.push(((rng >> 32) as f64) / (u32::MAX as f64) * 0.7 + 0.01);
            }

            loop {
                std::thread::sleep(std::time::Duration::from_millis(8));

                // Random walk the mid price (snap to tick grid)
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let r = ((rng >> 32) as f64) / (u32::MAX as f64);
                let direction = if r < 0.10 {
                    price += tick;
                    PriceDirection::Up
                } else if r < 0.20 {
                    price -= tick;
                    PriceDirection::Down
                } else {
                    PriceDirection::Unchanged
                };

                // Tweak ~4 random levels per side
                for _ in 0..4 {
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let idx = ((rng >> 32) as usize) % 30;
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let delta = (((rng >> 32) as f64) / (u32::MAX as f64) - 0.5) * 0.15;
                    bid_qtys[idx] = (bid_qtys[idx] + delta).max(0.005);

                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let idx = ((rng >> 32) as usize) % 30;
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let delta = (((rng >> 32) as f64) / (u32::MAX as f64) - 0.5) * 0.15;
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
        .expect("failed to spawn showcase book ticker")
}

impl ShowcaseScreen {
    pub(super) fn build_order_book(&self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        let mid = 66_422.14;
        let tick = 0.01;

        // Initial book (overwritten by the feed within ~8ms).
        let mut bids = Vec::new();
        let mut rng = 42u64;
        for i in 0..30 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let r = ((rng >> 32) as f64) / (u32::MAX as f64);
            bids.push(BookLevel {
                price: mid - tick * (i + 1) as f64,
                quantity: 0.01 + r * 0.7,
                cumulative: 0.0,
                order_count: None,
                classification: LevelClassification::Normal,
                metadata: Vec::new(),
            });
        }

        let mut asks = Vec::new();
        for i in 0..30 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let r = ((rng >> 32) as f64) / (u32::MAX as f64);
            asks.push(BookLevel {
                price: mid + tick * (i + 1) as f64,
                quantity: 0.01 + r * 0.7,
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

        // Detached live feed; exits when the OrderBook is dropped on tab switch.
        #[cfg(not(target_arch = "wasm32"))]
        let _ = spawn_book_ticker(update_tx, mid);
        #[cfg(target_arch = "wasm32")]
        {
            drop(update_tx);
            cx.tree.add_child(
                parent,
                Box::new(Label::new(
                    "Initial book only: the mock live feed requires native worker threads.",
                )),
                None,
                0,
            );
        }

        // Wire tree: OrderBook -> ScrollArea -> OrderBookBody, filling the tab.
        let ob_id = cx.tree.add_child(parent, Box::new(order_book), None, 0);
        cx.tree.set_flex(ob_id, FlexParams::flexible(1.0));

        let scroll_area = ScrollArea::new_unwired().with_scrollbars(false);
        let sa_id = cx.tree.add_child(ob_id, Box::new(scroll_area), None, 0);

        let body_id = cx.tree.add_child(sa_id, Box::new(body), None, 0);

        if let Some(node) = cx.tree.node_mut(sa_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }

        if let Some(node) = cx.tree.node_mut(ob_id) {
            if let Some(ob) = node.widget_mut().as_any_mut().downcast_mut::<OrderBook>() {
                ob.set_scroll_area(sa_id);
            }
        }
    }
}
