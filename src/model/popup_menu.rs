// SPEC-HAL-216: PopupMenu Controller
// Visibility coordinator. Delegates interaction to MenuSurface (SPEC-HAL-223).
// Selection emitted to external sink immediately. Zero memory after emission.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::model::menu_item::{MenuItem, MenuItemId};
use crate::model::menu_surface::MenuSurface;

/// External selection sink. Written by controller, read by caller.
/// Controller retains zero knowledge of emitted values.
pub type SelectionSink = Rc<Cell<Option<MenuItemId>>>;

/// Visibility coordinator for popup menu.
/// Items are temporary (injected at show, cleared at dismiss).
/// Selection is emitted to an external sink — no internal selection state.
pub struct PopupMenuController {
    surface: MenuSurface,
}

impl PopupMenuController {
    fn new(selection_sink: SelectionSink) -> Self {
        Self {
            surface: MenuSurface::new(selection_sink),
        }
    }

    /// Open the popup with the given items. Previous state is fully replaced.
    pub fn show(&mut self, items: Vec<MenuItem>) {
        self.surface.show(items);
    }

    /// Close and fully reset. Items cleared, no state survives.
    pub fn dismiss(&mut self) {
        self.surface.dismiss();
    }

    /// Emit selection to external sink, then dismiss. Zero retention.
    pub fn select(&mut self, id: MenuItemId) {
        self.surface.select_id(id);
    }

    pub fn is_open(&self) -> bool {
        self.surface.is_open()
    }

    pub fn items(&self) -> &[MenuItem] {
        self.surface.items()
    }

    pub fn surface(&self) -> &MenuSurface {
        &self.surface
    }

    pub fn surface_mut(&mut self) -> &mut MenuSurface {
        &mut self.surface
    }
}

/// Shared handle for cross-component access.
pub type PopupMenuHandle = Rc<RefCell<PopupMenuController>>;

/// Create a shared PopupMenuController handle and its external selection sink.
pub fn popup_menu_handle() -> (PopupMenuHandle, SelectionSink) {
    let sink = Rc::new(Cell::new(None));
    let ctrl = Rc::new(RefCell::new(PopupMenuController::new(sink.clone())));
    (ctrl, sink)
}
