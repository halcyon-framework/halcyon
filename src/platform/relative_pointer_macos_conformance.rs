//! Isolated real-macOS evidence capture for SPEC-HAL-362 §§10.1 and 12.2.
//! Native adapter capture and a separate diagnostic real-engine capture.
//! Neither enables ordinary production capability admission.

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use serde_json::{Value, json};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, DeviceEvents, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowAttributes};

use crate::core::event::{
    FusedInputEvent, GestureCandidateId, GestureSessionId, InteractionDomainId, PointerButton,
    PointerDevice, PointerId, RawInputEvent, WindowId,
};
use crate::core::input_fusion::InputFusion;
use crate::core::math::Point;
use crate::core::relative_pointer::{
    CursorRestorePolicy, RelativePointerIngressGate, RelativePointerLeaseBinding,
    RelativePointerLeaseIdAllocator, RelativePointerNormalizationEpoch,
};
use crate::platform::relative_pointer::RelativePointerPlatform;
use crate::platform::relative_pointer_macos::{
    MacOsRelativePointerPlatform, refresh_cursor_rects_after_focus,
    relative_motion_from_device_event,
};
use crate::utils::id::TreeWidgetId;

#[path = "relative_pointer_macos_conformance_analysis.rs"]
mod analysis;

const WINDOW_ID: WindowId = WindowId::PRIMARY;

struct Harness {
    log: File,
    records: Vec<Value>,
    progress: analysis::CaptureProgress,
    last_guidance: String,
    started: Instant,
    sequence: u64,
    window: Option<Arc<Window>>,
    platform: Option<MacOsRelativePointerPlatform<Arc<Window>>>,
    fusion: InputFusion,
    lease_ids: RelativePointerLeaseIdAllocator,
    gate: Option<RelativePointerIngressGate>,
    anchor: Option<Point>,
    anchor_monitor: Option<MonitorHandle>,
    anchor_scale: f64,
    focused: bool,
    last_absolute: Option<Point>,
    quartz_source: Option<CGEventSource>,
    fatal: Option<String>,
}

impl Harness {
    fn new(log: File) -> Self {
        Self {
            log,
            records: Vec::new(),
            progress: analysis::CaptureProgress::default(),
            last_guidance: String::new(),
            started: Instant::now(),
            sequence: 0,
            window: None,
            platform: None,
            fusion: InputFusion::new(),
            lease_ids: RelativePointerLeaseIdAllocator::new(),
            gate: None,
            anchor: None,
            anchor_monitor: None,
            anchor_scale: 0.0,
            focused: false,
            last_absolute: None,
            quartz_source: CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok(),
            fatal: None,
        }
    }

    fn record(&mut self, kind: &str, detail: Value) {
        self.sequence += 1;
        let line = json!({
            "sequence": self.sequence,
            "elapsed_ms": self.started.elapsed().as_secs_f64() * 1000.0,
            "kind": kind,
            "detail": detail,
        });
        if let Err(error) = writeln!(self.log, "{line}") {
            let message = format!("conformance log write failed: {error}");
            eprintln!("{message}");
            self.fatal = Some(message);
        }
        let show_guidance = kind != "exit"
            && !(kind == "cleanup"
                && matches!(
                    line["detail"]["reason"].as_str(),
                    Some("shutdown" | "capture_aborted" | "event_loop_exit")
                ));
        self.progress.observe(&line);
        self.records.push(line);
        if show_guidance {
            let current_scale = self
                .window
                .as_ref()
                .and_then(|window| analysis::known_scale(window.scale_factor()));
            let next = self
                .progress
                .next_action(self.gate.is_some(), current_scale);
            let guidance = format!("COVERAGE {}\nNEXT {next}", self.progress.summary());
            if guidance != self.last_guidance {
                println!("{guidance}");
                if let Some(window) = self.window.as_ref() {
                    window.set_title(&format!("SPEC-HAL-362 | {next}"));
                }
                self.last_guidance = guidance;
            }
        }
    }

