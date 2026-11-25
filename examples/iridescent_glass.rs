//! Native thin-film comparison. Swap angles to exchange the two film-lit samples.

use halcyon::core::{
    app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext},
    appearance::ButtonMaterialPolicy,
    layout::{Constraints, LayoutResult},
    math::{Point, RectF, Size},
    paint::{
        AnalyticSurfaceLight, Color, ColorStop, GlassSurfaceOptics, GlassSurfaceOpticsParameters,
        GradientDef, GradientKind, RoundedCorners, SmokedGlassParameters, SurfaceAnalytic,
        SurfaceGradient, ThinFilmParams,
    },
    widget::{LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast},
    widget_tree::{NodeId, WidgetTree},
};
use halcyon::{
    model::form_binder::FormBinder,
    prelude::{Button, Label},
    utils::id::{TreeWidgetId, WidgetInternalId},
};
use std::any::Any;

const WINDOW_SIZE: Size = Size::new(1100.0, 720.0);
const MARGIN: f32 = 54.0;

fn specimens() -> [Specimen; 3] {
    let neutral = Color::from_linear(0.08, 0.08, 0.08, 1.0);
    let gradient = SurfaceGradient::new(GradientDef {
        kind: GradientKind::Linear,
        p0: (0.0, 0.0),
        p1: (1.0, 0.0),
        stops: [0.0, 0.30, 1.0]
            .map(|offset| ColorStop {
                offset,
                color: neutral,
            })
            .into(),
    })
    .expect("neutral glass body");
    let incident = |elevation: f64, strength| {
        let (z, x) = elevation.to_radians().sin_cos();
        AnalyticSurfaceLight::try_new(
            [x as f32, 0.0, z as f32],
            Color::WHITE,
            strength,
            Point::new(0.5, 0.5),
            Point::new(0.35, 0.38),
            Point::new(1.0, 0.0),
        )
        .expect("checked white incident source")
    };
    let inactive = incident(90.0, 0.0);
    let body = SurfaceAnalytic::try_smoked_glass(
        gradient,
        [inactive; 2],
        SmokedGlassParameters {
            opacity: 1.0,
            bevel_width: 2.0,
            bevel_slope: 0.5,
            roughness: 0.28,
            ambient_strength: 0.0,
            diffuse_strength: 0.0,
            specular_strength: 0.0,
            max_interior_uplift: 0.0,
        },
        None,
    )
    .expect("checked neutral smoked face");
    let film = ThinFilmParams::try_new(1.0, 320.0, 1.4).expect("checked 320 nm film");

    [
        ("Film off", 5.0, None),
        ("5 degrees", 5.0, Some(film)),
        ("80 degrees", 80.0, Some(film)),
    ]
    .map(|(label, elevation, film)| {
        let optics = GlassSurfaceOptics::try_new(
            GlassSurfaceOpticsParameters {
                bevel_width: 6.0,
                bevel_slope: 0.55,
                roughness: 1.0,
                normal_reflectance: 0.5,
                reflection_strength: 1.0,
                rim_strength: 0.0,
                rim_falloff: 1.5,
            },
            [incident(elevation, 1.0), inactive],
            Color::WHITE,
            Color::WHITE,
        )
        .expect("checked angular coating")
        .try_with_thin_film(film)
        .expect("checked film selection");
        Specimen {
            label,
            surface: body.clone().with_glass_optics(Some(optics)),
        }
    })
}

struct Specimen {
    label: &'static str,
    surface: SurfaceAnalytic,
}

struct Comparison {
    id: WidgetInternalId,
    specimens: [Specimen; 3],
    swap: bool,
}

impl WidgetDowncast for Comparison {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
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

    fn needs_update(&self) -> bool {
        self.swap
    }

