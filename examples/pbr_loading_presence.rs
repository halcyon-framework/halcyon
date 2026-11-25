//! Real HAL381 Spectral Presence. The HAL-owned original/proxy pairs
//! are independent of manufacturer assets or FAB011. Space deliberately
//! requests the real full PBR admission, so the unknown waiting stage can be
//! inspected for any duration without a fake loading timer.
//! cargo run --release --example pbr_loading_presence
use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::layout::{Constraints, EdgeInsets, FlexParams, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::Color;
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::{Flex, Label};
use halcyon::three_d::*;
use halcyon::widgets::slider::Slider;
use sha2::Digest;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::{cell::Cell, rc::Rc};

/// Example-local inherited clipping, using the ordinary Widget contract. It
/// paints no background or Image: the one viewport output is clipped once.
struct RoundedViewportFrame {
    id: halcyon::utils::id::WidgetInternalId,
    size: Size,
}
impl RoundedViewportFrame {
    fn new() -> Self {
        Self {
            id: halcyon::utils::id::WidgetInternalId::new(),
            size: Size::zero(),
        }
    }
}
impl WidgetDowncast for RoundedViewportFrame {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl Widget for RoundedViewportFrame {
    fn id(&self) -> halcyon::utils::id::WidgetInternalId {
        self.id
    }
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let size = Size::new(
            if constraints.max_width.is_finite() {
                constraints.max_width
            } else {
                constraints.min_width
            },
            if constraints.max_height.is_finite() {
                constraints.max_height
            } else {
                constraints.min_height
            },
        );
        for &child in children {
            cx.layout_child(child, Constraints::tight(size));
            cx.set_child_rect(child, RectF::new(0.0, 0.0, size.width, size.height));
        }
        self.size = size;
        LayoutResult::fit(size)
    }
    fn paint(&self, _: &mut PaintContext, _: RectF, _: &[NodeId]) {}
    fn clip_bounds(&self) -> Option<RectF> {
        Some(RectF::new(0.0, 0.0, self.size.width, self.size.height))
    }
    fn clip_bounds_rounded(&self) -> Option<halcyon::core::paint::RoundedCorners> {
        Some(halcyon::core::paint::RoundedCorners::uniform(20.0))
    }
    fn hit_testable(&self) -> bool {
        false
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    App::new(AppConfig::new("Halcyon Spectral Presence").with_size(900.0, 650.0))
        .run(PresenceScreen::new());
}

fn view() -> ViewDescriptor3 {
    ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(0.0, 0.0, 6.8).expect("fixed camera"),
            Orientation3::identity(),
        ),
        PerspectiveProjection::try_finite(40.0_f32.to_radians(), 0.1, 30.0)
            .expect("fixed perspective")
            .into(),
    )
}

struct CpuFixture {
    proxy: LoadingPresenceProxyPrepared3D,
    geometry: Option<PbrGeometryCpuPrepared3D>,
    placement: ModelPlacement3,
    identity: u64,
    base: [f32; 4],
}

