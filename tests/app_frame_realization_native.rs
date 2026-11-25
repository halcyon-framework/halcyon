//! Native public-API proofs: --run-native, --run-ui-scale-native, --run-startup-native,
//! --run-flat-viewport3d-transfer-native, or --run-pbr-object-reveal-native
//! or --run-r027-mrt4-native [--samples-one] (PBR modes require test-harness).
//! --run-idle-wake-native requires test-harness and observes a wake-only N4->N1 update.
//! --run-spatial-hud-native [--samples-one] observes HAL375 on the submitted frame.
//! --run-spatial-hud-zero-view-native [--inactive-context] observes the ordinary Plot/Label path.
//! --run-arch-floor-native [--samples-one] measures a flat PBR floor on the macOS 2x target.
//! Each mode uses canonical App::run.
//! The normal test suite builds it without opening OS windows.

use halcyon::compositor::SurfaceId;
use halcyon::core::app::{
    App, AppConfig, Screen, ScreenContext, ScreenFrameContext, UiWakeError, UiWakeHandle,
};
use halcyon::core::frame_realization::{
    AppFrameAttemptId, AppFrameCompileOutcome, AppFrameObserver, AppFrameRealization,
};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::Color;
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::renderer::PhysicalRect;
use halcyon::renderer::custom_render_pass::CustomRenderPass;
use halcyon::renderer::wgpu_renderer::GpuResources;
use halcyon::renderer::{
    CustomRenderActivity, CustomRenderAdmission, CustomRenderParticipant,
    CustomRenderPreparationOutcome, CustomRenderTargetMetrics, TextRealizationFailureReason,
    TextRealizationOutcome,
};
use halcyon::utils::id::WidgetInternalId;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
#[path = "app_frame_realization_native/missing_cluster.rs"]
mod missing_cluster_native;

#[cfg(target_os = "macos")]
#[path = "app_frame_realization_native/native_quit.rs"]
mod native_quit;

#[cfg(all(target_os = "macos", feature = "test-harness"))]
#[path = "app_frame_realization_native/arch_floor.rs"]
mod arch_floor_native;

#[cfg(feature = "test-harness")]
#[path = "../examples/pbr_loading_presence.rs"]
mod loading_presence_example;

#[cfg(feature = "test-harness")]
#[path = "../examples/pbr_selection_outline.rs"]
mod selection_outline_example;

const PROBE_TITLE: &str = "Frame realization public API proof";

#[cfg(target_os = "macos")]
fn owned_native_window(title: &str) -> objc2::rc::Retained<objc2_app_kit::NSWindow> {
    let mtm = objc2_foundation::MainThreadMarker::new().expect("native proof main thread");
    let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
    let windows = app.windows();
    let mut matching = windows
        .iter_retained()
        .filter(|window| window.title().to_string() == title);
    let window = matching.next().expect("this proof's native window");
    assert!(matching.next().is_none(), "ambiguous native proof window");
    window
}

#[cfg(all(target_os = "macos", feature = "test-harness"))]
fn resize_native_proof_window(title: &str, width: f32, height: f32) {
    owned_native_window(title)
        .setContentSize(objc2_foundation::NSSize::new(f64::from(width), f64::from(height)));
}

#[derive(Default)]
struct ProbeState {
    requested: u8,
    prepared: Option<(AppFrameAttemptId, SurfaceId, u8)>,
    done: bool,
    ui_waker: Option<UiWakeHandle>,
    wake_worker: Option<std::thread::JoinHandle<Result<(), UiWakeError>>>,
}

struct ProbeWidget {
    id: WidgetInternalId,
    state: Arc<Mutex<ProbeState>>,
    gpu: Option<GpuResources>,
    admission: Option<CustomRenderAdmission>,
    pass: Option<CustomRenderPass>,
    published: Option<u8>,
}

impl ProbeWidget {
    fn new(state: Arc<Mutex<ProbeState>>, gpu: Option<GpuResources>) -> Self {
        let admission = gpu.as_ref().map(CustomRenderAdmission::from_gpu_resources);
        Self {
            id: WidgetInternalId::new(),
            state,
            gpu,
            admission,
            pass: None,
            published: None,
        }
    }
}

impl Drop for ProbeWidget {
    fn drop(&mut self) {
        if let Some(pass) = self.pass.take() {
            pass.destroy_deferred().expect("retire probe front");
        }
    }
}

impl WidgetDowncast for ProbeWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ProbeWidget {
    fn layout(
        &mut self,
        _: &mut LayoutContext,
        constraints: Constraints,
        _: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        cx.scene.push_rect(rect, Color::BLACK, 0, None);
        cx.scene.push_target_bound_image(
            rect,
            self.admission.as_ref().unwrap().presentation_id(),
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            1,
            None,
        );
        let text = if self.state.lock().unwrap().requested == 1 {
            "A\u{10ffff}"
        } else {
            "A A"
        };
        let run = cx.text_engine.shape(text, 18.0);
        cx.text_engine
            .draw(&run, rect.x + 8.0, rect.y + 8.0, Color::WHITE, 2, cx.scene);
    }

    fn needs_update(&self) -> bool {
        !self.state.lock().unwrap().done
    }
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn custom_render_admission(&self) -> Option<&CustomRenderAdmission> {
        self.admission.as_ref()
    }
    fn custom_render_participant_mut(&mut self) -> Option<&mut dyn CustomRenderParticipant> {
        Some(self)
    }
}

impl CustomRenderParticipant for ProbeWidget {
    fn prepare_custom_render(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
    ) -> CustomRenderPreparationOutcome {
        let mut state = self.state.lock().unwrap();
        let attempt = metrics
            .app_frame_attempt
            .expect("canonical App attempt before preparation");
        state.prepared = Some((attempt, metrics.surface_id, state.requested));
        let CustomRenderActivity::Active {
            physical_render_extent,
            ..
        } = metrics.activity
        else {
            return CustomRenderPreparationOutcome::default();
        };
        let rect = PhysicalRect {
            x: 0,
            y: 0,
            width: physical_render_extent.width,
            height: physical_render_extent.height,
        };
        if self.pass.is_none() {
            self.pass = Some(
                CustomRenderPass::new(self.id.raw(), self.gpu.as_ref().unwrap().clone(), rect)
                    .unwrap(),
            );
        }
        let pass = self.pass.as_mut().unwrap();
        let resized = pass.physical_rect() != rect;
        if resized {
            pass.resize(rect).unwrap();
        }
        let changed = resized || self.published != Some(state.requested);
        if changed {
            let mut frame = pass
                .begin_frame(Some("app-frame-realization-native"))
                .unwrap();
            {
                let (encoder, view) = frame.encoder_and_view();
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("generic candidate"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        depth_slice: None,
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.1,
                                g: 0.2,
                                b: 0.3,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    multiview_mask: None,
                    occlusion_query_set: None,
                });
            }
            pass.submit_and_swap(frame).unwrap();
            self.published = Some(state.requested);
        }
        CustomRenderPreparationOutcome::with_presentation_binding(
            changed,
            self.admission
                .as_ref()
                .unwrap()
                .presentation_binding(pass.texture_id()),
        )
    }
}

struct ProbeScreen {
    state: Arc<Mutex<ProbeState>>,
    observer: AppFrameObserver,
    stub: ProbeWidget,
    node: Option<NodeId>,
    stage: u8,
    first: Option<Arc<AppFrameRealization>>,
    started: Instant,
    scale_proof: bool,
    #[cfg(windows)]
    inactive: Option<(AppFrameAttemptId, u64)>,
}

impl Screen for ProbeScreen {
    fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
        let waker = cx.app.ui_waker().expect("native Screen sees its App proxy");
        let worker_waker = waker.clone();
        let worker = std::thread::spawn(move || {
            // Only the public send-only handle crosses the CPU boundary.
            for _ in 0..4 {
                worker_waker.wake()?;
            }
            Ok(())
        });
        {
            let mut state = self.state.lock().unwrap();
            state.ui_waker = Some(waker);
            state.wake_worker = Some(worker);
        }
        assert!(
            cx.app
                .frame_realization_observer()
                .unwrap()
                .latest()
                .is_none()
        );
        cx.request_child_window("Frame receipt second surface", 160, 100, "");
    }

    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let widget = ProbeWidget::new(
            self.state.clone(),
            Some(cx.app.gpu_resources().unwrap().clone()),
        );
        self.node = Some(cx.mount_child(Box::new(widget)));
        &mut self.stub
    }

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        assert!(
            self.started.elapsed() < Duration::from_secs(20),
            "native frame receipt proof timed out"
        );
        if !self
            .state
            .lock()
            .unwrap()
            .wake_worker
            .as_ref()
            .expect("one native send worker")
            .is_finished()
        {
            return;
        }
        let Some(receipt) = cx.app.frame_realization_observer().unwrap().latest() else {
            return;
        };
        assert!(Arc::ptr_eq(&receipt, &self.observer.latest().unwrap()));
        if receipt.compile_outcome() != &AppFrameCompileOutcome::Compiled
            || receipt.surfaces().len() != 2
        {
            return;
        }
        let mut state = self.state.lock().unwrap();
        let (attempt, primary, candidate) = state.prepared.expect("preparation evidence");
        let main = receipt.surface(primary).unwrap();
        let secondary = receipt
            .surfaces()
            .iter()
            .find(|surface| surface.surface_id() != primary)
            .unwrap();
        assert!(
            secondary
                .text_realization()
                .unwrap()
                .outcome()
                .is_complete()
        );
        assert!(
            secondary.output_produced() && secondary.submitted() && secondary.presented_called()
        );
        #[cfg(windows)]
        if self.scale_proof && self.stage == 3 {
            use halcyon::core::app::UiScaleTargetOutput;
            let primary_status = cx.app.ui_scale_status(Some(primary));
            let target = primary_status.target.unwrap();
            if target.output != UiScaleTargetOutput::Inactive {
                return;
            }
            let effective = target.effective.unwrap();
            assert!(effective.physical_size.0 == 0 || effective.physical_size.1 == 0);
            assert_eq!(primary_status.admitted.get(), 1.6);
            assert_eq!(primary_status.admitted_revision, 1);
            assert_eq!(effective.admitted_revision, 1);
            assert_eq!(
                effective.effective_scale_factor,
                effective.platform_density * 1.6
            );
            assert!(main.text_realization().is_none());
            assert!(!main.output_produced() && !main.submitted() && !main.presented_called());
            assert!(receipt.attempt_id().sequence() > attempt.sequence());
            let child = cx.app.ui_scale_status(Some(secondary.surface_id()));
            let child_target = child.target.unwrap();
            let child_metrics = child_target.effective.unwrap();
            assert_eq!(child.admitted.get(), 1.6);
            assert_eq!(child.admitted_revision, 1);
            assert_eq!(child_metrics.admitted_revision, 1);
            assert_eq!(
                child_metrics.effective_scale_factor,
                child_metrics.platform_density * 1.6
            );
            assert!(child_metrics.physical_size.0 > 0 && child_metrics.physical_size.1 > 0);
            assert_eq!(child_target.output, UiScaleTargetOutput::Submitted);
            let first = self.first.as_ref().unwrap();
            assert_eq!(
                receipt
                    .surfaces()
                    .iter()
                    .map(|surface| surface.surface_id())
                    .collect::<Vec<_>>(),
                first
                    .surfaces()
                    .iter()
                    .map(|surface| surface.surface_id())
                    .collect::<Vec<_>>(),
            );
            println!(
                "ui_scale_native: inactive attempt={} primary={primary:?} P={:?} metrics_revision={} secondary={:?} P={:?} primary_output=false secondary_submitted=true S=1.6 revision=1",
                receipt.attempt_id().sequence(),
                effective.physical_size,
                effective.metrics_revision,
                secondary.surface_id(),
                child_metrics.physical_size,
            );
            self.inactive = Some((receipt.attempt_id(), effective.metrics_revision));
            self.stage = 4;
            state.requested = self.stage;
            cx.tree
                .as_deref_mut()
                .unwrap()
                .mark_paint_dirty(self.node.unwrap());
            drop(state);
            startup_native_proof::set_primary_minimized(PROBE_TITLE, false);
            return;
        }
        #[cfg(windows)]
        if self.scale_proof && self.stage == 4 {
            let target = cx.app.ui_scale_status(Some(primary)).target.unwrap();
            let effective = target.effective.unwrap();
            if effective.physical_size.0 == 0 || effective.physical_size.1 == 0 || !main.submitted()
            {
                return;
            }
            let (inactive_attempt, inactive_metrics) = self.inactive.unwrap();
            assert!(receipt.attempt_id().sequence() > inactive_attempt.sequence());
            assert!(effective.metrics_revision > inactive_metrics);
            assert_eq!(
                self.first.as_ref().unwrap().surfaces()[0].surface_id(),
                primary
            );
            println!(
                "ui_scale_native: restored attempt={} primary={primary:?} P={:?} metrics_revision={} primary_submitted=true secondary_submitted=true",
                receipt.attempt_id().sequence(),
                effective.physical_size,
                effective.metrics_revision,
            );
        }
        assert_eq!(receipt.attempt_id(), attempt);
        assert_eq!(candidate, self.stage);
        if self.scale_proof {
            let expected = if self.stage == 0 { 1.25 } else { 1.6 };
            for surface in [primary, secondary.surface_id()] {
                let status = cx.app.ui_scale_status(Some(surface));
                assert_eq!(status.admitted.get(), expected);
                assert_eq!(status.admitted_revision, u64::from(self.stage > 0));
                let effective = status.target.unwrap().effective.unwrap();
                assert_eq!(effective.admitted_revision, status.admitted_revision);
                assert_eq!(
                    effective.effective_scale_factor,
                    effective.platform_density * expected
                );
                assert!(effective.physical_size.0 > 0 && effective.physical_size.1 > 0);
                println!(
                    "ui_scale_native: stage={} surface={surface:?} D={} P={:?} S={} E={} revision={}",
                    self.stage,
                    effective.platform_density,
                    effective.physical_size,
                    expected,
                    effective.effective_scale_factor,
                    effective.admitted_revision
                );
            }
            if self.stage == 0 {
                let next = halcyon::core::app::UiScale::try_from(1.6).unwrap();
                assert!(matches!(
                    cx.app.request_ui_scale(next),
                    Ok(halcyon::core::app::UiScaleReceipt::Queued { .. })
                ));
                assert_eq!(cx.app.ui_scale_status(None).admitted.get(), 1.25);
            }
        }
        let outcome = main.text_realization().unwrap().outcome();
        let primary_text = if self.stage == 1 {
            let TextRealizationOutcome::Degraded {
                missing_cluster_count,
                first_failure,
                ..
            } = outcome
            else {
                panic!("expected contained missing-cluster text at stage 1, got {outcome:?}");
            };
            assert_eq!(first_failure.reason, TextRealizationFailureReason::MissingGlyph);
            assert!(*missing_cluster_count > 0);
            assert!(!outcome.is_complete());
            assert!(outcome.is_renderable());
            "Degraded(MissingGlyph)"
        } else {
            assert!(outcome.is_complete());
            "Complete"
        };
        assert!(main.output_produced() && main.submitted() && main.presented_called());
        if let Some(first) = &self.first {
            assert_ne!(first.attempt_id(), receipt.attempt_id());
            assert!(first.attempt_id().sequence() < receipt.attempt_id().sequence());
            assert!(first.surface(primary).unwrap().submitted());
        } else {
            self.first = Some(receipt.clone());
        }
        println!(
            "app_frame_realization_native: candidate={} attempt={} surfaces=2 primary_output={} secondary_output={} primary_text={primary_text} secondary_text=Complete primary_submitted={} secondary_submitted={} primary_present_called={} secondary_present_called={}",
            candidate,
            attempt.sequence(),
            main.output_produced(),
            secondary.output_produced(),
            main.submitted(),
            secondary.submitted(),
            main.presented_called(),
            secondary.presented_called(),
        );
        #[cfg(windows)]
        if self.scale_proof && self.stage == 2 {
            self.stage = 3;
            drop(state);
            startup_native_proof::set_primary_minimized(PROBE_TITLE, true);
            return;
        }
        if self.stage >= 2 {
            state.done = true;
            cx.request_exit();
        } else {
            self.stage += 1;
            state.requested = self.stage;
            cx.tree
                .as_deref_mut()
                .unwrap()
                .mark_paint_dirty(self.node.unwrap());
        }
    }
}

#[cfg(windows)]
mod startup_native_proof {
    use super::*;
    use halcyon::renderer::image_binding::{
        ImageBinding, ImageBindingIdentity, ImageBindingRequest,
    };
    use std::ffi::c_void;

