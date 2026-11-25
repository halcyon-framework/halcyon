// SPEC-HAL-307: Gauge Widget — Radial Arc Meter

use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_CONTENT};
use crate::core::widget::{LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast};
use crate::utils::id::WidgetInternalId;
use std::sync::Arc;

const DEFAULT_GAUGE_SIZE: f32 = 160.0;

/// SPEC-HAL-307: Gauge configuration.
pub struct GaugeConfig {
    /// Minimum value. Default: 0.0.
    pub min: f64,
    /// Maximum value. Default: 100.0.
    pub max: f64,
    /// Fill origin value. Fill arc extends FROM origin
    /// TOWARD the current value. Default: min.
    pub origin: f64,
    /// Arc sweep in degrees. Range: (0, 360]. Default: 270.
    pub arc_degrees: f32,
    /// Colored zones on the arc track. Maximum 4.
    pub zones: Vec<GaugeZone>,
    /// Display mode. Default: Fill.
    pub mode: GaugeMode,
    /// Show numeric value text. Default: true.
    pub show_value: bool,
    /// Value formatter. Default: "{:.0}".
    pub formatter: Option<Arc<dyn Fn(f64) -> String>>,
    /// Label below the value text. Default: None.
    pub label: Option<String>,
    /// Track thickness as fraction of radius. Default: 0.15.
    pub thickness: f32,
    /// Track background color. None = theme border subtle.
    pub track_color: Option<Color>,
    /// Optional widget background slab. None = inherit parent surface.
    pub background_color: Option<Color>,
    /// Fill arc color. None = theme accent primary.
    pub fill_color: Option<Color>,
    /// Fill color for negative direction (value < origin).
    /// When None, fill_color is used for both directions.
    /// When Some, this color is used for values below origin,
    /// fill_color for values above origin.
    pub fill_color_negative: Option<Color>,
    /// Fill rendering style. Default: Solid.
    pub fill_style: GaugeFillStyle,
    /// Tick marks on the arc. Default: none.
    pub tick_marks: GaugeTickMarks,
    /// Show a thin pointer line at the current value position.
    /// Overlaid on top of any fill style. Default: false.
    pub show_pointer: bool,
    /// If true, widget signals needs_animation() to drive continuous frames.
    /// Use for live/real-time data displays. Default: false.
    pub live: bool,
}

impl Default for GaugeConfig {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 100.0,
            origin: 0.0,
            arc_degrees: 270.0,
            zones: Vec::new(),
            mode: GaugeMode::Fill,
            show_value: true,
            formatter: None,
            label: None,
            thickness: 0.15,
            track_color: None,
            background_color: None,
            fill_color: None,
            fill_color_negative: None,
            fill_style: GaugeFillStyle::Solid,
            tick_marks: GaugeTickMarks::default(),
            show_pointer: false,
            live: false,
        }
    }
}

pub struct GaugeTickMarks {
    /// Number of intervals. 4 = ticks at 0%, 25%, 50%, 75%, 100% (5 ticks).
    /// 0 = no tick marks.
    pub major_divisions: u32,
    /// Minor ticks between each major. 0 = none.
    pub minor_per_major: u32,
    /// Show value labels at major tick positions.
    pub show_labels: bool,
    /// Major tick length as fraction of radius. Default: 0.08.
    pub major_length: f32,
    /// Minor tick length as fraction of radius. Default: 0.04.
    pub minor_length: f32,
}

