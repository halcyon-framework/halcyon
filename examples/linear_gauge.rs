// Linear Gauge Demo — Horizontal + Vertical bar gauges with spring-damped tracking

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::FlexParams;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::linear_gauge::{
    GaugeFillStyle, GaugeMode, GaugeTickMarks, GaugeZone, LinearGauge, LinearGaugeConfig,
    LinearGaugeOrientation, TickSide,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ─── Random target generator ─────────────────────────────

struct JumpGenerator {
    value: f64,
    min: f64,
    max: f64,
    seed: u64,
    interval: Duration,
    last_jump: Instant,
}

impl JumpGenerator {
    fn new(initial: f64, min: f64, max: f64, seed: u64, interval_ms: u64) -> Self {
        Self {
            value: initial,
            min,
            max,
            seed,
            interval: Duration::from_millis(interval_ms),
            last_jump: Instant::now(),
        }
    }

    fn poll(&mut self) -> Option<f64> {
        let now = Instant::now();
        if now.duration_since(self.last_jump) >= self.interval {
            self.last_jump = now;
            self.seed = self
                .seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let rand_norm = ((self.seed >> 33) as f64) / (u32::MAX as f64);
            self.value = self.min + rand_norm * (self.max - self.min);
            Some(self.value)
        } else {
            None
        }
    }
}

// ─── Spring-damper ───────────────────────────────────────

struct SpringDamper {
    position: f64,
    velocity: f64,
    target: f64,
}

impl SpringDamper {
    fn new(initial: f64) -> Self {
        Self {
            position: initial,
            velocity: 0.0,
            target: initial,
        }
    }

    fn set_target(&mut self, target: f64) {
        self.target = target;
    }

    fn step(&mut self, dt: f64) -> f64 {
        let diff = self.target - self.position;

        if diff.abs() < 0.01 && self.velocity.abs() < 0.01 {
            self.position = self.target;
            self.velocity = 0.0;
            return self.position;
        }

        let stiffness = 400.0;
        let damping = 6.0;

        let force = diff * stiffness - self.velocity * damping;
        self.velocity += force * dt;
        self.position += self.velocity * dt;
        self.position
    }
}

// ─── Screen ──────────────────────────────────────────────

fn main() {
    println!("=== Linear Gauge Demo ===");
    println!("Display-only — no keybinds");
    println!("=========================\n");

    let app = App::new(AppConfig::new("Linear Gauge Demo").with_size(900.0, 600.0));
    app.run(LinearGaugeScreen::new());
}

struct LinearGaugeScreen {
    placeholder: Label,
    generators: [JumpGenerator; 9],
    springs: [SpringDamper; 9],
    last_time: Instant,
}

impl LinearGaugeScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            generators: [
                // 0-4: horizontal gauges
                JumpGenerator::new(50.0, 0.0, 100.0, 0xDEAD_BEEF, 800),
                JumpGenerator::new(50.0, 0.0, 100.0, 0xCAFE_BABE, 1200),
                JumpGenerator::new(0.0, -50.0, 50.0, 0x1337_C0DE, 600),
                JumpGenerator::new(50.0, 0.0, 100.0, 0xBAAD_F00D, 1000),
                JumpGenerator::new(50.0, 0.0, 100.0, 0xFACE_CAFE, 900),
                // 5-8: vertical gauges
                JumpGenerator::new(60.0, 0.0, 100.0, 0xA0A0_B0B0, 700),
                JumpGenerator::new(40.0, 0.0, 100.0, 0xC0C0_D0D0, 950),
                JumpGenerator::new(0.0, -50.0, 50.0, 0xE0E0_F0F0, 550),
                JumpGenerator::new(70.0, 0.0, 100.0, 0x1010_2020, 850),
            ],
            springs: [
                SpringDamper::new(50.0),
                SpringDamper::new(50.0),
                SpringDamper::new(0.0),
                SpringDamper::new(50.0),
                SpringDamper::new(50.0),
                SpringDamper::new(60.0),
                SpringDamper::new(40.0),
                SpringDamper::new(0.0),
                SpringDamper::new(70.0),
            ],
            last_time: Instant::now(),
        }
    }
}

