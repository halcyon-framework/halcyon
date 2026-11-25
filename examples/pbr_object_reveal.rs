//! Native HAL372 whole-source PBR reveal. Run with:
//! cargo run --release --example pbr_object_reveal
//!
//! 1 / 2 select immutable sources; Space switches between them. Escape belongs
//! to the framework. The deliberately slow 900ms demo does not change product
//! timing. No image opacity, source mutation, private clock or test-only API.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext, UiWakeHandle};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::layout::{EdgeInsets, FlexParams};
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::prelude::{Flex, Label};
use halcyon::three_d::*;
use halcyon::utils::id::TreeWidgetId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const NAMES: [&str; 2] = ["1 · Amber cuboid", "2 · Cobalt steps"];
const DURATION: Duration = Duration::from_millis(900);
type Completion = Result<PbrObjectRevealExtentCpuResult3D, PbrObjectRevealExtentCpuError3D>;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    App::new(AppConfig::new("Halcyon PBR Object Reveal").with_size(1060.0, 800.0))
        .run(RevealScreen::new());
}

fn view() -> ViewDescriptor3 {
    ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(0.0, 0.0, 6.8).expect("finite camera"),
            Orientation3::identity(),
        ),
        PerspectiveProjection::try_finite(40.0f32.to_radians(), 0.1, 30.0)
            .expect("finite projection")
            .into(),
    )
}

