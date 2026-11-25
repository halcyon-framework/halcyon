// Plot Live Demo — real-time candle formation via data channel
//
// Single chart with 200 candles of history. A background ticker thread
// pushes DataUpdate messages every 50ms. Every 2 seconds a new candle
// opens. An EMA-20 line recomputes on each tick. Plot drains the
// channel in Widget::update() and auto-scales Y.

#![allow(unused_imports, unused_variables)]

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::plot::{
    BarPoint, BarSeries, BarStyle, CandleSeries, DataBounds, DataUpdate, LabelConfig, LineSeries,
    LineStyle, Plot, PlotBezier, PlotCircle, PlotLine, PlotLineStyle, PlotPath, PlotRegion,
    PlotRenderPool, PlotRouter, PlotStyledPath, PlotTag, PriceLevel, PriceLevelLine, RenderCandle,
    StyledSegment, TagAnchor, TagLine, ViewState,
};
use halcyon::prelude::*;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Plot Live Demo ===");
    println!("Real-time candle formation via data channel");
    println!("Background thread pushes updates every 50ms");
    println!("New candle every 2 seconds");
    println!("=======================");

    let app = App::new(AppConfig::new("Plot Live Demo").with_size(1200.0, 700.0));
    app.run(LiveScreen::new());
}

// -- PRNG ----------------------------------------------------------------

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn rand_f64(state: &mut u64) -> f64 {
    (splitmix64(state) >> 11) as f64 / ((1u64 << 53) as f64)
}

// -- Live state ----------------------------------------------------------

const CANDLE_INTERVAL: f64 = 60.0;
const CANDLE_PERIOD_MS: u64 = 650;
const TICK_INTERVAL_MS: u64 = 16;
const HISTORY_COUNT: usize = 200;
const EMA_PERIOD: usize = 20;

// Series indices in the Plot (order they are added via add_series):
const SERIES_VOL: usize = 0;
const SERIES_CANDLE: usize = 1;
const SERIES_EMA: usize = 2;

struct LiveState {
    candles: Vec<RenderCandle>,
    volumes: Vec<BarPoint>,
    rng: u64,
    last_candle: Instant,
    price: f64,
}

impl LiveState {
    fn new() -> Self {
        let mut rng: u64 = 0xDEAD_BEEF_CAFE_1234;
        let mut price = 100.0;
        let mut candles = Vec::with_capacity(HISTORY_COUNT);

        for i in 0..HISTORY_COUNT {
            let x_open = i as f64 * CANDLE_INTERVAL;
            let x_close = x_open + CANDLE_INTERVAL;

            let r = rand_f64(&mut rng);
            let volatility = 0.005 + rand_f64(&mut rng) * 0.015;
            let open = price;
            let close = open * (1.0 + (r - 0.5) * volatility);
            let body_top = open.max(close);
            let body_bottom = open.min(close);
            let body_size = (body_top - body_bottom).abs().max(price * 0.0001);
            let high = body_top + body_size * rand_f64(&mut rng) * 2.0;
            let low = body_bottom - body_size * rand_f64(&mut rng) * 2.0;

            candles.push(RenderCandle {
                x_open,
                x_close,
                high,
                low,
                body_top,
                body_bottom,
                is_up: close >= open,
                volume: 0.0,
                body_color_override: None,
            });

            price = close;
        }

        let volumes = Self::compute_volumes(&candles, &mut rng);

        let now = Instant::now();
        Self {
            candles,
            volumes,
            rng,
            last_candle: now,
            price,
        }
    }

