// ============================================================================
// SPEC-HAL-020: Built-in Actions - Stage 3
// ============================================================================

use crate::core::event::KeyCode;
use crate::runtime::actions::{
    Action, ActionHandler, ActionId, ActionRegistry, ActionScope, Shortcut,
};
use crate::runtime::binding_registry::*;

/// Register built-in actions into the action registry.
///
/// Stage 3: Registers one action (Toggle Theme) to verify the action system.
/// Stage 7: Adds Open Action Palette action.
pub fn register_builtin_actions(registry: &mut ActionRegistry) {
    // Action 1: Toggle Theme (Ctrl+Shift+L)
    registry.add(Action {
        id: ActionId(1),
        name: "Toggle Theme",
        description: "Switch between light and dark theme",
        category: "System",
        shortcut: Some(Shortcut::ctrl_shift(KeyCode::Character('l'))),
        scope: ActionScope::Global,
        handler: |cx| {
            cx.theme.toggle();
            tracing::info!(
                "Theme toggled to {}",
                if cx.theme.is_dark() { "dark" } else { "light" }
            );
        },
    });

    // Action 2: Open Action Palette (Cmd+Shift+P on macOS, Ctrl+Shift+P elsewhere)
    registry.add(Action {
        id: ActionId(2),
        name: "Open Action Palette",
        description: "Search and execute actions",
        category: "System",
        shortcut: Some(Shortcut::platform_command(KeyCode::Character('p'), true)),
        scope: ActionScope::Global,
        handler: |cx| {
            let modal = Box::new(crate::core::action_palette_modal::ActionPaletteModal::new());
            cx.queue_nav(crate::core::app::NavCommand::OpenModal(modal));
        },
    });

    // ========================================================================
    // SPEC-HAL-176: Engine-level actions (dispatched by event router, not handler)
    // ========================================================================

    let noop: ActionHandler = |_cx| {};

    // Spatial navigation
    registry.add(Action {
        id: ActionId::FOCUS_LEFT,
        name: "Focus Left",
        description: "Move focus to nearest widget to the left",
        category: "Navigation",
        shortcut: None,
        scope: ActionScope::Global,
        handler: noop,
    });
    registry.add(Action {
        id: ActionId::FOCUS_RIGHT,
        name: "Focus Right",
        description: "Move focus to nearest widget to the right",
        category: "Navigation",
        shortcut: None,
        scope: ActionScope::Global,
        handler: noop,
    });
    registry.add(Action {
        id: ActionId::FOCUS_UP,
        name: "Focus Up",
        description: "Move focus to nearest widget above",
        category: "Navigation",
        shortcut: None,
        scope: ActionScope::Global,
        handler: noop,
    });
    registry.add(Action {
        id: ActionId::FOCUS_DOWN,
        name: "Focus Down",
        description: "Move focus to nearest widget below",
        category: "Navigation",
        shortcut: None,
        scope: ActionScope::Global,
        handler: noop,
    });

    // Structural navigation
    registry.add(Action {
        id: ActionId::FOCUS_NEXT,
        name: "Focus Next",
        description: "Move focus to next widget in tab order",
        category: "Navigation",
        shortcut: None,
        scope: ActionScope::Global,
        handler: noop,
    });
    registry.add(Action {
        id: ActionId::FOCUS_PREV,
        name: "Focus Previous",
        description: "Move focus to previous widget in tab order",
        category: "Navigation",
        shortcut: None,
        scope: ActionScope::Global,
        handler: noop,
    });

    // Mode transitions
    registry.add(Action {
        id: ActionId::ENTER_WIDGET,
        name: "Enter Widget",
        description: "Enter capture mode or push focus scope",
        category: "Navigation",
        shortcut: None,
        scope: ActionScope::Global,
        handler: noop,
    });
    registry.add(Action {
        id: ActionId::RELEASE_CAPTURE,
        name: "Release Capture",
        description: "Release keyboard capture, return to navigation mode",
        category: "Navigation",
        shortcut: None,
        scope: ActionScope::Global,
        handler: noop,
    });
    registry.add(Action {
        id: ActionId::EXIT_SCOPE,
        name: "Exit Scope",
        description: "Pop current focus scope level",
        category: "Navigation",
        shortcut: None,
        scope: ActionScope::Global,
        handler: noop,
    });

    // Activation
    registry.add(Action {
        id: ActionId::ACTIVATE,
        name: "Activate",
        description: "Activate focused widget without mode change",
        category: "Navigation",
        shortcut: None,
        scope: ActionScope::Global,
        handler: noop,
    });

    // ========================================================================
    // SPEC-HAL-176: Default Navigation-mode bindings (tagged "navigation")
    // ========================================================================

    // Spatial: hjkl (vim) — Navigation mode only
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Character('h')),
        action_id: ActionId::FOCUS_LEFT,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Character('l')),
        action_id: ActionId::FOCUS_RIGHT,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Character('k')),
        action_id: ActionId::FOCUS_UP,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Character('j')),
        action_id: ActionId::FOCUS_DOWN,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });

    // Spatial: arrow keys — Navigation mode only
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Left),
        action_id: ActionId::FOCUS_LEFT,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Right),
        action_id: ActionId::FOCUS_RIGHT,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Up),
        action_id: ActionId::FOCUS_UP,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Down),
        action_id: ActionId::FOCUS_DOWN,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });

    // Structural: Tab / Shift+Tab — Navigation mode
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Tab),
        action_id: ActionId::FOCUS_NEXT,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });
    registry.binding_registry.add(Binding {
        pattern: InputPattern {
            key: (KeyCode::Tab).into(),
            ctrl: false,
            alt: false,
            shift: true,
            meta: false,
        },
        action_id: ActionId::FOCUS_PREV,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });

    // Enter widget: i or Enter — Navigation mode
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Character('i')),
        action_id: ActionId::ENTER_WIDGET,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Enter),
        action_id: ActionId::ENTER_WIDGET,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });

    // Activate: Space — Navigation mode (inline activation, no mode change)
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Space),
        action_id: ActionId::ACTIVATE,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });

    // Escape in Navigation mode: exit scope
    registry.binding_registry.add(Binding {
        pattern: InputPattern::key_only(KeyCode::Escape),
        action_id: ActionId::EXIT_SCOPE,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: Some("navigation"),
    });
}
