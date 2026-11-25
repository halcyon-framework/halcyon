// SPEC-HAL-333 Rev 3: MenuModel — single source of truth for the application menu.
// Owns exactly three tables (INV-A1): structure (MenuItem trees, SPEC-HAL-222),
// bindings (MenuItemId -> ActionId, SPEC-HAL-020), and live state
// (MenuItemId -> MenuItemState). No pixels, geometry, theme, focus, or
// platform state.
//
// INV-A2: the model owns all live item state; hosts mirror, never own.
// INV-A4: command identity belongs to the Action System; no menu-local registry.
// INV-A5: mutation applies to the model immediately and queues the item id
//         dirty; sync is projection refresh, never an edit channel.
// INV-A6 (Rev 3): shortcut display hints are derived at bind time from the
//         bound Action's structured Shortcut, never authored and never
//         stored — the menu system holds no Shortcut anywhere.

use std::collections::HashMap;

use crate::model::menu_item::{MenuItem, MenuItemId};
use crate::runtime::actions::{ActionId, ActionRegistry};

/// Live, host-mirrored state of one item (SPEC-HAL-333 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuItemState {
    pub enabled: bool,
    /// None = not checkable (no check slot). Some(b) = checkable with value b.
    pub checked: Option<bool>,
}

/// One top-level menu title and its item tree.
pub struct MenuTitle {
    pub title: String,
    pub items: Vec<MenuItem>,
}

/// Single source of truth for the application menu (SPEC-HAL-333 Class A).
pub struct MenuModel {
    titles: Vec<MenuTitle>,
    /// Per-item command binding (INV-A4): MenuItemId -> ActionId.
    bindings: HashMap<MenuItemId, ActionId>,
    state: HashMap<MenuItemId, MenuItemState>,
    /// Item ids mutated since the last drain (INV-A5). Deduplicated, FIFO.
    dirty: Vec<MenuItemId>,
}

impl MenuModel {
    pub fn new() -> Self {
        Self {
            titles: Vec::new(),
            bindings: HashMap::new(),
            state: HashMap::new(),
            dirty: Vec::new(),
        }
    }

    // ------------------------------------------------------------------
    // Build API (application setup)
    // ------------------------------------------------------------------

    /// Append a top-level title with its item tree. Seeds live state for
    /// every ActionRow, recursively: enabled = !disabled, checked = None.
    pub fn add_title(&mut self, title: &str, items: Vec<MenuItem>) {
        seed_states(&items, &mut self.state);
        self.titles.push(MenuTitle {
            title: title.to_string(),
            items,
        });
    }

    /// Bind an activatable leaf to its Action (INV-A4). Derives the display
    /// hint from the Action's structured Shortcut and writes it into the
    /// item's shortcut slot (INV-A6 Rev 3) — derived output replaces any
    /// authored hint unconditionally; an unregistered action clears the
    /// slot. No Shortcut is stored (the Rev 2 snapshot served native key
    /// equivalents, retired).
    pub fn bind(&mut self, id: MenuItemId, action: ActionId, registry: &ActionRegistry) {
        self.bindings.insert(id, action);
        let hint = registry
            .actions
            .get(&action)
            .and_then(|a| a.shortcut)
            .map(|s| s.to_display_string());
        if let Some(MenuItem::ActionRow { shortcut: slot, .. }) = self.find_item_mut(id) {
            *slot = hint;
        }
    }

    /// Declare an item checkable; initial value false. Build-time API:
    /// does not mark dirty. No effect on items already checkable.
    pub fn set_checkable(&mut self, id: MenuItemId) {
        if let Some(st) = self.state.get_mut(&id) {
            if st.checked.is_none() {
                st.checked = Some(false);
            }
        }
    }

    // ------------------------------------------------------------------
    // Mutation API (application runtime — INV-A5)
    // ------------------------------------------------------------------

    /// Set enabled state. Marks dirty only when the value changes.
    pub fn set_enabled(&mut self, id: MenuItemId, enabled: bool) {
        let Some(st) = self.state.get_mut(&id) else {
            return;
        };
        if st.enabled != enabled {
            st.enabled = enabled;
            self.mark_dirty(id);
        }
    }

    /// Set checked value. Applies only to items declared checkable via
    /// set_checkable — no silent upgrade of non-checkable items. Marks
    /// dirty only when the value changes.
    pub fn set_checked(&mut self, id: MenuItemId, checked: bool) {
        let Some(st) = self.state.get_mut(&id) else {
            return;
        };
        let Some(current) = st.checked else {
            return;
        };
        if current != checked {
            st.checked = Some(checked);
            self.mark_dirty(id);
        }
    }

    /// Set the item's label in the structure tree. Marks dirty only when
    /// the value changes.
    pub fn set_label(&mut self, id: MenuItemId, label: String) {
        let mut changed = false;
        if let Some(MenuItem::ActionRow { label: slot, .. }) = self.find_item_mut(id) {
            if slot.as_str() != label.as_str() {
                *slot = label;
                changed = true;
            }
        }
        if changed {
            self.mark_dirty(id);
        }
    }

