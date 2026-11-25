// Scatter Demo (SPEC-HAL-296A) — SDF circle rendering with live data channel
//
// Single chart with 200 initial scatter points. A background ticker thread
// pushes a new point every 100ms via DataUpdate::AppendCircle. Per-point
// radius and color overrides exercise the SDF path.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::plot::{
    Circle, CircleSeries, CircleStyle, DataUpdate, LabelConfig, Plot, PlotRenderPool, ViewState,
};
use halcyon::prelude::*;
use std::sync::Arc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Scatter Demo (SPEC-HAL-296A) ===");
    println!("SDF circle rendering with live data channel");
    println!("200 initial points, new point every 100ms");
    println!("Per-point color + radius overrides");
    println!("Close window to exit.");

    let app = App::new(AppConfig::new("Scatter Demo").with_size(1200.0, 700.0));
    app.run(ScatterScreen::new());
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

const HISTORY_COUNT: usize = 200;
const POINT_INTERVAL: f64 = 10.0;
const TICK_INTERVAL_MS: u64 = 100;

const SERIES_SCATTER: usize = 0;

struct LiveState {
    points: Vec<Circle>,
    rng: u64,
    x_cursor: f64,
    y_center: f64,
}

impl LiveState {
    fn new() -> Self {
        let mut rng: u64 = 0xCAFE_BABE_1234_5678;
        let mut points = Vec::with_capacity(HISTORY_COUNT);
        let mut y_center = 50.0;

        for i in 0..HISTORY_COUNT {
            let x = i as f64 * POINT_INTERVAL;
            let r = rand_f64(&mut rng);
            y_center += (r - 0.5) * 2.0;
            let y = y_center + (x * 0.05).sin() * 10.0;

            // ~25% get radius override (2-12px)
            let radius = if rand_f64(&mut rng) > 0.75 {
                Some(2.0 + rand_f64(&mut rng) as f32 * 10.0)
            } else {
                None
            };

            // ~40% get color override (cool→warm by Y)
            let fill_color = if rand_f64(&mut rng) > 0.6 {
                let t = ((y - 20.0) / 60.0).clamp(0.0, 1.0) as f32;
                Some(Color::from_linear(
                    0.2 + t * 0.8,
                    0.4 * (1.0 - t) + 0.2,
                    1.0 - t * 0.7,
                    0.9,
                ))
            } else {
                None
            };

            points.push(Circle {
                x,
                y,
                radius,
                fill_color,
            });
        }

        Self {
            x_cursor: HISTORY_COUNT as f64 * POINT_INTERVAL,
            points,
            rng,
            y_center,
        }
    }

    fn view_state_for_latest(&self) -> ViewState {
        let visible_count = 100usize;
        let n = self.points.len();
        let start = n.saturating_sub(visible_count);

        let x_min = self.points[start].x - POINT_INTERVAL;
        let x_max = self.points.last().unwrap().x + POINT_INTERVAL * 5.0;

        let y_min = self.points[start..]
            .iter()
            .map(|p| p.y)
            .fold(f64::INFINITY, f64::min);
        let y_max = self.points[start..]
            .iter()
            .map(|p| p.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let y_pad = (y_max - y_min) * 0.1;

        let mut vs = ViewState::new(x_min, x_max, y_min - y_pad, y_max + y_pad);
        vs.follow_latest = true;
        vs
    }
}

// -- Ticker thread -------------------------------------------------------

fn spawn_ticker(
    tx: crossbeam_channel::Sender<DataUpdate>,
    initial_state: LiveState,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("scatter-ticker".into())
        .spawn(move || {
            let mut state = initial_state;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(TICK_INTERVAL_MS));

                let r = rand_f64(&mut state.rng);
                state.y_center += (r - 0.5) * 2.0;
                let x = state.x_cursor;
                let y = state.y_center + (x * 0.05).sin() * 10.0;
                state.x_cursor += POINT_INTERVAL;

                let radius = if rand_f64(&mut state.rng) > 0.75 {
                    Some(2.0 + rand_f64(&mut state.rng) as f32 * 10.0)
                } else {
                    None
                };

                let fill_color = if rand_f64(&mut state.rng) > 0.6 {
                    let t = ((y - 20.0) / 60.0).clamp(0.0, 1.0) as f32;
                    Some(Color::from_linear(
                        0.2 + t * 0.8,
                        0.4 * (1.0 - t) + 0.2,
                        1.0 - t * 0.7,
                        0.9,
                    ))
                } else {
                    None
                };

                let point = Circle {
                    x,
                    y,
                    radius,
                    fill_color,
                };
                state.points.push(point);

                if tx
                    .send(DataUpdate::AppendCircle {
                        series_index: SERIES_SCATTER,
                        circle: point,
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .expect("failed to spawn scatter ticker")
}

// -- Screen --------------------------------------------------------------

struct ScatterScreen {
    needs_rebuild: bool,
    placeholder: Label,
    render_pool: Arc<PlotRenderPool>,
    _data_tx: Option<crossbeam_channel::Sender<DataUpdate>>,
    _ticker_handle: Option<std::thread::JoinHandle<()>>,
}

impl ScatterScreen {
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

impl Screen for ScatterScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let gpu = cx
            .app
            .gpu_resources()
            .expect("GpuResources not available")
            .clone();

        let live = LiveState::new();

        let mut scatter = CircleSeries::new().with_style(CircleStyle {
            fill_color: Color::from_linear(0.2, 0.7, 1.0, 0.9),
            radius: 5.0,
            border_color: Color::from_linear(1.0, 1.0, 1.0, 0.6),
            border_width: 1.0,
        });
        scatter.set_points(live.points.clone());

        let view_state = live.view_state_for_latest();

        let label_config = LabelConfig {
            font_size: 11.0,
            y_tick_count: 5,
            x_tick_count: 6,
            ..LabelConfig::default()
        };

        let (plot, data_tx) = Plot::new()
            .with_gpu_resources(gpu)
            .with_view_state(view_state)
            .with_render_pool(self.render_pool.clone())
            .add_series(Box::new(scatter))
            .with_label_config(Some(label_config))
            .with_title("Scatter Demo \u{00b7} Live")
            .with_data_channel();

        let ticker_handle = spawn_ticker(data_tx.clone(), live);
        self._data_tx = Some(data_tx);
        self._ticker_handle = Some(ticker_handle);

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
