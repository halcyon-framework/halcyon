//! Real HAL374 visible-only selection on three certified PBR occurrences.
//! cargo run --release --example pbr_selection_outline
//! 1 / 2 / 0 select A / B / None; Space cycles. Arrows orbit/zoom.
//! Escape remains owned by the engine. There is no timer or animation loop.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::layout::{EdgeInsets, FlexParams};
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::prelude::{Flex, Label};
use halcyon::three_d::*;
use halcyon::utils::id::TreeWidgetId;
use std::sync::Arc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    App::new(AppConfig::new("Halcyon PBR Selection Outline").with_size(1060., 800.))
        .run(OutlineScreen::with_samples(SampleCount3D::FOUR));
}

fn camera(yaw: f32, distance: f64) -> ViewDescriptor3 {
    ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(
                f64::from(yaw.sin()) * distance,
                0.,
                f64::from(yaw.cos()) * distance,
            )
            .expect("finite orbit"),
            Orientation3::try_from_xyzw(0., (yaw * 0.5).sin(), 0., (yaw * 0.5).cos())
                .expect("finite camera orientation"),
        ),
        PerspectiveProjection::try_finite(45.0f32.to_radians(), 0.1, 30.)
            .expect("finite camera projection")
            .into(),
    )
}

fn source(context: &Context3D, samples: SampleCount3D) -> PbrSourceResult3D<PbrScene3D> {
    let id = PbrOriginalIdentity3D {
        source: 374001,
        revision: 1,
        node: 0,
        mesh: 0,
        primitive: 0,
    };
    let mut original = Vec::new();
    let mut render = Vec::new();
    let mut triangles = Vec::new();
    // Six actual faces, exact original/render corner mapping, outward normals.
    // One admitted mesh is intentionally shared at THREE different placements.
    for axis in 0..3 {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        for sign in [-1., 1.] {
            let mut normal = [0.; 3];
            normal[axis] = sign;
            let mut tangent = [0., 0., 0., 1.];
            tangent[u] = 1.;
            let mut corners = [[0.; 3]; 4];
            for (corner, (x, y)) in
                corners
                    .iter_mut()
                    .zip([(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)])
            {
                corner[axis] = sign * 0.5;
                corner[u] = x;
                corner[v] = y;
            }
            for i in if sign > 0. {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            } {
                original.push(PbrOriginalVertex3D {
                    position: corners[i],
                    normal,
                    uv0: None,
                    color: None,
                });
                render.push(PbrVertex3D {
                    position: corners[i],
                    normal,
                    uv0: [0.; 2],
                    color: [255; 4],
                    tangent,
                });
            }
            for _ in 0..2 {
                let triangle = triangles.len();
                triangles.push(PbrTriangleCertificate3D {
                    original: id,
                    triangle,
                    certificate_identity: 374010 + triangle as u64,
                    certificate_revision: 1,
                    frame: PbrFrameCertificate3D::NormalOnly {
                        separator: normal.map(f64::from),
                        lower: [0.5; 3],
                    },
                });
            }
        }
    }
    let mapping = (0..render.len() as u32).collect();
    let geometry = context.admit_pbr_geometry(
        PbrGeometryDescriptor3D {
            original: PbrOriginalGeometry3D {
                identity: id,
                vertices: original.into(),
                indices: GeometryIndices::Unindexed,
                signed_tangents: Arc::from([]),
                tangent_provenance: PbrTangentProvenance3D::Authored,
            },
            render: render.into(),
            render_indices: GeometryIndices::Unindexed,
            render_vertex_sources: mapping,
            triangles: triangles.into(),
            mapped: false,
            maximum_certificate_bytes: 65536,
            maximum_verification_items: 10000,
        },
        None,
    )?;
    let mut packets = Vec::new();
    for (slot, color) in [
        [0.88, 0.27, 0.055, 1.],
        [0.045, 0.34, 0.88, 1.],
        [0.3, 0.32, 0.35, 1.],
    ]
    .into_iter()
    .enumerate()
    {
        let mut material =
            PbrMaterialDescriptor3D::new(374100 + slot as u64, 1, PbrSamplingFamily3D::V1);
        material.base_rgba = color;
        material.metallic = 0.15;
        material.roughness = 0.65;
        let material = context.admit_pbr_material(material, None)?;
        packets.push(PbrDrawPacket3D {
            geometry: geometry.clone(),
            material,
            raster: RasterMode::SingleSided,
            stable_ordinal: 900 - slot as u64,
        });
    }
    let snapshot = context.admit_pbr_snapshot(StructuralRevision::new(1), packets)?;
    let rotation = Orientation3::try_from_xyzw(0.19f32.sin(), 0., 0., 0.19f32.cos())
        .expect("finite pitch")
        .compose_local(
            Orientation3::try_from_xyzw(0., (-0.27f32).sin(), 0., (-0.27f32).cos())
                .expect("finite yaw"),
        )
        .expect("finite object orientation");
    let placement = |p: [f64; 3], s: [f32; 3]| {
        ModelPlacement3::try_new(
            WorldPoint3::try_new(p[0], p[1], p[2]).expect("finite placement"),
            rotation,
            PositiveScale3::try_new(s[0], s[1], s[2]).expect("positive scale"),
        )
        .expect("finite model")
    };
    let model = context.admit_pbr_model(
        &snapshot,
        ModelRevision::new(1),
        vec![
            placement([-0.7, 0., 0.], [1.1; 3]),
            placement([0.7, 0., 0.], [1.; 3]),
            placement([0., 0., 2.], [0.35, 1.6, 0.2]),
        ],
    )?;
    let inv = 14.0f64.sqrt().recip();
    let direct = PbrDirectSource3D::capture(
        PbrDirectDescriptor3D {
            identity: 374200,
            revision: 1,
            meters_per_scene_unit: 1.,
            directional: Some(PbrDirectionalLight3D {
                surface_to_source: [-1., 2., 3.].map(|v| v * inv),
                irradiance: [3., 2.8, 2.5],
                shadow_far_m: 24.,
            }),
            points: Arc::from([PbrPointLight3D {
                position: WorldPoint3::try_new(3., 1., 4.).expect("finite light"),
                intensity: [6., 8., 10.],
                radius_m: 0.5,
                range_m: 24.,
            }]),
            spots: Arc::from([]),
        },
        &snapshot,
        &model,
    )?;
    // Source r=.65 gives response<3.21; incident<=43, total RGB<139<256.
    context.admit_pbr_scene(PbrSceneDescriptor3D {
        snapshot,
        model,
        hdr: PbrHdrSource3D {
            identity: 374300,
            revision: 1,
            exposure_ev: 0.,
            maximum_rgb: [256.; 3],
        },
        samples,
        direct: Some(direct),
        indirect: None,
    })
}