    fn display(&self) -> Value {
        let Some(window) = self.window.as_ref() else {
            return Value::Null;
        };
        let monitor = window.current_monitor().map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            json!({
                "name": monitor.name(),
                "origin_physical": [position.x, position.y],
                "size_physical": [size.width, size.height],
                "scale": monitor.scale_factor(),
            })
        });
        json!({
            "window_scale": window.scale_factor(),
            "window_size_physical": [window.inner_size().width, window.inner_size().height],
            "monitor": monitor,
        })
    }

    fn quartz_location(&self) -> Option<[f64; 2]> {
        let event = CGEvent::new(self.quartz_source.as_ref()?.clone()).ok()?;
        let location = event.location();
        Some([location.x, location.y])
    }

    fn fused_detail(out: &[FusedInputEvent]) -> Value {
        let relative: Vec<[f32; 2]> = out
            .iter()
            .filter_map(|event| match event {
                FusedInputEvent::RelativePointerMotion { delta, .. } => {
                    Some([delta.x(), delta.y()])
                }
                _ => None,
            })
            .collect();
        let absolute_count = out
            .iter()
            .filter(|event| matches!(event, FusedInputEvent::PointerMove { .. }))
            .count();
        json!({"relative_deltas": relative, "absolute_move_count": absolute_count})
    }

    fn acquire(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if self.gate.is_some() {
            let result = self
                .platform
                .as_mut()
                .expect("window has platform")
                .acquire(WINDOW_ID);
            self.record(
                "lock_attempt",
                json!({
                    "result": format!("{result:?}"),
                    "already_locked": true,
                    "display": self.display(),
                }),
            );
            return;
        }
        let scale = window.scale_factor();
        let size = window.inner_size();
        let valid_anchor = self.last_absolute.is_some_and(|anchor| {
            anchor.x.is_finite()
                && anchor.y.is_finite()
                && anchor.x >= 0.0
                && anchor.y >= 0.0
                && (anchor.x as f64) < size.width as f64 / scale
                && (anchor.y as f64) < size.height as f64 / scale
        });
        if !self.focused || !valid_anchor {
            self.record(
                "lock_precondition_failed",
                json!({
                    "focused": self.focused,
                    "has_window_local_anchor": valid_anchor,
                }),
            );
            return;
        }
        let anchor = self.last_absolute.expect("valid anchor checked above");
        let monitor = window.current_monitor();
        let result = self
            .platform
            .as_mut()
            .expect("window has platform")
            .acquire(WINDOW_ID);
        self.record(
            "lock_attempt",
            json!({
                "result": format!("{result:?}"),
                "already_locked": self.gate.is_some(),
                "display": self.display(),
            }),
        );
        if result.is_err() {
            return;
        }
        let hide_result = self
            .platform
            .as_mut()
            .expect("window has platform")
            .hide_cursor(WINDOW_ID);
        self.record(
            "hide_attempt",
            json!({"result": format!("{hide_result:?}")}),
        );
        if hide_result.is_err() {
            self.cleanup("hide_failed", false);
            return;
        }

        let lease_id = self
            .lease_ids
            .mint()
            .expect("diagnostic lease ID exhausted");
        let epoch = RelativePointerNormalizationEpoch::new(1).expect("nonzero epoch");
        let session = GestureSessionId(1);
        let binding = RelativePointerLeaseBinding::try_new(
            lease_id,
            WINDOW_ID,
            InteractionDomainId::PRIMARY,
            PointerId::MOUSE,
            session,
            GestureCandidateId { session, local: 1 },
            TreeWidgetId(1),
            PointerButton::Primary,
            CursorRestorePolicy::DownAnchor,
            Some(anchor),
            epoch,
        )
        .expect("valid diagnostic ingress binding");
        self.gate = Some(RelativePointerIngressGate::from_binding(&binding));
        self.anchor = Some(anchor);
        self.anchor_monitor = monitor;
        self.anchor_scale = scale;
        self.record("lock_active", json!({
            "diagnostic_gate_only": true,
            "anchor_logical": [anchor.x, anchor.y],
            "lease_id": lease_id.get(),
            "window_scale": scale,
            "window_size_physical": [size.width, size.height],
            "quartz_location": self.quartz_location(),
            "support": format!("{:?}", self.platform.as_ref().expect("window has platform").support(WINDOW_ID)),
        }));
    }

    fn cleanup(&mut self, reason: &str, request_restore: bool) {
        let lease_id = self.gate.map(|gate| gate.lease_id().get());
        let was_active = self.gate.take().is_some();
        let mut flushed = Vec::new();
        self.fusion.flush(&mut flushed);
        let safe_restore = request_restore
            && was_active
            && self.focused
            && self.anchor.is_some()
            && self.anchor_monitor.is_some()
            && self.window.as_ref().is_some_and(|window| {
                window.current_monitor() == self.anchor_monitor
                    && window.scale_factor() == self.anchor_scale
            });
        let mut operations = Vec::new();
        let restore = if safe_restore {
            Some(
                self.platform
                    .as_mut()
                    .expect("active lock has platform")
                    .restore_cursor(WINDOW_ID, self.anchor.expect("safe anchor")),
            )
        } else {
            None
        };
        if let Some(result) = restore.as_ref() {
            operations.push(json!({"operation": "restore", "result": format!("{result:?}")}));
        }
        let release = self
            .platform
            .as_mut()
            .map(|platform| platform.release(WINDOW_ID));
        if let Some(result) = release.as_ref() {
            operations.push(json!({"operation": "release", "result": format!("{result:?}")}));
        }
        let show = self
            .platform
            .as_mut()
            .map(|platform| platform.show_cursor(WINDOW_ID));
        if let Some(result) = show.as_ref() {
            operations.push(json!({"operation": "show", "result": format!("{result:?}")}));
        }
        self.record(
            "cleanup",
            json!({
                "reason": reason,
                "lease_id": lease_id,
                "was_active": was_active,
                "restore_requested": request_restore,
                "restore_safe": safe_restore,
                "restore_result": restore.map(|result| format!("{result:?}")),
                "release_result": release.map(|result| format!("{result:?}")),
                "show_result": show.map(|result| format!("{result:?}")),
                "operations": operations,
                "flushed": Self::fused_detail(&flushed),
                "quartz_location_after": self.quartz_location(),
                "display": self.display(),
            }),
        );
        self.anchor = None;
        self.anchor_monitor = None;
        self.anchor_scale = 0.0;
        self.last_absolute = None;
    }

    fn absolute_motion(&mut self, physical: winit::dpi::PhysicalPosition<f64>) {
        let scale = self.window.as_ref().expect("window exists").scale_factor();
        let logical = Point::new((physical.x / scale) as f32, (physical.y / scale) as f32);
        let delta = self.last_absolute.map_or((0.0, 0.0), |previous| {
            (logical.x - previous.x, logical.y - previous.y)
        });
        self.last_absolute = Some(logical);
        let mut out = Vec::new();
        self.fusion.process_raw_with_relative_ingress(
            RawInputEvent::PointerMoved {
                window_id: WINDOW_ID,
                time: Instant::now(),
                device: PointerDevice::Mouse,
                position: logical,
                delta,
            },
            self.gate.as_ref(),
            &mut out,
        );
        self.fusion.flush(&mut out);
        self.record(
            "cursor_moved",
            json!({
                "physical_window": [physical.x, physical.y],
                "logical_window": [logical.x, logical.y],
                "logical_delta": [delta.0, delta.1],
                "window_scale": scale,
                "locked": self.gate.is_some(),
                "focused": self.focused,
                "lease_id": self.gate.map(|gate| gate.lease_id().get()),
                "fused": Self::fused_detail(&out),
                "quartz_location": self.quartz_location(),
            }),
        );
    }

    fn device_motion(&mut self, delta: (f64, f64)) {
        let gate = self.gate;
        let focused_window = self.focused.then_some(WINDOW_ID);
        let raw = relative_motion_from_device_event(
            gate,
            focused_window,
            WINDOW_ID,
            delta,
            Instant::now(),
        );
        let normalized = raw.as_ref().and_then(|raw| match raw {
            RawInputEvent::RelativePointerMotion { delta, .. } => Some([delta.0, delta.1]),
            _ => None,
        });
        let mut out = Vec::new();
        if let Some(raw) = raw {
            self.fusion
                .process_raw_with_relative_ingress(raw, gate.as_ref(), &mut out);
            self.fusion.flush(&mut out);
        }
        self.record(
            "device_mouse_motion",
            json!({
                "native_delta": [delta.0, delta.1],
                "normalized_logical_delta": normalized,
                "locked": gate.is_some(),
                "lease_id": gate.map(|gate| gate.lease_id().get()),
                "focused": self.focused,
                "window_scale": self.window.as_ref().map(|window| window.scale_factor()),
                "window_size_physical": self.window.as_ref().map(|window| [window.inner_size().width, window.inner_size().height]),
                "quartz_location": self.quartz_location(),
                "last_window_absolute_logical": self.last_absolute.map(|point| [point.x, point.y]),
                "fused": Self::fused_detail(&out),
            }),
        );
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(platform) = self.platform.as_mut() {
            let _ = platform.release(WINDOW_ID);
            let _ = platform.show_cursor(WINDOW_ID);
        }
    }
}

