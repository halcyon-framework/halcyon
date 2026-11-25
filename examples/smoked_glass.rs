//! HAL386/389 ordinary native comparison. Click to switch the real backdrop A/B.
//! Fixture values only: no default control/theme changes, animation or backdrop sampling.
use halcyon::core::app::{App, AppConfig, Screen, ScreenFrameContext};
use halcyon::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::paint::{
    AnalyticSurfaceLight, Color, ColorStop, GradientDef, GradientKind, RoundedCorners,
    SmokedGlassParameters, SurfaceAnalytic, SurfaceGradient, SurfaceRimLight,
};
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::Label;
use halcyon::utils::id::WidgetInternalId;

struct Comparison {
    id: WidgetInternalId,
    backdrop_b: bool,
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
        let p = SmokedGlassParameters {
            opacity: 1.0,
            bevel_width: 4.0,
            bevel_slope: 0.85,
            roughness: 0.30,
            ambient_strength: 0.05,
            diffuse_strength: 0.8,
            specular_strength: 1.0,
            max_interior_uplift: 0.25,
        };
        let rim =
            Some(SurfaceRimLight::try_new(Color::from_linear(0.78, 0.86, 1.0, 1.0), 0.2).unwrap());
        Self {
            id: WidgetInternalId::new(),
            backdrop_b: false,
            faces: [
                SurfaceAnalytic::try_smoked_glass(base.clone(), lights, p, None).unwrap(),
                SurfaceAnalytic::try_smoked_glass(base.clone(), lights, p, rim).unwrap(),
                SurfaceAnalytic::try_smoked_glass(
                    base,
                    lights,
                    SmokedGlassParameters { opacity: 0.5, ..p },
                    rim,
                )
                .unwrap(),
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
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        c: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(c.constrain(Size::new(c.max_width, c.max_height)))
    }
    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase == EventPhase::Target
            && matches!(event,Event::Pointer(p) if p.kind==PointerEventKind::Up && p.button==Some(PointerButton::Primary))
        {
            self.backdrop_b = !self.backdrop_b;
            cx.invalidate_paint();
            return true;
        }
        false
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene.push_rect(rect, Color::from_hex(0x0b1018), 0, None);
        let draw_text = |cx: &mut PaintContext, text: &str, x: f32, y: f32, size: f32| {
            let run = cx.text_engine.shape(text, size);
            cx.text_engine
                .draw(&run, x, y, Color::from_hex(0xd7e1ed), 30, cx.scene);
        };
        draw_text(
            cx,
            "SMOKED GLASS / INDEPENDENT REFLECTIVE RIM",
            rect.x + 32.0,
            rect.y + 32.0,
            22.0,
        );
        let width = (rect.width - 96.0) / 3.0;
        for (i, title) in ["PLAIN GRADIENT", "LIT FACE / RIM OFF", "SAME FACE / RIM ON"]
            .iter()
            .enumerate()
        {
            let face = RectF::new(
                rect.x + 24.0 + i as f32 * (width + 24.0),
                rect.y + 130.0,
                width,
                150.0,
            );
            draw_text(cx, title, face.x, face.y - 36.0, 14.0);
            if i == 0 {
                cx.scene.push_gradient_rect(
                    face,
                    16.0,
                    self.faces[0].base().definition(),
                    10,
                    None,
                );
            } else {
                cx.scene
                    .push_analytic_surface(
                        face,
                        RoundedCorners::uniform(16.0),
                        &self.faces[i - 1],
                        10,
                        None,
                    )
                    .expect("admitted fixture face");
            }
            draw_text(cx, "Sharp native text", face.x + 20.0, face.y + 68.0, 17.0);
        }
        let backing = RectF::new(rect.x + 24.0, rect.y + 340.0, rect.width - 48.0, 150.0);
        let (a, b) = if self.backdrop_b {
            (0x59482e, 0x153b52)
        } else {
            (0x173849, 0x423849)
        };
        for i in 0..8 {
            cx.scene.push_rect(
                RectF::new(
                    backing.x + backing.width * i as f32 / 8.0,
                    backing.y,
                    backing.width / 8.0,
                    backing.height,
                ),
                Color::from_hex(if i % 2 == 0 { a } else { b }),
                5,
                None,
            );
        }
        cx.scene
            .push_analytic_surface(
                backing,
                RoundedCorners {
                    tl: 24.0,
                    tr: 8.0,
                    bl: 18.0,
                    br: 4.0,
                },
                &self.faces[2],
                10,
                None,
            )
            .expect("admitted translucent fixture");
        draw_text(
            cx,
            "OPACITY 0.5 / ordinary OVER, no backdrop read",
            backing.x + 20.0,
            backing.y + 60.0,
            19.0,
        );
        draw_text(
            cx,
            if self.backdrop_b {
                "Backdrop B — click to switch to A"
            } else {
                "Backdrop A — click to switch to B"
            },
            rect.x + 32.0,
            rect.y + rect.height - 54.0,
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
    App::new(
        AppConfig::new("Smoked Glass and Reflective Rim - HAL386/389").with_size(1080.0, 590.0),
    )
    .run(Example {
        placeholder: Label::new(""),
    });
}
