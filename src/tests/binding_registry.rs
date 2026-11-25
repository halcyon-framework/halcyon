use crate::core::event::KeyCode;
use crate::runtime::actions::{ActionId, Shortcut};
use crate::runtime::binding_registry::*;

#[test]
fn physical_keyboard_binding_classifies_reserved_and_unobserved() {
    use crate::core::event::{
        BindingKey, KeyEvent, KeyboardCaptureInterruptionReason as Reason, KeyboardKeyReservation,
        Modifiers, PhysicalBindingUnavailable as Unavailable, PhysicalKeyCode,
    };
    assert_eq!(
        Shortcut::from_physical_response(None, None),
        Err(Unavailable::Unobserved)
    );
    let key = KeyEvent {
        key_code: KeyCode::Character('ф'),
        physical_key: PhysicalKeyCode::KeyA,
        is_down: true,
        is_repeat: false,
        modifiers: Modifiers {
            ctrl: true,
            meta: true,
            shift: false,
            alt: false,
        },
        text: None,
    };
    let binding = Shortcut::from_physical_response(Some(&key), None).unwrap();
    assert_eq!(binding.key, BindingKey::Physical(PhysicalKeyCode::KeyA));
    assert!(binding.ctrl && binding.meta && !binding.shift && !binding.alt);
    assert!(binding.matches_key_event(&key));
    assert_eq!(
        Shortcut::from_physical_response(
            Some(&KeyEvent {
                physical_key: PhysicalKeyCode::Unknown,
                ..key.clone()
            }),
            None
        ),
        Err(Unavailable::UnknownPhysicalKey),
    );
    for (logical, physical, reason) in [
        (KeyCode::Escape, PhysicalKeyCode::Escape, Reason::Escape),
        (KeyCode::Tab, PhysicalKeyCode::Tab, Reason::Tab),
        (KeyCode::F9, PhysicalKeyCode::F9, Reason::AppConsumed),
    ] {
        let reserved = KeyEvent {
            key_code: logical,
            physical_key: physical,
            ..key.clone()
        };
        assert_eq!(
            Shortcut::from_physical_response(Some(&reserved), None),
            Err(Unavailable::Reserved(reason))
        );
    }
    let f12 = KeyEvent {
        key_code: KeyCode::F12,
        physical_key: PhysicalKeyCode::F12,
        ..key.clone()
    };
    assert_eq!(
        Shortcut::from_physical_response(Some(&f12), None)
            .unwrap()
            .key,
        BindingKey::Physical(PhysicalKeyCode::F12)
    );
    let fullscreen = KeyEvent {
        key_code: KeyCode::Character('F'),
        physical_key: PhysicalKeyCode::KeyJ,
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
        ..key.clone()
    };
    assert_eq!(
        Shortcut::from_physical_response(Some(&fullscreen), None),
        Err(Unavailable::Reserved(Reason::Reserved(
            KeyboardKeyReservation::Fullscreen
        ))),
    );
    let other_layout_observation = KeyEvent {
        key_code: KeyCode::Character('ф'),
        physical_key: PhysicalKeyCode::KeyF,
        ..fullscreen
    };
    assert_eq!(
        Shortcut::from_physical_response(Some(&other_layout_observation), None)
            .unwrap()
            .key,
        BindingKey::Physical(PhysicalKeyCode::KeyF),
    );
    let clipboard = KeyEvent {
        key_code: KeyCode::Character('v'),
        physical_key: PhysicalKeyCode::KeyV,
        ..key.clone()
    };
    assert!(
        Shortcut::from_physical_response(Some(&clipboard), Some(KeyboardKeyReservation::Clipboard))
            .is_ok()
    );
    assert_eq!(
        Shortcut::from_physical_response(
            Some(&KeyEvent {
                is_repeat: true,
                ..key.clone()
            }),
            None
        ),
        Err(Unavailable::NotInitialDown),
    );
    assert_eq!(
        Shortcut::from_physical_response(
            Some(&KeyEvent {
                physical_key: PhysicalKeyCode::ControlRight,
                ..key.clone()
            }),
            None
        ),
        Err(Unavailable::ModifierOnly),
    );
    let logical = Shortcut::from_key_and_modifiers(key.key_code, key.modifiers);
    assert_eq!(logical.key, BindingKey::Logical(KeyCode::Character('ф')));
}

