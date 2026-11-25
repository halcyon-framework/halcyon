//! Local browser transport probe using the ordinary Halcyon App and widgets.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::frame_realization::{AppFrameAttemptId, AppFrameObserver};
use halcyon::core::layout::EdgeInsets;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::core::widget_tree::ControlSubtreePlacement;
use halcyon::model::form_binder::FormBinder;
use halcyon::widgets::{button::Button, container::Container, flex::Flex, label::Label, text_field::TextField};

#[allow(dead_code)]
#[path = "frosted_glass.rs"]
mod frosted_glass;

struct LocalProbe {
    binder: FormBinder,
    clicks: u32,
    status: Option<NodeId>,
    pending_status: Option<String>,
    rebuild: bool,
    placeholder: Label,
    observer: AppFrameObserver,
    observed_attempt: Option<AppFrameAttemptId>,
}

impl LocalProbe {
    fn new(observer: AppFrameObserver) -> Self {
        Self {
            binder: FormBinder::new(),
            clicks: 0,
            status: None,
            pending_status: None,
            rebuild: true,
            placeholder: Label::new(""),
            observer,
            observed_attempt: None,
        }
    }
}

impl Screen for LocalProbe {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_padding(EdgeInsets::all(32.0))
                .with_spacing(16.0),
        ));
        cx.tree.add_child(
            root,
            Box::new(Label::new("Halcyon").with_size(30.0)),
            None,
            0,
        );
        cx.tree
            .add_child(root, Box::new(Label::new("Local browser probe")), None, 1);
        cx.tree.add_child(
            root,
            Box::new(TextField::new().with_text("Browser input")),
            None,
            2,
        );
        let writer = self.binder.writer();
        let pressed = writer.intern("pressed");
        cx.tree.add_child(
            root,
            Box::new(Button::with_text("Test Halcyon button").on_click(move || {
                writer.set_bool(pressed, true);
            })),
            None,
            3,
        );
        self.status = Some(cx.tree.add_child(
            root,
            Box::new(Label::new(format!("Clicks: {}", self.clicks))),
            None,
            4,
        ));
        let glass = cx.tree.add_child(
            root,
            Box::new(Container::empty().with_height(400.0)),
            None,
            5,
        );
        let comparison = frosted_glass::Comparison::new();
        #[cfg(target_arch = "wasm32")]
        let comparison = comparison.with_probe_bloom();
        cx.tree.add_child(glass, Box::new(comparison), None, 0);
        let order = cx.tree.children(root).to_vec();
        cx.tree.set_control_subtree_order(
            root, &order, ControlSubtreePlacement::Main { z_index: 0 },
        ).expect("probe's current complete child order");
        &mut self.placeholder
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        if let Some(receipt) = self.observer.latest()
            && self.observed_attempt != Some(receipt.attempt_id())
        {
            self.observed_attempt = Some(receipt.attempt_id());
            #[cfg(target_arch = "wasm32")]
            browser_diagnostic(&format!("\nActual Halcyon frame: {receipt:?}\n"));
        }
        let mut pressed = false;
        self.binder.bind(cx, |binding| {
            pressed = binding.bool("pressed").take().unwrap_or(false);
        });
        if pressed {
            self.clicks = self
                .clicks
                .checked_add(1)
                .expect("probe click count overflow");
            self.pending_status = Some(format!("Clicks: {}", self.clicks));
        }
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        if let (Some(node), Some(text)) = (self.status, self.pending_status.take()) {
            tree.node_mut(node)
                .expect("current status mount")
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<Label>()
                .expect("status Label")
                .set_text(text);
            tree.mark_layout_dirty(node);
        }
    }

    fn mark_rebuild(&mut self) {
        self.rebuild = true;
    }
    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.rebuild)
    }
}

/// Runs the same retained tree through the platform's ordinary App transport.
pub fn run_probe() {
    let (mut config, observer) = AppConfig::new("Halcyon — Local browser probe")
        .with_size(760.0, 480.0)
        .with_frame_realization_observer();
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    App::new(config).run(LocalProbe::new(observer));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn browser_start() {
    if let Err(error) = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_max_level(tracing::Level::WARN)
        .with_writer(|| BrowserDiagnostics)
        .try_init()
    {
        browser_diagnostic(&error.to_string());
    }
    std::panic::set_hook(Box::new(|info| {
        browser_diagnostic(&info.to_string());
    }));
    run_probe();
}

#[cfg(target_arch = "wasm32")]
struct BrowserDiagnostics;

#[cfg(target_arch = "wasm32")]
impl std::io::Write for BrowserDiagnostics {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        browser_diagnostic(&String::from_utf8_lossy(bytes));
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
fn browser_diagnostic(message: &str) {
    web_sys::console::error_1(&message.into());
    if let Some(output) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("boot-error"))
    {
        let previous = output.text_content().unwrap_or_default();
        let bounded: String = previous
            .chars()
            .chain(message.chars())
            .take(16_384)
            .collect();
        output.set_text_content(Some(&bounded));
        if let Err(error) = output.remove_attribute("hidden") {
            web_sys::console::error_1(&error);
        }
    }
}