impl ApplicationHandler for Harness {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = WindowAttributes::default()
            .with_title("SPEC-HAL-362 macOS conformance: guided capture; Q finish, Z abort")
            .with_inner_size(PhysicalSize::new(900_u32, 600_u32));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fatal = Some(format!("window creation failed: {error}"));
                self.record(
                    "window_creation_failed",
                    json!({"error": error.to_string()}),
                );
                event_loop.exit();
                return;
            }
        };
        self.platform = Some(MacOsRelativePointerPlatform::new(WINDOW_ID, window.clone()));
        self.window = Some(window);
        self.record("window_created", json!({
            "display": self.display(),
            "quartz_source_available": self.quartz_source.is_some(),
            "support": format!("{:?}", self.platform.as_ref().expect("created platform").support(WINDOW_ID)),
        }));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if self
            .window
            .as_ref()
            .is_none_or(|window| window.id() != window_id)
        {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.cleanup("window_destroy", false);
                self.record("window_destroy", json!({}));
                self.platform = None;
                self.window = None;
                self.focused = false;
                self.resumed(event_loop);
            }
            WindowEvent::Focused(focused) => {
                let cursor_rects_refreshed = focused
                    && self
                        .window
                        .as_ref()
                        .is_some_and(|window| refresh_cursor_rects_after_focus(window));
                let mut out = Vec::new();
                self.fusion.process_raw_with_relative_ingress(
                    RawInputEvent::FocusChanged {
                        window_id: WINDOW_ID,
                        time: Instant::now(),
                        focused,
                    },
                    self.gate.as_ref(),
                    &mut out,
                );
                self.fusion.flush(&mut out);
                self.focused = focused;
                if !focused {
                    self.last_absolute = None;
                }
                self.record(
                    "focus_changed",
                    json!({
                        "focused": focused,
                        "cursor_rects_refreshed": cursor_rects_refreshed,
                        "fused": Self::fused_detail(&out),
                    }),
                );
                if !focused {
                    self.cleanup("focus_loss_or_app_switch", false);
                }
            }
            WindowEvent::CursorMoved { position, .. } => self.absolute_motion(position),
            WindowEvent::CursorLeft { .. } => {
                self.record("cursor_left_window", json!({"locked": self.gate.is_some()}));
                self.last_absolute = None;
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.last_absolute = None;
                self.record(
                    "scale_changed",
                    json!({
                        "new_scale": scale_factor,
                        "display": self.display(),
                    }),
                );
                if self.gate.is_some() {
                    self.cleanup("scale_changed", false);
                }
            }
            WindowEvent::Moved(position) => {
                self.last_absolute = None;
                self.record(
                    "window_moved",
                    json!({
                        "outer_position_physical": [position.x, position.y],
                        "display": self.display(),
                    }),
                );
                if self.gate.is_some()
                    && self
                        .window
                        .as_ref()
                        .is_some_and(|window| window.current_monitor() != self.anchor_monitor)
                {
                    self.cleanup("monitor_changed", false);
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyL) => self.acquire(),
                    PhysicalKey::Code(KeyCode::KeyR) => self.cleanup("down_anchor_restore", true),
                    PhysicalKey::Code(KeyCode::KeyU) => self.cleanup("explicit_release", false),
                    PhysicalKey::Code(KeyCode::KeyX) => {
                        self.cleanup("simulated_owner_unmount", false)
                    }
                    PhysicalKey::Code(KeyCode::KeyD) => {
                        self.cleanup("window_destroy", false);
                        self.record("window_destroy", json!({"keyboard_triggered": true}));
                        self.platform = None;
                        self.window = None;
                        self.focused = false;
                        self.resumed(event_loop);
                    }
                    PhysicalKey::Code(KeyCode::KeyQ) => {
                        if self.gate.is_some() {
                            self.cleanup("shutdown", false);
                            event_loop.exit();
                        } else {
                            println!("Q requires an active lock; move pointer inside and press L");
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyZ) => {
                        self.cleanup("capture_aborted", false);
                        event_loop.exit();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.device_motion(delta);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.cleanup("suspended", false);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.cleanup("event_loop_exit", false);
        self.record("exit", json!({"fatal": self.fatal}));
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::current_dir()?.join("target/relative_pointer_macos_conformance");
    fs::create_dir_all(&directory)?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let path: PathBuf = directory.join(format!("{timestamp}.jsonl"));
    let log = File::create(&path)?;
    println!("SPEC-HAL-362 native evidence: {}", path.display());
    println!(
        "Follow COVERAGE/NEXT in this terminal. L lock; move through every logical window edge while locked; hold primary button and drag for real locked CursorMoved samples; R restore; U release; X simulated unmount; D destroy/recreate; Cmd-Tab focus switch; Q record active-lock shutdown (partial coverage remains incomplete); Z abort incomplete capture."
    );

    let event_loop = EventLoop::new()?;
    event_loop.listen_device_events(DeviceEvents::Always);
    let mut harness = Harness::new(log);
    harness.record("start", json!({
        "spec": "SPEC-HAL-362",
        "native_backend": "macOS/winit 0.30.12",
        "quartz_location_unit": "CGEvent global display coordinates; recorded without conversion",
    }));
    event_loop.run_app(&mut harness)?;
    let verdicts = analysis::analyze(&harness.records);
    let report_path = path.with_extension("verdict.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&verdicts)?)?;
    println!("SPEC-HAL-362 verdict: {}", report_path.display());
    let failures: Vec<_> = verdicts
        .iter()
        .filter(|verdict| verdict.status == "FAIL")
        .map(|verdict| verdict.requirement)
        .collect();
    if failures.is_empty() {
        println!("Automated checks found no FAIL.");
    } else {
        println!("Automated FAIL: {}", failures.join(", "));
    }
    if harness.progress.complete() {
        println!("Recorded coverage complete. Human observations still required:");
        println!(
            "- Confirm the physical mouse remained usable after the numerically recorded virtual edge crossings."
        );
        println!(
            "- Confirm the cursor stayed visible and usable after cleanup, including across displays."
        );
        println!(
            "- Native OS denial and a real engine unmount need separate native/owner evidence; Cmd-Tab was recorded only as focus loss."
        );
    } else {
        println!("Capture INCOMPLETE; the report records the evidence obtained so far.");
    }
    if let Some(message) = harness.fatal.as_ref() {
        return Err(Box::new(io::Error::other(message.clone())));
    }
    if !harness.progress.complete() {
        return Err(Box::new(io::Error::other(
            "required recorded coverage is incomplete",
        )));
    }
    Ok(())
}

pub fn reanalyze(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
        return Err(
            io::Error::new(io::ErrorKind::InvalidInput, "expected a .jsonl capture").into(),
        );
    }

    let mut records = Vec::new();
    for (index, line) in BufReader::new(File::open(path)?).lines().enumerate() {
        let line = line?;
        let record: Value = serde_json::from_str(&line).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}:{}: {error}", path.display(), index + 1),
            )
        })?;
        if record["sequence"].as_u64() != Some(index as u64 + 1)
            || !record["kind"].is_string()
            || !record["detail"].is_object()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}:{}: invalid or noncontiguous capture record",
                    path.display(),
                    index + 1
                ),
            )
            .into());
        }
        records.push(record);
    }

    let first = records
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "capture contains no records"))?;
    let (verdicts, completion_label, complete) = match first["kind"].as_str() {
        Some("start")
            if first["detail"]["spec"] == "SPEC-HAL-362"
                && first["detail"]["native_backend"] == "macOS/winit 0.30.12"
                && !records[1..]
                    .iter()
                    .any(|record| record["kind"] == "engine_start") =>
        {
            let mut progress = analysis::CaptureProgress::default();
            for record in &records {
                progress.observe(record);
            }
            (
                analysis::analyze(&records),
                "Native coverage complete",
                progress.complete(),
            )
        }
        Some("engine_start")
            if first["detail"]["spec"] == "SPEC-HAL-362"
                && (first["detail"]["diagnostic_support_only"] == true
                    || first["detail"]["conformance_instrumented"] == true)
                && !records[1..].iter().any(|record| record["kind"] == "start") =>
        {
            let complete = records
                .iter()
                .any(|record| record["kind"] == "engine_run_return");
            (
                analysis::analyze_engine(&records),
                "Engine run returned",
                complete,
            )
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "capture does not identify an isolated SPEC-HAL-362 native or engine run",
            )
            .into());
        }
    };

    let report_path = path.with_extension("verdict.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&verdicts)?)?;
    println!("SPEC-HAL-362 reanalyzed: {}", path.display());
    println!("Report: {}", report_path.display());
    println!("{completion_label}: {complete}");
    for result in &verdicts {
        println!(
            "{}: {} — {}",
            result.status, result.requirement, result.detail
        );
    }
    if verdicts.iter().any(|result| result.status == "FAIL") {
        return Err(io::Error::other("reanalyzed capture contains FAIL verdicts").into());
    }
    Ok(())
}

