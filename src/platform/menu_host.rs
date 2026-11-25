use crate::core::app::UiWakeHandle;
use crate::model::menu_item::MenuItemId;
use crate::model::menu_model::MenuModel;
use crate::runtime::actions::ActionRegistry;

pub trait MenuHost {
    /// HAL333 §7: initialize native application roles even without a product model.
    /// External and in-window hosts keep their existing behavior.
    fn initialize_application_roles(&mut self, _wake: Option<UiWakeHandle>) {}
    /// Consume one coalesced standard lifecycle role, never an ordinary item ID.
    fn take_quit_request(&mut self) -> bool {
        false
    }
    fn install(&mut self, model: &MenuModel, registry: &ActionRegistry);
    fn sync_item(&mut self, model: &MenuModel, id: MenuItemId);
    fn drain_activations(&mut self) -> Vec<MenuItemId>;
    fn occupies_in_window_bar(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuHostKind {
    MacOsNative,
    InWindow,
}

pub fn resolve_menu_host_kind() -> MenuHostKind {
    #[cfg(target_os = "macos")]
    {
        MenuHostKind::MacOsNative
    }

    #[cfg(not(target_os = "macos"))]
    {
        MenuHostKind::InWindow
    }
}
