// SPEC-HAL-211: BusyIndicator (Indeterminate Progress)
// Display-only observer. Renders animated indeterminate "working" state.
// SPEC-HAL-208: Integrated label support — label is owned, not a child widget.
//
// INV-A1: No state authority.
// INV-A2: No async ownership.
// INV-A3: No implicit transitions.
// INV-C1: Busy is explicit (widget is always indeterminate by construction).
// INV-C2: Busy animation is purely visual (no progress, no duration, no completion).
// INV-C3: Busy does not transition to determinate.
// INV-D1: Non-interactive (no focus, no pointer, no keyboard).
// INV-D2: Geometry is self-contained.
// INV-D3: Optional label is visual only.
// INV-E1: No time authority (no clocks, no scheduling, no inferred elapsed time).
// INV-E2: Animation is frame-driven only (uses frame_time_ms from global frame loop).
// INV-F1: Application-layer feeding only.
// INV-F2: No change emission.
// INV-G1: Disabled is visual only.
// INV-H1: Deterministic rendering (same frame_time_ms → same output).
// INV-H2: Zero allocation steady state.

use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::LAYER_CONTENT;
use crate::core::paint::{RoundedCorners, TrackDisplayStyle};
use crate::core::style::{BorderStyle, DecorationStack, DecorationState};
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

/// Animation cycle duration in milliseconds.
/// INV-E1: Not a clock — just a modular constant for phase computation.
const CYCLE_MS: u64 = 1500;

// ============================================================================
// Widget (SPEC-HAL-211)
// ============================================================================

/// BusyIndicator widget — indeterminate progress visualization.
///
/// Display-only. Renders a horizontal track with a sliding indicator bar
/// that cycles continuously. Animation is driven entirely by `frame_time_ms`
/// from the global frame loop (INV-E2).
///
/// This widget never accepts input, never emits events, never owns timing.
/// The indicator is always indeterminate by construction (INV-C1).
pub struct BusyIndicator {
    id: WidgetInternalId,
    /// SPEC-HAL-208: Owned label text. Visual only (INV-D3).
    label: Option<String>,
    /// Visual-only disabled state (INV-G1).
    disabled: bool,
    display_style: TrackDisplayStyle,
}

