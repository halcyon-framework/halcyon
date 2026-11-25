//! Same-surface scene Z order (SPEC-HAL-359).
//!
//! Three overlapping rectangles share the App's base surface. Red has Z=0,
//! green Z=5, and blue Z=10.
//!
//! The console also prints an isolated compositor descriptor snapshot.
//! The App owns a separate live surface stack. SPEC-HAL-081A INV-COMP-P01
//! governs ordering between surfaces. INV-COMP-P04 governs output handoff.
//!
//! # Run the example
//! ```
//! cargo run --release --example compositor_surface_order
//! ```

use halcyon::compositor::{Compositor, SceneKey};
use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{Color, PaintSink};
use halcyon::core::scene_identity::SceneNodeKey;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::Label;
use halcyon::utils::id::WidgetInternalId;

fn main() {
    println!("=== Same-surface Scene Z order (HAL-359) ===");
    println!();
    println!("Red (Z=0), green (Z=5), and blue (Z=10) overlap on the App's base surface.");
    println!();
    println!("INV-COMP-P01 orders compositor surfaces by Z.");
    println!("INV-COMP-P04 hands composed output to RenderGraph or the swapchain.");
    println!("================================================");

    let app = App::new(AppConfig::new("Same-surface Scene Z Order").with_size(700.0, 500.0));
    app.run(SurfaceOrderDemoScreen::new());
}

struct SurfaceOrderDemoScreen {
    placeholder: Label,
}

impl SurfaceOrderDemoScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
        }
    }
}

impl Screen for SurfaceOrderDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        cx.mount_child(Box::new(SurfaceOrderDemoWidget::new()));
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }
}

/// Demo widget showing primitive Z order on the App's base surface.
struct SurfaceOrderDemoWidget {
    id: WidgetInternalId,
}

impl SurfaceOrderDemoWidget {
    fn new() -> Self {
        let viewport = RectF::new(0.0, 0.0, 700.0, 500.0);
        let mut compositor = Compositor::new(viewport);

        let base_surface = compositor.create_surface(0, viewport, SceneKey::BASE);

        println!();
        println!("Standalone compositor descriptor snapshot");
        println!("The App owns a separate live surface stack.");
        println!("  Base surface: {base_surface:?}");
        println!("  Viewport: {viewport:?}");
        println!();

        let snapshot = compositor.snapshot();
        println!("  Surface descriptors (lowest Z first per INV-COMP-P01)");
        for (i, surface) in snapshot.iter().enumerate() {
            println!(
                "  [{}] id={}, z={}, scene_key={}, visible={}",
                i,
                surface.id.raw(),
                surface.z_index,
                surface.scene_key,
                surface.is_visible()
            );
        }
        println!();

        Self {
            id: WidgetInternalId::new(),
        }
    }
}

fn push_demo_rectangles(scene: &mut dyn PaintSink, scene_key: Option<SceneNodeKey>) {
    scene.push_rect(
        RectF::new(50.0, 100.0, 250.0, 250.0),
        Color::from_srgb(0.9, 0.2, 0.2, 1.0),
        0,
        scene_key,
    );
    scene.push_rect(
        RectF::new(150.0, 150.0, 250.0, 250.0),
        Color::from_srgb(0.2, 0.8, 0.2, 1.0),
        5,
        scene_key,
    );
    scene.push_rect(
        RectF::new(250.0, 200.0, 250.0, 250.0),
        Color::from_srgb(0.2, 0.4, 0.9, 1.0),
        10,
        scene_key,
    );
}

