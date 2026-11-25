//! HAL333 §7: native action -> ordinary App return -> caller-owned cleanup.
use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::frame_realization::{AppFrameObserver, AppFrameRealization};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::Color;
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::model::menu_model::MenuModel;
use halcyon::utils::id::WidgetInternalId;
use objc2::sel;
use objc2_app_kit::{NSApplication, NSMenu};
use objc2_foundation::MainThreadMarker;
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing_subscriber::prelude::*;

// Reuse AppKit's existing main queue, not a test thread/timer or an App mode.
#[link(name = "System")]
unsafe extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(
        queue: *const c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
}

struct State {
    case: String,
    observer: AppFrameObserver,
    scheduled: Cell<bool>,
    action_returned: Cell<bool>,
    drops: Cell<u32>,
    cleanup: Cell<u32>,
    paints: Cell<u32>,
    flushes: Cell<u32>,
    before: RefCell<Option<Arc<AppFrameRealization>>>,
    paints_before: Cell<u32>,
    flushes_before: Cell<u32>,
}

fn assert_startup_roles(app: &NSApplication) {
    // Inspect this ordinary App's installed shell before its first Screen build;
    // this is not a fabricated MenuModel or a second native host.
    unsafe {
        let main = app
            .mainMenu()
            .expect("standard roles exist before the first native frame");
        let menu = main.itemAtIndex(0).unwrap().submenu().unwrap();
        for action in [
            sel!(orderFrontStandardAboutPanel:),
            sel!(hide:),
            sel!(hideOtherApplications:),
            sel!(unhideAllApplications:),
            sel!(halcyonApplicationQuit:),
        ] {
            assert!(
                menu.itemArray()
                    .iter()
                    .any(|item| item.action() == Some(action)),
                "missing startup role {action:?}"
            );
        }
        let services = menu
            .itemArray()
            .iter()
            .find(|item| item.title().to_string() == "Services")
            .unwrap()
            .submenu()
            .unwrap();
        let registered = app
            .servicesMenu()
            .expect("startup Services registration preserved");
        assert!(std::ptr::eq::<NSMenu>(&*services, &*registered));
        let (submenu, index) = native_quit_item(&main).unwrap();
        let quit = submenu.itemAtIndex(index).unwrap();
        assert!(quit.target().is_some());
        assert_eq!(
            quit.keyEquivalentModifierMask(),
            objc2_app_kit::NSCommandKeyMask
        );
        println!(
            "HAL333_QUIT startup_roles=About,Services,Hide,HideOthers,ShowAll,Quit registered_services=true"
        );
    }
}

fn native_quit_item(menu: &NSMenu) -> Option<(objc2::rc::Retained<NSMenu>, isize)> {
    // All callers are on this child application's main thread.
    unsafe {
        for item in menu.itemArray().iter() {
            if let Some(submenu) = item.submenu() {
                for (index, candidate) in submenu.itemArray().iter().enumerate() {
                    if candidate.keyEquivalent().to_string() == "q" && candidate.action().is_some()
                    {
                        return Some((submenu.clone(), index as isize));
                    }
                }
            }
        }
    }
    None
}

extern "C" fn activate_native_quit(context: *mut c_void) {
    // The only retained payload is this child fixture's evidence state. There
    // is no AppContext/tree reference and no call to request_exit or a latch.
    let state = unsafe { Box::from_raw(context.cast::<Rc<State>>()) };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        activate_native_quit_inner(&state)
    }));
    if result.is_err() {
        eprintln!(
            "HAL333_QUIT native proof callback failed; parent must reject absent return/cleanup markers"
        );
    }
}

