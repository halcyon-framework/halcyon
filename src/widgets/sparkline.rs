// SPEC-HAL-308: Sparkline Widget — Miniature Inline Chart
//
// Display-only. Renders a filled area or line from consumer data.
// No GPU pipeline, no offscreen texture — scene graph Path only.
//
// INV-308-0: No interaction, no events.
// INV-308-3: No GPU pipeline overhead.

use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::Color;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::renderer::path::Path;
use crate::utils::id::WidgetInternalId;

pub struct SparklineConfig {
    /// Widget width in logical pixels. Default: 60.0.
    pub width: f32,
    /// Widget height in logical pixels. Default: 16.0.
    pub height: f32,
    /// Fill/line color. None = use theme accent at paint time.
    pub color: Option<Color>,
    /// True = filled area below line. False = line only.
    /// Phase 1: fill mode only. Line mode in Phase 2.
    pub fill: bool,
    /// If true, widget signals needs_animation() to drive continuous frames.
    /// Use for live/real-time data displays. Default: false.
    pub live: bool,
}

impl Default for SparklineConfig {
    fn default() -> Self {
        Self {
            width: 60.0,
            height: 16.0,
            color: None,
            fill: true,
            live: false,
        }
    }
}

pub struct Sparkline {
    id: WidgetInternalId,
    config: SparklineConfig,
    data: Vec<f64>,
}

impl Sparkline {
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            config: SparklineConfig::default(),
            data: Vec::new(),
        }
    }

    pub fn with_config(mut self, config: SparklineConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_data(mut self, data: Vec<f64>) -> Self {
        self.data = data;
        self
    }

    pub fn set_data(&mut self, data: Vec<f64>) {
        self.data = data;
    }
}

impl Default for Sparkline {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for Sparkline {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Sparkline {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(self.config.width, self.config.height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        if self.data.len() < 2 {
            return; // INV-308-4: empty or single value = nothing
        }

        let w = rect.width;
        let h = rect.height;
        let count = self.data.len();

        // Auto-scale Y (INV-308-1)
        let min = self.data.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = self.data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = if (max - min).abs() < f64::EPSILON {
            1.0 // flat line → center
        } else {
            max - min
        };

        // Map value to pixel Y (inverted: higher value = lower Y)
        let to_y = |v: f64| -> f32 { rect.y + h - ((v - min) / range) as f32 * h };
        let to_x = |i: usize| -> f32 { rect.x + (i as f32 / (count - 1) as f32) * w };

        // Build filled path: bottom-left → data points → bottom-right → close
        let mut path = Path::new();
        path.move_to(rect.x, rect.y + h); // bottom-left
        for i in 0..count {
            path.line_to(to_x(i), to_y(self.data[i]));
        }
        path.line_to(rect.x + w, rect.y + h); // bottom-right
        path.close();

        // INV-308-5: color from config or theme accent
        let color = self.config.color.unwrap_or(cx.theme.colors.accents.primary);

        cx.scene.push_path(path, color, 2, None);
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        false // INV-308-0
    }

    fn hit_testable(&self) -> bool {
        false // INV-308-0
    }

    fn needs_animation(&self) -> bool {
        self.config.live
    }

    fn debug_type_name(&self) -> &'static str {
        "Sparkline"
    }
}
