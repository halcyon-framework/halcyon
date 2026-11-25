// SPEC-HAL-311: LinearGauge — Horizontal/Vertical Bar Meter

use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_CONTENT, RoundedCorners};
use crate::core::widget::{LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast};
use crate::utils::id::WidgetInternalId;
use std::sync::Arc;

// Reuse shared types from radial gauge (INV-311-3)
pub use crate::widgets::gauge::{GaugeFillStyle, GaugeMode, GaugeTickMarks, GaugeZone};

pub enum LinearGaugeOrientation {
    Horizontal,
    Vertical,
}

impl Default for LinearGaugeOrientation {
    fn default() -> Self {
        Self::Horizontal
    }
}

/// Which side ticks/labels are drawn on.
/// Horizontal: Above or Below the bar. Vertical: Left or Right.
pub enum TickSide {
    /// Horizontal: ticks below bar. Vertical: ticks to the right.
    After,
    /// Horizontal: ticks above bar. Vertical: ticks to the left.
    Before,
}

impl Default for TickSide {
    fn default() -> Self {
        Self::After
    }
}

pub struct LinearGaugeConfig {
    pub min: f64,
    pub max: f64,
    /// Fill origin. Default: min (fill from left edge).
    pub origin: f64,
    pub orientation: LinearGaugeOrientation,
    /// Maximum 4 zones. Reuses GaugeZone.
    pub zones: Vec<GaugeZone>,
    pub mode: GaugeMode,
    pub fill_style: GaugeFillStyle,
    pub show_value: bool,
    pub formatter: Option<Arc<dyn Fn(f64) -> String>>,
    pub label: Option<String>,
    /// Bar thickness in logical pixels. Default: 16.0.
    pub bar_height: f32,
    pub track_color: Option<Color>,
    pub fill_color: Option<Color>,
    /// Corner radius. Default: 2.0. Set 0 for square.
    pub corner_radius: f32,
    /// Tick marks along the bar. Default: none (0 divisions).
    pub tick_marks: GaugeTickMarks,
    /// Which side ticks/labels are drawn on. Default: After (below/right).
    pub tick_side: TickSide,
    /// Show triangle pointer above bar at value position.
    pub show_pointer: bool,
    /// If true, widget signals needs_animation() to drive continuous frames.
    /// Use for live/real-time data displays. Default: false.
    pub live: bool,
}

impl Default for LinearGaugeConfig {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 100.0,
            origin: 0.0,
            orientation: LinearGaugeOrientation::Horizontal,
            zones: Vec::new(),
            mode: GaugeMode::Fill,
            fill_style: GaugeFillStyle::Solid,
            show_value: true,
            formatter: None,
            label: None,
            bar_height: 26.0,
            track_color: None,
            fill_color: None,
            corner_radius: 0.0,
            tick_marks: GaugeTickMarks::default(),
            tick_side: TickSide::After,
            show_pointer: false,
            live: false,
        }
    }
}

pub struct LinearGauge {
    id: WidgetInternalId,
    config: LinearGaugeConfig,
    value: f64,
}

impl LinearGauge {
    pub fn new(config: LinearGaugeConfig) -> Self {
        let value = config.min;
        Self {
            id: WidgetInternalId::new(),
            config,
            value,
        }
    }

    pub fn with_value(mut self, value: f64) -> Self {
        self.value = value.clamp(self.config.min, self.config.max);
        self
    }

    pub fn set_value(&mut self, value: f64) {
        self.value = value.clamp(self.config.min, self.config.max);
    }
}

