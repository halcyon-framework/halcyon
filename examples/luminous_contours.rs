//! SPEC-HAL-358 native luminous-contour visual and cadence proof.
//!
//! Run:
//! `cargo run --release --features render-audit --example luminous_contours`
//!
//! Live tuning: `1`/`2`/`3` presets, `[`/`]` outer radius, `-`/`=` outer
//! intensity, `A`/`S` inner radius, `Z`/`X` inner intensity, `C`/`V` core lift,
//! `,`/`.` front strength, and `N`/`M` front half-extent. Framework Escape
//! ownership is unchanged.

use std::cell::Cell;
use std::f32::consts::TAU;
use std::rc::Rc;
use std::sync::Arc;

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{Event, EventPhase, FusedInputEvent, KeyCode};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::paint::{
    Color, LAYER_CONTENT, LuminousContourAppearance, LuminousContourFront, LuminousContourFronts,
};
use halcyon::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::Label;
use halcyon::utils::id::WidgetInternalId;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(AppConfig::new("Luminous Contours").with_size(1180.0, 760.0));
    app.run(LuminousContoursScreen::new());
}

#[derive(Clone)]
struct ContourGeometry {
    points: Arc<[Point]>,
    length: f32,
}

const CORE_WIDTH: f32 = 1.0;
const CORE_ALPHA: f32 = 0.84;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExamplePreset {
    Subtle,
    Balanced,
    Strong,
    Custom,
}

