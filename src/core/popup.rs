//! Shared popup placement utilities.
//! Used by DatePickerField, ColorPickerField, ComboBox,
//! ContextMenu, Tooltip, and any future popup widget.

use crate::core::math::RectF;

/// Width negotiation shared by popup-hosting widgets. It is independent of
/// application keyboard authority and of the popup's visual appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupWidthPolicy {
    /// Keep the owner's preferred content width, at least as wide as the trigger.
    #[default]
    Content,
    /// Use the exact allocated trigger width, including its existing text clip.
    MatchTrigger,
}

/// Construction-time popup interaction/geometry rules. The widget still owns
/// its anchor, viewport placement, input routing and resolver transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PopupInteractionPolicy {
    pub width: PopupWidthPolicy,
}

impl PopupInteractionPolicy {
    pub fn resolve_width(self, trigger_width: f32, preferred_content_width: f32) -> f32 {
        match self.width {
            PopupWidthPolicy::Content => trigger_width.max(preferred_content_width),
            PopupWidthPolicy::MatchTrigger => trigger_width,
        }
    }
}

/// Compute the best popup position given an anchor rect,
/// popup size, and available viewport bounds.
///
/// The popup tries to appear below-right of the anchor.
/// If it doesn't fit, it shifts:
/// - Vertically: above the anchor if no room below
/// - Horizontally: left-aligned to fit within viewport
///
/// Returns: (popup_rect, opened_above)
pub fn compute_popup_rect(
    anchor: RectF,
    popup_w: f32,
    popup_h: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> (RectF, bool) {
    // Vertical: prefer below, fall back to above
    let below_y = anchor.y + anchor.height;
    let above_y = anchor.y - popup_h;
    let fits_below = below_y + popup_h <= viewport_h;
    let fits_above = above_y >= 0.0;

    let (y, opened_above) = if fits_below {
        (below_y, false)
    } else if fits_above {
        (above_y, true)
    } else {
        // Neither fits — pick whichever has more space
        if (viewport_h - below_y) >= anchor.y {
            (below_y, false)
        } else {
            (above_y, true)
        }
    };

    // Horizontal: prefer left-aligned with anchor, shift if clips right edge
    let mut x = anchor.x;
    if x + popup_w > viewport_w {
        // Shift left so popup's right edge aligns with viewport right
        x = (viewport_w - popup_w).max(0.0);
    }
    if x < 0.0 {
        x = 0.0;
    }

    (RectF::new(x, y, popup_w, popup_h), opened_above)
}
