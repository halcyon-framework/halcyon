//! SPEC-HAL-357 domain-neutral visible proof.
//!
//! A Planar2 Cartesian grid with mechanical X/Y axes and a Spatial3 cube share
//! one Viewport3D depth world. The camera and cube remain fixed while only the
//! plane domain's embedding moves and rotates.

use std::f64::consts::TAU;
use std::sync::Arc;

use halcyon::core::app::{App, AppConfig, Screen, ScreenFrameContext};
use halcyon::core::layout::{Constraints, EdgeInsets, FlexParams, LayoutResult};
use halcyon::core::math::RectF;
use halcyon::core::paint::Color;
use halcyon::core::widget::{LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::*;
use halcyon::renderer::wgpu_renderer::GpuResources;
use halcyon::renderer::{
    CustomRenderAdmission, CustomRenderParticipant, CustomRenderPreparationOutcome,
    CustomRenderTargetMetrics,
};
use halcyon::three_d::{
    AppearanceSourceDescriptor, CameraState3, Context3D, DomainComposition3,
    DomainComposition3Descriptor, DomainContent3Descriptor, DomainContentId3,
    DomainContentPlacement3, DomainEmbedding3, DomainFrame3Descriptor, DomainState3,
    DomainState3Descriptor, DomainStateRevision3, DrawPacketDescriptor, GeometryIndices,
    GeometrySemantics, GeometrySourceDescriptor, LinearVertexColorRgba8, LocalDomain3Descriptor,
    LocalDomainId3, LocalDomainKind3, LocalPoint3, LocalVector3, ModelRevision, Orientation3,
    PerspectiveProjection, RasterMode, SampleCount3D, StructuralRevision, TextureCoordinate2,
    ViewDescriptor3, Viewport3D, Viewport3DViewRevision, WorldPoint3,
};
use halcyon::utils::id::WidgetInternalId;

const PLANAR_DOMAIN: LocalDomainId3 = LocalDomainId3::new(1);
const SPATIAL_DOMAIN: LocalDomainId3 = LocalDomainId3::new(2);
const PLANAR_CONTENT: DomainContentId3 = DomainContentId3::new(11);
const SPATIAL_CONTENT: DomainContentId3 = DomainContentId3::new(22);

const PLANAR_APPEARANCE: &str = r#"
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
    let side = select(0.42, 0.78, front_facing);
    return vertex_color_linear.rgb * color_factor_linear.rgb * side;
}
"#;

const SPATIAL_APPEARANCE: &str = r#"
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
    let key_light = normalize(vec3<f32>(-0.42, 0.68, 0.58));
    let view = normalize(view_direction_world);
    let diffuse = 0.055 + 0.28 * max(dot(normal, key_light), 0.0);
    let rim = pow(1.0 - max(dot(normal, view), 0.0), 3.0);
    let edge_distance = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let edge_core = 1.0 - smoothstep(0.0, 0.010, edge_distance);
    let edge_halo = 1.0 - smoothstep(0.010, 0.045, edge_distance);
    let cool_rim = vec3<f32>(0.004, 0.026, 0.045) * rim;
    let cool_edge = vec3<f32>(0.012, 0.18, 0.28) * (edge_core * 0.82 + edge_halo * 0.08);
    return vertex_color_linear.rgb * color_factor_linear.rgb * diffuse + cool_rim + cool_edge;
}
"#;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(AppConfig::new("Shared 3D Spatial Domains").with_size(1100.0, 760.0));
    app.run(SharedSpatialDomainsScreen::new());
}

struct SharedSpatialDomainsDemo {
    id: WidgetInternalId,
    viewport: Viewport3D,
    context: Arc<Context3D>,
    composition: DomainComposition3,
    fixed_spatial_state: DomainState3,
    view: ViewDescriptor3,
    planar_revision: DomainStateRevision3,
    model_revision: ModelRevision,
    phase: f64,
    running: bool,
}