#[cfg(debug_assertions)]
pub use engine::run_engine;
#[cfg(debug_assertions)]
pub(crate) use engine::{EngineConformancePlatform, EngineRecorder};

#[cfg(debug_assertions)]
mod engine {
    use std::fs::{self, File};
    use std::io::{self, Write};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    use serde_json::{Value, json};
    use winit::window::Window;

    use crate::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
    use crate::core::event::{
        Event, EventPhase, FusedInputEvent, GestureKind, InteractionDomainState, KeyCode,
        PointerEventKind, PointerId, WindowId,
    };
    use crate::core::layout::{Constraints, LayoutResult};
    use crate::core::math::{Point, RectF, Size};
    use crate::core::relative_pointer::{
        CursorRestorePolicy, PointerMotionPolicy, RelativePointerNormalizationEpoch,
        RelativePointerTerminalOutcome,
    };
    use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
    use crate::core::widget_tree::{NodeId, WidgetTree};
    use crate::platform::relative_pointer::{
        RelativePointerPlatform, RelativePointerPlatformError, RelativePointerPlatformSupport,
    };
    use crate::platform::relative_pointer_macos::MacOsRelativePointerPlatform;
    use crate::utils::id::{TreeWidgetId, WidgetInternalId};
    use crate::widgets::label::Label;