impl Default for GaugeTickMarks {
    fn default() -> Self {
        Self {
            major_divisions: 0,
            minor_per_major: 0,
            show_labels: false,
            major_length: 0.08,
            minor_length: 0.04,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum GaugeFillStyle {
    /// Solid fill color overlaid on zones.
    #[default]
    Solid,
    /// Filled region shows zones at full brightness,
    /// unfilled region dimmed to 30%. No fill_color used.
    ZoneColored,
    /// Fill interpolates from fill_color toward zone color.
    ValueLerp,
    /// Transparent at origin, full at value.
    /// "The further you go, the more color you meet."
    IntensityRamp,
    /// Full at origin, transparent at value.
    /// "Color starts at the base and fades outward."
    IntensityRampReverse,
}

#[derive(Clone)]
pub struct GaugeZone {
    pub start: f64,
    pub end: f64,
    pub color: Color,
}

#[derive(Clone, Copy, PartialEq)]
pub enum GaugeMode {
    Fill,
    Needle,
}

pub struct Gauge {
    id: WidgetInternalId,
    config: GaugeConfig,
    /// Current value (prop, set by consumer).
    value: f64,
    /// Display value (set by with_value, used in paint).
    display_value: f64,
}

impl Gauge {
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            config: GaugeConfig::default(),
            value: 0.0,
            display_value: 0.0,
        }
    }

    pub fn with_config(mut self, config: GaugeConfig) -> Self {
        self.config = config;
        self
    }

    /// Set value (builder). Sets both value and display_value
    /// so the gauge renders at this position immediately.
    pub fn with_value(mut self, value: f64) -> Self {
        let clamped = value.clamp(self.config.min, self.config.max);
        self.value = clamped;
        self.display_value = clamped;
        self
    }

    /// Update current value. Widget eases toward it.
    pub fn set_value(&mut self, value: f64) {
        let clamped = value.clamp(self.config.min, self.config.max);
        self.value = clamped;
        self.display_value = clamped;
    }

    /// Map a value to normalized [0, 1] within the value range.
    fn normalize(&self, value: f64) -> f32 {
        let range = self.config.max - self.config.min;
        if range <= 0.0 {
            return 0.0;
        }
        ((value - self.config.min) / range).clamp(0.0, 1.0) as f32
    }
}

impl WidgetDowncast for Gauge {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Gauge {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[crate::core::widget_tree::NodeId],
    ) -> LayoutResult {
        // INV-307-4: Square layout. Use min dimension.
        let max_size = match (
            constraints.max_width.is_finite(),
            constraints.max_height.is_finite(),
        ) {
            (true, true) => constraints.max_width.min(constraints.max_height),
            (true, false) => constraints.max_width,
            (false, true) => constraints.max_height,
            (false, false) => DEFAULT_GAUGE_SIZE,
        };
        let min_size = constraints.min_width.max(constraints.min_height);
        let size = DEFAULT_GAUGE_SIZE.min(max_size).max(min_size.min(max_size));
        LayoutResult::fit(Size::new(size, size))
    }

