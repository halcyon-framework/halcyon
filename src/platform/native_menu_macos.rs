#![cfg(target_os = "macos")]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability, sel};
use objc2_app_kit::{
    NSAlternateKeyMask, NSApplication, NSBackspaceCharacter, NSCarriageReturnCharacter,
    NSCommandKeyMask, NSControlKeyMask, NSControlStateValueOff, NSControlStateValueOn,
    NSDeleteCharacter, NSDownArrowFunctionKey, NSEndFunctionKey, NSEventModifierFlags,
    NSF1FunctionKey, NSF2FunctionKey, NSF3FunctionKey, NSF4FunctionKey, NSF5FunctionKey,
    NSF6FunctionKey, NSF7FunctionKey, NSF8FunctionKey, NSF9FunctionKey, NSF10FunctionKey,
    NSF11FunctionKey, NSF12FunctionKey, NSHomeFunctionKey, NSLeftArrowFunctionKey, NSMenu,
    NSMenuItem, NSPageDownFunctionKey, NSPageUpFunctionKey, NSRightArrowFunctionKey,
    NSShiftKeyMask, NSTabCharacter, NSUpArrowFunctionKey,
};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSProcessInfo, NSString};

use crate::core::app::UiWakeHandle;
use crate::core::event::{BindingKey, KeyCode};
use crate::model::menu_item::{MenuItem, MenuItemId};
use crate::model::menu_model::MenuModel;
use crate::platform::menu_host::MenuHost;
use crate::runtime::actions::{ActionRegistry, Shortcut};

const ESCAPE_KEY_EQUIVALENT: u32 = 0x001b;

#[derive(Clone)]
struct MenuTargetIvars {
    activations: Rc<RefCell<Vec<MenuItemId>>>,
    roles: Rc<ApplicationRoles>,
}

/// INV-333-QUIT-OWNER/ONCE: this host's bounded main-thread request custody.
/// It contains no AppRoot, widget, model or cleanup reference.
struct ApplicationRoles {
    live: Cell<bool>,
    quit: Cell<bool>,
    wake: RefCell<Option<UiWakeHandle>>,
}
impl ApplicationRoles {
    fn new() -> Self {
        Self {
            live: Cell::new(true),
            quit: Cell::new(false),
            wake: RefCell::new(None),
        }
    }
    fn request_quit(&self) {
        if !self.live.get() || self.quit.replace(true) {
            return;
        }
        // Publish before the existing unit wake. Duplicate callbacks do not
        // allocate, wake again or repeat this transition's error diagnostic.
        match self.wake.try_borrow() {
            Ok(wake) => match wake.as_ref() {
                Some(wake) => {
                    if let Err(error) = wake.wake() {
                        quit_wake_diagnostic(format_args!("{error}"));
                    }
                }
                None => quit_wake_diagnostic(format_args!("no UI wake attached")),
            },
            Err(_) => quit_wake_diagnostic(format_args!("UI wake is borrowed")),
        }
    }
    fn take_quit(&self) -> bool {
        self.live.get() && self.quit.replace(false)
    }
    fn retire(&self) {
        self.live.set(false);
        self.quit.set(false);
        self.wake.borrow_mut().take();
    }
}

fn quit_wake_diagnostic(reason: std::fmt::Arguments<'_>) {
    // A broken diagnostic sink cannot unwind across the Objective-C callback
    // or authorize a terminate fallback. The original request remains pending.
    match writeln!(
        std::io::stderr().lock(),
        "INV-333-QUIT-RETURN: native Quit remains pending: {reason}"
    ) {
        Ok(()) => {}
        Err(_) => {}
    }
}