// Each face has genuine original corners, outward normals, exact corner mapping
// and its own NormalOnly certificate. UVs and normal textures are absent. Boxes
// overlap in the stepped source: normal reverse-Z depth resolves their occlusion.
fn source(context: &Context3D, variant: usize) -> PbrSourceResult3D<PbrScene3D> {
    let identity = PbrOriginalIdentity3D {
        source: 372_100 + variant as u64,
        revision: 1,
        node: 0,
        mesh: 0,
        primitive: 0,
    };
    let boxes: &[([f32; 3], [f32; 3])] = if variant == 0 {
        &[([-1.0, -1.3, -0.8], [1.0, 1.3, 0.8])]
    } else {
        &[
            ([-1.4, -1.1, -0.9], [1.4, -0.35, 0.9]),
            ([-1.0, -0.35, -0.7], [1.0, 0.4, 0.7]),
            ([-0.6, 0.4, -0.5], [0.6, 1.15, 0.5]),
        ]
    };
    let mut original = Vec::with_capacity(boxes.len() * 36);
    let mut render = Vec::with_capacity(boxes.len() * 36);
    let mut certificates = Vec::with_capacity(boxes.len() * 12);
    for &(lo, hi) in boxes {
        // Cyclic axes guarantee cross(+u,+v)=+normal. Reverse each negative
        // face's corner order, preserving the authored face normal and winding.
        for axis in 0..3 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            for sign in [-1.0, 1.0] {
                let mut normal = [0.0; 3];
                normal[axis] = sign;
                let mut tangent = [0.0, 0.0, 0.0, 1.0];
                tangent[u] = 1.0;
                let mut corners = [[0.0; 3]; 4];
                for (corner, (a, b)) in corners.iter_mut().zip([
                    (lo[u], lo[v]),
                    (hi[u], lo[v]),
                    (hi[u], hi[v]),
                    (lo[u], hi[v]),
                ]) {
                    corner[axis] = if sign > 0.0 { hi[axis] } else { lo[axis] };
                    corner[u] = a;
                    corner[v] = b;
                }
                let order = if sign > 0.0 {
                    [0, 1, 2, 0, 2, 3]
                } else {
                    [0, 2, 1, 0, 3, 2]
                };
                for corner in order {
                    original.push(PbrOriginalVertex3D {
                        position: corners[corner],
                        normal,
                        uv0: None,
                        color: None,
                    });
                    render.push(PbrVertex3D {
                        position: corners[corner],
                        normal,
                        uv0: [0.0; 2],
                        color: [255; 4],
                        tangent,
                    });
                }
                for _ in 0..2 {
                    let triangle = certificates.len();
                    certificates.push(PbrTriangleCertificate3D {
                        original: identity,
                        triangle,
                        certificate_identity: identity.source * 100 + triangle as u64,
                        certificate_revision: 1,
                        frame: PbrFrameCertificate3D::NormalOnly {
                            separator: normal.map(f64::from),
                            lower: [0.5; 3],
                        },
                    });
                }
            }
        }
    }
    let mapping = (0..render.len() as u32).collect();
    let geometry = context.admit_pbr_geometry(
        PbrGeometryDescriptor3D {
            original: PbrOriginalGeometry3D {
                identity,
                vertices: original.into(),
                indices: GeometryIndices::Unindexed,
                signed_tangents: Arc::from([]),
                tangent_provenance: PbrTangentProvenance3D::Authored,
            },
            render: render.into(),
            render_indices: GeometryIndices::Unindexed,
            render_vertex_sources: mapping,
            triangles: certificates.into(),
            mapped: false,
            maximum_certificate_bytes: 65_536,
            maximum_verification_items: 10_000,
        },
        None,
    )?;
    let mut material = PbrMaterialDescriptor3D::new(identity.source, 1, PbrSamplingFamily3D::V1);
    material.base_rgba = if variant == 0 {
        [0.88, 0.27, 0.055, 1.0]
    } else {
        [0.045, 0.34, 0.88, 1.0]
    };
    material.metallic = 0.15;
    material.roughness = 0.65;
    let material = context.admit_pbr_material(material, None)?;
    let snapshot = context.admit_pbr_snapshot(
        StructuralRevision::new(1),
        vec![PbrDrawPacket3D {
            geometry,
            material,
            raster: RasterMode::SingleSided,
            stable_ordinal: 0,
        }],
    )?;
    let pitch = 22.0f32.to_radians() * 0.5;
    let yaw = -32.0f32.to_radians() * 0.5;
    let rotation = Orientation3::try_from_xyzw(pitch.sin(), 0.0, 0.0, pitch.cos())
        .expect("finite pitch")
        .compose_local(
            Orientation3::try_from_xyzw(0.0, yaw.sin(), 0.0, yaw.cos()).expect("finite yaw"),
        )
        .expect("finite orientation");
    let model = context.admit_pbr_model(
        &snapshot,
        ModelRevision::new(1),
        vec![
            ModelPlacement3::try_new(WorldPoint3::origin(), rotation, PositiveScale3::one())
                .expect("finite unit placement"),
        ],
    )?;
    let inverse_light_length = 14.0f64.sqrt().recip();
    let direct = PbrDirectSource3D::capture(
        PbrDirectDescriptor3D {
            identity: identity.source,
            revision: 1,
            meters_per_scene_unit: 1.0,
            directional: Some(PbrDirectionalLight3D {
                surface_to_source: [-1.0, 2.0, 3.0].map(|v| v * inverse_light_length),
                irradiance: [3.0, 2.8, 2.5],
                shadow_far_m: 24.0,
            }),
            points: Arc::from([PbrPointLight3D {
                position: WorldPoint3::try_new(3.0, 1.0, 4.0).expect("finite fill light"),
                intensity: [6.0, 8.0, 10.0],
                radius_m: 0.5,
                range_m: 24.0,
            }]),
            spots: Arc::from([]),
        },
        &snapshot,
        &model,
    )?;
    // Complete conservative source envelope: re=.65, response<3.21;
    // incident RGB <=3+10/.5²=43, so all RGB <139<256. Emission is zero.
    context.admit_pbr_scene(PbrSceneDescriptor3D {
        snapshot,
        model,
        hdr: PbrHdrSource3D {
            identity: identity.source,
            revision: 1,
            exposure_ev: 0.0,
            maximum_rgb: [256.0; 3],
        },
        samples: SampleCount3D::FOUR,
        direct: Some(direct),
        indirect: None,
    })
}

struct RevealScreen {
    placeholder: Label,
    built: bool,
    context: Option<Arc<Context3D>>,
    sources: Option<[PbrScene3D; 2]>,
    extents: [Option<PbrObjectRevealExtent3D>; 2],
    pending: Option<[PbrObjectRevealExtentPending3D; 2]>,
    completion: Arc<Mutex<Option<[Completion; 2]>>>,
    cancel: Arc<AtomicBool>,
    _worker: Option<std::thread::JoinHandle<()>>,
    waker: Option<UiWakeHandle>,
    viewport: Option<TreeWidgetId>,
    status: Option<TreeWidgetId>,
    active: usize,
    desired: usize,
    revision: u64,
    message: Option<String>,
    last_failure: Option<String>,
    reported_front: Option<(usize, bool)>,
}