impl ExamplePreset {
    const fn name(self) -> &'static str {
        match self {
            Self::Subtle => "SUBTLE",
            Self::Balanced => "BALANCED",
            Self::Strong => "STRONG",
            Self::Custom => "CUSTOM",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ExampleTuning {
    preset: ExamplePreset,
    core_luminance_lift: f32,
    inner_aura_radius: f32,
    inner_aura_intensity: f32,
    halo_radius: f32,
    halo_intensity: f32,
    halo_falloff: f32,
    front_strength: f32,
    front_half_extent: f32,
}

impl ExampleTuning {
    const fn subtle() -> Self {
        Self {
            preset: ExamplePreset::Subtle,
            core_luminance_lift: 0.18,
            inner_aura_radius: 0.90,
            inner_aura_intensity: 0.28,
            halo_radius: 2.25,
            halo_intensity: 0.025,
            halo_falloff: 3.50,
            front_strength: 0.45,
            front_half_extent: 14.0,
        }
    }

    const fn balanced() -> Self {
        Self {
            preset: ExamplePreset::Balanced,
            core_luminance_lift: 0.42,
            inner_aura_radius: 1.60,
            inner_aura_intensity: 0.52,
            halo_radius: 4.00,
            halo_intensity: 0.055,
            halo_falloff: 3.25,
            front_strength: 0.75,
            front_half_extent: 18.0,
        }
    }

    const fn strong() -> Self {
        Self {
            preset: ExamplePreset::Strong,
            core_luminance_lift: 0.58,
            inner_aura_radius: 2.15,
            inner_aura_intensity: 0.72,
            halo_radius: 5.50,
            halo_intensity: 0.090,
            halo_falloff: 2.75,
            front_strength: 1.0,
            front_half_extent: 22.0,
        }
    }
}

impl Default for ExampleTuning {
    fn default() -> Self {
        Self::balanced()
    }
}

impl ContourGeometry {
    fn from_points(points: Vec<Point>) -> Self {
        let length = points
            .windows(2)
            .map(|segment| segment[0].distance(segment[1]))
            .sum();
        Self {
            points: Arc::from(points),
            length,
        }
    }

    fn empty() -> Self {
        Self {
            points: Arc::from([Point::new(0.0, 0.0), Point::new(1.0, 0.0)]),
            length: 1.0,
        }
    }
}

struct LuminousContoursDemo {
    id: WidgetInternalId,
    size: Size,
    phase: f32,
    ordinary: [ContourGeometry; 4],
    luminous: [ContourGeometry; 4],
    tuning: Rc<Cell<ExampleTuning>>,
    #[cfg(feature = "render-audit")]
    audit_elapsed: f32,
    #[cfg(feature = "render-audit")]
    audit_updates: u64,
    #[cfg(feature = "render-audit")]
    audit_previous: Option<(halcyon::renderer::RenderCadenceSnapshot, u64)>,
}

impl LuminousContoursDemo {
    fn new(tuning: Rc<Cell<ExampleTuning>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            size: Size::zero(),
            phase: 0.0,
            ordinary: std::array::from_fn(|_| ContourGeometry::empty()),
            luminous: std::array::from_fn(|_| ContourGeometry::empty()),
            tuning,
            #[cfg(feature = "render-audit")]
            audit_elapsed: 0.0,
            #[cfg(feature = "render-audit")]
            audit_updates: 0,
            #[cfg(feature = "render-audit")]
            audit_previous: None,
        }
    }

    fn rebuild_geometry(&mut self, size: Size) {
        if self.size == size {
            return;
        }
        self.size = size;
        let width = size.width.max(320.0);
        let height = size.height.max(320.0);
        let column_width = width * 0.34;
        let left_x = width * 0.09;
        let right_x = width * 0.57;
        let rows = [height * 0.26, height * 0.45, height * 0.65, height * 0.84];

        self.ordinary = build_column(left_x, column_width, rows);
        self.luminous = build_column(right_x, column_width, rows);
    }

    fn front(distance: f32, half_extent: f32, intensity: f32) -> LuminousContourFront {
        LuminousContourFront::try_new(distance, half_extent, intensity)
            .expect("example front stays inside SPEC-HAL-358 bounds")
    }

    fn fronts<const N: usize>(fronts: [LuminousContourFront; N]) -> LuminousContourFronts {
        LuminousContourFronts::try_from_slice(&fronts)
            .expect("example front count stays inside SPEC-HAL-358 bounds")
    }

    fn appearance(tuning: ExampleTuning) -> LuminousContourAppearance {
        LuminousContourAppearance::try_new_zoned(
            tuning.core_luminance_lift,
            tuning.inner_aura_radius,
            tuning.inner_aura_intensity,
            tuning.halo_radius,
            tuning.halo_intensity,
            tuning.halo_falloff,
        )
        .expect("example tuning stays inside SPEC-HAL-358 bounds")
    }

    #[cfg(feature = "render-audit")]
    fn record_cadence(&mut self, dt: f32) {
        self.audit_elapsed += dt.max(0.0);
        self.audit_updates += 1;
        if self.audit_elapsed < 2.0 {
            return;
        }
        let elapsed = f64::from(self.audit_elapsed);
        let current = halcyon::renderer::render_cadence_snapshot();
        if let Some((previous, previous_updates)) = self.audit_previous {
            let rate = |now: u64, before: u64| now.saturating_sub(before) as f64 / elapsed;
            tracing::info!(
                updates_per_second = rate(self.audit_updates, previous_updates),
                submissions_per_second =
                    rate(current.surface_submissions, previous.surface_submissions),
                presentations_per_second = rate(
                    current.surface_presentations,
                    previous.surface_presentations
                ),
                renderer_advances_per_second = rate(
                    current.renderer_frame_advances,
                    previous.renderer_frame_advances
                ),
                "SPEC-HAL-358 cadence"
            );
        }
        self.audit_previous = Some((current, self.audit_updates));
        self.audit_elapsed = 0.0;
    }
}

impl WidgetDowncast for LuminousContoursDemo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for LuminousContoursDemo {
    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        if dt.is_finite() && dt > 0.0 {
            self.phase = (self.phase + dt.min(0.05) * 0.12).fract();
            #[cfg(feature = "render-audit")]
            self.record_cadence(dt);
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        true
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let size = constraints.constrain(Size::new(constraints.max_width, constraints.max_height));
        self.rebuild_geometry(size);
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene.push_rect(rect, Color::from_hex(0x020407), 0, None);

        let colors = [
            Color::from_srgb(0.18, 0.74, 0.80, CORE_ALPHA),
            Color::from_srgb(0.10, 0.66, 0.72, CORE_ALPHA),
            Color::from_srgb(0.90, 0.52, 0.16, CORE_ALPHA),
            Color::from_srgb(0.12, 0.72, 0.80, CORE_ALPHA),
        ];
        for (index, geometry) in self.ordinary.iter().enumerate() {
            cx.scene
                .push_polyline(geometry.points.to_vec(), CORE_WIDTH, colors[index], 1, None);
        }

        let tuning = self.tuning.get();
        let appearance = Self::appearance(tuning);
        let moving = |geometry: &ContourGeometry, offset: f32| {
            (self.phase + offset).fract() * geometry.length
        };
        let front_intensity = |base: f32| (base * tuning.front_strength).clamp(0.0, 1.0);
        let front_sets = [
            LuminousContourFronts::empty(),
            Self::fronts([Self::front(
                moving(&self.luminous[1], 0.0),
                tuning.front_half_extent,
                front_intensity(0.90),
            )]),
            Self::fronts([
                Self::front(
                    moving(&self.luminous[2], 0.0),
                    tuning.front_half_extent,
                    front_intensity(1.0),
                ),
                Self::front(
                    moving(&self.luminous[2], 0.5),
                    tuning.front_half_extent,
                    front_intensity(0.60),
                ),
            ]),
            Self::fronts([
                Self::front(
                    moving(&self.luminous[3], 0.0),
                    tuning.front_half_extent,
                    front_intensity(1.0),
                ),
                Self::front(
                    moving(&self.luminous[3], 0.33),
                    tuning.front_half_extent,
                    front_intensity(0.65),
                ),
                Self::front(
                    moving(&self.luminous[3], 0.67),
                    tuning.front_half_extent,
                    front_intensity(0.45),
                ),
            ]),
        ];

        for (index, geometry) in self.luminous.iter().enumerate() {
            cx.scene
                .push_luminous_contour(
                    geometry.points.clone(),
                    CORE_WIDTH,
                    colors[index],
                    appearance,
                    front_sets[index],
                    2,
                    None,
                )
                .expect("example geometry remains admitted after resize");
        }

        let divider = RectF::new(
            rect.x + rect.width * 0.5,
            rect.y + 42.0,
            1.0,
            rect.height - 84.0,
        );
        cx.scene
            .push_rect(divider, Color::from_srgb(0.15, 0.23, 0.28, 0.55), 3, None);

        let heading_color = Color::from_srgb(0.56, 0.68, 0.73, 0.92);
        let detail_color = Color::from_srgb(0.42, 0.54, 0.59, 0.82);
        let ordinary_heading = cx.text_engine.shape("ORDINARY STROKE", 13.0);
        cx.text_engine.draw(
            &ordinary_heading,
            rect.x + rect.width * 0.09,
            rect.y + 30.0,
            heading_color,
            LAYER_CONTENT + 2,
            cx.scene,
        );
        let luminous_heading = cx.text_engine.shape("LUMINOUS CONTOUR", 13.0);
        cx.text_engine.draw(
            &luminous_heading,
            rect.x + rect.width * 0.57,
            rect.y + 30.0,
            heading_color,
            LAYER_CONTENT + 2,
            cx.scene,
        );
        let values = format!(
            "{}  core lift {:.2}  inner aura {:.2}px / {:.2}  core alpha {:.2} fixed",
            tuning.preset.name(),
            tuning.core_luminance_lift,
            tuning.inner_aura_radius,
            tuning.inner_aura_intensity,
            CORE_ALPHA,
        );
        let outer_values = format!(
            "outer halo {:.2}px / {:.3}  falloff {:.2}  front {:.2} / {:.0}px",
            tuning.halo_radius,
            tuning.halo_intensity,
            tuning.halo_falloff,
            tuning.front_strength,
            tuning.front_half_extent,
        );
        let values_run = cx.text_engine.shape(&values, 11.0);
        cx.text_engine.draw(
            &values_run,
            rect.x + rect.width * 0.09,
            rect.y + 57.0,
            detail_color,
            LAYER_CONTENT + 2,
            cx.scene,
        );
        let outer_values_run = cx.text_engine.shape(&outer_values, 11.0);
        cx.text_engine.draw(
            &outer_values_run,
            rect.x + rect.width * 0.09,
            rect.y + 76.0,
            detail_color,
            LAYER_CONTENT + 2,
            cx.scene,
        );
        let controls =
            "1/2/3 presets    [ ] outer radius    - = outer intensity    A S inner radius";
        let controls_run = cx.text_engine.shape(controls, 11.0);
        cx.text_engine.draw(
            &controls_run,
            rect.x + rect.width * 0.09,
            rect.y + 97.0,
            detail_color,
            LAYER_CONTENT + 2,
            cx.scene,
        );
        let more_controls =
            "Z X inner intensity    C V core lift    , . front strength    N M front radius";
        let more_controls_run = cx.text_engine.shape(more_controls, 11.0);
        cx.text_engine.draw(
            &more_controls_run,
            rect.x + rect.width * 0.09,
            rect.y + 116.0,
            detail_color,
            LAYER_CONTENT + 2,
            cx.scene,
        );
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "LuminousContoursDemo"
    }
}

fn build_column(x: f32, width: f32, rows: [f32; 4]) -> [ContourGeometry; 4] {
    let line =
        ContourGeometry::from_points(vec![Point::new(x, rows[0]), Point::new(x + width, rows[0])]);

    let curve = ContourGeometry::from_points(
        (0..=64)
            .map(|index| {
                let t = index as f32 / 64.0;
                Point::new(x + width * t, rows[1] + (t * TAU * 1.5).sin() * 28.0)
            })
            .collect(),
    );

    let square_half = width.min(150.0) * 0.34;
    let square_center = Point::new(x + width * 0.5, rows[2]);
    let square = ContourGeometry::from_points(vec![
        Point::new(
            square_center.x - square_half,
            square_center.y - square_half * 0.62,
        ),
        Point::new(
            square_center.x + square_half,
            square_center.y - square_half * 0.62,
        ),
        Point::new(
            square_center.x + square_half,
            square_center.y + square_half * 0.62,
        ),
        Point::new(
            square_center.x - square_half,
            square_center.y + square_half * 0.62,
        ),
        Point::new(
            square_center.x - square_half,
            square_center.y - square_half * 0.62,
        ),
    ]);

    let radius = width.min(170.0) * 0.30;
    let center = Point::new(x + width * 0.5, rows[3]);
    let circle = ContourGeometry::from_points(
        (0..=96)
            .map(|index| {
                let angle = index as f32 / 96.0 * TAU;
                Point::new(
                    center.x + angle.cos() * radius,
                    center.y + angle.sin() * radius,
                )
            })
            .collect(),
    );

    [line, curve, square, circle]
}

struct LuminousContoursScreen {
    needs_rebuild: bool,
    placeholder: Label,
    tuning: Rc<Cell<ExampleTuning>>,
}

impl LuminousContoursScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            tuning: Rc::new(Cell::new(ExampleTuning::default())),
        }
    }

    fn apply_tuning_key(&mut self, key_code: KeyCode) -> bool {
        let mut tuning = self.tuning.get();
        match key_code {
            KeyCode::Character('1') => tuning = ExampleTuning::subtle(),
            KeyCode::Character('2') => tuning = ExampleTuning::balanced(),
            KeyCode::Character('3') => tuning = ExampleTuning::strong(),
            KeyCode::Character('[') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.halo_radius -= 0.25;
            }
            KeyCode::Character(']') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.halo_radius += 0.25;
            }
            KeyCode::Character('-') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.halo_intensity -= 0.01;
            }
            KeyCode::Character('=') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.halo_intensity += 0.01;
            }
            KeyCode::Character('a' | 'A') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.inner_aura_radius -= 0.10;
            }
            KeyCode::Character('s' | 'S') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.inner_aura_radius += 0.10;
            }
            KeyCode::Character('z' | 'Z') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.inner_aura_intensity -= 0.025;
            }
            KeyCode::Character('x' | 'X') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.inner_aura_intensity += 0.025;
            }
            KeyCode::Character('c' | 'C') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.core_luminance_lift -= 0.025;
            }
            KeyCode::Character('v' | 'V') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.core_luminance_lift += 0.025;
            }
            KeyCode::Character(',') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.front_strength -= 0.05;
            }
            KeyCode::Character('.') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.front_strength += 0.05;
            }
            KeyCode::Character('n' | 'N') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.front_half_extent -= 1.0;
            }
            KeyCode::Character('m' | 'M') => {
                tuning.preset = ExamplePreset::Custom;
                tuning.front_half_extent += 1.0;
            }
            _ => return false,
        }
        tuning.halo_radius = tuning.halo_radius.clamp(0.0, 8.0);
        tuning.halo_intensity = tuning.halo_intensity.clamp(0.0, 0.25);
        tuning.inner_aura_radius = tuning
            .inner_aura_radius
            .clamp(0.0, 4.0_f32.min(tuning.halo_radius));
        tuning.inner_aura_intensity = tuning.inner_aura_intensity.clamp(0.0, 0.75);
        tuning.core_luminance_lift = tuning.core_luminance_lift.clamp(0.0, 0.75);
        tuning.front_strength = tuning.front_strength.clamp(0.0, 1.0);
        tuning.front_half_extent = tuning.front_half_extent.clamp(2.0, 32.0);
        self.tuning.set(tuning);
        tracing::info!(
            preset = tuning.preset.name(),
            core_luminance_lift = tuning.core_luminance_lift,
            inner_aura_radius = tuning.inner_aura_radius,
            inner_aura_intensity = tuning.inner_aura_intensity,
            halo_radius = tuning.halo_radius,
            halo_intensity = tuning.halo_intensity,
            halo_falloff = tuning.halo_falloff,
            front_strength = tuning.front_strength,
            front_half_extent = tuning.front_half_extent,
            "SPEC-HAL-358 example tuning"
        );
        true
    }
}