impl BusyIndicator {
    /// Create a new BusyIndicator.
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: None,
            disabled: false,
            display_style: TrackDisplayStyle::Track,
        }
    }

    /// SPEC-HAL-208: Set label (builder pattern). Visual only (INV-D3).
    pub fn with_label(mut self, text: impl Into<String>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// Set disabled state. Visual only — affects colors, not logic (INV-G1).
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    /// Set the display style (builder pattern).
    pub fn with_display_style(mut self, style: TrackDisplayStyle) -> Self {
        self.display_style = style;
        self
    }

    fn paint_label(&self, cx: &mut PaintContext, rect: RectF, track_length: f32) {
        let theme = cx.theme;
        let geom = &theme.geometry.busy_indicator;
        if let Some(label_text) = &self.label {
            let text_x = rect.x + track_length + geom.label_gap;
            let font_size = theme.typography.font_size;
            let run = cx.text_engine.shape(label_text, font_size);
            let metrics = cx.text_engine.font_metrics(font_size);
            let baseline_y = rect.y
                + compute_control_text_baseline_y(
                    rect.height,
                    metrics.ascent,
                    metrics.descent,
                    theme.typography.baseline_offset,
                );
            let label_y = baseline_y - metrics.ascent;
            let text_color = if self.disabled {
                theme.colors.typography.muted
            } else {
                theme.colors.typography.primary
            };
            cx.text_engine.draw(
                &run,
                text_x,
                label_y,
                text_color,
                LAYER_CONTENT + 2,
                cx.scene,
            );
        }
    }

    /// Compute the ping-pong eased position for the sliding segment.
    fn compute_indicator_t(&self, cx: &PaintContext) -> f32 {
        let phase_raw = (cx.frame_time_ms() % CYCLE_MS) as f32 / CYCLE_MS as f32;
        let t = if phase_raw < 0.5 {
            phase_raw * 2.0
        } else {
            (1.0 - phase_raw) * 2.0
        };
        // Ease in-out for smooth motion
        t * t * (3.0 - 2.0 * t)
    }

    fn paint_track_style(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        let geom = &theme.geometry.busy_indicator;

        let track_length = geom.track_length;
        let track_thickness = geom.track_thickness;
        let track_radius = geom.radius;

        let track_y = rect.y + (rect.height - track_thickness) * 0.5;
        let track_rect = RectF::new(rect.x, track_y, track_length, track_thickness);

        let track_bg = theme.colors.surfaces.content;
        let track_border = theme.colors.borders.default;

        let stack = DecorationStack::background(track_bg, track_radius).with_border(
            BorderStyle::new(geom.border_thickness, track_border, track_radius),
        );

        let deco_state = DecorationState {
            focused: false,
            focus_t: 0.0,
            hover_t: 0.0,
            draw_hover: false,
        };
        stack.paint(cx.scene, track_rect, deco_state, 2);

        let indicator_width = track_length * geom.indicator_ratio;
        let travel = track_length - indicator_width;
        let eased = self.compute_indicator_t(cx);
        let indicator_x = rect.x + eased * travel;

        let indicator_rect = RectF::new(indicator_x, track_y, indicator_width, track_thickness);
        let indicator_color = if self.disabled {
            theme.colors.borders.default
        } else {
            theme.colors.accents.primary
        };
        cx.scene.push_rounded_rect(
            indicator_rect,
            indicator_color,
            RoundedCorners::uniform(track_radius),
            3,
            None,
        );

        self.paint_label(cx, rect, track_length);
    }

    fn paint_bar_style(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        let geom = &theme.geometry.track_bar;
        let busy_geom = &theme.geometry.busy_indicator;
        let track_length = busy_geom.track_length;
        let bar_height = geom.bar_height;

        let bar_y = rect.y + (rect.height - bar_height) * 0.5;
        let bar_rect = RectF::new(rect.x, bar_y, track_length, bar_height);

        // Background (rect pipeline only!)
        cx.scene
            .push_rect(bar_rect, theme.colors.surfaces.content, 1, None);
        cx.scene
            .push_border_rect(bar_rect, theme.colors.borders.default, 1.0, 2, None);

        // Animated sliding segment
        let indicator_width = track_length * busy_geom.indicator_ratio;
        let travel = track_length - indicator_width;
        let eased = self.compute_indicator_t(cx);
        let indicator_x = rect.x + eased * travel;

        let indicator_rect = RectF::new(indicator_x, bar_y, indicator_width, bar_height);
        let indicator_color = if self.disabled {
            theme.colors.borders.default
        } else {
            theme.colors.accents.primary
        };
        cx.scene.push_rect(indicator_rect, indicator_color, 3, None);

        self.paint_label(cx, rect, track_length);
    }

    fn paint_pill_style(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        let geom = &theme.geometry.track_bar;
        let busy_geom = &theme.geometry.busy_indicator;
        let track_length = busy_geom.track_length;
        let bar_height = geom.bar_height;
        let pill_radius = bar_height * 0.5;
        let corners = RoundedCorners::uniform(pill_radius);

        let bar_y = rect.y + (rect.height - bar_height) * 0.5;
        let bar_rect = RectF::new(rect.x, bar_y, track_length, bar_height);

        // Background (path pipeline only!)
        cx.scene
            .push_rounded_rect(bar_rect, theme.colors.surfaces.content, corners, 1, None);
        cx.scene.push_border_rounded_rect(
            bar_rect,
            theme.colors.borders.default,
            1.0,
            corners,
            2,
            None,
        );

        // Animated sliding segment
        let indicator_width = track_length * busy_geom.indicator_ratio;
        let travel = track_length - indicator_width;
        let eased = self.compute_indicator_t(cx);
        let indicator_x = rect.x + eased * travel;

        let indicator_rect = RectF::new(indicator_x, bar_y, indicator_width, bar_height);
        let indicator_color = if self.disabled {
            theme.colors.borders.default
        } else {
            theme.colors.accents.primary
        };
        cx.scene
            .push_rounded_rect(indicator_rect, indicator_color, corners, 3, None);

        self.paint_label(cx, rect, track_length);
    }

    fn paint_radial_style(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        let size = theme.geometry.track_bar.bar_height * 2.0;

        let track_color = theme.colors.surfaces.content;
        let fill_color = if self.disabled {
            theme.colors.borders.default
        } else {
            theme.colors.accents.primary
        };

        let fill = self.compute_indicator_t(cx);
        let gauge_rect = RectF::new(rect.x, rect.y, size, size);
        let empty_zones = [(0.0, 0.0, [0.0; 4]); 4];
        cx.scene.push_radial_gauge(
            gauge_rect,
            fill,
            0.15,
            track_color,
            fill_color,
            fill_color,
            0.0,
            1.0,
            0.0,
            0,
            0,
            0,
            empty_zones,
            0,
            0,
            0.0,
            0.0,
            [0.0; 4],
            2,
            None,
        );

        self.paint_label(cx, rect, size);
    }
}