    // ------------------------------------------------------------------
    // Drain & queries (AppRoot / hosts)
    // ------------------------------------------------------------------

    /// Take all item ids mutated since the last drain (INV-A5).
    pub fn drain_dirty(&mut self) -> Vec<MenuItemId> {
        std::mem::take(&mut self.dirty)
    }

    pub fn titles(&self) -> &[MenuTitle] {
        &self.titles
    }

    pub fn item_state(&self, id: MenuItemId) -> Option<MenuItemState> {
        self.state.get(&id).copied()
    }

    pub fn item_label(&self, id: MenuItemId) -> Option<&str> {
        for title in &self.titles {
            if let Some(MenuItem::ActionRow { label, .. }) = find_row(&title.items, id) {
                return Some(label.as_str());
            }
        }
        None
    }

    pub fn bound_action(&self, id: MenuItemId) -> Option<ActionId> {
        self.bindings.get(&id).copied()
    }

    // ------------------------------------------------------------------
    // Internal
    // ------------------------------------------------------------------

    fn mark_dirty(&mut self, id: MenuItemId) {
        if !self.dirty.contains(&id) {
            self.dirty.push(id);
        }
    }

    fn find_item_mut(&mut self, id: MenuItemId) -> Option<&mut MenuItem> {
        for title in &mut self.titles {
            if let Some(item) = find_row_mut(&mut title.items, id) {
                return Some(item);
            }
        }
        None
    }
}

impl Default for MenuModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Seed live state for every ActionRow in the tree, recursively.
fn seed_states(items: &[MenuItem], state: &mut HashMap<MenuItemId, MenuItemState>) {
    for item in items {
        if let MenuItem::ActionRow {
            id,
            disabled,
            submenu,
            ..
        } = item
        {
            state.insert(
                *id,
                MenuItemState {
                    enabled: !*disabled,
                    checked: None,
                },
            );
            if let Some(children) = submenu {
                seed_states(children, state);
            }
        }
    }
}

/// Find the ActionRow with the given id, recursing into submenus.
pub(crate) fn find_row_mut(items: &mut [MenuItem], id: MenuItemId) -> Option<&mut MenuItem> {
    for item in items.iter_mut() {
        let hit = match &*item {
            MenuItem::ActionRow { id: row_id, .. } => *row_id == id,
            MenuItem::Separator => false,
        };
        if hit {
            return Some(item);
        }
        if let MenuItem::ActionRow {
            submenu: Some(children),
            ..
        } = item
        {
            if let Some(found) = find_row_mut(children.as_mut_slice(), id) {
                return Some(found);
            }
        }
    }
    None
}

