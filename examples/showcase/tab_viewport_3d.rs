use halcyon::core::animation::{
    Animation, AnimationDescriptor, AnimationSample, Easing, Playback, RepeatCount,
    TransitionHandle,
};
use halcyon::core::event::{CursorIcon, Event, EventPhase, PointerButton, PointerEventKind};
use halcyon::core::math::RectF;
use halcyon::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use halcyon::renderer::wgpu_renderer::GpuResources;
use halcyon::renderer::{
    CustomRenderActivity, CustomRenderAdmission, CustomRenderParticipant,
    CustomRenderPreparationOutcome, CustomRenderTargetMetrics,
};
use halcyon::three_d::{
    AppearanceMode, AppearanceSourceDescriptor, CameraNavigationController3, CameraNavigationMode3,
    CameraNavigationPolicy3, CameraPointerOperation3, CameraState3, CameraWheelOperation3,
    Context3D, DrawPacketDescriptor, GeometryIndices, GeometrySemantics, GeometrySourceDescriptor,
    LinearVertexColorRgba8, LocalPoint3, LocalVector3, ModelPlacement3, ModelRevision, ModelState,
    ModelStateDescriptor, Orientation3, OrthographicProjection, PerspectiveProjection,
    PositiveScale3, Projection3, RasterMode, Ray3, RenderSnapshot, RenderSnapshotDescriptor,
    SampleCount3D, Sampler3D, SamplerAddressMode, SamplerFilterMode, StructuralRevision, Texture3D,
    TextureCoordinate2, TextureFormat3D, Viewport3D, Viewport3DTuple, Viewport3DViewRevision,
    WorldPoint3,
};
use halcyon::widgets::radio_group::RadioId;
use halcyon::widgets::radio_group_widget::{RadioGroup, RadioGroupId};

use super::*;

const VIEWPORT_DIAGNOSTIC_INTERVAL: Duration = Duration::from_millis(250);

const FAR_UNLIT_SOURCE: &str = r#"
fn halcyon_appearance(
    position_render_relative: vec3<f32>,
    normal_world_interpolated: vec3<f32>,
    view_direction_world: vec3<f32>,
    uv: vec2<f32>,
    vertex_color_linear: vec4<f32>,
    front_facing: bool,
    color_factor_linear: vec4<f32>,
    appearance_texture: texture_2d<f32>,
    appearance_sampler: sampler,
) -> vec3<f32> {
    return vec3<f32>(0.035, 0.16, 0.27);
}
"#;

const FALLBACK_TEXTURE_VERTEX_COLOR_SOURCE: &str = r#"
fn halcyon_appearance(
    position_render_relative: vec3<f32>,
    normal_world_interpolated: vec3<f32>,
    view_direction_world: vec3<f32>,
    uv: vec2<f32>,
    vertex_color_linear: vec4<f32>,
    front_facing: bool,
    color_factor_linear: vec4<f32>,
    appearance_texture: texture_2d<f32>,
    appearance_sampler: sampler,
) -> vec3<f32> {
    let sampled = textureSample(appearance_texture, appearance_sampler, uv);
    return sampled.rgb * vertex_color_linear.rgb * color_factor_linear.rgb;
}
"#;

const ALPHA_MASK_SOURCE: &str = r#"
fn halcyon_appearance(
    position_render_relative: vec3<f32>,
    normal_world_interpolated: vec3<f32>,
    view_direction_world: vec3<f32>,
    uv: vec2<f32>,
    vertex_color_linear: vec4<f32>,
    front_facing: bool,
    color_factor_linear: vec4<f32>,
    appearance_texture: texture_2d<f32>,
    appearance_sampler: sampler,
) -> vec3<f32> {
    if uv.x + uv.y > 1.0 {
        discard;
    }
    return vec3<f32>(1.0, 0.46, 0.055) * color_factor_linear.rgb;
}
"#;

const BACKFACE_PROBE_SOURCE: &str = r#"
fn halcyon_appearance(
    position_render_relative: vec3<f32>,
    normal_world_interpolated: vec3<f32>,
    view_direction_world: vec3<f32>,
    uv: vec2<f32>,
    vertex_color_linear: vec4<f32>,
    front_facing: bool,
    color_factor_linear: vec4<f32>,
    appearance_texture: texture_2d<f32>,
    appearance_sampler: sampler,
) -> vec3<f32> {
    if front_facing {
        return vec3<f32>(0.1, 0.9, 0.8);
    }
    return vec3<f32>(0.95, 0.08, 0.72);
}
"#;

const CLOSED_VOLUME_SOURCE: &str = r#"
fn halcyon_appearance(
    position_render_relative: vec3<f32>,
    normal_world_interpolated: vec3<f32>,
    view_direction_world: vec3<f32>,
    uv: vec2<f32>,
    vertex_color_linear: vec4<f32>,
    front_facing: bool,
    color_factor_linear: vec4<f32>,
    appearance_texture: texture_2d<f32>,
    appearance_sampler: sampler,
) -> vec3<f32> {
    let normal = normalize(normal_world_interpolated);
    let key_light = normalize(vec3<f32>(-0.45, 0.65, 0.58));
    let diffuse = 0.26 + 0.74 * max(dot(normal, key_light), 0.0);
    return vertex_color_linear.rgb * color_factor_linear.rgb * diffuse;
}
"#;

const MODEL_SPIN_DURATION: f32 = 4.0;
const WHEEL_ZOOM_TRANSITION_DURATION: f32 = 0.080;
const SHOWCASE_QUAD_TRIANGLES: [[(f32, f32, f32); 3]; 2] = [
    [(-1.0, -1.0, 0.0), (1.0, -1.0, 0.0), (1.0, 1.0, 0.0)],
    [(-1.0, -1.0, 0.0), (1.0, 1.0, 0.0), (-1.0, 1.0, 0.0)],
];
const SHOWCASE_TRIANGLE: [[(f32, f32, f32); 3]; 1] =
    [[(-0.9, -0.72, 0.0), (0.9, -0.72, 0.0), (0.0, 0.96, 0.0)]];
const SHOWCASE_CLOCKWISE_TRIANGLE: [[(f32, f32, f32); 3]; 1] =
    [[(-0.9, -0.72, 0.0), (0.0, 0.96, 0.0), (0.9, -0.72, 0.0)]];
