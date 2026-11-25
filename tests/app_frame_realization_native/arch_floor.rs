//! Native HAL355/HAL370 baseline for the current unlinked PBR path.

use super::*;
use glam::{DQuat, DVec3};
use halcyon::three_d::{self, *};
use halcyon::utils::id::TreeWidgetId;
use halcyon::widgets::screen_host::ScreenHost;
use std::sync::atomic::{AtomicBool, Ordering};

#[path = "arch_floor_source.rs"]
mod source;
use source::{EXTENT, FloorSource, MOVED_SLOT, OCCURRENCES, bounds, camera, sun_direction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Cold,
    Identical,
    Camera,
    Placement,
    RevisionOnly,
    FullControl,
}

impl Step {
    fn view_revision(self) -> Viewport3DViewRevision {
        Viewport3DViewRevision::new(match self {
            Self::Cold => 1,
            Self::FullControl | Self::Identical => 3,
            _ => 2,
        })
    }
}

// These are analytic intersections with the authored box faces, used only to
// identify which source occurrence should cover the measured GPU pixels.
fn box_entry(ray: Ray3, placement: ModelPlacement3) -> Option<f64> {
    let (lo, hi) = bounds(placement);
    let point = ray.origin();
    let direction = ray.direction();
    let origin = [point.x(), point.y(), point.z()];
    let direction = [direction.x(), direction.y(), direction.z()];
    let mut enter = 0.0_f64;
    let mut exit = f64::INFINITY;
    for axis in 0..3 {
        if direction[axis] == 0.0 {
            if !(lo[axis]..=hi[axis]).contains(&origin[axis]) {
                return None;
            }
        } else {
            let a = (lo[axis] - origin[axis]) / direction[axis];
            let b = (hi[axis] - origin[axis]) / direction[axis];
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
        }
    }
    (enter > 0.0 && enter <= exit).then_some(enter)
}

fn project(view: ViewDescriptor3, point: DVec3) -> [u32; 2] {
    let pose = view.pose();
    let eye = pose.translation();
    let q = pose.orientation();
    let rotation = DQuat::from_xyzw(
        f64::from(q.x()),
        f64::from(q.y()),
        f64::from(q.z()),
        f64::from(q.w()),
    );
    let local = rotation.conjugate() * (point - DVec3::new(eye.x(), eye.y(), eye.z()));
    let Projection3::Perspective(projection) = view.projection() else {
        unreachable!("floor perspective view");
    };
    let depth = -local.z;
    assert!(depth > f64::from(projection.near()));
    let half_y = depth * (f64::from(projection.vertical_fov_radians()) / 2.0).tan();
    let [w, h] = EXTENT.map(f64::from);
    let x = (w / 2.0 + (h / 2.0) * local.x / half_y).floor();
    let y = ((h / 2.0) * (1.0 - local.y / half_y)).floor();
    assert!((0.0..w).contains(&x) && (0.0..h).contains(&y));
    [x as u32, y as u32]
}