impl SharedSpatialDomainsDemo {
    fn new(gpu: GpuResources) -> Result<Self, String> {
        let context = Arc::new(Context3D::new(gpu).map_err(|error| error.to_string())?);
        let composition = build_composition(&context)?;
        let planar_revision = DomainStateRevision3::new(1);
        let model_revision = ModelRevision::new(1);
        let planar_state = admit_state(
            &context,
            &composition,
            PLANAR_DOMAIN,
            PLANAR_CONTENT,
            planar_revision,
            moving_planar_embedding(0.0)?,
        )?;
        let fixed_spatial_state = admit_state(
            &context,
            &composition,
            SPATIAL_DOMAIN,
            SPATIAL_CONTENT,
            DomainStateRevision3::new(1),
            fixed_spatial_embedding()?,
        )?;
        let frame = context
            .admit_domain_frame(
                &composition,
                DomainFrame3Descriptor::new(
                    model_revision,
                    vec![planar_state, fixed_spatial_state.clone()],
                ),
            )
            .map_err(|error| error.to_string())?;
        let view = fixed_view()?;
        let viewport = Viewport3D::new_domain_composition(
            context.clone(),
            Viewport3DViewRevision::new(1),
            view,
            frame,
            SampleCount3D::FOUR,
        )
        .map_err(|error| error.to_string())?;

        Ok(Self {
            id: WidgetInternalId::new(),
            viewport,
            context,
            composition,
            fixed_spatial_state,
            view,
            planar_revision,
            model_revision,
            phase: 0.0,
            running: true,
        })
    }

    fn advance(&mut self, dt: f32) -> Result<bool, String> {
        if !dt.is_finite() || dt <= 0.0 {
            return Ok(false);
        }
        let next_phase = (self.phase + f64::from(dt.min(0.05)) * 0.62).rem_euclid(TAU);
        let next_planar_revision = self
            .planar_revision
            .checked_next()
            .map_err(|error| error.to_string())?;
        let next_model_revision = self
            .model_revision
            .checked_next()
            .map_err(|error| error.to_string())?;
        let moving_planar_state = admit_state(
            &self.context,
            &self.composition,
            PLANAR_DOMAIN,
            PLANAR_CONTENT,
            next_planar_revision,
            moving_planar_embedding(next_phase)?,
        )?;
        let frame = self
            .context
            .admit_domain_frame(
                &self.composition,
                DomainFrame3Descriptor::new(
                    next_model_revision,
                    vec![moving_planar_state, self.fixed_spatial_state.clone()],
                ),
            )
            .map_err(|error| error.to_string())?;
        self.viewport
            .set_domain_composition_request(
                Viewport3DViewRevision::new(1),
                self.view,
                frame,
                SampleCount3D::FOUR,
            )
            .map_err(|error| error.to_string())?;
        self.phase = next_phase;
        self.planar_revision = next_planar_revision;
        self.model_revision = next_model_revision;
        Ok(true)
    }
}

impl WidgetDowncast for SharedSpatialDomainsDemo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl CustomRenderParticipant for SharedSpatialDomainsDemo {
    fn prepare_custom_render(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
    ) -> CustomRenderPreparationOutcome {
        CustomRenderParticipant::prepare_custom_render(&mut self.viewport, metrics)
    }
}

impl Widget for SharedSpatialDomainsDemo {
    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        if !self.running {
            return;
        }
        match self.advance(dt) {
            Ok(true) => cx.invalidate_paint(),
            Ok(false) => {}
            Err(error) => {
                tracing::error!(%error, "shared spatial-domain animation stopped");
                self.running = false;
            }
        }
    }

    fn needs_update(&self) -> bool {
        self.running
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
        cx.scene
            .push_rect(rect, Color::from_hex(0x070B10), 1, cx.scene_key);
        self.viewport.paint(cx, rect, children);
    }

    fn custom_render_admission(&self) -> Option<&CustomRenderAdmission> {
        self.viewport.custom_render_admission()
    }

    fn custom_render_participant_mut(&mut self) -> Option<&mut dyn CustomRenderParticipant> {
        Some(self)
    }

    fn on_unmount(&mut self) {
        self.running = false;
        self.viewport.on_unmount();
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "SharedSpatialDomainsDemo"
    }
}

struct SharedSpatialDomainsScreen {
    needs_rebuild: bool,
    built: bool,
    placeholder: Label,
}

