// SPEC-HAL-296: Plot Quad Demo
// 4 independent Plot widgets in a 2×2 SplitPane grid.
// Proves Plot works inside the App/Screen framework.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::plot::{
    AxisAuthority, CandleSeries, Plot, PlotRenderPool, Probe, RenderCandle, ViewState,
};
use halcyon::prelude::*;
use halcyon::replace_widget;
use halcyon::widgets::split_pane::SplitPane;
use std::sync::{Arc, Mutex};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Plot Quad Demo ===");
    println!("4 synchronized charts in a 2x2 grid (500K candles each)");
    println!("Pan: click-drag");
    println!("Zoom: scroll wheel");
    println!("Box zoom: right-click drag");
    println!("Double-click: reset view");
    println!("Range select: Shift+drag");
    println!("Click on candle: select (logged to console)");
    println!("Escape: clear selection");
    println!("=======================\n");

    let app = App::new(AppConfig::new("Plot Quad Demo").with_size(1280.0, 800.0));
    app.run(PlotQuadScreen::new());
}

/// Generate mock candles with a configurable seed offset for variety.
fn generate_candles(count: usize, base_price: f64, seed_offset: usize) -> Vec<RenderCandle> {
    let mut candles = Vec::with_capacity(count);
    let mut price = base_price;
    let interval = 60.0;

    for i in 0..count {
        let idx = i + seed_offset;
        let x_open = i as f64 * interval;
        let x_close = x_open + interval;

        let rand1 = ((idx * 1103515245 + 12345) % 2147483648) as f64 / 2147483648.0;
        let rand2 = ((idx * 134775813 + 1) % 2147483648) as f64 / 2147483648.0;
        let rand3 = ((idx * 1664525 + 1013904223) % 2147483648) as f64 / 2147483648.0;
        let rand4 = ((idx * 22695477 + 1) % 2147483648) as f64 / 2147483648.0;

        let volatility = 0.005 + rand4 * 0.015;
        let open = price;
        let close = price * (1.0 + (rand1 - 0.5) * volatility);
        let body_top = open.max(close);
        let body_bottom = open.min(close);

        let (high, low) = if rand2 > 0.4 {
            let body_size = (body_top - body_bottom).abs().max(price * 0.0001);
            (
                body_top + body_size * rand3 * 3.0,
                body_bottom - body_size * rand4 * 3.0,
            )
        } else {
            (body_top, body_bottom)
        };

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

    candles
}

/// Compute ViewState showing first 1000 candles (or all if fewer).
fn view_state_from_candles(candles: &[RenderCandle]) -> ViewState {
    let x_min = candles.first().map(|c| c.x_open).unwrap_or(0.0);
    let initial_count = candles.len().min(200);
    let x_max = if initial_count > 0 {
        candles[initial_count - 1].x_close
    } else {
        1.0
    };

    // Price range from visible candles only
    let y_min = candles
        .iter()
        .take(initial_count)
        .map(|c| c.low)
        .fold(f64::INFINITY, f64::min);
    let y_max = candles
        .iter()
        .take(initial_count)
        .map(|c| c.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let y_pad = (y_max - y_min) * 0.05;
    ViewState::new(x_min, x_max, y_min - y_pad, y_max + y_pad)
}

struct PlotQuadScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl PlotQuadScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }

    /// Build one Plot widget with mock data and add it to the tree.
    fn build_chart(
        tree: &mut halcyon::core::widget_tree::WidgetTree,
        parent: halcyon::core::widget_tree::NodeId,
        gpu: halcyon::renderer::wgpu_renderer::GpuResources,
        candle_count: usize,
        base_price: f64,
        seed_offset: usize,
        slot: i16,
        shared_x: Arc<Mutex<AxisAuthority>>,
        shared_probe: Arc<Mutex<Probe>>,
        render_pool: Arc<PlotRenderPool>,
    ) -> halcyon::core::widget_tree::NodeId {
        let candles = generate_candles(candle_count, base_price, seed_offset);
        let view_state = view_state_from_candles(&candles);

        // Data bounds for pan constraint
        let data_x_min = candles.first().map(|c| c.x_open).unwrap_or(0.0);
        let data_x_max = candles.last().map(|c| c.x_close).unwrap_or(1.0);

        let mut candle_series = CandleSeries::new();
        candle_series.set_candles(candles.clone());

        // Volume bars — ratio-gradient encoding
        // Height = total volume. Color = buy/sell composition.
        // Green (100% buy) → Grey (50/50) → Red (100% sell)
        let volume_bars: Vec<halcyon::plot::BarPoint> = candles
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let idx = (i + seed_offset) as u64;

                // Two independent PRNGs: one for total volume, one for buy ratio
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

                // Total volume: [0.1, 1.0]
                let vol = 0.1 + rand_vol * 0.9;

                // Buy ratio: bias slightly toward candle direction for realism
                // Up candle → ratio biased toward 0.6–0.9
                // Down candle → ratio biased toward 0.1–0.4
                let buy_ratio = if c.is_up {
                    0.4 + rand_ratio * 0.5 // [0.4, 0.9]
                } else {
                    0.1 + rand_ratio * 0.5 // [0.1, 0.6]
                };

                // Map buy_ratio [0, 1] to color gradient:
                // 0.0 = pure red, 0.5 = grey, 1.0 = pure green
                let (r, g, b) = if buy_ratio >= 0.5 {
                    // Green side: grey → green
                    let t = (buy_ratio - 0.5) * 2.0; // [0, 1]
                    (
                        0.4 * (1.0 - t),       // red fades out
                        0.3 + 0.5 * t,         // green increases
                        0.4 * (1.0 - t) * 0.3, // blue fades
                    )
                } else {
                    // Red side: red → grey
                    let t = buy_ratio * 2.0; // [0, 1] where 0=pure red, 1=grey
                    (
                        0.7 - 0.3 * t, // red decreases
                        0.1 + 0.2 * t, // green increases slightly
                        0.1 * t * 0.3, // blue fades in slightly
                    )
                };

                let mid_x = (c.x_open + c.x_close) / 2.0;
                halcyon::plot::BarPoint {
                    x: mid_x,
                    y: vol,
                    color: Some(Color::from_linear(r as f32, g as f32, b as f32, 0.6)),
                }
            })
            .collect();

        let mut vol_bars = halcyon::plot::BarSeries::new()
            .with_style(halcyon::plot::BarStyle {
                color: Color::from_srgb(0.3, 0.5, 1.0, 0.4),
                width_fraction: 0.8,
                baseline: 0.0,
                viewport_fraction: Some(0.15),
            })
            .with_z_order(50)
            .with_auto_scale_y(false);
        vol_bars.set_points(volume_bars);

        let mut plot = Plot::new()
            .with_gpu_resources(gpu)
            .with_view_state(view_state)
            .with_data_bounds(halcyon::plot::DataBounds {
                x_min: Some(data_x_min),
                x_max: Some(data_x_max),
                y_min: None,
                y_max: None,
            })
            .with_shared_x(shared_x)
            .with_probe(shared_probe)
            .with_render_pool(render_pool)
            .add_series(Box::new(vol_bars))
            .add_series(Box::new(candle_series));

        // Fibonacci EMAs: 5, 8, 13, 21, 34, 55, 89, 144, 233
        let ema_configs: &[(usize, Color)] = &[
            (5, Color::from_srgb(1.0, 0.3, 0.3, 0.8)),   // red
            (8, Color::from_srgb(1.0, 0.5, 0.2, 0.8)),   // orange
            (13, Color::from_srgb(1.0, 0.8, 0.0, 0.8)),  // amber
            (21, Color::from_srgb(1.0, 1.0, 0.0, 0.8)),  // yellow
            (34, Color::from_srgb(0.5, 1.0, 0.2, 0.8)),  // lime
            (55, Color::from_srgb(0.0, 1.0, 0.4, 0.8)),  // green
            (89, Color::from_srgb(0.0, 0.8, 1.0, 0.8)),  // cyan
            (144, Color::from_srgb(0.3, 0.5, 1.0, 0.8)), // blue
            (233, Color::from_srgb(0.7, 0.3, 1.0, 0.8)), // purple
        ];

        for (z_offset, &(period, color)) in ema_configs.iter().enumerate() {
            if candles.is_empty() {
                continue;
            }
            let k = 2.0 / (period as f64 + 1.0);
            let mut ema = (candles[0].body_top + candles[0].body_bottom) / 2.0;
            let mut points = Vec::with_capacity(candles.len());

            for c in &candles {
                let price = (c.body_top + c.body_bottom) / 2.0;
                ema = price * k + ema * (1.0 - k);
                points.push(halcyon::plot::LinePoint {
                    x: (c.x_open + c.x_close) / 2.0,
                    y: ema,
                });
            }

            let mut line = halcyon::plot::LineSeries::new()
                .with_style(halcyon::plot::LineStyle { color, width: 1.5 })
                .with_z_order(10 + z_offset as u32);
            line.set_points(points);
            plot = plot.add_series(Box::new(line));
        }

        // Mock histogram on chart 4 (consumer would compute MACD, momentum, etc.)
        if slot == 3 {
            let hist_points: Vec<halcyon::plot::HistogramPoint> = candles
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let mid_x = (c.x_open + c.x_close) / 2.0;
                    let y = (i as f64 * 0.05).sin() * 2.0;
                    halcyon::plot::HistogramPoint { x: mid_x, y }
                })
                .collect();

            let mut hist = halcyon::plot::HistogramSeries::new()
                .with_style(halcyon::plot::HistogramStyle {
                    positive_color: Color::from_hex(0x2ECC71).with_alpha(0.5),
                    negative_color: Color::from_hex(0xF05454).with_alpha(0.5),
                    width_fraction: 0.8,
                })
                .with_z_order(3);
            hist.set_points(hist_points);
            plot = plot.add_series(Box::new(hist));
        }

        let label_config = halcyon::plot::LabelConfig {
            font_size: 11.0,
            y_tick_count: 5,
            x_tick_count: 6,
            y_formatter: None,
            x_formatter: Some(std::sync::Arc::new(
                |ctx: halcyon::plot::axis::AxisFormatContext| {
                    let val = ctx.value;
                    let total_mins = (val / 60.0) as i64;
                    let days = total_mins / 1440;
                    let hours = (total_mins % 1440) / 60;
                    let mins = total_mins % 60;
                    if days > 0 {
                        format!("{}d {}h", days, hours)
                    } else if hours > 0 {
                        format!("{}h {:02}m", hours, mins)
                    } else {
                        format!("{}m", mins)
                    }
                },
            )),
            ..halcyon::plot::LabelConfig::default()
        };
        plot = plot.with_label_config(Some(label_config));

        let titles = [
            "Chart 1 \u{00b7} 100 base",
            "Chart 2 \u{00b7} 50 base",
            "Chart 3 \u{00b7} 200 base",
            "Chart 4 \u{00b7} MACD",
        ];
        plot = plot.with_title(titles[slot as usize]);

        tree.add_child(parent, Box::new(plot), None, slot)
    }
}