fn read_box_pixel(
    viewport: &Viewport3D,
    target: Viewport3DTargetStatus<'_>,
    scene: &PbrScene3D,
    placements: &[ModelPlacement3],
    point: DVec3,
    expected_slot: usize,
) -> PbrFrontPixelForTestHarness3D {
    let tuple = target.status().pbr_committed().unwrap();
    let pixel = project(tuple.view(), point);
    // Check the center and all four corners of the witness pixel. The actual
    // N1/N4 coverage is measured separately from the committed FP16 alpha.
    for (dx, dy) in [(0.5, 0.5), (0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)] {
        let u = (f64::from(pixel[0]) + dx) / f64::from(EXTENT[0]);
        let v = (f64::from(pixel[1]) + dy) / f64::from(EXTENT[1]);
        let ray = target.committed_ray(u, v).unwrap().unwrap();
        let winner = placements
            .iter()
            .enumerate()
            .filter_map(|(i, p)| box_entry(ray, *p).map(|depth| (i, depth)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert_eq!(winner.0, expected_slot, "source winner at {pixel:?}");
    }
    let proof = viewport
        .pbr_front_pixel_for_test_harness(target.surface_id(), pixel)
        .unwrap();
    assert!(proof.source.same_identity(scene));
    assert_eq!(proof.extent, EXTENT);
    assert_eq!(proof.target_key, tuple.target_key());
    assert_eq!(proof.request_serial, tuple.request_serial());
    assert_eq!(proof.view_revision, tuple.view_revision());
    assert_eq!(proof.rgba[3], 1.0);
    assert!(
        proof.rgba[..3]
            .iter()
            .all(|&c| c.is_finite() && (0.0..=1.0).contains(&c))
    );
    println!(
        "arch_floor_pixel source={:?} serial={} view={} image={} slot={expected_slot} pixel={pixel:?} fp16={:?}",
        scene.identity(),
        proof.request_serial,
        proof.view_revision,
        proof.image_id,
        proof.rgba,
    );
    proof
}

type Front = (
    Viewport3DTargetCompatibilityKey,
    Viewport3DRequestSerial,
    u64,
);

struct FloorScreen {
    host: ScreenHost,
    context: Option<Arc<Context3D>>,
    fixture: Option<FloorSource>,
    scene: Option<PbrScene3D>,
    viewport: Option<TreeWidgetId>,
    observer: AppFrameObserver,
    samples: SampleCount3D,
    step: Step,
    armed_initial_observation: bool,
    last_attempt: u64,
    last_front: Option<Front>,
    camera_pixels: Option<[[f32; 4]; 2]>,
    settled_pixels: Option<[[f32; 4]; 2]>,
    lit_floor_pixel: Option<[f32; 4]>,
    shadow_floor_pixel: Option<[f32; 4]>,
    request_at: Instant,
    done: Arc<AtomicBool>,
}

impl Screen for FloorScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let gpu = cx.app.gpu_resources().unwrap();
        assert!(gpu.device.features().is_empty());
        println!(
            "arch_floor_device features={:?} timestamp_available={} samples={}",
            gpu.device.features(),
            gpu.device
                .features()
                .contains(wgpu::Features::TIMESTAMP_QUERY),
            self.samples.value(),
        );
        let start = Instant::now();
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let context_us = start.elapsed().as_micros();
        let start = Instant::now();
        let fixture = FloorSource::admit(&context)
            .unwrap_or_else(|e| panic!("540-occurrence geometry/material/snapshot refused: {e:?}"));
        let fixture_us = start.elapsed().as_micros();
        let start = Instant::now();
        let scene = fixture
            .scene(&context, self.samples, 1)
            .unwrap_or_else(|e| panic!("540-occurrence model/Direct/HDR refused: {e:?}"));
        let scene_us = start.elapsed().as_micros();
        assert_eq!(scene.snapshot().packet_count(), OCCURRENCES);
        assert_eq!(scene.presence(), (true, false, false));
        assert_eq!(context.resource_usage().unwrap().resident_geometry_bytes, 0);
        println!(
            "arch_floor_admit occurrences={} geometry={:?} materials={:?} context_us={context_us} geometry_material_snapshot_us={fixture_us} model_direct_hdr_us={scene_us} resources={:?}",
            scene.snapshot().packet_count(),
            fixture.geometry_generation,
            fixture.material_generations,
            context.resource_usage().unwrap(),
        );
        self.request_at = Instant::now();
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            scene.clone(),
            self.step.view_revision(),
            camera(false),
        )
        .unwrap_or_else(|e| panic!("initial unlinked PBR request refused: {e:?}"));
        println!(
            "arch_floor_new_viewport_us={}",
            self.request_at.elapsed().as_micros()
        );
        viewport.set_pbr_timing_for_test(true);
        let node = cx.mount_child(Box::new(viewport));
        self.viewport = Some(cx.tree.node(node).unwrap().id());
        self.context = Some(context);
        self.fixture = Some(fixture);
        self.scene = Some(scene);
        &mut self.host
    }

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        if self.done.load(Ordering::Acquire) {
            return;
        }
        let Some(receipt) = self.observer.latest() else {
            if !std::mem::replace(&mut self.armed_initial_observation, true) {
                cx.app.ui_waker().unwrap().wake().unwrap();
            }
            return;
        };
        let attempt = receipt.attempt_id();
        if attempt.sequence() <= self.last_attempt {
            return;
        }
        let Some(id) = self.viewport else {
            return;
        };
        let tree = cx.tree.as_deref_mut().unwrap();
        let node = tree.find_by_id(id).unwrap();
        let viewport = tree
            .node_mut(node)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Viewport3D>()
            .unwrap();
        let Some(target) = viewport.current_prepared_target() else {
            return;
        };
        let metrics = target.metrics().unwrap();
        if metrics.app_frame_attempt != Some(attempt) {
            return;
        }
        let surface = target.surface_id();
        let context = self.context.as_ref().unwrap();
        assert!(viewport.context().same_resource_family(context));
        if let Some(error) = viewport.pbr_failure(surface) {
            panic!(
                "arch_floor native prepare refused N{} step={:?} exact_error={error:?} state={:?} old={:?} request={:?}",
                self.samples.value(),
                self.step,
                target.status().state(),
                target
                    .status()
                    .pbr_committed()
                    .map(|tuple| (tuple.source().identity(), tuple.request_serial())),
                target
                    .status()
                    .pbr_requested()
                    .map(|tuple| (tuple.source().identity(), tuple.request_serial())),
            );
        }
        assert_eq!(target.status().state(), Viewport3DLifecycleState::Committed);
        let committed = target.status().pbr_committed().unwrap();
        let scene = self.scene.as_ref().unwrap();
        let key = committed.target_key();
        let extent = key.physical_render_extent();
        assert_eq!([extent.width, extent.height], EXTENT);
        assert_eq!(key.sample_count(), self.samples);
        assert_eq!(key.surface_id(), surface);
        assert_eq!(viewport.target_statuses().len(), 1);
        assert!(committed.source().same_identity(scene));
        let request = target.status().pbr_requested().unwrap();
        assert!(request.source().same_identity(scene));
        assert_eq!(committed.request_serial(), request.request_serial());
        assert_eq!(committed.view_revision(), self.step.view_revision());
        assert_eq!(
            committed.view(),
            camera(self.step != Step::Cold)
        );
        assert_eq!(committed.structural_revision(), StructuralRevision::new(1));
        assert_eq!(committed.model_revision(), scene.model().revision());
        assert!(!viewport.needs_update());

        let output = receipt.surface(surface).unwrap();
        assert_eq!(output.visible_spatial_hud_pairs_for_test_harness(), None);
        let timing = viewport.pbr_timing_for_test().unwrap();
        let unchanged = self.step == Step::Identical;
        assert_eq!(
            (
                timing.raster.direct_slots_reused,
                timing.raster.direct_slots_rebuilt
            ),
            match self.step {
                Step::Placement => (OCCURRENCES - 1, 1),
                Step::RevisionOnly => (OCCURRENCES, 0),
                _ => (0, 0),
            }
        );
        assert_eq!(
            timing.raster.direct_source_qualification_reused,
            self.step == Step::Camera
        );
        if output.submitted() {
            assert_eq!(receipt.compile_outcome(), &AppFrameCompileOutcome::Compiled);
            assert!(output.output_produced() && output.presented_called());
            assert!(output.text_realization().unwrap().outcome().is_complete());
            assert_eq!(output.ordinary_draw_counts_for_test_harness(), (1, 0, 2));
        } else {
            assert!(unchanged);
            assert_eq!(
                receipt.compile_outcome(),
                &AppFrameCompileOutcome::SceneUnchanged
            );
            assert!(!(output.output_produced() || output.presented_called()));
        }
        if unchanged {
            assert!(!timing.complete);
            assert_eq!(timing.hdr, HdrRenderTiming3D::default());
            assert_eq!(timing.raster.gpu_creations, [0; 5]);
            assert_eq!(timing.raster.material_group2_creations, 0);
            assert_eq!(timing.raster.material_group2_hits, 0);
            assert_eq!(timing.raster.material_group2_scratch_bytes, 0);
            assert_eq!(timing.raster.shadow_faces, 0);
        } else {
            assert!(timing.complete && timing.raster.direct_present);
            assert_eq!(timing.raster.branch, "New");
            assert_eq!(timing.raster.gpu_creations[1], 10);
            assert_eq!(timing.raster.material_group2_creations, 4);
            assert_eq!(timing.raster.material_group2_hits, OCCURRENCES - 4);
            assert!(timing.raster.material_group2_scratch_bytes > 0);
            assert_eq!(timing.raster.shadow_faces, 3);
            assert_eq!(timing.raster.direct_receiver_triangles, 12 * OCCURRENCES);
            assert_eq!(
                timing.raster.geometry_plans,
                usize::from(self.step == Step::Cold)
            );
        }
        let resources = context.resource_usage().unwrap();
        assert_eq!(resources.resident_geometry_bytes, 36 * (52 + 12));
        let p = u64::from(extent.width) * u64::from(extent.height);
        let attachments = p * (20 * u64::from(self.samples.value()) + 16);
        println!(
            "arch_floor_frame N{} {:?} attempt={} source={:?} direct={:?} structure={:?} model={:?} serial={} view={} key={key:?} scale={} request_to_observer_us={} hdr_attachment_payload={attachments} logical_r025_pool_each=54525952 resources={resources:?} compile={:?} output={} submitted={} presented_called={}",
            self.samples.value(),
            self.step,
            attempt.sequence(),
            scene.identity(),
            scene.direct().unwrap().identity(),
            committed.structural_revision(),
            committed.model_revision(),
            committed.request_serial(),
            committed.view_revision(),
            metrics.scale_factor,
            self.request_at.elapsed().as_micros(),
            receipt.compile_outcome(),
            output.output_produced(),
            output.submitted(),
            output.presented_called(),
        );
        println!(
            "arch_floor_cpu_us {:?} setter={} setter_validate={} active_prepare={} prepare_validate={} raster={} direct_projection={} hdr={} hdr_reused={} encode={} queue_submit_swap={} publish={} creations={:?} geometry_plans={} shadow_faces={} receiver_triangles={} branch={:?}",
            self.step,
            timing.request_total.as_micros(),
            timing.request_validation.as_micros(),
            timing.active_total.as_micros(),
            timing.prepare_validation.as_micros(),
            timing.raster.total.as_micros(),
            timing.raster.direct_projection.as_micros(),
            timing.hdr_total.as_micros(),
            timing.hdr_reused,
            timing.hdr.encode_cpu.as_micros(),
            timing.hdr.submit_swap_cpu.as_micros(),
            timing.publication.as_micros(),
            timing.raster.gpu_creations,
            timing.raster.geometry_plans,
            timing.raster.shadow_faces,
            timing.raster.direct_receiver_triangles,
            timing.raster.branch,
        );
        // Source-accounted calls from the executed encoder loops in raster.rs.
        // D expands each box to nonindexed triangles once per geometry lease.
        println!(
            "arch_floor_draws {:?} scene={} shadow={} indexed=0 tone={}",
            self.step,
            if unchanged { 0 } else { OCCURRENCES },
            timing.raster.shadow_faces * OCCURRENCES,
            usize::from(!unchanged),
        );
        println!(
            "arch_floor_group2 {:?} created={} hits={} scratch_bytes={} create_ns={} lookup_ns={}",
            self.step,
            timing.raster.material_group2_creations,
            timing.raster.material_group2_hits,
            timing.raster.material_group2_scratch_bytes,
            timing.raster.material_group2_create_cpu.as_nanos(),
            timing.raster.material_group2_lookup_cpu.as_nanos(),
        );
        println!(
            "arch_floor_slots {:?} reused={} rebuilt={} plan_ns={} rebuild_ns={} table_recompose_ns={} incoming_cpu_bytes={} seed_ns={} seed_cpu_bytes={}",
            self.step,
            timing.raster.direct_slots_reused,
            timing.raster.direct_slots_rebuilt,
            timing.raster.direct_slot_compare.as_nanos(),
            timing.raster.direct_slot_rebuild.as_nanos(),
            timing.raster.direct_slot_recompose.as_nanos(),
            timing.raster.direct_slot_reserved_cpu_bytes,
            timing.raster.direct_slot_seed.as_nanos(),
            timing.raster.direct_slot_seed_cpu_bytes,
        );

        let placements = &self.fixture.as_ref().unwrap().placements;
        let (floor_point, fixture_point) = match self.step {
            Step::Cold => (DVec3::new(-0.15, 0.0, -14.3), DVec3::new(5.6, 0.8, -10.8)),
            Step::Camera => (DVec3::new(7.8, 0.0, -8.8), DVec3::new(5.6, 0.8, -10.8)),
            Step::Placement | Step::RevisionOnly | Step::FullControl | Step::Identical => {
                (DVec3::new(7.8, 0.0, -8.8), DVec3::new(6.4, 0.8, -10.8))
            }
        };
        let floor = read_box_pixel(viewport, target, scene, placements, floor_point, 0);
        let fixture = read_box_pixel(
            viewport,
            target,
            scene,
            placements,
            fixture_point,
            MOVED_SLOT,
        );
        assert!(floor.rgba[2] > floor.rgba[0] && floor.rgba[0] > 0.0);
        assert!(fixture.rgba[0] > fixture.rgba[2] && fixture.rgba[2] > 0.0);
        assert_eq!(floor.image_id, fixture.image_id);
        let shadow_floor = if self.step != Step::Cold {
            // Light ray from an actual floor receiver passes through the
            // fixture after its 0.8 m move. The view ray must still hit floor.
            let p = DVec3::new(6.16, 0.0, -8.8);
            let l = sun_direction();
            let ray = Ray3::try_new(
                WorldPoint3::try_new(p.x, p.y, p.z).unwrap(),
                WorldVector3::try_new(l.x, l.y, l.z).unwrap(),
            )
            .unwrap();
            let caster = placements
                .iter()
                .enumerate()
                .filter_map(|(slot, &placement)| box_entry(ray, placement).map(|d| (slot, d)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(slot, _)| slot);
            assert_eq!(caster, (self.step != Step::Camera).then_some(MOVED_SLOT));
            Some(read_box_pixel(viewport, target, scene, placements, p, 0).rgba)
        } else {
            None
        };
        assert!(
            viewport
                .pbr_outline_pixel_for_test_harness(surface, floor.pixel)
                .unwrap()
                .is_none()
        );
        let front = (key, floor.request_serial, floor.image_id);
        if let Some(prior) = self.last_front {
            assert_eq!(prior.0, front.0);
            if unchanged {
                assert_eq!(prior, front);
            } else {
                assert!(front.1 > prior.1);
                // The owner swaps the registered backing view under one Image ID.
                assert_eq!(front.2, prior.2);
                assert!(timing.hdr_reused);
            }
        }
        self.last_front = Some(front);
        self.last_attempt = attempt.sequence();

        let next = match self.step {
            Step::Cold => Step::Camera,
            Step::Camera => {
                let destination = read_box_pixel(
                    viewport,
                    target,
                    scene,
                    placements,
                    DVec3::new(6.4, 0.8, -10.8),
                    183,
                );
                self.camera_pixels = Some([fixture.rgba, destination.rgba]);
                self.lit_floor_pixel = shadow_floor;
                Step::Placement
            }
            Step::Placement => {
                let vacated = read_box_pixel(
                    viewport,
                    target,
                    scene,
                    placements,
                    DVec3::new(5.6, 0.8, -10.8),
                    0,
                );
                let [before_old, before_new] = self.camera_pixels.unwrap();
                assert_ne!(vacated.rgba, before_old);
                assert_ne!(fixture.rgba, before_new);
                assert!(vacated.rgba[2] > vacated.rgba[0] && vacated.rgba[0] > 0.0);
                self.settled_pixels = Some([floor.rgba, fixture.rgba]);
                self.shadow_floor_pixel = shadow_floor;
                assert!(shadow_floor.unwrap()[2] < self.lit_floor_pixel.unwrap()[2]);
                Step::RevisionOnly
            }
            Step::RevisionOnly | Step::FullControl => {
                assert_eq!(self.settled_pixels, Some([floor.rgba, fixture.rgba]));
                assert_eq!(self.shadow_floor_pixel, shadow_floor);
                if self.step == Step::RevisionOnly {
                    Step::FullControl
                } else {
                    Step::Identical
                }
            }
            Step::Identical => {
                assert_eq!(self.settled_pixels, Some([floor.rgba, fixture.rgba]));
                assert_eq!(self.shadow_floor_pixel, shadow_floor);
                println!(
                    "arch_floor_native N{} PASS: 540 source occurrences, one mesh, four materials, exact physical 1920x1080, current native fronts and fixture move. GPU execution time and physical scanout unmeasured.",
                    self.samples.value()
                );
                self.done.store(true, Ordering::Release);
                cx.request_exit();
                return;
            }
        };

        self.request_at = Instant::now();
        if next == Step::Placement {
            let fixture = self.fixture.as_mut().unwrap();
            fixture.move_fixture();
            let successor = fixture
                .scene(context, self.samples, 2)
                .unwrap_or_else(|e| panic!("placement-only successor source refused: {e:?}"));
            assert!(successor.snapshot().same_identity(&scene.snapshot()));
            assert!(!successor.model().same_identity(&scene.model()));
            println!(
                "arch_floor_placement_admit_us={} changed_slot={MOVED_SLOT} translate_x_m=0.8",
                self.request_at.elapsed().as_micros(),
            );
            self.scene = Some(successor);
        } else if matches!(next, Step::RevisionOnly | Step::FullControl) {
            // Same placements and physical view. The FullControl view revision
            // changes, which makes the original full path mandatory.
            let revision = if next == Step::RevisionOnly { 3 } else { 4 };
            self.scene = Some(
                self.fixture
                    .as_ref()
                    .unwrap()
                    .scene(context, self.samples, revision)
                    .unwrap(),
            );
        }
        let next_view = camera(next != Step::Cold);
        viewport.set_pbr_timing_for_test(true);
        let changed = viewport
            .request_pbr_scene(
                self.scene.as_ref().unwrap().clone(),
                next.view_revision(),
                next_view,
            )
            .unwrap_or_else(|e| panic!("request {next:?} refused: {e:?}"));
        assert_eq!(changed, next != Step::Identical);
        assert_eq!(
            viewport
                .status_for(surface)
                .unwrap()
                .pbr_committed()
                .unwrap()
                .request_serial(),
            front.1,
            "a request must not advance the previously committed front"
        );
        self.step = next;
        // One observation wake per transition. The actual output still uses the
        // ordinary App preparation, Compile, Queue and surface presentation.
        cx.app.ui_waker().unwrap().wake().unwrap();
    }
}

pub(super) fn run(samples: SampleCount3D) {
    let (config, observer) = AppConfig::new("Open floor PBR baseline")
        .with_size(960.0, 540.0)
        .with_frame_realization_observer();
    let done = Arc::new(AtomicBool::new(false));
    App::new(config).run(FloorScreen {
        host: ScreenHost::new(),
        context: None,
        fixture: None,
        scene: None,
        viewport: None,
        observer,
        samples,
        step: Step::Cold,
        armed_initial_observation: false,
        last_attempt: 0,
        last_front: None,
        camera_pixels: None,
        settled_pixels: None,
        lit_floor_pixel: None,
        shadow_floor_pixel: None,
        request_at: Instant::now(),
        done: done.clone(),
    });
    assert!(
        done.load(Ordering::Acquire),
        "floor fixture exited before placement evidence"
    );
}
