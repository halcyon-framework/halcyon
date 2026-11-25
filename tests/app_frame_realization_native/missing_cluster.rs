//! HAL360 §7: ordinary same-surface input continues beside uncovered text.
use super::*;
use halcyon::core::layout::{Alignment, EdgeInsets, FlexParams};
use halcyon::prelude::{Flex, Label, TextBlock, TextField};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::{NSPoint, NSString};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing_subscriber::{Layer, filter::filter_fn, prelude::*};

const WINDOW_TITLE: &str = "HAL360 missing-cluster native proof";

fn post_test_keystroke() {
    let window = owned_native_window(WINDOW_TITLE);
    let text = NSString::from_str("x");
    const ANSI_X: u16 = 0x07;
    // AppKit queues the key pair for this process's proof window.
    for kind in [NSEventType::KeyDown, NSEventType::KeyUp] {
        let event = unsafe {
            NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                kind,
                NSPoint::new(0.0, 0.0),
                NSEventModifierFlags::empty(),
                0.0,
                window.windowNumber(),
                None,
                &text,
                &text,
                false,
                ANSI_X,
            )
            .expect("native proof key event")
        };
        unsafe { window.postEvent_atStart(&event, false) };
    }
}

struct AdapterWitness(Arc<AtomicBool>);
impl<S: tracing::Subscriber> Layer<S> for AdapterWitness {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        #[derive(Default)]
        struct Fields {
            event: String,
            backend: String,
            name: String,
        }
        impl tracing::field::Visit for Fields {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                match field.name() {
                    "event" => self.event = format!("{value:?}"),
                    "backend" => self.backend = format!("{value:?}"),
                    "name" => self.name = format!("{value:?}"),
                    _ => {}
                }
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                match field.name() {
                    "event" => self.event = value.to_owned(),
                    "backend" => self.backend = value.to_owned(),
                    "name" => self.name = value.to_owned(),
                    _ => {}
                }
            }
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        if fields.event == "adapter" {
            assert_eq!(fields.backend, "Metal");
            self.0.store(true, Ordering::Release);
            println!(
                "HAL_TEXT_NATIVE_ADAPTER backend={} name={}",
                fields.backend, fields.name
            );
        }
    }
}

struct Proof {
    placeholder: Label,
    setup: Option<std::sync::mpsc::SyncSender<UiWakeHandle>>,
    changed: std::sync::mpsc::SyncSender<String>,
    type_requested: Arc<AtomicBool>,
    exit: Arc<AtomicBool>,
}

impl Screen for Proof {
    fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
        self.setup
            .take()
            .unwrap()
            .send(cx.app.ui_waker().unwrap())
            .unwrap();
    }
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_align_items(Alignment::Stretch)
                .with_padding(EdgeInsets::all(16.0))
                .with_background(Color::from_hex(0x18232f)),
        ));
        let text = cx.tree.add_child(
            root,
            Box::new(
                TextBlock::new("Healthy text A \u{10ffff} B remains visible.")
                    .with_color(Color::WHITE),
            ),
            None,
            0,
        );
        cx.tree.set_flex(text, FlexParams::fixed(40.0));
        let mut field = TextField::new();
        field.set_start_captured(true);
        let changed = self.changed.clone();
        field.set_on_change(move |value| {
            changed.send(value.to_owned()).unwrap();
        });
        let input = cx.tree.add_child(root, Box::new(field), None, 1);
        cx.tree.set_flex(input, FlexParams::fixed(32.0));
        &mut self.placeholder
    }
    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        if self.exit.load(Ordering::Acquire) {
            cx.app.request_exit();
        } else if self.type_requested.swap(false, Ordering::AcqRel) {
            post_test_keystroke();
        }
    }
}

fn wait_output(observer: &AppFrameObserver, after: u64, deadline: Instant) -> AppFrameAttemptId {
    loop {
        assert!(
            Instant::now() < deadline,
            "ordinary native text output deadline"
        );
        if let Some(receipt) = observer.latest()
            && receipt.attempt_id().sequence() > after
            && receipt.surfaces().iter().any(|surface| {
                surface.output_produced()
                    && surface.submitted()
                    && surface.presented_called()
                    && matches!(
                        surface.text_realization().unwrap().outcome(),
                        TextRealizationOutcome::Degraded {
                            missing_cluster_count: 1,
                            ..
                        }
                    )
            })
        {
            println!(
                "HAL_TEXT_NATIVE_FRAME attempt={:?} surfaces={:?}",
                receipt.attempt_id(),
                receipt.surfaces()
            );
            return receipt.attempt_id();
        }
        // Test-controller observation only; this never wakes the native event loop.
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn run() {
    let adapter = Arc::new(AtomicBool::new(false));
    let subscriber =
        tracing_subscriber::registry().with(AdapterWitness(adapter.clone()).with_filter(
            filter_fn(|m| m.target() == "halcyon::native_layout_transition_pacer"),
        ));
    let _subscriber = tracing::subscriber::set_default(subscriber);
    let (config, observer) = AppConfig::new(WINDOW_TITLE)
        .with_size(520.0, 180.0)
        .with_frame_realization_observer();
    let (setup, ready) = std::sync::mpsc::sync_channel::<UiWakeHandle>(1);
    let (changed, edits) = std::sync::mpsc::sync_channel(4);
    let exit = Arc::new(AtomicBool::new(false));
    let stop = exit.clone();
    let type_requested = Arc::new(AtomicBool::new(false));
    let request_type = type_requested.clone();
    let controller = std::thread::spawn(move || {
        let wake = ready.recv_timeout(Duration::from_secs(20)).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let baseline = wait_output(&observer, 0, Instant::now() + Duration::from_secs(20));
            request_type.store(true, Ordering::Release);
            wake.wake().unwrap();
            assert_eq!(edits.recv_timeout(Duration::from_secs(20)).unwrap(), "x");
            let next = wait_output(
                &observer,
                baseline.sequence(),
                Instant::now() + Duration::from_secs(20),
            );
            println!(
                "HAL_TEXT_NATIVE_INPUT_PRESENTED baseline={baseline:?} edited={next:?} source_unchanged=true; press Enter after owned-window capture"
            );
            let mut ack = String::new();
            std::io::stdin().read_line(&mut ack).unwrap();
        }));
        stop.store(true, Ordering::Release);
        wake.wake().unwrap();
        result
    });
    App::new(config).run(Proof {
        placeholder: Label::new(""),
        setup: Some(setup),
        changed,
        type_requested,
        exit,
    });
    controller.join().unwrap().unwrap();
    assert!(
        adapter.load(Ordering::Acquire),
        "actual native renderer adapter witness"
    );
    println!(
        "HAL_TEXT_NATIVE_PASS ordinary AppKit/WGPU same-surface Degraded presentation and routed TextField input; no consumer recovery hook; scanout not certified"
    );
}