    use super::analysis;

    struct RecorderState {
        file: File,
        records: Vec<Value>,
        started: Instant,
        sequence: u64,
        frame: u64,
        last_state: Option<(bool, bool, bool)>,
        write_error: Option<String>,
    }

    #[derive(Clone)]
    pub(crate) struct EngineRecorder(Arc<Mutex<RecorderState>>);

    impl EngineRecorder {
        fn new(file: File) -> Self {
            Self(Arc::new(Mutex::new(RecorderState {
                file,
                records: Vec::new(),
                started: Instant::now(),
                sequence: 0,
                frame: 0,
                last_state: None,
                write_error: None,
            })))
        }

        pub(crate) fn set_frame(&self, frame: u64) {
            self.0.lock().expect("conformance recorder poisoned").frame = frame;
        }

        pub(crate) fn record(&self, kind: &str, detail: Value) {
            let mut state = self.0.lock().expect("conformance recorder poisoned");
            state.sequence += 1;
            let record = json!({
                "sequence": state.sequence,
                "frame": state.frame,
                "elapsed_ms": state.started.elapsed().as_secs_f64() * 1000.0,
                "kind": kind,
                "detail": detail,
            });
            if let Err(error) = writeln!(state.file, "{record}") {
                state.write_error.get_or_insert_with(|| error.to_string());
            }
            state.records.push(record);
        }