declare_class!(
    struct HalcyonMenuTarget;

    unsafe impl ClassType for HalcyonMenuTarget {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "HalcyonMenuTarget";
    }

    impl DeclaredClass for HalcyonMenuTarget {
        type Ivars = MenuTargetIvars;
    }

    unsafe impl NSObjectProtocol for HalcyonMenuTarget {}

    unsafe impl HalcyonMenuTarget {
        #[method(halcyonMenuItemActivated:)]
        fn halcyon_menu_item_activated(&self, sender: &NSMenuItem) {
            if !self.ivars().roles.live.get() { return; }
            if !unsafe { sender.isEnabled() } {
                return;
            }
            let tag = unsafe { sender.tag() };
            if tag >= 0 {
                self.ivars().activations.borrow_mut().push(tag as MenuItemId);
            }
        }

        #[method(halcyonApplicationQuit:)]
        fn halcyon_application_quit(&self, _sender: &NSMenuItem) {
            // No UI mutation, cleanup, dispatch or panic crosses this callback.
            self.ivars().roles.request_quit();
        }
    }
);

impl HalcyonMenuTarget {
    fn new(
        activations: Rc<RefCell<Vec<MenuItemId>>>,
        roles: Rc<ApplicationRoles>,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(MenuTargetIvars { activations, roles });
        unsafe { msg_send_id![super(this), init] }
    }
}

pub struct MacOsMenuHost {
    activations: Rc<RefCell<Vec<MenuItemId>>>,
    roles: Rc<ApplicationRoles>,
    target: Retained<HalcyonMenuTarget>,
    items: HashMap<MenuItemId, Retained<NSMenuItem>>,
    main_menu: Option<Retained<NSMenu>>,
    application_menu: Option<Retained<NSMenuItem>>,
    mtm: MainThreadMarker,
}

impl MacOsMenuHost {
    pub fn new() -> Self {
        let mtm =
            MainThreadMarker::new().expect("MacOsMenuHost must be created on the main thread");
        let activations = Rc::new(RefCell::new(Vec::new()));
        let roles = Rc::new(ApplicationRoles::new());
        let target = HalcyonMenuTarget::new(Rc::clone(&activations), roles.clone(), mtm);

        Self {
            activations,
            roles,
            target,
            items: HashMap::new(),
            main_menu: None,
            application_menu: None,
            mtm,
        }
    }

    fn project_title(
        &mut self,
        main_menu: &NSMenu,
        title: &str,
        items: &[MenuItem],
        model: &MenuModel,
        registry: &ActionRegistry,
    ) {
        let title_item = make_item(title, None, "");
        let submenu = make_menu(title, self.mtm);
        self.project_items(&submenu, items, model, registry);
        title_item.setSubmenu(Some(&submenu));
        main_menu.addItem(&title_item);
    }

    fn project_items(
        &mut self,
        menu: &NSMenu,
        items: &[MenuItem],
        model: &MenuModel,
        registry: &ActionRegistry,
    ) {
        for item in items {
            match item {
                MenuItem::Separator => {
                    let separator = NSMenuItem::separatorItem(self.mtm);
                    menu.addItem(&separator);
                }
                MenuItem::ActionRow {
                    id,
                    label,
                    disabled,
                    submenu,
                    ..
                } => {
                    let action_id = model.bound_action(*id);
                    let selector = action_id.map(|_| sel!(halcyonMenuItemActivated:));
                    let shortcut = action_id
                        .and_then(|bound| registry.actions.get(&bound))
                        .and_then(|action| action.shortcut)
                        .and_then(shortcut_projection);
                    let key = shortcut.as_ref().map(|(key, _)| key.as_str()).unwrap_or("");
                    let menu_item = make_item(label, selector, key);

                    unsafe {
                        menu_item.setTag(*id as isize);
                        menu_item.setEnabled(
                            model
                                .item_state(*id)
                                .map(|state| state.enabled)
                                .unwrap_or(!*disabled),
                        );
                        if selector.is_some() {
                            let target: &AnyObject = self.target.as_super().as_super();
                            menu_item.setTarget(Some(target));
                        }
                        if let Some(state) = model.item_state(*id).and_then(|state| state.checked) {
                            menu_item.setState(if state {
                                NSControlStateValueOn
                            } else {
                                NSControlStateValueOff
                            });
                        }
                    }

                    if let Some((_, modifiers)) = shortcut {
                        menu_item.setKeyEquivalentModifierMask(modifiers);
                    }

                    if let Some(children) = submenu {
                        let child_menu = make_menu(label, self.mtm);
                        self.project_items(&child_menu, children, model, registry);
                        menu_item.setSubmenu(Some(&child_menu));
                    }

                    menu.addItem(&menu_item);
                    self.items.insert(*id, menu_item);
                }
            }
        }
    }
}

