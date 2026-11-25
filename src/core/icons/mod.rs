//! Built-in icon set; SPEC-HAL-360 owns the SETTINGS addition.
//!
//! Icons are authored using `IconBuilder` geometric primitives.
//! Stroke-style icons use thin filled polygons (~1.5px width).

use crate::core::icon::*;
use crate::core::icon_builder::IconBuilder;

fn def_close() -> IconDefinition {
    IconBuilder::new(24.0)
        .line(5.5, 5.5, 18.5, 18.5, 2.2)
        .line(18.5, 5.5, 5.5, 18.5, 2.2)
        .build(
            IconId::CLOSE,
            "close",
            IconRole::Action,
            IconRenderMode::Stroke,
        )
}

fn def_chevron_right() -> IconDefinition {
    IconBuilder::new(24.0)
        .line(9.0, 6.0, 15.0, 12.0, 1.5)
        .line(15.0, 12.0, 9.0, 18.0, 1.5)
        .build(
            IconId::CHEVRON_RIGHT,
            "chevron-right",
            IconRole::Navigation,
            IconRenderMode::Stroke,
        )
}

fn def_chevron_down() -> IconDefinition {
    IconBuilder::new(24.0)
        .line(6.0, 9.0, 12.0, 15.0, 1.5)
        .line(12.0, 15.0, 18.0, 9.0, 1.5)
        .build(
            IconId::CHEVRON_DOWN,
            "chevron-down",
            IconRole::Navigation,
            IconRenderMode::Stroke,
        )
}

fn def_plus() -> IconDefinition {
    // Original authored as manual rects (vertex order differs from line_segment).
    // rect() preserves the original move_to/line_to sequence for hash parity.
    IconBuilder::new(24.0)
        .rect(5.0, 11.0, 14.0, 2.0)
        .rect(11.0, 5.0, 2.0, 14.0)
        .build(IconId::PLUS, "plus", IconRole::Action, IconRenderMode::Fill)
}

fn def_minus() -> IconDefinition {
    // Original authored as manual rect — rect() preserves vertex order.
    IconBuilder::new(24.0).rect(5.0, 11.0, 14.0, 2.0).build(
        IconId::MINUS,
        "minus",
        IconRole::Action,
        IconRenderMode::Fill,
    )
}

fn def_check() -> IconDefinition {
    IconBuilder::new(24.0)
        .line(6.0, 12.0, 10.0, 17.0, 1.5)
        .line(10.0, 17.0, 18.0, 7.0, 1.5)
        .build(
            IconId::CHECK,
            "check",
            IconRole::Status,
            IconRenderMode::Stroke,
        )
}

fn def_search() -> IconDefinition {
    IconBuilder::new(24.0)
        .ring(10.0, 10.0, 4.25, 5.75)
        .line(14.0, 14.0, 19.0, 19.0, 1.5)
        .build(
            IconId::SEARCH,
            "search",
            IconRole::Action,
            IconRenderMode::Stroke,
        )
}

fn def_arrow_up() -> IconDefinition {
    IconBuilder::new(24.0)
        .line(12.0, 19.0, 12.0, 7.0, 1.5)
        .line(12.0, 5.0, 7.0, 10.0, 1.5)
        .line(12.0, 5.0, 17.0, 10.0, 1.5)
        .build(
            IconId::ARROW_UP,
            "arrow-up",
            IconRole::Navigation,
            IconRenderMode::Stroke,
        )
}

fn def_arrow_down() -> IconDefinition {
    IconBuilder::new(24.0)
        .line(12.0, 5.0, 12.0, 17.0, 1.5)
        .line(12.0, 19.0, 7.0, 14.0, 1.5)
        .line(12.0, 19.0, 17.0, 14.0, 1.5)
        .build(
            IconId::ARROW_DOWN,
            "arrow-down",
            IconRole::Navigation,
            IconRenderMode::Stroke,
        )
}

fn def_warning() -> IconDefinition {
    IconBuilder::new(24.0)
        .polygon(&[(12.0, 4.0), (21.0, 20.0), (3.0, 20.0)])
        .build(
            IconId::WARNING,
            "warning",
            IconRole::Status,
            IconRenderMode::Fill,
        )
}

