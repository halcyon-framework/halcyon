// SPEC-HAL-321: TabSwitcher — content multiplexer for tab layouts.
//
// Owns no widgets; instead, takes its children directly from the tree
// and renders only the one at `active.get()`. Inactive children are
// still laid out (with tight zero constraints) so the engine's
// INV-PAINT-REQUIRES-LAYOUT-EPOCH invariant holds and they have a
// stamped `layout_epoch` for the current frame, but their rect is
// collapsed to zero so they paint nothing visible.
//
// Selection is external: a shared `Rc<Cell<usize>>` is passed in by
// the assembly that built this switcher, and the matching tab selector
// (typically a SegmentedControl) writes to it. The selector also has
// to mark this widget's tree node dirty when it writes — see
// `SegmentedControl::add_invalidate_target` and the assembly site in
// `process_create_tab_layout`.

use std::cell::Cell;
use std::rc::Rc;

use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

pub struct TabSwitcher {
    id: WidgetInternalId,
    active: Rc<Cell<usize>>,
}

impl TabSwitcher {
    pub fn new(active: Rc<Cell<usize>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            active,
        }
    }
}

impl WidgetDowncast for TabSwitcher {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TabSwitcher {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        if children.is_empty() {
            return LayoutResult::fit(Size::zero());
        }
        let target = self.active.get().min(children.len() - 1);
        let zero_constraints = Constraints::tight(Size::zero());
        let mut active_size = Size::zero();
        for (i, &child_id) in children.iter().enumerate() {
            if i == target {
                let size = cx.layout_child(child_id, constraints);
                cx.set_child_rect(child_id, RectF::new(0.0, 0.0, size.width, size.height));
                active_size = size;
            } else {
                let _ = cx.layout_child(child_id, zero_constraints);
                cx.set_child_rect(child_id, RectF::zero());
            }
        }
        LayoutResult::fit(active_size)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {
        // No own visuals; the framework paints children.
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "TabSwitcher"
    }
}
