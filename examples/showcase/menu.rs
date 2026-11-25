// Halcyon — Showcase: application main menu (SPEC-HAL-333).
// Demonstrates the in-window menu pipeline end to end: MenuModel as the
// single source of truth (Class A), Action System binding (INV-A4),
// runtime state mutation with per-frame host sync (INV-A5), and
// activation dispatch through AppRoot (Class E/G).
//
// Every item is a real command — no placeholder entries.

use halcyon::core::app::{AppContext, ScreenContext};
use halcyon::model::menu_item::{MenuItem, MenuItemId};
use halcyon::model::menu_model::MenuModel;
use halcyon::runtime::actions::{Action, ActionId, ActionRegistry, ActionScope};

// Item ids — Showcase menu block.
const ITEM_FILE_EXIT: MenuItemId = 101;
const ITEM_DEMO_PUSH_TOAST: MenuItemId = 201;
const ITEM_DEMO_TOGGLE_TOAST_ENABLED: MenuItemId = 202;
const ITEM_HELP_ABOUT: MenuItemId = 301;

// Action ids — Showcase menu block (3001–3004). Builtins occupy 1–2 and
// the engine range 100–199; plot demos reserve 2000–2099.
const ACTION_EXIT: ActionId = ActionId(3001);
const ACTION_PUSH_TOAST: ActionId = ActionId(3002);
const ACTION_TOGGLE_TOAST_ENABLED: ActionId = ActionId(3003);
const ACTION_ABOUT: ActionId = ActionId(3004);

/// Install the Showcase main menu once. Idempotent: no-op when a model is
/// already supplied. The application owns the model (INV-G1); AppRoot
/// installs it on the active host on the next frame.
pub fn ensure_main_menu(cx: &mut ScreenContext<'_>) {
    if cx.app.menu_model_mut().is_some() {
        return;
    }
    register_menu_actions(cx.app);
    let mut model = build_model();
    bind_model(&mut model, &cx.app.registry);
    cx.app.set_menu_model(model);
}

fn register_menu_actions(app: &mut AppContext) {
    app.registry.add(Action {
        id: ACTION_EXIT,
        name: "Exit",
        description: "Exit the Showcase",
        category: "Showcase Menu",
        shortcut: None,
        scope: ActionScope::Global,
        handler: handle_exit,
    });
    app.registry.add(Action {
        id: ACTION_PUSH_TOAST,
        name: "Push Toast",
        description: "Push a toast from the main menu",
        category: "Showcase Menu",
        shortcut: None,
        scope: ActionScope::Global,
        handler: handle_push_toast,
    });
    app.registry.add(Action {
        id: ACTION_TOGGLE_TOAST_ENABLED,
        name: "Toggle \"Push Toast\" Enabled",
        description: "Live INV-A5 demo: disable or re-enable a sibling item",
        category: "Showcase Menu",
        shortcut: None,
        scope: ActionScope::Global,
        handler: handle_toggle_toast_enabled,
    });
    app.registry.add(Action {
        id: ACTION_ABOUT,
        name: "About Halcyon",
        description: "About the Halcyon Showcase",
        category: "Showcase Menu",
        shortcut: None,
        scope: ActionScope::Global,
        handler: handle_about,
    });
}

fn build_model() -> MenuModel {
    let mut model = MenuModel::new();
    model.add_title("File", vec![action_row(ITEM_FILE_EXIT, "Exit")]);
    model.add_title(
        "Demo",
        vec![
            action_row(ITEM_DEMO_PUSH_TOAST, "Push Toast"),
            MenuItem::Separator,
            action_row(ITEM_DEMO_TOGGLE_TOAST_ENABLED, "Disable \"Push Toast\""),
        ],
    );
    model.add_title("Help", vec![action_row(ITEM_HELP_ABOUT, "About Halcyon")]);
    model
}

fn bind_model(model: &mut MenuModel, registry: &ActionRegistry) {
    model.bind(ITEM_FILE_EXIT, ACTION_EXIT, registry);
    model.bind(ITEM_DEMO_PUSH_TOAST, ACTION_PUSH_TOAST, registry);
    model.bind(
        ITEM_DEMO_TOGGLE_TOAST_ENABLED,
        ACTION_TOGGLE_TOAST_ENABLED,
        registry,
    );
    model.bind(ITEM_HELP_ABOUT, ACTION_ABOUT, registry);
}

fn action_row(id: MenuItemId, label: &str) -> MenuItem {
    MenuItem::ActionRow {
        id,
        label: label.to_string(),
        icon: None,
        shortcut: None,
        disabled: false,
        submenu: None,
    }
}

// ---------------------------------------------------------------------------
// Handlers — dispatched by AppRoot::sync_menu_host (SPEC-HAL-333 Class E/G).
// ---------------------------------------------------------------------------

fn handle_exit(cx: &mut AppContext) {
    cx.request_exit();
}

fn handle_push_toast(cx: &mut AppContext) {
    let _ = cx.push_toast("Toast pushed from the main menu", 3000, None);
}

/// Live INV-A5 demonstrator: flips the enabled state of "Push Toast" and
/// rewrites this item's own label. Both mutations mark the model dirty;
/// AppRoot mirrors them to the host next frame and the bar re-projects.
fn handle_toggle_toast_enabled(cx: &mut AppContext) {
    let Some(model) = cx.menu_model_mut() else {
        return;
    };
    let enabled = model
        .item_state(ITEM_DEMO_PUSH_TOAST)
        .map(|s| s.enabled)
        .unwrap_or(true);
    let next = !enabled;
    model.set_enabled(ITEM_DEMO_PUSH_TOAST, next);
    model.set_label(
        ITEM_DEMO_TOGGLE_TOAST_ENABLED,
        if next {
            "Disable \"Push Toast\"".to_string()
        } else {
            "Enable \"Push Toast\"".to_string()
        },
    );
}

fn handle_about(cx: &mut AppContext) {
    let _ = cx.push_toast(
        "Halcyon Showcase \u{2014} GPU-first professional UI framework",
        4000,
        None,
    );
}