fn def_folder() -> IconDefinition {
    use crate::core::icon::LayerRole;
    // Classic folder: back panel, front face, tab.
    // Rendered back-to-front (layers[0] painted first).
    IconBuilder::new(24.0)
        // Layer 0: back panel visible above front face (shadow tone)
        .layer(LayerRole::Shadow)
        .polygon(&[(2.0, 4.0), (22.0, 4.0), (22.0, 10.0), (2.0, 10.0)])
        // Layer 1: front face (base tone)
        .layer(LayerRole::Base)
        .polygon(&[(2.0, 8.0), (22.0, 8.0), (22.0, 21.0), (2.0, 21.0)])
        // Layer 2: folder tab (highlight tone)
        .layer(LayerRole::Highlight)
        .polygon(&[(2.0, 3.0), (11.0, 3.0), (13.0, 7.0), (2.0, 7.0)])
        .build(
            IconId::FOLDER,
            "folder",
            IconRole::Structural,
            IconRenderMode::Fill,
        )
}

fn def_file() -> IconDefinition {
    use crate::core::icon::LayerRole;
    // Document page with dog-ear fold in upper-right.
    // Rendered back-to-front.
    IconBuilder::new(24.0)
        // Layer 0: page body with corner cut (base tone)
        .layer(LayerRole::Base)
        .polygon(&[
            (4.0, 2.0),
            (16.0, 2.0),
            (20.0, 6.0),
            (20.0, 22.0),
            (4.0, 22.0),
        ])
        // Layer 1: fold face — the visible folded triangle (highlight tone)
        .layer(LayerRole::Highlight)
        .polygon(&[(16.0, 2.0), (20.0, 6.0), (16.0, 6.0)])
        // Layer 2: fold crease shadow — thin line under the fold (shadow tone)
        .layer(LayerRole::Shadow)
        .line(16.0, 6.0, 20.0, 6.0, 0.6)
        .line(16.0, 2.0, 16.0, 6.0, 0.6)
        .build(
            IconId::FILE,
            "file",
            IconRole::Structural,
            IconRenderMode::Fill,
        )
}

fn def_folder_open() -> IconDefinition {
    use crate::core::icon::LayerRole;
    // Open folder: lid tilted back, front face slightly shorter.
    // Rendered back-to-front (layers[0] painted first).
    IconBuilder::new(24.0)
        // Layer 0: Shadow (back panel / lid, tilted)
        .layer(LayerRole::Shadow)
        .polygon(&[(2.0, 2.0), (22.0, 2.0), (22.0, 8.0), (2.0, 8.0)])
        // Layer 1: Base (front face, shorter — folder is "open")
        .layer(LayerRole::Base)
        .polygon(&[(1.0, 10.0), (23.0, 10.0), (21.0, 21.0), (3.0, 21.0)])
        // Layer 2: Highlight (lid top edge)
        .layer(LayerRole::Highlight)
        .polygon(&[(2.0, 2.0), (11.0, 2.0), (13.0, 5.0), (2.0, 5.0)])
        .build(
            IconId::FOLDER_OPEN,
            "folder-open",
            IconRole::Structural,
            IconRenderMode::Fill,
        )
}

fn def_calendar() -> IconDefinition {
    use crate::core::icon::LayerRole;
    // Calendar: rounded-rect body, top bar, two hang-tabs, three dot grid.
    IconBuilder::new(24.0)
        // Layer 0 (Shadow): calendar body outline/frame
        .layer(LayerRole::Shadow)
        .rounded_rect(3.0, 5.0, 18.0, 16.0, 1.5)
        // Layer 1 (Base): body fill (inner area)
        .layer(LayerRole::Base)
        .rounded_rect(4.0, 6.0, 16.0, 14.0, 1.0)
        // Layer 2 (Highlight): top header bar
        .layer(LayerRole::Highlight)
        .rect(4.0, 6.0, 16.0, 5.0)
        // Layer 3 (Outline): hang tabs (two small rects straddling the top edge)
        .layer(LayerRole::Outline)
        .rect(8.0, 3.0, 2.0, 4.0)
        .rect(14.0, 3.0, 2.0, 4.0)
        // Layer 4 (Shadow): 6 dots in a 3x2 grid representing days
        .layer(LayerRole::Shadow)
        .rect(7.0, 13.0, 2.0, 2.0)
        .rect(11.0, 13.0, 2.0, 2.0)
        .rect(15.0, 13.0, 2.0, 2.0)
        .rect(7.0, 17.0, 2.0, 2.0)
        .rect(11.0, 17.0, 2.0, 2.0)
        .rect(15.0, 17.0, 2.0, 2.0)
        .build(
            IconId::CALENDAR,
            "calendar",
            IconRole::Entity,
            IconRenderMode::Fill,
        )
}