impl Default for BusyIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for BusyIndicator {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for BusyIndicator {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let geom = &cx.theme.geometry.busy_indicator;

        if self.display_style == TrackDisplayStyle::Radial {
            let size = cx.theme.geometry.track_bar.bar_height * 2.0;
            let mut total_width = size;
            let mut height = size;
            if self.label.is_some() {
                let font_size = cx.theme.typography.font_size;
                let metrics = cx.text_engine.font_metrics(font_size);
                let label_height = metrics.ascent + metrics.descent;
                height = height.max(label_height);
            }
            if let Some(label_text) = &self.label {
                let font_size = cx.theme.typography.font_size;
                let (text_width, _) = cx.text_engine.measure(label_text, font_size);
                total_width += geom.label_gap + text_width;
            }
            let width = total_width.min(constraints.max_width);
            let height = height.clamp(constraints.min_height, constraints.max_height);
            return LayoutResult::fit(Size::new(width, height));
        }

        let track_height = match self.display_style {
            TrackDisplayStyle::Track => geom.track_thickness,
            TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => {
                cx.theme.geometry.track_bar.bar_height
            }
            TrackDisplayStyle::Radial => unreachable!(),
        };
        let track_width = geom.track_length;

        let mut total_width = track_width;
        let mut height = track_height;

        // Account for label height
        if self.label.is_some() {
            let font_size = cx.theme.typography.font_size;
            let metrics = cx.text_engine.font_metrics(font_size);
            let label_height = metrics.ascent + metrics.descent;
            height = height.max(label_height);
        }

        // Account for label width
        if let Some(label_text) = &self.label {
            let font_size = cx.theme.typography.font_size;
            let (text_width, _) = cx.text_engine.measure(label_text, font_size);
            total_width += geom.label_gap + text_width;
        }

        let width = total_width.min(constraints.max_width);
        let height = height.clamp(constraints.min_height, constraints.max_height);
        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        match self.display_style {
            TrackDisplayStyle::Track => self.paint_track_style(cx, rect),
            TrackDisplayStyle::Bar => self.paint_bar_style(cx, rect),
            TrackDisplayStyle::Pill => self.paint_pill_style(cx, rect),
            TrackDisplayStyle::Radial => self.paint_radial_style(cx, rect),
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    // INV-D1: Non-interactive. No hit-testing, no focus, no input.
    fn hit_testable(&self) -> bool {
        false
    }

    fn focusable(&self) -> bool {
        false
    }

    fn needs_animation(&self) -> bool {
        true
    }
}