const SHOWCASE_CUBE_TRIANGLES: [[(f32, f32, f32); 3]; 12] = [
    [(-0.5, -0.5, 0.5), (0.5, -0.5, 0.5), (0.5, 0.5, 0.5)],
    [(-0.5, -0.5, 0.5), (0.5, 0.5, 0.5), (-0.5, 0.5, 0.5)],
    [(0.5, -0.5, -0.5), (-0.5, -0.5, -0.5), (-0.5, 0.5, -0.5)],
    [(0.5, -0.5, -0.5), (-0.5, 0.5, -0.5), (0.5, 0.5, -0.5)],
    [(0.5, -0.5, 0.5), (0.5, -0.5, -0.5), (0.5, 0.5, -0.5)],
    [(0.5, -0.5, 0.5), (0.5, 0.5, -0.5), (0.5, 0.5, 0.5)],
    [(-0.5, -0.5, -0.5), (-0.5, -0.5, 0.5), (-0.5, 0.5, 0.5)],
    [(-0.5, -0.5, -0.5), (-0.5, 0.5, 0.5), (-0.5, 0.5, -0.5)],
    [(-0.5, 0.5, 0.5), (0.5, 0.5, 0.5), (0.5, 0.5, -0.5)],
    [(-0.5, 0.5, 0.5), (0.5, 0.5, -0.5), (-0.5, 0.5, -0.5)],
    [(-0.5, -0.5, -0.5), (0.5, -0.5, -0.5), (0.5, -0.5, 0.5)],
    [(-0.5, -0.5, -0.5), (0.5, -0.5, 0.5), (-0.5, -0.5, 0.5)],
];
const SHOWCASE_TRANSFORM_LOCAL_Z_RADIANS_PER_LOGICAL_PIXEL: f32 = 0.008;
const SHOWCASE_TRANSFORM_MAXIMUM_POINTER_DELTA_LOGICAL_PIXELS: f32 = 4096.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShowcaseTransformOperation {
    TransformRotate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShowcaseFigure {
    FarUnlitQuad,
    TexturedVertexColorTriangle,
    AlphaMaskQuad,
    BackfaceProbe,
    ClosedVolumeCube,
}

impl ShowcaseFigure {
    fn options() -> [(RadioId, &'static str); 5] {
        [
            (RadioId(101), "Far unlit quad"),
            (RadioId(102), "Textured vertex-color triangle"),
            (RadioId(103), "Alpha-mask quad"),
            (RadioId(104), "Backface probe"),
            (RadioId(105), "Closed volume cube"),
        ]
    }

    const fn radio_id(self) -> RadioId {
        match self {
            Self::FarUnlitQuad => RadioId(101),
            Self::TexturedVertexColorTriangle => RadioId(102),
            Self::AlphaMaskQuad => RadioId(103),
            Self::BackfaceProbe => RadioId(104),
            Self::ClosedVolumeCube => RadioId(105),
        }
    }

    fn from_selection(selection: Option<u32>) -> Option<Self> {
        match selection {
            Some(101) => Some(Self::FarUnlitQuad),
            Some(102) => Some(Self::TexturedVertexColorTriangle),
            Some(103) => Some(Self::AlphaMaskQuad),
            Some(104) => Some(Self::BackfaceProbe),
            Some(105) => Some(Self::ClosedVolumeCube),
            Some(_) | None => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ShowcaseTransformPolicy {
    local_z_radians_per_logical_pixel: f32,
    maximum_pointer_delta_logical_pixels: f32,
}

impl ShowcaseTransformPolicy {
    fn local_z_rotation(self, delta_x: f32) -> Result<Option<Orientation3>, String> {
        if !delta_x.is_finite() {
            return Err("Showcase transform pointer delta must be finite".to_string());
        }
        let delta_x = delta_x.clamp(
            -self.maximum_pointer_delta_logical_pixels,
            self.maximum_pointer_delta_logical_pixels,
        );
        if delta_x == 0.0 {
            return Ok(None);
        }
        local_z_orientation(delta_x * self.local_z_radians_per_logical_pixel).map(Some)
    }
}

impl Default for ShowcaseTransformPolicy {
    fn default() -> Self {
        Self {
            local_z_radians_per_logical_pixel: SHOWCASE_TRANSFORM_LOCAL_Z_RADIANS_PER_LOGICAL_PIXEL,
            maximum_pointer_delta_logical_pixels:
                SHOWCASE_TRANSFORM_MAXIMUM_POINTER_DELTA_LOGICAL_PIXELS,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ShowcaseTransformGesture {
    button: PointerButton,
    target: ShowcaseFigure,
    last_position: (f32, f32),
}

#[derive(Debug, Clone)]
struct ShowcaseFigurePlacements {
    far_unlit_quad: ModelPlacement3,
    textured_vertex_color_triangle: ModelPlacement3,
    alpha_mask_quad: ModelPlacement3,
    backface_probe: ModelPlacement3,
    closed_volume_cube: ModelPlacement3,
}

impl ShowcaseFigurePlacements {
    fn materialize(&self, spin_angle: Option<f32>) -> Result<Arc<[ModelPlacement3]>, String> {
        let backface_probe = match spin_angle {
            Some(angle) => {
                rotate_placement_local(self.backface_probe, local_z_orientation(angle)?)?
            }
            None => self.backface_probe,
        };
        Ok(Arc::from([
            self.far_unlit_quad,
            self.textured_vertex_color_triangle,
            self.alpha_mask_quad,
            backface_probe,
            self.closed_volume_cube,
        ]))
    }

    fn rotate_local_z(
        &mut self,
        target: ShowcaseFigure,
        local_rotation: Orientation3,
    ) -> Result<(), String> {
        let placement = match target {
            ShowcaseFigure::FarUnlitQuad => &mut self.far_unlit_quad,
            ShowcaseFigure::TexturedVertexColorTriangle => &mut self.textured_vertex_color_triangle,
            ShowcaseFigure::AlphaMaskQuad => &mut self.alpha_mask_quad,
            ShowcaseFigure::BackfaceProbe => &mut self.backface_probe,
            ShowcaseFigure::ClosedVolumeCube => &mut self.closed_volume_cube,
        };
        *placement = rotate_placement_local(*placement, local_rotation)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct WheelZoomTransition3 {
    handle: TransitionHandle,
    base: CameraState3,
    target: CameraState3,
    anchor: Option<WorldPoint3>,
}

#[derive(Debug, Default)]
struct ShowcaseWheelZoomTransition {
    transition: Option<WheelZoomTransition3>,
    cancellation_pending: bool,
}

impl ShowcaseWheelZoomTransition {
    fn request_cancellation(&mut self) {
        self.cancellation_pending = self.transition.is_some();
    }

    fn discard(&mut self) -> Option<WheelZoomTransition3> {
        self.cancellation_pending = false;
        self.transition.take()
    }

    fn cancel_in_event(&mut self, cx: &mut EventContext) {
        if let Some(transition) = self.discard() {
            cx.cancel_animation(transition.handle);
        }
    }

    fn cancel_in_update(&mut self, cx: &mut UpdateContext) {
        if let Some(transition) = self.discard() {
            cx.cancel_animation(transition.handle);
        }
    }

    fn needs_update(&self) -> bool {
        self.transition.is_some() || self.cancellation_pending
    }

    fn observe_preparation_outcome(&mut self, outcome: &CustomRenderPreparationOutcome) {
        if outcome.presentation_binding().is_none() {
            self.request_cancellation();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Viewport3DControlState {
    fly: bool,
    orthographic: bool,
    msaa_4x: bool,
    double_sided: bool,
    transform_target: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Viewport3DControlChange {
    request_changed: bool,
    focus_transition_required: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ShowcaseFlyKeyboardCapture {
    pending: bool,
}

impl ShowcaseFlyKeyboardCapture {
    fn new(fly: bool) -> Self {
        Self { pending: fly }
    }

    fn set_after_mode_change(&mut self, fly: bool, mode_changed: bool) {
        if mode_changed {
            self.pending = fly;
        }
    }

    fn pending(self) -> bool {
        self.pending
    }

    fn acknowledge_entry(&mut self) {
        self.pending = false;
    }
}

impl Viewport3DControlState {
    fn sample_count(self) -> SampleCount3D {
        if self.msaa_4x {
            SampleCount3D::FOUR
        } else {
            SampleCount3D::ONE
        }
    }

    fn selected_transform_target(self) -> Option<ShowcaseFigure> {
        ShowcaseFigure::from_selection(self.transform_target)
    }
}

pub(super) struct Viewport3DDemo {
    id: WidgetInternalId,
    viewport: Viewport3D,
    context: Arc<Context3D>,
    packet_templates: Vec<DrawPacketDescriptor>,
    placements: ShowcaseFigurePlacements,
    snapshot: RenderSnapshot,
    model: ModelState,
    _retained_texture_generation: RenderSnapshot,
    view_revision: Viewport3DViewRevision,
    observed_controls: Viewport3DControlState,
    active_controls: Viewport3DControlState,
    controller_error: Option<String>,
    spin_animation: Option<TransitionHandle>,
    spin_enabled: bool,
    last_spin_angle: Option<f32>,
    navigation: CameraNavigationController3,
    wheel_zoom: ShowcaseWheelZoomTransition,
    fly_keyboard_capture: ShowcaseFlyKeyboardCapture,
    navigation_button: Option<PointerButton>,
    navigation_cursor_hidden: bool,
    transform_policy: ShowcaseTransformPolicy,
    transform_gesture: Option<ShowcaseTransformGesture>,
    #[cfg(feature = "render-audit")]
    cadence: Viewport3DDemoCadence,
    #[cfg(feature = "render-audit")]
    last_cadence_sample: Option<Viewport3DDemoCadenceSample>,
}

#[cfg(feature = "render-audit")]
#[derive(Debug, Clone, Copy, Default)]
struct Viewport3DDemoCadence {
    admitted_updates: u64,
    model_revisions: u64,
    unchanged_model_samples: u64,
}

#[cfg(feature = "render-audit")]
#[derive(Debug, Clone, Copy)]
struct Viewport3DDemoCadenceSample {
    captured_at: Instant,
    demo: Viewport3DDemoCadence,
    viewport: halcyon::three_d::Viewport3DCadenceSnapshot,
    renderer: halcyon::renderer::RenderCadenceSnapshot,
}

impl Viewport3DDemo {
    fn new(gpu: GpuResources, controls: Viewport3DControlState) -> Result<Self, String> {
        let context = Arc::new(Context3D::new(gpu).map_err(|error| error.to_string())?);
        let SceneBuild {
            packet_templates,
            placements,
            retained_texture_generation,
        } = build_scene_packets(&context, controls.double_sided)?;
        let snapshot = admit_snapshot(
            &context,
            &packet_templates,
            controls.double_sided,
            StructuralRevision::new(2),
        )?;
        let model = admit_model(
            &context,
            &snapshot,
            placements.materialize(None)?,
            ModelRevision::new(1),
        )?;
        let view_revision = Viewport3DViewRevision::new(1);
        let camera = initial_camera(controls.orthographic)?;
        let policy = CameraNavigationPolicy3::default();
        let navigation =
            CameraNavigationController3::try_new(camera, policy, navigation_mode(controls.fly))
                .map_err(|error| error.to_string())?;
        let view = navigation.view_descriptor();
        let viewport = Viewport3D::new(
            context.clone(),
            view_revision,
            view,
            snapshot.clone(),
            model.clone(),
            controls.sample_count(),
        )
        .map_err(|error| error.to_string())?;

        Ok(Self {
            id: WidgetInternalId::new(),
            viewport,
            context,
            packet_templates,
            placements,
            snapshot,
            model,
            _retained_texture_generation: retained_texture_generation,
            view_revision,
            observed_controls: controls,
            active_controls: controls,
            controller_error: None,
            spin_animation: None,
            spin_enabled: true,
            last_spin_angle: None,
            navigation,
            wheel_zoom: ShowcaseWheelZoomTransition::default(),
            fly_keyboard_capture: ShowcaseFlyKeyboardCapture::new(controls.fly),
            navigation_button: None,
            navigation_cursor_hidden: false,
            transform_policy: ShowcaseTransformPolicy::default(),
            transform_gesture: None,
            #[cfg(feature = "render-audit")]
            cadence: Viewport3DDemoCadence::default(),
            #[cfg(feature = "render-audit")]
            last_cadence_sample: None,
        })
    }

    fn sync_controls(&mut self, controls: Viewport3DControlState) -> Viewport3DControlChange {
        if controls == self.observed_controls {
            return Viewport3DControlChange::default();
        }
        self.observed_controls = controls;

        match self.apply_controls(controls) {
            Ok(mode_changed) => {
                self.controller_error = None;
                Viewport3DControlChange {
                    request_changed: true,
                    focus_transition_required: mode_changed && !controls.fly,
                }
            }
            Err(error) => {
                self.controller_error = Some(error);
                Viewport3DControlChange {
                    request_changed: true,
                    focus_transition_required: false,
                }
            }
        }
    }

    fn apply_controls(&mut self, controls: Viewport3DControlState) -> Result<bool, String> {
        self.wheel_zoom.request_cancellation();
        let mode_changed = controls.fly != self.active_controls.fly;
        let structure_changed = controls.double_sided != self.active_controls.double_sided;
        let snapshot = if structure_changed {
            let revision = self
                .snapshot
                .structural_revision()
                .checked_next()
                .map_err(|error| error.to_string())?;
            admit_snapshot(
                &self.context,
                &self.packet_templates,
                controls.double_sided,
                revision,
            )?
        } else {
            self.snapshot.clone()
        };
        let model = if structure_changed {
            let revision = self
                .model
                .revision()
                .checked_next()
                .map_err(|error| error.to_string())?;
            admit_model(
                &self.context,
                &snapshot,
                self.placements.materialize(self.last_spin_angle)?,
                revision,
            )?
        } else {
            self.model.clone()
        };

        let mut navigation = self.navigation.clone();
        if controls.orthographic != self.active_controls.orthographic {
            navigation
                .set_projection(showcase_projection(controls.orthographic)?)
                .map_err(|error| error.to_string())?;
        }
        navigation.set_mode(navigation_mode(controls.fly));
        let view_changed = navigation.view_descriptor() != self.navigation.view_descriptor();
        let view_revision = if view_changed {
            self.view_revision
                .checked_next()
                .map_err(|error| error.to_string())?
        } else {
            self.view_revision
        };

        self.viewport
            .set_request(
                view_revision,
                navigation.view_descriptor(),
                snapshot.clone(),
                model.clone(),
                controls.sample_count(),
            )
            .map_err(|error| error.to_string())?;

        self.snapshot = snapshot;
        self.model = model;
        self.navigation = navigation;
        self.view_revision = view_revision;
        self.active_controls = controls;
        self.fly_keyboard_capture
            .set_after_mode_change(controls.fly, mode_changed);
        Ok(mode_changed)
    }

    fn fly_keyboard_capture_pending(&self) -> bool {
        self.fly_keyboard_capture.pending()
    }

    fn apply_camera(&mut self) {
        let result = (|| {
            let revision = self
                .view_revision
                .checked_next()
                .map_err(|error| error.to_string())?;
            self.viewport
                .set_request(
                    revision,
                    self.navigation.view_descriptor(),
                    self.snapshot.clone(),
                    self.model.clone(),
                    self.active_controls.sample_count(),
                )
                .map_err(|error| error.to_string())?;
            self.view_revision = revision;
            Ok::<(), String>(())
        })();

        match result {
            Ok(()) => self.controller_error = None,
            Err(error) => self.controller_error = Some(error),
        }
    }

    fn start_spin(&mut self, cx: &mut UpdateContext) -> Result<(), String> {
        let descriptor = AnimationDescriptor::new(
            Animation::new(
                0.0_f32,
                std::f32::consts::TAU,
                MODEL_SPIN_DURATION,
                Easing::Linear,
            ),
            Playback::Loop(RepeatCount::Infinite),
        );
        self.spin_animation = cx
            .start_animation_descriptor(descriptor)
            .map_err(|error| error.to_string())?;
        if self.spin_animation.is_none() {
            self.spin_enabled = false;
        }
        Ok(())
    }

    fn apply_spin(&mut self, angle: f32) -> Result<bool, String> {
        if self.last_spin_angle == Some(angle) {
            #[cfg(feature = "render-audit")]
            {
                self.cadence.unchanged_model_samples += 1;
            }
            return Ok(false);
        }
        self.publish_model_placements(self.placements.materialize(Some(angle))?)?;
        self.last_spin_angle = Some(angle);
        Ok(true)
    }

    fn publish_model_placements(
        &mut self,
        placements: Arc<[ModelPlacement3]>,
    ) -> Result<(), String> {
        let revision = self
            .model
            .revision()
            .checked_next()
            .map_err(|error| error.to_string())?;
        let model = admit_model(&self.context, &self.snapshot, placements, revision)?;
        self.viewport
            .set_request(
                self.view_revision,
                self.navigation.view_descriptor(),
                self.snapshot.clone(),
                model.clone(),
                self.active_controls.sample_count(),
            )
            .map_err(|error| error.to_string())?;
        self.model = model;
        #[cfg(feature = "render-audit")]
        {
            self.cadence.model_revisions += 1;
        }
        Ok(())
    }

    fn begin_transform_gesture(
        &mut self,
        operation: ShowcaseTransformOperation,
        button: PointerButton,
        target: ShowcaseFigure,
        position: (f32, f32),
    ) -> Result<bool, String> {
        if self.transform_gesture.is_some() {
            return Ok(false);
        }
        if !(position.0.is_finite() && position.1.is_finite()) {
            return Err("Showcase transform pointer position must be finite".to_string());
        }
        match operation {
            ShowcaseTransformOperation::TransformRotate => {
                self.transform_gesture = Some(ShowcaseTransformGesture {
                    button,
                    target,
                    last_position: position,
                });
                Ok(true)
            }
        }
    }

    fn transform_moved(&mut self, position: (f32, f32)) -> Result<bool, String> {
        let Some(gesture) = self.transform_gesture else {
            return Ok(false);
        };
        if !(position.0.is_finite() && position.1.is_finite()) {
            return Err("Showcase transform pointer position must be finite".to_string());
        }
        let Some(local_rotation) = self
            .transform_policy
            .local_z_rotation(position.0 - gesture.last_position.0)?
        else {
            self.transform_gesture = Some(ShowcaseTransformGesture {
                last_position: position,
                ..gesture
            });
            return Ok(false);
        };

        let mut placements = self.placements.clone();
        placements.rotate_local_z(gesture.target, local_rotation)?;
        self.publish_model_placements(placements.materialize(self.last_spin_angle)?)?;
        self.placements = placements;
        self.transform_gesture = Some(ShowcaseTransformGesture {
            last_position: position,
            ..gesture
        });
        Ok(true)
    }

    fn cancel_transform_gesture(&mut self) {
        self.transform_gesture = None;
    }

    fn reset_camera(&mut self, cx: &mut EventContext) {
        self.wheel_zoom.cancel_in_event(cx);
        let result = initial_camera(self.active_controls.orthographic).and_then(|camera| {
            self.navigation
                .reset(camera)
                .map_err(|error| error.to_string())
        });
        match result {
            Ok(true) => self.apply_camera(),
            Ok(false) => self.controller_error = None,
            Err(error) => self.controller_error = Some(error),
        }
    }

    fn committed_showcase_wheel_anchor(&self, global_position: (f32, f32)) -> Option<WorldPoint3> {
        if !matches!(
            self.navigation.state().projection(),
            Projection3::Perspective(_)
        ) {
            return None;
        }
        let current = self.viewport.current_prepared_target()?;
        let metrics = current.metrics()?;
        let committed = current.status().committed()?;
        if committed.view() != self.navigation.view_descriptor()
            || !committed.model().matches_snapshot(committed.snapshot())
        {
            return None;
        }
        let target_bounds = RectF::new(
            0.0,
            0.0,
            metrics.target_logical_extent.width,
            metrics.target_logical_extent.height,
        );
        let visible_rect = metrics
            .final_logical_rect
            .intersection(&metrics.final_logical_viewport)?
            .intersection(&target_bounds)?;
        if !(global_position.0.is_finite()
            && global_position.1.is_finite()
            && metrics.final_logical_rect.width.is_finite()
            && metrics.final_logical_rect.height.is_finite()
            && metrics.final_logical_rect.width > 0.0
            && metrics.final_logical_rect.height > 0.0
            && visible_rect.contains_point(global_position.0, global_position.1))
        {
            return None;
        }
        let u = f64::from(
            (global_position.0 - metrics.final_logical_rect.x) / metrics.final_logical_rect.width,
        );
        let v = f64::from(
            (global_position.1 - metrics.final_logical_rect.y) / metrics.final_logical_rect.height,
        );
        let ray = current.committed_ray(u, v).ok()??;
        nearest_showcase_triangle_hit(ray, committed)
    }

    fn restore_navigation_cursor(&mut self, cx: &mut EventContext) {
        if std::mem::take(&mut self.navigation_cursor_hidden) {
            cx.request_cursor(CursorIcon::Default);
        }
    }

    fn cancel_interaction_input(&mut self, cx: &mut EventContext) {
        self.wheel_zoom.cancel_in_event(cx);
        self.navigation.cancel_input();
        self.navigation_button = None;
        self.restore_navigation_cursor(cx);
        self.cancel_transform_gesture();
        cx.release_pointer();
    }

    fn diagnostics(&self) -> [String; 3] {
        let target_count = self.viewport.target_statuses().len();
        let Some(current) = self.viewport.current_prepared_target() else {
            return [
                "target: waiting for first custom-render preparation".to_string(),
                "requested: —  |  committed: —".to_string(),
                self.controller_status("lifecycle: uninitialized"),
            ];
        };
        let Some(metrics) = current.metrics() else {
            return [
                format!(
                    "target: SurfaceId({}) · metrics unavailable",
                    current.surface_id().raw()
                ),
                "requested: —  |  committed: —".to_string(),
                self.controller_status("lifecycle: target has no prepared metrics"),
            ];
        };

        let (extent, activity) = match metrics.activity {
            CustomRenderActivity::Active {
                physical_render_extent,
                ..
            } => (physical_render_extent, "active"),
            CustomRenderActivity::Inactive => {
                (halcyon::renderer::PhysicalExtent::new(0, 0), "inactive")
            }
        };
        let target = format!(
            "target: SurfaceId({}) · {}x{} px · DPR {:.2} · {} · targets {}",
            current.surface_id().raw(),
            extent.width,
            extent.height,
            metrics.scale_factor,
            activity,
            target_count,
        );

        let status = current.status();

        let requested = status
            .requested()
            .map(tuple_diagnostic)
            .unwrap_or_else(|| "—".to_string());
        let committed = status
            .committed()
            .map(tuple_diagnostic)
            .unwrap_or_else(|| "—".to_string());
        let revisions = format!("requested: {requested}  |  committed: {committed}");
        let lifecycle = if let Some(failure) = status.failure() {
            format!(
                "lifecycle: {:?} · request #{} · {}",
                status.state(),
                failure.request_serial().value(),
                failure.error(),
            )
        } else {
            format!("lifecycle: {:?} · error: none", status.state())
        };

        [target, revisions, self.controller_status(&lifecycle)]
    }

    fn controller_status(&self, lifecycle: &str) -> String {
        let mode = match self.navigation.mode() {
            CameraNavigationMode3::Orbit => "Orbit",
            CameraNavigationMode3::Fly => "Fly",
        };
        let position = self.navigation.state().position();
        let camera = format!(
            "mode: {mode} · camera: ({:.2}, {:.2}, {:.2}) · held: {}",
            position.x(),
            position.y(),
            position.z(),
            self.navigation.held_key_count(),
        );
        match &self.controller_error {
            Some(error) => format!("{lifecycle} · controller: {error} · {camera}"),
            None => format!("{lifecycle} · controller: ok · {camera}"),
        }
    }

    #[cfg(feature = "render-audit")]
    fn cadence_diagnostics(&mut self, now: Instant) -> Option<[String; 3]> {
        let current = Viewport3DDemoCadenceSample {
            captured_at: now,
            demo: self.cadence,
            viewport: halcyon::three_d::viewport_cadence_snapshot(),
            renderer: halcyon::renderer::render_cadence_snapshot(),
        };
        let previous = self.last_cadence_sample.replace(current);
        let Some(previous) = previous else {
            return Some([
                "cadence/s: admitted update — · model revision — · custom prepare — · encode —"
                    .to_string(),
                "cadence/s: submit — · swap — · front publish — · surface present call — · renderer advance —"
                    .to_string(),
                "audit window: warming up".to_string(),
            ]);
        };

        let elapsed = current
            .captured_at
            .duration_since(previous.captured_at)
            .as_secs_f64();
        let rate = |current: u64, previous: u64| {
            current.saturating_sub(previous) as f64 / elapsed.max(f64::EPSILON)
        };
        let viewport = current.viewport;
        let previous_viewport = previous.viewport;
        let renderer = current.renderer;
        let previous_renderer = previous.renderer;

        Some([
            format!(
                "cadence/s: admitted update {:>6.1} · model revision {:>6.1} · custom prepare {:>6.1} · encode {:>6.1}",
                rate(
                    current.demo.admitted_updates,
                    previous.demo.admitted_updates
                ),
                rate(current.demo.model_revisions, previous.demo.model_revisions),
                rate(
                    viewport.custom_preparations,
                    previous_viewport.custom_preparations,
                ),
                rate(viewport.encodes, previous_viewport.encodes),
            ),
            format!(
                "cadence/s: submit {:>6.1} · swap {:>6.1} · front publish {:>6.1} · surface present call {:>6.1} · renderer advance {:>6.1}",
                rate(viewport.submissions, previous_viewport.submissions),
                rate(viewport.swaps, previous_viewport.swaps),
                rate(viewport.publications, previous_viewport.publications),
                rate(
                    renderer.surface_presentations,
                    previous_renderer.surface_presentations,
                ),
                rate(
                    renderer.renderer_frame_advances,
                    previous_renderer.renderer_frame_advances,
                ),
            ),
            format!(
                "audit window: {:.0} ms · unchanged {} · inactive {} · failed {} · equal model samples {}",
                elapsed * 1_000.0,
                viewport
                    .unchanged_preparations
                    .saturating_sub(previous_viewport.unchanged_preparations),
                viewport
                    .inactive_preparations
                    .saturating_sub(previous_viewport.inactive_preparations),
                viewport
                    .failed_preparations
                    .saturating_sub(previous_viewport.failed_preparations),
                current
                    .demo
                    .unchanged_model_samples
                    .saturating_sub(previous.demo.unchanged_model_samples),
            ),
        ])
    }
}

impl WidgetDowncast for Viewport3DDemo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl CustomRenderParticipant for Viewport3DDemo {
    fn prepare_custom_render(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
    ) -> CustomRenderPreparationOutcome {
        let outcome = CustomRenderParticipant::prepare_custom_render(&mut self.viewport, metrics);
        self.wheel_zoom.observe_preparation_outcome(&outcome);
        outcome
    }
}

impl Widget for Viewport3DDemo {
    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        #[cfg(feature = "render-audit")]
        {
            self.cadence.admitted_updates += 1;
        }

        if self.wheel_zoom.cancellation_pending {
            self.wheel_zoom.cancel_in_update(cx);
        }

        match self.navigation.advance(dt) {
            Ok(true) => {
                self.apply_camera();
                cx.invalidate_paint();
            }
            Ok(false) => {}
            Err(error) => {
                self.wheel_zoom.cancel_in_update(cx);
                self.controller_error = Some(error.to_string());
                if self.navigation.active_pointer_operation().is_none() {
                    self.navigation.cancel_input();
                }
            }
        }

        if let Some(transition) = self.wheel_zoom.transition {
            match cx.sample_animation::<f64>(transition.handle) {
                Some(AnimationSample::Active(progress)) => match self
                    .navigation
                    .interpolate_wheel_zoom(transition.base, transition.target, progress)
                    .and_then(|state| self.navigation.apply_wheel_zoom_transition(state))
                {
                    Ok(true) => {
                        self.controller_error = None;
                        self.apply_camera();
                        cx.invalidate_paint();
                    }
                    Ok(false) => {}
                    Err(error) => {
                        self.wheel_zoom.cancel_in_update(cx);
                        self.controller_error = Some(error.to_string());
                    }
                },
                Some(AnimationSample::Completed(_)) => {
                    match self
                        .navigation
                        .apply_wheel_zoom_transition(transition.target)
                    {
                        Ok(true) => {
                            self.controller_error = None;
                            self.apply_camera();
                            cx.invalidate_paint();
                        }
                        Ok(false) => {}
                        Err(error) => {
                            cx.cancel_animation(transition.handle);
                            self.controller_error = Some(error.to_string());
                        }
                    }
                    self.wheel_zoom.discard();
                }
                Some(
                    AnimationSample::Cancelled
                    | AnimationSample::Missing
                    | AnimationSample::TypeMismatch,
                )
                | None => {
                    self.wheel_zoom.discard();
                }
            }
        }

        if !self.spin_enabled {
            return;
        }
        if self.spin_animation.is_none() {
            if let Err(error) = self.start_spin(cx) {
                self.controller_error = Some(error);
                self.spin_enabled = false;
                return;
            }
        }
        let Some(handle) = self.spin_animation else {
            return;
        };
        let sample = cx.sample_animation::<f32>(handle);
        let angle = match sample {
            Some(AnimationSample::Active(angle)) => angle,
            Some(
                AnimationSample::Completed(_)
                | AnimationSample::Cancelled
                | AnimationSample::Missing
                | AnimationSample::TypeMismatch,
            )
            | None => {
                cx.cancel_animation(handle);
                self.spin_animation = None;
                self.spin_enabled = false;
                return;
            }
        };
        match self.apply_spin(angle) {
            Ok(changed) => {
                self.controller_error = None;
                if changed {
                    cx.invalidate_paint();
                }
            }
            Err(error) => {
                cx.cancel_animation(handle);
                self.controller_error = Some(error);
                self.spin_enabled = false;
                self.spin_animation = None;
                return;
            }
        }
    }

    fn needs_update(&self) -> bool {
        self.spin_enabled || self.navigation.needs_update() || self.wheel_zoom.needs_update()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        self.viewport.layout(cx, constraints, children)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId]) {
        self.viewport.paint(cx, rect, children);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        if self.wheel_zoom.cancellation_pending {
            self.wheel_zoom.cancel_in_event(cx);
        }
        if let Event::Key(key) = event {
            let handled = self.navigation.handle_key_event(key, cx.keyboard_mode);
            if handled {
                cx.request_continuous_frames();
            }
            return handled;
        }
        let Event::Pointer(pointer) = event else {
            return false;
        };

        match pointer.kind {
            PointerEventKind::Down => {
                let Some(button) = pointer.button else {
                    return false;
                };
                if let Some(operation) = self.navigation.pointer_operation_for_button(button) {
                    self.wheel_zoom.cancel_in_event(cx);
                    match self
                        .navigation
                        .begin_pointer_operation(operation, pointer.local_pos)
                    {
                        Ok(true) => {
                            self.controller_error = None;
                            self.navigation_button = Some(button);
                            self.navigation_cursor_hidden =
                                operation == CameraPointerOperation3::FreeLook;
                            cx.request_focus();
                            cx.request_keyboard_capture();
                            cx.capture_pointer(button);
                            if self.navigation_cursor_hidden {
                                cx.request_cursor(CursorIcon::Hidden);
                            }
                            true
                        }
                        Ok(false) => false,
                        Err(error) => {
                            self.controller_error = Some(error.to_string());
                            false
                        }
                    }
                } else if self.navigation.mode() == CameraNavigationMode3::Fly
                    && button == PointerButton::Primary
                {
                    let Some(target) = self.active_controls.selected_transform_target() else {
                        return false;
                    };
                    match self.begin_transform_gesture(
                        ShowcaseTransformOperation::TransformRotate,
                        button,
                        target,
                        pointer.local_pos,
                    ) {
                        Ok(true) => {
                            self.controller_error = None;
                            cx.capture_pointer(button);
                            true
                        }
                        Ok(false) => false,
                        Err(error) => {
                            self.controller_error = Some(error);
                            false
                        }
                    }
                } else {
                    false
                }
            }
            PointerEventKind::DragStart
                if self.navigation.active_pointer_operation().is_some()
                    || self.transform_gesture.is_some() =>
            {
                true
            }
            PointerEventKind::Move if self.navigation.active_pointer_operation().is_some() => {
                match self.navigation.pointer_moved(pointer.local_pos) {
                    Ok(true) => {
                        self.controller_error = None;
                        self.apply_camera();
                        cx.invalidate_paint();
                    }
                    Ok(false) => {}
                    Err(error) => {
                        self.controller_error = Some(error.to_string());
                        self.cancel_interaction_input(cx);
                    }
                }
                true
            }
            PointerEventKind::Move if self.transform_gesture.is_some() => {
                match self.transform_moved(pointer.local_pos) {
                    Ok(true) => {
                        self.controller_error = None;
                        cx.invalidate_paint();
                    }
                    Ok(false) => {}
                    Err(error) => {
                        self.controller_error = Some(error);
                        self.cancel_transform_gesture();
                        cx.release_pointer();
                    }
                }
                true
            }
            PointerEventKind::Up if pointer.button == self.navigation_button => {
                self.navigation.end_pointer_operation();
                self.navigation_button = None;
                self.restore_navigation_cursor(cx);
                cx.release_pointer();
                true
            }
            PointerEventKind::Up
                if pointer.button == self.transform_gesture.map(|gesture| gesture.button) =>
            {
                self.cancel_transform_gesture();
                cx.release_pointer();
                true
            }
            PointerEventKind::Scroll { delta_y, .. } => {
                let (base, anchor) = match self.wheel_zoom.transition {
                    Some(transition) => (transition.target, transition.anchor),
                    None => (
                        self.navigation.state(),
                        self.committed_showcase_wheel_anchor(pointer.global_pos),
                    ),
                };
                let target = match (self.navigation.wheel_operation(), anchor) {
                    (CameraWheelOperation3::PerspectiveOpticalFovZoom, Some(anchor)) => self
                        .navigation
                        .wheel_zoom_target_anchored(base, delta_y, anchor),
                    (CameraWheelOperation3::PerspectiveOpticalFovZoom, None)
                    | (CameraWheelOperation3::OrthographicSpanZoom, _) => {
                        self.navigation.wheel_zoom_target(base, delta_y)
                    }
                };
                match target {
                    Ok(Some(target)) => {
                        self.wheel_zoom.cancel_in_event(cx);
                        let base = self.navigation.state();
                        let animation = Animation::new(
                            0.0_f64,
                            1.0_f64,
                            WHEEL_ZOOM_TRANSITION_DURATION,
                            Easing::EaseOut,
                        );
                        if let Some(handle) = cx.start_animation(animation) {
                            self.wheel_zoom.transition = Some(WheelZoomTransition3 {
                                handle,
                                base,
                                target,
                                anchor,
                            });
                        } else if let Err(error) =
                            self.navigation.apply_wheel_zoom_transition(target)
                        {
                            self.controller_error = Some(error.to_string());
                        } else {
                            self.controller_error = None;
                            self.apply_camera();
                        }
                        cx.invalidate_paint();
                    }
                    Ok(None) => {}
                    Err(error) => self.controller_error = Some(error.to_string()),
                }
                delta_y != 0.0
            }
            PointerEventKind::DoubleClick
                if self.navigation.mode() == CameraNavigationMode3::Orbit
                    && pointer.button
                        == Some(self.navigation.policy().pointer_bindings().orbit_rotate()) =>
            {
                self.reset_camera(cx);
                cx.invalidate_paint();
                true
            }
            _ => false,
        }
    }

    fn custom_render_admission(&self) -> Option<&CustomRenderAdmission> {
        self.viewport.custom_render_admission()
    }

    fn custom_render_participant_mut(&mut self) -> Option<&mut dyn CustomRenderParticipant> {
        Some(self)
    }

    fn on_unmount(&mut self) {
        self.wheel_zoom = ShowcaseWheelZoomTransition::default();
        self.navigation.cancel_input();
        self.navigation_button = None;
        self.navigation_cursor_hidden = false;
        self.cancel_transform_gesture();
        self.viewport.on_unmount();
    }

    fn on_capture_enter(&mut self, _cx: &mut EventContext) {
        self.fly_keyboard_capture.acknowledge_entry();
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        self.cancel_interaction_input(cx);
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        self.cancel_interaction_input(cx);
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }

    fn capture_eligible(&self) -> bool {
        true
    }

    fn wants_scroll(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "Viewport3DDemo"
    }
}

impl ShowcaseScreen {
    pub(super) fn build_viewport_3d(&mut self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        self.viewport_3d_last_diagnostics = None;
        #[cfg(feature = "render-audit")]
        {
            self.viewport_3d_cadence_label_ids = [None; 3];
        }
        let outer = cx.tree.add_child(
            parent,
            Box::new(
                Flex::column()
                    .with_spacing(6.0)
                    .with_padding(EdgeInsets::all(8.0)),
            ),
            None,
            0,
        );
        cx.tree.set_flex(outer, FlexParams::flexible(1.0));

        cx.tree.add_child(
            outer,
            Box::new(Label::new(
                "Public Viewport3D pressure test — near mask → vertex color → far unlit plane",
            )),
            None,
            0,
        );
        cx.tree.add_child(
            outer,
            Box::new(
                Label::new(
                    "Orbit: LMB orbit rotate · Fly: LMB rotates the selected figure locally, RMB free look, Space+RMB tether orbit, WASD + Shift · MMB camera pan · wheel: Perspective FOV (nearest-triangle anchor when valid; mask/cull may differ), orthographic span · Escape releases",
                )
                .with_size(11.0),
            ),
            None,
            1,
        );

        let controls = cx.tree.add_child(
            outer,
            Box::new(
                Flex::row()
                    .with_spacing(18.0)
                    .with_align_items(Alignment::Center),
            ),
            None,
            2,
        );
        let writer = self.binder.writer();
        let mut fly = Switch::on(self.viewport_3d_fly).with_label("Fly camera");
        fly.set_binding("viewport_3d_fly", writer.clone());
        self.viewport_3d_fly_control_id = Some(cx.tree.add_child(controls, Box::new(fly), None, 0));

        let mut projection = Switch::on(self.viewport_3d_orthographic).with_label("Orthographic");
        projection.set_binding("viewport_3d_orthographic", writer.clone());
        cx.tree.add_child(controls, Box::new(projection), None, 1);

        let mut msaa = Switch::on(self.viewport_3d_msaa_4x).with_label("4x MSAA");
        msaa.set_binding("viewport_3d_msaa_4x", writer.clone());
        cx.tree.add_child(controls, Box::new(msaa), None, 2);

        let mut double_sided =
            Switch::on(self.viewport_3d_double_sided).with_label("Double-sided probe");
        double_sided.set_binding("viewport_3d_double_sided", writer.clone());
        cx.tree.add_child(controls, Box::new(double_sided), None, 3);

        let transform_controls = cx.tree.add_child(
            outer,
            Box::new(
                Flex::row()
                    .with_spacing(12.0)
                    .with_align_items(Alignment::Center),
            ),
            None,
            3,
        );
        cx.tree.add_child(
            transform_controls,
            Box::new(Label::new("Fly LMB target (local +Z):").with_size(11.0)),
            None,
            0,
        );
        let options = ShowcaseFigure::options();
        let mut transform_target = RadioGroup::with_labeled_options(RadioGroupId(356), &options);
        transform_target.set_selected(
            ShowcaseFigure::from_selection(self.viewport_3d_transform_target)
                .map(ShowcaseFigure::radio_id),
        );
        transform_target.set_binding("viewport_3d_transform_target", writer);
        cx.tree
            .add_child(transform_controls, Box::new(transform_target), None, 1);

        let underlay = cx
            .tree
            .add_child(outer, Box::new(Panel::new(NodeId(0))), None, 4);
        cx.tree.set_flex(underlay, FlexParams::flexible(1.0));

        let controls = self.viewport_3d_controls();
        let viewport = cx
            .app
            .gpu_resources()
            .cloned()
            .ok_or_else(|| "GpuResources not available".to_string())
            .and_then(|gpu| Viewport3DDemo::new(gpu, controls));

        match viewport {
            Ok(viewport) => {
                let viewport_id = cx.tree.add_child(underlay, Box::new(viewport), None, 0);
                self.viewport_3d_id = Some(viewport_id);
                halcyon::replace_widget!(
                    cx.tree,
                    underlay,
                    Panel::new(viewport_id)
                        .with_padding(0.0)
                        .with_background(Color::from_hex(0x244D63))
                        .with_border(Color::from_hex(0x5C91AC), 1.0)
                        .with_radius(5.0)
                );
            }
            Err(error) => {
                let error_id = cx.tree.add_child(
                    underlay,
                    Box::new(Label::new(format!(
                        "Viewport3D initialization failed: {error}"
                    ))),
                    None,
                    0,
                );
                halcyon::replace_widget!(
                    cx.tree,
                    underlay,
                    Panel::new(error_id)
                        .with_padding(12.0)
                        .with_background(Color::from_hex(0x3C1D24))
                        .with_border(Color::from_hex(0xA9515F), 1.0)
                );
            }
        }

        cx.tree.add_child(
            outer,
            Box::new(
                Label::new(
                    "Public Texture/Sampler generations: live sRGB checker + retained pre-replacement snapshot.",
                )
                .with_size(11.0),
            ),
            None,
            5,
        );
        self.viewport_3d_target_label_id = Some(cx.tree.add_child(
            outer,
            Box::new(
                Label::new("target: waiting for first custom-render preparation").with_size(11.0),
            ),
            None,
            6,
        ));
        self.viewport_3d_revision_label_id = Some(cx.tree.add_child(
            outer,
            Box::new(Label::new("requested: —  |  committed: —").with_size(11.0)),
            None,
            7,
        ));
        self.viewport_3d_lifecycle_label_id = Some(cx.tree.add_child(
            outer,
            Box::new(Label::new("lifecycle: uninitialized · controller: ok").with_size(11.0)),
            None,
            8,
        ));
        #[cfg(feature = "render-audit")]
        {
            let initial = [
                "cadence/s: admitted update 99999.9 · model revision 99999.9 · custom prepare 99999.9 · encode 99999.9",
                "cadence/s: submit 99999.9 · swap 99999.9 · front publish 99999.9 · surface present call 99999.9 · renderer advance 99999.9",
                "audit window: 9999 ms · unchanged 999999 · inactive 999999 · failed 999999 · equal model samples 999999",
            ];
            for (index, text) in initial.into_iter().enumerate() {
                self.viewport_3d_cadence_label_ids[index] = Some(cx.tree.add_child(
                    outer,
                    Box::new(Label::new(text).with_size(11.0)),
                    None,
                    9 + index as i16,
                ));
            }
        }
    }

    pub(super) fn update_viewport_3d(&mut self, tree: &mut halcyon::core::widget_tree::WidgetTree) {
        let controls = self.viewport_3d_controls();
        let Some(viewport_id) = self.viewport_3d_id else {
            return;
        };

        let now = Instant::now();
        let diagnostics_due = self
            .viewport_3d_last_diagnostics
            .is_none_or(|last| now.duration_since(last) >= VIEWPORT_DIAGNOSTIC_INTERVAL);

        let result = tree.node_mut(viewport_id).and_then(|node| {
            node.widget_mut()
                .as_any_mut()
                .downcast_mut::<Viewport3DDemo>()
                .map(|viewport| {
                    let control_change = viewport.sync_controls(controls);
                    let diagnostics = (control_change.request_changed || diagnostics_due)
                        .then(|| viewport.diagnostics());
                    let fly_keyboard_capture_pending = viewport.fly_keyboard_capture_pending();
                    #[cfg(feature = "render-audit")]
                    let cadence = diagnostics_due
                        .then(|| viewport.cadence_diagnostics(now))
                        .flatten();
                    #[cfg(feature = "render-audit")]
                    {
                        (
                            control_change,
                            fly_keyboard_capture_pending,
                            diagnostics,
                            cadence,
                        )
                    }
                    #[cfg(not(feature = "render-audit"))]
                    {
                        (control_change, fly_keyboard_capture_pending, diagnostics)
                    }
                })
        });
        #[cfg(feature = "render-audit")]
        let Some((control_change, fly_keyboard_capture_pending, diagnostics, cadence)) = result
        else {
            return;
        };
        #[cfg(not(feature = "render-audit"))]
        let Some((control_change, fly_keyboard_capture_pending, diagnostics)) = result else {
            return;
        };

        if control_change.request_changed {
            tree.mark_paint_dirty(viewport_id);
        }
        if controls.fly && fly_keyboard_capture_pending {
            let Some(fly_control_id) = self.viewport_3d_fly_control_id else {
                return;
            };
            tree.defer_focus_sequence([fly_control_id, viewport_id]);
        } else if control_change.focus_transition_required {
            let Some(fly_control_id) = self.viewport_3d_fly_control_id else {
                return;
            };
            tree.defer_focus_sequence([fly_control_id]);
        }
        if diagnostics_due {
            self.viewport_3d_last_diagnostics = Some(now);
        }
        if let Some(diagnostics) = diagnostics {
            update_label(
                tree,
                self.viewport_3d_target_label_id,
                diagnostics[0].as_str(),
            );
            update_label(
                tree,
                self.viewport_3d_revision_label_id,
                diagnostics[1].as_str(),
            );
            update_label(
                tree,
                self.viewport_3d_lifecycle_label_id,
                diagnostics[2].as_str(),
            );
        }
        #[cfg(feature = "render-audit")]
        if let Some(cadence) = cadence {
            for (id, text) in self
                .viewport_3d_cadence_label_ids
                .into_iter()
                .zip(cadence.iter())
            {
                update_label(tree, id, text);
            }
        }
    }

    fn viewport_3d_controls(&self) -> Viewport3DControlState {
        Viewport3DControlState {
            fly: self.viewport_3d_fly,
            orthographic: self.viewport_3d_orthographic,
            msaa_4x: self.viewport_3d_msaa_4x,
            double_sided: self.viewport_3d_double_sided,
            transform_target: self.viewport_3d_transform_target,
        }
    }
}

fn update_label(tree: &mut halcyon::core::widget_tree::WidgetTree, id: Option<NodeId>, text: &str) {
    let Some(id) = id else {
        return;
    };
    let updated = tree.node_mut(id).is_some_and(|node| {
        node.widget_mut()
            .as_any_mut()
            .downcast_mut::<Label>()
            .is_some_and(|label| {
                if label.text == text {
                    false
                } else {
                    label.set_text(text);
                    true
                }
            })
    });
    if updated {
        tree.mark_paint_dirty(id);
    }
}

fn tuple_diagnostic(tuple: &halcyon::three_d::Viewport3DTuple) -> String {
    format!(
        "#{} v{} s{} m{} {}x",
        tuple.request_serial().value(),
        tuple.view_revision().value(),
        tuple.structural_revision().value(),
        tuple.model_revision().value(),
        tuple.sample_count().value(),
    )
}

fn nearest_showcase_triangle_hit(ray: Ray3, committed: &Viewport3DTuple) -> Option<WorldPoint3> {
    let packets = committed.snapshot().draw_packets();
    let placements = committed.model().placements();
    if packets.len() != 5
        || placements.len() != 5
        || packets
            .iter()
            .enumerate()
            .any(|(index, packet)| packet.stable_ordinal() != index as u64)
    {
        return None;
    }

    nearest_showcase_declared_triangle_hit(ray, placements)
}

fn nearest_showcase_declared_triangle_hit(
    ray: Ray3,
    placements: &[ModelPlacement3],
) -> Option<WorldPoint3> {
    if placements.len() != 5 {
        return None;
    }
    let mut nearest = None;
    for (ordinal, placement) in placements.iter().enumerate() {
        let local_ray = ray.to_local(*placement).ok()?;
        let triangles = match ordinal as u64 {
            0 | 2 => &SHOWCASE_QUAD_TRIANGLES[..],
            1 => &SHOWCASE_TRIANGLE[..],
            3 => &SHOWCASE_CLOCKWISE_TRIANGLE[..],
            4 => &SHOWCASE_CUBE_TRIANGLES[..],
            _ => return None,
        };
        for triangle in triangles {
            let hit = match local_ray
                .intersect_triangle(
                    point3(triangle[0].0, triangle[0].1, triangle[0].2),
                    point3(triangle[1].0, triangle[1].1, triangle[1].2),
                    point3(triangle[2].0, triangle[2].1, triangle[2].2),
                )
                .ok()?
            {
                Some(hit) => hit,
                None => continue,
            };
            let distance = hit.hit().world_distance();
            let point = ray.point_at(distance).ok()?;
            if nearest.is_none_or(|(nearest_distance, _)| distance < nearest_distance) {
                nearest = Some((distance, point));
            }
        }
    }
    nearest.map(|(_, point)| point)
}

fn admit_snapshot(
    context: &Context3D,
    packet_templates: &[DrawPacketDescriptor],
    double_sided: bool,
    revision: StructuralRevision,
) -> Result<RenderSnapshot, String> {
    let packets = packet_templates
        .iter()
        .map(|packet| {
            let raster_mode = if packet.stable_ordinal() == 3 {
                if double_sided {
                    RasterMode::DoubleSided
                } else {
                    RasterMode::SingleSided
                }
            } else {
                packet.raster_mode()
            };
            DrawPacketDescriptor::new(
                packet.geometry().clone(),
                packet.appearance().clone(),
                raster_mode,
                packet.stable_ordinal(),
            )
        })
        .collect();
    context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(revision, packets))
        .map_err(|error| error.to_string())
}

fn admit_model(
    context: &Context3D,
    snapshot: &RenderSnapshot,
    placements: Arc<[ModelPlacement3]>,
    revision: ModelRevision,
) -> Result<ModelState, String> {
    context
        .admit_model_state(
            snapshot,
            ModelStateDescriptor::from_shared(revision, placements),
        )
        .map_err(|error| error.to_string())
}

struct SceneBuild {
    packet_templates: Vec<DrawPacketDescriptor>,
    placements: ShowcaseFigurePlacements,
    retained_texture_generation: RenderSnapshot,
}

fn build_scene_packets(context: &Context3D, double_sided: bool) -> Result<SceneBuild, String> {
    let quad = context
        .register_geometry(quad_geometry())
        .map_err(|error| error.to_string())?;
    let triangle = context
        .register_geometry(triangle_geometry(false))
        .map_err(|error| error.to_string())?;
    let backface = context
        .register_geometry(triangle_geometry(true))
        .map_err(|error| error.to_string())?;
    let closed_volume = context
        .register_geometry(closed_volume_cube_geometry())
        .map_err(|error| error.to_string())?;

    let far_unlit = context
        .register_appearance(
            AppearanceSourceDescriptor::try_new(FAR_UNLIT_SOURCE, GeometrySemantics::NONE)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    let first_texture = context
        .register_texture(checker_texture(false)?)
        .map_err(|error| error.to_string())?;
    let first_sampler = context
        .register_sampler(Sampler3D::LINEAR_CLAMP)
        .map_err(|error| error.to_string())?;
    let textured_vertex_color = context
        .register_appearance(
            AppearanceSourceDescriptor::try_new(
                FALLBACK_TEXTURE_VERTEX_COLOR_SOURCE,
                GeometrySemantics::new(false, true, true),
            )
            .map_err(|error| error.to_string())?
            .with_texture(
                first_texture.handle().clone(),
                first_sampler.handle().clone(),
            ),
        )
        .map_err(|error| error.to_string())?;
    let alpha_mask = context
        .register_appearance(
            AppearanceSourceDescriptor::try_new_with_mode(
                ALPHA_MASK_SOURCE,
                GeometrySemantics::new(false, true, false),
                AppearanceMode::AlphaMask,
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    let backface_probe = context
        .register_appearance(
            AppearanceSourceDescriptor::try_new(BACKFACE_PROBE_SOURCE, GeometrySemantics::NONE)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    let closed_volume_appearance = context
        .register_appearance(
            AppearanceSourceDescriptor::try_new(
                CLOSED_VOLUME_SOURCE,
                GeometrySemantics::new(true, false, true),
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

    let mut packet_templates = vec![
        DrawPacketDescriptor::new(
            quad.handle().clone(),
            far_unlit.handle().clone(),
            RasterMode::SingleSided,
            0,
        ),
        DrawPacketDescriptor::new(
            triangle.handle().clone(),
            textured_vertex_color.handle().clone(),
            RasterMode::SingleSided,
            1,
        ),
        DrawPacketDescriptor::new(
            quad.handle().clone(),
            alpha_mask.handle().clone(),
            RasterMode::DoubleSided,
            2,
        ),
        DrawPacketDescriptor::new(
            backface.handle().clone(),
            backface_probe.handle().clone(),
            RasterMode::SingleSided,
            3,
        ),
        DrawPacketDescriptor::new(
            closed_volume.handle().clone(),
            closed_volume_appearance.handle().clone(),
            RasterMode::SingleSided,
            4,
        ),
    ];
    let placements = ShowcaseFigurePlacements {
        far_unlit_quad: placement(-0.15, 0.0, -4.3, 0.0, (1.8, 1.15))?,
        textured_vertex_color_triangle: placement(-0.35, -0.08, -2.85, -0.12, (1.25, 1.25))?,
        alpha_mask_quad: placement(0.42, 0.12, -2.05, 0.18, (0.78, 0.78))?,
        backface_probe: placement(1.14, -0.68, -2.35, 0.0, (0.54, 0.54))?,
        closed_volume_cube: closed_volume_cube_placement()?,
    };
    let retained_texture_generation = admit_snapshot(
        context,
        &packet_templates,
        double_sided,
        StructuralRevision::new(1),
    )?;

    let replacement_texture = context
        .replace_texture(first_texture.handle(), checker_texture(true)?)
        .map_err(|error| error.to_string())?;
    let replacement_sampler = context
        .replace_sampler(
            first_sampler.handle(),
            Sampler3D::new(
                SamplerAddressMode::Repeat,
                SamplerAddressMode::Repeat,
                SamplerFilterMode::Nearest,
                SamplerFilterMode::Nearest,
            ),
        )
        .map_err(|error| error.to_string())?;
    let replacement_appearance = context
        .replace_appearance(
            textured_vertex_color.handle(),
            AppearanceSourceDescriptor::try_new(
                FALLBACK_TEXTURE_VERTEX_COLOR_SOURCE,
                GeometrySemantics::new(false, true, true),
            )
            .map_err(|error| error.to_string())?
            .with_texture(
                replacement_texture.handle().clone(),
                replacement_sampler.handle().clone(),
            ),
        )
        .map_err(|error| error.to_string())?;
    let textured_packet = &packet_templates[1];
    packet_templates[1] = DrawPacketDescriptor::new(
        textured_packet.geometry().clone(),
        replacement_appearance.handle().clone(),
        textured_packet.raster_mode(),
        textured_packet.stable_ordinal(),
    );

    Ok(SceneBuild {
        packet_templates,
        placements,
        retained_texture_generation,
    })
}

fn checker_texture(replacement: bool) -> Result<Texture3D, String> {
    const EXTENT: u32 = 8;
    let mut texels = Vec::with_capacity((EXTENT * EXTENT * 4) as usize);
    for y in 0..EXTENT {
        for x in 0..EXTENT {
            let even = (x + y) % 2 == 0;
            let rgba = match (replacement, even) {
                (false, true) => [255, 50, 82, 255],
                (false, false) => [38, 92, 255, 255],
                (true, true) => [255, 214, 64, 255],
                (true, false) => [25, 229, 190, 255],
            };
            texels.extend_from_slice(&rgba);
        }
    }
    Texture3D::try_new(EXTENT, EXTENT, TextureFormat3D::Rgba8UnormSrgb, texels)
        .map_err(|error| error.to_string())
}

fn quad_geometry() -> GeometrySourceDescriptor {
    let positions = vec![
        point(-1.0, -1.0),
        point(1.0, -1.0),
        point(1.0, 1.0),
        point(-1.0, -1.0),
        point(1.0, 1.0),
        point(-1.0, 1.0),
    ];
    let uvs = vec![
        uv(0.0, 0.0),
        uv(1.0, 0.0),
        uv(1.0, 1.0),
        uv(0.0, 0.0),
        uv(1.0, 1.0),
        uv(0.0, 1.0),
    ];
    let colors = vec![
        color(34, 122, 178),
        color(35, 204, 156),
        color(122, 86, 224),
        color(34, 122, 178),
        color(122, 86, 224),
        color(35, 204, 156),
    ];
    GeometrySourceDescriptor::try_new(
        positions,
        None,
        Some(uvs),
        Some(colors),
        GeometryIndices::Unindexed,
    )
    .expect("valid Showcase quad geometry")
}

fn triangle_geometry(clockwise: bool) -> GeometrySourceDescriptor {
    let mut positions = vec![point(-0.9, -0.72), point(0.9, -0.72), point(0.0, 0.96)];
    let mut uvs = vec![uv(0.0, 0.0), uv(1.0, 0.0), uv(0.5, 1.0)];
    let mut colors = vec![color(255, 45, 88), color(30, 225, 135), color(55, 120, 255)];
    if clockwise {
        positions.swap(1, 2);
        uvs.swap(1, 2);
        colors.swap(1, 2);
    }
    GeometrySourceDescriptor::try_new(
        positions,
        None,
        Some(uvs),
        Some(colors),
        GeometryIndices::Unindexed,
    )
    .expect("valid Showcase triangle geometry")
}

fn closed_volume_cube_geometry() -> GeometrySourceDescriptor {
    let positions = vec![
        point3(-0.5, -0.5, 0.5),
        point3(0.5, -0.5, 0.5),
        point3(0.5, 0.5, 0.5),
        point3(-0.5, 0.5, 0.5),
        point3(0.5, -0.5, -0.5),
        point3(-0.5, -0.5, -0.5),
        point3(-0.5, 0.5, -0.5),
        point3(0.5, 0.5, -0.5),
        point3(0.5, -0.5, 0.5),
        point3(0.5, -0.5, -0.5),
        point3(0.5, 0.5, -0.5),
        point3(0.5, 0.5, 0.5),
        point3(-0.5, -0.5, -0.5),
        point3(-0.5, -0.5, 0.5),
        point3(-0.5, 0.5, 0.5),
        point3(-0.5, 0.5, -0.5),
        point3(-0.5, 0.5, 0.5),
        point3(0.5, 0.5, 0.5),
        point3(0.5, 0.5, -0.5),
        point3(-0.5, 0.5, -0.5),
        point3(-0.5, -0.5, -0.5),
        point3(0.5, -0.5, -0.5),
        point3(0.5, -0.5, 0.5),
        point3(-0.5, -0.5, 0.5),
    ];
    let mut normals = Vec::with_capacity(24);
    for normal in [
        vector(0.0, 0.0, 1.0),
        vector(0.0, 0.0, -1.0),
        vector(1.0, 0.0, 0.0),
        vector(-1.0, 0.0, 0.0),
        vector(0.0, 1.0, 0.0),
        vector(0.0, -1.0, 0.0),
    ] {
        normals.extend([normal; 4]);
    }
    let mut colors = Vec::with_capacity(24);
    for face_color in [
        color(72, 168, 255),
        color(38, 86, 178),
        color(86, 218, 190),
        color(49, 132, 117),
        color(255, 196, 94),
        color(199, 90, 154),
    ] {
        colors.extend([face_color; 4]);
    }
    GeometrySourceDescriptor::try_new(
        positions,
        Some(normals),
        None,
        Some(colors),
        GeometryIndices::from_u16(vec![
            0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15, 16,
            17, 18, 16, 18, 19, 20, 21, 22, 20, 22, 23,
        ]),
    )
    .expect("valid closed Showcase cube geometry")
}

fn point(x: f32, y: f32) -> LocalPoint3 {
    point3(x, y, 0.0)
}

fn point3(x: f32, y: f32, z: f32) -> LocalPoint3 {
    LocalPoint3::try_new(x, y, z).expect("finite Showcase point")
}

fn vector(x: f32, y: f32, z: f32) -> LocalVector3 {
    LocalVector3::try_new(x, y, z).expect("finite Showcase vector")
}

fn uv(u: f32, v: f32) -> TextureCoordinate2 {
    TextureCoordinate2::try_new(u, v).expect("finite Showcase texture coordinate")
}

fn color(red: u8, green: u8, blue: u8) -> LinearVertexColorRgba8 {
    LinearVertexColorRgba8::from_linear_unorm8(red, green, blue, 255)
}

fn placement(
    x: f64,
    y: f64,
    z: f64,
    rotation_z: f32,
    scale: (f32, f32),
) -> Result<ModelPlacement3, String> {
    let orientation = local_z_orientation(rotation_z)?;
    ModelPlacement3::try_new(
        WorldPoint3::try_new(x, y, z).map_err(|error| error.to_string())?,
        orientation,
        PositiveScale3::try_new(scale.0, scale.1, 1.0).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn closed_volume_cube_placement() -> Result<ModelPlacement3, String> {
    let half_yaw = -0.55_f32 * 0.5;
    let half_pitch = 0.35_f32 * 0.5;
    let orientation = Orientation3::try_from_xyzw(
        half_pitch.sin() * half_yaw.cos(),
        half_yaw.sin() * half_pitch.cos(),
        -half_yaw.sin() * half_pitch.sin(),
        half_yaw.cos() * half_pitch.cos(),
    )
    .map_err(|error| error.to_string())?;
    ModelPlacement3::try_new(
        WorldPoint3::try_new(1.12, 0.42, -3.15).map_err(|error| error.to_string())?,
        orientation,
        PositiveScale3::try_new(0.62, 0.62, 0.62).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn local_z_orientation(angle: f32) -> Result<Orientation3, String> {
    if !angle.is_finite() {
        return Err("Showcase local-Z rotation must be finite".to_string());
    }
    let half = angle * 0.5;
    Orientation3::try_from_xyzw(0.0, 0.0, half.sin(), half.cos()).map_err(|error| error.to_string())
}

fn rotate_placement_local(
    placement: ModelPlacement3,
    local_rotation: Orientation3,
) -> Result<ModelPlacement3, String> {
    ModelPlacement3::try_new(
        placement.translation(),
        placement
            .orientation()
            .compose_local(local_rotation)
            .map_err(|error| error.to_string())?,
        placement.scale(),
    )
    .map_err(|error| error.to_string())
}

fn navigation_mode(fly: bool) -> CameraNavigationMode3 {
    if fly {
        CameraNavigationMode3::Fly
    } else {
        CameraNavigationMode3::Orbit
    }
}

fn showcase_projection(orthographic: bool) -> Result<Projection3, String> {
    if orthographic {
        OrthographicProjection::try_new(5.0, 0.1, 100.0)
            .map(Projection3::from)
            .map_err(|error| error.to_string())
    } else {
        PerspectiveProjection::try_finite(55.0_f32.to_radians(), 0.1, 100.0)
            .map(Projection3::from)
            .map_err(|error| error.to_string())
    }
}

fn initial_camera(orthographic: bool) -> Result<CameraState3, String> {
    CameraState3::try_orbit(
        WorldPoint3::try_new(0.0, 0.0, -3.0).map_err(|error| error.to_string())?,
        0.0,
        0.0,
        3.8,
        showcase_projection(orthographic)?,
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use halcyon::core::animation::{AnimationContext, Timeline};
    use halcyon::core::app::{
        AppContext, ModalStack, ScreenContext, ScreenStack, compute_absolute_paint_rects,
        compute_absolute_positions,
    };
    use halcyon::core::event::{FusedInputEvent, InteractionDomainState, PointerDevice, WindowId};
    use halcyon::core::event_router::EventRouter;
    use halcyon::core::layout::Constraints;
    use halcyon::core::math::{Point, RectF};
    use halcyon::core::rect_authority::LayoutAuthorityGuard;
    use halcyon::core::theme::Theme;
    use halcyon::core::widget::{LayoutContext, Widget};
    use halcyon::core::widget_tree::WidgetTree;
    use halcyon::model::form_binder::FormBinder;
    use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
    use halcyon::runtime::binding_registry::BindingRegistry;
    use halcyon::three_d::WorldVector3;
    use std::time::Instant;

    #[test]
    fn showcase_fly_capture_entry_remains_pending_until_router_acknowledges_it() {
        let mut capture = ShowcaseFlyKeyboardCapture::new(true);
        assert!(capture.pending());

        assert!(
            capture.pending(),
            "scheduling a deferred focus route is not capture acknowledgement"
        );
        capture.acknowledge_entry();
        assert!(!capture.pending());

        capture.set_after_mode_change(false, true);
        assert!(!capture.pending());
        capture.set_after_mode_change(true, true);
        assert!(capture.pending());
    }

    fn placements() -> ShowcaseFigurePlacements {
        ShowcaseFigurePlacements {
            far_unlit_quad: placement(-0.15, 0.0, -4.3, 0.0, (1.8, 1.15)).unwrap(),
            textured_vertex_color_triangle: placement(-0.35, -0.08, -2.85, -0.12, (1.25, 1.25))
                .unwrap(),
            alpha_mask_quad: placement(0.42, 0.12, -2.05, 0.18, (0.78, 0.78)).unwrap(),
            backface_probe: placement(1.14, -0.68, -2.35, 0.0, (0.54, 0.54)).unwrap(),
            closed_volume_cube: closed_volume_cube_placement().unwrap(),
        }
    }

    #[test]
    fn showcase_closed_volume_cube_is_closed_with_outward_winding_and_normals() {
        let cube = closed_volume_cube_geometry();
        assert_eq!(cube.vertex_count(), 24);
        assert_eq!(cube.index_count(), 36);
        assert_eq!(cube.semantics(), GeometrySemantics::new(true, false, true));
        let normals = cube.normals().unwrap();
        assert_eq!(normals.len(), 24);
        assert_eq!(cube.colors().unwrap().len(), 24);
        let GeometryIndices::U16(indices) = cube.indices() else {
            panic!("closed Showcase cube must use indexed triangle geometry");
        };
        assert_eq!(indices.len(), 36);

        for face in normals.chunks_exact(4) {
            assert!(face.iter().all(|normal| *normal == face[0]));
            let axis_count = [face[0].x(), face[0].y(), face[0].z()]
                .into_iter()
                .filter(|component| component.abs() == 1.0)
                .count();
            assert_eq!(axis_count, 1, "each cube face must carry one axis normal");
        }

        let positions = cube.positions();
        let mut edge_counts = std::collections::BTreeMap::new();
        for triangle in indices.chunks_exact(3) {
            let a = positions[usize::from(triangle[0])];
            let b = positions[usize::from(triangle[1])];
            let c = positions[usize::from(triangle[2])];
            let ab = (b.x() - a.x(), b.y() - a.y(), b.z() - a.z());
            let ac = (c.x() - a.x(), c.y() - a.y(), c.z() - a.z());
            let cross = (
                ab.1 * ac.2 - ab.2 * ac.1,
                ab.2 * ac.0 - ab.0 * ac.2,
                ab.0 * ac.1 - ab.1 * ac.0,
            );
            let normal = normals[usize::from(triangle[0])];
            let outward_dot = cross.0 * normal.x() + cross.1 * normal.y() + cross.2 * normal.z();
            assert!(
                outward_dot > 0.0,
                "each triangle must wind CCW from its outward face"
            );
            let centroid = (
                (a.x() + b.x() + c.x()) / 3.0,
                (a.y() + b.y() + c.y()) / 3.0,
                (a.z() + b.z() + c.z()) / 3.0,
            );
            assert!(
                centroid.0 * normal.x() + centroid.1 * normal.y() + centroid.2 * normal.z() > 0.0,
                "each cube face normal must point away from the local origin"
            );

            let key = |point: LocalPoint3| {
                (
                    (point.x() * 2.0) as i8,
                    (point.y() * 2.0) as i8,
                    (point.z() * 2.0) as i8,
                )
            };
            for (left, right) in [(key(a), key(b)), (key(b), key(c)), (key(c), key(a))] {
                let edge = if left < right {
                    (left, right)
                } else {
                    (right, left)
                };
                *edge_counts.entry(edge).or_insert(0_u8) += 1;
            }
        }
        assert_eq!(
            edge_counts.len(),
            18,
            "cube faces must form one closed triangular shell"
        );
        assert!(
            edge_counts.values().all(|count| *count == 2),
            "every closed cube edge, including each face diagonal, must have two incident triangles"
        );
    }

    #[test]
    fn showcase_anchor_search_skips_individual_misses_and_reaches_declared_geometry() {
        let away = placement(100.0, 100.0, -2.0, 0.0, (1.0, 1.0)).unwrap();
        let mut placements = [away; 5];
        placements[0] = placement(0.0, 0.0, -2.0, 0.0, (1.0, 1.0)).unwrap();
        let second_quad_triangle_ray = Ray3::try_new(
            WorldPoint3::try_new(-0.75, 0.75, 0.0).unwrap(),
            WorldVector3::try_new(0.0, 0.0, -1.0).unwrap(),
        )
        .unwrap();
        let second_quad_triangle_hit =
            nearest_showcase_declared_triangle_hit(second_quad_triangle_ray, &placements).unwrap();
        assert!((second_quad_triangle_hit.x() + 0.75).abs() < 1.0e-12);
        assert!((second_quad_triangle_hit.y() - 0.75).abs() < 1.0e-12);
        assert!((second_quad_triangle_hit.z() + 2.0).abs() < 1.0e-12);

        placements[0] = away;
        placements[4] = ModelPlacement3::try_new(
            WorldPoint3::try_new(0.0, 0.0, -3.0).unwrap(),
            Orientation3::identity(),
            PositiveScale3::one(),
        )
        .unwrap();
        let cube_ray = Ray3::try_new(
            WorldPoint3::try_new(0.25, -0.1, 0.0).unwrap(),
            WorldVector3::try_new(0.0, 0.0, -1.0).unwrap(),
        )
        .unwrap();
        let cube_hit = nearest_showcase_declared_triangle_hit(cube_ray, &placements).unwrap();
        assert!((cube_hit.x() - 0.25).abs() < 1.0e-12);
        assert!((cube_hit.y() + 0.1).abs() < 1.0e-12);
        assert!((cube_hit.z() + 2.5).abs() < 1.0e-12);
    }

    #[test]
    fn three_d_showcase_transform_rotates_only_the_selected_named_figure() {
        let baseline = placements();
        let before = baseline.materialize(Some(0.4)).unwrap();
        let mut transformed = baseline.clone();
        transformed
            .rotate_local_z(
                ShowcaseFigure::AlphaMaskQuad,
                local_z_orientation(0.5).unwrap(),
            )
            .unwrap();
        let after = transformed.materialize(Some(0.4)).unwrap();

        assert_eq!(after[0], before[0]);
        assert_eq!(after[1], before[1]);
        assert_eq!(after[3], before[3]);
        assert_eq!(after[4], before[4]);
        assert_eq!(after[2].translation(), before[2].translation());
        assert_eq!(after[2].scale(), before[2].scale());
        assert_ne!(after[2].orientation(), before[2].orientation());

        let mut transformed_probe = baseline;
        transformed_probe
            .rotate_local_z(
                ShowcaseFigure::BackfaceProbe,
                local_z_orientation(0.5).unwrap(),
            )
            .unwrap();
        let probe_after = transformed_probe.materialize(Some(0.4)).unwrap();
        assert_eq!(probe_after[3].translation(), before[3].translation());
        assert_eq!(probe_after[3].scale(), before[3].scale());
        assert_ne!(probe_after[3].orientation(), before[3].orientation());
        assert_eq!(probe_after[4], before[4]);
    }

    #[test]
    fn routed_figure_selector_click_binds_and_targets_the_named_fly_transform() {
        let mut binder = FormBinder::new();
        let mut selector =
            RadioGroup::with_labeled_options(RadioGroupId(356), &ShowcaseFigure::options());
        selector.set_binding("viewport_3d_transform_target", binder.writer());

        let theme = Theme::dark();
        let mut layout_tree = WidgetTree::new();
        let mut layout_text_engine = FontManagerTextEngine::new();
        let layout = {
            let mut cx = LayoutContext::new(&mut layout_tree, &mut layout_text_engine, &theme);
            selector.layout(&mut cx, Constraints::unbounded(), &[])
        };

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(selector), None, 0);
        {
            let _layout = LayoutAuthorityGuard::enter();
            tree.set_rect(
                root,
                RectF::new(0.0, 0.0, layout.size.width, layout.size.height),
            );
            compute_absolute_positions(&mut tree, root, Point::zero());
            compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }

        let mut router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();
        let mut text_engine = FontManagerTextEngine::new();
        let mut bindings = BindingRegistry::new();
        let row_stride = (layout.size.height + 1.0) / ShowcaseFigure::options().len() as f32;

        for (index, (radio_id, _)) in ShowcaseFigure::options().iter().enumerate() {
            let y = index as f32 * row_stride + 1.0;
            router.route_input_event(
                &mut tree,
                FusedInputEvent::PointerDown {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos: Point::new(1.0, y),
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                    click_count: 1,
                },
                Some(root),
                &mut text_engine,
                None,
                &mut domain_state,
                &mut bindings,
            );
            router.route_input_event(
                &mut tree,
                FusedInputEvent::PointerUp {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos: Point::new(1.0, y),
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                },
                Some(root),
                &mut text_engine,
                None,
                &mut domain_state,
                &mut bindings,
            );

            let selected = tree
                .node(root)
                .and_then(|node| node.widget().as_any().downcast_ref::<RadioGroup>())
                .and_then(RadioGroup::selected);
            assert_eq!(selected, Some(*radio_id));

            let mut app = AppContext::new(Theme::dark());
            let mut screens = ScreenStack::new();
            let mut modals = ModalStack::new();
            let binding_cx = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
            let mut controls = Viewport3DControlState {
                fly: true,
                orthographic: false,
                msaa_4x: false,
                double_sided: false,
                transform_target: None,
            };
            binder.bind(&binding_cx, |binding| {
                binding
                    .selection("viewport_3d_transform_target")
                    .sync(&mut controls.transform_target);
            });

            let target = controls
                .selected_transform_target()
                .expect("a routed selector value must resolve to a named Showcase figure");
            assert_eq!(target.radio_id(), *radio_id);

            let baseline = placements();
            let mut transformed = baseline.clone();
            transformed
                .rotate_local_z(target, local_z_orientation(0.5).unwrap())
                .unwrap();
            match target {
                ShowcaseFigure::FarUnlitQuad => {
                    assert_eq!(
                        transformed.textured_vertex_color_triangle,
                        baseline.textured_vertex_color_triangle
                    );
                    assert_eq!(transformed.alpha_mask_quad, baseline.alpha_mask_quad);
                    assert_eq!(transformed.backface_probe, baseline.backface_probe);
                    assert_eq!(transformed.closed_volume_cube, baseline.closed_volume_cube);
                    assert_eq!(
                        transformed.far_unlit_quad.translation(),
                        baseline.far_unlit_quad.translation()
                    );
                    assert_eq!(
                        transformed.far_unlit_quad.scale(),
                        baseline.far_unlit_quad.scale()
                    );
                    assert_ne!(
                        transformed.far_unlit_quad.orientation(),
                        baseline.far_unlit_quad.orientation()
                    );
                }
                ShowcaseFigure::TexturedVertexColorTriangle => {
                    assert_eq!(transformed.far_unlit_quad, baseline.far_unlit_quad);
                    assert_eq!(transformed.alpha_mask_quad, baseline.alpha_mask_quad);
                    assert_eq!(transformed.backface_probe, baseline.backface_probe);
                    assert_eq!(transformed.closed_volume_cube, baseline.closed_volume_cube);
                    assert_eq!(
                        transformed.textured_vertex_color_triangle.translation(),
                        baseline.textured_vertex_color_triangle.translation()
                    );
                    assert_eq!(
                        transformed.textured_vertex_color_triangle.scale(),
                        baseline.textured_vertex_color_triangle.scale()
                    );
                    assert_ne!(
                        transformed.textured_vertex_color_triangle.orientation(),
                        baseline.textured_vertex_color_triangle.orientation()
                    );
                }
                ShowcaseFigure::AlphaMaskQuad => {
                    assert_eq!(transformed.far_unlit_quad, baseline.far_unlit_quad);
                    assert_eq!(
                        transformed.textured_vertex_color_triangle,
                        baseline.textured_vertex_color_triangle
                    );
                    assert_eq!(transformed.backface_probe, baseline.backface_probe);
                    assert_eq!(transformed.closed_volume_cube, baseline.closed_volume_cube);
                    assert_eq!(
                        transformed.alpha_mask_quad.translation(),
                        baseline.alpha_mask_quad.translation()
                    );
                    assert_eq!(
                        transformed.alpha_mask_quad.scale(),
                        baseline.alpha_mask_quad.scale()
                    );
                    assert_ne!(
                        transformed.alpha_mask_quad.orientation(),
                        baseline.alpha_mask_quad.orientation()
                    );
                }
                ShowcaseFigure::BackfaceProbe => {
                    assert_eq!(transformed.far_unlit_quad, baseline.far_unlit_quad);
                    assert_eq!(
                        transformed.textured_vertex_color_triangle,
                        baseline.textured_vertex_color_triangle
                    );
                    assert_eq!(transformed.alpha_mask_quad, baseline.alpha_mask_quad);
                    assert_eq!(transformed.closed_volume_cube, baseline.closed_volume_cube);
                    assert_eq!(
                        transformed.backface_probe.translation(),
                        baseline.backface_probe.translation()
                    );
                    assert_eq!(
                        transformed.backface_probe.scale(),
                        baseline.backface_probe.scale()
                    );
                    assert_ne!(
                        transformed.backface_probe.orientation(),
                        baseline.backface_probe.orientation()
                    );
                }
                ShowcaseFigure::ClosedVolumeCube => {
                    assert_eq!(transformed.far_unlit_quad, baseline.far_unlit_quad);
                    assert_eq!(
                        transformed.textured_vertex_color_triangle,
                        baseline.textured_vertex_color_triangle
                    );
                    assert_eq!(transformed.alpha_mask_quad, baseline.alpha_mask_quad);
                    assert_eq!(transformed.backface_probe, baseline.backface_probe);
                    assert_eq!(
                        transformed.closed_volume_cube.translation(),
                        baseline.closed_volume_cube.translation()
                    );
                    assert_eq!(
                        transformed.closed_volume_cube.scale(),
                        baseline.closed_volume_cube.scale()
                    );
                    assert_ne!(
                        transformed.closed_volume_cube.orientation(),
                        baseline.closed_volume_cube.orientation()
                    );
                }
            }
        }
    }

    #[test]
    fn showcase_unavailable_presentation_cancels_wheel_and_anchor_before_any_later_sample() {
        let mut timeline = Timeline::new();
        let handle = timeline
            .start(Animation::new(
                0.0_f64,
                1.0_f64,
                WHEEL_ZOOM_TRANSITION_DURATION,
                Easing::EaseOut,
            ))
            .unwrap();
        let camera = initial_camera(false).unwrap();
        let anchor = WorldPoint3::try_new(0.25, -0.2, -2.0).unwrap();
        let mut wheel_zoom = ShowcaseWheelZoomTransition {
            transition: Some(WheelZoomTransition3 {
                handle,
                base: camera,
                target: camera,
                anchor: Some(anchor),
            }),
            cancellation_pending: false,
        };

        wheel_zoom.observe_preparation_outcome(&CustomRenderPreparationOutcome::default());
        assert!(wheel_zoom.needs_update());
        {
            let animation = AnimationContext::new(&mut timeline);
            let mut cx = UpdateContext::new(Some(animation));
            wheel_zoom.cancel_in_update(&mut cx);
        }

        assert!(wheel_zoom.transition.is_none());
        assert!(!wheel_zoom.cancellation_pending);
        assert!(!wheel_zoom.needs_update());
        assert_eq!(timeline.sample::<f64>(handle), AnimationSample::Cancelled);
    }

    #[test]
    fn showcase_perspective_wheel_rebases_outward_scroll_to_the_default_fov() {
        let mut navigation = CameraNavigationController3::try_new(
            initial_camera(false).unwrap(),
            CameraNavigationPolicy3::default(),
            CameraNavigationMode3::Fly,
        )
        .unwrap();
        let initial = navigation.state();
        let narrow_target = navigation
            .wheel_zoom_target(initial, 56.0)
            .unwrap()
            .unwrap();
        let displayed = navigation
            .interpolate_wheel_zoom(initial, narrow_target, 0.5)
            .unwrap();
        assert!(navigation.apply_wheel_zoom_transition(displayed).unwrap());

        let target = navigation
            .wheel_zoom_target(narrow_target, -f32::MAX)
            .unwrap()
            .unwrap();
        assert_eq!(target.projection(), initial.projection());
        assert_eq!(target.pose(), initial.pose());
        assert_eq!(target.orbit_target(), initial.orbit_target());
        assert_eq!(target.orbit_distance(), initial.orbit_distance());

        let rebase_base = navigation.state();
        assert_eq!(rebase_base, displayed);
        let endpoint = navigation
            .interpolate_wheel_zoom(rebase_base, target, 1.0)
            .unwrap();
        assert_eq!(endpoint, initial);
        assert!(
            navigation
                .wheel_zoom_target(target, -f32::MAX)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn showcase_named_figure_selector_values_resolve_without_positional_identity() {
        for (id, _) in ShowcaseFigure::options() {
            let figure = ShowcaseFigure::from_selection(Some(id.0)).unwrap();
            assert_eq!(figure.radio_id(), id);
        }
        assert!(ShowcaseFigure::from_selection(Some(0)).is_none());
    }
}
