// Depth Demo — candle chart + order book depth sidebar
//
// Side-by-side layout: candle chart (left) with depth sidebar (right).
// Both share a Y axis (price). Depth bars show bid/ask quantity.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::NodeId;
use halcyon::plot::{
    AxisAuthority, BookLevel, BookSide, CandleSeries, DepthBarSeries, LabelConfig,
    LevelClassification, Plot, PlotRenderPool, Probe, RenderCandle, ViewState,
};
use halcyon::prelude::*;
use halcyon::replace_widget;
use halcyon::widgets::split_pane::SplitPane;
use std::sync::{Arc, Mutex};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(AppConfig::new("Depth Demo").with_size(1200.0, 700.0));
    app.run(DepthScreen::new());
}

fn generate_mock_candles(count: usize, base_price: f64, interval: f64) -> Vec<RenderCandle> {
    let mut candles = Vec::with_capacity(count);
    let mut price = base_price;
    let mut rng_state = 12345u64;

    for i in 0..count {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let r = ((rng_state >> 33) as f64) / (u32::MAX as f64) - 0.5;

        let open = price;
        let drift = (base_price - price) * 0.02;
        price += r * 0.5 + drift;
        let close = price;
        let high = open.max(close) + r.abs() * 0.3;
        let low = open.min(close) - r.abs() * 0.3;

        candles.push(RenderCandle {
            x_open: i as f64 * interval,
            x_close: (i as f64 + 0.8) * interval,
            high,
            low,
            body_top: open.max(close),
            body_bottom: open.min(close),
            is_up: close >= open,
            volume: 100.0 + r.abs() * 500.0,
            body_color_override: None,
        });
    }
    candles
}

fn generate_mock_book(
    mid_price: f64,
    tick_size: f64,
    levels_per_side: usize,
) -> (BookSide, BookSide) {
    let mut bids = Vec::new();
    for i in 0..levels_per_side {
        let price = mid_price - tick_size * (i + 1) as f64;
        let qty = 100.0 + (i as f64).sin().abs() * 500.0;
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
    for i in 0..levels_per_side {
        let price = mid_price + tick_size * (i + 1) as f64;
        let qty = 100.0 + (i as f64 * 1.3).sin().abs() * 500.0;
        asks.push(BookLevel {
            price,
            quantity: qty,
            cumulative: 0.0,
            order_count: None,
            classification: LevelClassification::Normal,
            metadata: Vec::new(),
        });
    }

    // Classify a few levels
    if bids.len() > 5 {
        bids[5].classification = LevelClassification::Wall;
        bids[5].quantity = 800.0;
    }
    if bids.len() > 15 {
        bids[15].classification = LevelClassification::OwnOrder;
    }
    if asks.len() > 8 {
        asks[8].classification = LevelClassification::Iceberg {
            inferred_total: 1200.0,
        };
    }
    if asks.len() > 3 {
        asks[3].classification = LevelClassification::SuspectedSpoof;
        asks[3].quantity = 900.0;
    }

    (BookSide { levels: bids }, BookSide { levels: asks })
}

struct DepthScreen {
    needs_rebuild: bool,
    placeholder: Label,
    render_pool: Arc<PlotRenderPool>,
}

impl DepthScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            render_pool: Arc::new(PlotRenderPool::new()),
        }
    }
}

impl Screen for DepthScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let gpu = cx
            .app
            .gpu_resources()
            .expect("GpuResources not available")
            .clone();

        // Candle data
        let candles = generate_mock_candles(500, 100.0, 60.0);
        let last_price = candles
            .last()
            .map(|c| (c.body_top + c.body_bottom) / 2.0)
            .unwrap_or(100.0);

        // Book data around last price
        let (bid_side, ask_side) = generate_mock_book(last_price, 0.01, 50);
        let max_qty = bid_side.max_quantity().max(ask_side.max_quantity());

        // Shared Y axis
        let y_lo = candles.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        let y_hi = candles
            .iter()
            .map(|c| c.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let y_pad = (y_hi - y_lo) * 0.15;
        let shared_y = Arc::new(Mutex::new(AxisAuthority::new(y_lo - y_pad, y_hi + y_pad)));
        let shared_probe = Arc::new(Mutex::new(Probe::new()));

        // Horizontal split: 80% candles, 20% depth
        let outer = cx.mount_child(Box::new(
            SplitPane::horizontal(NodeId(0), NodeId(0)).initial_ratio(0.8),
        ));
        let tree = &mut cx.tree;

        // Left: Candle chart
        let visible_count = 100usize;
        let n = candles.len();
        let start = if n > visible_count {
            n - visible_count
        } else {
            0
        };
        let x_min = candles[start].x_open;
        let x_max = candles.last().unwrap().x_close + 60.0 * 2.0;
        let mut vs = ViewState::new(x_min, x_max, y_lo - y_pad, y_hi + y_pad);
        vs.follow_latest = true;

        let mut candle_series = CandleSeries::new();
        candle_series.set_candles(candles);

        let label_config = LabelConfig {
            font_size: 11.0,
            y_tick_count: 5,
            x_tick_count: 6,
            y_formatter: None,
            x_formatter: Some(Arc::new(|ctx: halcyon::plot::axis::AxisFormatContext| {
                let val = ctx.value;
                let total_mins = (val / 60.0) as i64;
                let hours = total_mins / 60;
                let mins = total_mins % 60;
                if hours > 0 {
                    format!("{}h{:02}", hours, mins)
                } else {
                    format!("{}m", mins)
                }
            })),
            ..LabelConfig::default()
        };

        let candle_plot = Plot::new()
            .with_gpu_resources(gpu.clone())
            .with_render_pool(self.render_pool.clone())
            .with_shared_y(shared_y.clone())
            .with_probe(shared_probe.clone())
            .add_series(Box::new(candle_series))
            .with_view_state(vs)
            .with_label_config(Some(label_config))
            .with_title("MOCK \u{00b7} 1m");

        let candle_node = tree.add_child(outer, Box::new(candle_plot), None, 0);

        // Right: Depth sidebar
        let mut depth = DepthBarSeries::new();
        depth.set_both(bid_side, ask_side);

        let depth_plot = Plot::new()
            .with_gpu_resources(gpu)
            .with_render_pool(self.render_pool.clone())
            .with_shared_y(shared_y)
            .with_probe(shared_probe)
            .add_series(Box::new(depth))
            .with_view_state(ViewState::new(
                -max_qty * 1.1,
                max_qty * 1.1,
                y_lo - y_pad,
                y_hi + y_pad,
            ))
            .without_labels()
            .with_title("Depth");

        let depth_node = tree.add_child(outer, Box::new(depth_plot), None, 1);

        replace_widget!(
            tree,
            outer,
            SplitPane::horizontal(candle_node, depth_node).initial_ratio(0.8)
        );

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