    fn paint(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        _children: &[crate::core::widget_tree::NodeId],
    ) {
        let cfg = &self.config;
        let display_val = self.display_value;

        // Compute normalized fill position
        let fill = self.normalize(display_val);
        let origin_fill = self.normalize(if cfg.origin < cfg.min {
            cfg.min
        } else {
            cfg.origin
        });

        // Arc angles: convert degrees to normalized [0, 1]
        let sweep_norm = cfg.arc_degrees / 360.0;
        // Arc is centered with gap at bottom.
        // For 270°: gap=90° centered at bottom (180°).
        // Gap spans 135° to 225°. Arc starts at 225° = 225/360 = 0.625.
        // Formula: (360 - arc) / 2 / 360 + 0.5 then % 1.0
        let start_norm = if cfg.arc_degrees >= 360.0 {
            0.0
        } else {
            ((360.0 - cfg.arc_degrees) / 2.0 / 360.0 + 0.5) % 1.0
        };

        // Colors
        let track_color = cfg.track_color.unwrap_or(cx.theme.colors.gauge.track);
        let fill_color = cfg.fill_color.unwrap_or(cx.theme.colors.gauge.fill);
        let fill_color_negative = cfg.fill_color_negative.unwrap_or(fill_color);

        // Build zone array (max 4)
        let zone_count = cfg.zones.len().min(4) as u32;
        let mut zones = [(0.0f32, 0.0f32, [0.0f32; 4]); 4];
        for (i, zone) in cfg.zones.iter().take(4).enumerate() {
            zones[i] = (
                self.normalize(zone.start),
                self.normalize(zone.end),
                [zone.color.r, zone.color.g, zone.color.b, zone.color.a],
            );
        }

        let mut mode = match cfg.mode {
            GaugeMode::Fill => 0u32,
            GaugeMode::Needle => 1u32,
        };
        if cfg.show_pointer {
            mode |= 2;
        }

        let fill_style = match cfg.fill_style {
            GaugeFillStyle::Solid => 0u32,
            GaugeFillStyle::ZoneColored => 1u32,
            GaugeFillStyle::ValueLerp => 2u32,
            GaugeFillStyle::IntensityRamp => 3u32,
            GaugeFillStyle::IntensityRampReverse => 4u32,
        };

        // Optional background slab. Default is transparent/inherited; the
        // application surface or parent panel supplies the BG prepass primitive.
        if let Some(background_color) = cfg.background_color {
            cx.scene
                .push_rect(rect, background_color, LAYER_CONTENT, None);
        }

        // Tick mark params
        let tick_color = [0.6, 0.6, 0.6, 1.0];

        // Push the radial gauge primitive
        cx.scene.push_radial_gauge(
            rect,
            fill,
            cfg.thickness,
            track_color,
            fill_color,
            fill_color_negative,
            start_norm,
            sweep_norm,
            origin_fill,
            mode,
            fill_style,
            zone_count,
            zones,
            cfg.tick_marks.major_divisions,
            cfg.tick_marks.minor_per_major,
            cfg.tick_marks.major_length,
            cfg.tick_marks.minor_length,
            tick_color,
            LAYER_CONTENT + 2,
            None,
        );

        // Value labels at major tick positions
        if cfg.tick_marks.show_labels && cfg.tick_marks.major_divisions > 0 {
            let font_size = 11.0;
            let center_x = rect.x + rect.width * 0.5;
            let center_y = rect.y + rect.height * 0.5;
            let label_radius = rect.width * 0.5 * 0.56;
            let first_label = if cfg.arc_degrees >= 360.0 - f32::EPSILON {
                1
            } else {
                0
            };

            for i in first_label..=cfg.tick_marks.major_divisions {
                let t = i as f64 / cfg.tick_marks.major_divisions as f64;
                let value = cfg.min + t * (cfg.max - cfg.min);

                let start_rad = start_norm * std::f32::consts::TAU;
                let angle_rad = start_rad + (t as f32) * sweep_norm * std::f32::consts::TAU;

                let lx = center_x + label_radius * angle_rad.sin();
                let ly = center_y - label_radius * angle_rad.cos();

                let label_text = if (cfg.max - cfg.min).abs() >= 10.0 {
                    format!("{:.0}", value)
                } else {
                    format!("{:.1}", value)
                };
                let run = cx.text_engine.shape(&label_text, font_size);
                cx.text_engine.draw(
                    &run,
                    lx - run.width * 0.5,
                    ly - run.height * 0.5,
                    cx.theme.colors.typography.secondary,
                    LAYER_CONTENT + 3,
                    cx.scene,
                );
            }
        }

        // Value text
        if cfg.show_value {
            let text = if let Some(ref fmt) = cfg.formatter {
                fmt(display_val)
            } else {
                format!("{:.0}", display_val)
            };
            let run = cx.text_engine.shape(&text, 18.0);
            let text_x = rect.x + (rect.width - run.width) * 0.5;
            let text_y = rect.y + rect.height * 0.5 - run.height * 0.5;
            cx.text_engine.draw(
                &run,
                text_x,
                text_y,
                cx.theme.colors.typography.primary,
                LAYER_CONTENT + 3,
                cx.scene,
            );

            // Label below value
            if let Some(ref label) = cfg.label {
                let label_run = cx.text_engine.shape(label, 12.0);
                let label_x = rect.x + (rect.width - label_run.width) * 0.5;
                let label_y = text_y + run.height + 4.0;
                cx.text_engine.draw(
                    &label_run,
                    label_x,
                    label_y,
                    cx.theme.colors.typography.secondary,
                    LAYER_CONTENT + 3,
                    cx.scene,
                );
            }
        }
    }

    fn update(&mut self, _dt: f32, _cx: &mut UpdateContext) {
        // Projection-only: display_value set by with_value()
    }

    fn needs_animation(&self) -> bool {
        self.config.live
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "Gauge"
    }
}