fn def_select_all() -> IconDefinition {
    use crate::core::icon::LayerRole;
    IconBuilder::new(24.0)
        .layer(LayerRole::Shadow)
        .polygon(&[(3.0, 2.0), (13.0, 2.0), (13.0, 12.0), (3.0, 12.0)])
        .layer(LayerRole::Base)
        .polygon(&[(6.0, 6.0), (21.0, 6.0), (21.0, 21.0), (6.0, 21.0)])
        .layer(LayerRole::Highlight)
        .line(9.0, 13.0, 12.0, 17.0, 1.8)
        .line(12.0, 17.0, 19.0, 9.0, 1.8)
        .build(
            IconId::SELECT_ALL,
            "select-all",
            IconRole::Action,
            IconRenderMode::Fill,
        )
}

fn def_clear_all() -> IconDefinition {
    use crate::core::icon::LayerRole;
    IconBuilder::new(24.0)
        .layer(LayerRole::Base)
        .polygon(&[(4.0, 4.0), (20.0, 4.0), (20.0, 20.0), (4.0, 20.0)])
        .layer(LayerRole::Highlight)
        .polygon(&[(5.5, 5.5), (18.5, 5.5), (18.5, 18.5), (5.5, 18.5)])
        .layer(LayerRole::Shadow)
        .line(8.0, 8.0, 16.0, 16.0, 1.5)
        .line(16.0, 8.0, 8.0, 16.0, 1.5)
        .build(
            IconId::CLEAR_ALL,
            "clear-all",
            IconRole::Action,
            IconRenderMode::Fill,
        )
}

fn def_settings() -> IconDefinition {
    // Each quadrant is one simple contour, including its inner opening.
    let mut quadrant = [
        (12.0, 2.0),
        (13.5, 2.0),
        (13.5, 4.5),
        (16.0, 5.5),
        (18.0, 3.5),
        (20.5, 6.0),
        (18.5, 8.0),
        (19.5, 10.5),
        (22.0, 10.5),
        (22.0, 12.0),
        (16.0, 12.0),
        (15.0, 9.0),
        (12.0, 8.0),
    ];
    let mut builder = IconBuilder::new(24.0);
    for _ in 0..4 {
        builder = builder.layer(LayerRole::Base).polygon(&quadrant);
        for (x, y) in &mut quadrant {
            (*x, *y) = (24.0 - *y, *x);
        }
    }
    builder.build(
        IconId::SETTINGS,
        "settings",
        IconRole::Action,
        IconRenderMode::Fill,
    )
}

fn def_globe() -> IconDefinition {
    // Each half-rim is a simple contour supported by the icon fill path.
    // The mirrored meridians meet the rim at (12,3) and (12,21).
    let angle = 9.0_f32.atan2(6.75).to_degrees();
    IconBuilder::new(24.0)
        .arc_stroke(12.0, 12.0, 9.25, -90.0, 90.0, 1.5)
        .layer(LayerRole::Base)
        .arc_stroke(12.0, 12.0, 9.25, 90.0, 270.0, 1.5)
        .layer(LayerRole::Base)
        .arc_stroke(18.75, 12.0, 11.25, 180.0 - angle, 180.0 + angle, 1.5)
        .layer(LayerRole::Base)
        .arc_stroke(5.25, 12.0, 11.25, -angle, angle, 1.5)
        .layer(LayerRole::Base)
        // The two latitude bars share a contour joined inside the left rim.
        .polygon(&[
            (4.0, 7.5),
            (20.0, 7.5),
            (20.0, 9.0),
            (4.0, 9.0),
            (3.5, 11.0),
            (3.5, 13.0),
            (4.0, 15.0),
            (20.0, 15.0),
            (20.0, 16.5),
            (4.0, 16.5),
            (3.0, 14.0),
            (2.5, 12.0),
            (3.0, 10.0),
        ])
        .build(
            IconId::GLOBE,
            "globe",
            IconRole::Status,
            IconRenderMode::Fill,
        )
}

