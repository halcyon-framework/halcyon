// SPEC-HAL-108 §13: Text Editing Policy Enforcement
//
// Centralized policy decision function for TextField and TextArea.
// Widgets call this before ordinary event side effects; lifecycle cleanup is separate.

use crate::core::event::{Event, KeyCode, KeyEvent};

// ============================================================================
// SPEC-HAL-109 Stage F: Single-line vs Multi-line Editing Policy
// ============================================================================

/// Policy governing single-line vs multi-line editing behavior.
///
/// This struct captures all legitimate behavioral differences between
/// TextField and TextArea. INV-F.P1 requires all such differences to
/// consult this policy explicitly.
#[derive(Debug, Clone, Copy)]
pub struct TextEditingPolicy {
    /// True for single-line widgets (TextField), false for multi-line (TextArea).
    pub single_line: bool,
    /// Whether Enter key inserts a newline. False for TextField.
    pub allow_newline: bool,
    /// Whether pasted newlines are normalized to spaces. True for TextField.
    pub normalize_paste_newlines: bool,
    /// Whether Up/Down arrow navigation is enabled. False for TextField.
    pub allow_vertical_navigation: bool,
    /// Whether scroll wheel events are handled. False for TextField.
    pub allow_vertical_scroll: bool,
}

impl TextEditingPolicy {
    /// Policy for single-line text fields.
    pub fn single_line() -> Self {
        Self {
            single_line: true,
            allow_newline: false,
            normalize_paste_newlines: true,
            allow_vertical_navigation: false,
            allow_vertical_scroll: false,
        }
    }

    /// Policy for multi-line text areas.
    pub fn multi_line() -> Self {
        Self {
            single_line: false,
            allow_newline: true,
            normalize_paste_newlines: false,
            allow_vertical_navigation: true,
            allow_vertical_scroll: true,
        }
    }
}

impl Default for TextEditingPolicy {
    fn default() -> Self {
        Self::multi_line()
    }
}

/// Policy configuration for text editing widgets.
///
/// This mirrors the conceptual TextEditPolicy from SPEC-HAL-108.
/// Widgets may construct this from their config fields.
#[derive(Debug, Clone, Copy, Default)]
pub struct TextEditPolicy {
    pub enabled: bool,
    pub read_only: bool,
}

/// Decision outcome from policy evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Event is allowed; widget continues normal routing.
    Allow,
    /// Event is denied; widget returns true (consumed, no mutation).
    DenyConsume,
    /// Event is denied; widget returns false (bubble, no interaction).
    DenyBubble,
}

/// Evaluate policy against an event.
///
/// Decision rules (SPEC-HAL-108 §13):
/// 1. enabled == false → DenyBubble
/// 2. enabled == true AND read_only == true AND event is mutation → DenyConsume
/// 3. Otherwise → Allow
pub fn decide_text_edit_policy(policy: &TextEditPolicy, event: &Event) -> PolicyDecision {
    decide_text_edit_operation(policy, || is_mutation_event(event))
}

pub(crate) fn decide_text_edit_operation(
    policy: &TextEditPolicy,
    mutates_document: impl FnOnce() -> bool,
) -> PolicyDecision {
    // Rule 1: Disabled widget denies all interaction
    if !policy.enabled {
        return PolicyDecision::DenyBubble;
    }

    // Rule 2: Read-only widget denies mutations but consumes them
    if policy.read_only && mutates_document() {
        return PolicyDecision::DenyConsume;
    }

    // Rule 3: Otherwise allow
    PolicyDecision::Allow
}

pub(crate) fn is_primary_shortcut(modifiers: crate::core::event::Modifiers) -> bool {
    primary_shortcut_for_target(
        modifiers,
        cfg!(target_arch = "wasm32"),
        cfg!(target_os = "macos"),
    )
}

fn primary_shortcut_for_target(
    modifiers: crate::core::event::Modifiers,
    browser: bool,
    macos: bool,
) -> bool {
    // A browser target does not identify the host OS. Both conventional editing
    // modifiers remain qualified by the original widget's keyboard Capture.
    if browser {
        modifiers.meta || modifiers.ctrl
    } else if macos {
        modifiers.meta
    } else {
        modifiers.ctrl
    }
}

pub(crate) fn is_redo_shortcut(modifiers: crate::core::event::Modifiers, text: &str) -> bool {
    redo_shortcut_for_target(
        modifiers,
        text,
        cfg!(target_arch = "wasm32"),
        cfg!(target_os = "macos"),
    )
}