        pub(crate) fn record_state(&self, trigger: &str, state: &InteractionDomainState) {
            let active_lease = state
                .relative_pointer_authority
                .as_ref()
                .and_then(|authority| authority.active())
                .is_some();
            let capture = state.pointer_capture_manager.is_captured(PointerId::MOUSE);
            let session = state.gesture_registry.has_session(PointerId::MOUSE);
            let current = (active_lease, capture, session);
            {
                let mut recorder = self.0.lock().expect("conformance recorder poisoned");
                if recorder.last_state == Some(current)
                    && matches!(trigger, "route" | "relative_route")
                {
                    return;
                }
                recorder.last_state = Some(current);
            }
            self.record(
                "engine_state",
                json!({
                    "trigger": trigger,
                    "active_lease": active_lease,
                    "capture": capture,
                    "session": session,
                }),
            );
        }

        fn finish(&self) -> io::Result<Vec<Value>> {
            let mut state = self.0.lock().expect("conformance recorder poisoned");
            state.file.flush()?;
            if let Some(error) = state.write_error.as_ref() {
                return Err(io::Error::other(error.clone()));
            }
            Ok(state.records.clone())
        }
    }

    pub(crate) struct EngineConformancePlatform {
        native: MacOsRelativePointerPlatform<Arc<Window>>,
        window: Arc<Window>,
        recorder: EngineRecorder,
    }

    impl EngineConformancePlatform {
        pub(crate) fn new(
            native: MacOsRelativePointerPlatform<Arc<Window>>,
            window: Arc<Window>,
            recorder: EngineRecorder,
        ) -> Self {
            Self {
                native,
                window,
                recorder,
            }
        }

        fn record_operation(
            &self,
            operation: &str,
            result: Result<(), RelativePointerPlatformError>,
        ) {
            self.recorder.record(
                "engine_platform",
                json!({
                    "operation": operation,
                    "result": format!("{result:?}"),
                }),
            );
        }
    }