#[test]
fn physical_keyboard_bindings_keep_identity_modifiers_and_single_action() {
    use crate::core::event::{BindingKey, KeyEvent, Modifiers, PhysicalKeyCode};
    let modifiers = Modifiers {
        ctrl: true,
        meta: false,
        alt: false,
        shift: true,
    };
    let physical =
        Shortcut::from_physical_key_and_modifiers(PhysicalKeyCode::KeyA, modifiers).unwrap();
    let logical = Shortcut::from_key_and_modifiers(KeyCode::Character('ф'), modifiers);
    assert_ne!(physical, logical);
    assert_eq!(physical.key, BindingKey::Physical(PhysicalKeyCode::KeyA));
    let key = KeyEvent {
        key_code: KeyCode::Character('ф'),
        physical_key: PhysicalKeyCode::KeyA,
        is_down: true,
        is_repeat: false,
        modifiers,
        text: None,
    };
    assert!(physical.matches_key_event(&key));
    assert!(logical.matches_key_event(&key));
    assert!(!physical.matches_key_event(&KeyEvent {
        modifiers: Modifiers {
            meta: true,
            ctrl: false,
            ..modifiers
        },
        ..key.clone()
    }));
    let mut registry = BindingRegistry::new();
    for (pattern, action_id, priority) in [
        (
            InputPattern::from(&logical),
            ActionId::FOCUS_NEXT,
            BindingPriority::Normal,
        ),
        (
            InputPattern::from(&physical),
            ActionId::ENTER_WIDGET,
            BindingPriority::Hard,
        ),
    ] {
        registry.add(Binding {
            pattern,
            action_id,
            priority,
            layer: BindingLayer::App,
            tag: None,
        });
    }
    assert_eq!(
        registry.lookup_key(key.key_code, key.physical_key, modifiers),
        Some(ActionId::ENTER_WIDGET)
    );
    assert_eq!(
        registry.lookup_key(KeyCode::Character('ф'), PhysicalKeyCode::KeyB, modifiers),
        Some(ActionId::FOCUS_NEXT)
    );
    assert_eq!(
        registry.lookup_key(KeyCode::Character('a'), PhysicalKeyCode::KeyA, modifiers),
        Some(ActionId::ENTER_WIDGET)
    );
    assert_eq!(
        registry.lookup_key(KeyCode::Character('a'), PhysicalKeyCode::KeyB, modifiers),
        None
    );
    assert!(
        InputPattern::from_physical_key_modifiers(PhysicalKeyCode::Unknown, modifiers).is_err()
    );
    assert_ne!(
        InputPattern::from_physical_key_modifiers(PhysicalKeyCode::Digit1, modifiers).unwrap(),
        InputPattern::from_physical_key_modifiers(PhysicalKeyCode::Numpad1, modifiers).unwrap()
    );
}

#[test]
fn physical_keyboard_binding_conflicts_follow_priority_layer_and_order() {
    use crate::core::event::{Modifiers, PhysicalKeyCode};
    let modifiers = Modifiers::default();
    let logical = InputPattern::from_key_modifiers(KeyCode::Character('ф'), modifiers);
    let physical =
        InputPattern::from_physical_key_modifiers(PhysicalKeyCode::KeyA, modifiers).unwrap();
    let add = |registry: &mut BindingRegistry, pattern, layer, priority, action_id| {
        registry.add(Binding {
            pattern,
            layer,
            priority,
            action_id,
            tag: None,
        });
    };
    let lookup = |registry: &mut BindingRegistry| {
        registry.lookup_key(KeyCode::Character('ф'), PhysicalKeyCode::KeyA, modifiers)
    };
    let mut registry = BindingRegistry::new();
    add(
        &mut registry,
        logical,
        BindingLayer::App,
        BindingPriority::Normal,
        ActionId(800),
    );
    add(
        &mut registry,
        physical,
        BindingLayer::Runtime,
        BindingPriority::Soft,
        ActionId(801),
    );
    assert_eq!(lookup(&mut registry), Some(ActionId(800)));
    add(
        &mut registry,
        physical,
        BindingLayer::Runtime,
        BindingPriority::Normal,
        ActionId(802),
    );
    assert_eq!(lookup(&mut registry), Some(ActionId(802)));
    add(
        &mut registry,
        logical,
        BindingLayer::Runtime,
        BindingPriority::Normal,
        ActionId(803),
    );
    assert_eq!(lookup(&mut registry), Some(ActionId(803)));
    add(
        &mut registry,
        physical,
        BindingLayer::Runtime,
        BindingPriority::Normal,
        ActionId(804),
    );
    assert_eq!(lookup(&mut registry), Some(ActionId(804)));
    add(
        &mut registry,
        logical,
        BindingLayer::System,
        BindingPriority::Hard,
        ActionId::RELEASE_CAPTURE,
    );
    assert_eq!(lookup(&mut registry), Some(ActionId::RELEASE_CAPTURE));
    add(
        &mut registry,
        physical,
        BindingLayer::App,
        BindingPriority::Hard,
        ActionId::ENTER_WIDGET,
    );
    assert_eq!(lookup(&mut registry), Some(ActionId::ENTER_WIDGET));
}