impl Screen for LuminousContoursScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        cx.mount_child(Box::new(LuminousContoursDemo::new(self.tuning.clone())));
        &mut self.placeholder
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        let FusedInputEvent::KeyDown {
            key_code,
            repeat: false,
            ..
        } = event
        else {
            return false;
        };
        if self.apply_tuning_key(*key_code) {
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    use halcyon::core::app::{AppContext, ModalStack, ScreenStack};
    use halcyon::core::event::{Modifiers, WindowId};
    use halcyon::core::theme::Theme;
    use halcyon::core::widget_tree::WidgetTree;

    fn mounted_child_count(screen: &mut LuminousContoursScreen) -> usize {
        let mut app = AppContext::new(Theme::dark());
        let mut tree = WidgetTree::new();
        let parent_node = tree.create_root(Box::new(Label::new("")), None, 0);
        {
            let mut cx = ScreenFrameContext {
                app: &mut app,
                tree: &mut tree,
                parent_node,
            };
            let _ = screen.build(&mut cx);
        }
        tree.children(parent_node).len()
    }

    #[test]
    fn requested_screen_rebuild_mounts_exactly_one_root_child_again() {
        let mut screen = LuminousContoursScreen::new();
        assert!(screen.take_rebuild_flag());
        assert!(!screen.take_rebuild_flag());
        assert_eq!(mounted_child_count(&mut screen), 1);

        screen.mark_rebuild();
        assert!(screen.take_rebuild_flag());
        assert_eq!(mounted_child_count(&mut screen), 1);
    }

    #[test]
    fn routed_tuning_key_updates_retained_paint_without_requesting_rebuild() {
        let mut screen = LuminousContoursScreen::new();
        assert!(screen.take_rebuild_flag());
        let mut app = AppContext::new(Theme::dark());
        let mut screens = ScreenStack::new();
        let mut modals = ModalStack::new();
        let event = FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code: KeyCode::Character(']'),
            physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat: false,
        };

        {
            let mut cx = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
            assert!(screen.handle_input(&event, &mut cx));
        }

        assert_eq!(screen.tuning.get().halo_radius, 4.25);
        assert!(!app.take_invalidate_requested());
        assert!(!screen.take_rebuild_flag());
    }

    #[test]
    fn contour_geometry_reuses_allocations_until_logical_size_changes() {
        let mut demo = LuminousContoursDemo::new(Rc::new(Cell::new(ExampleTuning::default())));
        demo.rebuild_geometry(Size::new(800.0, 600.0));
        let initial = demo.luminous[0].points.clone();

        demo.rebuild_geometry(Size::new(800.0, 600.0));
        assert!(Arc::ptr_eq(&initial, &demo.luminous[0].points));

        demo.rebuild_geometry(Size::new(960.0, 720.0));
        assert!(!Arc::ptr_eq(&initial, &demo.luminous[0].points));
        assert_eq!(demo.size, Size::new(960.0, 720.0));
    }

    #[test]
    fn tuning_keys_are_bounded_and_do_not_claim_framework_escape() {
        let mut screen = LuminousContoursScreen::new();
        assert!(screen.apply_tuning_key(KeyCode::Character('3')));
        assert_eq!(screen.tuning.get(), ExampleTuning::strong());
        for _ in 0..32 {
            assert!(screen.apply_tuning_key(KeyCode::Character(']')));
            assert!(screen.apply_tuning_key(KeyCode::Character('=')));
            assert!(screen.apply_tuning_key(KeyCode::Character('s')));
            assert!(screen.apply_tuning_key(KeyCode::Character('x')));
            assert!(screen.apply_tuning_key(KeyCode::Character('v')));
            assert!(screen.apply_tuning_key(KeyCode::Character('m')));
        }
        assert_eq!(screen.tuning.get().preset, ExamplePreset::Custom);
        assert_eq!(screen.tuning.get().halo_radius, 8.0);
        assert_eq!(screen.tuning.get().halo_intensity, 0.25);
        assert_eq!(screen.tuning.get().inner_aura_radius, 4.0);
        assert_eq!(screen.tuning.get().inner_aura_intensity, 0.75);
        assert_eq!(screen.tuning.get().core_luminance_lift, 0.75);
        assert_eq!(screen.tuning.get().front_half_extent, 32.0);
        assert!(!screen.apply_tuning_key(KeyCode::Escape));
    }
}
