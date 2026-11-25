//! HAL385: current moving backdrop, finite ordinary Timeline, then genuine idle.
use halcyon::core::{
    animation::{Animation, AnimationSample, Easing, TransitionHandle},
    app::{App, AppConfig, Screen, ScreenFrameContext},
    layout::{Constraints, LayoutResult},
    math::{Point, RectF, Size},
    paint::{
        AnalyticSurfaceLight, Color, ColorStop, FrostedGlassAppearance, GradientDef, GradientKind,
        RoundedCorners, SmokedGlassParameters, SurfaceAnalytic, SurfaceGradient,
    },
    widget::{LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast},
    widget_tree::NodeId,
};
use halcyon::{prelude::Label, utils::id::WidgetInternalId};

pub(crate) struct Comparison {
    id: WidgetInternalId,
    smoked: SurfaceAnalytic,
    reference: FrostedGlassAppearance,
    frost: FrostedGlassAppearance,
    feathered: FrostedGlassAppearance,
    handle: Option<TransitionHandle>,
    started: bool,
    phase: f32,
    milestone: u8,
}
impl Comparison {
    pub(crate) fn new() -> Self {
        let base = SurfaceGradient::new(GradientDef {
            kind: GradientKind::Linear,
            p0: (0.0, 0.0),
            p1: (0.0, 1.0),
            stops: vec![
                ColorStop {
                    offset: 0.0,
                    color: Color::from_linear(0.035, 0.055, 0.075, 1.0),
                },
                ColorStop {
                    offset: 0.3,
                    color: Color::from_linear(0.02, 0.03, 0.05, 1.0),
                },
                ColorStop {
                    offset: 1.0,
                    color: Color::from_linear(0.01, 0.02, 0.03, 1.0),
                },
            ],
        })
        .unwrap();
        let light = AnalyticSurfaceLight::try_new(
            [0.0, -0.6, 0.8],
            Color::WHITE,
            0.5,
            Point::new(0.4, 0.2),
            Point::new(1.0, 1.0),
            Point::new(1.0, 0.0),
        )
        .unwrap();
        let smoked = SurfaceAnalytic::try_smoked_glass(
            base,
            [light, light],
            SmokedGlassParameters {
                opacity: 0.64,
                bevel_width: 3.0,
                bevel_slope: 0.7,
                roughness: 0.4,
                ambient_strength: 0.05,
                diffuse_strength: 0.5,
                specular_strength: 0.5,
                max_interior_uplift: 0.2,
            },
            None,
        )
        .unwrap();
        let frost = FrostedGlassAppearance::try_new(
            7.0,
            Color::from_linear(0.025, 0.05, 0.075, 0.35),
            0.86,
        )
        .unwrap();
        let feathered = frost.clone().try_with_edge_feather(8.0).unwrap();
        Self {
            id: WidgetInternalId::new(),
            smoked,
            reference: FrostedGlassAppearance::try_new(0.0, Color::TRANSPARENT, 1.0).unwrap(),
            frost,
            feathered,
            handle: None,
            started: false,
            phase: 0.0,
            milestone: 0,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn with_probe_bloom(mut self) -> Self {
        use halcyon::core::paint::{GlassSurfaceOptics, GlassSurfaceOpticsParameters, SurfaceBloom};

        let lights = [
            Color::from_linear(1.0, 0.25, 0.10, 1.0),
            Color::from_linear(0.08, 0.35, 1.0, 1.0),
        ].map(|color| {
            AnalyticSurfaceLight::try_new(
                [0.0, 0.0, 1.0], color, 0.40, Point::new(0.5, 0.5),
                Point::new(0.2, 0.25), Point::new(1.0, 0.0),
            ).expect("checked browser Bloom source")
        });
        let optics = GlassSurfaceOptics::try_new(
            GlassSurfaceOpticsParameters {
                bevel_width: 1.75, bevel_slope: 0.94, roughness: 0.28,
                normal_reflectance: 0.5, reflection_strength: 1.0, rim_strength: 0.0, rim_falloff: 1.5,
            }, lights, Color::WHITE, Color::WHITE,
        ).expect("checked browser coating");
        self.feathered = self.feathered.try_with_optics(Some(optics), 0.0, 1.0)
            .expect("checked browser feathered coating");
        // Compare the same coated body while retaining the original one Bloom source.
        let bloom = SurfaceBloom::try_new(8.0, 8.0, 1.0).expect("checked browser Bloom");
        self.frost = self.frost.try_with_optics(Some(optics.with_bloom(Some(bloom))), 0.0, 1.0)
            .expect("browser Frosted and Bloom uses positive opacity");
        self
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
    fn needs_update(&self) -> bool {
        !self.started || self.handle.is_some()
    }
    fn needs_animation(&self) -> bool {
        self.handle.is_some()
    }
    fn update(&mut self, _: f32, c: &mut UpdateContext) {
        if !self.started {
            self.started = true;
            self.handle = c.start_animation(Animation::new(0.0_f32, 1.0, 12.0, Easing::Linear));
            assert!(
                self.handle.is_some(),
                "ordinary Timeline required by this native comparison"
            );
        }
        if let Some(h) = self.handle {
            match c.sample_animation::<f32>(h) {
                Some(AnimationSample::Active(v)) => self.phase = v,
                Some(AnimationSample::Completed(v)) => {
                    self.phase = v;
                    self.handle = None;
                }
                _ => {}
            }
            c.invalidate_paint();
        }
        if self.milestone == 0 && self.phase >= 0.12 {
            self.milestone = 1;
            println!("HAL385_NATIVE state=A phase={:.3}", self.phase);
        }
        if self.milestone == 1 && self.phase >= 0.62 {
            self.milestone = 2;
            println!("HAL385_NATIVE state=B phase={:.3}", self.phase);
        }
        if self.milestone == 2 && self.handle.is_none() {
            self.milestone = 3;
            println!("HAL385_NATIVE state=IDLE phase=1 finite_timeline_complete=true");
        }
    }
    fn layout(&mut self, _: &mut LayoutContext, c: Constraints, _: &[NodeId]) -> LayoutResult {
        LayoutResult::fit(c.constrain(Size::new(c.max_width, c.max_height)))
    }
    fn paint(&self, c: &mut PaintContext, r: RectF, _: &[NodeId]) {
        c.scene.push_rect(r, Color::from_hex(0x0a1019), 0, None);
        let text = |c: &mut PaintContext, s: &str, x: f32, y: f32, size: f32, z: u32| {
            let run = c.text_engine.shape(s, size);
            c.text_engine
                .draw(&run, x, y, Color::from_hex(0xf1f4f8), z, c.scene);
        };
        text(
            c,
            "CURRENT BACKDROP / HAL385",
            r.x + 28.0,
            r.y + 26.0,
            22.0,
            60,
        );
        // Reserve room for the existing panel insets and corner radii at narrow widths.
        let min_face_width = 2.0 * (8.0 + 18.0);
        let gap = ((r.width - 4.0 * min_face_width) / 5.0).floor().clamp(0.0, 24.0);
        let width = (r.width - 5.0 * gap) / 4.0;
        for i in 0..4 {
            let face = RectF::new(
                r.x + gap + i as f32 * (width + gap),
                r.y + 124.0,
                width,
                r.height - 222.0,
            );
            c.scene.begin_clip(face, None);
            c.scene.push_rect(face, Color::from_hex(0x18384a), 1, None);
            for n in -4..40 {
                let x = face.x + n as f32 * 13.0 + self.phase * 41.0;
                c.scene.push_rect(
                    RectF::new(x, face.y, 5.0, face.height),
                    if n % 3 == 0 {
                        Color::from_hex(0xe4a35b)
                    } else {
                        Color::from_hex(0x59bacc)
                    },
                    2,
                    None,
                );
            }
            text(
                c,
                "LOWER ORDER / REAL TEXT",
                face.x + 8.0,
                face.y + 118.0,
                16.0,
                3,
            );
            let panel = RectF::new(
                face.x + 8.0,
                face.y + 12.0,
                face.width - 16.0,
                face.height - 24.0,
            );
            if i == 0 {
                c.scene
                    .push_analytic_surface(
                        panel,
                        RoundedCorners::uniform(18.0),
                        &self.smoked,
                        10,
                        None,
                    )
                    .unwrap();
            } else {
                c.scene
                    .push_frosted_glass(
                        panel,
                        RoundedCorners::uniform(18.0),
                        [&self.reference, &self.frost, &self.feathered][i - 1],
                        10 + i as u32,
                        None,
                    )
                    .unwrap();
            }
            text(
                c,
                "Sharp foreground",
                face.x + 22.0,
                face.y + 52.0,
                18.0,
                30,
            );
            text(
                c,
                "Real Text / same surface",
                face.x + 22.0,
                face.y + 80.0,
                13.0,
                30,
            );
            c.scene.end_clip();
            let frosted_label = if self.frost.glass_optics().and_then(|optics| optics.bloom()).is_some() {
                "FROSTED + BLOOM"
            } else {
                "FROSTED / sigma 7"
            };
            text(
                c,
                [
                    "SMOKED / no blur",
                    "SIGMA ZERO / identity",
                    frosted_label,
                    "FROSTED / 8 px feather",
                ][i],
                face.x,
                face.y - 35.0,
                15.0,
                60,
            );
        }
        text(
            c,
            "12-second ordinary Timeline; background settles to idle. No input or backdrop history.",
            r.x + 28.0,
            r.y + r.height - 60.0,
            14.0,
            60,
        );
    }
}
struct Example {
    placeholder: Label,
}
impl Screen for Example {
    fn build<'a>(&'a mut self, c: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        c.mount_child(Box::new(Comparison::new()));
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
    App::new(AppConfig::new("Current Backdrop Frosted Glass - HAL385").with_size(1080.0, 590.0))
        .run(Example {
            placeholder: Label::new(""),
        });
}
