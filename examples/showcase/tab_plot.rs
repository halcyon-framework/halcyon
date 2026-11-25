use halcyon::core::app::ScreenFrameContext;
use halcyon::core::layout::FlexParams;
use halcyon::core::paint::Color;
use halcyon::core::widget_tree::NodeId;
use halcyon::model::order_book::{BookLevel, BookSide, LevelClassification};
use halcyon::plot::{
    BarPoint, BarSeries, CandleSeries, DataBounds, DataUpdate, LabelConfig, LineSeries,
    MarketDetailLink, Plot, PlotBezier, PlotCircle, PlotLine, PlotLineStyle, PlotPath, PlotRegion,
    PlotStyledPath, PlotTag, PriceLevel, PriceLevelLine, RenderCandle, StyledSegment, TagAnchor,
    TagLine, ViewState,
};
use halcyon::widgets::{AlignedOrderBook, AlignedOrderBookRowLayout};
use std::sync::Arc;
use std::time::Instant;

use super::*;

const CANDLE_INTERVAL: f64 = 60.0;
const CANDLE_PERIOD_MS: u64 = 650;
const TICK_INTERVAL_MS: u64 = 16;
const HISTORY_COUNT: usize = 200;
const EMA_PERIOD: usize = 20;
const VOLUME_BAR_ALPHA: f32 = 0.82;

const SERIES_VOL: usize = 0;
const SERIES_CANDLE: usize = 1;
const SERIES_EMA: usize = 2;

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

struct LiveState {
    candles: Vec<RenderCandle>,
    volumes: Vec<BarPoint>,
    ema: Vec<halcyon::plot::LinePoint>,
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
        let ema = Self::compute_ema(&candles);

        let now = Instant::now();
        Self {
            candles,
            volumes,
            ema,
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
                    color: Some(Color::from_linear(
                        r as f32,
                        g as f32,
                        b as f32,
                        VOLUME_BAR_ALPHA,
                    )),
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
                    let new_vol = BarPoint {
                        x: mid_x,
                        y: vol,
                        color: Some(Color::from_linear(0.3, 0.4, 0.1, VOLUME_BAR_ALPHA)),
                    };
                    state.volumes.push(new_vol);
                    if tx
                        .send(DataUpdate::AppendBarPoint {
                            series_index: SERIES_VOL,
                            point: new_vol,
                        })
                        .is_err()
                    {
                        break;
                    }

                    let previous_ema = state.ema.last().map(|point| point.y).unwrap_or(open);
                    let k = 2.0 / (EMA_PERIOD as f64 + 1.0);
                    let ema_point = halcyon::plot::LinePoint {
                        x: (x_open + x_close) * 0.5,
                        y: open * k + previous_ema * (1.0 - k),
                    };
                    state.ema.push(ema_point);
                    if tx
                        .send(DataUpdate::AppendLinePoint {
                            series_index: SERIES_EMA,
                            point: ema_point,
                        })
                        .is_err()
                    {
                        break;
                    }
                }

                let r = rand_f64(&mut state.rng);
                let step = state.price * 0.002 * (r - 0.5);
                state.price += step;

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

                let candle_is_up = candle.is_up;
                let candle_body_bottom = candle.body_bottom;
                let candle_snapshot = candle.clone();
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
                    vol.color = Some(Color::from_linear(
                        rv as f32,
                        gv as f32,
                        bv as f32,
                        VOLUME_BAR_ALPHA,
                    ));
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

                let previous_ema = if state.ema.len() > 1 {
                    state.ema[state.ema.len() - 2].y
                } else {
                    candle_body_bottom
                };
                if let Some(current_ema) = state.ema.last_mut() {
                    let k = 2.0 / (EMA_PERIOD as f64 + 1.0);
                    current_ema.y = close * k + previous_ema * (1.0 - k);
                    if tx
                        .send(DataUpdate::UpdateLastLinePoint {
                            series_index: SERIES_EMA,
                            point: *current_ema,
                        })
                        .is_err()
                    {
                        break;
                    }
                }