    const IMAGE: u64 = 263_361_990;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class_name: *const u16, window_name: *const u16) -> *mut c_void;
        fn ShowWindow(window: *mut c_void, command: i32) -> i32;
        fn IsIconic(window: *mut c_void) -> i32;
        fn IsWindowVisible(window: *mut c_void) -> i32;
        fn GetClientRect(window: *mut c_void, rect: *mut ClientRect) -> i32;
        fn GetDC(window: *mut c_void) -> *mut c_void;
        fn ReleaseDC(window: *mut c_void, dc: *mut c_void) -> i32;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn GetPixel(dc: *mut c_void, x: i32, y: i32) -> u32;
    }

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmFlush() -> i32;
    }

    #[repr(C)]
    #[derive(Default)]
    struct ClientRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    fn primary_window(title: &str) -> *mut c_void {
        let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        let window = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        assert!(!window.is_null(), "startup proof primary HWND must exist");
        window
    }

    fn primary_window_visible(title: &str) -> bool {
        let window = primary_window(title);
        unsafe { IsWindowVisible(window) != 0 }
    }

    pub(super) fn set_primary_minimized(title: &str, minimized: bool) {
        const SW_MINIMIZE: i32 = 6;
        const SW_RESTORE: i32 = 9;
        let window = primary_window(title);
        unsafe {
            ShowWindow(window, if minimized { SW_MINIMIZE } else { SW_RESTORE });
        }
        assert_eq!(unsafe { IsIconic(window) != 0 }, minimized);
    }

    fn assert_first_visible_pixels(title: &str) {
        let window = primary_window(title);
        assert!(unsafe { IsWindowVisible(window) != 0 });
        assert!(unsafe { DwmFlush() } >= 0, "DWM composition flush failed");
        let mut rect = ClientRect::default();
        assert!(unsafe { GetClientRect(window, &mut rect) } != 0);
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        assert!(width >= 4 && height >= 4);
        let dc = unsafe { GetDC(window) };
        assert!(!dc.is_null(), "startup proof primary client DC unavailable");
        let positions = [
            (rect.left + width / 4, rect.top + height / 4),
            (rect.left + 3 * width / 4, rect.top + height / 4),
            (rect.left + width / 4, rect.top + 3 * height / 4),
            (rect.left + 3 * width / 4, rect.top + 3 * height / 4),
        ];
        let samples = positions.map(|(x, y)| unsafe { GetPixel(dc, x, y) });
        assert!(unsafe { ReleaseDC(window, dc) } != 0);
        let colors = samples.map(|sample| {
            assert_ne!(
                sample,
                u32::MAX,
                "primary client GDI pixel observation unavailable"
            );
            [
                (sample & 0xff) as u8,
                ((sample >> 8) & 0xff) as u8,
                ((sample >> 16) & 0xff) as u8,
            ]
        });
        println!("app_frame_realization_startup_native: first_visible_client_rgb={colors:?}");
        let expected = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]];
        for (index, (actual, expected)) in colors.into_iter().zip(expected).enumerate() {
            assert!(
                actual
                    .into_iter()
                    .zip(expected)
                    .all(|(channel, expected)| channel.abs_diff(expected) <= 24),
                "first visible primary client sample {index} was {actual:?}, expected near {expected:?}; GDI alone cannot distinguish flip-model observation failure from incorrect visible pixels",
            );
        }
    }

    #[derive(Default)]
    struct StartupState {
        binding: Option<ImageBinding>,
        painted_binding: Option<ImageBindingIdentity>,
        entered_hidden: bool,
        pending_flushes: usize,
        done: bool,
    }

    struct ImageProbe {
        id: WidgetInternalId,
        state: Arc<Mutex<StartupState>>,
    }

    impl ImageProbe {
        fn new(state: Arc<Mutex<StartupState>>) -> Self {
            Self {
                id: WidgetInternalId::new(),
                state,
            }
        }
    }

    impl WidgetDowncast for ImageProbe {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for ImageProbe {
        fn layout(
            &mut self,
            _: &mut LayoutContext,
            constraints: Constraints,
            _: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
        }

        fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
            cx.scene.push_rect(rect, Color::BLACK, 0, None);
            let mut state = self.state.lock().unwrap();
            if let Some(binding) = state.binding.as_ref() {
                binding
                    .availability()
                    .expect("startup image remains admitted");
                cx.scene.push_admitted_image(
                    rect,
                    binding,
                    RectF::new(0.0, 0.0, 1.0, 1.0),
                    Color::WHITE,
                    1,
                    None,
                );
                state.painted_binding = Some(binding.identity());
            }
        }

        fn needs_update(&self) -> bool {
            !self.state.lock().unwrap().done
        }

        fn id(&self) -> WidgetInternalId {
            self.id
        }
    }

    struct StartupScreen {
        title: String,
        state: Arc<Mutex<StartupState>>,
        observer: AppFrameObserver,
        request: Option<ImageBindingRequest>,
        node: Option<NodeId>,
        stub: ImageProbe,
        fullscreen: bool,
    }

    impl Screen for StartupScreen {
        fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
            assert!(
                !primary_window_visible(&self.title),
                "on_enter must run hidden"
            );
            assert!(self.observer.latest().is_none());
            self.state.lock().unwrap().entered_hidden = true;
            self.request = Some(
                cx.app
                    .gpu_resources()
                    .expect("startup probe GPU resources")
                    .queue_image_rgba8_srgb(
                        IMAGE,
                        2,
                        2,
                        vec![
                            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
                        ],
                    )
                    .expect("queue startup image pixels"),
            );
        }

        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            self.node = Some(cx.mount_child(Box::new(ImageProbe::new(self.state.clone()))));
            &mut self.stub
        }

        fn take_rebuild_flag(&mut self) -> bool {
            false
        }

        fn startup_ready(&self) -> Result<bool, String> {
            let state = self.state.lock().unwrap();
            match &state.binding {
                Some(binding) => binding
                    .availability()
                    .map(|()| true)
                    .map_err(|failure| format!("startup image unavailable: {failure:?}")),
                None => Ok(false),
            }
        }

        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            let mut state = self.state.lock().unwrap();
            if state.done {
                return;
            }
            if state.binding.is_none() {
                assert!(
                    !primary_window_visible(&self.title),
                    "primary window became visible before ordinary image admission settled"
                );
                match self
                    .request
                    .as_ref()
                    .expect("on_enter queued startup image")
                    .try_resolve()
                    .expect("startup image admission failed")
                {
                    None => {
                        state.pending_flushes += 1;
                        return;
                    }
                    Some(binding) => {
                        assert!(
                            state.pending_flushes > 0,
                            "ordinary pending frame must precede admission"
                        );
                        assert_eq!(binding.identity().texture_id(), IMAGE);
                        binding
                            .availability()
                            .expect("startup image admission availability");
                        state.binding = Some(binding);
                        cx.tree
                            .as_deref_mut()
                            .unwrap()
                            .mark_paint_dirty(self.node.expect("startup image probe mounted"));
                        return;
                    }
                }
            }

            assert!(
                primary_window_visible(&self.title),
                "the completed ready frame must reveal the actual primary HWND"
            );
            let receipt = self
                .observer
                .latest()
                .expect("ready frame published its receipt");
            assert!(Arc::ptr_eq(
                &receipt,
                &cx.app
                    .frame_realization_observer()
                    .unwrap()
                    .latest()
                    .unwrap(),
            ));
            assert_eq!(receipt.compile_outcome(), &AppFrameCompileOutcome::Compiled);
            assert_eq!(
                receipt.surfaces().len(),
                1,
                "startup proof owns only the primary surface"
            );
            let primary = &receipt.surfaces()[0];
            assert!(primary.text_realization().unwrap().outcome().is_complete());
            assert!(primary.output_produced());
            assert!(primary.submitted());
            assert!(primary.presented_called());
            assert_eq!(
                state.painted_binding,
                Some(state.binding.as_ref().unwrap().identity()),
                "ready frame must paint the exact admitted image binding",
            );
            assert_first_visible_pixels(&self.title);
            println!(
                "app_frame_realization_startup_native: fullscreen={} pending_flushes={} attempt={} primary_visible=true primary_output=true submitted=true present_called=true",
                self.fullscreen,
                state.pending_flushes,
                receipt.attempt_id().sequence(),
            );
            state.done = true;
            cx.request_exit();
        }
    }

    pub fn run(fullscreen: bool) {
        let title = format!(
            "Halcyon startup native proof {} fullscreen={fullscreen}",
            std::process::id(),
        );
        let (config, observer) = AppConfig::new(&title)
            .with_size(320.0, 180.0)
            .with_fullscreen(fullscreen)
            .with_frame_realization_observer();
        let state = Arc::new(Mutex::new(StartupState::default()));
        let screen = StartupScreen {
            title,
            state: state.clone(),
            observer,
            request: None,
            node: None,
            stub: ImageProbe::new(state.clone()),
            fullscreen,
        };
        App::new(config).run(screen);
        let state = state.lock().unwrap();
        assert!(state.entered_hidden);
        assert!(state.pending_flushes > 0);
        assert!(
            state.done,
            "startup proof exited before visibility and frame evidence"
        );
        println!(
            "app_frame_realization_startup_native: PASS (HWND visibility and primary submit/present ordering)"
        );
    }
}

mod flat_viewport3d_transfer_native {
    use super::*;
    use halcyon::core::widget::UpdateContext;
    use halcyon::three_d::{
        AppearanceMode, AppearanceSourceDescriptor, Context3D, DrawPacketDescriptor,
        FlatViewport3DLinkId, FlatViewport3DPublicationStatus, FlatViewport3DTransferOutcome,
        FlatViewport3DTransferStage, GeometryIndices, GeometrySemantics, GeometrySourceDescriptor,
        LocalPoint3, ModelPlacement3, ModelRevision, ModelState, ModelStateDescriptor,
        Orientation3, PerspectiveProjection, PositiveScale3, RasterMode, RenderSnapshot,
        RenderSnapshotDescriptor, RigidPose3, SampleCount3D, StructuralRevision, ViewDescriptor3,
        Viewport3D, Viewport3DError, Viewport3DLifecycleState, Viewport3DRequestSerial,
        Viewport3DViewRevision, WorldPoint3,
    };
    use halcyon::utils::id::TreeWidgetId;
    use halcyon::widgets::screen_host::ScreenHost;
    use std::num::NonZeroUsize;

    fn view_at(x: f64) -> ViewDescriptor3 {
        ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(x, 0.0, 0.0).unwrap(),
                Orientation3::identity(),
            ),
            PerspectiveProjection::try_finite(std::f32::consts::FRAC_PI_2, 0.1, 10.0)
                .unwrap()
                .into(),
        )
    }

    fn request_values(context: &Context3D) -> (RenderSnapshot, ModelState) {
        let geometry = context
            .register_geometry(
                GeometrySourceDescriptor::try_new(
                    vec![
                        LocalPoint3::try_new(-0.78, -0.74, 0.0).unwrap(),
                        LocalPoint3::try_new(0.82, -0.66, 0.0).unwrap(),
                        LocalPoint3::try_new(-0.07, 0.79, 0.0).unwrap(),
                    ],
                    None,
                    None,
                    None,
                    GeometryIndices::Unindexed,
                )
                .unwrap(),
            )
            .unwrap();
        let appearance = context
            .register_appearance(
                AppearanceSourceDescriptor::try_new_with_mode(
                    r#"
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
    return vec3<f32>(0.3, 0.7, 1.0);
}
"#,
                    GeometrySemantics::NONE,
                    AppearanceMode::Opaque,
                )
                .unwrap(),
            )
            .unwrap();
        let snapshot = context
            .admit_render_snapshot(RenderSnapshotDescriptor::new(
                StructuralRevision::new(1),
                vec![DrawPacketDescriptor::new(
                    geometry.handle().clone(),
                    appearance.handle().clone(),
                    RasterMode::SingleSided,
                    0,
                )],
            ))
            .unwrap();
        let model = context
            .admit_model_state(
                &snapshot,
                ModelStateDescriptor::new(
                    ModelRevision::new(1),
                    vec![
                        ModelPlacement3::try_new(
                            WorldPoint3::try_new(0.0, 0.0, -1.0).unwrap(),
                            Orientation3::identity(),
                            PositiveScale3::one(),
                        )
                        .unwrap(),
                    ],
                ),
            )
            .unwrap();
        (snapshot, model)
    }

    #[derive(Clone)]
    struct PublicationEvidence {
        correlation: u64,
        dt: f32,
        revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
    }

    struct TimedProducer {
        id: WidgetInternalId,
        link: Option<FlatViewport3DLinkId>,
        snapshot: RenderSnapshot,
        model: ModelState,
        view: ViewDescriptor3,
        revision: Viewport3DViewRevision,
        next_publication: Option<bool>,
        publication: Option<PublicationEvidence>,
        finished: bool,
    }

    impl TimedProducer {
        fn new(snapshot: RenderSnapshot, model: ModelState) -> Self {
            Self {
                id: WidgetInternalId::new(),
                link: None,
                snapshot,
                model,
                view: view_at(0.0),
                revision: Viewport3DViewRevision::new(1),
                next_publication: Some(true),
                publication: None,
                finished: false,
            }
        }
    }

    impl WidgetDowncast for TimedProducer {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for TimedProducer {
        fn layout(
            &mut self,
            _: &mut LayoutContext,
            constraints: Constraints,
            _: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
        }

        fn paint(&self, _: &mut PaintContext, _: RectF, _: &[NodeId]) {}

        fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
            let Some(changed) = self.next_publication else {
                return;
            };
            assert!(dt.is_finite() && dt >= 0.0, "canonical frame delta");
            if changed && dt == 0.0 {
                return;
            }
            if changed {
                self.revision = self.revision.checked_next().unwrap();
                self.view = view_at(0.05 + f64::from(dt).min(0.25));
            }
            let correlation = if changed { 1 } else { 2 };
            let admission = cx
                .publish_flat_viewport3d_request(
                    self.link
                        .as_ref()
                        .expect("application supplied this producer's link"),
                    correlation,
                    self.revision,
                    self.view,
                    self.snapshot.clone(),
                    self.model.clone(),
                    SampleCount3D::ONE,
                )
                .expect("own timed Update admits the complete immutable request");
            assert_eq!(
                admission,
                FlatViewport3DPublicationStatus::AcceptedForCutoff
            );
            self.publication = Some(PublicationEvidence {
                correlation,
                dt,
                revision: self.revision,
                view: self.view,
            });
            self.next_publication = None;
        }

        fn needs_update(&self) -> bool {
            !self.finished
        }

        fn hit_testable(&self) -> bool {
            false
        }

        fn id(&self) -> WidgetInternalId {
            self.id
        }
    }

    struct TransferScreen {
        stub: ScreenHost,
        link: Option<FlatViewport3DLinkId>,
        source: Option<TreeWidgetId>,
        target: Option<TreeWidgetId>,
        observer: AppFrameObserver,
        first_serial: Option<Viewport3DRequestSerial>,
        stage: u8,
        started: Instant,
        done: Arc<Mutex<bool>>,
    }

    impl Screen for TransferScreen {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let context = Arc::new(
                Context3D::new(cx.app.gpu_resources().expect("canonical App GPU").clone())
                    .expect("explicit public 3D context"),
            );
            let (snapshot, model) = request_values(&context);
            let root = cx.mount_child(Box::new(ScreenHost::new()));
            let source = cx.tree.add_child(
                root,
                Box::new(TimedProducer::new(snapshot.clone(), model.clone())),
                None,
                0,
            );
            let target = cx.tree.add_child(
                root,
                Box::new(
                    Viewport3D::new(
                        context,
                        Viewport3DViewRevision::new(1),
                        view_at(0.0),
                        snapshot,
                        model,
                        SampleCount3D::ONE,
                    )
                    .unwrap(),
                ),
                None,
                0,
            );
            let source_id = cx.tree.node(source).unwrap().id();
            let target_id = cx.tree.node(target).unwrap().id();
            assert_ne!(source_id, target_id);
            let link = cx
                .app
                .admit_flat_viewport3d_link(cx.tree, source_id, target_id)
                .expect("separate ordinary mounted producer and flat viewport");
            cx.tree
                .node_mut(source)
                .unwrap()
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<TimedProducer>()
                .unwrap()
                .link = Some(link.clone());
            self.source = Some(source_id);
            self.target = Some(target_id);
            self.link = Some(link);
            &mut self.stub
        }

        fn take_rebuild_flag(&mut self) -> bool {
            false
        }

        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            assert!(
                self.started.elapsed() < Duration::from_secs(20),
                "native flat transfer proof timed out"
            );
            let Some(link) = self.link.as_ref() else {
                return;
            };
            let Some(result) = cx
                .app
                .flat_viewport3d_link_result(link)
                .expect("exact original application link identity")
            else {
                return;
            };
            if matches!(
                &result.stage,
                FlatViewport3DTransferStage::AcceptedForCutoff
            ) {
                return;
            }
            let expected_stage = FlatViewport3DTransferStage::Settled(if self.stage == 0 {
                FlatViewport3DTransferOutcome::Transferred
            } else {
                FlatViewport3DTransferOutcome::Unchanged
            });
            assert_eq!(result.stage, expected_stage);
            assert_eq!(result.link, *link);
            assert_eq!(result.source, self.source.unwrap());
            assert_eq!(result.target, self.target.unwrap());

            // The scoped tree borrows end before application receipt consumption.
            let (evidence, snapshot, model, surface, metrics, status) = {
                let tree = cx.tree.as_deref().expect("ordinary Screen tree boundary");
                let producer = tree
                    .node(tree.find_by_id(self.source.unwrap()).unwrap())
                    .unwrap()
                    .widget()
                    .as_any()
                    .downcast_ref::<TimedProducer>()
                    .unwrap();
                let evidence = producer.publication.clone().unwrap();
                let viewport = tree
                    .node(tree.find_by_id(self.target.unwrap()).unwrap())
                    .unwrap()
                    .widget()
                    .as_any()
                    .downcast_ref::<Viewport3D>()
                    .unwrap();
                let prepared = viewport
                    .target_statuses()
                    .next()
                    .expect("ordinary target preparation");
                (
                    evidence,
                    producer.snapshot.clone(),
                    producer.model.clone(),
                    prepared.surface_id(),
                    prepared.metrics().expect("prepared final target metrics"),
                    prepared.status().clone(),
                )
            };
            assert_eq!(result.correlation, evidence.correlation);
            assert_eq!(result.view_revision, evidence.revision);
            assert_eq!(result.cutoff_frame, metrics.app_frame_attempt);
            let cutoff = result
                .cutoff_frame
                .expect("canonical cutoff carries frame identity");
            assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
            let requested = status.requested().unwrap();
            let committed = status.committed().unwrap();
            assert_eq!(requested.view_revision(), evidence.revision);
            assert_eq!(committed.view_revision(), evidence.revision);
            assert_eq!(requested.view(), evidence.view);
            assert_eq!(committed.view(), evidence.view);
            assert!(committed.snapshot().same_identity(&snapshot));
            assert!(committed.model().same_identity(&model));
            assert_eq!(committed.sample_count(), SampleCount3D::ONE);
            assert_eq!(requested.request_serial(), committed.request_serial());
            if self.stage == 0 {
                let realized = self
                    .observer
                    .latest()
                    .expect("canonical App completed the frame");
                assert_eq!(realized.attempt_id(), cutoff);
                let output = realized
                    .surface(surface)
                    .expect("same realized target surface");
                assert!(evidence.dt > 0.0);
                assert_ne!(evidence.view, view_at(0.0));
                assert!(
                    output.output_produced() && output.submitted() && output.presented_called()
                );
            } else {
                assert_eq!(Some(committed.request_serial()), self.first_serial);
            }

            let direct = {
                let tree = cx.tree.as_deref_mut().unwrap();
                tree.node_mut(tree.find_by_id(self.target.unwrap()).unwrap())
                    .unwrap()
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<Viewport3D>()
                    .unwrap()
                    .set_request(
                        evidence.revision,
                        evidence.view,
                        snapshot.clone(),
                        model.clone(),
                        SampleCount3D::ONE,
                    )
            };
            assert_eq!(direct, Err(Viewport3DError::LinkedWriterExclusive));
            let taken = cx
                .app
                .take_flat_viewport3d_link_result(link)
                .unwrap()
                .expect("terminal receipt is explicitly consumable");
            assert_eq!(taken.link, result.link);
            assert_eq!(taken.correlation, result.correlation);
            assert_eq!(taken.stage, result.stage);
            assert_eq!(taken.cutoff_frame, result.cutoff_frame);
            assert!(cx.app.flat_viewport3d_link_result(link).unwrap().is_none());
            println!(
                "flat_viewport3d_transfer_native: correlation={} dt={} cutoff_attempt={} prepared_attempt={} exact_view=true request_serial={:?} stage={:?}",
                evidence.correlation,
                evidence.dt,
                cutoff.sequence(),
                metrics.app_frame_attempt.unwrap().sequence(),
                committed.request_serial(),
                taken.stage,
            );

            if self.stage == 0 {
                self.first_serial = Some(committed.request_serial());
                self.stage = 1;
                let tree = cx.tree.as_deref_mut().unwrap();
                tree.node_mut(tree.find_by_id(self.source.unwrap()).unwrap())
                    .unwrap()
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<TimedProducer>()
                    .unwrap()
                    .next_publication = Some(false);
                return;
            }
            cx.app.retire_flat_viewport3d_link(link).unwrap();
            let tree = cx.tree.as_deref_mut().unwrap();
            tree.node_mut(tree.find_by_id(self.target.unwrap()).unwrap())
                .unwrap()
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<Viewport3D>()
                .unwrap()
                .set_request(
                    evidence.revision,
                    evidence.view,
                    snapshot,
                    model,
                    SampleCount3D::ONE,
                )
                .expect("direct requests resume after lawful retirement");
            tree.node_mut(tree.find_by_id(self.source.unwrap()).unwrap())
                .unwrap()
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<TimedProducer>()
                .unwrap()
                .finished = true;
            *self.done.lock().unwrap() = true;
            cx.request_exit();
        }
    }

    pub(super) fn run() {
        let (config, observer) = AppConfig::new("Halcyon public flat viewport transfer proof")
            .with_size(320.0, 180.0)
            .with_flat_viewport3d_transfer_capacity(NonZeroUsize::new(1).unwrap())
            .with_frame_realization_observer();
        let done = Arc::new(Mutex::new(false));
        let screen = TransferScreen {
            stub: ScreenHost::new(),
            link: None,
            source: None,
            target: None,
            observer,
            first_serial: None,
            stage: 0,
            started: Instant::now(),
            done: done.clone(),
        };
        App::new(config).run(screen);
        assert!(
            *done.lock().unwrap(),
            "public G1 proof exited before both stages"
        );
        println!(
            "flat_viewport3d_transfer_native: PASS (canonical mounted transfer and presentation calls; no GPU/display completion claim)"
        );
    }
}