impl Screen for LinearGaugeScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let now = Instant::now();
        let dt = now.duration_since(self.last_time).as_secs_f64().min(0.1);
        self.last_time = now;

        for i in 0..9 {
            if let Some(new_val) = self.generators[i].poll() {
                self.springs[i].set_target(new_val);
            }
        }

        let vals: Vec<f64> = (0..9).map(|i| self.springs[i].step(dt)).collect();

        // Root: row with horizontal (left, flex=1) + vertical (right)
        let bg_node = cx.mount_child(Box::new(Label::new("")));
        let root_row =
            cx.tree
                .add_child(bg_node, Box::new(Flex::row().with_spacing(16.0)), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            bg_node,
            Container::new(root_row, 16.0).with_background(cx.app.theme.colors.surfaces.base0)
        );

        let theme = &cx.app.theme;

        // ─── Left: horizontal gauges in a column ─────────
        let col_id = cx.tree.add_child(
            root_row,
            Box::new(Flex::column().with_spacing(24.0)),
            None,
            0,
        );
        cx.tree.set_flex(col_id, FlexParams::flexible(1.0));

        // 1. Simple — Solid fill, tick marks + pointer
        let g1 = LinearGauge::new(LinearGaugeConfig {
            min: 0.0,
            max: 100.0,
            show_value: true,
            label: Some("Simple".to_string()),
            live: true,
            formatter: Some(Arc::new(|v: f64| format!("{:.1}", v))),
            tick_marks: GaugeTickMarks {
                major_divisions: 5,
                minor_per_major: 4,
                show_labels: true,
                major_length: 0.5,
                minor_length: 0.25,
            },
            show_pointer: true,
            corner_radius: 0.0,
            ..Default::default()
        })
        .with_value(vals[0]);
        cx.tree.add_child(col_id, Box::new(g1), None, 0);

        // 2. Zones — ZoneColored
        let g2 = LinearGauge::new(LinearGaugeConfig {
            min: 0.0,
            max: 100.0,
            fill_style: GaugeFillStyle::ZoneColored,
            zones: vec![
                GaugeZone {
                    start: 0.0,
                    end: 60.0,
                    color: theme.colors.semantic.success,
                },
                GaugeZone {
                    start: 60.0,
                    end: 80.0,
                    color: theme.colors.semantic.warning,
                },
                GaugeZone {
                    start: 80.0,
                    end: 100.0,
                    color: theme.colors.semantic.error,
                },
            ],
            show_value: true,
            label: Some("Zones".to_string()),
            live: true,
            formatter: Some(Arc::new(|v: f64| format!("{:.1}", v))),
            tick_marks: GaugeTickMarks {
                major_divisions: 5,
                minor_per_major: 1,
                show_labels: true,
                major_length: 0.4,
                minor_length: 0.2,
            },
            corner_radius: 0.0,
            ..Default::default()
        })
        .with_value(vals[1]);
        cx.tree.add_child(col_id, Box::new(g2), None, 1);

        // 3. Bidirectional — origin=0, range -50..50
        let g3 = LinearGauge::new(LinearGaugeConfig {
            min: -50.0,
            max: 50.0,
            origin: 0.0,
            show_value: true,
            label: Some("Bidirectional".to_string()),
            live: true,
            formatter: Some(Arc::new(|v: f64| format!("{:+.1}", v))),
            tick_marks: GaugeTickMarks {
                major_divisions: 4,
                minor_per_major: 4,
                show_labels: true,
                major_length: 0.5,
                minor_length: 0.25,
            },
            show_pointer: true,
            corner_radius: 0.0,
            ..Default::default()
        })
        .with_value(vals[2]);
        cx.tree.add_child(col_id, Box::new(g3), None, 2);

        // 4. Needle — thin marker
        let g4 = LinearGauge::new(LinearGaugeConfig {
            min: 0.0,
            max: 100.0,
            mode: GaugeMode::Needle,
            show_value: true,
            label: Some("Needle".to_string()),
            live: true,
            formatter: Some(Arc::new(|v: f64| format!("{:.1}", v))),
            tick_marks: GaugeTickMarks {
                major_divisions: 5,
                minor_per_major: 4,
                show_labels: true,
                major_length: 0.5,
                minor_length: 0.25,
            },
            corner_radius: 0.0,
            ..Default::default()
        })
        .with_value(vals[3]);
        cx.tree.add_child(col_id, Box::new(g4), None, 3);

        // 5. Progress — compact
        let g5 = LinearGauge::new(LinearGaugeConfig {
            min: 0.0,
            max: 100.0,
            bar_height: 20.0,
            corner_radius: 0.0,
            show_value: true,
            label: Some("Progress".to_string()),
            live: true,
            formatter: Some(Arc::new(|v: f64| format!("{:.0}%", v))),
            ..Default::default()
        })
        .with_value(vals[4]);
        cx.tree.add_child(col_id, Box::new(g5), None, 4);

        // ─── Right: vertical gauges in a row ─────────────
        let vert_row =
            cx.tree
                .add_child(root_row, Box::new(Flex::row().with_spacing(8.0)), None, 1);

        // V1. Simple vertical — solid fill + ticks
        let vg1 = LinearGauge::new(LinearGaugeConfig {
            min: 0.0,
            max: 100.0,
            orientation: LinearGaugeOrientation::Vertical,
            show_value: false,
            live: true,
            tick_marks: GaugeTickMarks {
                major_divisions: 5,
                minor_per_major: 1,
                show_labels: true,
                major_length: 0.4,
                minor_length: 0.2,
            },
            corner_radius: 0.0,
            ..Default::default()
        })
        .with_value(vals[5]);
        cx.tree.add_child(vert_row, Box::new(vg1), None, 0);

        // V2. Zone-colored vertical
        let vg2 = LinearGauge::new(LinearGaugeConfig {
            min: 0.0,
            max: 100.0,
            orientation: LinearGaugeOrientation::Vertical,
            tick_side: TickSide::Before,
            fill_style: GaugeFillStyle::ZoneColored,
            zones: vec![
                GaugeZone {
                    start: 0.0,
                    end: 60.0,
                    color: theme.colors.semantic.success,
                },
                GaugeZone {
                    start: 60.0,
                    end: 80.0,
                    color: theme.colors.semantic.warning,
                },
                GaugeZone {
                    start: 80.0,
                    end: 100.0,
                    color: theme.colors.semantic.error,
                },
            ],
            show_value: false,
            live: true,
            corner_radius: 0.0,
            ..Default::default()
        })
        .with_value(vals[6]);
        cx.tree.add_child(vert_row, Box::new(vg2), None, 1);

        // V3. Bidirectional vertical
        let vg3 = LinearGauge::new(LinearGaugeConfig {
            min: -50.0,
            max: 50.0,
            origin: 0.0,
            orientation: LinearGaugeOrientation::Vertical,
            tick_side: TickSide::Before,
            show_value: false,
            live: true,
            show_pointer: true,
            corner_radius: 0.0,
            ..Default::default()
        })
        .with_value(vals[7]);
        cx.tree.add_child(vert_row, Box::new(vg3), None, 2);

        // V4. Needle vertical
        let vg4 = LinearGauge::new(LinearGaugeConfig {
            min: 0.0,
            max: 100.0,
            orientation: LinearGaugeOrientation::Vertical,
            mode: GaugeMode::Needle,
            show_value: false,
            live: true,
            tick_marks: GaugeTickMarks {
                major_divisions: 5,
                minor_per_major: 0,
                show_labels: true,
                major_length: 0.4,
                minor_length: 0.2,
            },
            corner_radius: 0.0,
            ..Default::default()
        })
        .with_value(vals[8]);
        cx.tree.add_child(vert_row, Box::new(vg4), None, 3);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn take_rebuild_flag(&mut self) -> bool {
        true
    }
}
