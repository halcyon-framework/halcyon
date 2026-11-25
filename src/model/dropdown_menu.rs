// SPEC-HAL-218: DropdownMenu Controller
// Action selector, not value control. Delegates interaction to MenuSurface (SPEC-HAL-223).
// Emits exactly one action per selection.
// Selection emitted to external sink immediately. Zero memory after emission.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::math::RectF;
use crate::model::menu_item::{MenuItem, MenuItemId};
use crate::model::menu_surface::MenuSurface;

/// External selection sink. Written by controller, read by caller.
/// Controller retains zero knowledge of emitted values.
pub type SelectionSink = Rc<Cell<Option<MenuItemId>>>;

/// Action selector controller for DropdownMenu.
/// Items are temporary (injected at show, cleared at dismiss).
/// Selection is emitted to an external sink — no internal selection state.
pub struct DropdownMenuController {
    surface: MenuSurface,
    anchor_rect: Option<RectF>,
}

impl DropdownMenuController {
    fn new(selection_sink: SelectionSink) -> Self {
        Self {
            surface: MenuSurface::new(selection_sink),
            anchor_rect: None,
        }
    }

    /// Open the dropdown menu anchored to the given rect with the given items.
    /// Previous state is fully replaced.
    pub fn show(&mut self, anchor: RectF, items: Vec<MenuItem>) {
        self.anchor_rect = Some(anchor);
        self.surface.show(items);
    }

    /// Close and fully reset. Items and anchor cleared, no state survives.
    pub fn dismiss(&mut self) {
        self.anchor_rect = None;
        self.surface.dismiss();
    }

    /// Emit selection to external sink, then dismiss. Zero retention.
    pub fn select(&mut self, id: MenuItemId) {
        self.surface.select_id(id);
        self.anchor_rect = None;
    }

    pub fn is_open(&self) -> bool {
        self.surface.is_open()
    }

    pub fn items(&self) -> &[MenuItem] {
        self.surface.items()
    }

    pub fn anchor_rect(&self) -> Option<RectF> {
        self.anchor_rect
    }

    pub fn surface(&self) -> &MenuSurface {
        &self.surface
    }

    pub fn surface_mut(&mut self) -> &mut MenuSurface {
        &mut self.surface
    }
}

/// Shared handle for cross-component access.
pub type DropdownMenuHandle = Rc<RefCell<DropdownMenuController>>;

/// Create a shared DropdownMenuController handle and its external selection sink.
pub fn dropdown_menu_handle() -> (DropdownMenuHandle, SelectionSink) {
    let sink = Rc::new(Cell::new(None));
    let ctrl = Rc::new(RefCell::new(DropdownMenuController::new(sink.clone())));
    (ctrl, sink)
}
