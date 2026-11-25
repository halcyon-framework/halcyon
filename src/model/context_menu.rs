// SPEC-HAL-217: ContextMenu Controller
// Visibility coordinator. Delegates interaction to MenuSurface (SPEC-HAL-223).
// Anchor + items injected at show(), cleared at dismiss().
// Selection emitted to external sink immediately. Zero memory after emission.
// One invocation, one instance: callers create new handle + sink each time.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::math::{Point, RectF};
use crate::model::menu_item::{MenuItem, MenuItemId};
use crate::model::menu_surface::MenuSurface;

/// Anchor for context menu positioning.
pub enum Anchor {
    /// Popup origin at this exact point.
    Point(Point),
    /// Popup origin at bottom-left of this rect.
    Rect(RectF),
}

impl Anchor {
    /// Compute the popup origin from this anchor.
    pub fn origin(&self) -> (f32, f32) {
        match self {
            Anchor::Point(p) => (p.x, p.y),
            Anchor::Rect(r) => (r.x, r.y + r.height),
        }
    }
}

/// External selection sink. Written by controller, read by caller.
/// Controller retains zero knowledge of emitted values.
pub type SelectionSink = Rc<Cell<Option<MenuItemId>>>;

/// Visibility coordinator for context menu.
/// Anchor and items are temporary (injected at show, cleared at dismiss).
/// Selection is emitted to an external sink — no internal selection state.
pub struct ContextMenuController {
    surface: MenuSurface,
    anchor: Option<Anchor>,
}

impl ContextMenuController {
    fn new(selection_sink: SelectionSink) -> Self {
        Self {
            surface: MenuSurface::new(selection_sink),
            anchor: None,
        }
    }

    /// Open the context menu at the given anchor with the given items.
    /// Previous state is fully replaced.
    pub fn show(&mut self, anchor: Anchor, items: Vec<MenuItem>) {
        self.anchor = Some(anchor);
        self.surface.show(items);
    }

    /// Close and fully reset. Items and anchor cleared, no state survives.
    pub fn dismiss(&mut self) {
        self.anchor = None;
        self.surface.dismiss();
    }

    /// Emit selection to external sink, then dismiss. Zero retention.
    pub fn select(&mut self, id: MenuItemId) {
        self.surface.select_id(id);
        self.anchor = None;
    }

    pub fn is_open(&self) -> bool {
        self.surface.is_open()
    }

    pub fn items(&self) -> &[MenuItem] {
        self.surface.items()
    }

    pub fn anchor(&self) -> Option<&Anchor> {
        self.anchor.as_ref()
    }

    pub fn surface(&self) -> &MenuSurface {
        &self.surface
    }

    pub fn surface_mut(&mut self) -> &mut MenuSurface {
        &mut self.surface
    }
}

/// Shared handle for cross-component access.
pub type ContextMenuHandle = Rc<RefCell<ContextMenuController>>;

/// Create a shared ContextMenuController handle and its external selection sink.
/// One invocation, one instance: create new handle + sink each time.
pub fn context_menu_handle() -> (ContextMenuHandle, SelectionSink) {
    let sink = Rc::new(Cell::new(None));
    let ctrl = Rc::new(RefCell::new(ContextMenuController::new(sink.clone())));
    (ctrl, sink)
}