#[cfg(feature = "test-harness")]
mod pbr_object_reveal_native {
    use super::*;
    use halcyon::three_d::*;
    use halcyon::utils::id::TreeWidgetId;
    use halcyon::widgets::screen_host::ScreenHost;
    use std::sync::atomic::{AtomicBool, Ordering};

    type Completion = Result<PbrObjectRevealExtentCpuResult3D, PbrObjectRevealExtentCpuError3D>;

    pub(super) fn source(
        context: &Context3D,
        samples: SampleCount3D,
        precision: bool,
    ) -> PbrScene3D {
        let identity = PbrOriginalIdentity3D {
            source: 372,
            revision: 1,
            node: 0,
            mesh: 0,
            primitive: 0,
        };
        let positions = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [-1.0, 1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        let uv = [
            [0.0, 0.0],
            [1.0, 0.0],
            [0.0, 1.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
        ];
        let geometry = context
            .admit_pbr_geometry(
                PbrGeometryDescriptor3D {
                    original: PbrOriginalGeometry3D {
                        identity,
                        vertices: (0..6)
                            .map(|i| PbrOriginalVertex3D {
                                position: positions[i],
                                normal: [0.0, 0.0, 1.0],
                                uv0: Some(uv[i]),
                                color: Some([255; 4]),
                            })
                            .collect(),
                        indices: GeometryIndices::Unindexed,
                        signed_tangents: Arc::from([[1.0, 0.0, 0.0, 1.0]; 6]),
                        tangent_provenance: PbrTangentProvenance3D::Authored,
                    },
                    render: (0..6)
                        .map(|i| PbrVertex3D {
                            position: positions[i],
                            normal: [0.0, 0.0, 1.0],
                            uv0: uv[i],
                            color: [255; 4],
                            tangent: [1.0, 0.0, 0.0, 1.0],
                        })
                        .collect(),
                    render_indices: GeometryIndices::Unindexed,
                    render_vertex_sources: Arc::from([0, 1, 2, 3, 4, 5]),
                    triangles: (0..2)
                        .map(|triangle| PbrTriangleCertificate3D {
                            original: identity,
                            triangle,
                            certificate_identity: 37200 + triangle as u64,
                            certificate_revision: 1,
                            frame: PbrFrameCertificate3D::NormalOnly {
                                separator: [0.0, 0.0, 1.0],
                                lower: [0.5; 3],
                            },
                        })
                        .collect(),
                    mapped: false,
                    maximum_certificate_bytes: 65536,
                    maximum_verification_items: 1000,
                },
                None,
            )
            .unwrap();
        let mut material = PbrMaterialDescriptor3D::new(372, 1, PbrSamplingFamily3D::V1);
        material.emissive_rgb = if precision {
            [2f32.powi(-26); 3]
        } else {
            [1.0, 0.25, 0.05]
        };
        let material = context.admit_pbr_material(material, None).unwrap();
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
            .unwrap();
        let model = context
            .admit_pbr_model(
                &snapshot,
                ModelRevision::new(1),
                vec![
                    ModelPlacement3::try_new(
                        WorldPoint3::origin(),
                        Orientation3::identity(),
                        PositiveScale3::one(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
        context
            .admit_pbr_scene(PbrSceneDescriptor3D {
                snapshot,
                model,
                hdr: PbrHdrSource3D {
                    identity: 372,
                    revision: 1,
                    exposure_ev: if precision { 16.0 } else { 0.0 },
                    maximum_rgb: [1.0; 3],
                },
                samples,
                direct: None,
                indirect: None,
            })
            .unwrap()
    }

    struct RevealScreen {
        stub: ScreenHost,
        context: Option<Arc<Context3D>>,
        source: Option<PbrScene3D>,
        target: Option<TreeWidgetId>,
        pending: Option<PbrObjectRevealExtentPending3D>,
        completion: Arc<Mutex<Option<Completion>>>,
        cancel: Arc<AtomicBool>,
        worker: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
        waker: Option<UiWakeHandle>,
        observer: AppFrameObserver,
        stage: u8,
        hidden: bool,
        partial: bool,
        started: Instant,
        done: Arc<AtomicBool>,
        samples: SampleCount3D,
        precision: bool,
    }

    impl Screen for RevealScreen {
        fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
            self.waker = cx.app.ui_waker();
        }
        fn on_leave(&mut self, _: &mut ScreenContext<'_>) {
            self.cancel.store(true, Ordering::Release);
        }
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let context =
                Arc::new(Context3D::new(cx.app.gpu_resources().unwrap().clone()).unwrap());
            let source = source(&context, self.samples, self.precision);
            assert_eq!(
                cx.app.gpu_resources().unwrap().device.features(),
                wgpu::Features::empty()
            );
            let view = ViewDescriptor3::new(
                RigidPose3::new(
                    WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                    Orientation3::identity(),
                ),
                OrthographicProjection::try_new(2.0, 0.1, 10.0)
                    .unwrap()
                    .into(),
            );
            let node = cx.mount_child(Box::new(
                Viewport3D::new_pbr(
                    context.clone(),
                    source.clone(),
                    Viewport3DViewRevision::new(1),
                    view,
                )
                .unwrap(),
            ));
            self.target = Some(cx.tree.node(node).unwrap().id());
            self.context = Some(context);
            self.source = Some(source);
            &mut self.stub
        }
        fn take_rebuild_flag(&mut self) -> bool {
            false
        }
        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            assert!(
                self.started.elapsed() < Duration::from_secs(20),
                "native Object Reveal deadline"
            );
            let Some(id) = self.target else {
                return;
            };
            let context = self.context.as_ref().unwrap();
            let source = self.source.as_ref().unwrap();
            if self.stage == 1 {
                let result = self.completion.lock().unwrap().take();
                if let Some(result) = result {
                    let extent = context
                        .finish_pbr_object_reveal_extent(
                            self.pending.take().unwrap(),
                            result.unwrap(),
                            source,
                        )
                        .unwrap();
                    let tree = cx.tree.as_deref_mut().unwrap();
                    tree.node_mut(tree.find_by_id(id).unwrap())
                        .unwrap()
                        .widget_mut()
                        .as_any_mut()
                        .downcast_mut::<Viewport3D>()
                        .unwrap()
                        .request_pbr_object_reveal(PbrObjectRevealRequest3D {
                            source: source.clone(),
                            request_revision: 1,
                            duration: Duration::from_millis(160),
                            enabled: true,
                            extent: Some(extent),
                        })
                        .unwrap();
                    self.stage = 2;
                    return;
                }
            }
            let tree = cx.tree.as_deref().unwrap();
            let viewport = tree
                .node(tree.find_by_id(id).unwrap())
                .unwrap()
                .widget()
                .as_any()
                .downcast_ref::<Viewport3D>()
                .unwrap();
            let Some(target) = viewport.current_prepared_target() else {
                return;
            };
            let Some(metrics) = target.metrics() else {
                return;
            };
            let Some(tuple) = target.status().pbr_committed() else {
                return;
            };
            let Some(receipt) = self.observer.latest() else {
                return;
            };
            if Some(receipt.attempt_id()) != metrics.app_frame_attempt {
                return;
            }
            let output = receipt.surface(target.surface_id()).unwrap();
            assert!(output.output_produced() && output.submitted() && output.presented_called());
            assert!(tuple.source().same_identity(source));
            assert_eq!(tuple.target_key().sample_count(), self.samples);
            if self.stage == 0 {
                if self.precision {
                    let extent = tuple.target_key().physical_render_extent();
                    let pixel = viewport
                        .pbr_front_pixel_for_test_harness(
                            target.surface_id(),
                            [extent.width / 2, extent.height / 2],
                        )
                        .unwrap();
                    assert!(pixel.source.same_identity(source));
                    assert_eq!(pixel.rgba[3], 1.0);
                    for c in &pixel.rgba[..3] {
                        assert!(*c > 0.0 && (f64::from(*c) - 1.0 / 1025.0).abs() <= 2f64.powi(-20));
                    }
                    println!(
                        "r027_mrt4_native N{} initial: attempt={} source=2^-26 EV16 final={:?}; Features::empty; output/submitted/presented_called=true",
                        self.samples.value(),
                        receipt.attempt_id().sequence(),
                        pixel.rgba
                    );
                }
                assert!(
                    !viewport.needs_update(),
                    "ordinary baseline has no effect demand"
                );
                let (job, pending) = context
                    .begin_pbr_object_reveal_extent(source, 1 << 20)
                    .unwrap();
                self.pending = Some(pending);
                let completion = self.completion.clone();
                let cancel = self.cancel.clone();
                let waker = self.waker.clone().expect("canonical native wake");
                *self.worker.lock().unwrap() = Some(std::thread::spawn(move || {
                    let result = job.verify(cancel);
                    *completion.lock().unwrap() = Some(result);
                    waker
                        .wake()
                        .expect("publish-before-wake while native owner is live");
                }));
                self.stage = 1;
                return;
            }
            if self.stage != 2 {
                return;
            }
            let extent = tuple.target_key().physical_render_extent();
            let bottom = viewport
                .pbr_front_pixel_for_test_harness(
                    target.surface_id(),
                    [extent.width / 2, extent.height * 3 / 4],
                )
                .unwrap();
            let top = viewport
                .pbr_front_pixel_for_test_harness(
                    target.surface_id(),
                    [extent.width / 2, extent.height / 4],
                )
                .unwrap();
            assert!(bottom.source.same_identity(source) && top.source.same_identity(source));
            if bottom.rgba[3] == 0.0 && top.rgba[3] == 0.0 {
                if !self.hidden {
                    println!(
                        "pbr_native N{} p0: attempt={} image={} hidden={:?}/{:?}",
                        self.samples.value(),
                        receipt.attempt_id().sequence(),
                        bottom.image_id,
                        bottom.rgba,
                        top.rgba
                    );
                }
                self.hidden = true;
            } else if self.hidden && bottom.rgba[3] == 1.0 && top.rgba[3] == 0.0 {
                if !self.partial {
                    println!(
                        "pbr_native N{} partial: attempt={} image={} retained={:?} hidden={:?}",
                        self.samples.value(),
                        receipt.attempt_id().sequence(),
                        bottom.image_id,
                        bottom.rgba,
                        top.rgba
                    );
                }
                self.partial = true;
            } else if self.hidden
                && !viewport.needs_update()
                && bottom.rgba[3] == 1.0
                && top.rgba[3] == 1.0
            {
                assert!(
                    self.partial,
                    "actual native progression must include a partial front"
                );
                println!(
                    "pbr_native N{} terminal: attempt={} image={} complete={:?}/{:?} effect_needs_update=false; same source, target and ordinary submit/present calls",
                    self.samples.value(),
                    receipt.attempt_id().sequence(),
                    bottom.image_id,
                    bottom.rgba,
                    top.rgba
                );
                self.done.store(true, Ordering::Release);
                self.stage = 3;
                cx.request_exit();
            }
        }
    }

    pub(super) fn run_idle_wake() {
        idle_wake::run();
    }

    mod idle_wake {
        use super::*;
        use std::sync::{atomic::AtomicU8, mpsc};

        fn observe_quiet(
            receiver: &mpsc::Receiver<u8>,
            observer: &AppFrameObserver,
            inputs: &std::sync::atomic::AtomicU64,
        ) -> Result<(), String> {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let before = observer.latest().unwrap().attempt_id();
                let input = inputs.load(Ordering::Acquire);
                // Observe real startup/retirement completion. This worker
                // sends no event and provides no cadence while observing.
                match receiver.recv_timeout(Duration::from_millis(80)) {
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    result => return Err(format!("unexpected quiet-window message {result:?}")),
                }
                if inputs.load(Ordering::Acquire) != input {
                    return Err("external input invalidated quiet observation".into());
                }
                let after = observer.latest().unwrap().attempt_id();
                if after == before {
                    println!(
                        "native_idle_wake:80ms genuine quiet at attempt{}; no observation wake or input",
                        after.sequence()
                    );
                    return Ok(());
                }
                println!(
                    "native_idle_wake: prior in-flight/Timeline work completed attempts{}->{}; not yet quiet",
                    before.sequence(),
                    after.sequence()
                );
                if Instant::now() >= deadline {
                    return Err("no quiet native interval after bounded real Timeline/retirement completion".into());
                }
            }
        }

        struct WakeScreen {
            stub: ScreenHost,
            observer: AppFrameObserver,
            target: Option<TreeWidgetId>,
            sources: Option<[PbrScene3D; 2]>,
            view: Option<ViewDescriptor3>,
            desired: Arc<AtomicU8>,
            progress: mpsc::Sender<u8>,
            receive: Option<mpsc::Receiver<u8>>,
            worker: Arc<Mutex<Option<std::thread::JoinHandle<Result<(), String>>>>>,
            stage: u8,
            switch_attempt: u64,
            switched: bool,
            done: Arc<AtomicBool>,
            inputs: Arc<std::sync::atomic::AtomicU64>,
            input_baseline: u64,
            observed_front_attempt: Arc<std::sync::atomic::AtomicU64>,
        }
        impl Screen for WakeScreen {
            fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
                let waker = cx.app.ui_waker().expect("actual App native proxy");
                let receiver = self.receive.take().unwrap();
                let desired = self.desired.clone();
                let observer = self.observer.clone();
                let inputs = self.inputs.clone();
                let observed_front_attempt = self.observed_front_attempt.clone();
                *self.worker.lock().unwrap() = Some(std::thread::spawn(move || {
                    let proof = (|| -> Result<(), String> {
                        if receiver
                            .recv_timeout(Duration::from_secs(10))
                            .map_err(|e| format!("initial N4: {e}"))?
                            != 0
                        {
                            return Err("wrong initial stage".into());
                        }
                        observe_quiet(&receiver, &observer, &inputs)?;
                        let before = observer.latest().unwrap().attempt_id();
                        desired.store(1, Ordering::Release);
                        // Deliver one source intent. No test wake follows its
                        // settlement until a genuinely new completed AppFrame
                        // has independently been observed.
                        waker.wake().map_err(|e| e.to_string())?;
                        if receiver
                            .recv_timeout(Duration::from_secs(10))
                            .map_err(|e| format!("UI source admission: {e}"))?
                            != 2
                        {
                            return Err("wrong source admission stage".into());
                        }
                        // Passive, bounded test observation only: no event,
                        // timer, redraw or polling is delivered to the UI.
                        let reported = match receiver.recv_timeout(Duration::from_secs(1)) {
                            Ok(1) => true,
                            Err(mpsc::RecvTimeoutError::Timeout) => false,
                            result => return Err(format!("unexpected completion {result:?}")),
                        };
                        if !reported {
                            let after = observer.latest().ok_or("missing completed AppFrame")?;
                            if after.attempt_id().sequence() <= before.sequence()
                                || !after.surfaces().iter().any(|surface| {
                                    surface.output_produced()
                                        && surface.submitted()
                                        && surface.presented_called()
                                })
                            {
                                return Err(format!(
                                    "no genuine successor output after settlement: before={before:?}, after={after:?}"
                                ));
                            }
                            println!(
                                "native_idle_wake: independent observer advanced {before:?}->{:?} before any post-settlement test wake; submitted/presented_called=true",
                                after.attempt_id()
                            );
                            observed_front_attempt
                                .store(after.attempt_id().sequence(), Ordering::Release);
                            // Observation only, after the liveness fact is
                            // already proven. UI must validate the SAME prior
                            // registered front, not a frame rescued by this wake.
                            waker.wake().map_err(|e| e.to_string())?;
                            if receiver
                                .recv_timeout(Duration::from_secs(10))
                                .map_err(|e| format!("registered N1 observation: {e}"))?
                                != 1
                            {
                                return Err("wrong completion stage".into());
                            }
                        }
                        observe_quiet(&receiver, &observer, &inputs)?;
                        Ok(())
                    })();
                    // Watchdog wake only exits a FAILED test; its delayed frame
                    // can never count as a successful render/liveness receipt.
                    desired.store(if proof.is_ok() { 2 } else { 3 }, Ordering::Release);
                    if let Err(e) = waker.wake() {
                        return Err(format!("test shutdown wake: {e}"));
                    }
                    proof
                }));
            }
            fn take_rebuild_flag(&mut self) -> bool {
                false
            }
            fn handle_input(
                &mut self,
                _: &halcyon::core::event::FusedInputEvent,
                _: &mut ScreenContext<'_>,
            ) -> bool {
                self.inputs.fetch_add(1, Ordering::AcqRel);
                false
            }
            fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
                let context =
                    Arc::new(Context3D::new(cx.app.gpu_resources().unwrap().clone()).unwrap());
                assert!(cx.app.gpu_resources().unwrap().device.features().is_empty());
                // Original HAL-owned certified fixture, NOT private baseball_bat.
                let a = source(&context, SampleCount3D::FOUR, false);
                let b = context
                    .admit_pbr_scene(PbrSceneDescriptor3D {
                        snapshot: a.snapshot(),
                        model: a.model(),
                        hdr: PbrHdrSource3D {
                            revision: 2,
                            ..a.hdr()
                        },
                        samples: SampleCount3D::ONE,
                        direct: None,
                        indirect: None,
                    })
                    .unwrap();
                let view = ViewDescriptor3::new(
                    RigidPose3::new(
                        WorldPoint3::try_new(0., 0., 3.).unwrap(),
                        Orientation3::identity(),
                    ),
                    OrthographicProjection::try_new(2., 0.1, 10.)
                        .unwrap()
                        .into(),
                );
                let node = cx.mount_child(Box::new(
                    Viewport3D::new_pbr(context, a.clone(), Viewport3DViewRevision::new(4), view)
                        .unwrap(),
                ));
                self.target = Some(cx.tree.node(node).unwrap().id());
                self.sources = Some([a, b]);
                self.view = Some(view);
                &mut self.stub
            }
            fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
                if self.done.load(Ordering::Acquire) {
                    return;
                }
                let desired = self.desired.load(Ordering::Acquire);
                if desired >= 2 {
                    self.done.store(desired == 2, Ordering::Release);
                    cx.request_exit();
                    return;
                }
                let Some(tree) = cx.tree.as_deref_mut() else {
                    return;
                };
                let Some(node) = self.target.and_then(|id| tree.find_by_id(id)) else {
                    return;
                };
                let viewport = tree
                    .node_mut(node)
                    .unwrap()
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<Viewport3D>()
                    .unwrap();
                assert!(
                    !viewport.needs_update() && !viewport.needs_animation(),
                    "quiet PBR has no animation/update escape from idle throttle"
                );
                if desired == 1 && !self.switched {
                    self.switch_attempt = self.observer.latest().unwrap().attempt_id().sequence();
                    assert_eq!(
                        self.inputs.load(Ordering::Acquire),
                        self.input_baseline,
                        "no OS input may deliver the changed source"
                    );
                    let changed = viewport
                        .request_pbr_scene(
                            self.sources.as_ref().unwrap()[1].clone(),
                            Viewport3DViewRevision::new(5),
                            self.view.unwrap(),
                        )
                        .unwrap();
                    assert!(changed);
                    assert!(!viewport.needs_update());
                    self.switched = true;
                    tree.mark_paint_dirty(node);
                    assert!(tree.node(node).unwrap().dirty_paint);
                    assert!(!tree.node(node).unwrap().dirty_layout);
                    println!(
                        "native_idle_wake: accepted original source N4->N1, view5, changed=true, needs_update=false, paint-only dirty, after attempt{}",
                        self.switch_attempt
                    );
                    self.progress.send(2).unwrap();
                    return;
                }
                let Some(target) = viewport.current_prepared_target() else {
                    return;
                };
                let Some(metrics) = target.metrics() else {
                    return;
                };
                let Some(receipt) = self
                    .observer
                    .latest()
                    .filter(|r| Some(r.attempt_id()) == metrics.app_frame_attempt)
                else {
                    return;
                };
                let Some(tuple) = target.status().pbr_committed() else {
                    return;
                };
                let expected = if self.switched { 1 } else { 0 };
                if !tuple
                    .source()
                    .same_identity(&self.sources.as_ref().unwrap()[expected])
                {
                    return;
                }
                let output = receipt.surface(target.surface_id()).unwrap();
                assert!(
                    output.output_produced() && output.submitted() && output.presented_called()
                );
                let extent = tuple.target_key().physical_render_extent();
                let pixel = viewport
                    .pbr_front_pixel_for_test_harness(
                        target.surface_id(),
                        [extent.width / 2, extent.height / 2],
                    )
                    .unwrap();
                assert!(pixel.source.same_identity(&tuple.source()));
                assert_eq!(pixel.rgba[3], 1.);
                assert_eq!(
                    tuple.target_key().sample_count(),
                    if expected == 0 {
                        SampleCount3D::FOUR
                    } else {
                        SampleCount3D::ONE
                    }
                );
                if self.stage == 0 && expected == 0 {
                    self.input_baseline = self.inputs.load(Ordering::Acquire);
                    self.stage = 1;
                    println!(
                        "native_idle_wake: original registered N4 matched at attempt{}",
                        receipt.attempt_id().sequence()
                    );
                    self.progress.send(0).unwrap();
                } else if self.stage == 1 && expected == 1 {
                    assert!(receipt.attempt_id().sequence() > self.switch_attempt);
                    let independently_observed =
                        self.observed_front_attempt.load(Ordering::Acquire);
                    if independently_observed != 0 {
                        assert_eq!(
                            receipt.attempt_id().sequence(),
                            independently_observed,
                            "the observation wake cannot substitute a newer rescued frame"
                        );
                    }
                    assert_eq!(tuple.view_revision(), Viewport3DViewRevision::new(5));
                    assert_eq!(self.inputs.load(Ordering::Acquire), self.input_baseline);
                    self.stage = 2;
                    println!(
                        "native_idle_wake: registered source-matched N1, request{}, attempt{}, F={:?}, no OS input",
                        pixel.request_serial.value(),
                        receipt.attempt_id().sequence(),
                        pixel.rgba
                    );
                    self.progress.send(1).unwrap();
                }
            }
        }
        pub(super) fn run() {
            let _ = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .try_init();
            let (config, observer) = AppConfig::new("Native wake-only PBR source regression")
                .with_size(384., 384.)
                .with_frame_realization_observer();
            let (progress, receive) = mpsc::channel();
            let worker = Arc::new(Mutex::new(None));
            let done = Arc::new(AtomicBool::new(false));
            App::new(config).run(WakeScreen {
                stub: ScreenHost::new(),
                observer,
                target: None,
                sources: None,
                view: None,
                desired: Arc::new(AtomicU8::new(0)),
                progress,
                receive: Some(receive),
                worker: worker.clone(),
                stage: 0,
                switch_attempt: 0,
                switched: false,
                done: done.clone(),
                inputs: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                input_baseline: 0,
                observed_front_attempt: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            });
            worker
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .join()
                .unwrap()
                .unwrap();
            assert!(done.load(Ordering::Acquire));
            println!(
                "native_idle_wake PASS: actual App::run/UiWakeHandle, one source-intent wake, no post-settlement rescue, qualified HAL PBR N4->N1 view5 registered current front, no OS input/resize/debugger, settled quiet. The external consumer's baseball_bat and physical display remain separate."
            );
        }
    }

    pub(super) fn run(samples: SampleCount3D, precision: bool) {
        let (config, observer) = AppConfig::new("Halcyon source-qualified PBR Object Reveal")
            .with_size(384.0, 384.0)
            .with_frame_realization_observer();
        let done = Arc::new(AtomicBool::new(false));
        let worker = Arc::new(Mutex::new(None));
        let screen = RevealScreen {
            stub: ScreenHost::new(),
            context: None,
            source: None,
            target: None,
            pending: None,
            completion: Arc::new(Mutex::new(None)),
            cancel: Arc::new(AtomicBool::new(false)),
            worker: worker.clone(),
            waker: None,
            observer,
            stage: 0,
            hidden: false,
            partial: false,
            started: Instant::now(),
            done: done.clone(),
            samples,
            precision,
        };
        App::new(config).run(screen);
        if let Some(worker) = worker.lock().unwrap().take() {
            worker.join().unwrap();
        }
        assert!(done.load(Ordering::Acquire));
        println!(
            "pbr_native: PASS canonical App::run + caller CPU extent/wake + actual N{} FP16 fronts + submit/present; low-radiance oracle={precision}; physical display/input/JC acceptance not observed",
            samples.value()
        );
    }
}