    fn update(&mut self, _: f32, cx: &mut UpdateContext) {
        if std::mem::take(&mut self.swap) {
            self.specimens.swap(1, 2);
            cx.invalidate_paint();
            println!(
                "Iridescent Glass angles: {}, {}",
                self.specimens[1].label, self.specimens[2].label
            );
        }
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let size = constraints.constrain(WINDOW_SIZE);
        let button = Size::new(132.0, 42.0);
        for &child in children {
            cx.layout_child(child, Constraints::tight(button));
            cx.set_child_rect(
                child,
                RectF::new(
                    (size.width - MARGIN - button.width).max(0.0),
                    66.0,
                    button.width,
                    button.height,
                ),
            );
        }
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        cx.scene.push_rounded_rect(
            rect,
            Color::from_hex(0x161b21),
            RoundedCorners::uniform(0.0),
            0,
            None,
        );
        let left = rect.x + MARGIN;
        let bright = Color::from_hex(0xf1f3f5);
        let muted = Color::from_hex(0xb5c0cb);
        text(cx, "Iridescent Glass", left, rect.y + 59.0, 39.0, bright);
        text(
            cx,
            "One white light, two angles, one 320 nm film.",
            left,
            rect.y + 119.0,
            18.0,
            muted,
        );
        let gap = 32.0;
        let width = (rect.width - 2.0 * (MARGIN + gap)) / 3.0;
        let height = (rect.height * 0.36).clamp(120.0, 274.0);
        let top = rect.y + (rect.height * 0.31).clamp(176.0, 235.0);
        if width < 24.0 {
            return;
        }
        cx.scene.push_rounded_rect(
            RectF::new(
                left - 12.0,
                top + height * 0.62,
                rect.width - 2.0 * (MARGIN - 12.0),
                height * 0.29,
            ),
            Color::from_hex(0x29323b),
            RoundedCorners::uniform(20.0),
            2,
            None,
        );
        for (index, specimen) in self.specimens.iter().enumerate() {
            let face = RectF::new(left + index as f32 * (width + gap), top, width, height);
            if let Err(error) = cx.scene.push_analytic_surface(
                face,
                RoundedCorners::uniform(12.0),
                &specimen.surface,
                40 + index as u32,
                None,
            ) {
                eprintln!("Iridescent Glass surface refused: {error}");
            }
            text(cx, specimen.label, face.x, top - 38.0, 19.0, bright);
        }
        text(
            cx,
            "Three-wavelength RGB surrogate for spectral light transport.",
            left,
            top + height + 45.0,
            16.0,
            muted,
        );
    }
}

fn text(cx: &mut PaintContext, value: &str, x: f32, y: f32, size: f32, color: Color) {
    let run = cx.text_engine.shape(value, size);
    cx.text_engine.draw(&run, x, y, color, 100, cx.scene);
}

struct Demo {
    binder: FormBinder,
    placeholder: Label,
    comparison: Option<TreeWidgetId>,
    swap: bool,
}

impl Screen for Demo {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Comparison {
            id: WidgetInternalId::new(),
            specimens: specimens(),
            swap: false,
        }));
        self.comparison = Some(cx.tree.node(root).expect("mounted comparison").id());
        let swap = Button::with_text("Swap angles").with_binding("swap", self.binder.writer());
        cx.tree.add_child(root, Box::new(swap), None, 0);
        &mut self.placeholder
    }

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        self.binder.bind(cx, |bindings| {
            self.swap ^= bindings.bool("swap").take().is_some();
        });
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        if !std::mem::take(&mut self.swap) {
            return;
        }
        let Some(node) = self
            .comparison
            .and_then(|id| tree.find_node_by_widget_id(id))
        else {
            return;
        };
        let comparison = tree
            .node_mut(node)
            .expect("current comparison")
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Comparison>()
            .expect("comparison widget");
        comparison.swap = true;
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();
    let mut config =
        AppConfig::new("Iridescent Glass").with_size(WINDOW_SIZE.width, WINDOW_SIZE.height);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    App::new(config).run(Demo {
        binder: FormBinder::new(),
        placeholder: Label::new(""),
        comparison: None,
        swap: false,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iridescent_glass_changes_incident_angle_on_identical_film() {
        let specimens = specimens();
        let [plain, low, high] = specimens
            .each_ref()
            .map(|sample| sample.surface.glass_optics().unwrap());
        assert_eq!(plain.thin_film(), None);
        assert_eq!(plain.lights(), low.lights());
        assert_eq!(
            low.thin_film(),
            Some(ThinFilmParams::try_new(1.0, 320.0, 1.4).unwrap())
        );
        assert_eq!(low.thin_film(), high.thin_film());
        assert_eq!(low.lights()[0].color(), Color::WHITE);
        assert_eq!(high.lights()[0].color(), Color::WHITE);
        assert_eq!(low.lights()[1], high.lights()[1]);
        for (light, angle) in [(low.lights()[0], 5.0), (high.lights()[0], 80.0)] {
            assert!((light.direction()[2].asin().to_degrees() - angle).abs() < 0.001);
        }
    }
}
