// Gauge Demo — Four gauges with spring-damped tracking and visual polish

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::gauge::{
    Gauge, GaugeConfig, GaugeFillStyle, GaugeMode, GaugeTickMarks, GaugeZone,
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
        let damping = 40.0;

        let force = diff * stiffness - self.velocity * damping;
        self.velocity += force * dt;
        self.position += self.velocity * dt;
        self.position
    }
}

// ─── Screen ──────────────────────────────────────────────

fn main() {
    let app = App::new(AppConfig::new("Gauge Demo").with_size(800.0, 600.0));
    app.run(GaugeScreen::new());
}

struct GaugeScreen {
    placeholder: Label,
    generators: [JumpGenerator; 5],
    springs: [SpringDamper; 5],
    last_time: Instant,
}

impl GaugeScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            generators: [
                JumpGenerator::new(50.0, 0.0, 100.0, 0xDEAD_BEEF, 37),
                JumpGenerator::new(50.0, 0.0, 100.0, 0xCAFE_BABE, 62),
                JumpGenerator::new(50.0, 0.0, 100.0, 0x1337_C0DE, 100),
                JumpGenerator::new(0.0, -1.0, 1.0, 0xBAAD_F00D, 80),
                JumpGenerator::new(0.0, -50.0, 50.0, 0xFACE_CAFE, 120),
            ],
            springs: [
                SpringDamper::new(50.0),
                SpringDamper::new(50.0),
                SpringDamper::new(50.0),
                SpringDamper::new(0.0),
                SpringDamper::new(0.0),
            ],
            last_time: Instant::now(),
        }
    }
}