#[cfg(feature = "test-harness")]
mod loading_presence_native {
    use super::*;
    use halcyon::core::event::{
        Event, EventPhase, KeyCode, KeyEvent, KeyboardAuthorityMode, PhysicalKeyCode,
    };
    use halcyon::core::widget::EventContext;
    use halcyon::core::widget_tree::WidgetTree;
    use halcyon::prelude::Label;
    use halcyon::three_d::{PbrScene3D, SampleCount3D, Viewport3D};
    use halcyon::widgets::slider::Slider;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn transition_controls(tree: &WidgetTree, viewport: NodeId) -> (NodeId, &Label) {
        let frame = tree.parent(viewport).unwrap();
        let children = tree.children(tree.parent(frame).unwrap());
        let slider = children
            .iter()
            .copied()
            .find(|&id| tree.node(id).unwrap().widget().as_any().is::<Slider>())
            .unwrap();
        let legend = children
            .iter()
            .filter_map(|&id| {
                tree.node(id)
                    .unwrap()
                    .widget()
                    .as_any()
                    .downcast_ref::<Label>()
            })
            .find(|label| label.text.starts_with("Ghost preview → Full PBR"))
            .unwrap();
        (slider, legend)
    }

    fn simulate_slider_key(tree: &mut WidgetTree, id: NodeId, key_code: KeyCode, expected: f32) {
        let node = tree.node_mut(id).unwrap();
        let mut event = EventContext::new(
            node.id(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false,
            KeyboardAuthorityMode::Capture,
        );
        let slider = node
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Slider>()
            .unwrap();
        // Exercise the real target handler and callback with a synthetic key.
        slider.on_focus(&mut event);
        assert!(slider.handle_event(
            &mut event,
            &Event::Key(KeyEvent {
                key_code,
                physical_key: PhysicalKeyCode::Unknown,
                is_down: true,
                is_repeat: false,
                modifiers: Default::default(),
                text: None,
            }),
            EventPhase::Target,
        ));
        assert_eq!(slider.value(), expected);
        slider.on_blur(&mut event);
        tree.mark_paint_dirty(id);
    }

    struct NativePresence {
        inner: loading_presence_example::PresenceScreen,
        observer: AppFrameObserver,
        stage: usize,
        partial: [bool; 3],
        a: Option<PbrScene3D>,
        started: Instant,
        done: Arc<AtomicBool>,
        samples: SampleCount3D,
    }
    impl Screen for NativePresence {
        fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
            self.inner.on_enter(cx);
        }
        fn on_leave(&mut self, cx: &mut ScreenContext<'_>) {
            self.inner.on_leave(cx);
        }
        fn take_rebuild_flag(&mut self) -> bool {
            self.inner.take_rebuild_flag()
        }
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            self.inner.build(cx)
        }
        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            assert!(
                self.started.elapsed() < Duration::from_secs(30),
                "HAL381 native proof deadline, stage {}",
                self.stage
            );
            self.inner.flush_pending_actions(cx);
            let Some(id) = self.inner.viewport else {
                return;
            };
            let Some(viewport) = cx
                .tree
                .as_deref()
                .and_then(|t| t.node(id))
                .and_then(|n| n.widget().as_any().downcast_ref::<Viewport3D>())
            else {
                return;
            };
            assert!(
                viewport.loading_presence_error().is_none(),
                "{:?}",
                viewport.loading_presence_error()
            );
            let Some(target) = viewport.current_prepared_target() else {
                return;
            };
            let Some(metrics) = target.metrics() else {
                return;
            };
            let Some(receipt) = self
                .observer
                .latest()
                .filter(|r| Some(r.attempt_id()) == metrics.app_frame_attempt)
            else {
                return;
            };
            let output = receipt
                .surface(target.surface_id())
                .expect("actual native surface receipt");
            assert!(output.output_produced() && output.submitted() && output.presented_called());
            let Some((weight, terminal)) =
                viewport.loading_presence_progress_for_test_harness(target.surface_id())
            else {
                return;
            };
            if self.stage % 2 == 0 {
                if target.status().pbr_requested().is_some() {
                    return;
                }
                assert_eq!(weight, 0.0);
                assert!(!terminal);
                let tree = cx.tree.as_deref().unwrap();
                let (slider_id, legend) = transition_controls(tree, id);
                let slider_rect = tree.get_absolute_paint_rect(slider_id).unwrap();
                let viewport_rect = tree.get_absolute_paint_rect(id).unwrap();
                assert!(slider_rect.width > 0.0 && slider_rect.height > 0.0);
                assert!(viewport_rect.width > 0.0 && viewport_rect.height > 0.0);
                assert!(slider_rect.y + slider_rect.height <= viewport_rect.y);
                assert!(slider_rect.x + slider_rect.width <= viewport_rect.x + viewport_rect.width);
                let millis = if self.stage == 0 { 160 } else { 3_000 };
                assert!(legend.text.contains(&format!("duration {millis} ms")));
                assert!(!legend.text.contains("applies to next selection"));
                let slider = tree
                    .node(slider_id)
                    .unwrap()
                    .widget()
                    .as_any()
                    .downcast_ref::<Slider>()
                    .unwrap();
                assert_eq!(slider.value(), millis as f32);
                println!(
                    "hal381_native N{} stage={} spectral presence: attempt={} duration_ms={millis} slider={slider_rect:?} viewport={viewport_rect:?} source_request=PENDING_FULL output/submitted/presented_called=true",
                    self.samples.value(),
                    self.stage,
                    receipt.attempt_id().sequence()
                );
                if self.stage == 0 {
                    simulate_slider_key(
                        cx.tree.as_deref_mut().unwrap(),
                        slider_id,
                        KeyCode::Right,
                        200.0,
                    );
                }
                self.inner.complete = true;
                self.stage += 1;
                return;
            }
            let Some(tuple) = target.status().pbr_committed() else {
                return;
            };
            assert_eq!(tuple.target_key().sample_count(), self.samples);
            let iteration = self.stage / 2;
            let mut adjust_next = false;
            if weight > 0.0 && weight < 1.0 {
                if !self.partial[iteration] {
                    println!(
                        "hal381_native N{} materialization {}: weight={weight} attempt={}",
                        self.samples.value(),
                        iteration,
                        receipt.attempt_id().sequence()
                    );
                    adjust_next = iteration == 0;
                }
                self.partial[iteration] = true;
            }
            if !terminal || viewport.needs_update() {
                if adjust_next {
                    let tree = cx.tree.as_deref_mut().unwrap();
                    let (slider, legend) = transition_controls(tree, id);
                    assert!(legend.text.contains("duration 200 ms"));
                    simulate_slider_key(tree, slider, KeyCode::End, 3_000.0);
                }
                return;
            }
            let (_, legend) = transition_controls(cx.tree.as_deref().unwrap(), id);
            assert!(legend.text.contains("duration 3000 ms"));
            assert_eq!(
                legend.text.contains("applies to next selection"),
                iteration == 0
            );
            assert!(
                self.partial[iteration],
                "actual native frames must include an intermediate materialization output"
            );
            let extent = tuple.target_key().physical_render_extent();
            let hole = viewport
                .pbr_front_pixel_for_test_harness(
                    target.surface_id(),
                    [extent.width / 2, extent.height / 2],
                )
                .unwrap();
            let pixel = viewport
                .pbr_front_pixel_for_test_harness(
                    target.surface_id(),
                    [extent.width / 2 + extent.height / 5, extent.height / 2],
                )
                .unwrap();
            assert!(hole.source.same_identity(&tuple.source()));
            assert_eq!(hole.image_id, pixel.image_id);
            assert_eq!(hole.rgba, [0.0; 4], "the original solid has a through-hole");
            assert!(pixel.source.same_identity(&tuple.source()));
            assert_eq!(pixel.rgba[3], 1.0);
            assert!(pixel.rgba[..3].iter().all(|v| v.is_finite() && *v >= 0.0));
            if iteration == 0 {
                self.a = Some(tuple.source().clone());
            } else if iteration == 1 {
                assert!(!self.a.as_ref().unwrap().same_identity(&tuple.source()));
            }
            if iteration == 2 {
                assert!(
                    self.a.as_ref().unwrap().same_identity(&tuple.source()),
                    "A/B/A reuses the actual immutable A source"
                );
            }
            println!(
                "hal381_native N{} terminal {}: attempt={} canonical_image={} F={:?} hole={:?}; pin/registered=true, animation_demand=false",
                self.samples.value(),
                iteration,
                receipt.attempt_id().sequence(),
                pixel.image_id,
                pixel.rgba,
                hole.rgba
            );
            if iteration == 2 {
                self.done.store(true, Ordering::Release);
                self.stage = 6;
                cx.request_exit();
            } else {
                self.inner.desired = if iteration == 0 { 1 } else { 0 };
                self.stage += 1;
            }
        }
    }
    pub(super) fn run(samples: halcyon::three_d::SampleCount3D) {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();
        let (width, height) = if samples.value() == 1 {
            (800.0, 600.0)
        } else {
            (900.0, 650.0)
        };
        let (config, observer) = AppConfig::new("HAL381 native source-qualified loading proof")
            .with_size(width, height)
            .with_frame_realization_observer();
        let done = Arc::new(AtomicBool::new(false));
        App::new(config).run(NativePresence {
            inner: loading_presence_example::PresenceScreen::with_samples(samples),
            observer,
            stage: 0,
            partial: [false; 3],
            a: None,
            started: Instant::now(),
            done: done.clone(),
            samples,
        });
        assert!(done.load(Ordering::Acquire));
        println!(
            "hal381_native N{} PASS: real App::run at {width}x{height}, nonoverlapping Slider, synthetic target keys, 160→200 ms before readiness, 3000 ms next-selection choice after readiness and explicit A/B/A reapplication, registered PBR pin, intermediate+terminal frames, no permanent demand. Physical scanout/keyboard/JC review UNOBSERVED.",
            samples.value()
        );
    }
}

#[cfg(feature = "test-harness")]
mod selection_outline_native {
    use super::*;
    use halcyon::three_d::{SampleCount3D, Viewport3D};
    use std::sync::atomic::{AtomicBool, Ordering};