impl SharedSpatialDomainsScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            built: false,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for SharedSpatialDomainsScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;

        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_spacing(8.0)
                .with_padding(EdgeInsets::all(16.0))
                .with_background(Color::from_hex(0x070B10)),
        ));
        cx.tree.add_child(
            root,
            Box::new(
                Label::new("Shared 3D spatial domains")
                    .with_size(22.0)
                    .with_color(Color::from_hex(0xD7E2EC)),
            ),
            None,
            0,
        );
        cx.tree.add_child(
            root,
            Box::new(
                Label::new(
                    "Amber Cartesian X/Y carrier: Planar2. Cool blue volume: an independent Spatial3 domain.",
                )
                .with_size(12.0)
                .with_color(Color::from_hex(0x8FA3B5)),
            ),
            None,
            1,
        );

        let demo = cx
            .app
            .gpu_resources()
            .cloned()
            .ok_or_else(|| "GpuResources not available".to_string())
            .and_then(SharedSpatialDomainsDemo::new);
        let demo_node = match demo {
            Ok(demo) => cx.tree.add_child(root, Box::new(demo), None, 2),
            Err(error) => cx.tree.add_child(
                root,
                Box::new(Label::new(format!(
                    "Shared spatial-domain initialization failed: {error}"
                ))),
                None,
                2,
            ),
        };
        cx.tree.set_flex(demo_node, FlexParams::flexible(1.0));

        cx.tree.add_child(
            root,
            Box::new(
                Label::new(
                    "Only the amber Planar2 embedding moves through the fixed cube. Shared occlusion proves one depth field; the camera stays fixed.",
                )
                .with_size(11.0)
                .with_color(Color::from_hex(0x788C9D)),
            ),
            None,
            3,
        );
        &mut self.placeholder
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}

