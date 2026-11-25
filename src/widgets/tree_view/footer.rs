// =============================================================================
// TreeViewFooter: Footer widget for TreeView
// =============================================================================
//
// Port of ListViewFooter for TreeView. Footer is outside ScrollArea
// (same scroll domain as header): subtract scroll_x for scrollable columns.
//
// ## Invariants
//
// INV-FOOTER-NO-SELECTION: Footer does not own or participate in row selection.
// INV-FOOTER-NO-SCROLL: Footer is fixed, does not scroll.
// INV-FOOTER-READONLY-STATE: Footer reads hovered_separator and
//   active_resize_separator but never mutates shared state.
//
// =============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::event::{Event, EventPhase};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::tabular_view::column::ColumnScrollDomain;

use super::body::TreeViewCore;

const FOOTER_BACKGROUND_LAYER: u32 = 0;
const FOOTER_SEPARATOR_LAYER: u32 = 1;
const FOOTER_BORDER_LAYER: u32 = FOOTER_SEPARATOR_LAYER;
const FOOTER_INTERACTION_SEPARATOR_LAYER: u32 = 2;

/// Footer widget for TreeView.
///
/// Structural completeness widget — renders column separators aligned with
/// header and body. Hit-testable for resize arming via event delivery to TreeView.
///
/// Default handle_event returns false (pure visual). TreeView owns resize logic
/// for the footer band via Y-acceptance expansion.
pub struct TreeViewFooter {
    id: WidgetInternalId,
    core: Rc<RefCell<TreeViewCore>>,
}

impl TreeViewFooter {
    pub(crate) fn new(core: Rc<RefCell<TreeViewCore>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            core,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_view_footer_layers_keep_chrome_above_background() {
        assert!(FOOTER_BACKGROUND_LAYER < FOOTER_SEPARATOR_LAYER);
        assert!(FOOTER_SEPARATOR_LAYER <= FOOTER_BORDER_LAYER);
        assert!(FOOTER_BORDER_LAYER < FOOTER_INTERACTION_SEPARATOR_LAYER);
    }
}

impl WidgetDowncast for TreeViewFooter {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TreeViewFooter {
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

        let core = self.core.borrow();
        let theme = cx.theme;

        // Footer is a separate structural child; match ListView footer's local
        // paint ladder so clipped separators never share the background layer.
        cx.scene.push_rect(
            rect,
            theme.colors.list.header_background,
            FOOTER_BACKGROUND_LAYER,
            None,
        );

        // Scroll-domain: footer is outside ScrollArea (same as header).
        // For scrollable columns: visual_x = rect.x + slot.x - scroll_x
        // For pinned columns (future): visual_x = rect.x + slot.x
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());

        // Clip footer content.
        cx.scene.begin_clip(rect, None);

        let slots: Vec<_> = core.cms.hts().column_layout().header_slots().collect();
        let inter_column_slots = &slots[..slots.len().saturating_sub(1)];
        let sf = cx.scene.scale_factor();
        let snapped_x = snap_to_physical(rect.x, sf);

        // ---- Normal separators ----
        for slot in inter_column_slots {
            let is_active = core.active_resize_separator == Some(slot.column_id);
            let is_hovered = core.hovered_separator == Some(slot.column_id);
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
            cx.scene.push_rect(
                sep_rect,
                theme.colors.borders.default,
                FOOTER_SEPARATOR_LAYER,
                None,
            );
        }

        // Footer top border.
        let border_rect = RectF::new(rect.x, rect.y, rect.width, 1.0);
        cx.scene.push_rect(
            border_rect,
            theme.colors.borders.default,
            FOOTER_BORDER_LAYER,
            None,
        );

        // ---- Hovered/active separators (above border) ----
        for slot in inter_column_slots {
            let is_active = core.active_resize_separator == Some(slot.column_id);
            let is_hovered = core.hovered_separator == Some(slot.column_id);
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
            cx.scene.push_rect(
                sep_rect,
                sep_color,
                FOOTER_INTERACTION_SEPARATOR_LAYER,
                None,
            );
        }

        cx.scene.end_clip();
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        // Default: return false for all events.
        // TreeView owns resize hover/arm for the footer band via Y-acceptance expansion.
        // If TreeView cannot receive footer pointer events, this must be upgraded to
        // perform hover/arm itself (fallback mode).
        false
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "TreeViewFooter"
    }
}
