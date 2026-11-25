// SPEC-HAL-226: TooltipSurface — Overlay-Only, Non-Action Informational Surface
//
// Binding invariants (mechanically true):
// INV-226-A1: Informational only — no actions, no selection, no commands
// INV-226-A2: No app state ownership
// INV-226-A3: Explicit content injection
// INV-226-B1: No timers inside tooltip
// INV-226-B2: Immediate rendering
// INV-226-C1: Non-interactive — no hit regions, no pointer/keyboard capture
// INV-226-C2: No input hijack — underlying UI remains interactive
// INV-226-C3: External dismissal policy — tooltip never decides when to hide
// INV-226-D1: Overlay-only rendering — no layout impact
// INV-226-D2: Explicit anchor fidelity
// INV-226-D3: Self-contained geometry — size = content + theme
// INV-226-D4: Deterministic — same inputs → same output
// INV-226-D5: Allocation discipline — no per-frame allocations

use crate::core::event::{Event, EventPhase};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::{LAYER_OVERLAY, TextBackgroundSpan};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::tooltip::TooltipPayload;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};

pub(crate) const TOOLTIP_PADDING_X: f32 = 8.0;
pub(crate) const TOOLTIP_PADDING_Y: f32 = 4.0;
pub(crate) const TOOLTIP_BORDER_WIDTH: f32 = 1.0;

pub struct TooltipSurface {
    id: WidgetInternalId,
    visible: bool,
    anchor: Point,
    payload: Option<TooltipPayload>,
}

impl TooltipSurface {
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            visible: false,
            anchor: Point::zero(),
            payload: None,
        }
    }

    /// INV-226-C3: External dismissal — caller controls visibility.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// INV-226-D2: Explicit anchor fidelity.
    pub fn set_anchor(&mut self, anchor: Point) {
        self.anchor = anchor;
    }

    /// INV-226-A3: Explicit content injection.
    pub fn set_content(&mut self, payload: TooltipPayload) {
        self.payload = Some(payload);
    }

    /// SPEC-HAL-226A: Paint a tooltip overlay without requiring a widget instance.
    /// Used by the framework tooltip manager to paint directly into the scene.
    pub fn paint_standalone(cx: &mut PaintContext, anchor: Point, content: &TooltipPayload) {
        let text = match content {
            TooltipPayload::Text(s) => s.as_str(),
        };

        let font_size = cx.theme.typography.font_size;
        let bg_color = cx.theme.colors.surfaces.level3;
        let border_color = cx.theme.colors.borders.default;
        let text_color = cx.theme.colors.typography.primary;

        let run = cx.text_engine.shape(text, font_size);
        let tooltip_width = run.width + TOOLTIP_PADDING_X * 2.0;
        let tooltip_height = run.height + TOOLTIP_PADDING_Y * 2.0;

        let tooltip_rect = RectF {
            x: anchor.x,
            y: anchor.y,
            width: tooltip_width,
            height: tooltip_height,
        };

        // Shadow
        let s = &cx.theme.shadows;
        cx.scene.push_shadow(
            tooltip_rect,
            0.0,
            crate::core::paint::ShadowMode::Drop,
            (0.0, s.tooltip_offset_y),
            0.0,
            s.tooltip_blur,
            crate::core::paint::Color::from_srgb(0.0, 0.0, 0.0, s.tooltip_alpha),
            LAYER_OVERLAY,
            None,
        );

        // Border
        cx.scene.push_border_rect(
            tooltip_rect,
            border_color,
            TOOLTIP_BORDER_WIDTH,
            LAYER_OVERLAY,
            None,
        );

        // Background + text
        let text_x = tooltip_rect.x + TOOLTIP_PADDING_X;
        let text_y = tooltip_rect.y + TOOLTIP_PADDING_Y;
        cx.scene.push_text_with_backgrounds(
            &run,
            text_x,
            text_y,
            text_color,
            vec![TextBackgroundSpan {
                x_offset: -TOOLTIP_PADDING_X,
                y_offset: -TOOLTIP_PADDING_Y,
                width: tooltip_width,
                height: tooltip_height,
                color: bg_color,
            }],
            LAYER_OVERLAY + 1,
            None,
        );
    }
}

impl WidgetDowncast for TooltipSurface {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TooltipSurface {
    // INV-226-D1: Zero-size layout — no layout participation, no reflow.
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(0.0, 0.0))
    }

    // INV-226-D1: No primary paint — overlay only.
    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    // INV-226-D1, D2, D3: Overlay-only rendering at explicit anchor.
    // No OverlayHitRegion registered (INV-226-C1: non-interactive).
    fn paint_overlay(&self, cx: &mut PaintContext, _rect: RectF, _tree_widget_id: TreeWidgetId) {
        if !self.visible {
            return;
        }
        let text = match &self.payload {
            Some(TooltipPayload::Text(s)) => s.as_str(),
            None => return,
        };

        let font_size = cx.theme.typography.font_size;
        let bg_color = cx.theme.colors.surfaces.level3;
        let border_color = cx.theme.colors.borders.default;
        let text_color = cx.theme.colors.typography.primary;

        // INV-226-D3: Size = content + theme only.
        let run = cx.text_engine.shape(text, font_size);
        let tooltip_width = run.width + TOOLTIP_PADDING_X * 2.0;
        let tooltip_height = run.height + TOOLTIP_PADDING_Y * 2.0;

        let tooltip_rect = RectF {
            x: self.anchor.x,
            y: self.anchor.y,
            width: tooltip_width,
            height: tooltip_height,
        };

        // SPEC-R-013-v2: Subtle drop shadow for tooltip elevation
        let s = &cx.theme.shadows;
        cx.scene.push_shadow(
            tooltip_rect,
            0.0, // sharp corners
            crate::core::paint::ShadowMode::Drop,
            (0.0, s.tooltip_offset_y),
            0.0,
            s.tooltip_blur,
            crate::core::paint::Color::from_srgb(0.0, 0.0, 0.0, s.tooltip_alpha),
            LAYER_OVERLAY, // same layer, will be behind border due to order
            None,
        );

        // Border (push_border_rect is safe at LAYER_OVERLAY — only push_rect occludes text)
        cx.scene.push_border_rect(
            tooltip_rect,
            border_color,
            TOOLTIP_BORDER_WIDTH,
            LAYER_OVERLAY,
            None,
        );

        // Background + text via TextBackgroundSpan (push_rect would occlude overlay text)
        let text_x = tooltip_rect.x + TOOLTIP_PADDING_X;
        let text_y = tooltip_rect.y + TOOLTIP_PADDING_Y;
        cx.scene.push_text_with_backgrounds(
            &run,
            text_x,
            text_y,
            text_color,
            vec![TextBackgroundSpan {
                x_offset: -TOOLTIP_PADDING_X,
                y_offset: -TOOLTIP_PADDING_Y,
                width: tooltip_width,
                height: tooltip_height,
                color: bg_color,
            }],
            LAYER_OVERLAY + 1,
            None,
        );
    }

    // INV-226-C1: Non-interactive — always returns false.
    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    // INV-226-C1: No hit regions.
    fn hit_testable(&self) -> bool {
        false
    }
}
