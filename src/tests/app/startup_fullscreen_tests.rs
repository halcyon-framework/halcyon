    use super::{
        AppConfig, AppContext, AppRoot, RectF, Screen, ScreenEntry, ScreenFrameContext, Theme,
        WidgetTree, startup_viewport_rect,
    };
    use crate::core::widget::Widget;
    use crate::widgets::label::Label;
    use winit::dpi::PhysicalSize;

    struct ReadinessScreen(Result<bool, String>, Label);

    impl Screen for ReadinessScreen {
        fn build<'a>(&'a mut self, _: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            &mut self.1
        }

        fn startup_ready(&self) -> Result<bool, String> {
            self.0.clone()
        }

        fn flush_pending_actions(&mut self, _: &mut super::ScreenContext<'_>) {
            if self.0 == Ok(false) {
                self.0 = Ok(true);
            }
        }
    }

    fn readiness_screen(readiness: Result<bool, String>) -> Box<dyn Screen> {
        Box::new(ReadinessScreen(readiness, Label::new("startup")))
    }

    #[test]
    fn startup_readiness_combines_active_screen_and_modals_without_masking_failure() {
        let mut tree = WidgetTree::new();
        let host = tree.create_root(Box::new(Label::new("host")), None, 0);
        let mut root = AppRoot::new(AppContext::new(Theme::default()), host);
        assert_eq!(root.startup_ready(), Ok(true));
        root.screens
            .push(readiness_screen(Err("inactive screen".into())));
        root.screens.push(readiness_screen(Ok(true)));
        assert_eq!(root.startup_ready(), Ok(true));
        root.modals
            .stack
            .push(ScreenEntry::new(readiness_screen(Ok(false))));
        assert_eq!(root.startup_ready(), Ok(false));
        root.screens.push(readiness_screen(Ok(false)));
        root.modals
            .stack
            .push(ScreenEntry::new(readiness_screen(Err(
                "modal resource failure".into(),
            ))));
        assert_eq!(root.startup_ready(), Err("modal resource failure".into()));
        root.modals.stack.clear();
        root.screens
            .push(readiness_screen(Err("screen resource failure".into())));
        assert_eq!(root.startup_ready(), Err("screen resource failure".into()));
    }

    #[test]
    fn startup_lower_modal_receives_action_settlement_before_readiness() {
        let mut tree = WidgetTree::new();
        let host = tree.create_root(Box::new(Label::new("host")), None, 0);
        let mut root = AppRoot::new(AppContext::new(Theme::default()), host);
        root.screens.push(readiness_screen(Ok(true)));
        for _ in 0..2 {
            root.modals
                .stack
                .push(ScreenEntry::new(readiness_screen(Ok(false))));
        }
        root.flush_pending_actions_all(&mut tree);
        assert_eq!(root.startup_ready(), Ok(false));
        root.flush_pending_startup_modal_actions(&mut tree);
        assert_eq!(root.startup_ready(), Ok(true));
        assert_eq!(root.modals.stack.len(), 2);
    }

    #[test]
    fn startup_fullscreen_config_defaults_and_builder_preserve_windowed_size() {
        let default = AppConfig::default();
        assert!(!default.fullscreen);
        assert_eq!(default.title, "Halcyon App");
        assert_eq!((default.width, default.height), (800.0, 600.0));

        let windowed = AppConfig::new("Home").with_size(720.0, 480.0);
        assert!(!windowed.fullscreen);
        let fullscreen = windowed.with_fullscreen(true);
        assert!(fullscreen.fullscreen);
        assert_eq!(fullscreen.title, "Home");
        assert_eq!((fullscreen.width, fullscreen.height), (720.0, 480.0));

        let resized = fullscreen.with_size(960.0, 640.0);
        assert!(resized.fullscreen);
        let restored = resized.with_fullscreen(false);
        assert!(!restored.fullscreen);
        assert_eq!(restored.title, "Home");
        assert_eq!((restored.width, restored.height), (960.0, 640.0));
    }

    #[test]
    fn startup_fullscreen_initial_bounds_use_obtained_extent_once() {
        let config = AppConfig::new("Home")
            .with_size(720.0, 480.0)
            .with_fullscreen(true);
        for (scale, expected) in [
            (1.0, RectF::new(0.0, 0.0, 1920.0, 1080.0)),
            (1.25, RectF::new(0.0, 0.0, 1536.0, 864.0)),
            (2.0, RectF::new(0.0, 0.0, 960.0, 540.0)),
        ] {
            let actual = startup_viewport_rect(PhysicalSize::new(1920, 1080), scale);
            assert_eq!(actual, expected);
            assert_ne!(actual, RectF::new(0.0, 0.0, config.width, config.height));
        }
        for scale in [1.0, 1.25, 2.0] {
            let physical = PhysicalSize::new(
                (config.width * scale) as u32,
                (config.height * scale) as u32,
            );
            assert_eq!(
                startup_viewport_rect(physical, scale),
                RectF::new(0.0, 0.0, config.width, config.height),
            );
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod native_output {
        use super::super::CompiledAppAttempt;
        use crate::compositor::SurfaceId;
        use crate::core::frame_realization::{
            AppFrameAttempts, AppFrameCompileOutcome, AppFrameRecorder,
        };
        use crate::renderer::image_binding::{ImageAdmissionError, ImageBindingFailure};
        use crate::renderer::{
            RendererCompileError, RendererCompileOutcome, RendererPreparationIdentity,
            RendererSurfaceError,
        };
        use std::panic::{AssertUnwindSafe, catch_unwind};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use std::time::Instant;

        fn attempt(startup_ready: bool, primary_output_ready: bool) -> CompiledAppAttempt {
            let now = Instant::now();
            CompiledAppAttempt {
                id: AppFrameAttempts::new().next(),
                plots: Vec::new(),
                recorder: None,
                primary_output_ready,
                startup_ready,
                should_present: true,
                render_time: now,
                build_scene_started: now,
                compile_started: now,
                draw_plan_us: 0,
            }
        }

        fn diagnostic(action: impl FnOnce()) -> String {
            let panic = catch_unwind(AssertUnwindSafe(action)).expect_err("startup must fail");
            *panic.downcast::<String>().expect("formatted startup diagnostic")
        }

        #[test]
        fn startup_ready_requires_primary_in_render_outputs() {
            let attempt = attempt(true, true);
            let primary = SurfaceId::test_id(3);
            let secondary = SurfaceId::test_id(4);
            let startup = attempt.native_startup_output(false, primary);

            for outputs in [Vec::new(), vec![secondary]] {
                let message = diagnostic(|| startup.require_primary(outputs.into_iter()));
                assert!(message.contains(&format!("{:?}", attempt.id)), "{message}");
                assert!(message.contains(&format!("expected primary {primary:?}")), "{message}");
                assert!(message.contains("Render produced no primary output"), "{message}");
            }
            startup.require_primary([secondary, primary].into_iter());
        }

        #[test]
        fn startup_render_and_submission_preserve_supplied_custody_causes() {
            let startup_attempt = attempt(true, true);
            let primary = SurfaceId::test_id(3);
            let startup = startup_attempt.native_startup_output(false, primary);
            let shown = startup_attempt.native_startup_output(true, primary);
            for (operation, cause) in [
                ("Render failed", ImageAdmissionError::MissingRegistration(41)),
                (
                    "Startup submission failed",
                    ImageAdmissionError::Unavailable(ImageBindingFailure::DeviceLost),
                ),
            ] {
                let message = diagnostic(|| {
                    let _ = startup.require_success(operation, Err::<(), _>(cause.clone()));
                });
                assert!(message.contains(&format!("{operation}: {cause:?}")), "{message}");
                assert_eq!(
                    shown.require_success(operation, Err::<(), _>(cause.clone())),
                    Err(cause),
                );
            }
        }

        #[test]
        fn startup_terminal_compile_records_no_targets_and_original_error() {
            let primary = SurfaceId::test_id(3);
            let error = RendererCompileError::Surface(RendererSurfaceError::OutOfMemory);
            for (outcome, recorded, detail) in [
                (
                    Ok(RendererCompileOutcome::<()>::NoTargets),
                    AppFrameCompileOutcome::NoTargets,
                    "Compile returned NoTargets",
                ),
                (
                    Err(error.clone()),
                    AppFrameCompileOutcome::Failed(error),
                    "Compile failed: Surface(OutOfMemory)",
                ),
            ] {
                let mut attempt = attempt(true, true);
                attempt.recorder = Some(AppFrameRecorder::new(attempt.id, [primary].into_iter()));
                let message =
                    diagnostic(|| attempt.check_compile_outcome(&outcome, false, primary));
                assert!(message.contains(detail), "{message}");
                assert_eq!(
                    attempt.recorder.unwrap().snapshot().compile_outcome(),
                    &recorded,
                );
            }
        }

        #[test]
        fn startup_pending_readiness_and_visible_attempts_allow_no_primary() {
            let primary = SurfaceId::test_id(3);
            for (visible, startup_ready, primary_output_ready) in
                [(false, false, true), (false, true, false), (true, true, true)]
            {
                let mut attempt = attempt(startup_ready, primary_output_ready);
                attempt
                    .native_startup_output(visible, primary)
                    .require_primary(std::iter::empty());
                attempt.check_compile_outcome(
                    &Ok(RendererCompileOutcome::<()>::NoTargets),
                    visible,
                    primary,
                );
            }
        }

        #[test]
        fn startup_pending_compile_keeps_its_original_wake() {
            let primary = SurfaceId::test_id(3);
            let mut attempt = attempt(true, true);
            let identity = RendererPreparationIdentity::new(Arc::new(()));
            identity.add_scope();
            let wake_count = Arc::new(AtomicUsize::new(0));
            let count = wake_count.clone();
            identity.on_completed(move || {
                count.fetch_add(1, Ordering::Relaxed);
            });

            attempt.check_compile_outcome(
                &Ok(RendererCompileOutcome::<()>::Pending(identity.clone())),
                false,
                primary,
            );
            assert_eq!(identity.qualification(), Ok(false));
            assert!(!identity.take_completion_notice());
            assert_eq!(wake_count.load(Ordering::Relaxed), 0);

            identity.finish_scope(false);
            assert!(identity.take_completion_notice());
            assert_eq!(wake_count.load(Ordering::Relaxed), 1);
        }
    }