fn fixture(variant: usize, cancel: Arc<AtomicBool>) -> Result<CpuFixture, String> {
    let started = std::time::Instant::now();
    let id = PbrOriginalIdentity3D {
        source: 381_100 + variant as u64,
        revision: 1,
        node: 0,
        mesh: 0,
        primitive: 0,
    };
    // Two independently authored closed annular solids. The through-holes,
    // uneven lobes and recessed stations belong to the original geometry.
    let (radius, hole, half_depth) = if variant == 0 {
        (
            [
                1.35, 1.50, 1.24, 0.98, 0.95, 1.27, 1.36, 1.30, 1.00, 1.10, 1.42, 1.52,
            ],
            [0.10, 0.08],
            0.23,
        )
    } else {
        (
            [
                1.36, 1.20, 1.06, 1.43, 1.49, 1.09, 1.09, 1.38, 1.40, 0.90, 1.08, 1.24,
            ],
            [-0.14, 0.03],
            0.31,
        )
    };
    let (outer, inner): (Vec<[f32; 2]>, Vec<[f32; 2]>) = radius
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let (s, c) = (std::f32::consts::TAU * i as f32 / 12.0).sin_cos();
            ([r * c, r * s], [hole[0] + 0.54 * c, hole[1] + 0.45 * s])
        })
        .unzip();
    let front = |[x, y]: [f32; 2]| glam::Vec3::new(x, y, half_depth);
    let back = |[x, y]: [f32; 2]| glam::Vec3::new(x, y, -half_depth);
    let mut original = Vec::new();
    let mut render = Vec::new();
    let mut certificates = Vec::new();
    for i in 0..outer.len() {
        let j = (i + 1) % outer.len();
        let (a, b, c, d) = (outer[i], outer[j], inner[j], inner[i]);
        for quad in [
            [front(a), front(b), front(c), front(d)],
            [back(a), back(d), back(c), back(b)],
            [back(a), back(b), front(b), front(a)],
            [back(d), front(d), front(c), back(c)],
        ] {
            for [a, b, c] in [[0, 1, 2], [0, 2, 3]].map(|ix| ix.map(|i| quad[i])) {
                let edge = b.as_dvec3() - a.as_dvec3();
                let original_normal = edge.cross(c.as_dvec3() - a.as_dvec3()).as_vec3();
                // R024 certifies the f32 packing of the original direction's
                // normalization. Preserve that original separately.
                let normal = original_normal.as_dvec3().normalize().as_vec3().to_array();
                let tangent = edge.normalize().as_vec3().extend(1.0).to_array();
                for p in [a, b, c] {
                    let position = p.to_array();
                    original.push(PbrOriginalVertex3D {
                        position,
                        normal: original_normal.to_array(),
                        uv0: None,
                        color: None,
                    });
                    render.push(PbrVertex3D {
                        position,
                        normal,
                        uv0: [0.0; 2],
                        color: [255; 4],
                        tangent,
                    });
                }
                let triangle = certificates.len();
                certificates.push(PbrTriangleCertificate3D {
                    original: id,
                    triangle,
                    certificate_identity: id.source * 100 + triangle as u64,
                    certificate_revision: 1,
                    frame: PbrFrameCertificate3D::NormalOnly {
                        separator: original_normal.as_dvec3().to_array(),
                        lower: [0.5; 3],
                    },
                });
            }
        }
    }
    let pitch = 22.0_f32.to_radians() * 0.5;
    let yaw = -32.0_f32.to_radians() * 0.5;
    let rotation = Orientation3::try_from_xyzw(pitch.sin(), 0.0, 0.0, pitch.cos())
        .map_err(|e| e.to_string())?
        .compose_local(
            Orientation3::try_from_xyzw(0.0, yaw.sin(), 0.0, yaw.cos())
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    let placement =
        ModelPlacement3::try_new(WorldPoint3::origin(), rotation, PositiveScale3::one())
            .map_err(|e| e.to_string())?;
    let identity = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::one(),
    )
    .map_err(|e| e.to_string())?;
    let base: [f32; 4] = if variant == 0 {
        [0.88, 0.27, 0.055, 1.0]
    } else {
        [0.045, 0.34, 0.88, 1.0]
    };
    // Canonical bytes of this owned fixture recipe. The actual proxy is the
    // identity triangle derivative of the SAME original array, never its AABB.
    let mut root = sha2::Sha256::new();
    root.update(b"HAL381 owned closed annular geometry recipe v2");
    root.update(id.source.to_le_bytes());
    let mut hash = sha2::Sha256::new();
    let mut positions = Vec::new();
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for vertex in &original {
        for c in vertex.position {
            root.update(c.to_bits().to_le_bytes());
            hash.update(c.to_bits().to_le_bytes());
        }
        for c in vertex.normal {
            root.update(c.to_bits().to_le_bytes());
        }
        for c in 0..3 {
            lo[c] = lo[c].min(vertex.position[c]);
            hi[c] = hi[c].max(vertex.position[c]);
        }
        positions.push(
            LocalPoint3::try_new(vertex.position[0], vertex.position[1], vertex.position[2])
                .map_err(|e| e.to_string())?,
        );
    }
    for c in base.into_iter().chain([pitch, yaw]) {
        root.update(c.to_bits().to_le_bytes());
    }
    let root: [u8; 32] = root.finalize().into();
    let hash: [u8; 32] = hash.finalize().into();
    let proxy = LoadingPresenceProxyPrepared3D::prepare(
        Arc::new(
            GeometrySourceDescriptor::try_new(
                positions,
                None,
                None,
                None,
                GeometryIndices::Unindexed,
            )
            .map_err(|e| e.to_string())?,
        ),
        LoadingPresenceSource3D {
            original_binding: root,
            source_revision: std::num::NonZeroU64::new(1).expect("fixed revision"),
            source_generation: std::num::NonZeroU64::new(1).expect("fixed generation"),
            original_root: format!("HAL381 owned source {}", id.source).into(),
            original_resource: "original immutable annular solid triangles".into(),
            proxy_hash_algorithm: "SHA256 LE f32 position bits, unindexed triangle order".into(),
            proxy_byte_hash: Arc::from(hash),
            qualification_receipt:
                "HAL-owned exact original triangle derivative; not manufacturer/HAC provenance"
                    .into(),
            model_frame_origin: WorldPoint3::origin(),
            source_to_proxy: identity,
        },
        placement,
        (lo, hi),
        1 << 20,
        cancel.clone(),
    )
    .map_err(|e| e.to_string())?;
    let mapping = (0..render.len() as u32).collect();
    let proxy_cpu_bytes = proxy.retained_cpu_bytes();
    let geometry = PbrGeometryCpuPrepared3D::prepare(
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
            triangles: certificates.into(),
            mapped: false,
            maximum_certificate_bytes: 65_536,
            maximum_verification_items: 10_000,
        },
        1 << 20,
        cancel,
    )
    .map_err(|e| e.to_string())?;
    tracing::info!(source=variant,worker=?std::thread::current().id(),cpu_us=started.elapsed().as_micros(),proxy_cpu_bytes,
        "HAL-owned original/proxy CPU fixture prepared; not a native selection benchmark");
    Ok(CpuFixture {
        proxy,
        geometry: Some(geometry),
        placement,
        identity: id.source,
        base,
    })
}

