//! HAL387: ordinary native opt-in material comparison, with no animation or input action.
use halcyon::core::app::{App, AppConfig, Screen, ScreenFrameContext};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::paint::{
    AnalyticSurfaceLight, Color, ColorStop, GradientDef, GradientKind, RoundedCorners,
    SmokedGlassParameters, SurfaceAnalytic, SurfaceGradient, SurfaceMicrograin,
};
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::Label;
use halcyon::utils::id::WidgetInternalId;

struct Comparison {
    id: WidgetInternalId,
    faces: [SurfaceAnalytic; 3],
}
impl Comparison {
    fn new() -> Self {
        let base = SurfaceGradient::new(GradientDef {
            kind: GradientKind::Linear,
            p0: (0.0, 0.0),
            p1: (0.0, 1.0),
            stops: vec![
                ColorStop {
                    offset: 0.0,
                    color: Color::from_linear(0.028, 0.040, 0.060, 1.0),
                },
                ColorStop {
                    offset: 0.30,
                    color: Color::from_linear(0.012, 0.020, 0.035, 1.0),
                },
                ColorStop {
                    offset: 1.0,
                    color: Color::from_linear(0.005, 0.010, 0.018, 1.0),
                },
            ],
        })
        .unwrap();
        let lights = [
            AnalyticSurfaceLight::try_new(
                [0.48, -0.64, 0.6],
                Color::from_linear(1.0, 0.62, 0.30, 1.0),
                1.0,
                Point::new(0.35, 0.22),
                Point::new(0.8, 0.6),
                Point::new(1.0, 0.0),
            )
            .unwrap(),
            AnalyticSurfaceLight::try_new(
                [-0.6, 0.0, 0.8],
                Color::from_linear(0.20, 0.55, 1.0, 1.0),
                1.0,
                Point::new(0.8, 0.75),
                Point::new(0.7, 0.8),
                Point::new(1.0, 0.0),
            )
            .unwrap(),
        ];
        let smooth = SurfaceAnalytic::try_smoked_glass(
            base,
            lights,
            SmokedGlassParameters {
                opacity: 1.0,
                bevel_width: 4.0,
                bevel_slope: 0.85,
                roughness: 0.30,
                ambient_strength: 0.05,
                diffuse_strength: 0.8,
                specular_strength: 1.0,
                max_interior_uplift: 0.25,
            },
            None,
        )
        .unwrap();
        Self {
            id: WidgetInternalId::new(),
            faces: [
                smooth.clone(),
                smooth.clone().with_micrograin(Some(
                    SurfaceMicrograin::try_new(1.5, 0.6 / 255.0, 387).unwrap(),
                )),
                smooth.with_micrograin(Some(
                    SurfaceMicrograin::try_new(1.5, 2.0 / 255.0, 387).unwrap(),
                )),
            ],
        }
    }
}
impl WidgetDowncast for Comparison {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl Widget for Comparison {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        false
    }
    fn layout(&mut self, _: &mut LayoutContext, c: Constraints, _: &[NodeId]) -> LayoutResult {
        LayoutResult::fit(c.constrain(Size::new(c.max_width, c.max_height)))
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        cx.scene.push_rect(rect, Color::from_hex(0x0b1018), 0, None);
        let text = |cx: &mut PaintContext, s: &str, x: f32, y: f32, size: f32| {
            let run = cx.text_engine.shape(s, size);
            cx.text_engine
                .draw(&run, x, y, Color::from_hex(0xd7e1ed), 30, cx.scene);
        };
        text(
            cx,
            "FILTERED MATERIAL MICROGRAIN / HAL387",
            rect.x + 32.0,
            rect.y + 30.0,
            22.0,
        );
        text(
            cx,
            "Stationary material-local texture; identical lighting, opacity and geometry",
            rect.x + 32.0,
            rect.y + 70.0,
            15.0,
        );
        let width = (rect.width - 96.0) / 3.0;
        for (i, title) in ["OFF", "RESTRAINED", "MAXIMUM"].into_iter().enumerate() {
            let face = RectF::new(
                rect.x + 24.0 + i as f32 * (width + 24.0),
                rect.y + 145.0,
                width,
                rect.height - 245.0,
            );
            if face.width < 32.0 || face.height < 32.0 {
                continue;
            }
            text(cx, title, face.x, face.y - 34.0, 15.0);
            cx.scene
                .push_analytic_surface(
                    face,
                    RoundedCorners::uniform(16.0),
                    &self.faces[i],
                    10,
                    None,
                )
                .expect("checked comparison face");
            text(cx, "Sharp native text", face.x + 20.0, face.y + 38.0, 18.0);
            text(
                cx,
                [
                    "No grain",
                    "0.6 / 255 reference amplitude",
                    "2 / 255 reference amplitude",
                ][i],
                face.x + 20.0,
                face.y + 75.0,
                13.0,
            );
        }
        text(
            cx,
            "Scale 1.5 target pixels / seed 387 / no rim / no animation",
            rect.x + 32.0,
            rect.y + rect.height - 55.0,
            15.0,
        );
    }
}
struct Example {
    placeholder: Label,
}
impl Screen for Example {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        cx.mount_child(Box::new(Comparison::new()));
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
    App::new(AppConfig::new("Filtered Material Micrograin - HAL387").with_size(1080.0, 590.0)).run(
        Example {
            placeholder: Label::new(""),
        },
    );
}