    fn compute_volumes(candles: &[RenderCandle], _rng: &mut u64) -> Vec<BarPoint> {
        candles
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let idx = i as u64;
                let mut z1 = idx
                    .wrapping_mul(0x9E3779B97F4A7C15)
                    .wrapping_add(0x6A09E667F3BCC908);
                z1 = (z1 ^ (z1 >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                z1 = (z1 ^ (z1 >> 27)).wrapping_mul(0x94D049BB133111EB);
                z1 = z1 ^ (z1 >> 31);

                let mut z2 = idx
                    .wrapping_mul(0x517CC1B727220A95)
                    .wrapping_add(0x9E3779B97F4A7C15);
                z2 = (z2 ^ (z2 >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                z2 = (z2 ^ (z2 >> 27)).wrapping_mul(0x94D049BB133111EB);
                z2 = z2 ^ (z2 >> 31);

                let rand_vol = (z1 % 1000000) as f64 / 1000000.0;
                let rand_ratio = (z2 % 1000000) as f64 / 1000000.0;

                let vol = 0.1 + rand_vol * 0.9;
                let buy_ratio = if c.is_up {
                    0.4 + rand_ratio * 0.5
                } else {
                    0.1 + rand_ratio * 0.5
                };

                let (r, g, b) = if buy_ratio >= 0.5 {
                    let t = (buy_ratio - 0.5) * 2.0;
                    (0.4 * (1.0 - t), 0.3 + 0.5 * t, 0.4 * (1.0 - t) * 0.3)
                } else {
                    let t = buy_ratio * 2.0;
                    (0.7 - 0.3 * t, 0.1 + 0.2 * t, 0.1 * t * 0.3)
                };

                let mid_x = (c.x_open + c.x_close) / 2.0;
                BarPoint {
                    x: mid_x,
                    y: vol,
                    color: Some(Color::from_linear(r as f32, g as f32, b as f32, 0.6)),
                }
            })
            .collect()
    }

    fn compute_ema(candles: &[RenderCandle]) -> Vec<halcyon::plot::LinePoint> {
        if candles.is_empty() {
            return Vec::new();
        }
        let k = 2.0 / (EMA_PERIOD as f64 + 1.0);
        let mut ema = (candles[0].body_top + candles[0].body_bottom) / 2.0;
        candles
            .iter()
            .map(|c| {
                let mid = (c.body_top + c.body_bottom) / 2.0;
                ema = mid * k + ema * (1.0 - k);
                halcyon::plot::LinePoint {
                    x: (c.x_open + c.x_close) / 2.0,
                    y: ema,
                }
            })
            .collect()
    }

    fn view_state_for_latest(&self) -> ViewState {
        let visible_count = 100usize;
        let n = self.candles.len();
        let start = if n > visible_count {
            n - visible_count
        } else {
            0
        };

        let x_min = self.candles[start].x_open;
        let x_max = self.candles.last().unwrap().x_close + CANDLE_INTERVAL * 2.0;

        let y_min = self.candles[start..]
            .iter()
            .map(|c| c.low)
            .fold(f64::INFINITY, f64::min);
        let y_max = self.candles[start..]
            .iter()
            .map(|c| c.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let y_pad = (y_max - y_min) * 0.08;

        let mut vs = ViewState::new(x_min, x_max, y_min - y_pad, y_max + y_pad);
        vs.follow_latest = true;
        vs
    }

    fn data_x_max(&self) -> f64 {
        self.candles
            .last()
            .map(|c| c.x_close)
            .unwrap_or(CANDLE_INTERVAL)
    }
}

// -- Ticker thread -------------------------------------------------------

fn spawn_ticker(
    tx: crossbeam_channel::Sender<DataUpdate>,
    initial_state: LiveState,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("live-ticker".into())
        .spawn(move || {
            let mut state = initial_state;

            loop {
                std::thread::sleep(std::time::Duration::from_millis(TICK_INTERVAL_MS));

                let now = Instant::now();
                let since_candle = now.duration_since(state.last_candle).as_millis() as u64;

                // -- New candle period? --
                if since_candle >= CANDLE_PERIOD_MS {
                    state.last_candle = now;
                    let last = state.candles.last().unwrap();
                    let x_open = last.x_close;
                    let x_close = x_open + CANDLE_INTERVAL;
                    let open = state.price;
                    let new_candle = RenderCandle {
                        x_open,
                        x_close,
                        high: open,
                        low: open,
                        body_top: open,
                        body_bottom: open,
                        is_up: true,
                        volume: 0.0,
                        body_color_override: None,
                    };
                    state.candles.push(new_candle.clone());
                    if tx
                        .send(DataUpdate::AppendCandle {
                            series_index: SERIES_CANDLE,
                            candle: new_candle,
                        })
                        .is_err()
                    {
                        break;
                    }

                    // New volume bar with gradient encoding
                    let bar_idx = state.candles.len() as u64 - 1;
                    let mut z1 = bar_idx
                        .wrapping_mul(0x9E3779B97F4A7C15)
                        .wrapping_add(0x6A09E667F3BCC908);
                    z1 = (z1 ^ (z1 >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                    z1 = (z1 ^ (z1 >> 27)).wrapping_mul(0x94D049BB133111EB);
                    z1 = z1 ^ (z1 >> 31);
                    let rand_vol = (z1 % 1000000) as f64 / 1000000.0;
                    let vol = 0.1 + rand_vol * 0.9;
                    let mid_x = (x_open + x_close) / 2.0;
                    // Initial color — will be updated by direction change
                    let new_vol = BarPoint {
                        x: mid_x,
                        y: vol,
                        color: Some(Color::from_linear(0.3, 0.4, 0.1, 0.6)),
                    };
                    state.volumes.push(new_vol);
                    // Replace all volume bars (simplest — bar count changes)
                    if tx
                        .send(DataUpdate::ReplaceBarPoints {
                            series_index: SERIES_VOL,
                            points: state.volumes.clone(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }

                // -- Random walk price --
                let r = rand_f64(&mut state.rng);
                let step = state.price * 0.002 * (r - 0.5);
                state.price += step;

                // -- Update working candle --
                let candle = state.candles.last_mut().unwrap();
                let open = if candle.is_up {
                    candle.body_bottom
                } else {
                    candle.body_top
                };
                let close = state.price;
                let body_top = open.max(close);
                let body_bottom = open.min(close);
                candle.body_top = body_top;
                candle.body_bottom = body_bottom;
                candle.is_up = close >= open;
                if close > candle.high {
                    candle.high = close;
                }
                if close < candle.low {
                    candle.low = close;
                }

                // Capture values before dropping mutable borrow
                let candle_is_up = candle.is_up;
                let candle_snapshot = candle.clone();
                // candle (&mut ref) no longer used after this point

                let bar_idx = (state.candles.len() - 1) as u64;

                if tx
                    .send(DataUpdate::UpdateLastCandle {
                        series_index: SERIES_CANDLE,
                        candle: candle_snapshot,
                    })
                    .is_err()
                {
                    break;
                }

                // -- Update volume bar color based on buy ratio gradient --
                if let Some(vol) = state.volumes.last_mut() {
                    let mut z2 = bar_idx
                        .wrapping_mul(0x517CC1B727220A95)
                        .wrapping_add(0x9E3779B97F4A7C15);
                    z2 = (z2 ^ (z2 >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                    z2 = (z2 ^ (z2 >> 27)).wrapping_mul(0x94D049BB133111EB);
                    z2 = z2 ^ (z2 >> 31);
                    let rand_ratio = (z2 % 1000000) as f64 / 1000000.0;
                    let buy_ratio = if candle_is_up {
                        0.4 + rand_ratio * 0.5
                    } else {
                        0.1 + rand_ratio * 0.5
                    };
                    let (rv, gv, bv) = if buy_ratio >= 0.5 {
                        let t = (buy_ratio - 0.5) * 2.0;
                        (0.4 * (1.0 - t), 0.3 + 0.5 * t, 0.4 * (1.0 - t) * 0.3)
                    } else {
                        let t = buy_ratio * 2.0;
                        (0.7 - 0.3 * t, 0.1 + 0.2 * t, 0.1 * t * 0.3)
                    };
                    vol.color = Some(Color::from_linear(rv as f32, gv as f32, bv as f32, 0.6));
                    if tx
                        .send(DataUpdate::UpdateLastBarPoint {
                            series_index: SERIES_VOL,
                            point: *vol,
                        })
                        .is_err()
                    {
                        break;
                    }
                }

                // -- Recompute EMA --
                let ema_points = LiveState::compute_ema(&state.candles);
                if tx
                    .send(DataUpdate::ReplaceLinePoints {
                        series_index: SERIES_EMA,
                        points: ema_points,
                    })
                    .is_err()
                {
                    break;
                }

                // -- Update last price marker --
                if tx
                    .send(DataUpdate::AddPriceLevel(PriceLevel {
                        id: 10,
                        y: state.price,
                        color: Color::WHITE.with_alpha(0.5),
                        line_style: PriceLevelLine::Dotted,
                        tag: vec![TagLine::new(format!("{:.2}", state.price))],
                        draggable: false,
                        ..Default::default()
                    }))
                    .is_err()
                {
                    break;
                }
            }
        })
        .expect("failed to spawn ticker thread")
}

// -- Screen --------------------------------------------------------------

struct LiveScreen {
    needs_rebuild: bool,
    placeholder: Label,
    render_pool: Arc<PlotRenderPool>,
    // Keep sender alive so the channel stays open.
    // The ticker thread exits when the sender is dropped (send returns Err).
    _data_tx: Option<crossbeam_channel::Sender<DataUpdate>>,
    _ticker_handle: Option<std::thread::JoinHandle<()>>,
}

impl LiveScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            render_pool: Arc::new(PlotRenderPool::new()),
            _data_tx: None,
            _ticker_handle: None,
        }
    }
}

impl Screen for LiveScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let gpu = cx
            .app
            .gpu_resources()
            .expect("GpuResources not available")
            .clone();

        let live = LiveState::new();
        let view_state = live.view_state_for_latest();
        let data_x_max = live.data_x_max();

        let mut candle_series = CandleSeries::new();
        candle_series.set_candles(live.candles.clone());

        let mut vol_bars = BarSeries::new()
            .with_style(BarStyle {
                color: Color::from_srgb(0.3, 0.5, 1.0, 0.4),
                width_fraction: 0.8,
                baseline: 0.0,
                viewport_fraction: Some(0.12),
            })
            .with_z_order(50)
            .with_auto_scale_y(false);
        vol_bars.set_points(live.volumes.clone());

        let mut ema_line = LineSeries::new()
            .with_style(LineStyle {
                color: Color::from_hex(0xE6B422).with_alpha(0.9),
                width: 1.5,
            })
            .with_z_order(10);
        let ema_points = LiveState::compute_ema(&live.candles);
        ema_line.set_points(ema_points);

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

        let (plot, data_tx) = Plot::new()
            .with_gpu_resources(gpu)
            .with_view_state(view_state)
            .with_data_bounds(DataBounds {
                x_min: None,
                x_max: None,
                y_min: None,
                y_max: None,
            })
            .with_render_pool(self.render_pool.clone())
            .add_series(Box::new(vol_bars))
            .add_series(Box::new(candle_series))
            .add_series(Box::new(ema_line))
            .with_label_config(Some(label_config))
            .with_title("MOCK \u{00b7} 1s")
            .with_price_level(PriceLevel {
                id: 1,
                y: 101.0,
                color: Color::from_hex(0x2ECC71).with_alpha(0.85),
                line_style: PriceLevelLine::Dashed,
                tag: vec![
                    TagLine::new("BUY 101.00"),
                    TagLine::new("Grid-A #1").dimmed(),
                ],
                draggable: true,
                ..Default::default()
            })
            .with_price_level(PriceLevel {
                id: 2,
                y: 98.5,
                color: Color::from_hex(0xF05454).with_alpha(0.85),
                line_style: PriceLevelLine::Solid,
                tag: vec![
                    TagLine::new("SELL 98.50"),
                    TagLine::new("Stop Loss").dimmed(),
                ],
                draggable: true,
                ..Default::default()
            })
            .with_price_level(PriceLevel {
                id: 10,
                y: live.price,
                color: Color::WHITE.with_alpha(0.5),
                line_style: PriceLevelLine::Dotted,
                tag: vec![TagLine::new(format!("{:.2}", live.price))],
                draggable: false,
                ..Default::default()
            })
            .with_tag(PlotTag {
                id: 100,
                x: 5400.0,
                y: 100.5,
                tag: vec![TagLine::new("Entry"), TagLine::new("100.50").dimmed()],
                color: Color::from_hex(0x2ECC71).with_alpha(0.9),
                anchor: TagAnchor::Above,
            })
            .with_tag(PlotTag {
                id: 101,
                x: 9000.0,
                y: 99.0,
                tag: vec![TagLine::new("Signal: RSI Div")],
                color: Color::from_hex(0xE6B422).with_alpha(0.9),
                anchor: TagAnchor::Below,
            })
            .with_region(PlotRegion {
                id: 1000,
                x_min: None,
                x_max: None,
                y_min: Some(99.0),
                y_max: Some(101.0),
                fill_color: Color::from_srgb(0.3, 0.5, 1.0, 0.06),
                border_color: Some(Color::from_srgb(0.3, 0.5, 1.0, 0.15)),
                label: Some("Active Range".into()),
                group: Some("ranges".into()),
            })
            .with_circle(PlotCircle {
                id: 1,
                x: 500.0,
                y: 100.5,
                radius: 12.0,
                ..PlotCircle::default()
            })
            .with_circle(PlotCircle {
                id: 2,
                x: 800.0,
                y: 99.0,
                radius: 8.0,
                fill_color: Color::from_hex(0xE74C3C).with_alpha(0.3),
                border_color: Color::from_hex(0xE74C3C).with_alpha(0.9),
                border_width: 1.0,
                group: None,
                ..PlotCircle::default()
            })
            .with_line(PlotLine {
                id: 100,
                x1: 200.0,
                y1: 99.0,
                x2: 600.0,
                y2: 101.0,
                color: Color::from_hex(0xF1C40F).with_alpha(0.8),
                width: 1.5,
                line_style: PlotLineStyle::Dashed {
                    dash: 6.0,
                    gap: 3.0,
                },
                arrow_end: true,
                ..PlotLine::default()
            })
            .with_line(PlotLine {
                id: 101,
                x1: 400.0,
                y1: 102.0,
                x2: 800.0,
                y2: 98.0,
                color: Color::from_hex(0xE74C3C).with_alpha(0.7),
                width: 1.0,
                arrow_start: true,
                arrow_end: true,
                ..PlotLine::default()
            })
            .with_bezier({
                let mut b = PlotBezier::arc(200, 300.0, 100.0, 700.0, 100.5, 0.3);
                b.color = Color::from_hex(0x9B59B6).with_alpha(0.8);
                b.width = 1.5;
                b.arrow_end = true;
                b
            })
            .with_bezier({
                let mut b = PlotBezier::arc(201, 500.0, 101.5, 900.0, 99.5, -0.25);
                b.color = Color::from_hex(0x1ABC9C).with_alpha(0.7);
                b.width = 1.0;
                b.arrow_start = true;
                b.arrow_end = true;
                b
            })
            .with_styled_path(PlotStyledPath {
                id: 300,
                segments: vec![
                    // Placed (gray, dashed)
                    StyledSegment {
                        x: 250.0,
                        y: 100.5,
                        color: Color::from_hex(0x888888).with_alpha(0.6),
                        width: 1.0,
                        line_style: PlotLineStyle::Dashed {
                            dash: 4.0,
                            gap: 2.0,
                        },
                    },
                    // Partially filled (blue, solid)
                    StyledSegment {
                        x: 400.0,
                        y: 100.8,
                        color: Color::from_hex(0x3498DB).with_alpha(0.8),
                        width: 1.5,
                        line_style: PlotLineStyle::Solid,
                    },
                    // Fully filled (green, solid, thicker)
                    StyledSegment {
                        x: 550.0,
                        y: 101.2,
                        color: Color::from_hex(0x2ECC71).with_alpha(0.9),
                        width: 2.0,
                        line_style: PlotLineStyle::Solid,
                    },
                    // Closed (dim, dotted)
                    StyledSegment {
                        x: 700.0,
                        y: 100.0,
                        color: Color::from_hex(0x555555).with_alpha(0.4),
                        width: 1.0,
                        line_style: PlotLineStyle::Dotted { dot: 2.0, gap: 2.0 },
                    },
                ],
                group: Some("orders".into()),
            })
            // Manual PlotPath (triangle shape)
            .with_path(PlotPath {
                id: 400,
                points: vec![(300.0, 99.0), (450.0, 101.5), (600.0, 99.5)],
                color: Color::from_hex(0xF39C12).with_alpha(0.7),
                width: 1.0,
                closed: true,
                ..PlotPath::default()
            })
            .with_tool(Box::new(halcyon::plot::tool::MeasureTool::new()))
            .with_tool(Box::new(halcyon::plot::tool::HorizontalLineTool::new()))
            .with_tool(Box::new(halcyon::plot::tool::DrawRectTool::new()))
            .with_data_channel();

        // Spawn ticker thread that pushes updates via data_tx
        let ticker_handle = spawn_ticker(data_tx.clone(), live);
        self._data_tx = Some(data_tx);
        self._ticker_handle = Some(ticker_handle);

        // Mount as the sole child
        cx.mount_child(Box::new(plot));

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