fn full_source(
    context: &Context3D,
    geometry: PbrGeometry3D,
    input: &CpuFixture,
    samples: SampleCount3D,
) -> Result<PbrScene3D, String> {
    let mut material = PbrMaterialDescriptor3D::new(input.identity, 1, PbrSamplingFamily3D::V1);
    material.base_rgba = input.base;
    material.metallic = 0.15;
    material.roughness = 0.65;
    let material = context
        .admit_pbr_material(material, None)
        .map_err(|e| e.to_string())?;
    let snapshot = context
        .admit_pbr_snapshot(
            StructuralRevision::new(1),
            vec![PbrDrawPacket3D {
                geometry,
                material,
                raster: RasterMode::SingleSided,
                stable_ordinal: 0,
            }],
        )
        .map_err(|e| e.to_string())?;
    let model = context
        .admit_pbr_model(&snapshot, ModelRevision::new(1), vec![input.placement])
        .map_err(|e| e.to_string())?;
    let inv = 14.0_f64.sqrt().recip();
    let direct = PbrDirectSource3D::capture(
        PbrDirectDescriptor3D {
            identity: input.identity,
            revision: 1,
            meters_per_scene_unit: 1.0,
            directional: Some(PbrDirectionalLight3D {
                surface_to_source: [-1.0, 2.0, 3.0].map(|v| v * inv),
                irradiance: [3.0, 2.8, 2.5],
                shadow_far_m: 24.0,
            }),
            points: Arc::from([PbrPointLight3D {
                position: WorldPoint3::try_new(3.0, 1.0, 4.0).expect("fixed light"),
                intensity: [6.0, 8.0, 10.0],
                radius_m: 0.5,
                range_m: 24.0,
            }]),
            spots: Arc::from([]),
        },
        &snapshot,
        &model,
    )
    .map_err(|e| e.to_string())?;
    // Original conservative full-PBR source envelope, independent of preview.
    context
        .admit_pbr_scene(PbrSceneDescriptor3D {
            snapshot,
            model,
            hdr: PbrHdrSource3D {
                identity: input.identity,
                revision: 1,
                exposure_ev: 0.0,
                maximum_rgb: [256.0; 3],
            },
            samples,
            direct: Some(direct),
            indirect: None,
        })
        .map_err(|e| e.to_string())
}