/// Read-only twin of find_row_mut.
pub(crate) fn find_row(items: &[MenuItem], id: MenuItemId) -> Option<&MenuItem> {
    for item in items {
        let hit = match item {
            MenuItem::ActionRow { id: row_id, .. } => *row_id == id,
            MenuItem::Separator => false,
        };
        if hit {
            return Some(item);
        }
        if let MenuItem::ActionRow {
            submenu: Some(children),
            ..
        } = item
        {
            if let Some(found) = find_row(children, id) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::KeyCode;
    use crate::runtime::actions::{Action, ActionScope, Shortcut};

    fn noop_handler(_cx: &mut crate::core::app::AppContext) {}

    fn row(id: MenuItemId, label: &str, disabled: bool) -> MenuItem {
        MenuItem::ActionRow {
            id,
            label: label.to_string(),
            icon: None,
            shortcut: None,
            disabled,
            submenu: None,
        }
    }

    fn row_with_submenu(id: MenuItemId, label: &str, children: Vec<MenuItem>) -> MenuItem {
        MenuItem::ActionRow {
            id,
            label: label.to_string(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: Some(children),
        }
    }

    fn registry_with(action_id: u64, shortcut: Option<Shortcut>) -> ActionRegistry {
        let mut registry = ActionRegistry::new();
        registry.add(Action {
            id: ActionId(action_id),
            name: "Test Action",
            description: "",
            category: "Test",
            shortcut,
            scope: ActionScope::Global,
            handler: noop_handler,
        });
        registry
    }

    #[test]
    fn seeding_sets_enabled_from_disabled_and_no_check() {
        let mut model = MenuModel::new();
        model.add_title("File", vec![row(1, "Open", false), row(2, "Revert", true)]);

        assert_eq!(
            model.item_state(1),
            Some(MenuItemState {
                enabled: true,
                checked: None
            })
        );
        assert_eq!(
            model.item_state(2),
            Some(MenuItemState {
                enabled: false,
                checked: None
            })
        );
    }

    #[test]
    fn set_checkable_initializes_unchecked_without_dirty() {
        let mut model = MenuModel::new();
        model.add_title("View", vec![row(1, "Show Toolbar", false)]);

        model.set_checkable(1);

        assert_eq!(
            model.item_state(1),
            Some(MenuItemState {
                enabled: true,
                checked: Some(false)
            })
        );
        assert!(model.drain_dirty().is_empty());
    }

    #[test]
    fn set_checked_requires_checkable() {
        let mut model = MenuModel::new();
        model.add_title("View", vec![row(1, "Show Toolbar", false)]);

        model.set_checked(1, true);

        assert_eq!(
            model.item_state(1),
            Some(MenuItemState {
                enabled: true,
                checked: None
            })
        );
        assert!(model.drain_dirty().is_empty());
    }

    #[test]
    fn set_checked_changes_and_marks_dirty_once() {
        let mut model = MenuModel::new();
        model.add_title("View", vec![row(1, "Show Toolbar", false)]);
        model.set_checkable(1);

        model.set_checked(1, true);
        assert_eq!(
            model.item_state(1),
            Some(MenuItemState {
                enabled: true,
                checked: Some(true)
            })
        );
        assert_eq!(model.drain_dirty(), vec![1]);

        // Same value again: no new dirty entry.
        model.set_checked(1, true);
        assert!(model.drain_dirty().is_empty());
    }

    #[test]
    fn set_enabled_marks_dirty_on_change_only_and_dedups() {
        let mut model = MenuModel::new();
        model.add_title("File", vec![row(1, "Save", false)]);

        // No-op write: value already true.
        model.set_enabled(1, true);
        assert!(model.drain_dirty().is_empty());

        // Two real mutations before drain: one dirty entry.
        model.set_enabled(1, false);
        model.set_enabled(1, true);
        assert_eq!(model.drain_dirty(), vec![1]);
        assert!(model.drain_dirty().is_empty());
    }

    #[test]
    fn set_label_updates_tree_and_marks_dirty_on_change_only() {
        let mut model = MenuModel::new();
        model.add_title("Edit", vec![row(1, "Undo", false)]);

        model.set_label(1, "Undo Typing".to_string());
        assert_eq!(model.item_label(1), Some("Undo Typing"));
        assert_eq!(model.drain_dirty(), vec![1]);

        // Same value again: no dirty.
        model.set_label(1, "Undo Typing".to_string());
        assert!(model.drain_dirty().is_empty());
    }

    #[test]
    fn bind_derives_display_hint() {
        let mut model = MenuModel::new();
        model.add_title("File", vec![row(1, "Save", false)]);
        let shortcut = Shortcut::new(KeyCode::Character('d'), true, false, false, false);
        let registry = registry_with(2000, Some(shortcut));

        model.bind(1, ActionId(2000), &registry);

        assert_eq!(model.bound_action(1), Some(ActionId(2000)));
        match find_row(&model.titles()[0].items, 1) {
            Some(MenuItem::ActionRow { shortcut: slot, .. }) => {
                assert_eq!(slot.as_deref(), Some("Ctrl+D"));
            }
            other => panic!("expected ActionRow, got {:?}", other.is_some()),
        }
    }

    #[test]
    fn bind_unregistered_action_stores_binding_and_clears_authored_hint() {
        let mut model = MenuModel::new();
        let authored = MenuItem::ActionRow {
            id: 1,
            label: "Save".to_string(),
            icon: None,
            shortcut: Some("Authored".to_string()),
            disabled: false,
            submenu: None,
        };
        model.add_title("File", vec![authored]);
        let registry = ActionRegistry::new();

        model.bind(1, ActionId(9999), &registry);

        assert_eq!(model.bound_action(1), Some(ActionId(9999)));
        match find_row(&model.titles()[0].items, 1) {
            Some(MenuItem::ActionRow { shortcut: slot, .. }) => {
                assert_eq!(slot.as_deref(), None);
            }
            other => panic!("expected ActionRow, got {:?}", other.is_some()),
        }
    }

    #[test]
    fn submenu_items_are_seeded_and_reachable() {
        let mut model = MenuModel::new();
        model.add_title(
            "File",
            vec![row_with_submenu(
                1,
                "Export",
                vec![row(10, "As PDF", false), row(11, "As PNG", true)],
            )],
        );

        assert_eq!(
            model.item_state(11),
            Some(MenuItemState {
                enabled: false,
                checked: None
            })
        );

        model.set_label(10, "As PDF…".to_string());
        assert_eq!(model.item_label(10), Some("As PDF…"));
        assert_eq!(model.drain_dirty(), vec![10]);

        let shortcut = Shortcut::new(KeyCode::Character('e'), true, false, true, false);
        let registry = registry_with(2001, Some(shortcut));
        model.bind(10, ActionId(2001), &registry);
        assert_eq!(model.bound_action(10), Some(ActionId(2001)));
    }
}