    struct NativeOutline {
        inner: selection_outline_example::OutlineScreen,
        observer: AppFrameObserver,
        stage: usize,
        last_image: Option<u64>,
        candidate: Option<(u64, u64)>,
        started: Instant,
        done: Arc<AtomicBool>,
        samples: SampleCount3D,
    }
    impl Screen for NativeOutline {
        fn take_rebuild_flag(&mut self) -> bool {
            self.inner.take_rebuild_flag()
        }
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            self.inner.build(cx)
        }
        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            if self.done.load(Ordering::Acquire) {
                return;
            }
            assert!(
                self.started.elapsed() < Duration::from_secs(30),
                "HAL374 native bounded deadline stage{}",
                self.stage
            );
            self.inner.flush_pending_actions(cx);
            let Some(viewport) = cx
                .tree
                .as_deref()
                .and_then(|t| {
                    self.inner
                        .viewport
                        .and_then(|id| t.find_by_id(id))
                        .and_then(|n| t.node(n))
                })
                .and_then(|n| n.widget().as_any().downcast_ref::<Viewport3D>())
            else {
                return;
            };
            let Some(target) = viewport.current_prepared_target() else {
                return;
            };
            let Some(metrics) = target.metrics() else {
                return;
            };
            let Some(receipt) = self
                .observer
                .latest()
                .filter(|r| Some(r.attempt_id()) == metrics.app_frame_attempt)
            else {
                return;
            };
            let output = receipt
                .surface(target.surface_id())
                .expect("actual canonical native surface receipt");
            assert!(
                output.output_produced() && output.submitted() && output.presented_called(),
                "stage={} receipt={receipt:?} committed={} pbr_failure={:?} outline_error={:?}",
                self.stage,
                target.status().pbr_committed().is_some(),
                viewport.pbr_failure(target.surface_id()),
                viewport.pbr_outline_error(target.surface_id())
            );
            assert!(
                viewport.pbr_failure(target.surface_id()).is_none(),
                "{:?}",
                viewport.pbr_failure(target.surface_id())
            );
            assert!(
                viewport.pbr_outline_error(target.surface_id()).is_none(),
                "{:?}",
                viewport.pbr_outline_error(target.surface_id())
            );
            let Some(tuple) = target.status().pbr_committed() else {
                return;
            };
            assert_eq!(tuple.target_key().sample_count(), self.samples);
            let original = viewport
                .pbr_front_pixel_for_test_harness(target.surface_id(), [0, 0])
                .unwrap();
            let private = viewport
                .pbr_outline_pixel_for_test_harness(target.surface_id(), [0, 0])
                .unwrap();
            let selected = self.stage != 2;
            if selected && private.is_none() {
                return;
            }
            if !selected {
                assert!(
                    private.is_none(),
                    "None must bypass private output entirely"
                );
            }
            if viewport.needs_update() {
                return;
            }
            let image = private.as_ref().map_or(original.image_id, |p| p.image_id);
            match self.candidate {
                Some((expected, attempt))
                    if expected == image && receipt.attempt_id().sequence() > attempt => {}
                Some((expected, _)) if expected == image => return,
                _ => {
                    // Require a COMPLETE subsequent real frame while this
                    // registered generation is eligible. An old receipt from
                    // before the consumer changed selection is insufficient.
                    self.candidate = Some((image, receipt.attempt_id().sequence()));
                    cx.app
                        .ui_waker()
                        .expect("actual native loop")
                        .wake()
                        .expect("bounded native proof stimulus");
                    return;
                }
            }
            if let Some(private) = private {
                assert!(private.source.same_identity(&original.source));
                assert_eq!(private.request_serial, original.request_serial);
                assert_eq!(private.target_key, original.target_key);
                assert_ne!(
                    private.image_id, original.image_id,
                    "private output cannot masquerade as canonical F"
                );
                if let Some(previous) = self.last_image {
                    assert_ne!(
                        private.image_id, previous,
                        "retargeted occurrence must retire old output"
                    );
                }
                assert_eq!(
                    private.rgba, original.rgba,
                    "uncovered corner retains exact original F"
                );
                self.last_image = Some(private.image_id);
                // Project the exact example cube's eight original corners.
                // The leftmost A / rightmost B silhouette is outside the
                // independent foreground. Read a bounded3x3 neighborhood of
                // that TRUE source vertex, never a guessed screen rectangle.
                let extent = tuple.target_key().physical_render_extent();
                let rotation =
                    glam::Quat::from_rotation_x(0.38) * glam::Quat::from_rotation_y(-0.54);
                let scale = if self.stage == 0 { 1.1 } else { 1. };
                let x = if self.stage == 0 { -0.7 } else { 0.7 };
                let mut corners = Vec::new();
                for a in [-0.5, 0.5] {
                    for b in [-0.5, 0.5] {
                        for c in [-0.5, 0.5] {
                            let p = rotation * (glam::Vec3::new(a, b, c) * scale)
                                + glam::Vec3::new(x, 0., 0.);
                            let reciprocal =
                                1. / ((5. - p.z) * (std::f32::consts::FRAC_PI_8).tan());
                            corners.push([
                                (p.x * reciprocal * extent.height as f32 / extent.width as f32
                                    * 0.5
                                    + 0.5)
                                    * extent.width as f32,
                                (0.5 - p.y * reciprocal * 0.5) * extent.height as f32,
                            ]);
                        }
                    }
                }
                corners.sort_by(|a, b| a[0].total_cmp(&b[0]));
                let q = if self.stage == 0 {
                    corners[0]
                } else {
                    corners[7]
                };
                let sx = (q[0] + if self.stage == 0 { -1. } else { 1. }).floor() as i32;
                let sy = q[1].floor() as i32;
                let mut witness = None;
                'pixels: for dy in -1..=1 {
                    for dx in -1..=1 {
                        let p = [sx + dx, sy + dy];
                        if p[0] < 0
                            || p[1] < 0
                            || p[0] >= extent.width as i32
                            || p[1] >= extent.height as i32
                        {
                            continue;
                        }
                        let p = p.map(|v| v as u32);
                        let f = viewport
                            .pbr_front_pixel_for_test_harness(target.surface_id(), p)
                            .unwrap();
                        let c = viewport
                            .pbr_outline_pixel_for_test_harness(target.surface_id(), p)
                            .unwrap()
                            .expect("same registered generation");
                        if c.rgba[3] > f.rgba[3] {
                            witness = Some((p, c.rgba, f.rgba));
                            break 'pixels;
                        }
                    }
                }
                assert!(
                    witness.is_some(),
                    "actual native-selected source silhouette must contribute visible premult stroke pixels"
                );
                println!(
                    "hal374_native N{} source-vertex outer-stroke readback {:?}",
                    self.samples.value(),
                    witness.unwrap()
                );
            }
            println!(
                "hal374_native N{} stage{} PASS: registered source/current/pinned output, attempt{}, submitted/presented_called, static no animation demand; stimulus is programmatic, not OS keyboard",
                self.samples.value(),
                self.stage,
                receipt.attempt_id().sequence()
            );
            if self.stage == 2 {
                self.done.store(true, Ordering::Release);
                cx.request_exit();
            } else {
                self.stage += 1;
                self.inner.desired = self.stage;
                self.candidate = None;
                cx.app
                    .ui_waker()
                    .expect("actual native loop")
                    .wake()
                    .expect("native consumer notification");
            }
        }
    }
    pub(super) fn run(samples: SampleCount3D) {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();
        let (config, observer) = AppConfig::new("HAL374 original PBR selection native proof")
            .with_size(900., 650.)
            .with_frame_realization_observer();
        let done = Arc::new(AtomicBool::new(false));
        App::new(config).run(NativeOutline {
            inner: selection_outline_example::OutlineScreen::with_samples(samples),
            observer,
            stage: 0,
            last_image: None,
            candidate: None,
            started: Instant::now(),
            done: done.clone(),
            samples,
        });
        assert!(done.load(Ordering::Acquire));
        println!(
            "hal374_native N{} PASS: real App::run + original 3D shared-geometry occurrences + A/B/None + registered private/ordinary F + settled input-driven frames. Physical scanout/keyboard/JC review UNOBSERVED.",
            samples.value()
        );
    }
}

#[cfg(feature = "test-harness")]
mod spatial_hud_native {
    use super::*;
    use halcyon::three_d::*;
    use halcyon::utils::id::TreeWidgetId;
    use halcyon::widgets::screen_host::ScreenHost;
    use std::num::NonZeroU64;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct HudScreen {
        host: ScreenHost,
        observer: AppFrameObserver,
        samples: SampleCount3D,
        viewport: Option<TreeWidgetId>,
        scene: Option<PbrScene3D>,
        inspection_sent: bool,
        first_attempt: Option<AppFrameAttemptId>,
        first_front: Option<(Viewport3DRequestSerial, u64)>,
        done: Arc<AtomicBool>,
    }

    impl Screen for HudScreen {
        fn take_rebuild_flag(&mut self) -> bool {
            false
        }

        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let gpu = cx.app.gpu_resources().unwrap();
            assert!(gpu.device.features().is_empty());
            let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let scene = pbr_object_reveal_native::source(&context, self.samples, false);
            let view = ViewDescriptor3::new(
                RigidPose3::new(
                    WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                    Orientation3::identity(),
                ),
                OrthographicProjection::try_new(3.0, 0.1, 10.0)
                    .unwrap()
                    .into(),
            );
            let anchor = |id, x, z, text| PbrSpatialHudAnchor3D {
                label_id: NonZeroU64::new(id).unwrap(),
                packet_slot: 0,
                world: WorldPoint3::try_new(x, 0.0, z).unwrap(),
                text: Arc::from(text),
            };
            let hud = context
                .admit_pbr_spatial_hud(
                    &scene,
                    &[
                        anchor(1, -0.55, 0.25, "Left exterior"),
                        anchor(2, 0.55, 0.25, "Right exterior"),
                        anchor(3, 0.0, -0.25, "Behind source"),
                    ],
                )
                .unwrap();
            let mut viewport =
                Viewport3D::new_pbr(context, scene.clone(), Viewport3DViewRevision::new(1), view)
                    .unwrap();
            viewport.request_pbr_spatial_hud(Some(hud)).unwrap();
            let node = cx.mount_child(Box::new(viewport));
            self.viewport = Some(cx.tree.node(node).unwrap().id());
            self.scene = Some(scene);
            &mut self.host
        }

        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            if self.done.load(Ordering::Acquire) {
                return;
            }
            let Some(viewport_id) = self.viewport else {
                return;
            };
            let Some(receipt) = self.observer.latest() else {
                if !std::mem::replace(&mut self.inspection_sent, true) {
                    cx.app.ui_waker().unwrap().wake().unwrap();
                }
                return;
            };
            let tree = cx.tree.as_deref_mut().unwrap();
            let node = tree.find_by_id(viewport_id).unwrap();
            let viewport = tree
                .node(node)
                .unwrap()
                .widget()
                .as_any()
                .downcast_ref::<Viewport3D>()
                .unwrap();
            let Some(target) = viewport.current_prepared_target() else {
                return;
            };
            let metrics = target.metrics().unwrap();
            if metrics.app_frame_attempt != Some(receipt.attempt_id())
                || self.first_attempt == Some(receipt.attempt_id())
            {
                return;
            }
            let Some(output) = receipt.surface(target.surface_id()) else {
                return;
            };
            if !(output.output_produced() && output.submitted() && output.presented_called()) {
                return;
            }
            assert!(output.text_realization().unwrap().outcome().is_complete());
            let extent = metrics.target_physical_extent;
            let pixel = viewport
                .pbr_front_pixel_for_test_harness(
                    target.surface_id(),
                    [extent.width / 2, extent.height / 2],
                )
                .unwrap();
            assert!(pixel.source.same_identity(self.scene.as_ref().unwrap()));
            assert_eq!(pixel.view_revision, Viewport3DViewRevision::new(1));
            assert_eq!(pixel.target_key.sample_count(), self.samples);
            assert_eq!(pixel.rgba[3], 1.0);
            assert_eq!(
                viewport.status_for(target.surface_id()).unwrap().state(),
                Viewport3DLifecycleState::Committed
            );
            if self.first_attempt.is_none() {
                assert_eq!(output.visible_spatial_hud_pairs_for_test_harness(), Some(2));
                assert!(
                    !viewport.needs_update(),
                    "a static HUD does not request more frames"
                );
                self.first_attempt = Some(receipt.attempt_id());
                self.first_front = Some((pixel.request_serial, pixel.image_id));
                let viewport = tree
                    .node_mut(node)
                    .unwrap()
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<Viewport3D>()
                    .unwrap();
                assert!(viewport.request_pbr_spatial_hud(None).unwrap());
                tree.mark_paint_dirty(node);
                cx.app.ui_waker().unwrap().wake().unwrap();
            } else {
                assert_eq!(output.visible_spatial_hud_pairs_for_test_harness(), None);
                assert_eq!(self.first_front, Some((pixel.request_serial, pixel.image_id)));
                assert!(!viewport.needs_update());
                println!(
                    "hal375_native N{} PASS: source matched, view {}, target {:?} {:?}, serial {}, same Image {}. GPU-visible pairs 2 -> absent, one initially occluded. Attempts {} -> {}; submitted/presented_called",
                    self.samples.value(),
                    pixel.view_revision.value(),
                    target.surface_id(),
                    pixel.extent,
                    pixel.request_serial.value(),
                    pixel.image_id,
                    self.first_attempt.unwrap().sequence(),
                    receipt.attempt_id().sequence()
                );
                self.done.store(true, Ordering::Release);
                cx.request_exit();
            }
        }
    }

    pub(super) fn run(samples: SampleCount3D) {
        let (config, observer) = AppConfig::new("HAL375 native spatial HUD proof")
            .with_size(800.0, 600.0)
            .with_frame_realization_observer();
        let done = Arc::new(AtomicBool::new(false));
        App::new(config).run(HudScreen {
            host: ScreenHost::new(),
            observer,
            samples,
            viewport: None,
            scene: None,
            inspection_sent: false,
            first_attempt: None,
            first_front: None,
            done: done.clone(),
        });
        assert!(
            done.load(Ordering::Acquire),
            "HAL375 native proof exited before its observed terminal frame"
        );
    }

    struct OverlayMark(WidgetInternalId);

    impl WidgetDowncast for OverlayMark {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for OverlayMark {
        fn id(&self) -> WidgetInternalId {
            self.0
        }

        fn layout(
            &mut self,
            _: &mut LayoutContext,
            limits: Constraints,
            _: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(limits.constrain(Size::new(24.0, 18.0)))
        }

        fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
            cx.scene.push_rect(
                rect,
                Color::WHITE,
                halcyon::core::paint::LAYER_OVERLAY,
                None,
            );
        }
    }

    struct ZeroViewScreen {
        host: ScreenHost,
        observer: AppFrameObserver,
        inactive_context: bool,
        viewport: Option<TreeWidgetId>,
        inspection_sent: bool,
        done: Arc<AtomicBool>,
    }

    impl Screen for ZeroViewScreen {
        fn take_rebuild_flag(&mut self) -> bool {
            false
        }

        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            use halcyon::core::layout::FlexParams;
            use halcyon::plot::{BarPoint, BarSeries, Plot, ViewState};
            use halcyon::prelude::{Flex, Label};

            let gpu = cx.app.gpu_resources().unwrap().clone();
            assert!(gpu.device.features().is_empty());
            let root = cx.mount_child(Box::new(Flex::column()));
            cx.tree.add_child(
                root,
                Box::new(Label::new("Ordinary Plot and overlay")),
                None,
                0,
            );
            let mut bars = BarSeries::new();
            bars.set_points(vec![
                BarPoint {
                    x: 1.0,
                    y: 2.0,
                    color: None,
                },
                BarPoint {
                    x: 2.0,
                    y: 3.0,
                    color: None,
                },
            ]);
            let plot = Plot::new()
                .with_gpu_resources(gpu.clone())
                .with_view_state(ViewState::new(0.0, 3.0, 0.0, 4.0))
                .add_series(Box::new(bars));
            let plot = cx.tree.add_child(root, Box::new(plot), None, 1);
            cx.tree.set_flex(plot, FlexParams::flexible(1.0));
            cx.tree.add_child(
                root,
                Box::new(OverlayMark(WidgetInternalId::new())),
                None,
                2,
            );
            if self.inactive_context {
                let context = Arc::new(Context3D::new(gpu).unwrap());
                let snapshot = context
                    .admit_render_snapshot(RenderSnapshotDescriptor::new(
                        StructuralRevision::new(1),
                        Vec::new(),
                    ))
                    .unwrap();
                let model = context
                    .admit_model_state(
                        &snapshot,
                        ModelStateDescriptor::new(ModelRevision::new(1), Vec::new()),
                    )
                    .unwrap();
                let view = ViewDescriptor3::new(
                    RigidPose3::new(
                        WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                        Orientation3::identity(),
                    ),
                    OrthographicProjection::try_new(3.0, 0.1, 10.0)
                        .unwrap()
                        .into(),
                );
                let viewport = Viewport3D::new(
                    context,
                    Viewport3DViewRevision::new(1),
                    view,
                    snapshot,
                    model,
                    SampleCount3D::FOUR,
                )
                .unwrap();
                let child = cx.tree.add_child(root, Box::new(viewport), None, 3);
                cx.tree.set_flex(child, FlexParams::fixed(0.0));
                self.viewport = Some(cx.tree.node(child).unwrap().id());
            }
            &mut self.host
        }

        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            if self.done.load(Ordering::Acquire) {
                return;
            }
            let Some(receipt) = self.observer.latest() else {
                if !std::mem::replace(&mut self.inspection_sent, true) {
                    cx.app.ui_waker().unwrap().wake().unwrap();
                }
                return;
            };
            let output = &receipt.surfaces()[0];
            if !(output.output_produced() && output.submitted() && output.presented_called()) {
                return;
            }
            assert!(output.text_realization().unwrap().outcome().is_complete());
            assert_eq!(output.visible_spatial_hud_pairs_for_test_harness(), None);
            let counts = output.ordinary_draw_counts_for_test_harness();
            if counts.0 == 0 {
                // Plot publishes its ordinary GPU front through its existing update.
                return;
            }
            assert_eq!(counts, (1, 1, 2));
            if let Some(viewport) = self.viewport {
                let tree = cx.tree.as_deref().unwrap();
                let node = tree.find_by_id(viewport).unwrap();
                let viewport = tree
                    .node(node)
                    .unwrap()
                    .widget()
                    .as_any()
                    .downcast_ref::<Viewport3D>()
                    .unwrap();
                let target = viewport.current_prepared_target().unwrap();
                assert_eq!(
                    target.metrics().unwrap().activity,
                    CustomRenderActivity::Inactive
                );
                assert_eq!(target.surface_id(), output.surface_id());
                assert_eq!(
                    viewport.status_for(target.surface_id()).unwrap().state(),
                    Viewport3DLifecycleState::Inactive
                );
            }
            println!(
                "hal375_zero_view_native PASS inactive_context={} images={} overlays={} base_passes={} ordinary Complete, submitted/presented_called, HUD absent",
                self.inactive_context, counts.0, counts.1, counts.2
            );
            self.done.store(true, Ordering::Release);
            cx.request_exit();
        }
    }

    pub(super) fn run_zero_view(inactive_context: bool) {
        let (config, observer) = AppConfig::new("HAL375 ordinary 2D absence proof")
            .with_size(800.0, 600.0)
            .with_frame_realization_observer();
        let done = Arc::new(AtomicBool::new(false));
        App::new(config).run(ZeroViewScreen {
            host: ScreenHost::new(),
            observer,
            inactive_context,
            viewport: None,
            inspection_sent: false,
            done: done.clone(),
        });
        assert!(
            done.load(Ordering::Acquire),
            "ordinary Plot/Label frame not observed"
        );
    }
}