impl MenuHost for MacOsMenuHost {
    fn initialize_application_roles(&mut self, wake: Option<UiWakeHandle>) {
        *self.roles.wake.borrow_mut() = wake;
        let main_menu = make_menu("", self.mtm);
        let app = NSApplication::sharedApplication(self.mtm);
        install_application_menu(
            &main_menu,
            &app,
            &self.target,
            &mut self.application_menu,
            self.mtm,
        );
        app.setMainMenu(Some(&main_menu));
        self.main_menu = Some(main_menu);
    }

    fn take_quit_request(&mut self) -> bool {
        self.roles.take_quit()
    }

    fn install(&mut self, model: &MenuModel, registry: &ActionRegistry) {
        self.items.clear();

        let main_menu = make_menu("", self.mtm);
        let app = NSApplication::sharedApplication(self.mtm);
        install_application_menu(
            &main_menu,
            &app,
            &self.target,
            &mut self.application_menu,
            self.mtm,
        );
        for title in model.titles() {
            self.project_title(&main_menu, &title.title, &title.items, model, registry);
        }

        app.setMainMenu(Some(&main_menu));
        self.main_menu = Some(main_menu);
    }

    fn sync_item(&mut self, model: &MenuModel, id: MenuItemId) {
        let Some(item) = self.items.get(&id) else {
            return;
        };
        let Some(state) = model.item_state(id) else {
            return;
        };
        let Some(label) = model.item_label(id) else {
            return;
        };

        unsafe {
            item.setTitle(&NSString::from_str(label));
            item.setEnabled(state.enabled);
            if let Some(checked) = state.checked {
                item.setState(if checked {
                    NSControlStateValueOn
                } else {
                    NSControlStateValueOff
                });
            }
        }
    }

    fn drain_activations(&mut self) -> Vec<MenuItemId> {
        std::mem::take(&mut self.activations.borrow_mut())
    }

    fn occupies_in_window_bar(&self) -> bool {
        false
    }
}

impl Drop for MacOsMenuHost {
    fn drop(&mut self) {
        // NSMenuItem targets are zeroing weak AppKit references. A target that
        // a native client separately retains is also inert after host retirement.
        self.roles.retire();
    }
}

fn make_menu(title: &str, mtm: MainThreadMarker) -> Retained<NSMenu> {
    let menu = unsafe { NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str(title)) };
    unsafe {
        menu.setAutoenablesItems(false);
    }
    menu
}

