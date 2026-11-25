//! HAL-388: static opt-in decoration. Run `cargo run --example local_ambient_glow`.
//! No input handlers, animation, default Button styling or new hit targets.
use halcyon::core::app::{App, AppConfig, Screen, ScreenFrameContext};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::paint::{Color, LocalAmbientGlowAppearance, LocalGlowLobe, RoundedCorners};
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::Label;
use halcyon::utils::id::WidgetInternalId;

struct Demo {
    id: WidgetInternalId,
    appearances: [LocalAmbientGlowAppearance; 3],
}
impl Demo {
    fn new() -> Self {
        let warm = LocalGlowLobe::try_new(
            Point::new(0.5, 1.0),
            Point::new(48.0, 36.0),
            Point::new(1.0, 0.0),
            Color::from_srgb(1.0, 0.55, 0.24, 0.9),
            0.85,
        )
        .unwrap();
        let left = LocalGlowLobe::try_new(
            Point::new(0.3, 1.0),
            warm.radii(),
            Point::new(0.96, 0.28),
            warm.color(),
            0.85,
        )
        .unwrap();
        let right = LocalGlowLobe::try_new(
            Point::new(0.7, 1.0),
            Point::new(48.0, 36.0),
            Point::new(0.96, -0.28),
            Color::from_srgb(0.2, 0.6, 1.0, 0.9),
            0.85,
        )
        .unwrap();
        Self {
            id: WidgetInternalId::new(),
            appearances: [
                LocalAmbientGlowAppearance::try_new(&[]).unwrap(),
                LocalAmbientGlowAppearance::try_new(&[warm]).unwrap(),
                LocalAmbientGlowAppearance::try_new(&[left, right]).unwrap(),
            ],
        }
    }
}
impl WidgetDowncast for Demo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl Widget for Demo {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        false
    }
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        c: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(c.constrain(Size::new(c.max_width, c.max_height)))
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene.push_rect(rect, Color::from_hex(0x080c12), 0, None);
        let title = cx.text_engine.shape("LOCAL AMBIENT GLOW", 24.0);
        cx.text_engine.draw(
            &title,
            rect.x + 40.0,
            rect.y + 48.0,
            Color::from_hex(0xd7e1ec),
            4,
            cx.scene,
        );
        for (index, label) in ["OFF", "ONE LOBE", "TWO LOBES"].iter().enumerate() {
            let center = rect.x + rect.width * (index as f32 + 0.5) / 3.0;
            let face = RectF::new(center - 80.0, rect.y + rect.height * 0.48, 160.0, 48.0);
            cx.scene
                .push_local_ambient_glow(face, self.appearances[index], 1, None)
                .expect("checked example optics");
            cx.scene.push_rounded_rect(
                face,
                Color::from_hex(0x18212c),
                RoundedCorners::uniform(8.0),
                2,
                None,
            );
            let text = cx.text_engine.shape("Action", 17.0);
            cx.text_engine.draw(
                &text,
                center - text.width * 0.5,
                face.y + 15.0,
                Color::from_hex(0xf0f4f8),
                3,
                cx.scene,
            );
            let heading = cx.text_engine.shape(label, 16.0);
            cx.text_engine.draw(
                &heading,
                center - heading.width * 0.5,
                face.y - 66.0,
                Color::from_hex(0xa7b6c8),
                4,
                cx.scene,
            );
        }
        let note = cx.text_engine.shape(
            "Same face and sharp text. Static, local, opt-in light. No default Button changes.",
            14.0,
        );
        cx.text_engine.draw(
            &note,
            rect.x + 40.0,
            rect.y + rect.height - 55.0,
            Color::from_hex(0x8393a6),
            4,
            cx.scene,
        );
    }
}
struct Example {
    placeholder: Label,
}
impl Screen for Example {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        cx.mount_child(Box::new(Demo::new()));
        &mut self.placeholder
    }
    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
}
fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    App::new(AppConfig::new("Local Ambient Glow - HAL388").with_size(1050.0, 440.0)).run(Example {
        placeholder: Label::new(""),
    });
}