#[cfg(all(target_os = "macos", feature = "test-harness"))]
mod native_resize {
    use super::*;
    use halcyon::core::layout::{EdgeInsets, FlexParams};
    use halcyon::prelude::{FixedSpacer, Flex, Label};
    use halcyon::three_d::{
        Context3DError, LoadingPresenceError3D, PbrScene3D, PbrSourceError3D, PolicyBudget3D,
        SampleCount3D, Viewport3D, Viewport3DLifecycleState,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use tracing_subscriber::{Layer, filter::filter_fn, prelude::*};

    const SIZES: &[(u32, u32)] = &[
        (1800, 1300),
        (2048, 1400),
        (2048, 1402),
        (2048, 1536),
        (2048, 1548),
        (2048, 1550),
        (1800, 1300),
        (2048, 1402),
        (2560, 2048),
        (1800, 1300),
    ];

    #[derive(Default)]
    struct ResizeWake {
        wake: Option<UiWakeHandle>,
        observe: bool,
        observe_deferred_resize: bool,
    }

    struct ObserveResizeOutput(Arc<Mutex<ResizeWake>>);

    impl<S: tracing::Subscriber> Layer<S> for ObserveResizeOutput {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut state = self.0.lock().unwrap();
            let fields = event.metadata().fields();
            if fields.field("resize_target_us").is_some() {
                state.observe = true;
                if state.observe_deferred_resize
                    && let Some(wake) = &state.wake
                {
                    wake.wake().unwrap();
                }
            } else if fields.field("submit_us").is_some() && state.observe {
                // This is an observation wake, after the existing output phase.
                // It never supplies the frame that handles a Resized event.
                state.observe = false;
                if let Some(wake) = &state.wake {
                    wake.wake().unwrap();
                }
            }
        }
    }

    fn resize_trace(m: &tracing::Metadata<'_>) -> bool {
        m.target() == "halcyon::core::app"
            && (m.fields().field("resize_target_us").is_some()
                || m.fields().field("submit_us").is_some())
    }

    struct HeldPlotSeries {
        armed: Arc<AtomicBool>,
        entered: Arc<AtomicBool>,
        release: crossbeam_channel::Receiver<()>,
        wake: UiWakeHandle,
    }

    impl halcyon::plot::series::Series for HeldPlotSeries {
        fn init(&mut self, _: &wgpu::Device, _: &wgpu::Queue, _: wgpu::TextureFormat) {}

        fn prepare(
            &mut self,
            _: &wgpu::Device,
            _: &wgpu::Queue,
            _: &halcyon::plot::Transform,
            _: halcyon::plot::series::PhysicalRect,
        ) {
            if self.armed.swap(false, Ordering::AcqRel) {
                self.entered.store(true, Ordering::Release);
                self.wake.wake().unwrap();
                self.release.recv_timeout(Duration::from_secs(20)).unwrap();
            }
        }

        fn render<'a>(
            &'a self,
            _: &mut wgpu::RenderPass<'a>,
            _: halcyon::plot::series::PhysicalRect,
        ) {
        }
        fn z_order(&self) -> u32 {
            0
        }
        fn hit_test(
            &self,
            _: f32,
            _: f32,
            _: &halcyon::plot::Transform,
        ) -> Option<Box<dyn std::any::Any>> {
            None
        }
        fn hit_proxy(&self) -> halcyon::plot::hit_proxy::HitProxy {
            halcyon::plot::hit_proxy::HitProxy::Candle {
                candles: Vec::new(),
            }
        }
    }

    struct PlotResizeProbe {
        placeholder: Label,
        observer: AppFrameObserver,
        wake: Arc<Mutex<ResizeWake>>,
        armed: Arc<AtomicBool>,
        entered: Arc<AtomicBool>,
        release: crossbeam_channel::Sender<()>,
        receiver: crossbeam_channel::Receiver<()>,
        plot: Option<NodeId>,
        start: Option<(SurfaceId, (u32, u32), u64)>,
        requested: bool,
        deferred: Option<u64>,
        done: Arc<AtomicBool>,
    }

    impl Screen for PlotResizeProbe {
        fn take_rebuild_flag(&mut self) -> bool {
            false
        }

        fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
            self.wake.lock().unwrap().wake = Some(cx.app.ui_waker().unwrap());
            cx.request_child_window("Independent native output", 160, 100, "");
        }

        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            use halcyon::plot::price_level::PriceLevel;
            use halcyon::plot::{BarPoint, BarSeries, Plot, ViewState};
            use halcyon::widgets::split_pane::SplitPane;

            let root = cx.mount_child(Box::new(SplitPane::horizontal(NodeId(0), NodeId(0))));
            let mut series = BarSeries::new();
            series.set_points(vec![BarPoint {
                x: 1.0,
                y: 3.0,
                color: None,
            }]);
            let plot = Plot::new()
                .with_gpu_resources(cx.app.gpu_resources().unwrap().clone())
                .with_view_state(ViewState::new(0.0, 2.0, 0.0, 4.0))
                .with_price_level(PriceLevel {
                    id: 1,
                    y: 2.0,
                    ..PriceLevel::default()
                })
                .with_market_detail_reference_level(1)
                .add_series(Box::new(series))
                .add_series(Box::new(HeldPlotSeries {
                    armed: self.armed.clone(),
                    entered: self.entered.clone(),
                    release: self.receiver.clone(),
                    wake: cx.app.ui_waker().unwrap(),
                }));
            let leading = cx.tree.add_child(root, Box::new(plot), None, 0);
            let trailing = cx
                .tree
                .add_child(root, Box::new(Label::new("Book")), None, 1);
            let split = cx
                .tree
                .node_mut(root)
                .unwrap()
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<SplitPane>()
                .unwrap();
            split.leading = leading;
            split.trailing = trailing;
            self.plot = Some(leading);
            &mut self.placeholder
        }

        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            let Some(frame) = self.observer.latest() else { return };
            let Some(output) = frame.surfaces().first() else { return };
            let surface = output.surface_id();
            let Some(child) = frame.surfaces().iter().find(|target| target.surface_id() != surface) else {
                return;
            };
            let sequence = frame.attempt_id().sequence();
            let submitted = output.submitted() && output.presented_called();
            let Some(target) = cx.app.ui_scale_status(Some(surface)).target else { return };
            let Some(effective) = target.effective else { return };
            let size = effective.physical_size;
            let Some((initial_surface, initial_size, warm)) = self.start else {
                if submitted && child.submitted()
                    && output.ordinary_draw_counts_for_test_harness().0 == 1
                {
                    self.start = Some((surface, size, sequence));
                    self.armed.store(true, Ordering::Release);
                    cx.tree
                        .as_deref_mut()
                        .unwrap()
                        .node_mut(self.plot.unwrap())
                        .unwrap()
                        .widget_mut()
                        .as_any_mut()
                        .downcast_mut::<halcyon::plot::Plot>()
                        .unwrap()
                        .update_price_level(1, |level| level.y = 2.5);
                }
                return;
            };
            assert_eq!(surface, initial_surface);
            if !self.requested && self.entered.load(Ordering::Acquire) {
                self.requested = true;
                resize_native_proof_window("Plot native resize proof", 680.0, 300.0);
            }
            if !self.requested || size == initial_size || sequence <= warm {
                return;
            }
            if let Some(deferred) = self.deferred {
                if sequence > deferred && submitted {
                    assert_eq!(output.ordinary_draw_counts_for_test_harness().0, 1);
                    println!(
                        "Plot native pending PASS surface={surface:?} size={initial_size:?}->{size:?} warm={warm} withheld={deferred} submitted={sequence} child={:?}",
                        child.surface_id(),
                    );
                    self.done.store(true, Ordering::Release);
                    cx.request_exit();
                }
            } else {
                assert_eq!(
                    target.output,
                    halcyon::core::app::UiScaleTargetOutput::Pending
                );
                assert!(
                    !output.output_produced() && !output.submitted() && !output.presented_called()
                );
                assert!(output.text_realization().is_none());
                assert!(child.output_produced() && child.submitted() && child.presented_called());
                self.deferred = Some(sequence);
                self.release.send(()).unwrap();
            }
        }
    }

    pub(super) fn run_plot_resize_pending() {
        let wake = Arc::new(Mutex::new(ResizeWake {
            observe_deferred_resize: true,
            ..Default::default()
        }));
        tracing_subscriber::registry()
            .with(ObserveResizeOutput(wake.clone()).with_filter(filter_fn(resize_trace)))
            .init();
        let (config, observer) = AppConfig::new("Plot native resize proof")
            .with_size(640.0, 300.0)
            .with_frame_realization_observer();
        let (release, receiver) = crossbeam_channel::bounded(1);
        let done = Arc::new(AtomicBool::new(false));
        App::new(config).run(PlotResizeProbe {
            placeholder: Label::new(""),
            observer,
            wake,
            armed: Arc::new(AtomicBool::new(false)),
            entered: Arc::new(AtomicBool::new(false)),
            release,
            receiver,
            plot: None,
            start: None,
            requested: false,
            deferred: None,
            done: done.clone(),
        });
        assert!(done.load(Ordering::Acquire));
    }

    enum Content {
        Outline(selection_outline_example::OutlineScreen),
        Presence(loading_presence_example::PresenceScreen),
        Text(Label),
    }

    struct Probe {
        content: Content,
        mode: String,
        samples: u32,
        observer: AppFrameObserver,
        wake: Arc<Mutex<ResizeWake>>,
        step: usize,
        pending: Option<((u32, u32), (u32, u32), Instant)>,
        seen: u64,
        label: Option<NodeId>,
        full_requested: bool,
        ready_at: (u32, u32),
        source: Option<PbrScene3D>,
        started: Instant,
        done: Arc<AtomicBool>,
    }

    impl Screen for Probe {
        fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
            {
                let mut state = self.wake.lock().unwrap();
                state.wake = Some(cx.app.ui_waker().expect("native observer wake"));
                state.observe = true;
            }
            if let Content::Presence(screen) = &mut self.content {
                screen.on_enter(cx);
            }
        }

        fn on_leave(&mut self, cx: &mut ScreenContext<'_>) {
            if let Content::Presence(screen) = &mut self.content {
                screen.on_leave(cx);
            }
        }

        fn take_rebuild_flag(&mut self) -> bool {
            false
        }

        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            match &mut self.content {
                Content::Outline(screen) => screen.build(cx),
                Content::Presence(screen) => screen.build(cx),
                Content::Text(placeholder) => {
                    let root = cx
                        .mount_child(Box::new(Flex::column().with_padding(EdgeInsets::all(20.0))));
                    cx.tree.add_child(
                        root,
                        Box::new(Label::new("Native resize text control")),
                        None,
                        0,
                    );
                    let fill =
                        cx.tree
                            .add_child(root, Box::new(FixedSpacer::vertical(1.0)), None, 1);
                    cx.tree.set_flex(fill, FlexParams::flexible(1.0));
                    self.label = Some(cx.tree.add_child(
                        root,
                        Box::new(Label::new("Bottom label follows window height")),
                        None,
                        2,
                    ));
                    placeholder
                }
            }
        }

        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            assert!(
                self.started.elapsed() < Duration::from_secs(55),
                "resize proof deadline step {}",
                self.step
            );
            match &mut self.content {
                Content::Outline(screen) => screen.flush_pending_actions(cx),
                Content::Presence(screen) => screen.flush_pending_actions(cx),
                Content::Text(_) => {}
            }
            let Some(receipt) = self.observer.latest() else {
                return;
            };
            let attempt = receipt.attempt_id();
            if attempt.sequence() == self.seen {
                return;
            }
            let Some(output) = receipt.surfaces().first() else {
                return;
            };
            let surface = output.surface_id();
            let Some(effective) = cx
                .app
                .ui_scale_status(Some(surface))
                .target
                .and_then(|t| t.effective)
            else {
                return;
            };
            let window = effective.physical_size;
            if self
                .pending
                .is_some_and(|(requested, _, _)| window != requested)
            {
                return;
            }
            let observed = Instant::now();
            let tree = cx.tree.as_deref().unwrap();
            let viewport_id = match &self.content {
                Content::Outline(screen) => screen.viewport.and_then(|id| tree.find_by_id(id)),
                Content::Presence(screen) => screen.viewport,
                Content::Text(_) => None,
            };
            let mut render = window;
            let mut start_full = false;
            if let Some(id) = viewport_id {
                let Some(viewport) = tree
                    .node(id)
                    .and_then(|n| n.widget().as_any().downcast_ref::<Viewport3D>())
                else {
                    return;
                };
                let Some(target) = viewport.current_prepared_target() else {
                    return;
                };
                let Some(metrics) = target
                    .metrics()
                    .filter(|m| m.app_frame_attempt == Some(attempt))
                else {
                    return;
                };
                assert_eq!(target.surface_id(), surface);
                let CustomRenderActivity::Active {
                    physical_render_extent,
                    physical_composition_coverage,
                } = metrics.activity
                else {
                    return;
                };
                render = (physical_render_extent.width, physical_render_extent.height);
                let requested = target.status().pbr_requested();
                let committed = target.status().pbr_committed();
                let pbr_error = viewport.pbr_failure(surface);
                let outline_error = viewport.pbr_outline_error(surface);
                let presence_error = viewport.loading_presence_error();
                let presence = viewport.loading_presence_progress_for_test_harness(surface);
                if matches!(self.content, Content::Presence(_)) {
                    if !self.full_requested && presence.is_none() && presence_error.is_none() {
                        return;
                    }
                    if self.full_requested
                        && self
                            .pending
                            .is_some_and(|(_, expected, _)| expected == self.ready_at)
                        && pbr_error.is_none()
                        && presence_error.is_none()
                        && !presence.is_some_and(|(_, terminal)| terminal)
                    {
                        return;
                    }
                    start_full = !self.full_requested
                        && self
                            .pending
                            .is_some_and(|(_, expected, _)| expected == self.ready_at);
                }
                if let Some(c) = &committed {
                    let source = c.source();
                    assert!(
                        self.source
                            .as_ref()
                            .is_none_or(|old| old.same_identity(&source))
                    );
                    self.source = Some(source);
                }
                let p = u64::from(render.0) * u64::from(render.1);
                let n = u64::from(self.samples);
                let policy = viewport.context().limits().policy;
                let canonical_bytes = p * (20 * n + 16) + 54_525_952;
                let presence_bytes = p * (32 * n + 24) + 54_525_952;
                let outline_bytes = p * (36 * n + 24) + 54_525_952;
                let cap = policy.max_single_target_attachment_bytes;
                if let Some(request) = &requested {
                    if canonical_bytes > cap {
                        assert!(matches!(
                            pbr_error,
                            Some(PbrSourceError3D::Context(Context3DError::PolicyBudgetExceeded {
                                budget: PolicyBudget3D::SingleTargetAttachmentBytes,
                                requested,
                                maximum,
                            })) if requested == canonical_bytes && maximum == cap
                        ));
                        assert!(committed.is_none());
                    } else {
                        assert!(pbr_error.is_none(), "canonical preparation: {pbr_error:?}");
                        assert_eq!(target.status().state(), Viewport3DLifecycleState::Committed);
                        let front = committed.as_ref().expect("complete matching PBR front");
                        assert_eq!(front.request_serial(), request.request_serial());
                        assert_eq!(front.view_revision(), request.view_revision());
                        assert_eq!(front.target_key(), request.target_key());
                    }
                }
                let timing = viewport.pbr_timing_for_test();
                println!(
                    "resize_pbr mode={} N{} seq={} state={:?} render={render:?} rect={:?} clip={physical_composition_coverage:?} requested={:?} committed={:?} pbr_error={pbr_error:?} outline_error={outline_error:?} presence_error={presence_error:?} presence={presence:?} canonical_with_direct={} outline_with_direct={} presence_with_direct={} single={} aggregate={} hdr_alloc_us={:?} active_us={:?}",
                    self.mode,
                    self.samples,
                    attempt.sequence(),
                    target.status().state(),
                    metrics.final_logical_rect,
                    requested.map(|t| (t.request_serial(), t.view_revision(), t.target_key())),
                    committed.map(|t| (t.request_serial(), t.view_revision(), t.target_key())),
                    canonical_bytes,
                    outline_bytes,
                    presence_bytes,
                    cap,
                    policy.max_total_attachment_bytes,
                    timing.map(|t| t.hdr_allocation.as_micros()),
                    timing.map(|t| t.active_total.as_micros()),
                );
                let outline_boundary = self.mode == "outline" && render == (2048, 1402);
                let presence_boundary = self.mode == "presence"
                    && self.full_requested
                    && render == (2048, 1550);
                if outline_boundary || presence_boundary {
                    let center = [render.0 / 2, render.1 / 2];
                    let point = if presence_boundary {
                        let hole = viewport
                            .pbr_front_pixel_for_test_harness(surface, center)
                            .unwrap();
                        assert_eq!(hole.rgba, [0.0; 4], "the original solid has a through-hole");
                        [center[0] + render.1 / 5, center[1]]
                    } else {
                        center
                    };
                    let pixel = viewport.pbr_front_pixel_for_test_harness(surface, point).unwrap();
                    assert!(pixel.source.same_identity(self.source.as_ref().unwrap()));
                    assert_eq!(pixel.rgba[3], 1.0);
                    if outline_boundary && self.samples == 4 {
                        assert!(matches!(
                            outline_error,
                            Some(PbrSourceError3D::Context(Context3DError::PolicyBudgetExceeded {
                                budget: PolicyBudget3D::SingleTargetAttachmentBytes,
                                requested,
                                maximum,
                            })) if *requested == outline_bytes && *maximum == cap
                        ));
                        assert!(
                            viewport
                                .pbr_outline_pixel_for_test_harness(surface, center)
                                .unwrap()
                                .is_none()
                        );
                    }
                    if presence_boundary && self.samples == 4 {
                        assert!(matches!(
                            presence_error,
                            Some(LoadingPresenceError3D::Context(Context3DError::PolicyBudgetExceeded {
                                budget: PolicyBudget3D::SingleTargetAttachmentBytes,
                                requested,
                                maximum,
                            })) if *requested == presence_bytes && *maximum == cap
                        ));
                        assert!(presence.is_none());
                    }
                    println!(
                        "resize_registration seq={} point={point:?} pbr_pixel={:?}",
                        attempt.sequence(),
                        (pixel.image_id, pixel.rgba)
                    );
                }
                if self.label.is_none() {
                    let parent = tree.parent(id).unwrap();
                    let chrome = if matches!(self.content, Content::Presence(_)) {
                        tree.parent(parent).unwrap()
                    } else {
                        parent
                    };
                    self.label = tree
                        .children(chrome)
                        .into_iter()
                        .rev()
                        .find(|&n| tree.node(n).unwrap().widget().as_any().is::<Label>());
                }
            } else if !matches!(self.content, Content::Text(_)) {
                return;
            }
            if let Some((_, expected, _)) = self.pending {
                assert_eq!(
                    render, expected,
                    "measured viewport must match requested test extent"
                );
            }
            let label = self.label.and_then(|id| tree.get_absolute_paint_rect(id));
            assert!(output.text_realization().unwrap().outcome().is_complete());
            assert!(output.output_produced() && output.submitted() && output.presented_called());
            if self.mode == "text" {
                let label = label.expect("mounted bottom text");
                assert_eq!(
                    label.y + label.height,
                    window.1 as f32 / effective.platform_density - 20.0,
                );
            }
            println!(
                "resize_frame mode={} N{} seq={} surface={surface:?} dpr={} window={window:?} render={render:?} text_rect={label:?} text={:?} outcome={:?} output={} submitted={} present={} observation_us={} since_request_us={:?}",
                self.mode,
                self.samples,
                attempt.sequence(),
                effective.platform_density,
                output.text_realization().map(|t| t.outcome()),
                receipt.compile_outcome(),
                output.output_produced(),
                output.submitted(),
                output.presented_called(),
                (observed - self.started).as_micros(),
                self.pending.map(|(_, _, t)| (observed - t).as_micros()),
            );
            self.seen = attempt.sequence();
            if start_full {
                if let Content::Presence(screen) = &mut self.content {
                    screen.complete = true;
                    self.full_requested = true;
                }
                return;
            }
            let Some(&next) = SIZES.get(self.step) else {
                println!(
                    "resize_observation_complete mode={} N{}; programmatic AppKit resize; physical drag/scanout unobserved",
                    self.mode, self.samples
                );
                self.done.store(true, Ordering::Release);
                cx.request_exit();
                return;
            };
            self.step += 1;
            if self.mode == "outline" {
                if let Content::Outline(screen) = &mut self.content {
                    if self.step == 7 {
                        screen.desired = 1;
                    } else if self.step == SIZES.len() {
                        screen.desired = 2;
                    }
                }
            }
            if let Some(v) = viewport_id.and_then(|id| {
                cx.tree
                    .as_deref_mut()?
                    .node_mut(id)?
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<Viewport3D>()
            }) {
                v.set_pbr_timing_for_test(true);
            }
            let next_window = (window.0 - render.0 + next.0, window.1 - render.1 + next.1);
            let dpr = effective.platform_density;
            let start = Instant::now();
            self.pending = Some((next_window, next, start));
            if self.mode == "text" && next == (2048, 1536) {
                for height in [1420, 1472, 1512] {
                    resize_native_proof_window(
                        "HAL native resize diagnostic",
                        next_window.0 as f32 / dpr,
                        height as f32 / dpr,
                    );
                }
            }
            resize_native_proof_window(
                "HAL native resize diagnostic",
                next_window.0 as f32 / dpr,
                next_window.1 as f32 / dpr,
            );
            println!(
                "resize_request mode={} window={next_window:?} render={next:?} at_us={}",
                self.mode,
                (start - self.started).as_micros()
            );
        }
    }

    pub(super) fn run(mode: &str, samples: SampleCount3D) {
        let wake = Arc::new(Mutex::new(ResizeWake::default()));
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_filter(filter_fn(|m| {
                        resize_trace(m) || *m.level() <= tracing::Level::WARN
                            || (m.target() == "winit::platform_impl::macos::app_state"
                                && m.fields().field("event").is_some())
                    })),
            )
            .with(ObserveResizeOutput(wake.clone()).with_filter(filter_fn(resize_trace)))
            .init();
        let content = match mode {
            "outline" | "pbr" => {
                let mut screen = selection_outline_example::OutlineScreen::with_samples(samples);
                if mode == "pbr" {
                    screen.desired = 2;
                }
                Content::Outline(screen)
            }
            "presence" | "presence-under" => Content::Presence(
                loading_presence_example::PresenceScreen::with_samples(samples),
            ),
            "text" => Content::Text(Label::new("")),
            _ => panic!("expected outline, pbr, presence, presence-under or text"),
        };
        let size = if samples == SampleCount3D::ONE {
            (800.0, 600.0)
        } else {
            (900.0, 650.0)
        };
        let (config, observer) = AppConfig::new("HAL native resize diagnostic")
            .with_size(size.0, size.1)
            .with_frame_realization_observer();
        let done = Arc::new(AtomicBool::new(false));
        App::new(config).run(Probe {
            content,
            mode: mode.into(),
            samples: samples.value(),
            observer,
            wake,
            step: 0,
            pending: None,
            seen: 0,
            label: None,
            full_requested: false,
            ready_at: if mode == "presence-under" {
                (2048, 1548)
            } else {
                (2048, 1550)
            },
            source: None,
            started: Instant::now(),
            done: done.clone(),
        });
        assert!(done.load(Ordering::Acquire));
    }
}

