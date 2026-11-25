// SPEC-HAL-225: TriggerChrome — Shared Trigger Painting Primitive
// Pure visual helper for button-like trigger controls.
// Stateless. No caching. No allocations. No behavior.
//
// INV-225-A1: No internal state.
// INV-225-A2: No caching between calls.
// INV-225-A3: No allocations per paint call.
// INV-225-A4: All geometry derived from inputs only.
// INV-225-A5: Deterministic output.
// INV-225-A6: Zero references to ComboBox, DropdownMenu, MenuSurface, MenuItem.

use crate::core::math::RectF;
use crate::core::paint::LAYER_CONTENT;
use crate::core::widget::PaintContext;

const LAYER_FOCUS: u32 = 5;

/// Visual flags for trigger chrome rendering.
/// All flags are explicit inputs — never inferred from widget state.
#[derive(Copy, Clone, Debug)]
pub struct TriggerChromeFlags {
    pub enabled: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub open: bool,
}

/// Adornment configuration for trigger chrome.
#[derive(Copy, Clone, Debug)]
pub struct TriggerChromeAdornments {
    pub caret: bool,
}

/// Paint a trigger chrome control surface.
/// Pure visual — no state, no caching, no behavior.
pub fn paint_trigger_chrome(
    cx: &mut PaintContext,
    rect: RectF,
    label: &str,
    flags: &TriggerChromeFlags,
    adornments: &TriggerChromeAdornments,
) {
    let font_size = cx.theme.typography.font_size;
    let padding_x = cx.theme.list_geom.padding_x;
    let list_colors = &cx.theme.colors.list;
    let content_color = if flags.enabled {
        list_colors.text
    } else {
        cx.theme.colors.typography.muted
    };

    let background_color = if flags.enabled && flags.hovered {
        list_colors.row_hover
    } else {
        list_colors.row_background
    };

    // Background
    cx.scene
        .push_rect(rect, background_color, LAYER_CONTENT, None);

    // Border
    cx.scene.push_border_rect(
        rect,
        cx.theme.colors.borders.default,
        1.0,
        LAYER_CONTENT,
        None,
    );

    // Label text
    let label_run = cx.text_engine.shape(label, font_size);
    let label_x = rect.x + padding_x;
    let label_y = rect.y + (rect.height - label_run.height) / 2.0;
    cx.scene.push_text_with_backgrounds(
        &label_run,
        label_x,
        label_y,
        content_color,
        vec![],
        LAYER_CONTENT + 1,
        None,
    );

    // Caret adornment
    if adornments.caret {
        let caret_run = cx.text_engine.shape("\u{25BE}", font_size);
        let caret_x = rect.x + rect.width - padding_x - caret_run.width;
        let caret_y = rect.y + (rect.height - caret_run.height) / 2.0;
        cx.scene.push_text_with_backgrounds(
            &caret_run,
            caret_x,
            caret_y,
            content_color,
            vec![],
            LAYER_CONTENT + 1,
            None,
        );
    }

    // Focus ring
    if flags.focused {
        let (focus_color, focus_width) = cx.theme.control_border(true, false, false);
        cx.scene
            .push_border_rect(rect, focus_color, focus_width, LAYER_FOCUS, None);
    }
}