fn build_composition(context: &Context3D) -> Result<DomainComposition3, String> {
    let planar_geometry = context
        .register_geometry(planar_carrier_geometry())
        .map_err(|error| error.to_string())?;
    let spatial_geometry = context
        .register_geometry(cube_geometry())
        .map_err(|error| error.to_string())?;
    let planar_appearance = context
        .register_appearance(
            AppearanceSourceDescriptor::try_new(
                PLANAR_APPEARANCE,
                GeometrySemantics::new(false, false, true),
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    let spatial_appearance = context
        .register_appearance(
            AppearanceSourceDescriptor::try_new(
                SPATIAL_APPEARANCE,
                GeometrySemantics::new(true, true, true),
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

    context
        .admit_domain_composition(DomainComposition3Descriptor::new(
            StructuralRevision::new(1),
            vec![
                LocalDomain3Descriptor::new(
                    PLANAR_DOMAIN,
                    LocalDomainKind3::Planar2,
                    vec![DomainContent3Descriptor::new(
                        PLANAR_CONTENT,
                        vec![DrawPacketDescriptor::new(
                            planar_geometry.handle().clone(),
                            planar_appearance.handle().clone(),
                            RasterMode::DoubleSided,
                            10,
                        )],
                    )],
                ),
                LocalDomain3Descriptor::new(
                    SPATIAL_DOMAIN,
                    LocalDomainKind3::Spatial3,
                    vec![DomainContent3Descriptor::new(
                        SPATIAL_CONTENT,
                        vec![DrawPacketDescriptor::new(
                            spatial_geometry.handle().clone(),
                            spatial_appearance.handle().clone(),
                            RasterMode::SingleSided,
                            20,
                        )],
                    )],
                ),
            ],
        ))
        .map_err(|error| error.to_string())
}

fn admit_state(
    context: &Context3D,
    composition: &DomainComposition3,
    domain_id: LocalDomainId3,
    content_id: DomainContentId3,
    revision: DomainStateRevision3,
    embedding: DomainEmbedding3,
) -> Result<DomainState3, String> {
    context
        .admit_domain_state(
            composition,
            DomainState3Descriptor::new(
                domain_id,
                revision,
                embedding,
                vec![(content_id, DomainContentPlacement3::identity())],
            ),
        )
        .map_err(|error| error.to_string())
}

fn moving_planar_embedding(phase: f64) -> Result<DomainEmbedding3, String> {
    let origin = WorldPoint3::try_new(
        -0.18 + phase.cos() * 0.20,
        (phase * 2.0).sin() * 0.08,
        -3.25 + phase.sin() * 0.92,
    )
    .map_err(|error| error.to_string())?;
    let yaw = orientation_y((phase.sin() * 0.72) as f32)?;
    let pitch = orientation_x((0.18 + (phase * 2.0).cos() * 0.10) as f32)?;
    let roll = orientation_z(((phase * 2.0).sin() * 0.22) as f32)?;
    let orientation = yaw
        .compose_local(pitch)
        .and_then(|value| value.compose_local(roll))
        .map_err(|error| error.to_string())?;
    DomainEmbedding3::try_new(origin, orientation, 1.24).map_err(|error| error.to_string())
}

fn fixed_spatial_embedding() -> Result<DomainEmbedding3, String> {
    let yaw = orientation_y(-0.52)?;
    let pitch = orientation_x(0.34)?;
    let orientation = yaw
        .compose_local(pitch)
        .map_err(|error| error.to_string())?;
    DomainEmbedding3::try_new(
        WorldPoint3::try_new(0.95, 0.0, -3.25).map_err(|error| error.to_string())?,
        orientation,
        1.12,
    )
    .map_err(|error| error.to_string())
}

fn fixed_view() -> Result<ViewDescriptor3, String> {
    CameraState3::try_orbit(
        WorldPoint3::try_new(0.0, 0.0, -3.25).map_err(|error| error.to_string())?,
        0.0,
        -0.10,
        4.65,
        PerspectiveProjection::try_finite(45.0_f32.to_radians(), 0.1, 100.0)
            .map_err(|error| error.to_string())?
            .into(),
    )
    .map(CameraState3::view_descriptor)
    .map_err(|error| error.to_string())
}

fn orientation_x(angle: f32) -> Result<Orientation3, String> {
    let half = angle * 0.5;
    Orientation3::try_from_xyzw(half.sin(), 0.0, 0.0, half.cos()).map_err(|error| error.to_string())
}

fn orientation_y(angle: f32) -> Result<Orientation3, String> {
    let half = angle * 0.5;
    Orientation3::try_from_xyzw(0.0, half.sin(), 0.0, half.cos()).map_err(|error| error.to_string())
}

fn orientation_z(angle: f32) -> Result<Orientation3, String> {
    let half = angle * 0.5;
    Orientation3::try_from_xyzw(0.0, 0.0, half.sin(), half.cos()).map_err(|error| error.to_string())
}

fn planar_carrier_geometry() -> GeometrySourceDescriptor {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let minor_grid = color(18, 10, 2);
    let major_grid = color(34, 19, 4);
    let axis_x = color(52, 29, 5);
    let axis_y = color(60, 35, 7);
    let edge_halo = color(34, 16, 2);
    let edge_core = color(142, 73, 12);

    for x in [-1.25, -0.75, -0.25, 0.25, 0.75, 1.25] {
        push_planar_rect(
            &mut positions,
            &mut colors,
            [x - 0.0012, -1.12],
            [x + 0.0012, 1.12],
            0.0,
            minor_grid,
        );
    }
    for y in [-0.75, -0.25, 0.25, 0.75] {
        push_planar_rect(
            &mut positions,
            &mut colors,
            [-1.62, y - 0.0012],
            [1.62, y + 0.0012],
            0.0,
            minor_grid,
        );
    }

    for x in [-1.50, -1.00, -0.50, 0.50, 1.00, 1.50] {
        push_planar_rect(
            &mut positions,
            &mut colors,
            [x - 0.0018, -1.12],
            [x + 0.0018, 1.12],
            0.001,
            major_grid,
        );
    }
    for y in [-1.00, -0.50, 0.50, 1.00] {
        push_planar_rect(
            &mut positions,
            &mut colors,
            [-1.62, y - 0.0018],
            [1.62, y + 0.0018],
            0.001,
            major_grid,
        );
    }

    push_planar_rect(
        &mut positions,
        &mut colors,
        [-1.72, -0.003],
        [1.72, 0.003],
        0.002,
        axis_x,
    );
    push_planar_rect(
        &mut positions,
        &mut colors,
        [-0.003, -1.22],
        [0.003, 1.22],
        0.002,
        axis_y,
    );

    for (half_width, z, fill) in [(0.007, 0.001, edge_halo), (0.0025, 0.004, edge_core)] {
        push_planar_rect(
            &mut positions,
            &mut colors,
            [-1.72, -1.22 - half_width],
            [1.72, -1.22 + half_width],
            z,
            fill,
        );
        push_planar_rect(
            &mut positions,
            &mut colors,
            [-1.72, 1.22 - half_width],
            [1.72, 1.22 + half_width],
            z,
            fill,
        );
        push_planar_rect(
            &mut positions,
            &mut colors,
            [-1.72 - half_width, -1.22],
            [-1.72 + half_width, 1.22],
            z,
            fill,
        );
        push_planar_rect(
            &mut positions,
            &mut colors,
            [1.72 - half_width, -1.22],
            [1.72 + half_width, 1.22],
            z,
            fill,
        );
    }

    push_local_basis_cue(&mut positions, &mut colors);

    GeometrySourceDescriptor::try_new(
        positions,
        None,
        None,
        Some(colors),
        GeometryIndices::Unindexed,
    )
    .expect("valid Planar2 carrier geometry")
}

fn push_local_basis_cue(
    positions: &mut Vec<LocalPoint3>,
    colors: &mut Vec<LinearVertexColorRgba8>,
) {
    let x_color = color(156, 28, 20);
    let y_color = color(30, 164, 48);
    let z_color = color(24, 88, 178);
    let origin_color = color(128, 74, 18);

    push_planar_arrow(
        positions,
        colors,
        [0.035, 0.0],
        [0.60, 0.0],
        0.012,
        0.008,
        0.12,
        0.045,
        x_color,
    );
    push_planar_arrow(
        positions,
        colors,
        [0.0, 0.035],
        [0.0, 0.60],
        0.013,
        0.008,
        0.12,
        0.045,
        y_color,
    );

    let shaft_half_width = 0.008;
    push_quad(
        positions,
        colors,
        [
            point(-shaft_half_width, 0.0, 0.035),
            point(shaft_half_width, 0.0, 0.035),
            point(shaft_half_width, 0.0, 0.50),
            point(-shaft_half_width, 0.0, 0.50),
        ],
        z_color,
    );
    push_quad(
        positions,
        colors,
        [
            point(0.0, -shaft_half_width, 0.035),
            point(0.0, shaft_half_width, 0.035),
            point(0.0, shaft_half_width, 0.50),
            point(0.0, -shaft_half_width, 0.50),
        ],
        z_color,
    );
    for base in [
        [point(-0.032, -0.032, 0.48), point(0.032, -0.032, 0.48)],
        [point(0.032, -0.032, 0.48), point(0.032, 0.032, 0.48)],
        [point(0.032, 0.032, 0.48), point(-0.032, 0.032, 0.48)],
        [point(-0.032, 0.032, 0.48), point(-0.032, -0.032, 0.48)],
    ] {
        push_triangle(
            positions,
            colors,
            [base[0], base[1], point(0.0, 0.0, 0.62)],
            z_color,
        );
    }
    push_box(
        positions,
        colors,
        [-0.035, -0.035, -0.035],
        [0.035, 0.035, 0.035],
        origin_color,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_planar_arrow(
    positions: &mut Vec<LocalPoint3>,
    colors: &mut Vec<LinearVertexColorRgba8>,
    tail: [f32; 2],
    tip: [f32; 2],
    z: f32,
    shaft_half_width: f32,
    head_length: f32,
    head_half_width: f32,
    fill: LinearVertexColorRgba8,
) {
    let delta = [tip[0] - tail[0], tip[1] - tail[1]];
    let length = delta[0].hypot(delta[1]);
    assert!(
        length.is_finite()
            && length > head_length
            && shaft_half_width.is_finite()
            && shaft_half_width > 0.0
            && head_length.is_finite()
            && head_length > 0.0
            && head_half_width.is_finite()
            && head_half_width > shaft_half_width,
        "valid planar arrow dimensions"
    );

    let direction = [delta[0] / length, delta[1] / length];
    let normal = [-direction[1], direction[0]];
    let neck = [
        tip[0] - direction[0] * head_length,
        tip[1] - direction[1] * head_length,
    ];
    let offset_point = |center: [f32; 2], offset: f32| {
        point(
            center[0] + normal[0] * offset,
            center[1] + normal[1] * offset,
            z,
        )
    };

    let tail_left = offset_point(tail, shaft_half_width);
    let tail_right = offset_point(tail, -shaft_half_width);
    let neck_left = offset_point(neck, shaft_half_width);
    let neck_right = offset_point(neck, -shaft_half_width);
    let base_left = offset_point(neck, head_half_width);
    let base_right = offset_point(neck, -head_half_width);
    let tip = point(tip[0], tip[1], z);

    for triangle in [
        [tail_right, neck_right, neck_left],
        [tail_right, neck_left, tail_left],
        [base_right, tip, neck_right],
        [neck_right, tip, neck_left],
        [neck_left, tip, base_left],
    ] {
        push_triangle(positions, colors, triangle, fill);
    }
}

fn push_quad(
    positions: &mut Vec<LocalPoint3>,
    colors: &mut Vec<LinearVertexColorRgba8>,
    corners: [LocalPoint3; 4],
    fill: LinearVertexColorRgba8,
) {
    positions.extend([
        corners[0], corners[1], corners[2], corners[0], corners[2], corners[3],
    ]);
    colors.extend([fill; 6]);
}

fn push_triangle(
    positions: &mut Vec<LocalPoint3>,
    colors: &mut Vec<LinearVertexColorRgba8>,
    vertices: [LocalPoint3; 3],
    fill: LinearVertexColorRgba8,
) {
    positions.extend(vertices);
    colors.extend([fill; 3]);
}

fn push_box(
    positions: &mut Vec<LocalPoint3>,
    colors: &mut Vec<LinearVertexColorRgba8>,
    min: [f32; 3],
    max: [f32; 3],
    fill: LinearVertexColorRgba8,
) {
    let corners = [
        point(min[0], min[1], min[2]),
        point(max[0], min[1], min[2]),
        point(max[0], max[1], min[2]),
        point(min[0], max[1], min[2]),
        point(min[0], min[1], max[2]),
        point(max[0], min[1], max[2]),
        point(max[0], max[1], max[2]),
        point(min[0], max[1], max[2]),
    ];
    for face in [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ] {
        push_quad(
            positions,
            colors,
            [
                corners[face[0]],
                corners[face[1]],
                corners[face[2]],
                corners[face[3]],
            ],
            fill,
        );
    }
}

fn push_planar_rect(
    positions: &mut Vec<LocalPoint3>,
    colors: &mut Vec<LinearVertexColorRgba8>,
    min: [f32; 2],
    max: [f32; 2],
    z: f32,
    fill: LinearVertexColorRgba8,
) {
    positions.extend([
        point(min[0], min[1], z),
        point(max[0], min[1], z),
        point(max[0], max[1], z),
        point(min[0], min[1], z),
        point(max[0], max[1], z),
        point(min[0], max[1], z),
    ]);
    colors.extend([fill; 6]);
}

fn cube_geometry() -> GeometrySourceDescriptor {
    let positions = vec![
        point(-0.5, -0.5, 0.5),
        point(0.5, -0.5, 0.5),
        point(0.5, 0.5, 0.5),
        point(-0.5, 0.5, 0.5),
        point(0.5, -0.5, -0.5),
        point(-0.5, -0.5, -0.5),
        point(-0.5, 0.5, -0.5),
        point(0.5, 0.5, -0.5),
        point(0.5, -0.5, 0.5),
        point(0.5, -0.5, -0.5),
        point(0.5, 0.5, -0.5),
        point(0.5, 0.5, 0.5),
        point(-0.5, -0.5, -0.5),
        point(-0.5, -0.5, 0.5),
        point(-0.5, 0.5, 0.5),
        point(-0.5, 0.5, -0.5),
        point(-0.5, 0.5, 0.5),
        point(0.5, 0.5, 0.5),
        point(0.5, 0.5, -0.5),
        point(-0.5, 0.5, -0.5),
        point(-0.5, -0.5, -0.5),
        point(0.5, -0.5, -0.5),
        point(0.5, -0.5, 0.5),
        point(-0.5, -0.5, 0.5),
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
        color(9, 27, 42),
        color(3, 9, 17),
        color(7, 30, 39),
        color(4, 15, 23),
        color(10, 36, 51),
        color(4, 13, 24),
    ] {
        colors.extend([face_color; 4]);
    }
    let mut uvs = Vec::with_capacity(24);
    for _ in 0..6 {
        uvs.extend([
            texture_coordinate(0.0, 0.0),
            texture_coordinate(1.0, 0.0),
            texture_coordinate(1.0, 1.0),
            texture_coordinate(0.0, 1.0),
        ]);
    }
    GeometrySourceDescriptor::try_new(
        positions,
        Some(normals),
        Some(uvs),
        Some(colors),
        GeometryIndices::from_u16(vec![
            0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15, 16,
            17, 18, 16, 18, 19, 20, 21, 22, 20, 22, 23,
        ]),
    )
    .expect("valid Spatial3 cube geometry")
}

fn point(x: f32, y: f32, z: f32) -> LocalPoint3 {
    LocalPoint3::try_new(x, y, z).expect("finite example point")
}

fn vector(x: f32, y: f32, z: f32) -> LocalVector3 {
    LocalVector3::try_new(x, y, z).expect("finite example vector")
}

fn texture_coordinate(u: f32, v: f32) -> TextureCoordinate2 {
    TextureCoordinate2::try_new(u, v).expect("finite example texture coordinate")
}

fn color(red: u8, green: u8, blue: u8) -> LinearVertexColorRgba8 {
    LinearVertexColorRgba8::from_linear_unorm8(red, green, blue, 255)
}