impl WidgetDowncast for SurfaceOrderDemoWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SurfaceOrderDemoWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(700.0, 500.0))
    }

    fn paint(&self, cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {
        // Dark background
        let bg_rect = RectF::new(0.0, 0.0, 700.0, 500.0);
        let bg_color = Color::from_srgb(0.1, 0.1, 0.1, 1.0);
        cx.scene.push_rect(bg_rect, bg_color, 0, cx.scene_key);

        push_demo_rectangles(cx.scene, cx.scene_key);

        // Info text
        cx.scene.push_text_probe(
            (20.0, 30.0),
            "Same-surface Scene Z order (HAL-359)",
            16.0,
            Color::WHITE,
            100,
            cx.scene_key,
        );

        cx.scene.push_text_probe(
            (20.0, 55.0),
            "Three rectangles on the App's base surface",
            12.0,
            Color::from_srgb(0.7, 0.7, 0.7, 1.0),
            100,
            cx.scene_key,
        );

        // GroupBox
        cx.scene.push_text_probe(
            (550.0, 100.0),
            "Primitive ZIndex",
            14.0,
            Color::WHITE,
            100,
            cx.scene_key,
        );
        cx.scene.push_text_probe(
            (550.0, 125.0),
            "Red = z:0",
            12.0,
            Color::from_srgb(0.9, 0.2, 0.2, 1.0),
            100,
            cx.scene_key,
        );
        cx.scene.push_text_probe(
            (550.0, 145.0),
            "Green = z:5",
            12.0,
            Color::from_srgb(0.2, 0.8, 0.2, 1.0),
            100,
            cx.scene_key,
        );
        cx.scene.push_text_probe(
            (550.0, 165.0),
            "Blue = z:10",
            12.0,
            Color::from_srgb(0.2, 0.4, 0.9, 1.0),
            100,
            cx.scene_key,
        );

        // Explanation
        cx.scene.push_text_probe(
            (20.0, 470.0),
            "Higher primitive ZIndex draws above lower ZIndex on this surface.",
            12.0,
            Color::from_srgb(0.5, 0.5, 0.5, 1.0),
            100,
            cx.scene_key,
        );
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halcyon::core::scene::{Color as SceneColor, SceneBuilder, SceneNode};

    #[test]
    fn overlapping_rectangles_keep_their_scene_z_values() {
        let mut builder = SceneBuilder::new();
        // The widget uses PaintContext's default ephemeral key.
        let scene_key = None;
        push_demo_rectangles(&mut builder, scene_key);
        let scene = builder.build();
        let [SceneNode::Rect(red), SceneNode::Rect(green), SceneNode::Rect(blue)] =
            scene.root.as_slice()
        else {
            panic!("Expected exactly three rectangles in one scene");
        };

        let expected = [
            (
                RectF::new(50.0, 100.0, 250.0, 250.0),
                Color::from_srgb(0.9, 0.2, 0.2, 1.0),
                0,
            ),
            (
                RectF::new(150.0, 150.0, 250.0, 250.0),
                Color::from_srgb(0.2, 0.8, 0.2, 1.0),
                5,
            ),
            (
                RectF::new(250.0, 200.0, 250.0, 250.0),
                Color::from_srgb(0.2, 0.4, 0.9, 1.0),
                10,
            ),
        ];
        for (node, (rect, color, z)) in [red, green, blue].into_iter().zip(expected) {
            assert_eq!(node.rect, rect);
            assert_eq!(node.color, SceneColor::from(color));
            assert_eq!(node.z_index, z);
            assert_eq!(node.key, scene_key);
        }
    }

    fn push_single_surface_rectangles(scene: &mut dyn PaintSink, scene_key: Option<SceneNodeKey>) {
        scene.push_rect(
            RectF::new(50.0, 50.0, 200.0, 200.0),
            Color::from_srgb(0.2, 0.4, 0.8, 1.0),
            1,
            scene_key,
        );
        scene.push_rect(
            RectF::new(150.0, 150.0, 200.0, 200.0),
            Color::from_srgb(0.8, 0.2, 0.2, 1.0),
            2,
            scene_key,
        );
    }

    #[test]
    fn blue_and_red_rectangles_keep_their_scene_z_values() {
        let mut builder = SceneBuilder::new();
        // The widget uses PaintContext's default ephemeral key.
        let scene_key = None;
        push_single_surface_rectangles(&mut builder, scene_key);
        let scene = builder.build();
        let [SceneNode::Rect(blue), SceneNode::Rect(red)] = scene.root.as_slice() else {
            panic!("Expected exactly two rectangles in one scene");
        };

        assert_eq!(blue.rect, RectF::new(50.0, 50.0, 200.0, 200.0));
        assert_eq!(blue.color, SceneColor::from(Color::from_srgb(0.2, 0.4, 0.8, 1.0)));
        assert_eq!(blue.z_index, 1);
        assert_eq!(blue.key, scene_key);

        assert_eq!(red.rect, RectF::new(150.0, 150.0, 200.0, 200.0));
        assert_eq!(red.color, SceneColor::from(Color::from_srgb(0.8, 0.2, 0.2, 1.0)));
        assert_eq!(red.z_index, 2);
        assert_eq!(red.key, scene_key);
    }
}