fn redo_shortcut_for_target(
    modifiers: crate::core::event::Modifiers,
    text: &str,
    browser: bool,
    macos: bool,
) -> bool {
    (modifiers.meta && (browser || macos) && modifiers.shift && text.eq_ignore_ascii_case("z"))
        || (modifiers.ctrl
            && (browser || !macos)
            && (text.eq_ignore_ascii_case("y")
                || (modifiers.shift && text.eq_ignore_ascii_case("z"))))
}

fn is_mutating_shortcut(text: &str) -> bool {
    ["x", "v", "z", "y"]
        .iter()
        .any(|key| text.eq_ignore_ascii_case(key))
}

/// Classify edits before clipboard, history, composition or caret side effects.
fn is_mutation_event(event: &Event) -> bool {
    match event {
        Event::Key(KeyEvent {
            key_code,
            modifiers,
            is_down: true,
            ..
        }) => {
            matches!(
                key_code,
                KeyCode::Backspace | KeyCode::Delete | KeyCode::Enter | KeyCode::Space
            ) || (is_primary_shortcut(*modifiers)
                && matches!(key_code, KeyCode::Character(ch) if matches!(ch.to_ascii_lowercase(), 'x' | 'v' | 'z' | 'y')))
        }
        Event::Text(text_event) => {
            if is_primary_shortcut(text_event.modifiers) {
                is_mutating_shortcut(&text_event.text)
            } else {
                !text_event.modifiers.ctrl && !text_event.modifiers.meta
            }
        }
        Event::Ime(
            crate::core::event::ImeEvent::PreEdit { .. }
            | crate::core::event::ImeEvent::Commit { .. },
        ) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_primary_shortcut, is_redo_shortcut, primary_shortcut_for_target,
        redo_shortcut_for_target,
    };
    use crate::core::event::Modifiers;

    #[test]
    fn safari_shortcut_primary_preserves_native_platform_policy() {
        let cases = [
            (false, false, false, [false, false, false]),
            (true, false, false, [true, false, true]),
            (false, true, false, [true, true, false]),
            (true, true, false, [true, true, true]),
            (false, false, true, [false, false, false]),
        ];
        for (ctrl, meta, alt, expected) in cases {
            let modifiers = Modifiers {
                ctrl,
                meta,
                alt,
                ..Default::default()
            };
            for (index, (browser, macos)) in [(true, false), (false, true), (false, false)]
                .into_iter()
                .enumerate()
            {
                assert_eq!(
                    primary_shortcut_for_target(modifiers, browser, macos),
                    expected[index],
                    "ctrl={ctrl} meta={meta} alt={alt} browser={browser} macos={macos}"
                );
            }
            assert_eq!(
                is_primary_shortcut(modifiers),
                primary_shortcut_for_target(
                    modifiers,
                    cfg!(target_arch = "wasm32"),
                    cfg!(target_os = "macos")
                )
            );
        }
    }

    #[test]
    fn safari_shortcut_redo_preserves_native_platform_policy() {
        let cases = [
            (false, true, true, "z", [true, true, false]),
            (false, true, true, "Z", [true, true, false]),
            (true, false, false, "y", [true, false, true]),
            (true, false, true, "Y", [true, false, true]),
            (true, false, true, "z", [true, false, true]),
            (false, true, false, "z", [false, false, false]),
            (true, false, false, "z", [false, false, false]),
            (false, true, false, "y", [false, false, false]),
            (false, false, true, "z", [false, false, false]),
            (true, true, true, "z", [true, true, true]),
            (true, true, true, "zz", [false, false, false]),
        ];
        for (ctrl, meta, shift, text, expected) in cases {
            let modifiers = Modifiers {
                ctrl,
                meta,
                shift,
                ..Default::default()
            };
            for (index, (browser, macos)) in [(true, false), (false, true), (false, false)]
                .into_iter()
                .enumerate()
            {
                assert_eq!(
                    redo_shortcut_for_target(modifiers, text, browser, macos),
                    expected[index],
                    "ctrl={ctrl} meta={meta} shift={shift} text={text:?} browser={browser} macos={macos}"
                );
            }
            assert_eq!(
                is_redo_shortcut(modifiers, text),
                redo_shortcut_for_target(
                    modifiers,
                    text,
                    cfg!(target_arch = "wasm32"),
                    cfg!(target_os = "macos")
                )
            );
        }
    }
}