    impl RelativePointerPlatform for EngineConformancePlatform {
        fn support(&self, window_id: WindowId) -> RelativePointerPlatformSupport {
            let native_support = self.native.support(window_id);
            let production_capability_enabled = matches!(
                native_support,
                RelativePointerPlatformSupport::Supported { .. }
            );
            let scale = self.window.scale_factor();
            let result = if native_support
                == RelativePointerPlatformSupport::NormalizationUnavailable
                && scale.is_finite()
                && scale > 0.0
            {
                RelativePointerNormalizationEpoch::new(scale.to_bits())
                    .map(
                        |normalization_epoch| RelativePointerPlatformSupport::Supported {
                            normalization_epoch,
                            restore_down_anchor: true,
                        },
                    )
                    .unwrap_or(RelativePointerPlatformSupport::NormalizationUnavailable)
            } else {
                native_support
            };
            self.recorder.record(
                "engine_diagnostic_support",
                json!({
                    "native_support": format!("{native_support:?}"),
                    "diagnostic_support": format!("{result:?}"),
                    "window_scale": scale,
                    "production_capability_enabled": production_capability_enabled,
                    "diagnostic_support_override": !production_capability_enabled
                        && matches!(result, RelativePointerPlatformSupport::Supported { .. }),
                }),
            );
            result
        }

        fn acquire(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
            let result = self.native.acquire(window_id);
            self.record_operation("acquire", result);
            result
        }

        fn release(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
            let result = self.native.release(window_id);
            self.record_operation("release", result);
            result
        }

        fn hide_cursor(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
            let result = self.native.hide_cursor(window_id);
            self.record_operation("hide", result);
            result
        }

        fn show_cursor(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
            let result = self.native.show_cursor(window_id);
            self.record_operation("show", result);
            result
        }

        fn restore_cursor(
            &mut self,
            window_id: WindowId,
            anchor: Point,
        ) -> Result<(), RelativePointerPlatformError> {
            let result = self.native.restore_cursor(window_id, anchor);
            self.recorder.record(
                "engine_platform",
                json!({
                    "operation": "restore",
                    "result": format!("{result:?}"),
                    "anchor": [anchor.x, anchor.y],
                }),
            );
            result
        }
    }

    struct EngineProbe {
        id: WidgetInternalId,
        recorder: EngineRecorder,
        committed: bool,
    }

    impl WidgetDowncast for EngineProbe {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for EngineProbe {
        fn layout(
            &mut self,
            _: &mut LayoutContext,
            constraints: Constraints,
            _: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(constraints.constrain(Size::new(800.0, 500.0)))
        }

        fn paint(&self, _: &mut PaintContext, _: RectF, _: &[NodeId]) {}

        fn id(&self) -> WidgetInternalId {
            self.id
        }

        fn gesture_surface(&self) -> &'static [GestureKind] {
            &[GestureKind::Drag]
        }

        fn pointer_motion_policy(&self, _: GestureKind) -> PointerMotionPolicy {
            PointerMotionPolicy::RelativeLocked {
                restore: CursorRestorePolicy::DownAnchor,
            }
        }

        fn handle_event(
            &mut self,
            cx: &mut EventContext,
            event: &Event,
            phase: EventPhase,
        ) -> bool {
            if phase != EventPhase::Target {
                return false;
            }
            match event {
                Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                    if self.committed {
                        self.recorder
                            .record("absolute_move_after_commit", json!({}));
                    } else {
                        self.recorder.record("commit_intent", json!({}));
                        cx.request_gesture_commit();
                    }
                }
                Event::Pointer(pointer) if pointer.kind == PointerEventKind::Up => {
                    self.recorder.record("up", json!({}));
                    self.recorder.record("semantic_complete", json!({}));
                }
                Event::RelativePointerMotion(motion) => {
                    self.recorder.record(
                        "relative_motion",
                        json!({
                            "lease_id": motion.lease_id.get(),
                            "delta": [motion.delta.x(), motion.delta.y()],
                        }),
                    );
                }
                _ => {}
            }
            false
        }

        fn on_gesture_commit(&mut self, _: &mut EventContext, _: GestureKind) {
            self.committed = true;
            self.recorder.record("gesture_commit", json!({}));
        }

