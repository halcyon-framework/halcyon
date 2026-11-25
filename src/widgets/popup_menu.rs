// SPEC-HAL-216: PopupMenu Overlay Widget
// Thin delegation to MenuSurface (SPEC-HAL-223).
// No local hover state, keyboard logic, or dismissal rules.

use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::popup_menu::PopupMenuHandle;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::menu_surface::MenuSurfaceOverlay;

const POPUP_MIN_WIDTH: f32 = 120.0;

pub struct PopupMenu {
    id: WidgetInternalId,
    controller: PopupMenuHandle,
    overlay: MenuSurfaceOverlay,
}

impl PopupMenu {
    pub fn new(controller: PopupMenuHandle) -> Self {
        Self {
            id: WidgetInternalId::new(),
            controller,
            overlay: MenuSurfaceOverlay::new(),
        }
    }
}

impl WidgetDowncast for PopupMenu {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for PopupMenu {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(0.0, 0.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        let ctrl = self.controller.borrow();
        if !ctrl.is_open() {
            return;
        }
        self.overlay.paint(
            cx,
            ctrl.surface(),
            (rect.x, rect.y),
            POPUP_MIN_WIDTH,
            tree_widget_id,
            Some(rect),
            rect,
        );
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        if !self.controller.borrow().is_open() {
            return false;
        }

        match event {
            // PopupMenu ignores non-Primary pointer events entirely (returns false).
            Event::Pointer(pe)
                if pe.kind == PointerEventKind::Down
                    && pe.button != Some(PointerButton::Primary) =>
            {
                false
            }
            _ => {
                let mut ctrl = self.controller.borrow_mut();
                let consumed = self
                    .overlay
                    .handle_event(ctrl.surface_mut(), event, false)
                    .unwrap_or(false);
                if consumed {
                    cx.invalidate_layout();
                }
                consumed
            }
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }
}