impl Screen for GaugeScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let now = Instant::now();
        let dt = now.duration_since(self.last_time).as_secs_f64().min(0.1);
        self.last_time = now;

        for i in 0..5 {
            if let Some(new_val) = self.generators[i].poll() {
                self.springs[i].set_target(new_val);
            }
        }

        let v1 = self.springs[0].step(dt);
        let v2 = self.springs[1].step(dt);
        let v3 = self.springs[2].step(dt);
        let v4 = self.springs[3].step(dt);
        let v5 = self.springs[4].step(dt);

        let bg_node = cx.mount_child(Box::new(Label::new("")));
        let col_id = cx.tree.add_child(
            bg_node,
            Box::new(Flex::column().with_spacing(16.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            bg_node,
            Container::new(col_id, 16.0).with_background(cx.app.theme.colors.surfaces.base0)
        );

        // ─── Row 1: RPM, TEMP, PROGRESS ─────────────────

        let row1 = Flex::row().with_spacing(24.0);
        let row1_id = cx.tree.add_child(col_id, Box::new(row1), None, 0);

        let g1 = Gauge::new()
            .with_config(GaugeConfig {
                min: 0.0,
                max: 100.0,
                origin: 0.0,
                arc_degrees: 270.0,
                zones: vec![
                    GaugeZone {
                        start: 0.0,
                        end: 60.0,
                        color: Color::from_hex(0x2ECC71),
                    },
                    GaugeZone {
                        start: 60.0,
                        end: 85.0,
                        color: Color::from_hex(0xF39C12),
                    },
                    GaugeZone {
                        start: 85.0,
                        end: 100.0,
                        color: Color::from_hex(0xE74C3C),
                    },
                ],
                mode: GaugeMode::Fill,
                label: Some("RPM".to_string()),
                fill_color: Some(Color::from_hex(0x3498DB)),
                fill_style: GaugeFillStyle::ZoneColored,
                show_pointer: true,
                formatter: Some(Arc::new(|v: f64| format!("{:.1}", v))),
                tick_marks: GaugeTickMarks {
                    major_divisions: 5,
                    minor_per_major: 1,
                    show_labels: true,
                    ..Default::default()
                },
                live: true,
                ..Default::default()
            })
            .with_value(v1);
        cx.tree.add_child(row1_id, Box::new(g1), None, 0);

        let g2 = Gauge::new()
            .with_config(GaugeConfig {
                min: 0.0,
                max: 100.0,
                origin: 0.0,
                arc_degrees: 180.0,
                mode: GaugeMode::Needle,
                label: Some("TEMP".to_string()),
                live: true,
                thickness: 0.12,
                fill_color: Some(Color::from_hex(0xE67E22)),
                formatter: Some(Arc::new(|v: f64| format!("{:.1}", v))),
                tick_marks: GaugeTickMarks {
                    major_divisions: 4,
                    minor_per_major: 0,
                    show_labels: true,
                    ..Default::default()
                },
                ..Default::default()
            })
            .with_value(v2);
        cx.tree.add_child(row1_id, Box::new(g2), None, 0);

        let g3 = Gauge::new()
            .with_config(GaugeConfig {
                min: 0.0,
                max: 100.0,
                origin: 0.0,
                arc_degrees: 360.0,
                mode: GaugeMode::Fill,
                thickness: 0.10,
                label: Some("PROGRESS".to_string()),
                live: true,
                fill_color: Some(Color::from_hex(0x1ABC9C)),
                fill_style: GaugeFillStyle::ValueLerp,
                formatter: Some(Arc::new(|v: f64| format!("{:.1}", v))),
                tick_marks: GaugeTickMarks {
                    major_divisions: 4,
                    minor_per_major: 2,
                    show_labels: true,
                    ..Default::default()
                },
                ..Default::default()
            })
            .with_value(v3);
        cx.tree.add_child(row1_id, Box::new(g3), None, 0);

        // ─── Row 2: DEVIATION + BALANCE ─────────────────

        let row2 = Flex::row().with_spacing(24.0);
        let row2_id = cx.tree.add_child(col_id, Box::new(row2), None, 0);

        let g4 = Gauge::new()
            .with_config(GaugeConfig {
                min: -1.0,
                max: 1.0,
                origin: 0.0,
                arc_degrees: 270.0,
                zones: vec![
                    GaugeZone {
                        start: -1.0,
                        end: -0.5,
                        color: Color::from_hex(0xE74C3C),
                    },
                    GaugeZone {
                        start: -0.5,
                        end: 0.5,
                        color: Color::from_hex(0x2ECC71),
                    },
                    GaugeZone {
                        start: 0.5,
                        end: 1.0,
                        color: Color::from_hex(0xE74C3C),
                    },
                ],
                mode: GaugeMode::Fill,
                label: Some("DEVIATION".to_string()),
                live: true,
                thickness: 0.12,
                fill_color: Some(Color::from_hex(0xF39C12)),
                fill_style: GaugeFillStyle::IntensityRamp,
                show_pointer: true,
                formatter: Some(Arc::new(|v: f64| format!("{:.2}", v))),
                tick_marks: GaugeTickMarks {
                    major_divisions: 4,
                    minor_per_major: 0,
                    show_labels: true,
                    ..Default::default()
                },
                ..Default::default()
            })
            .with_value(v4);
        cx.tree.add_child(row2_id, Box::new(g4), None, 0);

        let g5 = Gauge::new()
            .with_config(GaugeConfig {
                min: -50.0,
                max: 50.0,
                origin: 0.0,
                arc_degrees: 180.0,
                zones: vec![
                    GaugeZone {
                        start: -50.0,
                        end: -25.0,
                        color: Color::from_hex(0xE74C3C),
                    },
                    GaugeZone {
                        start: -25.0,
                        end: 25.0,
                        color: Color::from_hex(0x2ECC71),
                    },
                    GaugeZone {
                        start: 25.0,
                        end: 50.0,
                        color: Color::from_hex(0xE74C3C),
                    },
                ],
                mode: GaugeMode::Fill,
                fill_color: Some(Color::from_hex(0x3498DB)),
                fill_style: GaugeFillStyle::IntensityRamp,
                show_pointer: true,
                thickness: 0.12,
                label: Some("BALANCE".to_string()),
                live: true,
                formatter: Some(Arc::new(|v: f64| format!("{:+.1}", v))),
                tick_marks: GaugeTickMarks {
                    major_divisions: 4,
                    minor_per_major: 1,
                    show_labels: true,
                    ..Default::default()
                },
                ..Default::default()
            })
            .with_value(v5);
        cx.tree.add_child(row2_id, Box::new(g5), None, 0);

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
