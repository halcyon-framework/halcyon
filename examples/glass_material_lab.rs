// HAL386 §11: ordinary controls, retained value publication and current-backdrop optics.
use halcyon as lab_framework;
use lab_framework::core::{
    animation::{Animation, AnimationSample, Easing, TransitionHandle},
    app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext},
    layout::{Alignment, Constraints, EdgeInsets, FlexParams, LayoutResult},
    math::{Point, RectF, Size},
    paint::{
        AnalyticSurfaceLight, Color, ColorStop, FrostedGlassAppearance, GlassSurfaceOptics,
        GlassSurfaceOpticsParameters, GradientDef, GradientKind, RoundedCorners,
        SmokedGlassParameters, SurfaceAnalytic, SurfaceEmission, SurfaceEmissionParameters,
        SurfaceBloom, SurfaceGradient, SurfaceMicrograin, ThinFilmParams,
    },
    widget::{LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast},
    widget_tree::{NodeId, OverflowMode, WidgetTree},
};
use lab_framework::{
    model::form_binder::FormBinder,
    prelude::{Flex, Label, ScrollArea},
    utils::id::{TreeWidgetId, WidgetInternalId},
    widgets::{
        checkbox::Checkbox,
        radio_group::RadioId,
        radio_group_widget::{RadioGroup, RadioGroupId},
        slider::Slider,
    },
};

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
enum K {
    Blur,
    Transmission,
    Refraction,
    Opacity,
    TintR,
    TintG,
    TintB,
    TintA,
    Corner,
    Width,
    Slope,
    Roughness,
    Reflectance,
    Reflection,
    Rim,
    RimFalloff,
    ReflectionR,
    ReflectionG,
    ReflectionB,
    ReflectionA,
    RimR,
    RimG,
    RimB,
    RimA,
    Az0,
    Elev0,
    Strength0,
    Cx0,
    Cy0,
    Rx0,
    Ry0,
    Az1,
    Elev1,
    Strength1,
    Cx1,
    Cy1,
    Rx1,
    Ry1,
    Grain,
    Scale,
    Face,
    Edge,
    Halo,
    EdgeWidth,
    HaloRadius,
    Falloff,
    Directionality,
    Direction,
    EmitR,
    EmitG,
    EmitB,
    EmitA,
    BodyWidth,
    BodySlope,
    BodyRoughness,
    Ambient,
    Diffuse,
    Specular,
    Uplift,
    Rot0,
    LightR0,
    LightG0,
    LightB0,
    Rot1,
    LightR1,
    LightG1,
    LightB1,
}
const COUNT: usize = K::LightB1 as usize + 1;
struct Control {
    field: K,
    key: &'static str,
    label: &'static str,
    min: f32,
    max: f32,
    step: f32,
    initial: f32,
}
macro_rules! control {
    ($k:ident,$label:literal,$lo:expr,$hi:expr,$step:expr,$v:expr) => {
        Control {
            field: K::$k,
            key: stringify!($k),
            label: $label,
            min: $lo,
            max: $hi,
            step: $step,
            initial: $v,
        }
    };
}
const CONTROLS: &[Control] = &[
    control!(Blur, "Blur [Frosted]", 0.0, 24.0, 0.25, 9.0),
    control!(
        Transmission,
        "Transmission [Frosted]",
        0.0,
        1.0,
        0.005,
        0.94
    ),
    control!(
        Refraction,
        "Signed displacement [Frosted]",
        -32.0,
        32.0,
        0.5,
        0.0
    ),
    control!(Opacity, "Body opacity", 0.0, 1.0, 0.01, 1.0),
    control!(TintR, "Tint linear red", 0.0, 1.0, 0.005, 0.03),
    control!(TintG, "Tint linear green", 0.0, 1.0, 0.005, 0.04),
    control!(TintB, "Tint linear blue", 0.0, 1.0, 0.005, 0.06),
    control!(TintA, "Tint alpha [Frosted]", 0.0, 1.0, 0.01, 0.25),
    control!(Corner, "Corner radius", 0.0, 64.0, 0.5, 18.0),
    control!(Width, "Coating bevel width", 0.0, 32.0, 0.25, 1.75),
    control!(Slope, "Coating slope", 0.0, 0.98, 0.01, 0.94),
    control!(Roughness, "Coating roughness", 0.04, 1.0, 0.01, 0.28),
    control!(Reflectance, "Normal reflectance", 0.0, 1.0, 0.01, 0.08),
    control!(Reflection, "Reflection strength", 0.0, 1.0, 0.01, 0.90),
    control!(Rim, "Directed rim strength", 0.0, 1.0, 0.01, 0.65),
    control!(RimFalloff, "Rim falloff", 1.0, 8.0, 0.1, 1.5),
    control!(ReflectionR, "Reflection red", 0.0, 1.0, 0.01, 1.0),
    control!(ReflectionG, "Reflection green", 0.0, 1.0, 0.01, 1.0),
    control!(ReflectionB, "Reflection blue", 0.0, 1.0, 0.01, 1.0),
    control!(ReflectionA, "Reflection alpha", 0.0, 1.0, 0.01, 1.0),
    control!(RimR, "Rim red", 0.0, 1.0, 0.01, 1.0),
    control!(RimG, "Rim green", 0.0, 1.0, 0.01, 1.0),
    control!(RimB, "Rim blue", 0.0, 1.0, 0.01, 1.0),
    control!(RimA, "Rim alpha", 0.0, 1.0, 0.01, 1.0),
    control!(Az0, "Light 1 azimuth", -180.0, 180.0, 0.0, -135.0),
    control!(Elev0, "Light 1 elevation", 1.0, 90.0, 0.0, 75.0),
    control!(Strength0, "Light 1 intensity", 0.0, 1.0, 0.01, 0.95),
    control!(Cx0, "Light 1 center X", 0.0, 1.0, 0.01, 0.16),
    control!(Cy0, "Light 1 center Y", 0.0, 1.0, 0.01, 0.40),
    control!(Rx0, "Light 1 radius X", 0.02, 2.0, 0.01, 0.13),
    control!(Ry0, "Light 1 radius Y", 0.02, 2.0, 0.01, 0.70),
    control!(Az1, "Light 2 azimuth", -180.0, 180.0, 0.0, -135.0),
    control!(Elev1, "Light 2 elevation", 1.0, 90.0, 0.0, 25.0),
    control!(Strength1, "Light 2 intensity", 0.0, 1.0, 0.01, 0.85),
    control!(Cx1, "Light 2 center X", 0.0, 1.0, 0.01, 0.06),
    control!(Cy1, "Light 2 center Y", 0.0, 1.0, 0.01, 0.10),
    control!(Rx1, "Light 2 radius X", 0.02, 2.0, 0.01, 0.11),
    control!(Ry1, "Light 2 radius Y", 0.02, 2.0, 0.01, 0.16),
    control!(Grain, "Grain reference codes", 0.0, 8.0, 0.1, 0.6),
    control!(Scale, "Grain scale (target px)", 0.75, 3.0, 0.05, 1.5),
    control!(Face, "Emission face", 0.0, 1.0, 0.005, 0.025),
    control!(Edge, "Emission edge", 0.0, 1.0, 0.01, 0.2),
    control!(Halo, "Emission halo", 0.0, 1.0, 0.01, 0.1),
    control!(EdgeWidth, "Emission edge width", 0.25, 32.0, 0.25, 2.0),
    control!(HaloRadius, "Halo radius", 0.0, 96.0, 0.5, 18.0),
    control!(Falloff, "Emission falloff", 1.0, 8.0, 0.1, 2.0),
    control!(
        Directionality,
        "Emission directionality",
        0.0,
        1.0,
        0.01,
        0.6
    ),
    control!(Direction, "Emission angle", -180.0, 180.0, 0.0, 90.0),
    control!(EmitR, "Emission linear red", 0.0, 1.0, 0.01, 0.0),
    control!(EmitG, "Emission linear green", 0.0, 1.0, 0.01, 0.0),
    control!(EmitB, "Emission linear blue", 0.0, 1.0, 0.01, 0.0),
    control!(EmitA, "Emission alpha", 0.0, 1.0, 0.01, 1.0),
    control!(BodyWidth, "Body bevel [Smoked]", 0.0, 4.0, 0.5, 3.0),
    control!(BodySlope, "Body slope [Smoked]", 0.0, 0.9, 0.01, 0.7),
    control!(
        BodyRoughness,
        "Body roughness [Smoked]",
        0.04,
        1.0,
        0.01,
        0.4
    ),
    control!(Ambient, "Ambient [Smoked]", 0.0, 1.0, 0.01, 0.05),
    control!(Diffuse, "Diffuse [Smoked]", 0.0, 1.0, 0.01, 0.5),
    control!(Specular, "Specular [Smoked]", 0.0, 1.0, 0.01, 0.5),
    control!(Uplift, "Interior uplift [Smoked]", 0.0, 0.25, 0.005, 0.2),
    control!(Rot0, "Light 1 rotation", -180.0, 180.0, 1.0, -20.0),
    control!(LightR0, "Light 1 linear red", 0.0, 1.0, 0.01, 1.0),
    control!(LightG0, "Light 1 linear green", 0.0, 1.0, 0.01, 1.0),
    control!(LightB0, "Light 1 linear blue", 0.0, 1.0, 0.01, 1.0),
    control!(Rot1, "Light 2 rotation", -180.0, 180.0, 1.0, -20.0),
    control!(LightR1, "Light 2 linear red", 0.0, 1.0, 0.01, 1.0),
    control!(LightG1, "Light 2 linear green", 0.0, 1.0, 0.01, 1.0),
    control!(LightB1, "Light 2 linear blue", 0.0, 1.0, 0.01, 1.0),
];
#[derive(Clone, Debug, PartialEq)]
struct Request {
    values: [f32; COUNT],
    material: Option<u32>,
    background: Option<u32>,
    coating: bool,
    emission: bool,
}
impl Request {
    fn initial() -> Self {
        let mut values = [0.0; COUNT];
        for c in CONTROLS {
            values[c.field as usize] = c.initial;
        }
        let e = Color::from_srgb(0.82, 0.74, 0.56, 1.0);
        for (k, v) in [(K::EmitR, e.r), (K::EmitG, e.g), (K::EmitB, e.b)] {
            values[k as usize] = v;
        }
        Self {
            values,
            material: Some(1),
            background: Some(0),
            coating: true,
            emission: false,
        }
    }
    fn get(&self, k: K) -> f32 {
        self.values[k as usize]
    }
    fn checked(&self) -> Result<Material, String> {
        if !matches!(self.background, Some(0) | Some(1)) {
            return Err("Select a background".into());
        }
        let v = |k| self.get(k);
        let light =
            |az: K, el: K, strength: K, cx: K, cy: K, rx: K, ry: K, rotation: K, rgb: [K; 3]| {
                let az = v(az).to_radians();
                let el = v(el).to_radians();
                let degrees = v(rotation);
                if !degrees.is_finite() || !(-180.0..=180.0).contains(&degrees) {
                    return Err(
                        "Light rotation must be finite and within [-180,180] degrees".to_owned(),
                    );
                }
                let (sin, cos) = degrees.to_radians().sin_cos();
                AnalyticSurfaceLight::try_new(
                    [az.cos() * el.cos(), az.sin() * el.cos(), el.sin()],
                    Color::from_linear(v(rgb[0]), v(rgb[1]), v(rgb[2]), 1.0),
                    v(strength),
                    Point::new(v(cx), v(cy)),
                    Point::new(v(rx), v(ry)),
                    Point::new(cos, sin),
                )
                .map_err(|e| e.to_string())
            };
        let lights = [
            light(
                K::Az0,
                K::Elev0,
                K::Strength0,
                K::Cx0,
                K::Cy0,
                K::Rx0,
                K::Ry0,
                K::Rot0,
                [K::LightR0, K::LightG0, K::LightB0],
            )?,
            light(
                K::Az1,
                K::Elev1,
                K::Strength1,
                K::Cx1,
                K::Cy1,
                K::Rx1,
                K::Ry1,
                K::Rot1,
                [K::LightR1, K::LightG1, K::LightB1],
            )?,
        ];
        let optics = if self.coating {
            Some(
                GlassSurfaceOptics::try_new(
                    GlassSurfaceOpticsParameters {
                        bevel_width: v(K::Width),
                        bevel_slope: v(K::Slope),
                        roughness: v(K::Roughness),
                        normal_reflectance: v(K::Reflectance),
                        reflection_strength: v(K::Reflection),
                        rim_strength: v(K::Rim),
                        rim_falloff: v(K::RimFalloff),
                    },
                    lights,
                    Color::from_linear(
                        v(K::ReflectionR),
                        v(K::ReflectionG),
                        v(K::ReflectionB),
                        v(K::ReflectionA),
                    ),
                    Color::from_linear(v(K::RimR), v(K::RimG), v(K::RimB), v(K::RimA)),
                )
                .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        let emission = if self.emission {
            let angle = v(K::Direction).to_radians();
            Some(
                SurfaceEmission::try_new(
                    Color::from_linear(v(K::EmitR), v(K::EmitG), v(K::EmitB), v(K::EmitA)),
                    SurfaceEmissionParameters {
                        face_strength: v(K::Face),
                        edge_strength: v(K::Edge),
                        halo_strength: v(K::Halo),
                        edge_width: v(K::EdgeWidth),
                        halo_radius: v(K::HaloRadius),
                        falloff: v(K::Falloff),
                        directionality: v(K::Directionality),
                        direction: Point::new(angle.cos(), angle.sin()),
                    },
                )
                .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        let grain = Some(
            SurfaceMicrograin::try_new(v(K::Scale), v(K::Grain) / 255.0, 387)
                .map_err(|e| e.to_string())?,
        );
        let tint = Color::from_linear(v(K::TintR), v(K::TintG), v(K::TintB), v(K::TintA));
        match self.material {
            Some(0) => Ok(Material::Smoked(
                SurfaceAnalytic::try_smoked_glass(
                    base(tint)?,
                    lights,
                    SmokedGlassParameters {
                        opacity: v(K::Opacity),
                        bevel_width: v(K::BodyWidth),
                        bevel_slope: v(K::BodySlope),
                        roughness: v(K::BodyRoughness),
                        ambient_strength: v(K::Ambient),
                        diffuse_strength: v(K::Diffuse),
                        specular_strength: v(K::Specular),
                        max_interior_uplift: v(K::Uplift),
                    },
                    None,
                )
                .map_err(|e| e.to_string())?
                .with_glass_optics(optics)
                .with_micrograin(grain)
                .with_surface_emission(emission),
            )),
            Some(1) => Ok(Material::Frosted(
                FrostedGlassAppearance::try_new(v(K::Blur), tint, v(K::Transmission))
                    .map_err(|e| e.to_string())?
                    .try_with_optics(optics, self.refraction(), v(K::Opacity))
                    .map_err(|e| e.to_string())?
                    .with_micrograin(grain)
                    .with_surface_emission(emission),
            )),
            _ => Err("Select Smoked or Frosted material".into()),
        }
    }
    fn refraction(&self) -> f32 {
        self.get(K::Refraction)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Material {
    None,
    Smoked(SurfaceAnalytic),
    Frosted(FrostedGlassAppearance),
}
fn base(tint: Color) -> Result<SurfaceGradient, String> {
    SurfaceGradient::new(GradientDef {
        kind: GradientKind::Linear,
        p0: (0.0, 0.0),
        p1: (0.0, 1.0),
        stops: [(0.0, 1.0), (0.3, 0.8), (1.0, 0.6)]
            .into_iter()
            .map(|(offset, gain)| ColorStop {
                offset,
                color: Color::from_linear(tint.r * gain, tint.g * gain, tint.b * gain, 1.0),
            })
            .collect(),
    })
    .map_err(|e| format!("Invalid base gradient: {e:?}"))
}
fn fixture_lights() -> [AnalyticSurfaceLight; 2] {
    let z = 0.75_f32.sqrt();
    [
        (
            [-0.3, -0.4, z],
            0.85,
            Point::new(0.25, 0.15),
            Point::new(1.0, 0.65),
        ),
        (
            [0.3, 0.4, z],
            0.25,
            Point::new(0.75, 0.85),
            Point::new(0.8, 0.8),
        ),
    ]
    .map(|(d, s, c, r)| {
        AnalyticSurfaceLight::try_new(d, Color::WHITE, s, c, r, Point::new(1.0, 0.0))
            .expect("constant admitted fixture light")
    })
}
fn fixed_profile(sigma: f32, transmission: f32, lens: bool, coated: bool) -> Material {
    let optics = if coated {
        Some(
            GlassSurfaceOptics::try_new(
                GlassSurfaceOpticsParameters {
                    bevel_width: if lens { 16.0 } else { 1.25 },
                    bevel_slope: if lens { 0.95 } else { 0.9 },
                    roughness: if lens { 0.2 } else { 0.35 },
                    normal_reflectance: 0.1,
                    reflection_strength: if lens { 0.7 } else { 0.55 },
                    rim_strength: if lens { 0.65 } else { 0.7 },
                    rim_falloff: 1.5,
                },
                fixture_lights(),
                Color::WHITE,
                Color::from_linear(0.8, 0.9, 1.0, 1.0),
            )
            .expect("constant admitted coating"),
        )
    } else {
        None
    };
    Material::Frosted(
        FrostedGlassAppearance::try_new(
            sigma,
            Color::from_linear(0.03, 0.04, 0.06, 0.25),
            transmission,
        )
        .expect("constant admitted glass")
        .try_with_optics(optics, if lens { 12.0 } else { 0.0 }, 1.0)
        .expect("constant admitted lens")
        .with_micrograin(Some(
            SurfaceMicrograin::try_new(1.5, 0.6 / 255.0, 387).expect("constant grain"),
        )),
    )
}
fn action_source() -> SurfaceAnalytic {
    SurfaceAnalytic::try_smoked_glass(
        base(Color::from_linear(0.008, 0.012, 0.019, 1.0)).expect("constant base"),
        fixture_lights(),
        SmokedGlassParameters {
            opacity: 1.0,
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
    .expect("constant dark face")
    .with_surface_emission(Some(
        SurfaceEmission::try_new(
            Color::from_srgb(0.82, 0.74, 0.56, 1.0),
            SurfaceEmissionParameters {
                face_strength: 0.025,
                edge_strength: 0.2,
                halo_strength: 0.1,
                edge_width: 2.0,
                halo_radius: 18.0,
                falloff: 2.0,
                directionality: 0.6,
                direction: Point::new(0.0, 1.0),
            },
        )
        .expect("constant source"),
    ))
}
fn text(c: &mut PaintContext, s: &str, x: f32, y: f32, size: f32, color: Color) {
    text_at_z(c, s, x, y, size, color, 100);
}
fn text_at_z(c: &mut PaintContext, s: &str, x: f32, y: f32, size: f32, color: Color, z: u32) {
    let run = c.text_engine.shape(s, size);
    c.text_engine.draw(&run, x, y, color, z, c.scene);
}
fn cell(r: RectF, index: usize) -> RectF {
    RectF::new(
        r.x + 12.0 + (index % 3) as f32 * 280.0,
        r.y + 64.0 + (index / 3) as f32 * 216.0,
        272.0,
        204.0,
    )
}
fn face(r: RectF) -> RectF {
    RectF::new(r.x + 16.0, r.y + 38.0, 240.0, 130.0)
}
macro_rules! downcast {
    () => {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    };
}
const CROSSING_FACE: Size = Size::new(320.0, 160.0);
const CROSSING_CAPTION: f32 = 32.0;
const CROSSING_COMPARISON: Size = Size::new(864.0, 296.0);
const THIN_FILM_COMPARISON: Size = Size::new(292.0, 104.0);
const THIN_FILM_SWAP_CONTROL: Size = Size::new(132.0, 28.0);
const STATUS_TOP: f32 = 800.0;
const CANVAS: Size = Size::new(888.0, STATUS_TOP + 840.0);

/// Ordinary layout only. Background and surface keep separate retained fragments.
struct Canvas {
    id: WidgetInternalId,
}
impl WidgetDowncast for Canvas {
    downcast!();
}
impl Widget for Canvas {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        false
    }
    fn paint(&self, c: &mut PaintContext, r: RectF, _: &[NodeId]) {
        c.scene.push_rect(r, Color::from_hex(0x101925), 0, None);
    }
    fn layout(
        &mut self,
        c: &mut LayoutContext,
        k: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        for (index, &child) in children.iter().enumerate() {
            let rect = match index {
                0..=5 => cell(RectF::new(0.0, 0.0, CANVAS.width, CANVAS.height), index),
                6 => RectF::new(0.0, 0.0, CANVAS.width, 64.0),
                7 => RectF::new(0.0, 520.0, CANVAS.width, 116.0),
                8 => RectF::new(0.0, STATUS_TOP, CANVAS.width, CANVAS.height - STATUS_TOP),
                9 => RectF::new(
                    12.0,
                    496.0,
                    CROSSING_COMPARISON.width,
                    CROSSING_COMPARISON.height,
                ),
                10 => RectF::new(12.0, 496.0, THIN_FILM_COMPARISON.width, THIN_FILM_COMPARISON.height),
                _ => unreachable!("unexpected glass lab canvas child"),
            };
            c.layout_child(
                child,
                Constraints::tight(Size::new(rect.width, rect.height)),
            );
            c.set_child_rect(child, rect);
        }
        LayoutResult::new(k.constrain(CANVAS), CANVAS)
    }
}

/// A real layout ancestor. Its OverflowMode is set by the Screen; only the
/// canonical traversal emits the clip, around both independently painted children.
struct CellHost {
    id: WidgetInternalId,
}
impl WidgetDowncast for CellHost {
    downcast!();
}
impl Widget for CellHost {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        false
    }
    fn paint(&self, _: &mut PaintContext, _: RectF, _: &[NodeId]) {}
    fn layout(
        &mut self,
        c: &mut LayoutContext,
        k: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let size = k.constrain(Size::new(272.0, 204.0));
        for &child in children {
            c.layout_child(child, Constraints::tight(size));
            c.set_child_rect(child, RectF::new(0.0, 0.0, size.width, size.height));
        }
        LayoutResult::fit(size)
    }
}
struct Background {
    id: WidgetInternalId,
    cell: usize,
    kind: Option<u32>,
    started: bool,
    handle: Option<TransitionHandle>,
    phase: f32,
    milestone: u8,
    failed: bool,
}
impl Background {
    fn new(cell: usize, kind: Option<u32>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            cell,
            kind,
            started: false,
            handle: None,
            phase: 0.0,
            milestone: 0,
            failed: false,
        }
    }
    fn set_kind(&mut self, kind: Option<u32>) -> bool {
        if self.kind == kind {
            false
        } else {
            self.kind = kind;
            true
        }
    }
}
impl WidgetDowncast for Background {
    downcast!();
}
impl Widget for Background {
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
                "HAL386 ordinary finite Timeline required"
            );
        }
        if let Some(h) = self.handle {
            match c.sample_animation::<f32>(h) {
                Some(AnimationSample::Active(v)) => self.phase = v,
                Some(AnimationSample::Completed(v)) => {
                    self.phase = v;
                    self.handle = None;
                }
                other => {
                    eprintln!("HAL_OPTICS_S2 background Timeline sample unavailable: {other:?}");
                    self.failed = true;
                    self.handle = None;
                }
            }
            c.invalidate_paint();
        }
        let next = if self.failed {
            4
        } else if self.handle.is_none() {
            3
        } else if self.phase >= 0.62 {
            2
        } else if self.phase >= 0.12 {
            1
        } else {
            0
        };
        if next > self.milestone {
            self.milestone = next;
            println!(
                "HAL_OPTICS_S2 background_state={} cell={} phase={} finite_complete={}",
                ["START", "A", "B", "IDLE", "ERROR"][next as usize],
                self.cell,
                self.phase,
                self.handle.is_none() && !self.failed
            );
        }
    }
    fn layout(&mut self, _: &mut LayoutContext, k: Constraints, _: &[NodeId]) -> LayoutResult {
        LayoutResult::fit(k.constrain(Size::new(272.0, 204.0)))
    }
    fn paint(&self, c: &mut PaintContext, r: RectF, _: &[NodeId]) {
        c.scene.push_rect(r, Color::from_hex(0x101925), 0, None);
        if !matches!(self.kind, Some(0) | Some(1)) {
            text(
                c,
                "Select a background",
                r.x + 14.0,
                r.y + 36.0,
                16.0,
                Color::WHITE,
            );
            return;
        }
        {
            let b = r;
            let p = face(b);
            c.scene.push_rect(
                b,
                if self.kind == Some(0) {
                    Color::from_hex(0x233349)
                } else {
                    Color::from_hex(0x263440)
                },
                1,
                None,
            );
            if self.kind == Some(0) {
                for (n, color) in [
                    (0, Color::from_hex(0x69b7db)),
                    (1, Color::from_hex(0xb97992)),
                ] {
                    let x = p.x - 38.0 + n as f32 * 135.0 + self.phase * 26.0;
                    let gradient = GradientDef {
                        kind: GradientKind::Radial,
                        p0: (0.35, 0.3),
                        p1: (0.95, 0.95),
                        stops: vec![
                            ColorStop { offset: 0.0, color },
                            ColorStop {
                                offset: 1.0,
                                color: Color::from_linear(
                                    color.r * 0.18,
                                    color.g * 0.18,
                                    color.b * 0.18,
                                    1.0,
                                ),
                            },
                        ],
                    };
                    c.scene.push_gradient_rect(
                        RectF::new(x, p.y - 28.0, 190.0, 190.0),
                        95.0,
                        &gradient,
                        2 + n,
                        None,
                    );
                }
                c.scene.push_rect(
                    RectF::new(p.x + 78.0 + self.phase * 9.0, p.y + 18.0, 2.0, 91.0),
                    Color::from_hex(0xe6edf1),
                    4,
                    None,
                );
                c.scene.push_rect(
                    RectF::new(p.x + 45.0, p.y + 77.0, 125.0, 2.0),
                    Color::from_hex(0x152332),
                    5,
                    None,
                );
            } else {
                c.scene.push_rect(
                    RectF::new(p.x + 8.0 + self.phase * 12.0, p.y + 10.0, 194.0, 92.0),
                    Color::from_hex(0x536b79),
                    2,
                    None,
                );
                for row in 0..3 {
                    c.scene.push_rect(
                        RectF::new(p.x + 14.0, p.y + 26.0 + row as f32 * 26.0, 205.0, 1.0),
                        Color::from_hex(0x9aaab3),
                        4,
                        None,
                    );
                }
                text_at_z(
                    c,
                    "Inbox / Review notes",
                    p.x + 15.0,
                    p.y + 16.0,
                    14.0,
                    Color::WHITE,
                    5,
                );
                text_at_z(
                    c,
                    "Today     3 items",
                    p.x + 15.0,
                    p.y + 52.0,
                    13.0,
                    Color::WHITE,
                    5,
                );
                c.scene.push_circle(
                    Point::new(p.x + 180.0, p.y + 82.0),
                    6.0,
                    Color::from_hex(0x9dc4b5),
                    6,
                    None,
                );
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Presentation {
    request: Request,
    checked: Result<Material, String>,
}
/// Immutable cell presentation. The background and clip belong to other tree nodes.
struct MaterialView {
    id: WidgetInternalId,
    cell: usize,
    checked: Result<Material, String>,
    corner: f32,
}
impl MaterialView {
    fn new(cell: usize, checked: Result<Material, String>, corner: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            cell,
            checked,
            corner,
        }
    }
    fn set_value(&mut self, value: &Presentation) -> bool {
        let corner = value.request.get(K::Corner);
        if self.checked == value.checked && self.corner == corner {
            return false;
        }
        self.checked = value.checked.clone();
        self.corner = corner;
        true
    }
}
impl WidgetDowncast for MaterialView {
    downcast!();
}
impl Widget for MaterialView {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        false
    }
    fn layout(&mut self, _: &mut LayoutContext, k: Constraints, _: &[NodeId]) -> LayoutResult {
        LayoutResult::fit(k.constrain(Size::new(272.0, 204.0)))
    }
    fn paint(&self, c: &mut PaintContext, b: RectF, _: &[NodeId]) {
        let p = face(b);
        let corners = RoundedCorners::uniform(self.corner);
        match &self.checked {
            Ok(Material::None) => {}
            Ok(Material::Smoked(m)) => {
                if let Err(e) =
                    c.scene
                        .push_analytic_surface(p, corners, m, 10 + self.cell as u32, None)
                {
                    eprintln!("HAL_OPTICS_S2 local paint refused: {e}");
                    text(c, "Appearance refused", p.x, p.y + 48.0, 14.0, Color::WHITE);
                }
            }
            Ok(Material::Frosted(m)) => {
                if let Err(e) =
                    c.scene
                        .push_frosted_glass(p, corners, m, 10 + self.cell as u32, None)
                {
                    eprintln!("HAL_OPTICS_S2 local paint refused: {e}");
                    text(c, "Appearance refused", p.x, p.y + 48.0, 14.0, Color::WHITE);
                }
            }
            Err(_) => text(
                c,
                "INVALID: not painted",
                p.x + 8.0,
                p.y + 66.0,
                15.0,
                Color::from_hex(0xffb9a0),
            ),
        }
        text(
            c,
            [
                "No material",
                "Thin clear",
                "Thin frosted",
                "Lens-like / +12",
                "S1 frost / no coating",
                "EDITABLE",
            ][self.cell],
            b.x + 16.0,
            b.y + 6.0,
            15.0,
            Color::WHITE,
        );
        text(
            c,
            "Sharp foreground",
            p.x + 18.0,
            p.y + 36.0,
            17.0,
            Color::WHITE,
        );
        text(
            c,
            "Native glyphs / no filtering",
            p.x + 18.0,
            p.y + 62.0,
            12.0,
            Color::WHITE,
        );
    }
}

struct Header {
    id: WidgetInternalId,
}
impl WidgetDowncast for Header {
    downcast!();
}
impl Widget for Header {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        false
    }
    fn layout(&mut self, _: &mut LayoutContext, k: Constraints, _: &[NodeId]) -> LayoutResult {
        LayoutResult::fit(k.constrain(Size::new(CANVAS.width, 64.0)))
    }
    fn paint(&self, c: &mut PaintContext, r: RectF, _: &[NodeId]) {
        text(
            c,
            "GLASS / MATERIAL LAB",
            r.x + 14.0,
            r.y + 12.0,
            22.0,
            Color::WHITE,
        );
        text(
            c,
            "Same geometry and current source. Scroll controls; all changes keep the mounted tree.",
            r.x + 14.0,
            r.y + 42.0,
            13.0,
            Color::from_hex(0xb7cad7),
        );
    }
}

struct ActionSource {
    id: WidgetInternalId,
    value: SurfaceAnalytic,
}
impl WidgetDowncast for ActionSource {
    downcast!();
}
impl Widget for ActionSource {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        false
    }
    fn layout(&mut self, _: &mut LayoutContext, k: Constraints, _: &[NodeId]) -> LayoutResult {
        LayoutResult::fit(k.constrain(Size::new(CANVAS.width, 116.0)))
    }
    fn paint(&self, c: &mut PaintContext, r: RectF, _: &[NodeId]) {
        let button = RectF::new(r.x + 332.0, r.y + 32.0, 200.0, 44.0);
        if let Err(e) = c.scene.push_analytic_surface(
            button,
            RoundedCorners::uniform(8.0),
            &self.value,
            40,
            None,
        ) {
            eprintln!("HAL_OPTICS_S2 source refused: {e}");
        }
        text(
            c,
            "Continue",
            button.x + 60.0,
            button.y + 13.0,
            17.0,
            Color::WHITE,
        );
        text(
            c,
            "Connected face / edge / halo (display only)",
            r.x + 260.0,
            r.y + 97.0,
            14.0,
            Color::from_hex(0xcbd6df),
        );
    }
}

const CROSSING_SECONDS: f32 = 2.4;
const CROSSING_START: [Point; 2] = [Point::new(0.20, 0.50), Point::new(0.80, 0.50)];

fn crossing_optics(centers: [Point; 2]) -> Result<GlassSurfaceOptics, String> {
    let light = |center, color| {
        AnalyticSurfaceLight::try_new(
            [0.0, 0.0, 1.0],
            color,
            0.40,
            center,
            Point::new(0.20, 0.25),
            Point::new(1.0, 0.0),
        )
        .map_err(|error| error.to_string())
    };
    GlassSurfaceOptics::try_new(
        GlassSurfaceOpticsParameters {
            bevel_width: 1.75,
            bevel_slope: 0.94,
            roughness: 0.28,
            normal_reflectance: 0.50,
            reflection_strength: 1.0,
            rim_strength: 0.0,
            rim_falloff: 1.5,
        },
        [
            light(centers[0], Color::from_linear(1.0, 0.25, 0.10, 1.0))?,
            light(centers[1], Color::from_linear(0.08, 0.35, 1.0, 1.0))?,
        ],
        Color::WHITE,
        Color::WHITE,
    )
    .map_err(|error| error.to_string())
}

fn start_crossing(cx: &mut UpdateContext) -> Result<[TransitionHandle; 2], String> {
    let [left, right] = CROSSING_START;
    let start = |cx: &mut UpdateContext, from, to| {
        cx.try_start_animation(Animation::new(from, to, CROSSING_SECONDS, Easing::Linear))
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "missing mounted Timeline".to_owned())
    };
    let warm = start(cx, left, right)?;
    match start(cx, right, left) {
        Ok(cool) => Ok([warm, cool]),
        Err(error) => {
            cx.cancel_animation(warm);
            Err(error)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CrossingPlayback {
    Pending,
    Playing([TransitionHandle; 2]),
    Finished,
    Failed,
}

/// The additional HAL-393 comparison owns only its material and two Timeline handles.
struct CrossingReflections {
    id: WidgetInternalId,
    material: SurfaceAnalytic,
    bloom_material: SurfaceAnalytic,
    playback: CrossingPlayback,
}

pub(crate) fn neutral_smoked_glass() -> SurfaceAnalytic {
    let neutral = Color::from_linear(0.08, 0.08, 0.08, 1.0);
    let body = SurfaceGradient::new(GradientDef {
        kind: GradientKind::Linear,
        p0: (0.0, 0.0),
        p1: (1.0, 0.0),
        stops: [0.0, 0.30, 1.0]
            .map(|offset| ColorStop { offset, color: neutral })
            .into(),
    })
    .expect("neutral glass body");
    let unlit = AnalyticSurfaceLight::try_new(
        [0.0, 0.0, 1.0],
        Color::WHITE,
        0.0,
        Point::new(0.50, 0.50),
        Point::new(0.20, 0.25),
        Point::new(1.0, 0.0),
    )
    .expect("unlit glass body source");
    SurfaceAnalytic::try_smoked_glass(
        body,
        [unlit; 2],
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
    .expect("neutral smoked-glass body")
}

impl CrossingReflections {
    fn new() -> Self {
        let optics = crossing_optics(CROSSING_START).expect("HAL-393 first pair");
        let bloom = SurfaceBloom::try_new(8.0, 16.0, 1.0).expect("HAL-395 fixture Bloom");
        let material = neutral_smoked_glass().with_glass_optics(Some(optics));
        let bloom_material = material.clone().with_glass_optics(Some(optics.with_bloom(Some(bloom))));
        Self {
            id: WidgetInternalId::new(),
            material,
            bloom_material,
            playback: CrossingPlayback::Pending,
        }
    }

    fn optics(&self) -> &GlassSurfaceOptics {
        self.material.glass_optics().expect("HAL-393 coating")
    }

    fn fail(&mut self, cx: &mut UpdateContext, reason: impl std::fmt::Display) {
        if let CrossingPlayback::Playing(handles) = self.playback {
            for handle in handles {
                cx.cancel_animation(handle);
            }
        }
        self.playback = CrossingPlayback::Failed;
        eprintln!("HAL-393 reflection playback refused: {reason}");
        cx.invalidate_paint();
    }

    fn face(rect: RectF) -> RectF {
        RectF::new(
            rect.right() - CROSSING_FACE.width,
            rect.y + CROSSING_CAPTION,
            CROSSING_FACE.width,
            CROSSING_FACE.height,
        )
    }

    fn bloom_face(rect: RectF) -> RectF {
        RectF::new(rect.x, rect.y + 160.0, CROSSING_FACE.width, 136.0)
    }
}

impl WidgetDowncast for CrossingReflections {
    downcast!();
}

impl Widget for CrossingReflections {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn needs_update(&self) -> bool {
        matches!(
            self.playback,
            CrossingPlayback::Pending | CrossingPlayback::Playing(_)
        )
    }

    fn update(&mut self, _: f32, cx: &mut UpdateContext) {
        if self.playback == CrossingPlayback::Pending {
            match start_crossing(cx) {
                Ok(handles) => {
                    self.playback = CrossingPlayback::Playing(handles);
                    println!("HAL-393 reflection playback started");
                }
                Err(error) => self.fail(cx, error),
            }
        }
        let CrossingPlayback::Playing(handles) = self.playback else {
            return;
        };
        let (centers, finished) = match handles.map(|handle| cx.sample_animation::<Point>(handle)) {
            [
                Some(AnimationSample::Active(warm)),
                Some(AnimationSample::Active(cool)),
            ] => ([warm, cool], false),
            [
                Some(AnimationSample::Completed(warm)),
                Some(AnimationSample::Completed(cool)),
            ] => ([warm, cool], true),
            samples => {
                self.fail(cx, format_args!("incomplete light pair {samples:?}"));
                return;
            }
        };
        if centers != self.optics().lights().map(AnalyticSurfaceLight::center_uv) {
            match crossing_optics(centers) {
                Ok(optics) => {
                    let bloom = self.bloom_material.glass_optics().and_then(GlassSurfaceOptics::bloom);
                    self.bloom_material = self.bloom_material.clone()
                        .with_glass_optics(Some(optics.with_bloom(bloom)));
                    self.material = self.material.clone().with_glass_optics(Some(optics));
                    cx.invalidate_paint();
                }
                Err(error) => {
                    self.fail(cx, error);
                    return;
                }
            }
        }
        if finished {
            self.playback = CrossingPlayback::Finished;
            println!("HAL-393 reflection playback complete");
        }
    }

    fn layout(&mut self, _: &mut LayoutContext, k: Constraints, _: &[NodeId]) -> LayoutResult {
        LayoutResult::fit(k.constrain(CROSSING_COMPARISON))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        if let Err(error) = cx.scene.push_analytic_surface(
            Self::face(rect),
            RoundedCorners::uniform(12.0),
            &self.material,
            40,
            None,
        ) {
            eprintln!("HAL-393 reflection surface refused: {error}");
        }
        let label = if self.playback == CrossingPlayback::Failed {
            "Crossing reflections unavailable"
        } else {
            "Crossing reflections · 2.4 s"
        };
        text(cx, label, Self::face(rect).x, rect.y + 5.0, 16.0, Color::WHITE);
        let bloom = Self::bloom_face(rect);
        if let Err(error) = cx.scene.push_analytic_surface(
            bloom, RoundedCorners::uniform(12.0), &self.bloom_material, 41, None,
        ) {
            eprintln!("HAL-395 comparison surface refused: {error}");
        }
        text(cx, "Selective Bloom", bloom.x + 12.0, bloom.y + 9.0, 16.0, Color::WHITE);
    }
}

struct ThinFilmComparison {
    id: WidgetInternalId,
    materials: [SurfaceAnalytic; 2],
    swapped: bool,
}

impl ThinFilmComparison {
    fn new() -> Self {
        let body = neutral_smoked_glass();
        let film = ThinFilmParams::try_new(1.0, 320.0, 1.4).expect("HAL-394 fixture film");
        let materials = [5.0_f64, 80.0].map(|elevation| {
            let (z, x) = elevation.to_radians().sin_cos();
            let light = AnalyticSurfaceLight::try_new(
                [x as f32, 0.0, z as f32],
                Color::WHITE,
                1.0,
                Point::new(0.5, 0.5),
                Point::new(0.35, 0.38),
                Point::new(1.0, 0.0),
            )
            .expect("HAL-394 white incident source");
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
                [light, body.lights()[1]],
                Color::WHITE,
                Color::WHITE,
            )
            .expect("HAL-394 angular comparison coating")
            .try_with_thin_film(Some(film))
            .expect("HAL-394 checked film selection");
            body.clone().with_glass_optics(Some(optics))
        });
        Self { id: WidgetInternalId::new(), materials, swapped: false }
    }

    fn faces(rect: RectF) -> [RectF; 2] {
        let width = (rect.width - 12.0) * 0.5;
        [rect.x, rect.x + width + 12.0]
            .map(|x| RectF::new(x, rect.y + 32.0, width, rect.height - 32.0))
    }
}

impl WidgetDowncast for ThinFilmComparison {
    downcast!();
}

impl Widget for ThinFilmComparison {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn layout(&mut self, cx: &mut LayoutContext, k: Constraints, children: &[NodeId]) -> LayoutResult {
        let size = k.constrain(THIN_FILM_COMPARISON);
        let control = THIN_FILM_SWAP_CONTROL;
        for &child in children {
            cx.layout_child(child, Constraints::tight(control));
            cx.set_child_rect(child, RectF::new(size.width - control.width, 0.0, control.width, control.height));
        }
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        text(cx, "Thin-film glass", rect.x, rect.y + 5.0, 16.0, Color::WHITE);
        let control = THIN_FILM_SWAP_CONTROL;
        // The native checkbox uses the theme's text color, so give it the matching surface.
        cx.scene.push_rounded_rect(
            RectF::new(
                rect.x + rect.width - control.width - 6.0,
                rect.y,
                control.width + 6.0,
                control.height,
            ),
            cx.theme.colors.surfaces.base0,
            RoundedCorners::uniform(4.0),
            0,
            None,
        );
        let order = if self.swapped { [1, 0] } else { [0, 1] };
        for (face, index) in Self::faces(rect).into_iter().zip(order) {
            if let Err(error) = cx.scene.push_analytic_surface(
                face, RoundedCorners::uniform(12.0), &self.materials[index], 40, None,
            ) {
                eprintln!("HAL-394 comparison surface refused: {error}");
            }
            text(cx, ["5°", "80°"][index], face.x + 12.0, face.y + face.height - 22.0, 14.0, Color::WHITE);
        }
    }
}

struct StatusView {
    id: WidgetInternalId,
    value: Presentation,
    snapshot: Vec<String>,
}
impl StatusView {
    fn new(value: Presentation) -> Self {
        let mut p = Self {
            id: WidgetInternalId::new(),
            value,
            snapshot: Vec::new(),
        };
        p.describe();
        p
    }
    fn describe(&mut self) {
        self.snapshot.clear();
        let mut words = String::new();
        for c in CONTROLS {
            words.push_str(&format!(
                "{}={:.4}  ",
                c.key,
                self.value.request.get(c.field)
            ));
        }
        words.push_str("grain seed=387. ");
        words.push_str(&format!("Checked={:?}", self.value.checked));
        let mut line = String::new();
        for word in words.split_whitespace() {
            if line.len() + word.len() > 112 {
                self.snapshot.push(std::mem::take(&mut line));
            }
            line.push_str(word);
            line.push(' ');
        }
        if !line.is_empty() {
            self.snapshot.push(line);
        }
    }
    fn set_value(&mut self, value: Presentation) -> bool {
        if self.value == value {
            return false;
        }
        self.value = value;
        self.describe();
        true
    }
}
impl WidgetDowncast for StatusView {
    downcast!();
}
impl Widget for StatusView {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        false
    }
    fn layout(&mut self, _: &mut LayoutContext, k: Constraints, _: &[NodeId]) -> LayoutResult {
        LayoutResult::fit(k.constrain(Size::new(CANVAS.width, CANVAS.height - STATUS_TOP)))
    }
    fn paint(&self, c: &mut PaintContext, r: RectF, _: &[NodeId]) {
        let status = match &self.value.checked {
            Ok(_) => "VALID: editable values rendered",
            Err(e) => e.as_str(),
        };
        text(
            c,
            status,
            r.x + 14.0,
            r.y + 16.0,
            15.0,
            if self.value.checked.is_ok() {
                Color::from_hex(0xa6dfcb)
            } else {
                Color::from_hex(0xffb9a0)
            },
        );
        text(
            c,
            "Frosted-only: blur, transmission, tint alpha, displacement. Smoked never reads the backdrop.",
            r.x + 14.0,
            r.y + 41.0,
            12.0,
            Color::WHITE,
        );
        for (row, line) in self.snapshot.iter().enumerate() {
            text(
                c,
                line,
                r.x + 14.0,
                r.y + 72.0 + row as f32 * 15.0,
                11.0,
                Color::from_hex(0xb7cad7),
            );
        }
    }
}

struct Lab {
    placeholder: Label,
    binder: FormBinder,
    request: Request,
    pending: Option<Presentation>,
    preview: Option<TreeWidgetId>,
    backgrounds: [Option<TreeWidgetId>; 6],
    status: Option<TreeWidgetId>,
    thin_film_comparison: Option<TreeWidgetId>,
    pending_film_swap: Option<bool>,
    publication_failed: bool,
}
impl Lab {
    fn new() -> Self {
        let request = Request::initial();
        println!("HAL_OPTICS_S2 initial={:?}", request);
        Self {
            placeholder: Label::new(""),
            binder: FormBinder::new(),
            request,
            pending: None,
            preview: None,
            backgrounds: [None; 6],
            status: None,
            thin_film_comparison: None,
            pending_film_swap: None,
            publication_failed: false,
        }
    }
}
fn add_scroll(cx: &mut ScreenFrameContext, parent: NodeId, index: i16) -> NodeId {
    cx.tree.add_child(
        parent,
        Box::new(ScrollArea::new_unwired().enable_horizontal_scrollbar(true)),
        None,
        index,
    )
}
fn wire_scroll(tree: &mut WidgetTree, scroll: NodeId, content: NodeId) {
    tree.node_mut(scroll)
        .expect("new ScrollArea mount")
        .widget_mut()
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .expect("qualified ScrollArea type")
        .wire_content(content);
}
impl Screen for Lab {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(
            Flex::row()
                .with_padding(EdgeInsets::all(12.0))
                .with_spacing(12.0)
                .with_align_items(Alignment::Stretch),
        ));
        let controls = add_scroll(cx, root, 0);
        cx.tree.set_flex(controls, FlexParams::fixed(330.0));
        let col = cx.tree.add_child(
            controls,
            Box::new(
                Flex::column()
                    .with_spacing(10.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            0,
        );
        wire_scroll(cx.tree, controls, col);
        cx.tree
            .add_child(col, Box::new(Label::new("EDITABLE MATERIAL")), None, 0);
        for (index, (key, options, selection)) in [
            (
                "material",
                [
                    (RadioId(0), "Smoked / no backdrop"),
                    (RadioId(1), "Frosted / current backdrop"),
                ],
                self.request.material,
            ),
            (
                "background",
                [
                    (RadioId(0), "Cool / warm shaded forms"),
                    (RadioId(1), "Ordinary interface content"),
                ],
                self.request.background,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let mut group = RadioGroup::with_labeled_options(
                RadioGroupId(if key == "material" { 386 } else { 385 }),
                &options,
            );
            group.set_selected(selection.map(RadioId));
            group.set_binding(key, self.binder.writer());
            cx.tree
                .add_child(col, Box::new(group), None, index as i16 + 1);
        }
        for (index, (key, label, checked)) in [
            (
                "coating",
                "Authored reflection and rim",
                self.request.coating,
            ),
            (
                "emission",
                "Independent surface emission",
                self.request.emission,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let mut check = Checkbox::checked(checked).with_label(label);
            check.set_binding(key, self.binder.writer());
            cx.tree
                .add_child(col, Box::new(check), None, index as i16 + 3);
        }
        for (index, control) in CONTROLS.iter().enumerate() {
            let mut slider = Slider::new(control.min, control.max)
                .with_label(control.label)
                .with_value(self.request.get(control.field))
                .with_value_display()
                .with_value_formatter(|v| format!("{v:.3}"));
            if control.step > 0.0 {
                slider = slider.with_step(control.step);
            }
            slider.set_binding(control.key, self.binder.writer());
            let tab = i16::try_from(index + 5).expect("finite control table fits tab order");
            let node = cx.tree.add_child(col, Box::new(slider), None, tab);
            cx.tree
                .set_diagnostic_hint(node, format!("Glass material {}", control.key));
        }
        let area = add_scroll(cx, root, 1);
        cx.tree.set_flex(area, FlexParams::flexible(1.0));
        let canvas = cx.tree.add_child(
            area,
            Box::new(Canvas {
                id: WidgetInternalId::new(),
            }),
            None,
            0,
        );
        wire_scroll(cx.tree, area, canvas);
        let profiles = [
            Material::None,
            fixed_profile(0.0, 0.985, false, true),
            fixed_profile(9.0, 0.94, false, true),
            fixed_profile(2.0, 0.97, true, true),
            fixed_profile(9.0, 0.94, false, false),
        ];
        for i in 0..6 {
            let host = cx.tree.add_child(
                canvas,
                Box::new(CellHost {
                    id: WidgetInternalId::new(),
                }),
                None,
                i as i16,
            );
            cx.tree.set_overflow(host, OverflowMode::Clip);
            let bg = cx.tree.add_child(
                host,
                Box::new(Background::new(i, self.request.background)),
                None,
                0,
            );
            self.backgrounds[i] = Some(cx.tree.node(bg).expect("new background mount").id());
            let (checked, corner) = if i < 5 {
                (Ok(profiles[i].clone()), 18.0)
            } else {
                (self.request.checked(), self.request.get(K::Corner))
            };
            let material = cx.tree.add_child(
                host,
                Box::new(MaterialView::new(i, checked, corner)),
                None,
                1,
            );
            if i == 5 {
                self.preview = Some(cx.tree.node(material).expect("new editable mount").id());
            }
        }
        cx.tree.add_child(
            canvas,
            Box::new(Header {
                id: WidgetInternalId::new(),
            }),
            None,
            6,
        );
        cx.tree.add_child(
            canvas,
            Box::new(ActionSource {
                id: WidgetInternalId::new(),
                value: action_source(),
            }),
            None,
            7,
        );
        let status = cx.tree.add_child(
            canvas,
            Box::new(StatusView::new(Presentation {
                request: self.request.clone(),
                checked: self.request.checked(),
            })),
            None,
            8,
        );
        self.status = Some(cx.tree.node(status).expect("new status mount").id());
        cx.tree
            .add_child(canvas, Box::new(CrossingReflections::new()), None, 9);
        let film = cx.tree.add_child(canvas, Box::new(ThinFilmComparison::new()), None, 10);
        self.thin_film_comparison = Some(cx.tree.node(film).expect("thin-film comparison mount").id());
        let mut swap = Checkbox::checked(false).with_label("Swap angles");
        swap.set_binding("swap_film_angles", self.binder.writer());
        cx.tree.add_child(film, Box::new(swap), None, 0);
        &mut self.placeholder
    }
    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let mut changed = false;
        self.binder.bind(cx, |b| {
            for control in CONTROLS {
                if let Some(v) = b.f32(control.key).take() {
                    let target = &mut self.request.values[control.field as usize];
                    if *target != v {
                        *target = v;
                        changed = true;
                    }
                }
            }
            if let Some(v) = b.selection("material").take() {
                changed |= self.request.material != v;
                self.request.material = v;
            }
            if let Some(v) = b.selection("background").take() {
                changed |= self.request.background != v;
                self.request.background = v;
            }
            if let Some(v) = b.bool("coating").take() {
                changed |= self.request.coating != v;
                self.request.coating = v;
            }
            if let Some(v) = b.bool("emission").take() {
                changed |= self.request.emission != v;
                self.request.emission = v;
            }
            if let Some(swapped) = b.bool("swap_film_angles").take() {
                self.pending_film_swap = Some(swapped);
            }
        });
        if changed {
            let checked = self.request.checked();
            println!(
                "HAL_OPTICS_S2 requested={:?} validation={}",
                self.request,
                if checked.is_ok() { "valid" } else { "refused" }
            );
            self.pending = Some(Presentation {
                request: self.request.clone(),
                checked,
            });
        }
    }
    fn update(&mut self, tree: &mut WidgetTree) {
        if let Some(swapped) = self.pending_film_swap.take()
            && let Some(node) = self.thin_film_comparison.and_then(|id| tree.find_by_id(id))
            && let Some(comparison) = tree.node_mut(node)
                .and_then(|n| n.widget_mut().as_any_mut().downcast_mut::<ThinFilmComparison>())
            && comparison.swapped != swapped
        {
            comparison.swapped = swapped;
            tree.mark_paint_dirty(node);
        }
        let Some(value) = self.pending.as_ref() else {
            return;
        };
        let targets = self
            .preview
            .and_then(|id| tree.find_by_id(id))
            .zip(self.status.and_then(|id| tree.find_by_id(id)))
            .zip(
                self.backgrounds
                    .iter()
                    .map(|id| id.and_then(|id| tree.find_by_id(id)))
                    .collect::<Option<Vec<_>>>(),
            );
        let Some(((preview, status), backgrounds)) = targets else {
            if !self.publication_failed {
                eprintln!("HAL386 presentation mount unavailable; value not published");
                self.publication_failed = true;
            }
            return;
        };
        // Qualify every concrete target before publishing any part of this value.
        let matches = tree
            .node(preview)
            .is_some_and(|n| n.widget().as_any().is::<MaterialView>())
            && tree
                .node(status)
                .is_some_and(|n| n.widget().as_any().is::<StatusView>())
            && backgrounds.iter().all(|&bg| {
                tree.node(bg)
                    .is_some_and(|n| n.widget().as_any().is::<Background>())
            });
        if !matches {
            if !self.publication_failed {
                eprintln!("HAL386 presentation concrete type changed; value not published");
                self.publication_failed = true;
            }
            return;
        }
        let repaint = tree
            .node_mut(preview)
            .expect("qualified editable mount")
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<MaterialView>()
            .expect("qualified editable type")
            .set_value(value);
        let statuspaint = tree
            .node_mut(status)
            .expect("qualified status mount")
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<StatusView>()
            .expect("qualified status type")
            .set_value(value.clone());
        let mut changed_backgrounds = Vec::new();
        for bg in backgrounds {
            if tree
                .node_mut(bg)
                .expect("qualified background mount")
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<Background>()
                .expect("qualified background type")
                .set_kind(value.request.background)
            {
                changed_backgrounds.push(bg);
            }
        }
        // All presentation borrows have ended. Only changed local paint is dirty.
        if repaint {
            tree.mark_paint_dirty(preview);
        }
        if statuspaint {
            tree.mark_paint_dirty(status);
        }
        let bgpaint = !changed_backgrounds.is_empty();
        for bg in changed_backgrounds {
            tree.mark_paint_dirty(bg);
        }
        self.pending = None;
        self.publication_failed = false;
        println!(
            "HAL_OPTICS_S2 published preview={} background={} status={} rebuilt=false",
            repaint, bgpaint, statuspaint
        );
    }
}
fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();
    App::new(AppConfig::new("Glass Material Lab / HAL386 S2").with_size(1280.0, 820.0))
        .run(Lab::new());
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use lab_framework::core::{
        app::{AppContext, EngineTicker, ModalStack, ScreenStack, build_scene_with_appearance},
        event::{InteractionDomains, KeyboardAuthorityMode, WindowId},
        icon::IconRegistry,
        layout_transition::{ActiveLayoutTransitions, InterpolatedVisualRects},
        scene::{SceneBuilder, SceneGraph, SceneNode},
        theme::Theme,
        transition::TransitionLayer,
    };
    use lab_framework::platform::font_manager_text_engine::FontManagerTextEngine;
    use std::time::{Duration, Instant};

    /// Public Screen construction and the ordinary scene/Update/Timeline route.
    /// The public scene helper creates a fresh cache; this is not cache-reuse evidence.
    struct Fixture {
        lab: Lab,
        app: AppContext,
        tree: WidgetTree,
        root: NodeId,
        canvas: NodeId,
        area: NodeId,
        text: FontManagerTextEngine,
        icons: IconRegistry,
        transitions: TransitionLayer,
        layout: ActiveLayoutTransitions,
        ticker: EngineTicker,
        start: Instant,
    }
    impl Fixture {
        fn new() -> Self {
            let mut lab = Lab::new();
            let mut app = AppContext::new(Theme::default());
            let mut tree = WidgetTree::new();
            let host = tree.create_root(Box::new(Flex::column()), None, 0);
            lab.build(&mut ScreenFrameContext {
                app: &mut app,
                tree: &mut tree,
                parent_node: host,
            });
            let root = tree.children(host)[0];
            let area = tree.children(root)[1];
            let canvas = tree.children(area)[0];
            let start = Instant::now();
            Self {
                lab,
                app,
                tree,
                root,
                canvas,
                area,
                text: FontManagerTextEngine::new_with_system_ui(),
                icons: IconRegistry::new(&[]),
                transitions: TransitionLayer::new(),
                layout: ActiveLayoutTransitions::new(),
                ticker: EngineTicker::new(start),
                start,
            }
        }
        fn frame(&mut self, millis: u64) -> SceneGraph {
            self.frame_with_text(millis, None)
        }
        fn frame_with_text(
            &mut self,
            millis: u64,
            text: Option<&mut dyn lab_framework::core::text_engine::TextEngine>,
        ) -> SceneGraph {
            self.frame_at_scale(millis, text, 2.0, None)
        }
        fn frame_at_scale(
            &mut self,
            millis: u64,
            text: Option<&mut dyn lab_framework::core::text_engine::TextEngine>,
            scale: f32,
            cancel: Option<TransitionHandle>,
        ) -> SceneGraph {
            let timing = self
                .ticker
                .begin_frame_at(self.start + Duration::from_millis(millis));
            self.transitions.advance(millis as f64 / 1000.0);
            if let Some(handle) = cancel {
                self.transitions.cancel_animation(handle);
            }
            self.tree.begin_layout_transition_frame();
            build_scene_with_appearance(
                &mut self.tree,
                self.root,
                Constraints::tight(Size::new(1280.0, 820.0)),
                &self.app.theme,
                text.unwrap_or(&mut self.text),
                &self.icons,
                self.ticker.frame_time_ms(timing.now),
                timing.dt.as_secs_f32(),
                &mut self.transitions,
                &mut self.layout,
                None,
                scale,
                None,
                None,
                false,
                &mut Vec::new(),
                None,
                &InteractionDomains::new(),
                WindowId::PRIMARY,
            )
        }
        fn cells(&self) -> Vec<NodeId> {
            self.tree.children(self.canvas)[..6].to_vec()
        }
        fn crossing_node(&self) -> NodeId {
            self.tree.children(self.canvas)[9]
        }
        fn crossing(&self) -> &CrossingReflections {
            self.tree
                .node(self.crossing_node())
                .unwrap()
                .widget()
                .as_any()
                .downcast_ref::<CrossingReflections>()
                .unwrap()
        }
        fn backgrounds(&self) -> Vec<NodeId> {
            self.lab
                .backgrounds
                .iter()
                .map(|id| self.tree.find_by_id(id.unwrap()).unwrap())
                .collect()
        }
        fn material(&self, cell: usize) -> &MaterialView {
            self.tree
                .node(self.tree.children(self.cells()[cell])[1])
                .unwrap()
                .widget()
                .as_any()
                .downcast_ref::<MaterialView>()
                .unwrap()
        }
        fn flush(&mut self) {
            self.lab.flush_pending_actions(&mut ScreenContext::new(
                &mut self.app,
                &mut ScreenStack::new(),
                &mut ModalStack::new(),
                Some(&mut self.tree),
            ));
        }
    }

    fn translate(r: RectF, offset: (f32, f32)) -> RectF {
        RectF::new(r.x + offset.0, r.y + offset.1, r.width, r.height)
    }

    /// Shared only by owner tests: real Lab/Timeline scenes, not a second producer.
    pub(crate) fn temporal_scenes() -> Vec<(
        u64,
        RectF,
        Vec<lab_framework::core::scene::GradientRectNode>,
        SceneGraph,
    )> {
        let mut f = Fixture::new();
        [
            0, 16, 32, 48, 64, 80, 96, 112, 128, 144, 160, 176, 192, 208, 224, 240, 800, 2000,
            7440, 7456, 7472, 12000,
        ]
        .into_iter()
        .map(|ms| {
            let scene = f.frame(ms);
            let b = f.tree.get_absolute_paint_rect(f.cells()[0]).unwrap();
            let first_phase = f
                .tree
                .node(f.backgrounds()[0])
                .unwrap()
                .widget()
                .as_any()
                .downcast_ref::<Background>()
                .unwrap()
                .phase;
            for id in f.backgrounds() {
                assert_eq!(
                    f.tree
                        .node(id)
                        .unwrap()
                        .widget()
                        .as_any()
                        .downcast_ref::<Background>()
                        .unwrap()
                        .phase,
                    first_phase
                );
            }
            let mut gradients = Vec::new();
            walk(
                &scene.root,
                &mut Vec::new(),
                (0.0, 0.0),
                &mut |n, clips, _| {
                    if let SceneNode::GradientRect(g) = n {
                        if g.rect.width == 190.0 && g.rect.height == 190.0 && clips.contains(&b) {
                            gradients.push(g.clone());
                        }
                    }
                },
            );
            assert_eq!(gradients.len(), 2);
            (ms, b, gradients, scene)
        })
        .collect()
    }
    fn walk(
        nodes: &[SceneNode],
        clips: &mut Vec<RectF>,
        offset: (f32, f32),
        visit: &mut impl FnMut(&SceneNode, &[RectF], (f32, f32)),
    ) {
        for node in nodes {
            visit(node, clips, offset);
            match node {
                SceneNode::RetainedFragment(n) => walk(&n.children, clips, offset, visit),
                SceneNode::Layer(n) => walk(&n.children, clips, offset, visit),
                SceneNode::Clip(n) => {
                    clips.push(translate(n.rect, offset));
                    walk(&n.children, clips, offset, visit);
                    clips.pop();
                }
                SceneNode::Scroll(n) => walk(
                    &n.children,
                    clips,
                    (offset.0 - n.offset.0, offset.1 - n.offset.1),
                    visit,
                ),
                _ => {}
            }
        }
    }

    const CROSSING_TIMES: [u64; 5] = [0, 600, 1200, 1800, 2401];

    #[test]
    fn glass_material_lab_crossing_uses_two_finite_tracks_and_refuses_partial_pairs() {
        let mut f = Fixture::new();
        let mut other_cadence = Fixture::new();
        other_cadence.frame(0);
        let mut handles = None;
        for (millis, x) in CROSSING_TIMES
            .into_iter()
            .zip([0.20, 0.35, 0.50, 0.65, 0.80])
        {
            f.frame(millis);
            let crossing = f.crossing();
            let centers = crossing
                .optics()
                .lights()
                .map(AnalyticSurfaceLight::center_uv);
            for (center, expected_x) in centers.into_iter().zip([x, 1.0 - x]) {
                assert!((center.x - expected_x).abs() < 1e-6);
                assert_eq!(center.y, 0.50);
            }
            if millis == 0 {
                let CrossingPlayback::Playing(pair) = crossing.playback else {
                    panic!("two starts")
                };
                assert_ne!(pair[0], pair[1]);
                handles = Some(pair);
            } else if millis < 2401 {
                assert_eq!(
                    crossing.playback,
                    CrossingPlayback::Playing(handles.unwrap())
                );
                other_cadence.frame(millis - 117);
                other_cadence.frame(millis);
                assert_eq!(crossing.material, other_cadence.crossing().material);
            }
        }
        let final_material = f.crossing().material.clone();
        assert_eq!(f.crossing().playback, CrossingPlayback::Finished);
        assert_eq!(
            f.crossing()
                .optics()
                .lights()
                .map(AnalyticSurfaceLight::center_uv),
            [CROSSING_START[1], CROSSING_START[0]]
        );
        assert!(!f.crossing().needs_update() && !f.crossing().needs_animation());
        f.frame(12_100);
        assert_eq!(f.crossing().material, final_material);
        assert!(!f.transitions.has_active_animations());

        // Cancellation, a retired handle, a wrong payload, and an out-of-domain
        // center each leave the last complete material and end both tracks.
        for case in 0..4 {
            let mut f = Fixture::new();
            f.frame(0);
            f.frame(600);
            let before = f.crossing().material.clone();
            let CrossingPlayback::Playing([warm, cool]) = f.crossing().playback else {
                panic!("two active tracks")
            };
            if case == 0 {
                f.frame_at_scale(900, None, 2.0, Some(cool));
            } else {
                f.transitions.cancel_animation(cool);
                if case >= 2 {
                    let mut context = f.transitions.animation_context();
                    let replacement = if case == 2 {
                        context.start(Animation::new(0.0_f32, 1.0, 2.4, Easing::Linear))
                    } else {
                        let invalid = Point::new(1.1, 0.5);
                        context.start(Animation::new(invalid, invalid, 2.4, Easing::Linear))
                    }
                    .unwrap();
                    let node = f.crossing_node();
                    f.tree
                        .node_mut(node)
                        .unwrap()
                        .widget_mut()
                        .as_any_mut()
                        .downcast_mut::<CrossingReflections>()
                        .unwrap()
                        .playback = CrossingPlayback::Playing([warm, replacement]);
                }
                f.frame(900);
            }
            assert_eq!(f.crossing().material, before, "case {case}");
            assert_eq!(
                f.crossing().playback,
                CrossingPlayback::Failed,
                "case {case}"
            );
            assert!(!f.crossing().needs_update());
            assert_eq!(
                f.transitions.timeline().sample::<Point>(warm),
                AnimationSample::Cancelled
            );
        }

        let mut f = Fixture::new();
        f.frame(0);
        let CrossingPlayback::Playing(pair) = f.crossing().playback else {
            panic!("two starts")
        };
        let node = f.crossing_node();
        f.tree.remove_node(node);
        f.frame(16);
        for handle in pair {
            assert!(matches!(
                f.transitions.timeline().sample::<Point>(handle),
                AnimationSample::Cancelled | AnimationSample::Missing
            ));
        }
    }

    pub(crate) fn crossing_frames(
        scale: f32,
        text: &mut dyn lab_framework::core::text_engine::TextEngine,
    ) -> Vec<(u64, SceneGraph, RectF, GlassSurfaceOptics)> {
        let mut f = Fixture::new();
        let mut frames = Vec::new();
        for millis in CROSSING_TIMES.into_iter().chain([2500]) {
            let scene = f.frame_at_scale(millis, Some(&mut *text), scale, None);
            let own_rect = f.tree.get_absolute_paint_rect(f.crossing_node()).unwrap();
            let face = CrossingReflections::face(own_rect);
            let mut optics = None;
            let mut caption = false;
            walk(
                &scene.root,
                &mut Vec::new(),
                (0.0, 0.0),
                &mut |node, clips, offset| match node {
                    SceneNode::AnalyticSurface(surface) if surface.rect() == face => {
                        assert_eq!(offset, (0.0, 0.0));
                        assert!(
                            clips
                                .iter()
                                .any(|clip| clip.contains(Point::new(face.x, face.y)))
                        );
                        assert_eq!(surface.corners(), RoundedCorners::uniform(12.0));
                        assert!(
                            optics
                                .replace(surface.material().glass_optics().unwrap().clone())
                                .is_none()
                        );
                    }
                    SceneNode::Text(text) if text.text == "Crossing reflections · 2.4 s" => {
                        assert!(text.z_index > 40);
                        caption = true;
                    }
                    _ => {}
                },
            );
            assert!(caption, "sharp caption stays in the ordinary text path");
            frames.push((millis, scene, face, optics.unwrap()));
        }
        frames
    }

    pub(crate) fn thin_film_frames(
        scale: f32,
        text: &mut dyn lab_framework::core::text_engine::TextEngine,
    ) -> ([SceneGraph; 2], [RectF; 2], [GlassSurfaceOptics; 2]) {
        let mut f = Fixture::new();
        let first = f.frame_at_scale(0, Some(&mut *text), scale, None);
        let comparison = f.tree.children(f.canvas)[10];
        let rect = f.tree.get_absolute_paint_rect(comparison).unwrap();
        let faces = ThinFilmComparison::faces(rect);
        let widget = f.tree.node(comparison).unwrap().widget();
        assert!(!widget.needs_update() && !widget.needs_animation());
        let optics = widget.as_any().downcast_ref::<ThinFilmComparison>().unwrap()
            .materials.each_ref().map(|material| *material.glass_optics().unwrap());
        let unaffected = f.cells().into_iter().map(|cell| f.tree.children(cell)[1]).collect::<Vec<_>>();

        let writer = f.lab.binder.writer();
        writer.set_bool(writer.intern("swap_film_angles"), true);
        f.flush();
        f.lab.update(&mut f.tree);
        assert!(f.tree.node(comparison).unwrap().dirty_paint);
        assert!(unaffected.iter().all(|&id| !f.tree.node(id).unwrap().dirty_paint));
        let swapped = f.frame_at_scale(16, Some(&mut *text), scale, None);
        assert_eq!(f.tree.get_absolute_paint_rect(comparison), Some(rect));
        writer.set_bool(writer.intern("swap_film_angles"), true);
        f.flush();
        f.lab.update(&mut f.tree);
        assert!(!f.tree.node(comparison).unwrap().dirty_paint);

        for (frame, order) in [(&first, [0, 1]), (&swapped, [1, 0])] {
            let mut seen = [false; 2];
            let mut labels = 0;
            walk(&frame.root, &mut Vec::new(), (0.0, 0.0), &mut |node, clips, offset| {
                match node {
                    SceneNode::AnalyticSurface(surface) => {
                        if let Some(index) = faces.iter().position(|face| *face == surface.rect()) {
                            let face = faces[index];
                            assert_eq!(offset, (0.0, 0.0));
                            assert!(clips.iter().any(|clip| clip.contains(Point::new(face.x, face.y))));
                            assert_eq!(surface.corners(), RoundedCorners::uniform(12.0));
                            assert_eq!(surface.material().glass_optics(), Some(&optics[order[index]]));
                            assert!(!seen[index]);
                            seen[index] = true;
                        }
                    }
                    SceneNode::Text(text) if ["5°", "80°"].contains(&text.text.as_str()) => {
                        assert!(text.z_index > 40);
                        labels += 1;
                    }
                    _ => {}
                }
            });
            assert_eq!(seen, [true; 2]);
            assert_eq!(labels, 2);
        }
        ([first, swapped], faces, optics)
    }

    #[test]
    fn glass_material_lab_traversal_owns_six_cell_clips_and_scroll_once() {
        let mut f = Fixture::new();
        let mut scene = f.frame(0);
        assert_eq!(f.tree.children(f.canvas).len(), 11);
        for scroll in [0.0, 48.0] {
            if scroll > 0.0 {
                f.tree
                    .node_mut(f.area)
                    .unwrap()
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<ScrollArea>()
                    .unwrap()
                    .scroll_to(0.0, scroll);
                scene = f.frame(16);
            }
            let origin = f.tree.get_absolute_paint_rect(f.canvas).unwrap();
            let actual_scroll = f
                .tree
                .node(f.area)
                .unwrap()
                .widget()
                .scroll_offset()
                .unwrap();
            assert_eq!(
                actual_scroll,
                (0.0, scroll),
                "scroll_to publishes its actual visual offset"
            );
            let cells = f.cells();
            let mut expected = Vec::new();
            for (i, &host) in cells.iter().enumerate() {
                let n = f.tree.node(host).unwrap();
                assert!(n.widget().as_any().is::<CellHost>());
                assert_eq!(n.overflow, OverflowMode::Clip);
                assert!(!n.widget().hit_testable());
                assert_eq!(f.tree.children(host).len(), 2);
                let b = cell(origin, i);
                assert_eq!(f.tree.get_absolute_paint_rect(host), Some(b));
                for child in f.tree.children(host) {
                    assert_eq!(f.tree.parent(child), Some(host));
                    assert_eq!(f.tree.get_absolute_paint_rect(child), Some(b));
                    assert!(!f.tree.node(child).unwrap().widget().hit_testable());
                }
                let accumulated = f.tree.cumulative_scroll_offset(host);
                assert_eq!(accumulated, actual_scroll);
                expected.push(translate(b, (-accumulated.0, -accumulated.1)));
            }
            let mut seen_clip = [0; 6];
            let mut seen_bg = [0; 6];
            let mut seen_material = [0; 6];
            let mut seen_foreground = [0; 6];
            walk(
                &scene.root,
                &mut Vec::new(),
                (0.0, 0.0),
                &mut |n, clips, offset| {
                    if let SceneNode::Clip(n) = n {
                        for (i, b) in expected.iter().enumerate() {
                            if translate(n.rect, offset) == *b {
                                seen_clip[i] += 1;
                            }
                        }
                    }
                    for (i, b) in expected.iter().enumerate() {
                        let material_z = 10 + i as u32;
                        match n {
                            SceneNode::Rect(n)
                                if n.z_index == 1 && translate(n.rect, offset) == *b =>
                            {
                                assert!(
                                    clips.contains(b),
                                    "cell {i}, scroll {scroll}: expected traversal clip {b:?}, actual ancestry {clips:?}"
                                );
                                seen_bg[i] += 1;
                            }
                            SceneNode::FrostedGlass(n) if n.z_index() == material_z => {
                                assert!(clips.contains(b));
                                assert_eq!(translate(n.rect(), offset), face(*b));
                                seen_material[i] += 1;
                            }
                            SceneNode::Text(n) if n.z_index == 100 && clips.contains(b) => {
                                seen_foreground[i] += 1
                            }
                            _ => {}
                        }
                    }
                },
            );
            assert_eq!(seen_clip, [1; 6]);
            assert_eq!(seen_bg, [1; 6]);
            assert_eq!(seen_material, [0, 1, 1, 1, 1, 1]);
            assert!(seen_foreground.iter().all(|&n| n == 3));
        }
    }

    #[test]
    fn glass_material_lab_local_paint_has_no_clips() {
        let mut f = Fixture::new();
        f.frame(0);
        let mut owners = vec![f.canvas];
        owners.extend(f.tree.children(f.canvas));
        for host in f.cells() {
            owners.extend(f.tree.children(host));
        }
        for owner in owners {
            let mut scene = SceneBuilder::new();
            let mut overlays = Vec::new();
            let mut tooltips = Vec::new();
            let rects = InterpolatedVisualRects::default();
            let mut cx = PaintContext::new(
                &f.tree,
                &mut scene,
                &f.app.theme,
                &mut f.text,
                &f.icons,
                0,
                f.transitions.timeline(),
                &rects,
                &mut overlays,
                &mut tooltips,
                2.0,
                None,
                KeyboardAuthorityMode::Navigation,
            );
            f.tree.node(owner).unwrap().widget().paint(
                &mut cx,
                f.tree.get_absolute_paint_rect(owner).unwrap(),
                &f.tree.children(owner),
            );
            let local = scene.build();
            walk(&local.root, &mut Vec::new(), (0.0, 0.0), &mut |n, _, _| {
                assert!(
                    !matches!(n, SceneNode::Clip(_) | SceneNode::Scroll(_)),
                    "local paint must not acquire clip or scroll authority"
                );
            });
        }
    }

    #[test]
    fn glass_material_lab_parameter_publication_is_narrow_and_mounts_stay_stable() {
        let mut f = Fixture::new();
        f.frame(0);
        f.frame(12000);
        let cells = f.cells();
        let backgrounds = f.backgrounds();
        let editable = f.tree.find_by_id(f.lab.preview.unwrap()).unwrap();
        let status = f.tree.find_by_id(f.lab.status.unwrap()).unwrap();
        let geometry = cells
            .iter()
            .chain(backgrounds.iter())
            .chain([&editable, &status])
            .map(|&id| {
                (
                    id,
                    f.tree.node(id).unwrap().id(),
                    f.tree.get_absolute_paint_rect(id),
                )
            })
            .collect::<Vec<_>>();
        let fixed = (0..5)
            .map(|i| f.material(i).checked.clone())
            .collect::<Vec<_>>();
        assert_eq!(f.lab.request.get(K::Blur), 9.0);
        let writer = f.lab.binder.writer();
        writer.set_f32(writer.intern("Blur"), 18.75);
        f.flush();
        assert!(f.lab.pending.as_ref().unwrap().checked.is_ok());
        f.lab.update(&mut f.tree);
        assert!(f.tree.node(editable).unwrap().dirty_paint);
        assert!(f.tree.node(status).unwrap().dirty_paint);
        for &id in &backgrounds {
            assert!(!f.tree.node(id).unwrap().dirty_paint);
        }
        for i in 0..5 {
            let id = f.tree.children(cells[i])[1];
            assert!(!f.tree.node(id).unwrap().dirty_paint);
            assert_eq!(f.material(i).checked, fixed[i]);
        }
        f.frame(12016);
        assert_eq!(
            geometry,
            cells
                .iter()
                .chain(backgrounds.iter())
                .chain([&editable, &status])
                .map(|&id| (
                    id,
                    f.tree.node(id).unwrap().id(),
                    f.tree.get_absolute_paint_rect(id)
                ))
                .collect::<Vec<_>>()
        );
        writer.set_f32(writer.intern("Blur"), 18.75);
        f.flush();
        f.lab.update(&mut f.tree);
        assert!(f.lab.pending.is_none());
        assert!(!f.tree.node(f.root).unwrap().dirty_composition);
        writer.set_selection(writer.intern("background"), Some(1));
        f.flush();
        f.lab.update(&mut f.tree);
        assert!(!f.tree.node(editable).unwrap().dirty_paint);
        for id in backgrounds {
            assert!(f.tree.node(id).unwrap().dirty_paint);
        }
        for (i, old) in fixed.iter().enumerate() {
            assert_eq!(&f.material(i).checked, old);
        }
        assert!(!f.lab.take_rebuild_flag());
    }

    #[test]
    fn glass_material_lab_six_owner_timelines_share_phase_and_genuinely_settle() {
        let mut f = Fixture::new();
        let fixed = (0..6)
            .map(|i| f.material(i).checked.clone())
            .collect::<Vec<_>>();
        for (millis, expected) in [
            (0, 0.0),
            (1500, 0.125),
            (8000, 2.0 / 3.0),
            (12000, 1.0),
            (13000, 1.0),
        ] {
            f.frame(millis);
            let backgrounds = f.backgrounds();
            let first = f
                .tree
                .node(backgrounds[0])
                .unwrap()
                .widget()
                .as_any()
                .downcast_ref::<Background>()
                .unwrap();
            assert!((first.phase - expected).abs() < 0.000001);
            for (i, id) in backgrounds.into_iter().enumerate() {
                let bg = f
                    .tree
                    .node(id)
                    .unwrap()
                    .widget()
                    .as_any()
                    .downcast_ref::<Background>()
                    .unwrap();
                assert_eq!(bg.cell, i);
                assert!(bg.started);
                assert!(!bg.failed);
                assert_eq!(bg.phase, first.phase);
                assert_eq!(bg.handle.is_some(), millis < 12000);
                if i > 0 && millis < 12000 {
                    assert_ne!(bg.handle, first.handle);
                }
                assert_eq!(bg.needs_update(), millis < 12000);
                assert_eq!(bg.needs_animation(), millis < 12000);
                assert_eq!(f.material(i).checked, fixed[i]);
            }
            assert_eq!(f.transitions.has_active_animations(), millis < 12000);
            if millis >= 12000 {
                assert!(!f.tree.any_widget_needs_animation());
            }
        }
    }

    fn frosted(material: &Material) -> &FrostedGlassAppearance {
        match material {
            Material::Frosted(value) => value,
            _ => panic!("selected editable Frosted material"),
        }
    }

    pub(crate) struct SideReflectionScene {
        pub scene: SceneGraph,
        pub prefix: SceneGraph,
        pub foreground_scene: SceneGraph,
        pub rect: RectF,
        pub corners: RoundedCorners,
        pub appearance: FrostedGlassAppearance,
        pub foreground: Vec<RectF>,
    }

    /// The existing Lab, checked Binder publication, canonical cell clip and
    /// actual renderer's admitted text engine supply this focused comparison.
    pub(crate) fn side_reflection_scene(
        mode: usize,
        text: &mut dyn lab_framework::core::text_engine::TextEngine,
    ) -> SideReflectionScene {
        let mut f = Fixture::new();
        f.frame_with_text(0, Some(text));
        let writer = f.lab.binder.writer();
        match mode {
            0 => writer.set_bool(writer.intern("coating"), false),
            1 => writer.set_f32(writer.intern("Rim"), 0.0),
            2 => writer.set_f32(writer.intern("Reflection"), 0.0),
            3 => {}
            4 => {
                for (key, value) in [
                    ("Az0", -45.0),
                    ("Az1", -45.0),
                    ("Cx0", 1.0 - f.lab.request.get(K::Cx0)),
                    ("Cx1", 1.0 - f.lab.request.get(K::Cx1)),
                    ("Rot0", 20.0),
                    ("Rot1", 20.0),
                ] {
                    writer.set_f32(writer.intern(key), value);
                }
            }
            5 => {
                for key in [
                    "LightR0", "LightG0", "LightB0", "LightR1", "LightG1", "LightB1",
                ] {
                    writer.set_f32(writer.intern(key), 0.0);
                }
            }
            _ => panic!("unknown authored comparison"),
        }
        f.flush();
        f.lab.update(&mut f.tree);
        let scene = f.frame_with_text(12000, Some(text));
        let cell = f.tree.get_absolute_paint_rect(f.cells()[5]).unwrap();
        let mut clip = None;
        let canvas = f.tree.get_absolute_paint_rect(f.canvas).unwrap();
        let mut canvas_background = None;
        walk(
            &scene.root,
            &mut Vec::new(),
            (0.0, 0.0),
            &mut |node, _, inherited| {
                if let SceneNode::Clip(value) = node {
                    if translate(value.rect, inherited) == cell {
                        assert!(clip.is_none());
                        clip = Some((value.clone(), inherited));
                    }
                }
                if let SceneNode::Rect(value) = node {
                    if value.z_index == 0 && translate(value.rect, inherited) == canvas {
                        assert!(canvas_background.is_none());
                        canvas_background = Some((node.clone(), inherited));
                    }
                }
            },
        );
        let (clip, inherited) = clip.expect("actual editable traversal clip");
        let offset = (cell.x - 16.0 - inherited.0, cell.y - 16.0 - inherited.1);
        let mut appearance = None;
        let mut foreground = Vec::new();
        walk(
            &clip.children,
            &mut Vec::new(),
            (0.0, 0.0),
            &mut |node, _, inner| match node {
                SceneNode::FrostedGlass(value) => {
                    assert!(appearance.is_none());
                    appearance = Some((
                        translate(value.rect(), (inner.0 - offset.0, inner.1 - offset.1)),
                        value.corners(),
                        value.appearance().clone(),
                    ));
                }
                SceneNode::Text(value) if value.z_index >= 100 => {
                    let run = text
                        .shape_bound_text(
                            &value.text,
                            value.font_size,
                            value.binding.as_ref().expect("actual current font binding"),
                        )
                        .unwrap();
                    foreground.push(RectF::new(
                        value.position.x + inner.0 - offset.0 - 1.0,
                        value.position.y + inner.1 - offset.1 - 1.0,
                        run.width + 2.0,
                        run.height + 2.0,
                    ));
                }
                _ => {}
            },
        );
        fn lower(nodes: &[SceneNode]) -> Vec<SceneNode> {
            nodes
                .iter()
                .filter_map(|node| {
                    let mut node = node.clone();
                    match &mut node {
                        SceneNode::FrostedGlass(_) => return None,
                        SceneNode::Text(value) if value.z_index >= 100 => return None,
                        SceneNode::RetainedFragment(value) => {
                            value.children = lower(&value.children).into()
                        }
                        SceneNode::Clip(value) => value.children = lower(&value.children),
                        SceneNode::Scroll(value) => value.children = lower(&value.children),
                        SceneNode::Layer(value) => value.children = lower(&value.children),
                        _ => {}
                    }
                    Some(node)
                })
                .collect()
        }
        fn foreground_only(nodes: &[SceneNode]) -> Vec<SceneNode> {
            nodes
                .iter()
                .filter_map(|node| {
                    let mut node = node.clone();
                    match &mut node {
                        SceneNode::Text(value) if value.z_index >= 100 => {}
                        SceneNode::RetainedFragment(value) => {
                            value.children = foreground_only(&value.children).into()
                        }
                        SceneNode::Clip(value) => value.children = foreground_only(&value.children),
                        SceneNode::Scroll(value) => {
                            value.children = foreground_only(&value.children)
                        }
                        SceneNode::Layer(value) => {
                            value.children = foreground_only(&value.children)
                        }
                        _ => return None,
                    }
                    Some(node)
                })
                .collect()
        }
        let mut full = SceneGraph::new();
        full.clear_color = scene.clear_color;
        let (canvas_background, canvas_inherited) =
            canvas_background.expect("actual Canvas lower-source fill outside the cell");
        full.root = vec![
            SceneNode::Scroll(lab_framework::core::scene::ScrollNode {
                offset: (
                    cell.x - 16.0 - canvas_inherited.0,
                    cell.y - 16.0 - canvas_inherited.1,
                ),
                children: vec![canvas_background],
                key: None,
            }),
            SceneNode::Scroll(lab_framework::core::scene::ScrollNode {
                offset,
                children: vec![SceneNode::Clip(clip)],
                key: None,
            }),
        ];
        let mut prefix = full.clone();
        prefix.root = lower(&full.root);
        let mut foreground_scene = full.clone();
        foreground_scene.clear_color = Color::TRANSPARENT.into();
        foreground_scene.root = foreground_only(&full.root);
        let (rect, corners, appearance) = appearance.unwrap();
        assert_eq!(
            rect,
            face(RectF::new(16.0, 16.0, cell.width, cell.height)),
            "actual scopes project into the test surface exactly once"
        );
        assert!(appearance.surface_emission().is_none());
        assert_eq!(f.lab.request.values.len(), 67);
        SideReflectionScene {
            scene: full,
            prefix,
            foreground_scene,
            rect,
            corners,
            appearance,
            foreground,
        }
    }

    #[test]
    fn glass_material_lab_authored_sources_and_all_67_controls_are_checked() {
        assert_eq!(COUNT, 67);
        assert_eq!(
            (
                K::Blur as usize,
                K::Uplift as usize,
                K::Rot0 as usize,
                K::LightB1 as usize
            ),
            (0, 58, 59, 66)
        );
        assert_eq!(CONTROLS.len(), COUNT);
        for (index, control) in CONTROLS.iter().enumerate() {
            assert_eq!(control.field as usize, index);
            assert!(control.min <= control.initial && control.initial <= control.max);
        }
        for control in &CONTROLS[59..] {
            let rotation = matches!(control.field, K::Rot0 | K::Rot1);
            assert_eq!(
                (control.min, control.max, control.step, control.initial),
                if rotation {
                    (-180.0, 180.0, 1.0, -20.0)
                } else {
                    (0.0, 1.0, 0.01, 1.0)
                }
            );
        }
        let initial = Request::initial();
        let material = initial.checked().unwrap();
        let glass = frosted(&material);
        assert_eq!(
            (
                glass.sigma_logical(),
                glass.transmission(),
                glass.coverage_opacity(),
                glass.refraction_logical()
            ),
            (9.0, 0.94, 1.0, 0.0)
        );
        assert!(glass.surface_emission().is_none());
        let optics = glass.glass_optics().unwrap();
        let p = optics.parameters();
        assert_eq!(
            (
                p.bevel_width,
                p.bevel_slope,
                p.roughness,
                p.normal_reflectance,
                p.reflection_strength,
                p.rim_strength,
                p.rim_falloff
            ),
            (1.75, 0.94, 0.28, 0.08, 0.9, 0.65, 1.5)
        );
        assert_eq!(
            (optics.reflection_tint(), optics.rim_tint()),
            (Color::WHITE, Color::WHITE)
        );
        for (index, (elevation, strength, center, radii)) in [
            (75.0_f32, 0.95, (0.16, 0.4), (0.13, 0.7)),
            (25.0, 0.85, (0.06, 0.1), (0.11, 0.16)),
        ]
        .into_iter()
        .enumerate()
        {
            let light = optics.lights()[index];
            let az = (-135.0_f32).to_radians();
            let el = elevation.to_radians();
            for (a, b) in light.direction().into_iter().zip([
                az.cos() * el.cos(),
                az.sin() * el.cos(),
                el.sin(),
            ]) {
                assert!((a - b).abs() < 1e-6);
            }
            assert_eq!(
                (
                    light.color(),
                    light.strength(),
                    light.center_uv(),
                    light.radii_uv()
                ),
                (
                    Color::WHITE,
                    strength,
                    Point::new(center.0, center.1),
                    Point::new(radii.0, radii.1)
                )
            );
            let rotation = light.rotation_cos_sin();
            assert!((rotation.x - (-20.0_f32).to_radians().cos()).abs() < 1e-6);
            assert!((rotation.y - (-20.0_f32).to_radians().sin()).abs() < 1e-6);
        }
        for field in [
            K::Rot0,
            K::Rot1,
            K::LightR0,
            K::LightG0,
            K::LightB0,
            K::LightR1,
            K::LightG1,
            K::LightB1,
        ] {
            for value in [
                f32::NAN,
                f32::INFINITY,
                if matches!(field, K::Rot0 | K::Rot1) {
                    181.0
                } else {
                    1.01
                },
                if matches!(field, K::Rot0 | K::Rot1) {
                    -181.0
                } else {
                    -0.01
                },
            ] {
                let mut request = initial.clone();
                request.values[field as usize] = value;
                assert!(request.checked().is_err());
            }
        }
        for (field, value) in [(K::Reflection, 0.0), (K::Rim, 0.0)] {
            let mut request = initial.clone();
            request.values[field as usize] = value;
            let changed = request.checked().unwrap();
            let changed = frosted(&changed);
            assert_eq!(
                (
                    changed.sigma_logical(),
                    changed.transmission(),
                    changed.tint(),
                    changed.micrograin()
                ),
                (
                    glass.sigma_logical(),
                    glass.transmission(),
                    glass.tint(),
                    glass.micrograin()
                )
            );
            let p = changed.glass_optics().unwrap().parameters();
            assert_eq!(
                (p.reflection_strength, p.rim_strength),
                if matches!(field, K::Reflection) {
                    (0.0, 0.65)
                } else {
                    (0.9, 0.0)
                }
            );
        }
        let mut black = initial.clone();
        for field in [K::LightR0, K::LightG0, K::LightB0] {
            black.values[field as usize] = 0.0;
        }
        assert_eq!(
            frosted(&black.checked().unwrap())
                .glass_optics()
                .unwrap()
                .lights()[0]
                .color(),
            Color::from_linear(0.0, 0.0, 0.0, 1.0)
        );
        for field in [K::LightR1, K::LightG1, K::LightB1] {
            black.values[field as usize] = 0.0;
        }
        assert!(
            frosted(&black.checked().unwrap())
                .glass_optics()
                .unwrap()
                .lights()
                .iter()
                .all(|light| light.color() == Color::from_linear(0.0, 0.0, 0.0, 1.0))
        );
    }

    #[test]
    fn glass_material_lab_source_controls_publish_without_rebuild_or_fixed_changes() {
        let mut f = Fixture::new();
        f.frame(0);
        f.frame(12000);
        let cells = f.cells();
        let backgrounds = f.backgrounds();
        let editable = f.tree.find_by_id(f.lab.preview.unwrap()).unwrap();
        let status = f.tree.find_by_id(f.lab.status.unwrap()).unwrap();
        let geometry = cells
            .iter()
            .chain(backgrounds.iter())
            .chain([&editable, &status])
            .map(|&node| {
                (
                    node,
                    f.tree.node(node).unwrap().id(),
                    f.tree.get_absolute_paint_rect(node),
                )
            })
            .collect::<Vec<_>>();
        let fixed = (0..5)
            .map(|i| f.material(i).checked.clone())
            .collect::<Vec<_>>();
        for (index, (field, value)) in [
            (K::Rot0, 37.0),
            (K::LightR0, 0.17),
            (K::LightG0, 0.27),
            (K::LightB0, 0.43),
            (K::Rot1, -63.0),
            (K::LightR1, 0.31),
            (K::LightG1, 0.62),
            (K::LightB1, 0.83),
        ]
        .into_iter()
        .enumerate()
        {
            let before = f.material(5).checked.clone();
            let writer = f.lab.binder.writer();
            let key = CONTROLS[field as usize].key;
            writer.set_f32(writer.intern(key), value);
            f.flush();
            assert!(f.lab.pending.as_ref().unwrap().checked.is_ok());
            f.lab.update(&mut f.tree);
            assert_ne!(f.material(5).checked, before);
            assert!(f.tree.node(editable).unwrap().dirty_paint);
            assert!(f.tree.node(status).unwrap().dirty_paint);
            assert!(!f.tree.node(editable).unwrap().dirty_layout);
            for &node in &backgrounds {
                assert!(!f.tree.node(node).unwrap().dirty_paint);
            }
            for (i, expected) in fixed.iter().enumerate() {
                assert_eq!(&f.material(i).checked, expected);
                assert!(
                    !f.tree
                        .node(f.tree.children(cells[i])[1])
                        .unwrap()
                        .dirty_paint
                );
            }
            let checked = f.material(5).checked.as_ref().unwrap();
            let lights = frosted(checked).glass_optics().unwrap().lights();
            let source = if (field as usize) < K::Rot1 as usize {
                0
            } else {
                1
            };
            match field {
                K::Rot0 | K::Rot1 => {
                    let rotation = lights[source].rotation_cos_sin();
                    assert!((rotation.x - value.to_radians().cos()).abs() < 1e-6);
                    assert!((rotation.y - value.to_radians().sin()).abs() < 1e-6);
                }
                K::LightR0 | K::LightR1 => assert_eq!(lights[source].color().r, value),
                K::LightG0 | K::LightG1 => assert_eq!(lights[source].color().g, value),
                _ => assert_eq!(lights[source].color().b, value),
            }
            f.frame(12016 + index as u64 * 16);
            assert_eq!(
                geometry,
                cells
                    .iter()
                    .chain(backgrounds.iter())
                    .chain([&editable, &status])
                    .map(|&node| (
                        node,
                        f.tree.node(node).unwrap().id(),
                        f.tree.get_absolute_paint_rect(node)
                    ))
                    .collect::<Vec<_>>()
            );
            writer.set_f32(writer.intern(key), value);
            f.flush();
            f.lab.update(&mut f.tree);
            assert!(f.lab.pending.is_none());
            assert!(!f.tree.node(f.root).unwrap().dirty_composition);
            assert!(!f.lab.take_rebuild_flag());
        }
        let status = f
            .tree
            .node(status)
            .unwrap()
            .widget()
            .as_any()
            .downcast_ref::<StatusView>()
            .unwrap();
        let text = status.snapshot.join(" ");
        for control in &CONTROLS[59..] {
            assert!(text.contains(control.key));
        }
        assert!(!text.contains("rotations=(1,0)"));
    }
}
