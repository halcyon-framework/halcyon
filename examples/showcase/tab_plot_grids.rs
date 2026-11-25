use halcyon::core::app::ScreenFrameContext;
use halcyon::core::layout::FlexParams;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::plot::{
    AxisAuthority, BarPoint, BarSeries, BarStyle, CandleSeries, DataBounds, LabelConfig,
    LineSeries, LineStyle, Plot, PlotRenderPool, Probe, RenderCandle, ViewState,
};
use halcyon::prelude::*;
use std::sync::{Arc, Mutex};

use super::*;

const CANDLE_COUNT: usize = 1000;
const VISIBLE_COUNT: usize = 200;

// Deterministic candle generation — identical to examples/plot_quad.rs and
// examples/plot_dashboard.rs (LCG so each chart's seed_offset yields its own
// series).
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

fn view_state_from_candles(candles: &[RenderCandle]) -> ViewState {
    let x_min = candles.first().map(|c| c.x_open).unwrap_or(0.0);
    let initial_count = candles.len().min(VISIBLE_COUNT);
    let x_max = if initial_count > 0 {
        candles[initial_count - 1].x_close
    } else {
        1.0
    };
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

// Build one Plot (candles + ratio-gradient volume + Fibonacci EMA ribbon),
// add it under `parent` at `slot`, return its NodeId. The shared X-axis and
// probe link every chart in the grid (synced pan/zoom + crosshair).
#[allow(clippy::too_many_arguments)]
fn build_chart(
    tree: &mut WidgetTree,
    parent: NodeId,
    gpu: halcyon::renderer::wgpu_renderer::GpuResources,
    base_price: f64,
    seed_offset: usize,
    slot: i16,
    shared_x: Arc<Mutex<AxisAuthority>>,
    shared_probe: Arc<Mutex<Probe>>,
    render_pool: Arc<PlotRenderPool>,
    font_size: f32,
    y_tick_count: usize,
    x_tick_count: usize,
) -> NodeId {
    let candles = generate_candles(CANDLE_COUNT, base_price, seed_offset);
    let view_state = view_state_from_candles(&candles);
    let data_x_min = candles.first().map(|c| c.x_open).unwrap_or(0.0);
    let data_x_max = candles.last().map(|c| c.x_close).unwrap_or(1.0);

    let mut candle_series = CandleSeries::new();
    candle_series.set_candles(candles.clone());

    let volume_bars: Vec<BarPoint> = candles
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let idx = (i + seed_offset) as u64;
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
                color: Some(Color::from_linear(r as f32, g as f32, b as f32, 0.5)),
            }
        })
        .collect();

    let mut vol_bars = BarSeries::new()
        .with_style(BarStyle {
            color: Color::from_srgb(0.3, 0.5, 1.0, 0.4),
            width_fraction: 0.8,
            baseline: 0.0,
            viewport_fraction: Some(0.12),
        })
        .with_z_order(50)
        .with_auto_scale_y(false);
    vol_bars.set_points(volume_bars);

    let mut plot = Plot::new()
        .with_gpu_resources(gpu)
        .with_view_state(view_state)
        .with_data_bounds(DataBounds {
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

    // Fibonacci EMA ribbon: 5, 8, 13, 21, 34, 55, 89, 144, 233.
    let ema_configs: &[(usize, Color)] = &[
        (5, Color::from_srgb(1.0, 0.3, 0.3, 0.8)),
        (8, Color::from_srgb(1.0, 0.5, 0.2, 0.8)),
        (13, Color::from_srgb(1.0, 0.8, 0.0, 0.8)),
        (21, Color::from_srgb(1.0, 1.0, 0.0, 0.8)),
        (34, Color::from_srgb(0.5, 1.0, 0.2, 0.8)),
        (55, Color::from_srgb(0.0, 1.0, 0.4, 0.8)),
        (89, Color::from_srgb(0.0, 0.8, 1.0, 0.8)),
        (144, Color::from_srgb(0.3, 0.5, 1.0, 0.8)),
        (233, Color::from_srgb(0.7, 0.3, 1.0, 0.8)),
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
        let mut line = LineSeries::new()
            .with_style(LineStyle { color, width: 1.0 })
            .with_z_order(10 + z_offset as u32);
        line.set_points(points);
        plot = plot.add_series(Box::new(line));
    }

    let label_config = LabelConfig {
        font_size,
        y_tick_count,
        x_tick_count,
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
    plot = plot.with_label_config(Some(label_config));

    tree.add_child(parent, Box::new(plot), None, slot)
}

// Fixed Flex grid of `rows`×`cols` charts filling `parent`, all sharing one
// X-axis and one crosshair probe.
fn build_plot_grid(
    cx: &mut ScreenFrameContext<'_>,
    parent: NodeId,
    rows: usize,
    cols: usize,
    font_size: f32,
    y_tick_count: usize,
    x_tick_count: usize,
) {
    let gpu = cx
        .app
        .gpu_resources()
        .expect("GpuResources not available")
        .clone();
    let render_pool = Arc::new(PlotRenderPool::new());

    // Shared X-axis + crosshair: every chart pans/zooms on one timeline and
    // shows the crosshair at the same X (matches plot_quad / plot_dashboard).
    let initial_x_span = VISIBLE_COUNT as f64 * 60.0;
    let shared_x = Arc::new(Mutex::new(AxisAuthority::new(0.0, initial_x_span)));
    let shared_probe = Arc::new(Mutex::new(Probe::new()));

    let col_stack = cx
        .tree
        .add_child(parent, Box::new(Flex::column().with_spacing(2.0)), None, 0);
    cx.tree.set_flex(col_stack, FlexParams::flexible(1.0));

    for r in 0..rows {
        let row = cx.tree.add_child(
            col_stack,
            Box::new(Flex::row().with_spacing(2.0)),
            None,
            r as i16,
        );
        cx.tree.set_flex(row, FlexParams::flexible(1.0));

        for c in 0..cols {
            let idx = r * cols + c;
            let base_price = 10.0 + idx as f64 * 23.7;
            let seed_offset = idx * 50000;
            let cell = build_chart(
                &mut cx.tree,
                row,
                gpu.clone(),
                base_price,
                seed_offset,
                c as i16,
                shared_x.clone(),
                shared_probe.clone(),
                render_pool.clone(),
                font_size,
                y_tick_count,
                x_tick_count,
            );
            cx.tree.set_flex(cell, FlexParams::flexible(1.0));
        }
    }
}

impl ShowcaseScreen {
    pub(super) fn build_plot_quad(&self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        build_plot_grid(cx, parent, 2, 2, 11.0, 5, 6);
    }

    pub(super) fn build_plot_dashboard(&self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        build_plot_grid(cx, parent, 5, 4, 9.0, 3, 4);
    }
}