impl Screen for PlotQuadScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Layout:
        //   ┌──────────────┬──────────────┐
        //   │   Chart 1    │   Chart 2    │
        //   │  (100 base)  │  (50 base)   │
        //   ├──────────────┼──────────────┤
        //   │   Chart 3    │   Chart 4    │
        //   │  (200 base)  │  (75 base)   │
        //   └──────────────┴──────────────┘

        let gpu = cx
            .app
            .gpu_resources()
            .expect("GpuResources not available")
            .clone();

        // Outer vertical split (placeholder NodeIds)
        let outer = cx.mount_child(Box::new(
            SplitPane::vertical(NodeId(0), NodeId(0)).initial_ratio(0.5),
        ));
        let tree = &mut cx.tree;

        // Top row: horizontal split
        let top_row = tree.add_child(
            outer,
            Box::new(SplitPane::horizontal(NodeId(0), NodeId(0)).initial_ratio(0.5)),
            None,
            0,
        );

        // Bottom row: horizontal split
        let bottom_row = tree.add_child(
            outer,
            Box::new(SplitPane::horizontal(NodeId(0), NodeId(0)).initial_ratio(0.5)),
            None,
            1,
        );

        // Shared X axis — all 4 charts scroll/zoom together
        let initial_x_span = 200.0 * 60.0; // 200 candles × 60s interval
        let shared_x = Arc::new(Mutex::new(AxisAuthority::new(0.0, initial_x_span)));
        let shared_probe = Arc::new(Mutex::new(Probe::new()));
        let render_pool = Arc::new(PlotRenderPool::new());

        // Build 4 charts with different data, shared X axis
        let chart1 = Self::build_chart(
            tree,
            top_row,
            gpu.clone(),
            500_000,
            100.0,
            0,
            0,
            shared_x.clone(),
            shared_probe.clone(),
            render_pool.clone(),
        );
        let chart2 = Self::build_chart(
            tree,
            top_row,
            gpu.clone(),
            500_000,
            50.0,
            10000,
            1,
            shared_x.clone(),
            shared_probe.clone(),
            render_pool.clone(),
        );
        let chart3 = Self::build_chart(
            tree,
            bottom_row,
            gpu.clone(),
            500_000,
            200.0,
            20000,
            0,
            shared_x.clone(),
            shared_probe.clone(),
            render_pool.clone(),
        );
        let chart4 = Self::build_chart(
            tree,
            bottom_row,
            gpu,
            500_000,
            75.0,
            30000,
            1,
            shared_x,
            shared_probe,
            render_pool,
        );

        // Replace SplitPanes with correct child NodeIds
        replace_widget!(
            tree,
            top_row,
            SplitPane::horizontal(chart1, chart2).initial_ratio(0.5)
        );
        replace_widget!(
            tree,
            bottom_row,
            SplitPane::horizontal(chart3, chart4).initial_ratio(0.5)
        );
        replace_widget!(
            tree,
            outer,
            SplitPane::vertical(top_row, bottom_row).initial_ratio(0.5)
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