fn activate_native_quit_inner(state: &State) {
    let mtm = MainThreadMarker::new().expect("native menu proof main queue");
    let app = NSApplication::sharedApplication(mtm);
    let mut hidden = false;
    if state.case == "hidden" {
        let windows = unsafe { app.windows() };
        let window = windows
            .iter()
            .find(|window| {
                unsafe { window.title() }.to_string() == "Native Quit caller-return proof"
            })
            .expect("this child fixture's own native window");
        window.orderOut(None);
        hidden = !window.isVisible();
        assert!(hidden, "owned native window did not become hidden");
    }
    let menu = unsafe { app.mainMenu() }.expect("ordinary App installed a native main menu");
    let (submenu, index) = native_quit_item(&menu).expect("actual installed standard Quit");
    *state.before.borrow_mut() = state.observer.latest();
    state.paints_before.set(state.paints.get());
    state.flushes_before.set(state.flushes.get());
    println!(
        "HAL333_QUIT native_action_enter case={} hidden={} selector={:?}",
        state.case,
        hidden,
        unsafe { submenu.itemAtIndex(index).unwrap().action() }
    );
    std::io::stdout()
        .flush()
        .expect("flush native action ingress");
    unsafe { submenu.performActionForItemAtIndex(index) };
    state.action_returned.set(true);
    println!("HAL333_QUIT native_action_return case={}", state.case);
    std::io::stdout()
        .flush()
        .expect("flush native callback return");
}

struct Leaf {
    id: WidgetInternalId,
    state: Rc<State>,
}
impl WidgetDowncast for Leaf {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl Widget for Leaf {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn needs_update(&self) -> bool {
        !self.state.scheduled.get()
    }
    fn layout(&mut self, _: &mut LayoutContext, c: Constraints, _: &[NodeId]) -> LayoutResult {
        LayoutResult::fit(c.constrain(Size::new(320.0, 180.0)))
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        self.state.paints.set(self.state.paints.get() + 1);
        cx.scene.push_rect(rect, Color::from_hex(0x203044), 0, None);
    }
}
struct QuitScreen {
    stub: Leaf,
    state: Rc<State>,
}
impl Screen for QuitScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        assert_startup_roles(&NSApplication::sharedApplication(
            MainThreadMarker::new().unwrap(),
        ));
        assert!(
            cx.app.ui_waker().is_some(),
            "ordinary native App must supply its actual unit waker"
        );
        if self.state.case == "projected" {
            let mut model = MenuModel::new();
            model.add_title("Quit proof product", Vec::new());
            cx.app.set_menu_model(model);
        }
        cx.mount_child(Box::new(Leaf {
            id: WidgetInternalId::new(),
            state: self.state.clone(),
        }));
        &mut self.stub
    }
    fn flush_pending_actions(&mut self, _: &mut ScreenContext<'_>) {
        self.state.flushes.set(self.state.flushes.get() + 1);
        if self.state.action_returned.get() {
            println!(
                "HAL333_QUIT screen_flush_after_action count={}",
                self.state.flushes.get()
            );
        }
        if self.state.scheduled.get() {
            return;
        }
        let Some(receipt) = self.state.observer.latest() else {
            return;
        };
        if !receipt
            .surfaces()
            .iter()
            .any(|s| s.submitted() && s.presented_called())
        {
            return;
        }
        let mtm = MainThreadMarker::new().expect("Screen main thread");
        let app = NSApplication::sharedApplication(mtm);
        let Some(menu) = (unsafe { app.mainMenu() }) else {
            return;
        };
        if self.state.case == "projected" {
            assert!(
                unsafe { menu.itemArray() }
                    .iter()
                    .any(|i| unsafe { i.title() }.to_string() == "Quit proof product")
            );
        }
        assert!(native_quit_item(&menu).is_some());
        self.state.scheduled.set(true);
        let context = Box::into_raw(Box::new(self.state.clone())).cast::<c_void>();
        unsafe {
            dispatch_async_f(
                std::ptr::addr_of!(_dispatch_main_q),
                context,
                activate_native_quit,
            );
        }
    }
}
impl Drop for QuitScreen {
    fn drop(&mut self) {
        self.state.drops.set(self.state.drops.get() + 1);
        println!(
            "HAL333_QUIT screen_drop count={} action_returned={}",
            self.state.drops.get(),
            self.state.action_returned.get()
        );
    }
}

