fn assert_browser_key_pair_order(callback: &str, state: &str) {
    let source = include_str!("../src/platform_impl/web/event_loop/window_target.rs");
    let callback = source
        .split_once(callback)
        .expect("the selected winit callback exists")
        .1
        .split_once("\n        );")
        .expect("the selected callback has its original closing delimiter")
        .0;
    assert!(callback.contains("modifiers.get() != active_modifiers"));
    assert!(callback.contains("modifiers.set(active_modifiers)"));
    assert_eq!(callback.matches("WindowEvent::ModifiersChanged").count(), 1);
    assert_eq!(callback.matches("WindowEvent::KeyboardInput").count(), 1);
    assert!(callback.contains(state));
    let dispatch: String = callback
        .split_once("runner.send_events(")
        .expect("the original dispatch is present")
        .1
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    assert!(
        dispatch.starts_with("modifiers_changed.into_iter().chain(iter::once(Event::WindowEvent{"),
        "the same DOM event's changed modifier snapshot must precede its one original key"
    );
}

#[test]
fn safari_shortcut_web_keydown_publishes_modifiers_before_key() {
    assert_browser_key_pair_order("canvas.on_keyboard_press(", "ElementState::Pressed");
}

#[test]
fn safari_shortcut_web_keyup_publishes_modifiers_before_key() {
    assert_browser_key_pair_order("canvas.on_keyboard_release(", "ElementState::Released");
}
