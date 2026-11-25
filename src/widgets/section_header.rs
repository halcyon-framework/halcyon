// SPEC-HAL-214: SectionHeader Primitive
// Visual label for grouped content. Descriptive only — not operative.
// Read-only selection only; no children, no hierarchy, no navigation.
// No divider rendering — separation is handled by Divider (SPEC-HAL-213).
// INV-E3: Primary text emphasized by case/role, secondary regular.

use crate::core::event::{Event, EventPhase};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::text_selection::TextSelectionPolicy;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::auxiliary_text::AuxiliaryText;
use crate::widgets::text_block::TextBlock;

/// Theme-owned visual emphasis for a descriptive section label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SectionHeaderEmphasis {
    /// Primary section marker: uppercase and primary typography color.
    #[default]
    Primary,
    /// Dense subsection marker: caller-authored case and secondary color.
    Subtle,
}

/// Visual section label. Owns primary text and optional secondary text directly.
/// No children, no hierarchy, no dividers.
/// Theme-driven typography and colors.
pub struct SectionHeader {
    id: WidgetInternalId,
    primary_text: String,
    primary_block: TextBlock,
    secondary_block: Option<AuxiliaryText>,
    primary_size: Size,
    secondary_y: f32,
    emphasis: SectionHeaderEmphasis,
}

impl SectionHeader {
    pub fn new(primary: impl Into<String>) -> Self {
        let primary = primary.into();
        Self {
            id: WidgetInternalId::new(),
            primary_block: TextBlock::new(primary.to_ascii_uppercase())
                .with_selection_policy(TextSelectionPolicy::Enabled),
            primary_text: primary,
            secondary_block: None,
            primary_size: Size::zero(),
            secondary_y: 0.0,
            emphasis: SectionHeaderEmphasis::Primary,
        }
    }

    /// Select the visual emphasis for this descriptive header.
    pub fn with_emphasis(mut self, emphasis: SectionHeaderEmphasis) -> Self {
        self.emphasis = emphasis;
        let displayed = match emphasis {
            SectionHeaderEmphasis::Primary => self.primary_text.to_ascii_uppercase(),
            SectionHeaderEmphasis::Subtle => self.primary_text.clone(),
        };
        self.primary_block =
            TextBlock::new(displayed).with_selection_policy(TextSelectionPolicy::Enabled);
        self
    }

    /// Read the configured visual emphasis.
    pub fn emphasis(&self) -> SectionHeaderEmphasis {
        self.emphasis
    }

    pub fn with_secondary(mut self, secondary: impl Into<String>) -> Self {
        self.secondary_block = Some(AuxiliaryText::new(secondary.into()));
        self
    }

    /// INV-E3: Primary text font size — compact hierarchy marker.
    fn primary_font_size(base: f32) -> f32 {
        (base - 1.0).max(10.0).round()
    }

    #[cfg(test)]
    pub(crate) fn test_primary_text(&self) -> &str {
        self.primary_block.text()
    }

    #[cfg(test)]
    pub(crate) fn test_resolved_primary_font_size(&self, base: f32) -> f32 {
        Self::primary_font_size(base)
    }

    /// Gap between primary and secondary text — theme-proportional.
    fn text_gap(line_height: f32) -> f32 {
        (line_height * 0.2).round()
    }

    fn shifted_event(event: &Event, y_offset: f32) -> Event {
        match event {
            Event::Pointer(pe) => {
                let mut pe = pe.clone();
                pe.local_pos.1 -= y_offset;
                Event::Pointer(pe)
            }
            _ => event.clone(),
        }
    }
}

impl WidgetDowncast for SectionHeader {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SectionHeader {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let primary_font_size = Self::primary_font_size(cx.theme.typography.font_size);
        self.primary_block.set_font_size(primary_font_size);
        let primary_color = match self.emphasis {
            SectionHeaderEmphasis::Primary => cx.theme.colors.typography.primary,
            SectionHeaderEmphasis::Subtle => cx.theme.colors.typography.secondary,
        };
        self.primary_block.set_color(Some(primary_color));
        self.primary_block
            .set_selection_policy(TextSelectionPolicy::Enabled);

        let primary_result = self.primary_block.layout(cx, constraints, &[]);
        self.primary_size = primary_result.size;

        let mut width = self.primary_size.width;
        let mut height = self.primary_size.height;
        self.secondary_y = height;

        if let Some(ref mut secondary) = self.secondary_block {
            let gap = Self::text_gap(self.primary_size.height);
            self.secondary_y = height + gap;
            let secondary_max_h = (constraints.max_height - self.secondary_y).max(0.0);
            let secondary_result = secondary.layout(
                cx,
                Constraints::new(
                    constraints.min_width,
                    constraints.max_width,
                    0.0,
                    secondary_max_h,
                ),
                &[],
            );
            width = width.max(secondary_result.size.width);
            height = self.secondary_y + secondary_result.size.height;
        }

        LayoutResult::fit(Size::new(
            width.clamp(constraints.min_width, constraints.max_width),
            height.clamp(constraints.min_height, constraints.max_height),
        ))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.primary_block.paint(
            cx,
            RectF::new(rect.x, rect.y, rect.width, self.primary_size.height),
            &[],
        );

        if let Some(ref secondary) = self.secondary_block {
            secondary.paint(
                cx,
                RectF::new(
                    rect.x,
                    rect.y + self.secondary_y,
                    rect.width,
                    (rect.height - self.secondary_y).max(0.0),
                ),
                &[],
            );
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if let Event::Pointer(pe) = event {
            if pe.local_pos.1 >= self.secondary_y {
                if let Some(ref mut secondary) = self.secondary_block {
                    let shifted = Self::shifted_event(event, self.secondary_y);
                    return secondary.handle_event(cx, &shifted, phase);
                }
            }
            return self.primary_block.handle_event(cx, event, phase);
        }

        if self.primary_block.handle_event(cx, event, phase) {
            return true;
        }
        if let Some(ref mut secondary) = self.secondary_block {
            return secondary.handle_event(cx, event, phase);
        }
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }

    fn capture_eligible(&self) -> bool {
        true
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        self.primary_block.on_capture_leave(cx);
        if let Some(ref mut secondary) = self.secondary_block {
            secondary.on_capture_leave(cx);
        }
    }

    fn debug_type_name(&self) -> &'static str {
        "SectionHeader"
    }
}