pub(super) fn run_child(case: &str) {
    assert!(matches!(case, "initial" | "projected" | "hidden"));
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .without_time()
            .with_filter(tracing_subscriber::filter::filter_fn(|m| {
                m.target() == "halcyon::native_quit"
            })),
    );
    let _subscriber = tracing::subscriber::set_default(subscriber);
    let (config, observer) = AppConfig::new("Native Quit caller-return proof")
        .with_size(320.0, 180.0)
        .with_frame_realization_observer();
    let state = Rc::new(State {
        case: case.into(),
        observer,
        scheduled: Cell::new(false),
        action_returned: Cell::new(false),
        drops: Cell::new(0),
        cleanup: Cell::new(0),
        paints: Cell::new(0),
        flushes: Cell::new(0),
        before: RefCell::new(None),
        paints_before: Cell::new(0),
        flushes_before: Cell::new(0),
    });
    let screen = QuitScreen {
        stub: Leaf {
            id: WidgetInternalId::new(),
            state: state.clone(),
        },
        state: state.clone(),
    };
    App::new(config).run(screen);
    println!("HAL333_QUIT app_run_return case={case}");
    assert!(state.action_returned.get());
    assert_eq!(
        state.drops.get(),
        1,
        "original Screen must be destroyed before caller cleanup"
    );
    let after = state
        .observer
        .latest()
        .expect("baseline receipt retained after return");
    let before = state.before.borrow();
    assert!(
        Arc::ptr_eq(before.as_ref().unwrap(), &after),
        "Quit required another frame receipt"
    );
    assert_eq!(
        state.paints_before.get(),
        state.paints.get(),
        "Quit required another Paint"
    );
    assert_eq!(
        state.flushes_before.get(),
        state.flushes.get(),
        "terminal Quit dispatched another Screen flush"
    );
    println!(
        "HAL333_QUIT no_frame_after_action=true hidden={} attempt={:?}",
        case == "hidden",
        after.attempt_id()
    );
    // An explicit ordinary caller operation, not a Drop substitute or worker ACK.
    state.cleanup.set(state.cleanup.get() + 1);
    println!("HAL333_QUIT caller_cleanup count={}", state.cleanup.get());
    assert_eq!(state.cleanup.get(), 1);
}

pub(super) fn run_parent() {
    let root = std::env::temp_dir().join(format!(
        "hal333-native-quit-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    std::fs::create_dir(&root).expect("exclusive native evidence directory");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let executable = std::env::current_exe().expect("exact current fixture executable");
    let mut failures = Vec::new();
    for case in ["initial", "projected", "hidden"] {
        let path = root.join(format!("{case}.log"));
        let log = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        let mut child = Command::new(&executable)
            .args(["native_quit_return_child", case])
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .expect("owned native child");
        // Only parent process supervision waits; App has no timer/polling path.
        let deadline = Instant::now() + Duration::from_secs(25);
        let status = loop {
            if let Some(status) = child.try_wait().expect("owned child status") {
                break Some(status);
            }
            if Instant::now() >= deadline {
                child.kill().expect("stop only timed-out owned child");
                child.wait().unwrap();
                break None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let log = std::fs::read_to_string(&path).unwrap();
        let markers = [
            "HAL333_QUIT native_action_enter",
            "HAL333_QUIT native_action_return",
            "HAL333_QUIT core_consumed",
            "HAL333_QUIT native_stop_called",
            "HAL333_QUIT screen_drop count=1 action_returned=true",
            "HAL333_QUIT app_run_return",
            "HAL333_QUIT no_frame_after_action=true",
            "HAL333_QUIT caller_cleanup count=1",
        ];
        let mut last = 0;
        let mut valid = log.len() <= 65536 && status.is_some_and(|s| s.success());
        for marker in markers {
            let positions: Vec<_> = log.match_indices(marker).map(|(at, _)| at).collect();
            if positions.len() != 1 || positions[0] < last {
                valid = false;
            } else {
                last = positions[0];
            }
        }
        if case == "hidden" && !log.contains("case=hidden hidden=true") {
            valid = false;
        }
        println!(
            "HAL333_QUIT case={case} child={status:?} ordered_return_proof={valid} evidence={}",
            path.display()
        );
        if !valid {
            failures.push(case);
        }
    }
    assert!(
        failures.is_empty(),
        "HAL333 native Quit failed {failures:?}; exit0 without post-return markers is not success; {}",
        root.display()
    );
}