#[test]
fn test_basic_lookup() {
    let mut reg = BindingRegistry::new();
    let pattern = InputPattern::key_only(KeyCode::Character('a'));
    let action = ActionId(1);

    reg.add(Binding {
        pattern,
        action_id: action,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: None,
    });

    assert_eq!(reg.lookup(&pattern), Some(action));
}

#[test]
fn test_layer_override() {
    let mut reg = BindingRegistry::new();
    let pattern = InputPattern::key_only(KeyCode::Character('x'));
    let system_action = ActionId(10);
    let app_action = ActionId(20);

    reg.add(Binding {
        pattern,
        action_id: system_action,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: None,
    });
    reg.add(Binding {
        pattern,
        action_id: app_action,
        layer: BindingLayer::App,
        priority: BindingPriority::Normal,
        tag: None,
    });

    // App wins over System (same priority, higher layer)
    assert_eq!(reg.lookup(&pattern), Some(app_action));
}

#[test]
fn test_priority_override() {
    let mut reg = BindingRegistry::new();
    let pattern = InputPattern::key_only(KeyCode::Character('z'));
    let app_soft = ActionId(30);
    let system_hard = ActionId(40);

    reg.add(Binding {
        pattern,
        action_id: app_soft,
        layer: BindingLayer::App,
        priority: BindingPriority::Soft,
        tag: None,
    });
    reg.add(Binding {
        pattern,
        action_id: system_hard,
        layer: BindingLayer::System,
        priority: BindingPriority::Hard,
        tag: None,
    });

    // Hard wins regardless of layer
    assert_eq!(reg.lookup(&pattern), Some(system_hard));
}

#[test]
fn test_runtime_layer_swap() {
    let mut reg = BindingRegistry::new();
    let pattern = InputPattern::key_only(KeyCode::Character('n'));
    let nav_action = ActionId(50);
    let capture_action = ActionId(60);

    // Add tagged runtime bindings ("nav_mode")
    reg.add(Binding {
        pattern,
        action_id: nav_action,
        layer: BindingLayer::Runtime,
        priority: BindingPriority::Normal,
        tag: Some("nav_mode"),
    });
    assert_eq!(reg.lookup(&pattern), Some(nav_action));

    // Remove by tag
    reg.remove_by_tag("nav_mode");
    assert_eq!(reg.lookup(&pattern), None);

    // Add different tagged bindings ("capture_mode")
    reg.add(Binding {
        pattern,
        action_id: capture_action,
        layer: BindingLayer::Runtime,
        priority: BindingPriority::Normal,
        tag: Some("capture_mode"),
    });
    assert_eq!(reg.lookup(&pattern), Some(capture_action));
}