enum Work {
    Geometry(
        usize,
        PbrGeometryCpuSuccessorJob3D,
        LoadingPresenceRequest3D,
    ),
}
enum Ready {
    Initial(Result<[CpuFixture; 2], String>),
    Geometry(
        usize,
        Result<PbrGeometryCpuResult3D, String>,
        LoadingPresenceRequest3D,
    ),
}

pub(crate) struct PresenceScreen {
    placeholder: Label,
    built: bool,
    context: Option<Arc<Context3D>>,
    input: Option<[CpuFixture; 2]>,
    proxies: Option<[LoadingPresenceProxy3D; 2]>,
    scenes: [Option<PbrScene3D>; 2],
    pending: [Option<PbrGeometryCpuPending3D>; 2],
    work: Option<SyncSender<Work>>,
    ready: Option<Receiver<Ready>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    pub(crate) viewport: Option<NodeId>,
    status: Option<NodeId>,
    transition_legend: Option<NodeId>,
    transition_choice: Rc<Cell<LoadingPresenceTransitionDuration3D>>,
    shown_transition: (LoadingPresenceTransitionDuration3D, bool),
    selected: usize,
    pub(crate) desired: usize,
    pub(crate) complete: bool,
    cpu_active: bool,
    message: String,
    shown_message: String,
    reported: Option<(usize, bool)>,
    samples: SampleCount3D,
}
impl PresenceScreen {
    fn new() -> Self {
        Self::with_samples(SampleCount3D::FOUR)
    }
    pub(crate) fn with_samples(samples: SampleCount3D) -> Self {
        Self {
            placeholder: Label::new(""),
            built: false,
            context: None,
            input: None,
            proxies: None,
            scenes: [None, None],
            pending: [None, None],
            work: None,
            ready: None,
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
            viewport: None,
            status: None,
            transition_legend: None,
            transition_choice: Rc::new(Cell::new(LoadingPresenceTransitionDuration3D::DEFAULT)),
            shown_transition: (LoadingPresenceTransitionDuration3D::DEFAULT, false),
            selected: 0,
            desired: 0,
            complete: false,
            cpu_active: false,
            message: "Preparing two qualified CPU fixtures".into(),
            shown_message: String::new(),
            reported: None,
            samples,
        }
    }

    fn transition_caption(
        &self,
        duration: LoadingPresenceTransitionDuration3D,
        next: bool,
    ) -> String {
        let suffix = if next {
            " · applies to next selection"
        } else {
            ""
        };
        format!(
            "Ghost preview → Full PBR | duration {} ms{suffix} · N{} reverse-Z",
            duration.as_millis(),
            self.samples.value()
        )
    }

