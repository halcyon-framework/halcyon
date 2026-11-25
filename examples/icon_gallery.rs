//! SPEC-HAL-265: Icon Gallery Demo
//!
//! Displays all 20 registered built-in icons at S (12), L (16), and XL (20)
//! for 60 samples in a 760×460 logical-pixel window.
//!
//! Run with: cargo run --example icon_gallery

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::icon::{IconId, IconRegistry, IconSize};
use halcyon::core::icons::builtin_registry;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{Color, LAYER_CONTENT};
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::{Event, EventPhase, Label};
use halcyon::utils::id::WidgetInternalId;

const WINDOW_SIZE: Size = Size::new(760.0, 460.0);

fn main() {
    println!("=== SPEC-HAL-265: Icon Gallery ===");
    println!(
        "Displays all {} registered built-in icons at S/L/XL sizes ({} samples).",
        ICON_IDS.len(),
        ICON_IDS.len() * SIZES.len(),
    );
    println!("Close the window to exit.");
    println!("=================================\n");

    let app = App::new(
        AppConfig::new("Icon Gallery (SPEC-HAL-265)")
            .with_size(WINDOW_SIZE.width, WINDOW_SIZE.height),
    );
    app.run(IconGalleryScreen::new());
}

// ============================================================================
// Screen
// ============================================================================

struct IconGalleryScreen {
    placeholder: Label,
}

impl IconGalleryScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
        }
    }
}

impl Screen for IconGalleryScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        cx.mount_child(Box::new(IconGalleryWidget::new()));
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {}

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
}

// ============================================================================
// Widget
// ============================================================================

const ICON_IDS: [IconId; 20] = [
    IconId::CLOSE,
    IconId::CHEVRON_RIGHT,
    IconId::CHEVRON_DOWN,
    IconId::PLUS,
    IconId::MINUS,
    IconId::CHECK,
    IconId::SEARCH,
    IconId::ARROW_UP,
    IconId::ARROW_DOWN,
    IconId::WARNING,
    IconId::FOLDER,
    IconId::FILE,
    IconId::FOLDER_OPEN,
    IconId::CALENDAR,
    IconId::SELECT_ALL,
    IconId::CLEAR_ALL,
    IconId::SETTINGS,
    IconId::GLOBE,
    IconId::MAIL,
    IconId::BRIEFCASE,
];

const SIZES: [IconSize; 3] = [IconSize::S, IconSize::L, IconSize::XL];
const COLUMNS: usize = 10;
const ROWS_PER_SIZE: usize = ICON_IDS.len().div_ceil(COLUMNS);

fn icon_rect(rect: RectF, size_index: usize, icon_index: usize) -> RectF {
    let column = icon_index % COLUMNS;
    let row = size_index * ROWS_PER_SIZE + icon_index / COLUMNS;
    let size = SIZES[size_index].logical_px();
    RectF::new(
        rect.x + 30.0 + column as f32 * 70.0,
        rect.y + 50.0 + row as f32 * 63.0,
        size,
        size,
    )
}

struct IconGalleryWidget {
    id: WidgetInternalId,
    registry: IconRegistry,
}

impl IconGalleryWidget {
    fn new() -> Self {
        let registry = builtin_registry();
        assert_eq!(
            registry.len(),
            ICON_IDS.len(),
            "Icon gallery catalog is incomplete",
        );
        Self {
            id: WidgetInternalId::new(),
            registry,
        }
    }
}

impl WidgetDowncast for IconGalleryWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for IconGalleryWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let color = Color::from_srgb(0.15, 0.15, 0.15, 1.0);
        for size_index in 0..SIZES.len() {
            for (icon_index, &id) in ICON_IDS.iter().enumerate() {
                let def = self
                    .registry
                    .get(id)
                    .unwrap_or_else(|| panic!("Built-in icon {id:?} is missing from the registry"));
                let dest = icon_rect(rect, size_index, icon_index);
                cx.scene
                    .push_icon(def, dest.x, dest.y, dest.width, color, LAYER_CONTENT);
            }
        }
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "IconGalleryWidget"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn gallery_lists_each_registered_builtin_in_order() {
        let registry = builtin_registry();
        assert_eq!(registry.len(), 20);
        assert!(ICON_IDS.into_iter().map(|id| id.0).eq(1..=20));
        for id in ICON_IDS {
            let definition = registry.get(id).expect("gallery icon must be registered");
            assert_eq!(definition.id, id);
            assert!(!definition.layers.is_empty(), "icon {id:?} has no geometry");
        }
    }

    #[test]
    fn gallery_fits_all_60_icon_samples_at_the_allocated_origin() {
        let viewport = RectF::new(11.0, 23.0, WINDOW_SIZE.width, WINDOW_SIZE.height);
        let mut origins = HashSet::new();
        for (size_index, expected_size) in [12.0, 16.0, 20.0].into_iter().enumerate() {
            for icon_index in 0..ICON_IDS.len() {
                let dest = icon_rect(viewport, size_index, icon_index);
                assert_eq!((dest.width, dest.height), (expected_size, expected_size));
                assert!(dest.x >= viewport.x && dest.right() <= viewport.right());
                assert!(dest.y >= viewport.y && dest.bottom() <= viewport.bottom());
                assert!(origins.insert((dest.x.to_bits(), dest.y.to_bits())));
            }
        }
        assert_eq!(origins.len(), 60);
        assert_eq!(
            icon_rect(viewport, 0, 0),
            RectF::new(41.0, 73.0, 12.0, 12.0),
        );
        assert_eq!(
            icon_rect(viewport, 2, 19),
            RectF::new(671.0, 388.0, 20.0, 20.0),
        );
    }
}