#[test]
fn test_remove_all_runtime() {
    let mut reg = BindingRegistry::new();
    let pattern_a = InputPattern::key_only(KeyCode::Character('a'));
    let pattern_b = InputPattern::key_only(KeyCode::Character('b'));
    let system_action = ActionId(70);
    let runtime_action_a = ActionId(80);
    let runtime_action_b = ActionId(90);

    // System binding
    reg.add(Binding {
        pattern: pattern_a,
        action_id: system_action,
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: None,
    });
    // Runtime bindings
    reg.add(Binding {
        pattern: pattern_a,
        action_id: runtime_action_a,
        layer: BindingLayer::Runtime,
        priority: BindingPriority::Normal,
        tag: Some("mode"),
    });
    reg.add(Binding {
        pattern: pattern_b,
        action_id: runtime_action_b,
        layer: BindingLayer::Runtime,
        priority: BindingPriority::Normal,
        tag: Some("mode"),
    });

    // Runtime overrides system for pattern_a
    assert_eq!(reg.lookup(&pattern_a), Some(runtime_action_a));
    assert_eq!(reg.lookup(&pattern_b), Some(runtime_action_b));

    // Clear runtime
    reg.remove_all_runtime();

    // System binding survives, runtime bindings gone
    assert_eq!(reg.lookup(&pattern_a), Some(system_action));
    assert_eq!(reg.lookup(&pattern_b), None);
}

#[test]
fn test_no_binding_returns_none() {
    let mut reg = BindingRegistry::new();
    let pattern = InputPattern::key_only(KeyCode::Character('q'));
    assert_eq!(reg.lookup(&pattern), None);
}

#[test]
fn test_single_action_per_input() {
    // FORBID-ACT-BIND-MULTI: same pattern never produces multiple ActionIds
    let mut reg = BindingRegistry::new();
    let pattern = InputPattern::key_only(KeyCode::Character('m'));

    reg.add(Binding {
        pattern,
        action_id: ActionId(100),
        layer: BindingLayer::System,
        priority: BindingPriority::Normal,
        tag: None,
    });
    reg.add(Binding {
        pattern,
        action_id: ActionId(200),
        layer: BindingLayer::App,
        priority: BindingPriority::Normal,
        tag: None,
    });
    reg.add(Binding {
        pattern,
        action_id: ActionId(300),
        layer: BindingLayer::Runtime,
        priority: BindingPriority::Soft,
        tag: None,
    });

    // Exactly one result — App/Normal beats Runtime/Soft (priority wins),
    // and App/Normal beats System/Normal (higher layer wins)
    let result = reg.lookup(&pattern);
    assert!(result.is_some());
    assert_eq!(result, Some(ActionId(200)));
}

#[test]
fn test_from_conversions() {
    use crate::core::event::Modifiers;

    let key = KeyCode::Character('p');
    let mods = Modifiers {
        ctrl: true,
        alt: false,
        shift: true,
        meta: false,
    };

    let from_method = InputPattern::from_key_modifiers(key, mods);

    let shortcut = Shortcut::new(key, true, false, true, false);
    let from_shortcut = InputPattern::from(&shortcut);

    assert_eq!(from_method, from_shortcut);
    assert_eq!(from_method.key, key);
    assert!(from_method.ctrl);
    assert!(!from_method.alt);
    assert!(from_method.shift);
    assert!(!from_method.meta);
}

#[test]
fn test_engine_action_ids_are_engine() {
    assert!(ActionId::FOCUS_LEFT.is_engine_action());
    assert!(ActionId::FOCUS_RIGHT.is_engine_action());
    assert!(ActionId::FOCUS_UP.is_engine_action());
    assert!(ActionId::FOCUS_DOWN.is_engine_action());
    assert!(ActionId::FOCUS_NEXT.is_engine_action());
    assert!(ActionId::FOCUS_PREV.is_engine_action());
    assert!(ActionId::ENTER_WIDGET.is_engine_action());
    assert!(ActionId::RELEASE_CAPTURE.is_engine_action());
    assert!(ActionId::EXIT_SCOPE.is_engine_action());
    assert!(ActionId::ACTIVATE.is_engine_action());
}

#[test]
fn test_app_actions_not_engine() {
    assert!(!ActionId(1).is_engine_action()); // theme toggle
    assert!(!ActionId(2).is_engine_action()); // action palette
    assert!(!ActionId(1000).is_engine_action());
}

#[test]
fn test_keyboard_mode_default() {
    use crate::core::event::KeyboardAuthorityMode;
    assert_eq!(
        KeyboardAuthorityMode::default(),
        KeyboardAuthorityMode::Navigation
    );
}

#[test]
fn test_domain_state_default_mode() {
    use crate::core::event::{InteractionDomainState, KeyboardAuthorityMode};
    let state = InteractionDomainState::new();
    assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
}