impl RevealScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            built: false,
            context: None,
            sources: None,
            extents: [None, None],
            pending: None,
            completion: Arc::new(Mutex::new(None)),
            cancel: Arc::new(AtomicBool::new(false)),
            _worker: None,
            waker: None,
            viewport: None,
            status: None,
            active: 0,
            desired: 0,
            revision: 0,
            message: None,
            last_failure: None,
            reported_front: None,
        }
    }

    fn initialize(&mut self, cx: &mut ScreenFrameContext<'_>) -> Result<Viewport3D, String> {
        let gpu = cx
            .app
            .gpu_resources()
            .cloned()
            .ok_or("Native GPU resources unavailable")?;
        let context = Arc::new(Context3D::new(gpu).map_err(|e| format!("Context: {e:?}"))?);
        let sources = [
            source(&context, 0).map_err(|e| format!("Source 1: {e:?}"))?,
            source(&context, 1).map_err(|e| format!("Source 2: {e:?}"))?,
        ];
        let viewport = Viewport3D::new_pbr(
            context.clone(),
            sources[0].clone(),
            Viewport3DViewRevision::new(1),
            view(),
        )
        .map_err(|e| format!("Viewport: {e:?}"))?;
        let (a, pa) = context
            .begin_pbr_object_reveal_extent(&sources[0], 1 << 20)
            .map_err(|e| format!("Extent 1: {e:?}"))?;
        let (b, pb) = context
            .begin_pbr_object_reveal_extent(&sources[1], 1 << 20)
            .map_err(|e| format!("Extent 2: {e:?}"))?;
        let waker = self
            .waker
            .clone()
            .ok_or("No canonical native UI wake handle")?;
        self.pending = Some([pa, pb]);
        self.context = Some(context);
        self.sources = Some(sources);
        let completion = self.completion.clone();
        let cancel = self.cancel.clone();
        // Exactly one finite example-owned CPU worker, exactly two jobs. No
        // Context/GPU/pending lease enters this closure, and no UI join on exit.
        self._worker = Some(std::thread::spawn(move || {
            let ready = [a.verify(cancel.clone()), b.verify(cancel)];
            *completion.lock().unwrap_or_else(|e| e.into_inner()) = Some(ready);
            if let Err(error) = waker.wake() {
                tracing::debug!(?error, "Native owner closed before extent wake");
            }
        }));
        Ok(viewport)
    }

    fn finish_extents(&mut self) -> Result<(), String> {
        let ready = self
            .completion
            .try_lock()
            .ok()
            .and_then(|mut slot| slot.take());
        let Some([a, b]) = ready else {
            return Ok(());
        };
        let [pa, pb] = self
            .pending
            .take()
            .ok_or("Extent completion has no live UI owner")?;
        let context = self.context.as_ref().ok_or("Extent Context retired")?;
        let sources = self.sources.as_ref().ok_or("Extent sources retired")?;
        self.extents = [
            Some(
                context
                    .finish_pbr_object_reveal_extent(
                        pa,
                        a.map_err(|e| format!("Extent 1 CPU: {e:?}"))?,
                        &sources[0],
                    )
                    .map_err(|e| format!("Extent 1 finish: {e:?}"))?,
            ),
            Some(
                context
                    .finish_pbr_object_reveal_extent(
                        pb,
                        b.map_err(|e| format!("Extent 2 CPU: {e:?}"))?,
                        &sources[1],
                    )
                    .map_err(|e| format!("Extent 2 finish: {e:?}"))?,
            ),
        ];
        self.message = Some(format!(
            "{} · selected · both source proofs ready",
            NAMES[self.active]
        ));
        tracing::info!(
            "Both immutable PBR sources and CPU extent proofs ready; 1 / 2 / Space switch sources"
        );
        Ok(())
    }
}

