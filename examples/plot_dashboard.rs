// SPEC-HAL-296: Plot Dashboard Demo
// 20 independent Plot widgets in a 5×4 grid.
// Stress-tests threaded rendering architecture.

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

    println!("=== Plot Dashboard Demo ===");
    println!("20 charts (100K candles each, 2M total)");
    println!("Each chart: candles + volume bars + 9 Fibonacci EMAs");
    println!("All sharing X axis, crosshair probe, and render pool");
    println!("Stress-testing PlotRenderPool architecture");
    println!("===========================\n");

    let app = App::new(AppConfig::new("Plot Dashboard Demo").with_size(1600.0, 1000.0));
    app.run(DashboardScreen::new());
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

/// Compute ViewState showing first 200 candles (or all if fewer).
fn view_state_from_candles(candles: &[RenderCandle]) -> ViewState {
    let x_min = candles.first().map(|c| c.x_open).unwrap_or(0.0);
    let initial_count = candles.len().min(200);
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

struct DashboardScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl DashboardScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }

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

        let data_x_min = candles.first().map(|c| c.x_open).unwrap_or(0.0);
        let data_x_max = candles.last().map(|c| c.x_close).unwrap_or(1.0);

        let mut candle_series = CandleSeries::new();
        candle_series.set_candles(candles.clone());

        // Volume bars — ratio-gradient encoding
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
                halcyon::plot::BarPoint {
                    x: mid_x,
                    y: vol,
                    color: Some(Color::from_linear(r as f32, g as f32, b as f32, 0.5)),
                }
            })
            .collect();

        let mut vol_bars = halcyon::plot::BarSeries::new()
            .with_style(halcyon::plot::BarStyle {
                color: Color::from_srgb(0.3, 0.5, 1.0, 0.4),
                width_fraction: 0.8,
                baseline: 0.0,
                viewport_fraction: Some(0.12),
            })
            .with_z_order(50)
            .with_auto_scale_y(false);
        vol_bars.set_points(volume_bars);

        let label_config = halcyon::plot::LabelConfig {
            font_size: 9.0,
            y_tick_count: 3,
            x_tick_count: 4,
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
            ..halcyon::plot::LabelConfig::default()
        };

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

            let mut line = halcyon::plot::LineSeries::new()
                .with_style(halcyon::plot::LineStyle { color, width: 1.0 })
                .with_z_order(10 + z_offset as u32);
            line.set_points(points);
            plot = plot.add_series(Box::new(line));
        }

        plot = plot.with_label_config(Some(label_config));

        tree.add_child(parent, Box::new(plot), None, slot)
    }

    /// Build a row of 4 charts as a binary split tree.
    fn build_row(
        tree: &mut halcyon::core::widget_tree::WidgetTree,
        parent: halcyon::core::widget_tree::NodeId,
        gpu: halcyon::renderer::wgpu_renderer::GpuResources,
        row_index: usize,
        slot: i16,
        shared_x: Arc<Mutex<AxisAuthority>>,
        shared_probe: Arc<Mutex<Probe>>,
        render_pool: Arc<PlotRenderPool>,
    ) -> halcyon::core::widget_tree::NodeId {
        // Row split: Split(Split(c1, c2), Split(c3, c4))
        let row = tree.add_child(
            parent,
            Box::new(SplitPane::horizontal(NodeId(0), NodeId(0)).initial_ratio(0.5)),
            None,
            slot,
        );

        let left = tree.add_child(
            row,
            Box::new(SplitPane::horizontal(NodeId(0), NodeId(0)).initial_ratio(0.5)),
            None,
            0,
        );
        let right = tree.add_child(
            row,
            Box::new(SplitPane::horizontal(NodeId(0), NodeId(0)).initial_ratio(0.5)),
            None,
            1,
        );

        let base = row_index * 4;
        let charts: Vec<_> = (0..4)
            .map(|col| {
                let chart_index = base + col;
                let parent_node = if col < 2 { left } else { right };
                let child_slot = (col % 2) as i16;
                Self::build_chart(
                    tree,
                    parent_node,
                    gpu.clone(),
                    100_000,
                    10.0 + chart_index as f64 * 23.7,
                    chart_index * 50000,
                    child_slot,
                    shared_x.clone(),
                    shared_probe.clone(),
                    render_pool.clone(),
                )
            })
            .collect();

        replace_widget!(
            tree,
            left,
            SplitPane::horizontal(charts[0], charts[1]).initial_ratio(0.5)
        );
        replace_widget!(
            tree,
            right,
            SplitPane::horizontal(charts[2], charts[3]).initial_ratio(0.5)
        );
        replace_widget!(
            tree,
            row,
            SplitPane::horizontal(left, right).initial_ratio(0.5)
        );

        row
    }
}