#[cfg(all(target_os = "macos", feature = "test-harness"))]
mod native_layout_transition_pacer {
    use super::*;
    use halcyon::core::layout::{Alignment, EdgeInsets, FlexParams};
    use halcyon::prelude::{Button, FixedSpacer, Flex, Label};
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tracing_subscriber::{Layer, filter::filter_fn, prelude::*};

    const TARGET: &str = "halcyon::native_layout_transition_pacer";
    const WINDOW_TITLE: &str = "HAL078 passive native pacer proof";
    const LIMIT: usize = 512;
    const QUIET: Duration = Duration::from_millis(300);

    #[derive(Debug)]
    struct Record(BTreeMap<String, String>);

    impl Record {
        fn is(&self, key: &str, value: &str) -> bool {
            self.0.get(key).is_some_and(|actual| actual == value)
        }
        fn number(&self, key: &str) -> u64 {
            self.0.get(key).unwrap().parse().unwrap()
        }
        fn progress(&self) -> f64 {
            self.0.get("progress").unwrap().parse().unwrap()
        }
        fn decimal(&self, key: &str) -> f64 {
            let value = self.0.get(key).unwrap();
            value.strip_prefix("Some(").and_then(|v| v.strip_suffix(')'))
                .unwrap_or(value).parse().unwrap()
        }
    }