        fn on_relative_pointer_terminal(
            &mut self,
            _: &mut EventContext,
            _: GestureKind,
            outcome: RelativePointerTerminalOutcome,
        ) {
            self.committed = false;
            let (terminal, reason) = match outcome {
                RelativePointerTerminalOutcome::Completed { .. } => ("completed", None),
                RelativePointerTerminalOutcome::Cancelled { reason, .. } => {
                    ("cancelled", Some(format!("{reason:?}")))
                }
                RelativePointerTerminalOutcome::AdmissionDenied(reason) => {
                    ("admission_denied", Some(format!("{reason:?}")))
                }
            };
            self.recorder.record(
                "engine_terminal",
                json!({"terminal": terminal, "reason": reason, "outcome": format!("{outcome:?}")}),
            );
        }
    }

    struct EngineScreen {
        recorder: EngineRecorder,
        placeholder: Label,
        mounted: bool,
        rebuild: bool,
        owner: Option<TreeWidgetId>,
        pending_semantic_cancel: bool,
    }

    impl EngineScreen {
        fn new(recorder: EngineRecorder) -> Self {
            Self {
                recorder,
                placeholder: Label::new(""),
                mounted: true,
                rebuild: false,
                owner: None,
                pending_semantic_cancel: false,
            }
        }
    }

    impl Screen for EngineScreen {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let node = if self.mounted {
                cx.mount_child(Box::new(EngineProbe {
                    id: WidgetInternalId::new(),
                    recorder: self.recorder.clone(),
                    committed: false,
                }))
            } else {
                cx.mount_child(Box::new(Label::new("Owner unmounted; press M to remount")))
            };
            self.owner = self
                .mounted
                .then(|| cx.tree.widget_id_for_node(node))
                .flatten();
            self.recorder.record(
                "screen_build",
                json!({"mounted": self.mounted, "owner": self.owner.map(|id| id.0)}),
            );
            &mut self.placeholder
        }

        fn handle_input(&mut self, event: &FusedInputEvent, cx: &mut ScreenContext<'_>) -> bool {
            let FusedInputEvent::KeyDown {
                key_code: KeyCode::Character(key),
                ..
            } = event
            else {
                return false;
            };
            match key.to_ascii_uppercase() {
                'U' => {
                    self.recorder
                        .record("engine_trigger", json!({"trigger": "owner_unmount"}));
                    self.mounted = false;
                    self.rebuild = true;
                }
                'M' => {
                    self.recorder
                        .record("engine_trigger", json!({"trigger": "owner_remount"}));
                    self.mounted = true;
                    self.rebuild = true;
                }
                'C' => {
                    self.recorder
                        .record("engine_trigger", json!({"trigger": "semantic_cancel"}));
                    self.pending_semantic_cancel = true;
                }
                'Q' => {
                    self.recorder
                        .record("engine_trigger", json!({"trigger": "application_shutdown"}));
                    cx.request_exit();
                }
                _ => return false,
            }
            true
        }

        fn mark_rebuild(&mut self) {
            self.rebuild = true;
        }

        fn take_rebuild_flag(&mut self) -> bool {
            std::mem::take(&mut self.rebuild)
        }

        fn update(&mut self, tree: &mut WidgetTree) {
            if self.pending_semantic_cancel {
                self.pending_semantic_cancel = false;
                if let Some(owner) = self.owner {
                    tree.request_relative_pointer_semantic_cancel(owner, PointerId::MOUSE);
                    self.recorder
                        .record("semantic_cancel_intent", json!({"owner": owner.0}));
                }
            }
        }
    }

    pub fn run_engine() -> Result<(), Box<dyn std::error::Error>> {
        let directory = std::env::current_dir()?.join("target/relative_pointer_macos_conformance");
        fs::create_dir_all(&directory)?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        let path: PathBuf = directory.join(format!("{timestamp}.engine.jsonl"));
        let recorder = EngineRecorder::new(File::create(&path)?);
        println!(
            "SPEC-HAL-362 real-engine diagnostic evidence: {}",
            path.display()
        );
        println!(
            "Inside the window: primary-button drag to commit; move then release to complete; drag then U to unmount; M remount; drag then C for semantic cancel; drag then Cmd-Tab for focus loss; M remount if needed, drag then Q for shutdown. Observe cursor visibility after each cleanup."
        );
        recorder.record("engine_start", json!({
            "spec": "SPEC-HAL-362",
            "path": "App::run/EventRouter/RelativePointerLeaseAuthority/InputFusion/MacOsRelativePointerPlatform",
            "conformance_instrumented": true,
        }));
        let config = AppConfig::new("SPEC-HAL-362 real-engine diagnostic")
            .with_relative_pointer_engine_conformance(recorder.clone());
        App::new(config).run(EngineScreen::new(recorder.clone()));
        recorder.record("engine_run_return", json!({}));
        let records = recorder.finish()?;
        let verdicts = analysis::analyze_engine(&records);
        let report_path = path.with_extension("verdict.json");
        fs::write(&report_path, serde_json::to_vec_pretty(&verdicts)?)?;
        println!(
            "SPEC-HAL-362 real-engine verdict: {}",
            report_path.display()
        );
        for result in &verdicts {
            println!(
                "{}: {} — {}",
                result.status, result.requirement, result.detail
            );
        }
        if verdicts.iter().any(|result| result.status == "FAIL") {
            return Err(Box::new(io::Error::other(
                "real-engine diagnostic checks failed",
            )));
        }
        Ok(())
    }
}
