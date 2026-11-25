use halcyon::core::paint::Color;
use halcyon::core::theme::{
    ControlChromeCategory, ControlChromeState, ControlValidationState, Theme,
};

#[test]
fn theme_capture_color_bordered_input_distinguishes_keyboard_ownership() {
    for theme in [Theme::light(), Theme::dark()] {
        let focused = theme.control_border(true, false, false);
        let captured = theme.control_border(true, true, false);
        assert_ne!(
            focused.0, captured.0,
            "keyboard Capture must not resolve to the focus-only border color"
        );
        assert_eq!(captured.0, theme.focus_ring.capture_color);
        assert_eq!(focused.0, theme.colors.borders.focus);
        assert_eq!(focused.1, theme.focus_ring.focused_width);
        assert_eq!(captured.1, theme.focus_ring.captured_width);
        assert_eq!(
            theme.control_border(false, false, false),
            (theme.colors.borders.default, 1.0)
        );
    }
}

#[test]
fn theme_capture_color_editable_chrome_distinguishes_keyboard_ownership() {
    for theme in [Theme::light(), Theme::dark()] {
        let focused = theme.control_chrome(
            ControlChromeCategory::RecessedEditable,
            ControlChromeState::Focused,
            ControlValidationState::Neutral,
        );
        let captured = theme.control_chrome(
            ControlChromeCategory::RecessedEditable,
            ControlChromeState::Captured,
            ControlValidationState::Neutral,
        );
        assert_ne!(
            focused.border_color, captured.border_color,
            "editable input Capture must remain distinct even with equal border widths"
        );
        assert_eq!(captured.border_color, theme.focus_ring.capture_color);
        assert_eq!(captured.focus_color, theme.focus_ring.capture_color);
        assert_eq!(focused.border_color, theme.colors.borders.focus);
        assert_eq!(focused.focus_color, theme.colors.borders.focus);
        assert_eq!(captured.border_width, theme.focus_ring.captured_width);
        assert_eq!(captured.focus_width, theme.focus_ring.captured_width);
        assert_eq!(captured.fill, focused.fill);
        assert_eq!(captured.radius, focused.radius);
        assert_eq!(captured.content_color, focused.content_color);
        assert_eq!(captured.focus_style, focused.focus_style);
    }
}

#[test]
fn theme_capture_color_preserves_configuration_and_validation_precedence() {
    let mut theme = Theme::light();
    theme.focus_ring.capture_color = Color::from_hex(0x984CB8);
    theme.focus_ring.focused_width = 1.5;
    theme.focus_ring.captured_width = 3.0;
    assert_eq!(
        theme.control_border(true, true, false),
        (theme.focus_ring.capture_color, 3.0)
    );
    assert_eq!(
        theme.control_border(true, false, false),
        (theme.colors.borders.focus, 1.5)
    );
    assert_eq!(
        theme.control_border(true, true, true),
        (theme.colors.semantic.error, 1.0)
    );
    for (validation, border) in [
        (
            ControlValidationState::Neutral,
            theme.focus_ring.capture_color,
        ),
        (ControlValidationState::Error, theme.colors.semantic.error),
        (
            ControlValidationState::Warning,
            theme.colors.semantic.warning,
        ),
        (
            ControlValidationState::Success,
            theme.colors.semantic.success,
        ),
    ] {
        let captured = theme.control_chrome(
            ControlChromeCategory::RecessedEditable,
            ControlChromeState::Captured,
            validation,
        );
        assert_eq!(captured.border_color, border);
        assert_eq!(captured.focus_color, theme.focus_ring.capture_color);
        assert_eq!(captured.border_width, 3.0);
        assert_eq!(captured.focus_width, 3.0);
    }
}

#[test]
fn theme_capture_color_does_not_recolor_selector_press_or_track_cues() {
    for theme in [Theme::light(), Theme::dark()] {
        // RadioButton uses BinarySelector/Captured for an armed press.
        for category in [
            ControlChromeCategory::BinarySelector,
            ControlChromeCategory::TrackRange,
        ] {
            let chrome = theme.control_chrome(
                category,
                ControlChromeState::Captured,
                ControlValidationState::Neutral,
            );
            assert_eq!(chrome.border_color, theme.colors.borders.focus);
            assert_eq!(chrome.focus_color, theme.colors.borders.focus);
        }
    }
}