impl Screen for DashboardScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let gpu = cx
            .app
            .gpu_resources()
            .expect("GpuResources not available")
            .clone();

        let initial_x_span = 200.0 * 60.0;
        let shared_x = Arc::new(Mutex::new(AxisAuthority::new(0.0, initial_x_span)));
        let shared_probe = Arc::new(Mutex::new(Probe::new()));
        let render_pool = Arc::new(PlotRenderPool::new());

        // 5 rows × 4 cols = 20 charts
        // Binary vertical split: Split(top3, bot2) ratio=0.6
        //   top3 = Split(row1, mid2) ratio=0.333
        //     mid2 = Split(row2, row3) ratio=0.5
        //   bot2 = Split(row4, row5) ratio=0.5

        let outer = cx.mount_child(Box::new(
            SplitPane::vertical(NodeId(0), NodeId(0)).initial_ratio(0.6),
        ));
        let tree = &mut cx.tree;

        // Top 3 rows container
        let top3 = tree.add_child(
            outer,
            Box::new(SplitPane::vertical(NodeId(0), NodeId(0)).initial_ratio(0.333)),
            None,
            0,
        );

        let row1 = Self::build_row(
            tree,
            top3,
            gpu.clone(),
            0,
            0,
            shared_x.clone(),
            shared_probe.clone(),
            render_pool.clone(),
        );

        // Mid 2 container (rows 2 and 3)
        let mid2 = tree.add_child(
            top3,
            Box::new(SplitPane::vertical(NodeId(0), NodeId(0)).initial_ratio(0.5)),
            None,
            1,
        );

        let row2 = Self::build_row(
            tree,
            mid2,
            gpu.clone(),
            1,
            0,
            shared_x.clone(),
            shared_probe.clone(),
            render_pool.clone(),
        );
        let row3 = Self::build_row(
            tree,
            mid2,
            gpu.clone(),
            2,
            1,
            shared_x.clone(),
            shared_probe.clone(),
            render_pool.clone(),
        );

        // Bottom 2 rows container
        let bot2 = tree.add_child(
            outer,
            Box::new(SplitPane::vertical(NodeId(0), NodeId(0)).initial_ratio(0.5)),
            None,
            1,
        );

        let row4 = Self::build_row(
            tree,
            bot2,
            gpu.clone(),
            3,
            0,
            shared_x.clone(),
            shared_probe.clone(),
            render_pool.clone(),
        );
        let row5 = Self::build_row(tree, bot2, gpu, 4, 1, shared_x, shared_probe, render_pool);

        // Wire up all the splits with correct children
        replace_widget!(
            tree,
            mid2,
            SplitPane::vertical(row2, row3).initial_ratio(0.5)
        );
        replace_widget!(
            tree,
            top3,
            SplitPane::vertical(row1, mid2).initial_ratio(0.333)
        );
        replace_widget!(
            tree,
            bot2,
            SplitPane::vertical(row4, row5).initial_ratio(0.5)
        );
        replace_widget!(
            tree,
            outer,
            SplitPane::vertical(top3, bot2).initial_ratio(0.6)
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
