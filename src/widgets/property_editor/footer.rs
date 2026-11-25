// =============================================================================
// PropertyEditorFooter: Footer widget for PropertyEditor (SPEC-HAL-260A §4.1)
// =============================================================================
//
// Parallel to `tree_view/footer.rs`. Footer is outside the ScrollArea (same
// scroll domain as the header): subtract `scroll_x` for scrollable columns
// so the separators paint aligned with the body's columns.
//
// PE's footer differs from TreeView's in one respect: when one or more rows
// are selected, the footer displays a right-aligned "N selected" label per
// Step 4 §I1. When the selection is empty, the label is omitted and the
// footer is structural-only (matches TreeView exactly).
//
// ## Invariants
//
// INV-PE-FOOTER-NO-SELECTION: Footer does not own or participate in row
//   selection. It reads `core.selection` only.
// INV-PE-FOOTER-NO-SCROLL: Footer is fixed; it does not scroll vertically.
// INV-PE-FOOTER-READONLY-STATE: Footer reads `hovered_separator` and
//   `active_resize_separator` but never mutates shared state.
//
// =============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::event::{Event, EventPhase};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::paint::LAYER_CONTENT;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::tabular::column::ColumnScrollDomain;
use crate::utils::id::WidgetInternalId;

use super::core::PropertyEditorCore;

/// Footer widget for `PropertyEditor`. Structural completeness widget —
/// renders column separators aligned with header and body, plus an
/// optional "N selected" label on the right edge.
///
/// Pure visual: `handle_event` returns `false`. PE Outer owns separator
/// hit-test on the header band; the footer band is not currently part of
/// the resize hit zone (mirrors TreeView's default behaviour).
pub struct PropertyEditorFooter {
    id: WidgetInternalId,
    core: Rc<RefCell<PropertyEditorCore>>,
}

impl PropertyEditorFooter {
    pub(crate) fn new(core: Rc<RefCell<PropertyEditorCore>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            core,
        }
    }
}

impl WidgetDowncast for PropertyEditorFooter {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for PropertyEditorFooter {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let size = Size::new(constraints.max_width, constraints.max_height);
        LayoutResult::new(size, size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        if rect.height <= 0.0 {
            return;
        }

        let theme = cx.theme;

        // Footer background at LAYER_CONTENT (must not be occluded by
        // outer's background).
        cx.scene.push_rect(
            rect,
            theme.colors.list.header_background,
            LAYER_CONTENT,
            None,
        );

        // Single short borrow gathers everything paint needs.
        let core = self.core.borrow();
        let slots: Vec<_> = core.cms.hts().column_layout().header_slots().collect();
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        let hovered_sep = core.hovered_separator;
        let active_sep = core.active_resize_separator;
        let selected_count = core.selection.selected_keys.len();
        drop(core);

        let sf = cx.scene.scale_factor();
        let snapped_x = snap_to_physical(rect.x, sf);
        let separator_color = theme.colors.borders.default;

        // Clip footer content (matches TreeView footer).
        cx.scene.begin_clip(rect, None);

        let inter_column_slots = &slots[..slots.len().saturating_sub(1)];

        // ---- Pass 1: normal (non-hovered, non-active) separators ----
        for slot in inter_column_slots {
            let is_active = active_sep == Some(slot.column_id);
            let is_hovered = hovered_sep == Some(slot.column_id);
            if is_active || is_hovered {
                continue;
            }
            let visual_x = if slot.scroll_domain == ColumnScrollDomain::Scrollable {
                snapped_x + slot.x - scroll_x
            } else {
                snapped_x + slot.x
            };
            let sep_x = snap_to_physical(visual_x + slot.width - 1.0, sf);
            let sep_rect = RectF::new(sep_x, rect.y, 1.0, rect.height);
            cx.scene
                .push_rect(sep_rect, separator_color, LAYER_CONTENT, None);
        }

        // Footer top border (full width).
        let border_rect = RectF::new(rect.x, rect.y, rect.width, 1.0);
        cx.scene
            .push_rect(border_rect, separator_color, LAYER_CONTENT, None);

        // ---- Pass 2: hovered/active separators (above the border) ----
        for slot in inter_column_slots {
            let is_active = active_sep == Some(slot.column_id);
            let is_hovered = hovered_sep == Some(slot.column_id);
            if !is_active && !is_hovered {
                continue;
            }
            let visual_x = if slot.scroll_domain == ColumnScrollDomain::Scrollable {
                snapped_x + slot.x - scroll_x
            } else {
                snapped_x + slot.x
            };
            let sep_x = snap_to_physical(visual_x + slot.width - 1.0, sf);
            let (sep_w, sep_color) = if is_active {
                (2.0, theme.colors.borders.focus)
            } else {
                (2.0, theme.colors.borders.default)
            };
            let sep_rect = RectF::new(sep_x, rect.y, sep_w, rect.height);
            cx.scene.push_rect(sep_rect, sep_color, LAYER_CONTENT, None);
        }

        // ---- Optional "N selected" label ----
        // Right-aligned inside the footer band. Suppressed when nothing
        // is selected (matches the TreeView footer's structural-only
        // appearance in that case).
        if selected_count > 0 {
            let label = format!("{} selected", selected_count);
            let font_size = theme.typography.font_size;
            let text_color = theme.colors.typography.secondary;
            let text_padding = theme.list_geom.padding_x;
            let run = cx.text_engine.shape(&label, font_size);
            let text_x = rect.x + rect.width - run.width - text_padding;
            let text_y = rect.y + (rect.height - run.height) / 2.0;
            cx.text_engine.draw(
                &run,
                text_x,
                text_y,
                text_color,
                LAYER_CONTENT + 1,
                cx.scene,
            );
        }

        cx.scene.end_clip();
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        // Pure visual: PE Outer owns separator hit-test on the header
        // band; footer-band resize-arming is not in I1's scope.
        false
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "PropertyEditorFooter"
    }
}