pub(crate) struct OutlineScreen {
    placeholder: Label,
    built: bool,
    samples: SampleCount3D,
    pub(crate) viewport: Option<TreeWidgetId>,
    status: Option<TreeWidgetId>,
    source: Option<PbrScene3D>,
    groups: Option<[PbrOutlineSelection3D; 2]>,
    pub(crate) desired: usize,
    active: usize,
    yaw: f32,
    distance: f64,
    view_revision: u64,
    view_dirty: bool,
    last_status: String,
}
impl OutlineScreen {
    pub(crate) fn with_samples(samples: SampleCount3D) -> Self {
        Self {
            placeholder: Label::new(""),
            built: false,
            samples,
            viewport: None,
            status: None,
            source: None,
            groups: None,
            desired: 0,
            active: 0,
            yaw: 0.,
            distance: 5.,
            view_revision: 1,
            view_dirty: false,
            last_status: String::new(),
        }
    }
    fn initialize(&mut self, cx: &mut ScreenFrameContext<'_>) -> Result<Viewport3D, String> {
        let gpu = cx
            .app
            .gpu_resources()
            .cloned()
            .ok_or("Native GPU owner unavailable")?;
        let context = Arc::new(Context3D::new(gpu).map_err(|e| format!("Context: {e:?}"))?);
        let scene = source(&context, self.samples).map_err(|e| format!("Source: {e:?}"))?;
        let groups = [
            context.admit_pbr_outline_selection(&scene, &[0]),
            context.admit_pbr_outline_selection(&scene, &[1]),
        ];
        let [a, b] = groups;
        let groups = [
            a.map_err(|e| format!("A: {e:?}"))?,
            b.map_err(|e| format!("B: {e:?}"))?,
        ];
        let mut viewport = Viewport3D::new_pbr(
            context,
            scene.clone(),
            Viewport3DViewRevision::new(1),
            camera(self.yaw, self.distance),
        )
        .map_err(|e| format!("Viewport: {e:?}"))?;
        viewport
            .request_pbr_outline(Some(groups[0].clone()))
            .map_err(|e| format!("Outline: {e:?}"))?;
        self.source = Some(scene);
        self.groups = Some(groups);
        Ok(viewport)
    }
}
impl Screen for OutlineScreen {
    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;
        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_spacing(10.)
                .with_padding(EdgeInsets::all(20.))
                .with_background(Color::from_hex(0x10151D)),
        ));
        cx.tree.add_child(
            root,
            Box::new(
                Label::new("PBR Selection Outline")
                    .with_size(24.)
                    .with_color(Color::from_hex(0xECF0F5)),
            ),
            None,
            0,
        );
        cx.tree.add_child(root,Box::new(Label::new("1: amber A    2: cobalt B    0: none    Space: cycle    Left/Right: orbit    Up/Down: zoom    Esc: exit")
            .with_size(13.).with_color(Color::from_hex(0xB2C1D3))),None,1);
        let node = match self.initialize(cx) {
            Ok(viewport) => {
                let n = cx.tree.add_child(root, Box::new(viewport), None, 2);
                self.viewport = Some(cx.tree.node(n).expect("inserted viewport").id());
                n
            }
            Err(e) => {
                tracing::error!(%e);
                cx.tree.add_child(
                    root,
                    Box::new(Label::new(e).with_color(Color::from_hex(0xFFAFA0))),
                    None,
                    2,
                )
            }
        };
        cx.tree.set_flex(node, FlexParams::flexible(1.));
        let n = cx.tree.add_child(
            root,
            Box::new(
                Label::new("Preparing original PBR and selected outline")
                    .with_color(Color::from_hex(0xD6DFEA)),
            ),
            None,
            3,
        );
        self.status = Some(cx.tree.node(n).expect("inserted status").id());
        cx.tree.add_child(root,Box::new(Label::new("2 physical pixels · actual triangle edges · grey foreground occludes selection · static, no pulse")
            .with_size(12.).with_color(Color::from_hex(0x91A2B8))),None,4);
        &mut self.placeholder
    }
    fn handle_input(&mut self, event: &FusedInputEvent, _: &mut ScreenContext<'_>) -> bool {
        let FusedInputEvent::KeyDown { key_code, .. } = event else {
            return false;
        };
        match key_code {
            KeyCode::Character('1') => self.desired = 0,
            KeyCode::Character('2') => self.desired = 1,
            KeyCode::Character('0') => self.desired = 2,
            KeyCode::Space => self.desired = (self.desired + 1) % 3,
            KeyCode::Left => {
                self.yaw -= 0.08;
                self.view_dirty = true;
            }
            KeyCode::Right => {
                self.yaw += 0.08;
                self.view_dirty = true;
            }
            KeyCode::Up => {
                self.distance = (self.distance - 0.2).max(3.5);
                self.view_dirty = true;
            }
            KeyCode::Down => {
                self.distance = (self.distance + 0.2).min(9.);
                self.view_dirty = true;
            }
            _ => return false,
        }
        true
    }
    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let Some(tree) = cx.tree.as_deref_mut() else {
            return;
        };
        let Some(node) = self.viewport.and_then(|id| tree.find_by_id(id)) else {
            return;
        };
        let Some(viewport) = tree
            .node_mut(node)
            .and_then(|n| n.widget_mut().as_any_mut().downcast_mut::<Viewport3D>())
        else {
            return;
        };
        let result = (|| -> Result<(), String> {
            if self.view_dirty {
                let revision = self
                    .view_revision
                    .checked_add(1)
                    .ok_or("View revision exhausted")?;
                viewport
                    .request_pbr_scene(
                        self.source.as_ref().ok_or("Source retired")?.clone(),
                        Viewport3DViewRevision::new(revision),
                        camera(self.yaw, self.distance),
                    )
                    .map_err(|e| format!("View: {e:?}"))?;
                self.view_revision = revision;
                self.view_dirty = false;
            }
            if self.desired != self.active {
                let selection = if self.desired == 2 {
                    None
                } else {
                    Some(self.groups.as_ref().ok_or("Membership retired")?[self.desired].clone())
                };
                viewport
                    .request_pbr_outline(selection)
                    .map_err(|e| format!("Selection: {e:?}"))?;
                self.active = self.desired;
            }
            Ok(())
        })();
        let message = match result {
            Err(e) => e,
            Ok(()) => {
                let error = viewport.current_prepared_target().and_then(|t| {
                    viewport
                        .pbr_failure(t.surface_id())
                        .map(|e| format!("PBR refused: {e:?}"))
                        .or_else(|| {
                            viewport
                                .pbr_outline_error(t.surface_id())
                                .map(|e| format!("Outline refused; original PBR retained: {e:?}"))
                        })
                });
                error.unwrap_or_else(|| {
                    format!(
                        "{} · N{} · source-qualified occurrence · view revision {}",
                        ["A selected", "B selected", "None (original PBR)"][self.active],
                        self.samples.value(),
                        self.view_revision
                    )
                })
            }
        };
        if message != self.last_status {
            tracing::info!(%message);
            self.last_status = message.clone();
            if let Some(n) = self.status.and_then(|id| tree.find_by_id(id)) {
                if let Some(label) = tree
                    .node_mut(n)
                    .and_then(|n| n.widget_mut().as_any_mut().downcast_mut::<Label>())
                {
                    label.set_text(message);
                }
                tree.mark_layout_dirty(n);
            }
        }
    }
}