/// Paint a `LinearGauge`'s scene primitives at `rect` using `cfg` and `value`.
///
/// Public entry point for linear-gauge rendering. The `LinearGauge` widget's
/// `paint` method delegates here so other call sites (e.g. custom
/// `CellPainter` implementations in tabular-view consumers) produce
/// identical output without instantiating a widget.
///
/// SPEC-HAL-311.
pub fn paint_linear_gauge(cx: &mut PaintContext, rect: RectF, cfg: &LinearGaugeConfig, value: f64) {
    let range = cfg.max - cfg.min;
    if range <= 0.0 {
        return;
    }

    let normalized = ((value - cfg.min) / range) as f32;
    let origin_t = ((cfg.origin.max(cfg.min) - cfg.min) / range) as f32;
    let normalized = normalized.clamp(0.0, 1.0);
    let origin_t = origin_t.clamp(0.0, 1.0);
    let radius = cfg.corner_radius;
    let track_color = cfg.track_color.unwrap_or(cx.theme.colors.gauge.track);

    let fill_color = match cfg.fill_style {
        GaugeFillStyle::ZoneColored => {
            let val_t = normalized as f64;
            cfg.zones
                .iter()
                .find(|z| {
                    let zs = (z.start - cfg.min) / range;
                    let ze = (z.end - cfg.min) / range;
                    val_t >= zs && val_t <= ze
                })
                .map(|z| z.color)
                .unwrap_or(cfg.fill_color.unwrap_or(cx.theme.colors.gauge.fill))
        }
        _ => cfg.fill_color.unwrap_or(cx.theme.colors.gauge.fill),
    };

    match cfg.orientation {
        LinearGaugeOrientation::Horizontal => paint_linear_gauge_horizontal(
            cx,
            rect,
            cfg,
            value,
            range,
            normalized,
            origin_t,
            radius,
            track_color,
            fill_color,
        ),
        LinearGaugeOrientation::Vertical => paint_linear_gauge_vertical(
            cx,
            rect,
            cfg,
            value,
            range,
            normalized,
            origin_t,
            radius,
            track_color,
            fill_color,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_linear_gauge_horizontal(
    cx: &mut PaintContext,
    rect: RectF,
    cfg: &LinearGaugeConfig,
    value: f64,
    range: f64,
    normalized: f32,
    origin_t: f32,
    radius: f32,
    track_color: Color,
    fill_color: Color,
) {
    let ticks_before = matches!(cfg.tick_side, TickSide::Before);
    let tick_len = if cfg.tick_marks.major_divisions > 0 {
        (cfg.tick_marks.major_length * cfg.bar_height).max(6.0)
    } else {
        0.0
    };
    let pointer_space = if cfg.show_pointer { 9.0 } else { 0.0 };

    // Pointer on opposite side from ticks; ticks After=below → pointer above
    let above_bar = if ticks_before {
        tick_len + pointer_space
    } else {
        pointer_space
    };
    let bar_rect = RectF::new(rect.x, rect.y + above_bar, rect.width, cfg.bar_height);

    // 1. Track
    cx.scene.push_rounded_rect(
        bar_rect,
        track_color,
        RoundedCorners::uniform(radius),
        LAYER_CONTENT + 2,
        None,
    );

    // 2. Zone backgrounds
    for zone in &cfg.zones {
        let zs = ((zone.start - cfg.min) / range) as f32;
        let ze = ((zone.end - cfg.min) / range) as f32;
        let (zs, ze) = (zs.clamp(0.0, 1.0), ze.clamp(0.0, 1.0));
        let zx = bar_rect.x + zs * bar_rect.width;
        let zw = (ze - zs) * bar_rect.width;
        if zw > 0.0 {
            let zc = match cfg.fill_style {
                GaugeFillStyle::ZoneColored => zone.color.with_alpha(0.4),
                _ => zone.color.with_alpha(0.25),
            };
            cx.scene.push_rounded_rect(
                RectF::new(zx, bar_rect.y, zw, bar_rect.height),
                zc,
                RoundedCorners::uniform(radius),
                LAYER_CONTENT + 2,
                None,
            );
        }
    }

    // 3. Fill
    let (fs, fe) = (origin_t.min(normalized), origin_t.max(normalized));
    let fx = bar_rect.x + fs * bar_rect.width;
    let fw = (fe - fs) * bar_rect.width;
    if fw > 0.5 {
        cx.scene.push_rounded_rect(
            RectF::new(fx, bar_rect.y, fw, bar_rect.height),
            fill_color,
            RoundedCorners::uniform(radius),
            LAYER_CONTENT + 3,
            None,
        );
    }

    // 4. Needle
    if cfg.mode == GaugeMode::Needle {
        let nx = bar_rect.x + normalized * bar_rect.width;
        let nc = cfg.fill_color.unwrap_or(cx.theme.colors.gauge.needle);
        cx.scene.push_rect(
            RectF::new(nx - 0.5, bar_rect.y, 1.0, bar_rect.height),
            nc,
            LAYER_CONTENT + 3,
            None,
        );
    }

    // 5. Tick marks
    if cfg.tick_marks.major_divisions > 0 {
        let total = cfg.tick_marks.major_divisions;
        for i in 0..=total {
            let t = i as f32 / total as f32;
            let tx = bar_rect.x + t * bar_rect.width;
            let ty = if ticks_before {
                bar_rect.y - tick_len
            } else {
                bar_rect.y + bar_rect.height
            };
            cx.scene.push_rect(
                RectF::new(tx - 0.5, ty, 1.0, tick_len),
                cx.theme.colors.gauge.tick,
                LAYER_CONTENT + 4,
                None,
            );
            if cfg.tick_marks.show_labels {
                let v = cfg.min + (i as f64 / total as f64) * (cfg.max - cfg.min);
                let lt = if (cfg.max - cfg.min).abs() >= 10.0 {
                    format!("{:.0}", v)
                } else {
                    format!("{:.1}", v)
                };
                let run = cx.text_engine.shape(&lt, 10.0);
                let lx = tx - run.width * 0.5;
                let ly = if ticks_before {
                    ty - run.height - 1.0
                } else {
                    ty + tick_len + 2.0
                };
                cx.text_engine.draw(
                    &run,
                    lx,
                    ly,
                    cx.theme.colors.gauge.tick_label,
                    LAYER_CONTENT + 6,
                    cx.scene,
                );
            }
            if i < total && cfg.tick_marks.minor_per_major > 0 {
                let mc = cfg.tick_marks.minor_per_major;
                let ml = (cfg.tick_marks.minor_length * bar_rect.height).max(3.0);
                for m in 1..=mc {
                    let mt = t + (m as f32 / ((mc + 1) as f32 * total as f32));
                    if mt > 1.0 {
                        break;
                    }
                    let mx = bar_rect.x + mt * bar_rect.width;
                    let my = if ticks_before {
                        bar_rect.y - ml
                    } else {
                        bar_rect.y + bar_rect.height
                    };
                    cx.scene.push_rect(
                        RectF::new(mx - 0.5, my, 1.0, ml),
                        cx.theme.colors.gauge.tick,
                        LAYER_CONTENT + 4,
                        None,
                    );
                }
            }
        }
    }

    // 6. Pointer (on opposite side from ticks)
    if cfg.show_pointer {
        let nx = bar_rect.x + normalized * bar_rect.width;
        let pc = cfg.fill_color.unwrap_or(cx.theme.colors.gauge.fill);
        let (ph, pw) = (8.0, 6.0);
        let mut path = crate::renderer::path::Path::new();
        if ticks_before {
            let tip = bar_rect.y + bar_rect.height + 1.0;
            path.move_to(nx, tip);
            path.line_to(nx - pw * 0.5, tip + ph);
            path.line_to(nx + pw * 0.5, tip + ph);
        } else {
            let tip = bar_rect.y - 1.0;
            path.move_to(nx, tip);
            path.line_to(nx - pw * 0.5, tip - ph);
            path.line_to(nx + pw * 0.5, tip - ph);
        }
        path.commands
            .push(crate::renderer::path::PathCommand::Close);
        cx.scene.push_path(path, pc, LAYER_CONTENT + 5, None);
    }

    // 7-8. Value + label (always at bottom of widget)
    let text_y_base = bar_rect.y
        + bar_rect.height
        + if !ticks_before {
            tick_len
                + if cfg.tick_marks.show_labels && tick_len > 0.0 {
                    let m = cx.text_engine.font_metrics(10.0);
                    m.ascent + m.descent.abs() + 2.0
                } else {
                    0.0
                }
        } else {
            0.0
        };
    if cfg.show_value {
        let text = match &cfg.formatter {
            Some(fmt) => fmt(value),
            None => format!("{:.0}", value),
        };
        let run = cx.text_engine.shape(&text, cx.theme.typography.font_size);
        let tx = bar_rect.x + (bar_rect.width - run.width) * 0.5;
        cx.text_engine.draw(
            &run,
            tx,
            text_y_base + 4.0,
            cx.theme.colors.typography.primary,
            LAYER_CONTENT + 6,
            cx.scene,
        );
    }
    if let Some(label) = &cfg.label {
        let fs = cx.theme.typography.font_size;
        let run = cx.text_engine.shape(label, fs);
        let tx = bar_rect.x + (bar_rect.width - run.width) * 0.5;
        let m = cx.text_engine.font_metrics(fs);
        let off = if cfg.show_value {
            m.ascent + m.descent.abs() + 4.0
        } else {
            0.0
        };
        cx.text_engine.draw(
            &run,
            tx,
            text_y_base + 4.0 + off,
            cx.theme.colors.typography.muted,
            LAYER_CONTENT + 6,
            cx.scene,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_linear_gauge_vertical(
    cx: &mut PaintContext,
    rect: RectF,
    cfg: &LinearGaugeConfig,
    value: f64,
    range: f64,
    normalized: f32,
    origin_t: f32,
    radius: f32,
    track_color: Color,
    fill_color: Color,
) {
    let bar_w = rect.width;
    let ticks_before = matches!(cfg.tick_side, TickSide::Before);
    let tick_len = if cfg.tick_marks.major_divisions > 0 {
        (cfg.tick_marks.major_length * bar_w).max(6.0)
    } else {
        0.0
    };
    let pointer_space = if cfg.show_pointer { 9.0 } else { 0.0 };

    // Compute tick/label space
    let mut tick_space = tick_len;
    if tick_len > 0.0 {
        tick_space += 2.0;
        if cfg.tick_marks.show_labels {
            tick_space += 30.0;
        }
    }

    // Bar x: ticks Before=left → bar shifts right; pointer on opposite side
    let bar_x = if ticks_before {
        rect.x + tick_space + pointer_space
    } else {
        rect.x + pointer_space
    };
    let bar_rect = RectF::new(bar_x, rect.y, bar_w, rect.height);

    // 1. Track
    cx.scene.push_rounded_rect(
        bar_rect,
        track_color,
        RoundedCorners::uniform(radius),
        LAYER_CONTENT + 2,
        None,
    );

    // 2. Zones (bottom-to-top)
    for zone in &cfg.zones {
        let zs = ((zone.start - cfg.min) / range) as f32;
        let ze = ((zone.end - cfg.min) / range) as f32;
        let (zs, ze) = (zs.clamp(0.0, 1.0), ze.clamp(0.0, 1.0));
        let zy = bar_rect.y + bar_rect.height - ze * bar_rect.height;
        let zh = (ze - zs) * bar_rect.height;
        if zh > 0.0 {
            let zc = match cfg.fill_style {
                GaugeFillStyle::ZoneColored => zone.color.with_alpha(0.4),
                _ => zone.color.with_alpha(0.25),
            };
            cx.scene.push_rounded_rect(
                RectF::new(bar_rect.x, zy, bar_w, zh),
                zc,
                RoundedCorners::uniform(radius),
                LAYER_CONTENT + 2,
                None,
            );
        }
    }

    // 3. Fill (bottom-to-top)
    let (fs, fe) = (origin_t.min(normalized), origin_t.max(normalized));
    let fh = (fe - fs) * bar_rect.height;
    let fy = bar_rect.y + bar_rect.height - fe * bar_rect.height;
    if fh > 0.5 {
        cx.scene.push_rounded_rect(
            RectF::new(bar_rect.x, fy, bar_w, fh),
            fill_color,
            RoundedCorners::uniform(radius),
            LAYER_CONTENT + 3,
            None,
        );
    }

    // 4. Needle
    if cfg.mode == GaugeMode::Needle {
        let ny = bar_rect.y + bar_rect.height - normalized * bar_rect.height;
        let nc = cfg.fill_color.unwrap_or(cx.theme.colors.gauge.needle);
        cx.scene.push_rect(
            RectF::new(bar_rect.x, ny - 0.5, bar_w, 1.0),
            nc,
            LAYER_CONTENT + 3,
            None,
        );
    }

    // 5. Ticks
    if cfg.tick_marks.major_divisions > 0 {
        let total = cfg.tick_marks.major_divisions;
        for i in 0..=total {
            let t = i as f32 / total as f32;
            let ty = bar_rect.y + bar_rect.height - t * bar_rect.height;
            if ticks_before {
                let tx = bar_rect.x - 2.0 - tick_len;
                cx.scene.push_rect(
                    RectF::new(tx, ty - 0.5, tick_len, 1.0),
                    cx.theme.colors.gauge.tick,
                    LAYER_CONTENT + 4,
                    None,
                );
                if cfg.tick_marks.show_labels {
                    let v = cfg.min + (i as f64 / total as f64) * (cfg.max - cfg.min);
                    let lt = if (cfg.max - cfg.min).abs() >= 10.0 {
                        format!("{:.0}", v)
                    } else {
                        format!("{:.1}", v)
                    };
                    let run = cx.text_engine.shape(&lt, 10.0);
                    cx.text_engine.draw(
                        &run,
                        tx - run.width - 2.0,
                        ty - run.height * 0.5,
                        cx.theme.colors.gauge.tick_label,
                        LAYER_CONTENT + 6,
                        cx.scene,
                    );
                }
            } else {
                let tx = bar_rect.x + bar_w + 2.0;
                cx.scene.push_rect(
                    RectF::new(tx, ty - 0.5, tick_len, 1.0),
                    cx.theme.colors.gauge.tick,
                    LAYER_CONTENT + 4,
                    None,
                );
                if cfg.tick_marks.show_labels {
                    let v = cfg.min + (i as f64 / total as f64) * (cfg.max - cfg.min);
                    let lt = if (cfg.max - cfg.min).abs() >= 10.0 {
                        format!("{:.0}", v)
                    } else {
                        format!("{:.1}", v)
                    };
                    let run = cx.text_engine.shape(&lt, 10.0);
                    cx.text_engine.draw(
                        &run,
                        tx + tick_len + 2.0,
                        ty - run.height * 0.5,
                        cx.theme.colors.gauge.tick_label,
                        LAYER_CONTENT + 6,
                        cx.scene,
                    );
                }
            }
            if i < total && cfg.tick_marks.minor_per_major > 0 {
                let mc = cfg.tick_marks.minor_per_major;
                let ml = (cfg.tick_marks.minor_length * bar_w).max(3.0);
                for m in 1..=mc {
                    let mt = t + (m as f32 / ((mc + 1) as f32 * total as f32));
                    if mt > 1.0 {
                        break;
                    }
                    let my = bar_rect.y + bar_rect.height - mt * bar_rect.height;
                    let mx = if ticks_before {
                        bar_rect.x - 2.0 - ml
                    } else {
                        bar_rect.x + bar_w + 2.0
                    };
                    cx.scene.push_rect(
                        RectF::new(mx, my - 0.5, ml, 1.0),
                        cx.theme.colors.gauge.tick,
                        LAYER_CONTENT + 4,
                        None,
                    );
                }
            }
        }
    }

    // 6. Pointer (on opposite side from ticks)
    if cfg.show_pointer {
        let ny = bar_rect.y + bar_rect.height - normalized * bar_rect.height;
        let pc = cfg.fill_color.unwrap_or(cx.theme.colors.gauge.fill);
        let (pw, ph) = (8.0, 6.0);
        let mut path = crate::renderer::path::Path::new();
        if ticks_before {
            let tip = bar_rect.x + bar_w + 1.0;
            path.move_to(tip, ny);
            path.line_to(tip + pw, ny - ph * 0.5);
            path.line_to(tip + pw, ny + ph * 0.5);
        } else {
            let tip = bar_rect.x - 1.0;
            path.move_to(tip, ny);
            path.line_to(tip - pw, ny - ph * 0.5);
            path.line_to(tip - pw, ny + ph * 0.5);
        }
        path.commands
            .push(crate::renderer::path::PathCommand::Close);
        cx.scene.push_path(path, pc, LAYER_CONTENT + 5, None);
    }

    // 7-8. Value + label (below the bar)
    if cfg.show_value {
        let text = match &cfg.formatter {
            Some(fmt) => fmt(value),
            None => format!("{:.0}", value),
        };
        let font_size = cx.theme.typography.font_size;
        let run = cx.text_engine.shape(&text, font_size);
        let tx = bar_rect.x + (bar_w - run.width) * 0.5;
        let ty = bar_rect.y + bar_rect.height + 4.0;
        cx.text_engine.draw(
            &run,
            tx,
            ty,
            cx.theme.colors.typography.primary,
            LAYER_CONTENT + 6,
            cx.scene,
        );
    }
    if let Some(label) = &cfg.label {
        let font_size = cx.theme.typography.font_size;
        let run = cx.text_engine.shape(label, font_size);
        let tx = bar_rect.x + (bar_w - run.width) * 0.5;
        let metrics = cx.text_engine.font_metrics(font_size);
        let line_h = metrics.ascent + metrics.descent.abs();
        let offset = if cfg.show_value { line_h + 4.0 } else { 0.0 };
        let ty = bar_rect.y + bar_rect.height + 4.0 + offset;
        cx.text_engine.draw(
            &run,
            tx,
            ty,
            cx.theme.colors.typography.muted,
            LAYER_CONTENT + 6,
            cx.scene,
        );
    }
}

impl WidgetDowncast for LinearGauge {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for LinearGauge {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[crate::core::widget_tree::NodeId],
    ) -> LayoutResult {
        let cfg = &self.config;

        match cfg.orientation {
            LinearGaugeOrientation::Horizontal => {
                let bar_h = cfg.bar_height;
                let mut total_h = bar_h;

                if cfg.show_pointer {
                    total_h += 9.0;
                }

                if cfg.tick_marks.major_divisions > 0 {
                    let tick_len = (cfg.tick_marks.major_length * cfg.bar_height).max(6.0);
                    total_h += tick_len;
                    if cfg.tick_marks.show_labels {
                        let label_metrics = cx.text_engine.font_metrics(10.0);
                        let label_h = label_metrics.ascent + label_metrics.descent.abs();
                        total_h += label_h + 2.0;
                    }
                }

                if cfg.show_value || cfg.label.is_some() {
                    let font_size = cx.theme.typography.font_size;
                    let metrics = cx.text_engine.font_metrics(font_size);
                    let line_h = metrics.ascent + metrics.descent.abs();
                    if cfg.show_value {
                        total_h += line_h + 4.0;
                    }
                    if cfg.label.is_some() {
                        total_h += line_h + 2.0;
                    }
                }

                let width = if constraints.max_width.is_finite() {
                    constraints.max_width
                } else {
                    200.0
                };

                LayoutResult::fit(Size::new(width, total_h))
            }
            LinearGaugeOrientation::Vertical => {
                let bar_w = cfg.bar_height;
                let mut total_w = bar_w;

                // Space for ticks + labels on tick_side
                if cfg.tick_marks.major_divisions > 0 {
                    let tick_len = (cfg.tick_marks.major_length * bar_w).max(6.0);
                    total_w += tick_len + 2.0;
                    if cfg.tick_marks.show_labels {
                        total_w += 30.0;
                    }
                }

                // Space for pointer on opposite side from ticks
                if cfg.show_pointer {
                    total_w += 9.0;
                }

                let mut extra_h = 0.0;
                if cfg.show_value || cfg.label.is_some() {
                    let font_size = cx.theme.typography.font_size;
                    let metrics = cx.text_engine.font_metrics(font_size);
                    let line_h = metrics.ascent + metrics.descent.abs();
                    if cfg.show_value {
                        extra_h += line_h + 4.0;
                    }
                    if cfg.label.is_some() {
                        extra_h += line_h + 2.0;
                    }
                }

                let height = if constraints.max_height.is_finite() {
                    constraints.max_height
                } else {
                    200.0 + extra_h
                };

                LayoutResult::fit(Size::new(total_w, height))
            }
        }
    }

    fn paint(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        _children: &[crate::core::widget_tree::NodeId],
    ) {
        paint_linear_gauge(cx, rect, &self.config, self.value);
    }

    fn update(&mut self, _dt: f32, _cx: &mut UpdateContext) {}

    fn needs_animation(&self) -> bool {
        self.config.live
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "LinearGauge"
    }
}
