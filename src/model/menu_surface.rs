// SPEC-HAL-223: MenuSurface Contract
// Centralized menu interaction authority.
// Owns items, highlighted index, open/closed state.
// Emits MenuItemId on activation to external sink.
// No knowledge of which widget hosts it. No persistent state after dismissal.
//
// INV-F1: MenuSurface is the sole interaction authority for all menu widgets.
// INV-C1: No geometry, hit-testing, or popup metrics. Index-and-intent only.
// INV-C2: Selection sink cleared on every dismissal — no stale intent survives.
// INV-C3: Highlighted index cleared on every dismissal path.
// INV-H1: All pointer/keyboard → index resolution happens in MenuSurfaceOverlay.

use std::cell::Cell;
use std::rc::Rc;

use crate::model::menu_item::{MenuItem, MenuItemId};

/// Centralized menu interaction state per SPEC-HAL-223.
/// All menu interaction logic lives here. Widget layer delegates to this.
/// Operates strictly on indices and intent — no geometry parameters allowed.
pub struct MenuSurface {
    open: bool,
    items: Vec<MenuItem>,
    highlighted_index: Option<usize>,
    selection_sink: Rc<Cell<Option<MenuItemId>>>,
}

impl MenuSurface {
    pub fn new(selection_sink: Rc<Cell<Option<MenuItemId>>>) -> Self {
        Self {
            open: false,
            items: Vec::new(),
            highlighted_index: None,
            selection_sink,
        }
    }

    /// Open with the given items. Previous state fully replaced.
    /// Selection sink cleared — no stale intent from prior lifetime.
    pub fn show(&mut self, items: Vec<MenuItem>) {
        self.selection_sink.set(None);
        self.items = items;
        self.highlighted_index = None;
        self.open = true;
    }

    /// Close and fully reset. Items cleared, no state survives.
    /// Selection sink cleared — no stale intent survives dismissal.
    pub fn dismiss(&mut self) {
        self.open = false;
        self.items.clear();
        self.highlighted_index = None;
        self.selection_sink.set(None);
    }

    /// Emit specific id to external sink, then dismiss. Zero retention.
    /// Dismiss runs first (clears sink), then sink is written — emitted value survives
    /// for consumer to read. Next show() clears stale intent (INV-C2).
    pub fn select_id(&mut self, id: MenuItemId) {
        self.dismiss();
        self.selection_sink.set(Some(id));
    }

    /// Select the currently highlighted item (if any). Used by keyboard Enter/Space.
    pub fn select_highlighted(&mut self) {
        if let Some(idx) = self.highlighted_index {
            if let Some(MenuItem::ActionRow {
                id,
                disabled: false,
                ..
            }) = self.items.get(idx)
            {
                let id = *id;
                self.select_id(id);
            }
        }
    }

    /// Activate item at the given index. Used by overlay after hit-testing.
    pub fn activate_index(&mut self, index: usize) {
        if let Some(MenuItem::ActionRow {
            id,
            disabled: false,
            ..
        }) = self.items.get(index)
        {
            let id = *id;
            self.select_id(id);
        }
    }

    /// Set highlighted index directly. Used by overlay after pointer hit-testing.
    pub fn set_highlighted(&mut self, index: Option<usize>) {
        self.highlighted_index = index;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }

    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted_index
    }

    /// Navigate highlight forward (Down) or backward (Up), wrapping, skipping separators.
    pub fn navigate_highlight(&mut self, forward: bool) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        let start = match self.highlighted_index {
            Some(i) => {
                if forward {
                    (i + 1) % len
                } else {
                    (i + len - 1) % len
                }
            }
            None => {
                if forward {
                    0
                } else {
                    len - 1
                }
            }
        };
        for step in 0..len {
            let idx = if forward {
                (start + step) % len
            } else {
                (start + len - step) % len
            };
            if matches!(
                self.items[idx],
                MenuItem::ActionRow {
                    disabled: false,
                    ..
                }
            ) {
                self.highlighted_index = Some(idx);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn action(id: MenuItemId, disabled: bool) -> MenuItem {
        MenuItem::ActionRow {
            id,
            label: format!("Item {id}"),
            icon: None,
            shortcut: None,
            disabled,
            submenu: None,
        }
    }

    #[test]
    fn disabled_index_activation_emits_no_intent() {
        let sink = Rc::new(Cell::new(None));
        let mut surface = MenuSurface::new(Rc::clone(&sink));
        surface.show(vec![action(1, true)]);

        surface.activate_index(0);

        assert!(surface.is_open());
        assert_eq!(sink.get(), None);
    }

    #[test]
    fn disabled_highlight_activation_emits_no_intent() {
        let sink = Rc::new(Cell::new(None));
        let mut surface = MenuSurface::new(Rc::clone(&sink));
        surface.show(vec![action(1, true)]);
        surface.set_highlighted(Some(0));

        surface.select_highlighted();

        assert!(surface.is_open());
        assert_eq!(sink.get(), None);
    }

    #[test]
    fn keyboard_navigation_skips_disabled_rows_and_separators() {
        let sink = Rc::new(Cell::new(None));
        let mut surface = MenuSurface::new(sink);
        surface.show(vec![
            action(1, true),
            MenuItem::Separator,
            action(2, false),
            action(3, true),
        ]);

        surface.navigate_highlight(true);
        assert_eq!(surface.highlighted_index(), Some(2));

        surface.navigate_highlight(true);
        assert_eq!(surface.highlighted_index(), Some(2));
    }
}
