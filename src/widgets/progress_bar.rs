// SPEC-HAL-211: ProgressBar (Determinate Progress)
// Display-only observer. Renders externally provided progress value.
// SPEC-HAL-208: Integrated label support — label is owned, not a child widget.
//
// INV-A1: No state authority. Renders values pushed from application layer.
// INV-A2: No async ownership.
// INV-A3: No implicit transitions.
// INV-B1: Explicit progress value (f32, domain 0.0..1.0).
// INV-B2: Clamping is explicit and documented (clamp to 0.0..1.0).
// INV-B3: Stateless rendering — depends only on current value.
// INV-D1: Non-interactive (no focus, no pointer, no keyboard).
// INV-D2: Geometry is self-contained (track + fill + optional label).
// INV-D3: Optional label is visual only.
// INV-E1: No time authority.
// INV-F1: Application-layer feeding only.
// INV-F2: No change emission (no callbacks, no bindings, no writers).
// INV-G1: Disabled is visual only (color/emphasis only).
// INV-H1: Deterministic rendering.
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

// ============================================================================
// Widget (SPEC-HAL-211)
// ============================================================================

/// ProgressBar widget — determinate progress visualization.
///
/// Display-only. Renders a horizontal track with fill proportional to `value`.
/// Value domain is `0.0..=1.0`. Values outside this range are clamped (INV-B2).
///
/// This widget never accepts input, never emits events, never owns timing.
pub struct ProgressBar {
    id: WidgetInternalId,
    /// Progress value in 0.0..=1.0. Clamped on set (INV-B2: explicit, documented).
    value: f32,
    /// SPEC-HAL-208: Owned label text. Visual only (INV-D3).
    label: Option<String>,
    /// Visual-only disabled state (INV-G1).
    disabled: bool,
    display_style: TrackDisplayStyle,
}

impl ProgressBar {
    /// Create a new ProgressBar with value 0.0.
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            value: 0.0,
            label: None,
            disabled: false,
            display_style: TrackDisplayStyle::Track,
        }
    }

    /// Set initial value (builder pattern). Clamped to 0.0..=1.0 (INV-B2).
    pub fn with_value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    /// Set progress value. Clamped to 0.0..=1.0 (INV-B2: explicit, documented).
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
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

    /// Get current value.
    pub fn value(&self) -> f32 {
        self.value
    }

    fn paint_label(&self, cx: &mut PaintContext, rect: RectF, track_length: f32) {
        let theme = cx.theme;
        let geom = &theme.geometry.progress_bar;
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

    fn paint_track_style(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        let geom = &theme.geometry.progress_bar;

        let track_length = geom.track_length;
        let track_thickness = geom.track_thickness;
        let track_radius = geom.radius;

        // Track centered vertically
        let track_y = rect.y + (rect.height - track_thickness) * 0.5;
        let track_rect = RectF::new(rect.x, track_y, track_length, track_thickness);

        // Track background + border
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

        // INV-B3: Fill depends only on current value.
        let fill_width = track_length * self.value;
        if fill_width > 0.0 {
            let fill_rect = RectF::new(rect.x, track_y, fill_width, track_thickness);
            if self.disabled {
                let fill_color = theme.colors.borders.default;
                cx.scene.push_rounded_rect(
                    fill_rect,
                    fill_color,
                    RoundedCorners::uniform(track_radius),
                    3,
                    None,
                );
            } else {
                cx.paint_linear_gradient(
                    fill_rect,
                    track_radius,
                    (0.0, 0.0),
                    (1.0, 0.0),
                    &[
                        crate::core::paint::ColorStop {
                            offset: 0.0,
                            color: theme.colors.accents.primary,
                        },
                        crate::core::paint::ColorStop {
                            offset: 1.0,
                            color: theme.colors.accents.hover,
                        },
                    ],
                    3,
                );
            }
        }

        self.paint_label(cx, rect, track_length);
    }

    fn paint_bar_style(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        let geom = &theme.geometry.track_bar;
        let track_length = theme.geometry.progress_bar.track_length;
        let bar_height = geom.bar_height;

        let bar_y = rect.y + (rect.height - bar_height) * 0.5;
        let bar_rect = RectF::new(rect.x, bar_y, track_length, bar_height);

        // Background (rect pipeline only!)
        cx.scene
            .push_rect(bar_rect, theme.colors.surfaces.content, 1, None);
        cx.scene
            .push_border_rect(bar_rect, theme.colors.borders.default, 1.0, 2, None);

        // Fill
        let fill_width = track_length * self.value;
        if fill_width > 0.0 {
            let fill_rect = RectF::new(rect.x, bar_y, fill_width, bar_height);
            let fill_color = if self.disabled {
                theme.colors.borders.default
            } else {
                theme.colors.accents.primary
            };
            cx.scene.push_rect(fill_rect, fill_color, 3, None);
        }

        self.paint_label(cx, rect, track_length);
    }

    fn paint_pill_style(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        let geom = &theme.geometry.track_bar;
        let track_length = theme.geometry.progress_bar.track_length;
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

        // Fill
        let fill_width = track_length * self.value;
        if fill_width > 0.0 {
            let fill_rect = RectF::new(rect.x, bar_y, fill_width, bar_height);
            let fill_color = if self.disabled {
                theme.colors.borders.default
            } else {
                theme.colors.accents.primary
            };
            cx.scene
                .push_rounded_rect(fill_rect, fill_color, corners, 3, None);
        }

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

        let gauge_rect = RectF::new(rect.x, rect.y, size, size);
        let empty_zones = [(0.0, 0.0, [0.0; 4]); 4];
        cx.scene.push_radial_gauge(
            gauge_rect,
            self.value,
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

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for ProgressBar {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ProgressBar {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let geom = &cx.theme.geometry.progress_bar;
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

    fn debug_type_name(&self) -> &'static str {
        "ProgressBar"
    }
}