                if tx
                    .send(DataUpdate::AddPriceLevel(PriceLevel {
                        id: 10,
                        y: state.price,
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

impl ShowcaseScreen {
    pub(super) fn build_plot(&mut self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        let gpu = cx
            .app
            .gpu_resources()
            .expect("GpuResources not available")
            .clone();
        let render_pool = self
            .plot_render_pool
            .get_or_insert_with(|| Arc::new(halcyon::plot::PlotRenderPool::new()))
            .clone();

        let live = LiveState::new();
        let view_state = live.view_state_for_latest();
        let _data_x_max = live.data_x_max();
        let market_detail = MarketDetailLink::new(view_state.y_min, view_state.y_max);
        let book_mid = live.price;
        let book_tick = 0.005;
        let bids = BookSide {
            levels: (0..24)
                .map(|index| BookLevel {
                    price: book_mid - book_tick * (index + 1) as f64,
                    quantity: 0.05 + ((index * 37 % 101) as f64 / 33.0),
                    cumulative: 0.0,
                    order_count: None,
                    classification: LevelClassification::Normal,
                    metadata: vec![format!("bid source level {}", index + 1)],
                })
                .collect(),
        };
        let asks = BookSide {
            levels: (0..24)
                .map(|index| BookLevel {
                    price: book_mid + book_tick * (index + 1) as f64,
                    quantity: 0.05 + ((index * 53 % 97) as f64 / 31.0),
                    cumulative: 0.0,
                    order_count: None,
                    classification: LevelClassification::Normal,
                    metadata: vec![format!("ask source level {}", index + 1)],
                })
                .collect(),
        };

        let mut candle_series = CandleSeries::new();
        candle_series.set_candles(live.candles.clone());

        let mut vol_bars = BarSeries::new()
            .with_viewport_fraction(0.12)
            .with_z_order(50)
            .with_auto_scale_y(false);
        vol_bars.set_points(live.volumes.clone());

        let mut ema_line = LineSeries::new().with_z_order(10);
        ema_line.set_points(live.ema.clone());

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
            .with_market_detail_link(market_detail.clone())
            .with_market_detail_reference_level(10)
            .with_data_bounds(DataBounds {
                x_min: None,
                x_max: None,
                y_min: None,
                y_max: None,
            })
            .with_render_pool(render_pool)
            .add_series(Box::new(vol_bars))
            .add_series(Box::new(candle_series))
            .add_series(Box::new(ema_line))
            .with_label_config(Some(label_config))
            .with_title(
                "MOCK \u{00b7} 1s \u{00b7} Drag: time pan \u{00b7} Space+drag: time/price pan",
            )
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
                    StyledSegment {
                        x: 400.0,
                        y: 100.8,
                        color: Color::from_hex(0x3498DB).with_alpha(0.8),
                        width: 1.5,
                        line_style: PlotLineStyle::Solid,
                    },
                    StyledSegment {
                        x: 550.0,
                        y: 101.2,
                        color: Color::from_hex(0x2ECC71).with_alpha(0.9),
                        width: 2.0,
                        line_style: PlotLineStyle::Solid,
                    },
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

        let ticker_handle = spawn_ticker(data_tx.clone(), live);
        self._plot_data_tx = Some(data_tx);
        self._plot_ticker_handle = Some(ticker_handle);

        let row_id = cx
            .tree
            .add_child(parent, Box::new(Flex::row().with_spacing(1.0)), None, 0);
        cx.tree.set_flex(row_id, FlexParams::flexible(1.0));

        let plot_id = cx.tree.add_child(row_id, Box::new(plot), None, 0);
        cx.tree.set_flex(plot_id, FlexParams::flexible(1.0));

        let ladder = AlignedOrderBook::new(market_detail)
            .with_row_layout(AlignedOrderBookRowLayout::ReadableAnchored)
            .with_book(bids, asks);
        let ladder_id = cx.tree.add_child(row_id, Box::new(ladder), None, 0);
        cx.tree.set_flex(ladder_id, FlexParams::fixed(300.0));
    }
}