    impl tracing::field::Visit for Record {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.0.insert(field.name().into(), format!("{value:?}"));
        }
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            self.0.insert(field.name().into(), value.into());
        }
        fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
            self.0.insert(field.name().into(), value.to_string());
        }
        fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
            self.0.insert(field.name().into(), value.to_string());
        }
        fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
            self.0.insert(field.name().into(), value.to_string());
        }
    }

    struct Collect {
        sender: crossbeam_channel::Sender<Record>,
        lost: Arc<AtomicBool>,
    }

    impl<S: tracing::Subscriber> Layer<S> for Collect {
        fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
            let mut record = Record(BTreeMap::new());
            event.record(&mut record);
            if self.sender.try_send(record).is_err() {
                self.lost.store(true, Ordering::Release);
            }
        }
    }

    enum FixtureChange {
        ExpandSpacer,
        Resize(f32, f32),
    }

    fn request_change(
        changes: &crossbeam_channel::Sender<FixtureChange>,
        wake: &UiWakeHandle,
        change: FixtureChange,
    ) -> Result<(), String> {
        changes.try_send(change).map_err(|error| error.to_string())?;
        wake.wake().map_err(|error| format!("{error:?}"))
    }

    struct Probe {
        placeholder: Label,
        setup: Option<std::sync::mpsc::SyncSender<UiWakeHandle>>,
        changes: crossbeam_channel::Receiver<FixtureChange>,
        spacer: Option<NodeId>,
        exit: Arc<AtomicBool>,
        stale: bool,
    }

    impl Screen for Probe {
        fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
            self.setup.take().unwrap().send(cx.app.ui_waker().unwrap()).unwrap();
        }
        fn take_rebuild_flag(&mut self) -> bool { false }
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let root = cx.mount_child(Box::new(
                if self.stale {
                    Flex::column().with_align_items(Alignment::Stretch)
                        .with_padding(EdgeInsets::all(8.0)).with_background(Color::from_hex(0x173344))
                } else { Flex::column().with_background(Color::from_hex(0x173344)) },
            ));
            let host = cx.tree.parent(root).unwrap();
            tracing::trace!(target: "halcyon::native_layout_transition_pacer",
                event = "fixture_mount", owner = cx.tree.node(root).unwrap().id().0,
                host_owner = cx.tree.node(host).unwrap().id().0,
                host_type = cx.tree.node(host).unwrap().widget().debug_type_name());
            let spacer = cx.tree.add_child(root, Box::new(FixedSpacer::vertical(1.0)), None, 0);
            if self.stale {
                self.spacer = Some(spacer);
                cx.tree.set_flex(spacer, FlexParams::fixed(144.0));
                let button = cx.tree.add_child(root, Box::new(Button::with_text("Action").with_enabled(false)), None, 1);
                cx.tree.set_flex(button, FlexParams::fixed(32.0));
                tracing::trace!(target: "halcyon::native_layout_transition_pacer",
                    event = "stale_mount", density_owner = cx.tree.node(spacer).unwrap().id().0,
                    button_owner = cx.tree.node(button).unwrap().id().0);
            } else {
                cx.tree.set_flex(spacer, FlexParams::flexible(1.0));
                cx.tree.add_child(root, Box::new(Label::new("Ordinary native layout reflow")), None, 1);
            }
            &mut self.placeholder
        }
        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            if self.exit.load(Ordering::Acquire) {
                cx.app.request_exit();
                return;
            }
            if let Ok(change) = self.changes.try_recv() {
                match change {
                    FixtureChange::ExpandSpacer => cx.tree.as_deref_mut().unwrap().set_flex(
                        self.spacer.expect("mounted stale-projection spacer"),
                        FlexParams::fixed(216.0),
                    ),
                    FixtureChange::Resize(width, height) => {
                        resize_native_proof_window(WINDOW_TITLE, width, height);
                    }
                }
            }
        }
    }

    fn no_competitor(record: &Record) -> bool {
        ["widget_active", "scroll_active", "presentation_active", "tooltip_active",
            "capture_active", "continuous_input", "material_pending", "plot_pending"]
            .iter().all(|field| record.is(field, "false"))
    }

    fn receive_until_quiet(
        receiver: &crossbeam_channel::Receiver<Record>, records: &mut Vec<Record>,
        deadline: Instant, ready: impl Fn(&[Record]) -> bool,
    ) -> Result<(), String> {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() { return Err("native witness deadline expired".into()); }
            let wait = QUIET.min(remaining);
            match receiver.recv_timeout(wait) {
                Ok(record) => {
                    if records.len() == LIMIT { return Err("native trace record bound exhausted".into()); }
                    records.push(record);
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) if wait == QUIET && ready(records) => return Ok(()),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(error) => return Err(error.to_string()),
            }
        }
    }

    fn prove(records: &[Record], baseline: u64) -> Result<String, String> {
        let adapter = records.iter().find(|r| r.is("event", "adapter")).ok_or("missing actual renderer adapter witness")?;
        if !adapter.is("backend", "Metal") { return Err(format!("native proof did not use Metal: {adapter:?}")); }
        let relevant: Vec<_> = records.iter().filter(|r|
            r.0.get("attempt_sequence").is_some_and(|v| v.parse::<u64>().unwrap() > baseline)
            || r.0.get("last_attempt_sequence").is_some_and(|v| v.parse::<u64>().unwrap() > baseline)
        ).collect();
        let first = relevant.iter().find(|r| r.is("event", "member") && r.is("phase", "postbuild"))
            .ok_or("no real post-layout HAL078 member after the single resize")?;
        let sequence = first.number("attempt_sequence");
        let owner = first.number("owner");
        let handle = first.number("handle");
        let mount = records.iter().find(|r| r.is("event", "fixture_mount")).ok_or("missing actual mounted tree identity")?;
        if owner != mount.number("owner") && owner != mount.number("host_owner") {
            return Err("transition owner is not the actual mounted Flex/host chain".into());
        }
        let frame = relevant.iter().find(|r| r.is("event", "frame") && r.number("attempt_sequence") == sequence)
            .ok_or("missing same-attempt aggregate")?;
        if !frame.is("prebuild_timeline", "false") || !frame.is("prebuild_demand", "false")
            || !frame.is("timeline_active", "true") || !frame.is("animation_needed", "true")
            || !no_competitor(frame) || !frame.is("geometry_ready", "true")
        { return Err(format!("first late transition did not uniquely supply demand: {frame:?}")); }
        if frame.number("physical_w") as f64 / frame.decimal("scale") != 480.0
            || frame.number("physical_h") as f64 / frame.decimal("scale") != 240.0
        { return Err(format!("real target did not receive the requested native resize: {frame:?}")); }
        if first.progress() != 0.0 || !first.is("finite", "true") || !first.is("current_matches", "true") {
            return Err(format!("invalid initial canonical endpoint witness: {first:?}"));
        }
        let pacer = relevant.iter().find(|r| r.is("event", "request") && r.is("cause", "pacer")
            && r.number("last_attempt_sequence") >= sequence
            && relevant.iter().any(|next| next.is("event", "member") && next.is("phase", "postbuild")
                && next.number("owner") == owner && next.number("handle") == handle
                && next.number("attempt_sequence") > r.number("last_attempt_sequence")
                && next.progress() > 0.0 && next.progress() < 1.0))
            .ok_or("no actual engine pacer request joined to subsequent progress")?;
        let progressed = relevant.iter().find(|r| r.is("event", "member") && r.is("phase", "postbuild")
            && r.number("owner") == owner && r.number("handle") == handle
            && r.number("attempt_sequence") > pacer.number("last_attempt_sequence")
            && r.progress() > 0.0 && r.progress() < 1.0)
            .ok_or("same original native handle never progressed after the actual pacer request")?;
        if !progressed.is("canonical_matches", "true") {
            return Err(format!("native progress endpoint no longer matches live canonical geometry: {progressed:?}"));
        }
        for field in ["from_x", "from_y", "from_w", "from_h", "to_x", "to_y", "to_w", "to_h"] {
            if first.0.get(field) != progressed.0.get(field) { return Err("native handle endpoints changed".into()); }
        }
        let completed = relevant.iter().find(|r| r.is("event", "member")
            && r.number("owner") == owner && r.number("handle") == handle
            && r.is("status", "Completed") && r.progress() == 1.0)
            .ok_or("no genuine Completed terminal sample before ordinary cleanup")?;
        if !relevant.iter().any(|r| r.is("event", "output")
            && r.number("attempt_sequence") == progressed.number("attempt_sequence")
            && r.is("output_produced", "true") && r.is("submitted", "true") && r.is("presented", "true")) {
            return Err("no same-attempt native output for the pacer-driven intermediate projection".into());
        }
        let quiet = relevant.iter().rev().find(|r| r.is("event", "frame")).ok_or("no final frame")?;
        if !no_competitor(quiet) || !quiet.is("animation_needed", "false")
            || !quiet.is("timeline_active", "false") || !quiet.is("scene_built", "false")
            || !quiet.is("visual_changed", "false")
        { return Err(format!("native final retained frame did not settle: {quiet:?}")); }
        let output = relevant.iter().find(|r| r.is("event", "output")
            && r.number("attempt_sequence") == quiet.number("attempt_sequence")).ok_or("no quiet output receipt")?;
        if ["compiled", "should_present", "output_produced", "submitted", "presented"]
            .iter().any(|field| !output.is(field, "false")) { return Err(format!("idle output work: {output:?}")); }
        for record in relevant.iter().filter(|r| r.is("event", "frame")) {
            if !no_competitor(record) { return Err(format!("competing native demand: {record:?}")); }
        }
        Ok(format!("backend={} adapter={}; first={sequence} owner={owner} handle={handle} mounted_flex={} mounted_host={} host_type={}; target={}x{} scale={}; endpoints={:?} -> {:?}; pacer_after={} progress_attempt={} value={} submitted/presented=true; completed_after={} quiet_attempt={} no_scene/compile/output=true; records={}",
            adapter.0["backend"], adapter.0["name"],
            mount.number("owner"), mount.number("host_owner"), mount.0["host_type"],
            frame.number("physical_w"), frame.number("physical_h"), frame.decimal("scale"),
            (&first.0["from_x"], &first.0["from_y"], &first.0["from_w"], &first.0["from_h"]),
            (&first.0["to_x"], &first.0["to_y"], &first.0["to_w"], &first.0["to_h"]),
            pacer.number("last_attempt_sequence"), progressed.number("attempt_sequence"), progressed.progress(),
            completed.number("attempt_sequence"), quiet.number("attempt_sequence"), records.len()))
    }

    #[cfg(debug_assertions)]
    fn denial_trace(epoch: u64, rows: &[halcyon::core::layout_transition::UnifiedDenialRecord]) {
        for row in rows {
            if let halcyon::core::layout_transition::UnifiedDenialRecord::Conflict { widget_id, reason } = row {
                tracing::trace!(target: "halcyon::native_layout_transition_pacer",
                    event = "denial", epoch, owner = widget_id.0, reason = ?reason);
            }
        }
    }

    fn receive_until_ready(
        receiver: &crossbeam_channel::Receiver<Record>, records: &mut Vec<Record>,
        deadline: Instant, ready: impl Fn(&[Record]) -> bool,
    ) -> Result<(), String> {
        while !ready(records) {
            if records.len() == LIMIT { return Err("native trace record bound exhausted".into()); }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() { return Err("native stale-projection deadline expired".into()); }
            records.push(receiver.recv_timeout(remaining).map_err(|e| e.to_string())?);
        }
        Ok(())
    }

    fn prove_stale(
        receiver: &crossbeam_channel::Receiver<Record>, records: &mut Vec<Record>,
        deadline: Instant, changes: &crossbeam_channel::Sender<FixtureChange>,
        wake: &UiWakeHandle, baseline: u64,
    ) -> Result<String, String> {
        let mount = records.iter().find(|r| r.is("event", "stale_mount")).ok_or("missing mounted Button")?;
        let button = mount.number("button_owner");
        request_change(changes, wake, FixtureChange::ExpandSpacer)?;
        receive_until_ready(receiver, records, deadline, |rows| rows.iter().any(|m|
            m.is("event", "member") && m.is("phase", "postbuild") && m.number("owner") == button
                && m.is("status", "Active") && m.number("attempt_sequence") > baseline
                && rows.iter().any(|o| o.is("event", "output")
                    && o.number("attempt_sequence") == m.number("attempt_sequence") && o.is("presented", "true"))))?;
        let primed = records.iter().find(|m| m.is("event", "member") && m.is("phase", "postbuild")
            && m.number("owner") == button && m.number("attempt_sequence") > baseline).unwrap();
        let handle = primed.number("handle");
        let prime_sequence = primed.number("attempt_sequence");
        if primed.decimal("to_y") - primed.decimal("from_y") != 72.0 {
            return Err(format!("density did not prime actual 16-to-24 row-height geometry: {primed:?}"));
        }
        request_change(changes, wake, FixtureChange::Resize(480.0, 400.0))?;
        receive_until_ready(receiver, records, deadline, |rows| rows.iter().any(|m|
            m.is("event", "member") && m.is("phase", "postbuild") && m.number("owner") == button
                && m.number("handle") == handle && m.number("attempt_sequence") > prime_sequence
                && m.is("canonical_matches", "false") && m.is("paint_rect_available", "true")
                && rows.iter().any(|o| o.is("event", "output")
                    && o.number("attempt_sequence") == m.number("attempt_sequence") && o.is("presented", "true"))))?;
        let member = records.iter().find(|m| m.is("event", "member") && m.is("phase", "postbuild")
            && m.number("owner") == button && m.number("handle") == handle
            && m.number("attempt_sequence") > prime_sequence && m.is("canonical_matches", "false")
            && m.is("paint_rect_available", "true")).unwrap();
        let sequence = member.number("attempt_sequence");
        let frame = records.iter().find(|r| r.is("event", "frame") && r.number("attempt_sequence") == sequence)
            .ok_or("missing native current-frame aggregate")?;
        if !member.is("status", "Active") || !(0.0..1.0).contains(&member.progress()) || !member.is("projected", "false")
            || [member.decimal("paint_x"),member.decimal("paint_y"),member.decimal("paint_w"),member.decimal("paint_h")]
                != [8.0,224.0,464.0,32.0] || !no_competitor(frame)
            || frame.number("physical_w") as f64 / frame.decimal("scale") != 480.0
            || frame.number("physical_h") as f64 / frame.decimal("scale") != 400.0
        { return Err(format!("native old interval was not purely excluded at current canonical paint: {member:?} {frame:?}")); }
        if !frame.is("layout_epoch_known", "true") || !records.iter().any(|r|
            r.is("event", "denial") && r.number("epoch") == frame.number("layout_epoch")
                && r.number("owner") == button && r.is("reason", "ParentTransitioning")) {
            return Err("missing actual same-layout-epoch ParentTransitioning denial".into());
        }
        let changed_progress = member.progress();
        let changed_epoch = frame.number("layout_epoch");
        receive_until_ready(receiver, records, deadline, |rows| rows.iter().any(|r|
            r.is("event", "retirement") && r.number("owner") == button && r.number("handle") == handle))?;
        let retirement_index = records.iter().position(|r| r.is("event", "retirement")
            && r.number("owner") == button && r.number("handle") == handle).unwrap();
        let retired = &records[retirement_index];
        if !retired.is("phase", "widget_update") || !retired.is("issuer_match", "true")
            || !retired.is("disposition", "Cancelled") || !retired.is("cancelled_visible", "true")
            || retired.number("previous_generation") == 0
        { return Err(format!("original native retirement did not authenticate legal Active->Cancelled: {retired:?}")); }
        request_change(changes, wake, FixtureChange::Resize(640.0, 480.0))?;
        receive_until_quiet(receiver, records, deadline, |rows| {
            rows.iter().rev().find(|r| r.is("event", "frame")).is_some_and(|r|
                r.number("attempt_sequence") > sequence && r.is("timeline_active", "false")
                    && r.is("animation_needed", "false") && r.is("scene_built", "false")
                    && r.number("physical_w") as f64 / r.decimal("scale") == 640.0
                    && r.number("physical_h") as f64 / r.decimal("scale") == 480.0)
        })?;
        if records[retirement_index + 1..].iter().any(|r| r.is("event", "member")
            && r.number("owner") == button && r.number("handle") == handle)
        { return Err("retired original handle resurfaced on native round trip".into()); }
        let retirement_frame = records[retirement_index + 1..].iter()
            .find(|r| r.is("event", "frame")).ok_or("no frame for legal B retirement")?;
        if retirement_frame.number("attempt_sequence") != sequence + 1
            || !retirement_frame.is("geometry_ready", "true")
        { return Err(format!("B was not the next admitted native Update: {retirement_frame:?}")); }
        let retirement_attempt = retirement_frame.number("attempt_sequence");
        let resumed = records[retirement_index + 1..].iter().find(|r| r.is("event", "frame")
            && r.number("physical_w") as f64 / r.decimal("scale") == 640.0
            && r.number("physical_h") as f64 / r.decimal("scale") == 480.0
            && records.iter().any(|o| o.is("event", "output")
                && o.number("attempt_sequence") == r.number("attempt_sequence") && o.is("presented", "true")))
            .ok_or("no ordinary native return-size output")?;
        let quiet = records.iter().rev().find(|r| r.is("event", "frame")).unwrap();
        let output = records.iter().find(|r| r.is("event", "output")
            && r.number("attempt_sequence") == quiet.number("attempt_sequence")).ok_or("no settled output receipt")?;
        if !no_competitor(quiet) || ["compiled", "should_present", "output_produced", "submitted", "presented"]
            .iter().any(|field| !output.is(field, "false"))
        { return Err(format!("native round-trip idle did unnecessary output: {quiet:?} {output:?}")); }
        let adapter = records.iter().find(|r| r.is("event", "adapter")).ok_or("missing native adapter")?;
        if !adapter.is("backend", "Metal") { return Err("not a real Metal native proof".into()); }
        Ok(format!("A+B backend=Metal adapter={} prime_attempt={prime_sequence} button={button} old_handle={handle} changed_attempt={sequence} old_status=Active progress={changed_progress} projected=false actual_paint=(8,224,464,32) ParentTransitioning_epoch={changed_epoch}; original issuer matched; retirement_attempt={retirement_attempt} next WidgetUpdate Active->Cancelled; return_attempt={} old_handle_absent=true quiet_attempt={} no_scene/compile/output=true records={}",
            adapter.0["name"], resumed.number("attempt_sequence"), quiet.number("attempt_sequence"), records.len()))
    }

    pub(super) fn run(stale: bool) {
        let (sender, receiver) = crossbeam_channel::bounded(LIMIT);
        let lost = Arc::new(AtomicBool::new(false));
        let subscriber = tracing_subscriber::registry()
            .with(Collect { sender, lost: lost.clone() }.with_filter(filter_fn(|m| m.target() == TARGET)))
            .with(tracing_subscriber::fmt::layer().with_ansi(false).with_filter(filter_fn(|m|
                *m.level() <= tracing::Level::WARN || m.target() == "halcyon::renderer::wgpu_renderer")));
        let _subscriber = tracing::subscriber::set_default(subscriber);
        let (setup, ready) = std::sync::mpsc::sync_channel::<UiWakeHandle>(1);
        let exit = Arc::new(AtomicBool::new(false));
        let (config, _observer) = AppConfig::new(WINDOW_TITLE)
            .with_size(if stale { 640.0 } else { 320.0 }, if stale { 480.0 } else { 180.0 })
            .with_frame_realization_observer();
        let (changes, stimulus_rx) = crossbeam_channel::bounded(1);
        let quit = exit.clone();
        let worker = std::thread::spawn(move || {
            let wake = ready.recv_timeout(Duration::from_secs(20)).expect("native App startup");
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut records = Vec::with_capacity(LIMIT);
            let result = (|| {
                receive_until_quiet(&receiver, &mut records, deadline, |rows| {
                    let frame = rows.iter().rev().find(|r| r.is("event", "frame"));
                    frame.is_some_and(|r| r.is("primary_visible", "true") && no_competitor(r)
                        && r.is("timeline_active", "false") && r.is("animation_needed", "false"))
                        && rows.iter().any(|r| r.is("event", "output") && r.is("presented", "true"))
                })?;
                let baseline = records.iter().rev().find(|r| r.is("event", "frame")).unwrap().number("attempt_sequence");
                if stale {
                    let evidence = prove_stale(&receiver, &mut records, deadline, &changes, &wake, baseline)?;
                    if lost.load(Ordering::Acquire) { return Err("passive native trace overflow/disconnect".into()); }
                    return Ok(evidence);
                }
                println!("hal078_native: startup settled at attempt {baseline}; one native resize 320x180 -> 480x240");
                request_change(&changes, &wake, FixtureChange::Resize(480.0, 240.0))?;
                receive_until_quiet(&receiver, &mut records, deadline, |rows|
                    rows.iter().any(|r| r.is("event", "member") && r.is("status", "Completed"))
                        && rows.iter().rev().find(|r| r.is("event", "frame")).is_some_and(|r|
                            r.number("attempt_sequence") > baseline && r.is("animation_needed", "false"))
                )?;
                if lost.load(Ordering::Acquire) { return Err("passive native trace overflow/disconnect".into()); }
                prove(&records, baseline)
            })();
            if result.is_err() { for record in &records { eprintln!("hal078_native_record {record:?}"); } }
            // This sole closing wake is after terminal proof or declared failure.
            quit.store(true, Ordering::Release);
            wake.wake().expect("close owned proof window");
            result
        });
        #[cfg(debug_assertions)]
        let previous_hook = halcyon::core::layout_transition::get_denial_telemetry_hook();
        #[cfg(debug_assertions)]
        if stale { halcyon::core::layout_transition::set_denial_telemetry_hook(Some(denial_trace)); }
        App::new(config).run(Probe {
            placeholder: Label::new(""),
            setup: Some(setup),
            changes: stimulus_rx,
            spacer: None,
            exit,
            stale,
        });
        #[cfg(debug_assertions)]
        halcyon::core::layout_transition::set_denial_telemetry_hook(previous_hook);
        let result = worker.join().expect("native proof controller").expect("HAL078 native pacer proof");
        println!("{}: PASS {result}; no observation wakes; AppKit/Metal submit/present calls, not physical scanout",
            if stale { "hal078_native_stale_projection" } else { "hal078_native_pacer" });
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "native_quit_return_child") {
        #[cfg(target_os = "macos")]
        native_quit::run_child(args.get(2).expect("native Quit child case"));
        #[cfg(not(target_os = "macos"))]
        panic!("native Quit return proof requires macOS");
        return;
    }
    if args.iter().any(|arg| arg == "native_quit_return") {
        #[cfg(target_os = "macos")]
        native_quit::run_parent();
        #[cfg(not(target_os = "macos"))]
        panic!("native Quit return proof requires macOS");
        return;
    }
    if args.iter().any(|arg|arg=="--run-missing-cluster-native") {
        #[cfg(target_os = "macos")]
        missing_cluster_native::run();
        #[cfg(not(target_os = "macos"))]
        panic!("missing-cluster native proof requires macOS");
        return;
    }
    if args.iter().any(|arg| arg == "native_layout_transition_pacer" || arg == "--run-layout-transition-pacer-native") {
        #[cfg(all(target_os = "macos", feature = "test-harness"))]
        native_layout_transition_pacer::run(false);
        #[cfg(not(all(target_os = "macos", feature = "test-harness")))]
        panic!("native layout-transition pacer proof requires macOS and test-harness");
        return;
    }
    if args.iter().any(|arg| arg == "native_stale_layout_projection") {
        #[cfg(all(target_os = "macos", feature = "test-harness", debug_assertions))]
        native_layout_transition_pacer::run(true);
        #[cfg(not(all(target_os = "macos", feature = "test-harness", debug_assertions)))]
        panic!("native stale projection proof requires macOS/test-harness/debug denial telemetry");
        return;
    }
    if args.iter().any(|arg| arg == "--run-arch-floor-native") {
        #[cfg(all(target_os = "macos", feature = "test-harness"))]
        arch_floor_native::run(if args.iter().any(|arg| arg == "--samples-one") {
            halcyon::three_d::SampleCount3D::ONE
        } else {
            halcyon::three_d::SampleCount3D::FOUR
        });
        #[cfg(not(all(target_os = "macos", feature = "test-harness")))]
        panic!("--run-arch-floor-native requires macOS and test-harness");
        return;
    }
    if let Some(mode) = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--run-resize-native="))
    {
        #[cfg(all(target_os = "macos", feature = "test-harness"))]
        if mode == "plot-pending" {
            native_resize::run_plot_resize_pending();
            return;
        }
        #[cfg(all(target_os = "macos", feature = "test-harness"))]
        native_resize::run(
            mode,
            if args.iter().any(|arg| arg == "--samples-one") {
                halcyon::three_d::SampleCount3D::ONE
            } else {
                halcyon::three_d::SampleCount3D::FOUR
            },
        );
        #[cfg(not(all(target_os = "macos", feature = "test-harness")))]
        panic!("native resize observation requires macOS and test-harness: {mode}");
        return;
    }
    let startup = args.iter().any(|arg| arg == "--run-startup-native");
    let flat_transfer = args
        .iter()
        .any(|arg| arg == "--run-flat-viewport3d-transfer-native");
    let reveal = args
        .iter()
        .any(|arg| arg == "--run-pbr-object-reveal-native");
    let mrt4 = args.iter().any(|arg| arg == "--run-r027-mrt4-native");
    let presence = args
        .iter()
        .any(|arg| arg == "--run-loading-presence-native");
    let outline = args
        .iter()
        .any(|arg| arg == "--run-selection-outline-native");
    let idle_wake = args.iter().any(|arg| arg == "--run-idle-wake-native");
    let spatial_hud = args.iter().any(|arg| arg == "--run-spatial-hud-native");
    let zero_view = args
        .iter()
        .any(|arg| arg == "--run-spatial-hud-zero-view-native");
    let scale_proof = args.iter().any(|arg| arg == "--run-ui-scale-native");
    let native = scale_proof || args.iter().any(|arg| arg == "--run-native");
    assert!(
        [
            startup,
            native,
            flat_transfer,
            reveal,
            mrt4,
            presence,
            outline,
            idle_wake,
            spatial_hud,
            zero_view,
        ]
        .into_iter()
        .filter(|v| *v)
        .count()
            <= 1,
        "choose one native proof mode per process"
    );
    if zero_view {
        #[cfg(feature = "test-harness")]
        spatial_hud_native::run_zero_view(args.iter().any(|arg| arg == "--inactive-context"));
        #[cfg(not(feature = "test-harness"))]
        panic!("native zero-view observation requires the existing test-harness feature");
        return;
    }
    if spatial_hud {
        #[cfg(feature = "test-harness")]
        spatial_hud_native::run(if args.iter().any(|a| a == "--samples-one") {
            halcyon::three_d::SampleCount3D::ONE
        } else {
            halcyon::three_d::SampleCount3D::FOUR
        });
        #[cfg(not(feature = "test-harness"))]
        panic!("native HAL375 observation requires the existing test-harness feature");
        return;
    }
    if idle_wake {
        #[cfg(feature = "test-harness")]
        pbr_object_reveal_native::run_idle_wake();
        #[cfg(not(feature = "test-harness"))]
        panic!("native wake observation requires the existing test-harness feature");
        return;
    }
    if outline {
        #[cfg(feature = "test-harness")]
        selection_outline_native::run(if args.iter().any(|a| a == "--samples-one") {
            halcyon::three_d::SampleCount3D::ONE
        } else {
            halcyon::three_d::SampleCount3D::FOUR
        });
        #[cfg(not(feature = "test-harness"))]
        panic!("native HAL374 observation requires the existing test-harness feature");
        return;
    }
    if presence {
        #[cfg(feature = "test-harness")]
        loading_presence_native::run(if args.iter().any(|a| a == "--samples-one") {
            halcyon::three_d::SampleCount3D::ONE
        } else {
            halcyon::three_d::SampleCount3D::FOUR
        });
        #[cfg(not(feature = "test-harness"))]
        panic!("native HAL381 observation requires the existing test-harness feature");
        return;
    }
    if reveal || mrt4 {
        #[cfg(feature = "test-harness")]
        pbr_object_reveal_native::run(
            if mrt4 && args.iter().any(|arg| arg == "--samples-one") {
                halcyon::three_d::SampleCount3D::ONE
            } else {
                halcyon::three_d::SampleCount3D::FOUR
            },
            mrt4,
        );
        #[cfg(not(feature = "test-harness"))]
        panic!("--run-pbr-object-reveal-native requires the existing test-harness feature");
        return;
    }
    if flat_transfer {
        flat_viewport3d_transfer_native::run();
        return;
    }
    if startup {
        #[cfg(windows)]
        startup_native_proof::run(args.iter().any(|arg| arg == "--fullscreen"));
        #[cfg(not(windows))]
        panic!("--run-startup-native requires Windows HWND visibility observation");
        return;
    }
    if !native {
        println!(
            "app_frame_realization_native: native execution skipped; pass --run-native, --run-startup-native or --run-flat-viewport3d-transfer-native to this executable"
        );
        return;
    }
    let (mut config, observer) = AppConfig::new(PROBE_TITLE)
        .with_size(320.0, 180.0)
        .with_frame_realization_observer();
    if scale_proof {
        config.ui_scale = halcyon::core::app::UiScale::try_from(1.25).unwrap();
    }
    assert!(observer.latest().is_none());
    let state = Arc::new(Mutex::new(ProbeState::default()));
    let screen = ProbeScreen {
        stub: ProbeWidget::new(state.clone(), None),
        state: state.clone(),
        observer,
        node: None,
        stage: 0,
        first: None,
        started: Instant::now(),
        scale_proof,
        #[cfg(windows)]
        inactive: None,
    };
    App::new(config).run(screen);
    let (worker, waker) = {
        let mut state = state.lock().unwrap();
        (
            state.wake_worker.take().unwrap(),
            state.ui_waker.take().unwrap(),
        )
    };
    // Joining is after the UI loop returns, never on its running callback path.
    assert_eq!(worker.join().expect("CPU wake sender"), Ok(()));
    assert_eq!(waker.wake(), Err(UiWakeError::EventLoopClosed));
    println!(
        "hal373_ui_waker_native: Screen Some, CPU enqueue x4, post-loop Closed; idle/hidden wake not established by this continuously updating receipt fixture"
    );
    assert!(
        state.lock().unwrap().done,
        "native proof exited before all cases"
    );
    println!(
        "app_frame_realization_native: PASS (submit/present calls, not GPU/display completion)"
    );
}