fn install_application_menu(
    main_menu: &NSMenu,
    app: &NSApplication,
    target: &HalcyonMenuTarget,
    retained: &mut Option<Retained<NSMenuItem>>,
    mtm: MainThreadMarker,
) {
    if let Some(item) = retained.as_ref() {
        // Preserve the registered standard-role subtree. AppKit menus/items
        // have one parent: detach our item before moving it to the new root.
        if let Some(previous) = unsafe { item.menu() } {
            unsafe { previous.removeItem(item); }
        }
        main_menu.addItem(item);
        return;
    }
    let app_menu_item = make_item("", None, "");
    let app_menu = make_menu("", mtm);
    let name = NSProcessInfo::processInfo().processName().to_string();
    let app_target: &AnyObject = app.as_super().as_super().as_super();
    let system_role = |title: &str, selector, key: &str, mask| {
        let item = make_item(title, Some(selector), key);
        item.setKeyEquivalentModifierMask(mask);
        unsafe {
            item.setEnabled(true);
            item.setTarget(Some(app_target));
        }
        item
    };
    app_menu.addItem(&system_role(
        &format!("About {name}"),
        sel!(orderFrontStandardAboutPanel:),
        "",
        NSEventModifierFlags::empty(),
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    let services_menu = make_menu("Services", mtm);
    let services = make_item("Services", None, "");
    services.setSubmenu(Some(&services_menu));
    app_menu.addItem(&services);
    unsafe {
        app.setServicesMenu(Some(&services_menu));
    }
    app_menu.addItem(&system_role(
        &format!("Hide {name}"),
        sel!(hide:),
        "h",
        NSCommandKeyMask,
    ));
    app_menu.addItem(&system_role(
        "Hide Others",
        sel!(hideOtherApplications:),
        "h",
        NSCommandKeyMask | NSAlternateKeyMask,
    ));
    app_menu.addItem(&system_role(
        "Show All",
        sel!(unhideAllApplications:),
        "",
        NSEventModifierFlags::empty(),
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    let quit_item = make_item(
        &format!("Quit {name}"),
        Some(sel!(halcyonApplicationQuit:)),
        "q",
    );
    quit_item.setKeyEquivalentModifierMask(NSCommandKeyMask);
    unsafe {
        quit_item.setEnabled(true);
        quit_item.setTarget(Some(target.as_super().as_super()));
    }

    app_menu.addItem(&quit_item);
    app_menu_item.setSubmenu(Some(&app_menu));
    main_menu.addItem(&app_menu_item);
    *retained = Some(app_menu_item);
}

fn make_item(
    title: &str,
    selector: Option<objc2::runtime::Sel>,
    key: &str,
) -> Retained<NSMenuItem> {
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            MainThreadMarker::new()
                .expect("NSMenuItem projection must be built on the main thread")
                .alloc(),
            &NSString::from_str(title),
            selector,
            &NSString::from_str(key),
        )
    }
}

fn shortcut_projection(shortcut: Shortcut) -> Option<(String, NSEventModifierFlags)> {
    if is_macos_reserved_shortcut(shortcut) {
        return None;
    }

    let BindingKey::Logical(logical_key) = shortcut.key else {
        return None;
    };
    let key = key_equivalent(logical_key)?;
    let mut modifiers = NSEventModifierFlags::empty();

    if shortcut.meta {
        modifiers |= NSCommandKeyMask;
    }
    if shortcut.ctrl {
        modifiers |= NSControlKeyMask;
    }
    if shortcut.alt {
        modifiers |= NSAlternateKeyMask;
    }
    if shortcut.shift {
        modifiers |= NSShiftKeyMask;
    }

    Some((key, modifiers))
}

fn is_macos_reserved_shortcut(shortcut: Shortcut) -> bool {
    matches!(
        shortcut,
        Shortcut {
            key: BindingKey::Logical(KeyCode::Character('q' | 'Q')),
            ctrl: false,
            alt: false,
            shift: false,
            meta: true,
        }
    )
}

fn key_equivalent(key: KeyCode) -> Option<String> {
    match key {
        KeyCode::Character(c) => Some(c.to_lowercase().collect()),
        KeyCode::Enter => private_key_equivalent(NSCarriageReturnCharacter),
        KeyCode::Tab => private_key_equivalent(NSTabCharacter),
        KeyCode::Backspace => private_key_equivalent(NSBackspaceCharacter),
        KeyCode::Delete => private_key_equivalent(NSDeleteCharacter),
        KeyCode::Left => private_key_equivalent(NSLeftArrowFunctionKey),
        KeyCode::Right => private_key_equivalent(NSRightArrowFunctionKey),
        KeyCode::Up => private_key_equivalent(NSUpArrowFunctionKey),
        KeyCode::Down => private_key_equivalent(NSDownArrowFunctionKey),
        KeyCode::Home => private_key_equivalent(NSHomeFunctionKey),
        KeyCode::End => private_key_equivalent(NSEndFunctionKey),
        KeyCode::PageUp => private_key_equivalent(NSPageUpFunctionKey),
        KeyCode::PageDown => private_key_equivalent(NSPageDownFunctionKey),
        KeyCode::Escape => private_key_equivalent(ESCAPE_KEY_EQUIVALENT),
        KeyCode::Space => Some(" ".to_string()),
        KeyCode::F1 => private_key_equivalent(NSF1FunctionKey),
        KeyCode::F2 => private_key_equivalent(NSF2FunctionKey),
        KeyCode::F3 => private_key_equivalent(NSF3FunctionKey),
        KeyCode::F4 => private_key_equivalent(NSF4FunctionKey),
        KeyCode::F5 => private_key_equivalent(NSF5FunctionKey),
        KeyCode::F6 => private_key_equivalent(NSF6FunctionKey),
        KeyCode::F7 => private_key_equivalent(NSF7FunctionKey),
        KeyCode::F8 => private_key_equivalent(NSF8FunctionKey),
        KeyCode::F9 => private_key_equivalent(NSF9FunctionKey),
        KeyCode::F10 => private_key_equivalent(NSF10FunctionKey),
        KeyCode::F11 => private_key_equivalent(NSF11FunctionKey),
        KeyCode::F12 => private_key_equivalent(NSF12FunctionKey),
        KeyCode::Unknown | KeyCode::Shift | KeyCode::Control | KeyCode::Alt | KeyCode::Meta => None,
    }
}

fn private_key_equivalent(key: u32) -> Option<String> {
    char::from_u32(key).map(|c| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_quit_callback_is_bounded_consumed_once_and_retired() {
        let roles = Rc::new(ApplicationRoles::new());
        let target = MenuTargetIvars {
            activations: Rc::new(RefCell::new(Vec::new())),
            roles: roles.clone(),
        };
        target.roles.request_quit();
        for _ in 0..64 {
            target.roles.request_quit();
        }
        assert!(
            target.activations.borrow().is_empty(),
            "Quit is not a sentinel MenuItemId"
        );
        assert!(roles.take_quit());
        assert!(!roles.take_quit());
        target.roles.request_quit();
        roles.retire();
        target.roles.request_quit();
        assert!(
            !roles.take_quit(),
            "a separately retained native target has no retired authority"
        );
    }

    #[test]
    fn standard_roles_and_product_projection_do_not_reset_pending_quit() {
        let roles = ApplicationRoles::new();
        roles.request_quit();
        // Initialization may replace the wake, but must not replace request custody.
        *roles.wake.borrow_mut() = None;
        assert!(roles.quit.get());
        let source = include_str!("native_menu_macos.rs");
        let install = source
            .split("fn install(&mut self, model:")
            .nth(1)
            .unwrap()
            .split("fn sync_item(")
            .next()
            .unwrap();
        assert!(install.contains("install_application_menu"));
        assert!(
            !install.contains("self.roles"),
            "model install/replacement cannot reset the host's pending role"
        );
        let callback = source
            .split("fn halcyon_application_quit(")
            .nth(1)
            .unwrap()
            .split("    }\n")
            .next()
            .unwrap();
        assert!(callback.contains("self.ivars().roles.request_quit()"));
        assert!(!callback.contains("request_exit") && !callback.contains("terminate:"));
        assert!(roles.take_quit());
        assert!(!roles.take_quit());
    }

    #[test]
    fn cmd_q_is_not_projected_as_halcyon_native_key_equivalent() {
        let shortcut = Shortcut::new(KeyCode::Character('q'), false, false, false, true);

        assert!(shortcut_projection(shortcut).is_none());
    }

    #[test]
    fn non_reserved_command_shortcut_projects() {
        let shortcut = Shortcut::new(KeyCode::Character('p'), false, false, false, true);

        assert!(shortcut_projection(shortcut).is_some());
    }
}