fn def_mail() -> IconDefinition {
    // Keep each layer a simple contour: the current fill path does not carry holes.
    IconBuilder::new(24.0)
        .polygon(&[
            (3.0, 5.0),
            (4.5, 5.0),
            (4.5, 17.5),
            (19.5, 17.5),
            (19.5, 5.0),
            (21.0, 5.0),
            (21.0, 19.0),
            (3.0, 19.0),
        ])
        .layer(LayerRole::Base)
        .rect(4.5, 5.0, 15.0, 1.5)
        .layer(LayerRole::Base)
        .polygon(&[
            (4.5, 6.5),
            (12.0, 12.0),
            (19.5, 6.5),
            (19.5, 8.25),
            (12.0, 13.75),
            (4.5, 8.25),
        ])
        .build(IconId::MAIL, "mail", IconRole::Entity, IconRenderMode::Fill)
}

fn def_briefcase() -> IconDefinition {
    IconBuilder::new(24.0)
        .polygon(&[
            (3.0, 8.0),
            (4.5, 8.0),
            (4.5, 18.5),
            (19.5, 18.5),
            (19.5, 8.0),
            (21.0, 8.0),
            (21.0, 20.0),
            (3.0, 20.0),
        ])
        .layer(LayerRole::Base)
        .rect(4.5, 8.0, 15.0, 1.5)
        .layer(LayerRole::Base)
        .polygon(&[
            (8.0, 8.0),
            (8.0, 5.5),
            (9.5, 4.0),
            (14.5, 4.0),
            (16.0, 5.5),
            (16.0, 8.0),
            (14.5, 8.0),
            (14.5, 6.0),
            (14.0, 5.5),
            (10.0, 5.5),
            (9.5, 6.0),
            (9.5, 8.0),
        ])
        .layer(LayerRole::Base)
        .polygon(&[
            (4.5, 12.0),
            (19.5, 12.0),
            (19.5, 13.5),
            (13.0, 13.5),
            (13.0, 15.0),
            (11.0, 15.0),
            (11.0, 13.5),
            (4.5, 13.5),
        ])
        .build(
            IconId::BRIEFCASE,
            "briefcase",
            IconRole::Entity,
            IconRenderMode::Fill,
        )
}

// ============================================================================
// Registry
// ============================================================================

/// Construct the registry of built-in icons.
/// Called once at startup; the backing slice is leaked for `&'static` lifetime.
pub fn builtin_registry() -> IconRegistry {
    let defs: &'static [IconDefinition] = Box::leak(Box::new([
        def_close(),
        def_chevron_right(),
        def_chevron_down(),
        def_plus(),
        def_minus(),
        def_check(),
        def_search(),
        def_arrow_up(),
        def_arrow_down(),
        def_warning(),
        def_folder(),
        def_file(),
        def_folder_open(),
        def_calendar(),
        def_select_all(),
        def_clear_all(),
        def_settings(),
        def_globe(),
        def_mail(),
        def_briefcase(),
    ]));
    IconRegistry::new(defs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::path::PathCommand;

    #[test]
    fn input_adornment_globe_uses_smooth_meridians_inside_the_rim() {
        let registry = builtin_registry();
        let globe = registry.get(IconId::GLOBE).unwrap();
        assert_eq!(globe.name, "globe");
        assert_eq!((globe.viewbox_width, globe.viewbox_height), (24.0, 24.0));
        assert_eq!(globe.default_mode, IconRenderMode::Fill);
        assert_eq!(globe.layers.len(), 5);
        for layer in &globe.layers[2..4] {
            assert_eq!(
                layer
                    .path
                    .commands
                    .iter()
                    .filter(|c| matches!(c, PathCommand::CubicTo(..)))
                    .count(),
                4
            );
            assert_eq!(
                layer
                    .path
                    .commands
                    .iter()
                    .filter(|c| matches!(c, PathCommand::LineTo(..)))
                    .count(),
                1,
                "one end cap, no angular meridian segments"
            );
            let bounds = layer.path.bounds();
            assert!(
                bounds.x >= 2.0
                    && bounds.y >= 2.0
                    && bounds.right() <= 22.0
                    && bounds.bottom() <= 22.0
            );
        }
    }
}
