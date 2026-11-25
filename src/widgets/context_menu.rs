// SPEC-HAL-217: ContextMenu Overlay Widget
// Thin delegation to MenuSurface (SPEC-HAL-223).
// Anchored to explicit position. No local hover state or dismissal ownership.

use std::cell::Cell;
use std::rc::Rc;

use crate::core::app::{Screen, ScreenContext, ScreenFrameContext};
use crate::core::event::{Event, EventPhase, FusedInputEvent, KeyCode};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::overlay_root::DismissPolicy;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::context_menu::ContextMenuHandle;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::label::Label;
use crate::widgets::menu_surface::MenuSurfaceOverlay;

const POPUP_MIN_WIDTH: f32 = 160.0;

pub struct ContextMenu {
    id: WidgetInternalId,
    controller: ContextMenuHandle,
    overlay: MenuSurfaceOverlay,
    popup_rect_sink: Option<Rc<Cell<RectF>>>,
}

impl ContextMenu {
    pub fn new(controller: ContextMenuHandle) -> Self {
        Self {
            id: WidgetInternalId::new(),
            controller,
            overlay: MenuSurfaceOverlay::new(),
            popup_rect_sink: None,
        }
    }

    pub fn with_popup_rect_sink(mut self, popup_rect_sink: Rc<Cell<RectF>>) -> Self {
        self.popup_rect_sink = Some(popup_rect_sink);
        self
    }

    fn update_popup_rect(&self, cx: &mut LayoutContext<'_>) {
        let Some(sink) = &self.popup_rect_sink else {
            return;
        };

        let ctrl = self.controller.borrow();
        if !ctrl.is_open() {
            sink.set(RectF::zero());
            return;
        }
        let Some(anchor) = ctrl.anchor() else {
            sink.set(RectF::zero());
            return;
        };

        let rect = MenuSurfaceOverlay::compute_popup_rect(
            cx.text_engine,
            cx.theme,
            ctrl.surface(),
            anchor.origin(),
            POPUP_MIN_WIDTH,
        );
        sink.set(rect);
    }
}

impl WidgetDowncast for ContextMenu {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ContextMenu {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        self.update_popup_rect(cx);
        // Keep the overlay controller in paint traversal; fully collapsed
        // children are skipped before paint_overlay is called.
        LayoutResult::fit(Size::new(1.0, 1.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        let ctrl = self.controller.borrow();
        if !ctrl.is_open() {
            return;
        }
        let anchor = match ctrl.anchor() {
            Some(a) => a,
            None => return,
        };
        let (anchor_x, anchor_y) = anchor.origin();

        if let Some(sink) = &self.popup_rect_sink {
            let rect = MenuSurfaceOverlay::compute_popup_rect(
                cx.text_engine,
                cx.theme,
                ctrl.surface(),
                (anchor_x, anchor_y),
                POPUP_MIN_WIDTH,
            );
            sink.set(rect);
        }

        // ContextMenu uses popup_rect as owner_abs_rect (None → overlay computes from popup).
        // Widget rect passed for coordinate offset computation.
        self.overlay.paint(
            cx,
            ctrl.surface(),
            (anchor_x, anchor_y),
            POPUP_MIN_WIDTH,
            tree_widget_id,
            None,
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

        // ContextMenu routes all events (any button dismisses outside popup).
        let mut ctrl = self.controller.borrow_mut();
        let previous_highlight = ctrl.surface().highlighted_index();
        let consumed = self
            .overlay
            .handle_event(ctrl.surface_mut(), event, false)
            .unwrap_or(false);
        if consumed || previous_highlight != ctrl.surface().highlighted_index() {
            cx.invalidate_layout();
        }
        consumed
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

pub struct ContextMenuScreen {
    controller: ContextMenuHandle,
    popup_rect: Rc<Cell<RectF>>,
    needs_rebuild: bool,
    placeholder: Label,
}

impl ContextMenuScreen {
    pub fn new(controller: ContextMenuHandle) -> Self {
        Self {
            controller,
            popup_rect: Rc::new(Cell::new(RectF::zero())),
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }

    fn update_menu_with<F>(&mut self, mutate: F) -> bool
    where
        F: FnOnce(&mut crate::model::context_menu::ContextMenuController),
    {
        let mut ctrl = self.controller.borrow_mut();
        if !ctrl.is_open() {
            return false;
        }
        mutate(&mut ctrl);
        self.needs_rebuild = true;
        true
    }
}

impl Drop for ContextMenuScreen {
    fn drop(&mut self) {
        if self.controller.borrow().is_open() {
            self.controller.borrow_mut().dismiss();
        }
    }
}

impl Screen for ContextMenuScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let widget = ContextMenu::new(self.controller.clone())
            .with_popup_rect_sink(Rc::clone(&self.popup_rect));
        let _root = cx.mount_child(Box::new(widget));
        &mut self.placeholder
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        self.needs_rebuild = true;
    }

    fn handle_input(&mut self, event: &FusedInputEvent, cx: &mut ScreenContext<'_>) -> bool {
        let FusedInputEvent::KeyDown { key_code, .. } = event else {
            return false;
        };

        match key_code {
            KeyCode::Escape => false,
            KeyCode::Down => {
                self.update_menu_with(|ctrl| ctrl.surface_mut().navigate_highlight(true))
            }
            KeyCode::Up => {
                self.update_menu_with(|ctrl| ctrl.surface_mut().navigate_highlight(false))
            }
            KeyCode::Enter | KeyCode::Space => {
                let selected =
                    self.update_menu_with(|ctrl| ctrl.surface_mut().select_highlighted());
                if selected && !self.controller.borrow().is_open() {
                    cx.nav_close_modal();
                }
                selected
            }
            _ => false,
        }
    }

    fn dismiss_policy(&self) -> DismissPolicy {
        DismissPolicy::popup_default()
    }

    fn covers_point(&self, logical_pos: Point) -> bool {
        self.popup_rect.get().contains(logical_pos)
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        if !self.controller.borrow().is_open() {
            cx.nav_close_modal();
        }
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
