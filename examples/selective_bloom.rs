//! Native Selective Bloom comparison. Use Replay to run the finite crossing again.

use halcyon::core::{
    animation::{Animation, AnimationSample, Easing, TransitionHandle},
    app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext},
    appearance::ButtonMaterialPolicy,
    layout::{Constraints, LayoutResult},
    math::{Point, RectF, Size},
    paint::{
        AnalyticSurfaceLight, Color, ColorStop, GlassSurfaceOptics, GlassSurfaceOpticsParameters,
        GradientDef, GradientKind, RoundedCorners, SmokedGlassParameters, SurfaceAnalytic,
        SurfaceBloom, SurfaceGradient,
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

const WINDOW_SIZE: (f32, f32) = (1150.0, 760.0);
const CROSSING_SECONDS: f32 = 2.4;
const START: [Point; 2] = [Point::new(0.20, 0.50), Point::new(0.80, 0.50)];
const MARGIN: f32 = 54.0;

fn smoked_body() -> SurfaceAnalytic {
    let neutral = Color::from_linear(0.08, 0.08, 0.08, 1.0);
    let body = SurfaceGradient::new(GradientDef {
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
    let unlit = AnalyticSurfaceLight::try_new(
        [0.0, 0.0, 1.0],
        Color::WHITE,
        0.0,
        Point::new(0.50, 0.50),
        Point::new(0.20, 0.25),
        Point::new(1.0, 0.0),
    )
    .expect("unlit glass source");
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
    .expect("checked neutral smoked face")
}

fn materials(body: &SurfaceAnalytic, centers: [Point; 2]) -> Result<[SurfaceAnalytic; 2], String> {
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
    let optics = GlassSurfaceOptics::try_new(
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
    .map_err(|error| error.to_string())?;
    let material = body.clone().with_glass_optics(Some(optics));
    let bloom = SurfaceBloom::try_new(8.0, 16.0, 1.0).map_err(|error| error.to_string())?;
    let blooming = material
        .clone()
        .with_glass_optics(Some(optics.with_bloom(Some(bloom))));
    Ok([material, blooming])
}

fn start_crossing(cx: &mut UpdateContext) -> Result<[TransitionHandle; 2], String> {
    let [left, right] = START;
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

struct Comparison {
    id: WidgetInternalId,
    body: SurfaceAnalytic,
    faces: [SurfaceAnalytic; 2],
    centers: [Point; 2],
    handles: Option<[TransitionHandle; 2]>,
    replay: bool,
    failed: bool,
}

impl Comparison {
    fn new() -> Self {
        let body = smoked_body();
        Self {
            id: WidgetInternalId::new(),
            faces: materials(&body, START).expect("checked Selective Bloom fixture"),
            body,
            centers: START,
            handles: None,
            replay: true,
            failed: false,
        }
    }

    fn cancel(&mut self, cx: &mut UpdateContext) {
        if let Some(handles) = self.handles.take() {
            for handle in handles {
                cx.cancel_animation(handle);
            }
        }
    }

    fn fail(&mut self, cx: &mut UpdateContext, reason: impl std::fmt::Display) {
        self.cancel(cx);
        self.failed = true;
        cx.invalidate_paint();
        eprintln!("Selective Bloom playback failed: {reason}");
    }
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
        self.replay || self.handles.is_some()
    }

    fn update(&mut self, _: f32, cx: &mut UpdateContext) {
        if std::mem::take(&mut self.replay) {
            self.cancel(cx);
            match start_crossing(cx) {
                Ok(handles) => {
                    self.handles = Some(handles);
                    if std::mem::take(&mut self.failed) {
                        cx.invalidate_paint();
                    }
                    println!("Selective Bloom playback started");
                }
                Err(error) => self.fail(cx, error),
            }
        }
        let Some(handles) = self.handles else { return };
        let (centers, finished) = match handles.map(|handle| cx.sample_animation::<Point>(handle)) {
            [Some(AnimationSample::Active(warm)), Some(AnimationSample::Active(cool))] => {
                ([warm, cool], false)
            }
            [Some(AnimationSample::Completed(warm)), Some(AnimationSample::Completed(cool))] => {
                ([warm, cool], true)
            }
            samples => {
                self.fail(cx, format_args!("incomplete light pair {samples:?}"));
                return;
            }
        };
        if centers != self.centers {
            match materials(&self.body, centers) {
                Ok(faces) => {
                    self.faces = faces;
                    self.centers = centers;
                    cx.invalidate_paint();
                }
                Err(error) => {
                    self.fail(cx, error);
                    return;
                }
            }
        }
        if finished {
            self.handles = None;
            println!("Selective Bloom playback complete");
        }
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let size = constraints.constrain(Size::new(WINDOW_SIZE.0, WINDOW_SIZE.1));
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
        text(cx, "Selective Bloom", left, rect.y + 59.0, 39.0, bright);
        text(
            cx,
            "The reflections produce a halo where they overlap.",
            left,
            rect.y + 119.0,
            18.0,
            muted,
        );
        let gap = 56.0;
        let width = (rect.width - 2.0 * MARGIN - gap) * 0.5;
        let height = (rect.height * 0.36).clamp(120.0, 274.0);
        let top = rect.y + (rect.height * 0.31).clamp(176.0, 235.0);
        if width < 24.0 {
            return;
        }
        for (index, label) in ["Bloom off", "Selective Bloom"].into_iter().enumerate() {
            let face = RectF::new(left + index as f32 * (width + gap), top, width, height);
            // Matched scene forms beneath both specimens are real lower-Z content.
            cx.scene.push_rounded_rect(
                RectF::new(
                    face.x - 14.0,
                    top + height * 0.64,
                    width + 28.0,
                    height * 0.27,
                ),
                Color::from_hex(0x29323b),
                RoundedCorners::uniform(20.0),
                2,
                None,
            );
            if let Err(error) = cx.scene.push_analytic_surface(
                face,
                RoundedCorners::uniform(12.0),
                &self.faces[index],
                40 + index as u32,
                None,
            ) {
                eprintln!("Selective Bloom surface refused: {error}");
            }
            text(cx, label, face.x, top - 38.0, 19.0, bright);
        }
        let footer = if self.failed {
            "Reflection playback unavailable"
        } else {
            "Internal HDR / SDR presentation"
        };
        text(cx, footer, left, top + height + 45.0, 16.0, muted);
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
    replay: bool,
}

impl Screen for Demo {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Comparison::new()));
        self.comparison = Some(cx.tree.node(root).expect("mounted comparison").id());
        let replay = Button::with_text("Replay").with_binding("replay", self.binder.writer());
        cx.tree.add_child(root, Box::new(replay), None, 0);
        &mut self.placeholder
    }

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        self.binder.bind(cx, |bindings| {
            self.replay |= bindings.bool("replay").take().is_some();
        });
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        if !std::mem::take(&mut self.replay) {
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
        comparison.replay = true;
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();
    let mut config = AppConfig::new("Selective Bloom").with_size(WINDOW_SIZE.0, WINDOW_SIZE.1);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    App::new(config).run(Demo {
        binder: FormBinder::new(),
        placeholder: Label::new(""),
        comparison: None,
        replay: false,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selective_bloom_has_one_effect_with_the_same_reflection_sources() {
        let centers = [Point::new(0.5, 0.5); 2];
        let [left, right] = materials(&smoked_body(), centers).expect("checked midpoint");
        let left = left.glass_optics().expect("reference coating");
        let right = right.glass_optics().expect("Bloom coating");
        assert_eq!(left.lights(), right.lights());
        assert_eq!(left.lights().map(AnalyticSurfaceLight::center_uv), centers);
        assert_eq!(left.bloom(), None);
        assert_eq!(left.thin_film(), None);
        assert_eq!(right.thin_film(), None);
        assert_eq!(
            right.bloom(),
            Some(SurfaceBloom::try_new(8.0, 16.0, 1.0).unwrap())
        );
    }
}
