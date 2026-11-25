// SPEC-HAL-222: Shared MenuItem Primitive
// Canonical data definition for menu items across all menu surfaces.
// No state, no behavior, no surface knowledge, no menu ownership.

use crate::core::icon::IconId;

/// Opaque identifier for a menu item.
pub type MenuItemId = u32;

/// A single menu entry: either an actionable row or a visual separator.
/// Shared across PopupMenu, ContextMenu, DropdownMenu.
///
/// Invariants (SPEC-HAL-222):
/// - No behavior: MenuItem defines data only, never executes actions.
/// - No surface knowledge: MenuItem does not know which menu renders it.
/// - No state ownership: no hover, selection, open, or focus state.
#[derive(Clone)]
pub enum MenuItem {
    ActionRow {
        id: MenuItemId,
        label: String,
        /// Leading icon. None = no icon.
        icon: Option<IconId>,
        /// Trailing keyboard shortcut hint (e.g., "⌘D"). None = no hint.
        shortcut: Option<String>,
        /// Greyed out, not clickable.
        disabled: bool,
        /// Nested submenu items. None = no submenu.
        submenu: Option<Vec<MenuItem>>,
    },
    Separator,
}