    fn begin_full(&mut self, viewport: &mut Viewport3D) -> Result<bool, String> {
        let input = self.input.as_mut().ok_or("CPU fixtures unavailable")?;
        let request = viewport
            .loading_presence_request(&input[self.selected].proxy.association())
            .map_err(|e| e.to_string())?;
        if let Some(scene) = &self.scenes[self.selected] {
            viewport
                .request_loading_presence_pbr(
                    scene.clone(),
                    &request,
                    Viewport3DViewRevision::new(1),
                    view(),
                )
                .map_err(|e| e.to_string())?;
            return Ok(true);
        }
        if self.cpu_active {
            return Ok(false);
        }
        let prepared = input[self.selected]
            .geometry
            .take()
            .ok_or("source geometry already consumed")?;
        let context = self.context.as_ref().ok_or("Context retired")?;
        let (job, pending) = context
            .begin_pbr_geometry_cpu_admission(prepared, None)
            .map_err(|e| e.to_string())?;
        self.pending[self.selected] = Some(pending);
        if let Err(error) = self
            .work
            .as_ref()
            .ok_or("CPU owner retired")?
            .try_send(Work::Geometry(self.selected, job, request))
        {
            self.pending[self.selected] = None;
            return Err(error.to_string());
        }
        self.cpu_active = true;
        self.message =
            "Full source requested · materialization waits for its real registered PBR front"
                .into();
        Ok(true)
    }
}
impl Drop for PresenceScreen {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

impl Screen for PresenceScreen {
    fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
        let Some(wake) = cx.app.ui_waker() else {
            self.message = "Native UI completion wake unavailable".into();
            return;
        };
        let (work, receive) = mpsc::sync_channel(1);
        let (send, ready) = mpsc::sync_channel(1);
        self.work = Some(work);
        self.ready = Some(ready);
        let stop = self.stop.clone();
        self.worker = Some(std::thread::spawn(move || {
            let initial = (|| Ok([fixture(0, stop.clone())?, fixture(1, stop.clone())?]))();
            if send.send(Ready::Initial(initial)).is_err() || wake.wake().is_err() {
                return;
            }
            while let Ok(Work::Geometry(i, job, request)) = receive.recv() {
                let result = job.verify(stop.clone()).map_err(|e| e.to_string());
                if send.send(Ready::Geometry(i, result, request)).is_err() || wake.wake().is_err() {
                    return;
                }
            }
        }));
    }
    fn on_leave(&mut self, _: &mut ScreenContext<'_>) {
        self.stop.store(true, Ordering::Release);
        self.work = None;
        self.ready = None;
        self.pending = [None, None];
        self.scenes = [None, None];
        self.proxies = None;
        self.input = None;
        self.context = None;
        // Dropping the handle never joins the CPU owner on the UI thread.
        self.worker = None;
    }
    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;
        match cx
            .app
            .gpu_resources()
            .cloned()
            .ok_or_else(|| "Native GPU owner unavailable".to_string())
            .and_then(|gpu| Context3D::new(gpu).map(Arc::new).map_err(|e| e.to_string()))
        {
            Ok(context) => self.context = Some(context),
            Err(error) => self.message = error,
        }
        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_spacing(10.0)
                .with_padding(EdgeInsets::all(20.0))
                .with_background(Color::from_hex(0x10151D)),
        ));
        cx.tree.add_child(
            root,
            Box::new(
                Label::new("Source-qualified Spectral Presence")
                    .with_size(23.0)
                    .with_color(Color::from_hex(0xECF0F5)),
            ),
            None,
            0,
        );
        cx.tree.add_child(
            root,
            Box::new(
                Label::new("1 / 2: choose source     Space: request full PBR     Esc: exit")
                    .with_color(Color::from_hex(0xB2C1D3)),
            ),
            None,
            1,
        );
        let choice = self.transition_choice.clone();
        let slider = Slider::new(80.0, 3_000.0)
            .with_label("Transition")
            .with_step(40.0)
            .with_value(choice.get().as_millis() as f32)
            .with_value_display()
            .with_value_formatter(|value| format!("{value:.0} ms"))
            .on_value_changed(move |value| {
                let millis = value as u32;
                if value == millis as f32
                    && let Ok(duration) =
                        LoadingPresenceTransitionDuration3D::try_from_millis(millis)
                {
                    choice.set(duration);
                }
            });
        cx.tree.add_child(root, Box::new(slider), None, 2);
        let frame = cx
            .tree
            .add_child(root, Box::new(RoundedViewportFrame::new()), None, 3);
        cx.tree.set_flex(frame, FlexParams::flexible(1.0));
        let viewport = cx.tree.add_child(
            frame,
            Box::new(Label::new(
                "Preparing the actual original/proxy CPU geometry",
            )),
            None,
            0,
        );
        self.viewport = Some(viewport);
        self.status = Some(cx.tree.add_child(
            root,
            Box::new(Label::new(&self.message).with_color(Color::from_hex(0xD6DFEA))),
            None,
            4,
        ));
        self.transition_legend = Some(
            cx.tree.add_child(
                root,
                Box::new(
                    Label::new(self.transition_caption(self.transition_choice.get(), false))
                        .with_size(12.0)
                        .with_color(Color::from_hex(0x91A2B8)),
                ),
                None,
                5,
            ),
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
        match key_code {
            KeyCode::Character('1') => self.desired = 0,
            KeyCode::Character('2') => self.desired = 1,
            KeyCode::Space => self.complete = true,
            _ => return false,
        }
        true
    }
    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let duration = self.transition_choice.get();
        let mut duration_for_next_selection = self.shown_transition.1;
        let result = (|| -> Result<(), String> {
            let Some(tree) = cx.tree.as_deref_mut() else {
                return Ok(());
            };
            let mut new_occurrence = false;
            while let Some(message) = self.ready.as_ref().and_then(|ready| ready.try_recv().ok()) {
                match message {
                    Ready::Initial(result) => {
                        let admission = std::time::Instant::now();
                        let input = result?;
                        let context = self.context.as_ref().ok_or("native Context unavailable")?;
                        let proxies = [
                            context
                                .admit_loading_presence_proxy(&input[0].proxy)
                                .map_err(|e| e.to_string())?,
                            context
                                .admit_loading_presence_proxy(&input[1].proxy)
                                .map_err(|e| e.to_string())?,
                        ];
                        let viewport = Viewport3D::new_loading_presence(
                            context.clone(),
                            proxies[self.desired].clone(),
                            Viewport3DViewRevision::new(1),
                            view(),
                            self.samples,
                            PresentationMotionPolicy {
                                allow_nonessential_motion: true,
                            },
                        )
                        .map_err(|e| e.to_string())?;
                        tree.replace_widget_only(
                            self.viewport.ok_or("viewport mount unavailable")?,
                            Box::new(viewport),
                        );
                        new_occurrence = true;
                        self.selected = self.desired;
                        self.input = Some(input);
                        self.proxies = Some(proxies);
                        tracing::info!(
                            ui_us = admission.elapsed().as_micros(),
                            "Two immutable proxy resources admitted; GPU target preparation is separate"
                        );
                        self.message =
                            "Qualified spectral presence · Space requests the real full source"
                                .into();
                    }
                    Ready::Geometry(i, result, request) => {
                        self.cpu_active = false;
                        let pending = self.pending[i]
                            .take()
                            .ok_or("CPU result has no UI reservation")?;
                        let result = result?;
                        let context = self.context.as_ref().ok_or("Context retired")?;
                        let geometry = context
                            .finish_pbr_geometry_cpu_admission(pending, result, None)
                            .map_err(|e| e.to_string())?;
                        let scene = full_source(
                            context,
                            geometry,
                            &self.input.as_ref().ok_or("CPU source retired")?[i],
                            self.samples,
                        )?;
                        self.scenes[i] = Some(scene.clone());
                        if self.selected == i && self.desired == i {
                            let viewport = tree
                                .node_mut(self.viewport.ok_or("viewport retired")?)
                                .and_then(|n| {
                                    n.widget_mut().as_any_mut().downcast_mut::<Viewport3D>()
                                })
                                .ok_or("viewport owner replaced")?;
                            if let Err(error) = viewport.request_loading_presence_pbr(
                                scene,
                                &request,
                                Viewport3DViewRevision::new(1),
                                view(),
                            ) {
                                self.message = format!("Obsolete completion refused: {error}");
                            }
                        }
                    }
                }
            }
            let Some(node) = self.viewport else {
                return Ok(());
            };
            let Some(viewport) = tree
                .node_mut(node)
                .and_then(|n| n.widget_mut().as_any_mut().downcast_mut::<Viewport3D>())
            else {
                return Ok(());
            };
            if self.desired != self.selected {
                new_occurrence |= viewport
                    .request_loading_presence(
                        self.proxies.as_ref().ok_or("proxy source unavailable")?[self.desired]
                            .clone(),
                        Viewport3DViewRevision::new(1),
                        view(),
                        self.samples,
                        PresentationMotionPolicy {
                            allow_nonessential_motion: true,
                        },
                    )
                    .map_err(|e| e.to_string())?;
                self.selected = self.desired;
                self.complete = false;
                self.reported = None;
                self.message =
                    "New original source · qualified spectral presence · Space requests full PBR"
                        .into();
            }
            if new_occurrence {
                viewport
                    .set_loading_presence_transition_duration(duration)
                    .map_err(|e| e.to_string())?;
                duration_for_next_selection = false;
            } else if duration != self.shown_transition.0 && !duration_for_next_selection {
                duration_for_next_selection = if self.reported == Some((self.selected, false)) {
                    true
                } else {
                    match viewport.set_loading_presence_transition_duration(duration) {
                        Ok(_) => false,
                        Err(LoadingPresenceError3D::Association(_)) => true,
                        Err(error) => return Err(error.to_string()),
                    }
                };
            }
            if self.complete && self.begin_full(viewport)? {
                self.complete = false;
            }
            if let Some(error) = viewport.loading_presence_error() {
                return Err(error.to_string());
            }
            if let Some(target) = viewport.current_prepared_target() {
                if let Some(error) = viewport.pbr_failure(target.surface_id()) {
                    return Err(error.to_string());
                }
                if target.status().pbr_committed().is_some_and(|tuple| {
                    self.scenes[self.selected]
                        .as_ref()
                        .is_some_and(|s| tuple.source().same_identity(s))
                }) {
                    let state = (self.selected, viewport.needs_update());
                    if self.reported != Some(state) {
                        tracing::info!(
                            source = self.selected,
                            animation_demand = state.1,
                            "Matching canonical PBR committed; physical display observation is separate"
                        );
                        self.reported = Some(state);
                        self.message = if state.1 {
                            "Registered full source · materializing".into()
                        } else {
                            "Full PBR · settled · choose the other source to inspect another wait"
                                .into()
                        };
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            tracing::error!(%error);
            self.message = error;
        }
        if self.message != self.shown_message {
            if let (Some(tree), Some(status)) = (cx.tree.as_deref_mut(), self.status) {
                if let Some(label) = tree
                    .node_mut(status)
                    .and_then(|n| n.widget_mut().as_any_mut().downcast_mut::<Label>())
                {
                    label.set_text(&self.message);
                }
                tree.mark_layout_dirty(status);
                self.shown_message = self.message.clone();
            }
        }
        let transition = (duration, duration_for_next_selection);
        if self.shown_transition != transition
            && let (Some(tree), Some(id)) = (cx.tree.as_deref_mut(), self.transition_legend)
        {
            if let Some(label) = tree
                .node_mut(id)
                .and_then(|n| n.widget_mut().as_any_mut().downcast_mut::<Label>())
            {
                label.set_text(self.transition_caption(duration, duration_for_next_selection));
            }
            tree.mark_layout_dirty(id);
            self.shown_transition = transition;
        }
    }
}