impl Drop for RevealScreen {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl Screen for RevealScreen {
    fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
        self.waker = cx.app.ui_waker();
    }
    fn on_leave(&mut self, _: &mut ScreenContext<'_>) {
        self.cancel.store(true, Ordering::Release);
        self.pending = None;
    }
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
                .with_spacing(10.0)
                .with_padding(EdgeInsets::all(20.0))
                .with_background(Color::from_hex(0x10151D)),
        ));
        cx.tree.add_child(
            root,
            Box::new(
                Label::new("PBR Object Reveal")
                    .with_size(24.0)
                    .with_color(Color::from_hex(0xECF0F5)),
            ),
            None,
            0,
        );
        cx.tree.add_child(
            root,
            Box::new(
                Label::new("1 / 2: switch & reveal     Space: switch     Esc: exit")
                    .with_size(14.0)
                    .with_color(Color::from_hex(0xB2C1D3)),
            ),
            None,
            1,
        );
        let node = match self.initialize(cx) {
            Ok(viewport) => {
                let n = cx.tree.add_child(root, Box::new(viewport), None, 2);
                self.viewport = Some(cx.tree.node(n).unwrap().id());
                n
            }
            Err(error) => {
                tracing::error!(%error);
                cx.tree.add_child(
                    root,
                    Box::new(Label::new(error).with_color(Color::from_hex(0xFFAFA0))),
                    None,
                    2,
                )
            }
        };
        cx.tree.set_flex(node, FlexParams::flexible(1.0));
        let initial_status = if self.viewport.is_some() {
            "1 · Amber cuboid · selected · preparing extent proofs"
        } else {
            "Initialization refused · no PBR viewport"
        };
        let status = cx.tree.add_child(
            root,
            Box::new(Label::new(initial_status).with_color(Color::from_hex(0xD6DFEA))),
            None,
            3,
        );
        self.status = Some(cx.tree.node(status).unwrap().id());
        cx.tree.add_child(
            root,
            Box::new(
                Label::new(
                    "900 ms sweep · N4 PBR · real depth · full-source shadows remain during reveal",
                )
                .with_size(12.0)
                .with_color(Color::from_hex(0x91A2B8)),
            ),
            None,
            4,
        );
        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _: &mut ScreenContext<'_>) -> bool {
        let FusedInputEvent::KeyDown {
            key_code,
            repeat: false,
            ..
        } = event
        else {
            return false;
        };
        self.desired = match key_code {
            KeyCode::Character('1') => 0,
            KeyCode::Character('2') => 1,
            KeyCode::Space => 1 - self.desired,
            _ => return false,
        };
        true
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        if let Err(error) = self.finish_extents() {
            tracing::error!(%error);
            self.message = Some(error);
        }
        if self.desired != self.active && self.extents[self.desired].is_some() {
            let result = (|| -> Result<(), String> {
                let revision = self
                    .revision
                    .checked_add(1)
                    .ok_or("Reveal request revision exhausted")?;
                let tree = cx.tree.as_deref_mut().ok_or("Viewport owner unavailable")?;
                let node = tree
                    .find_by_id(self.viewport.ok_or("Viewport not mounted")?)
                    .ok_or("Viewport retired")?;
                let viewport = tree
                    .node_mut(node)
                    .ok_or("Viewport removed")?
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<Viewport3D>()
                    .ok_or("Viewport owner replaced")?;
                let source = &self.sources.as_ref().ok_or("Source owner retired")?[self.desired];
                viewport
                    .replace_pbr_scene(source.clone(), Viewport3DViewRevision::new(1), view())
                    .map_err(|e| format!("Source request: {e:?}"))?;
                viewport
                    .request_pbr_object_reveal(PbrObjectRevealRequest3D {
                        source: source.clone(),
                        request_revision: revision,
                        duration: DURATION,
                        enabled: true,
                        extent: self.extents[self.desired].clone(),
                    })
                    .map_err(|e| format!("Reveal request: {e:?}"))?;
                self.revision = revision;
                self.active = self.desired;
                self.message = Some(format!(
                    "{} · 900 ms reveal · choose the other source to replay",
                    NAMES[self.active]
                ));
                tracing::info!(
                    source = NAMES[self.active],
                    revision,
                    "Source + matching reveal requested in deferred UI phase"
                );
                Ok(())
            })();
            if let Err(error) = result {
                tracing::error!(%error);
                self.message = Some(error);
                self.desired = self.active;
            }
        }
        if let (Some(tree), Some(id)) = (cx.tree.as_deref(), self.viewport) {
            if let Some(viewport) = tree
                .find_by_id(id)
                .and_then(|n| tree.node(n))
                .and_then(|n| n.widget().as_any().downcast_ref::<Viewport3D>())
            {
                let failure = viewport
                    .current_prepared_target()
                    .and_then(|target| viewport.pbr_failure(target.surface_id()))
                    .map(|error| format!("PBR preparation refused: {error:?}"));
                if failure != self.last_failure {
                    if let Some(error) = &failure {
                        tracing::error!(%error);
                        self.message = Some(error.clone());
                    }
                    self.last_failure = failure;
                }
                if let Some(tuple) = viewport
                    .current_prepared_target()
                    .and_then(|t| t.status().pbr_committed())
                {
                    if self
                        .sources
                        .as_ref()
                        .is_some_and(|sources| tuple.source().same_identity(&sources[self.active]))
                    {
                        let state = (self.active, viewport.needs_update());
                        if self.reported_front != Some(state) {
                            tracing::info!(source = NAMES[self.active], extent = ?tuple.target_key().physical_render_extent(), effect_update_demand = state.1, "Current source committed to PBR front (not physical display observation)");
                            self.reported_front = Some(state);
                        }
                    }
                }
            }
        }
        if let (Some(message), Some(id), Some(tree)) =
            (self.message.take(), self.status, cx.tree.as_deref_mut())
        {
            if let Some(node) = tree.find_by_id(id) {
                if let Some(label) = tree
                    .node_mut(node)
                    .and_then(|n| n.widget_mut().as_any_mut().downcast_mut::<Label>())
                {
                    label.set_text(message);
                }
                tree.mark_layout_dirty(node);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halcyon::renderer::wgpu_renderer::WgpuRenderer;

    #[test]
    fn two_certified_3d_sources_have_matching_worker_extents_and_replay_identity() {
        let renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let context = Arc::new(Context3D::new(renderer.gpu_resources()).unwrap());
        let sources = [source(&context, 0).unwrap(), source(&context, 1).unwrap()];
        assert!(!sources[0].same_identity(&sources[1]));
        assert_ne!(sources[0].identity(), sources[1].identity());
        let (a, pa) = context
            .begin_pbr_object_reveal_extent(&sources[0], 1 << 20)
            .unwrap();
        let (b, pb) = context
            .begin_pbr_object_reveal_extent(&sources[1], 1 << 20)
            .unwrap();
        let ui = std::thread::current().id();
        let (a, b, worker) = std::thread::spawn(move || {
            (
                a.verify(Arc::new(AtomicBool::new(false))).unwrap(),
                b.verify(Arc::new(AtomicBool::new(false))).unwrap(),
                std::thread::current().id(),
            )
        })
        .join()
        .unwrap();
        assert_ne!(ui, worker);
        let a = context
            .finish_pbr_object_reveal_extent(pa, a, &sources[0])
            .unwrap();
        let b = context
            .finish_pbr_object_reveal_extent(pb, b, &sources[1])
            .unwrap();
        let mut viewport = Viewport3D::new_pbr(
            context,
            sources[0].clone(),
            Viewport3DViewRevision::new(1),
            view(),
        )
        .unwrap();
        let extents = [a, b];
        let identities = [sources[0].identity(), sources[1].identity()];
        for (revision, selected) in [(1, 1), (2, 0), (3, 1), (4, 0)] {
            assert!(
                viewport
                    .replace_pbr_scene(
                        sources[selected].clone(),
                        Viewport3DViewRevision::new(1),
                        view()
                    )
                    .unwrap()
            );
            assert!(
                viewport
                    .request_pbr_object_reveal(PbrObjectRevealRequest3D {
                        source: sources[selected].clone(),
                        request_revision: revision,
                        duration: DURATION,
                        enabled: true,
                        extent: Some(extents[1 - selected].clone()),
                    })
                    .is_err()
            );
            let request = PbrObjectRevealRequest3D {
                source: sources[selected].clone(),
                request_revision: revision,
                duration: DURATION,
                enabled: true,
                extent: Some(extents[selected].clone()),
            };
            assert!(viewport.request_pbr_object_reveal(request.clone()).unwrap());
            assert!(!viewport.request_pbr_object_reveal(request).unwrap());
            assert_eq!([sources[0].identity(), sources[1].identity()], identities);
        }
        assert_eq!(DURATION, Duration::from_millis(900));
    }
}
