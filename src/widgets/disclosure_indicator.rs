// SPEC-HAL-224: DisclosureIndicator — Passive Visual Primitive
// Renders an expand/collapse indicator reflecting externally provided state.
// Strictly visual. No state ownership, no interaction, no layout authority.
//
// INV-A1: No state ownership — expanded flag is an explicit input.
// INV-A2: No control authority — caller owns all semantics.
// INV-B1: Non-interactive — no pointer, keyboard, or focus handling.
// INV-B2: No implicit actions — paint output only.
// INV-C1: Deterministic visual mapping — identical inputs produce identical output.
// INV-C2: Theme-driven — color and size resolved from PaintContext theme.
// INV-C3: No animation state — visual is immediate from inputs.
// INV-D1: Always embedded — no standalone widget, no layout authority.
// INV-D2: Caller provides bounding rect — no intrinsic size computation.
// INV-E1: Deterministic — pure function of (rect, expanded, layer, theme).
// INV-E2: Zero allocation steady state — no stored allocations across frames.
// INV-E3: Invariant supremacy — these constraints override convenience.
//
// No references to MenuSurface, MenuItem, TreeView, ComboBox, DropdownMenu.

use crate::core::math::{RectF, snap_to_physical};
use crate::core::paint::RoundedCorners;
use crate::core::widget::PaintContext;

const BAR_LENGTH: f32 = 7.0;
const BAR_THICKNESS: f32 = 1.5;
const BAR_GAP: f32 = 3.0;
const BAR_RADIUS: f32 = 0.5;

/// Paint a disclosure indicator centered within the given rect.
///
/// - `expanded = false`: two vertical bars (collapsed).
/// - `expanded = true`: one centered vertical bar (expanded).
///
/// The indicator uses the same one/two-bar affordance language as dropdown controls,
/// rotated for section disclosure. Caller controls positioning via rect.
/// Color is `theme.colors.icons.primary`. Layer is caller-specified.
pub fn paint_disclosure_indicator(cx: &mut PaintContext, rect: RectF, expanded: bool, layer: u32) {
    let color = cx.theme.colors.icons.primary;
    let available = rect.width.min(rect.height).max(0.0);
    if available <= 0.0 {
        return;
    }

    let sf = cx.scene.scale_factor();
    let bar_width = BAR_THICKNESS.min(available);
    let bar_height = BAR_LENGTH.min(available);
    let cx0 = snap_to_physical(rect.x + rect.width * 0.5, sf);
    let cy0 = snap_to_physical(rect.y + rect.height * 0.5, sf);
    let radius = RoundedCorners::uniform(BAR_RADIUS);

    let paint_bar = |cx: &mut PaintContext, x: f32, y: f32| {
        let bar_rect = RectF::new(x, y, bar_width, bar_height).snap_to_physical_pixels(sf);
        cx.scene
            .push_rounded_rect(bar_rect, color, radius, layer, None);
    };

    if expanded {
        paint_bar(cx, cx0 - bar_width * 0.5, cy0 - bar_height * 0.5);
    } else {
        let total_width = bar_width * 2.0 + BAR_GAP;
        let start_x = cx0 - total_width * 0.5;
        let y = cy0 - bar_height * 0.5;
        paint_bar(cx, start_x, y);
        paint_bar(cx, start_x + bar_width + BAR_GAP, y);
    }
}
