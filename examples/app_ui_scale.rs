//! Canonical App UI Scale demo. Run: `cargo run --example app_ui_scale`.
//! Non-default startup: `cargo run --example app_ui_scale -- --scale 1.25`.
//! Reuses the nine-color texture from `nine_patch_button`; no external assets.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};

use halcyon::core::app::{
    App, AppConfig, Screen, ScreenContext, ScreenFrameContext, UiScale, UiScaleRequestOutcome,
    UiScaleStatus, UiScaleTargetOutput,
};
use halcyon::core::appearance::{
    AppearanceConfigurationId, AppearanceConfigurationOwner, AppearanceFallbackPolicy,
    AppearanceImageSource, AppearanceSelector, AppearanceSurface, ButtonAppearanceConfiguration,
    ButtonAppearanceContract, ButtonAppearanceDefinition, ButtonAppearanceMetrics,
    ButtonMaterialPolicy, PreparedButtonStyle, ProductAppearanceOwner,
};
use halcyon::core::frame_realization::AppFrameObserver;
use halcyon::core::layout::{
    Alignment, Constraints, EdgeInsets, FlexParams, LayoutResult, Padding,
};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{
    Color, NinePatchDestinationInsets, NinePatchSourceInsets, SurfaceNinePatch,
};
use halcyon::core::theme::Theme;
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::renderer::image_binding::{ImageBinding, ImageBindingRequest};
use halcyon::utils::id::WidgetInternalId;
use halcyon::widgets::{button::Button, flex::Flex, label::Label, scroll_area::ScrollArea};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let initial = match args.next().as_deref() {
        None => UiScale::ONE,
        Some("--scale") => {
            let value = args.next().ok_or("--scale requires a number")?;
            UiScale::try_from(value.parse::<f32>().map_err(|error| error.to_string())?)
                .map_err(|error| format!("Invalid UI scale: {error:?}"))?
        }
        Some("--help") => {
            println!("cargo run --example app_ui_scale -- --scale 1.25");
            return Ok(());
        }
        Some(argument) => return Err(format!("Unknown argument: {argument}")),
    };
    if args.next().is_some() {
        return Err("Unexpected argument after scale".into());
    }
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();
    let (mut config, observer) = AppConfig::new("Halcyon — App UI Scale")
        .with_size(840.0, 620.0)
        .with_ui_scale(initial)
        .with_frame_realization_observer();
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let (sender, actions) = mpsc::channel();
    App::new(config).run(ScaleDemo {
        placeholder: Label::new(""),
        observer,
        sender,
        actions,
        nodes: None,
        request: None,
        binding: None,
        refresh: Rc::new(Cell::new(true)),
        previous: None,
        clicks: 0,
        rejection: None,
    });
    Ok(())
}

enum Action {
    Scale(UiScale),
    Activate,
}

#[derive(Clone, Copy)]
struct Nodes {
    admitted: NodeId,
    requested: NodeId,
    density: NodeId,
    clicks: NodeId,
    image: NodeId,
    nine_patch: NodeId,
}

struct ScaleDemo {
    placeholder: Label,
    observer: AppFrameObserver,
    sender: Sender<Action>,
    actions: Receiver<Action>,
    nodes: Option<Nodes>,
    request: Option<ImageBindingRequest>,
    binding: Option<ImageBinding>,
    refresh: Rc<Cell<bool>>,
    previous: Option<UiScaleStatus>,
    clicks: u64,
    rejection: Option<String>,
}

impl Screen for ScaleDemo {
    fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
        let colors = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
            [255, 255, 255, 255],
            [0, 255, 255, 255],
            [255, 0, 255, 255],
            [128, 128, 128, 255],
            [255, 128, 0, 255],
        ];
        let mut pixels = Vec::with_capacity(96 * 96 * 4);
        for y in 0..96 {
            for x in 0..96 {
                pixels.extend_from_slice(&colors[(y / 32) * 3 + x / 32]);
            }
        }
        self.request = Some(
            cx.app
                .gpu_resources()
                .expect("canonical App renderer")
                .queue_image_rgba8_srgb(1, 96, 96, pixels)
                .expect("existing nine-color sample"),
        );
    }

    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.nodes.is_some() {
            return &mut self.placeholder;
        }
        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_padding(EdgeInsets::all(20.0))
                .with_spacing(10.0),
        ));
        cx.tree.add_child(
            root,
            Box::new(Label::new("App UI Scale").with_size(24.0)),
            None,
            0,
        );
        add_label(
            cx.tree,
            root,
            "Change this App's scale independently of Windows display scaling.",
            1,
        );
        let controls = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 2);
        for (order, (caption, value)) in [("100%", 1.0), ("125%", 1.25), ("150%", 1.5)]
            .into_iter()
            .enumerate()
        {
            let scale = UiScale::try_from(value).expect("positive finite demo preset");
            let sender = self.sender.clone();
            cx.tree.add_child(
                controls,
                Box::new(Button::with_text(caption).on_click(move || {
                    sender
                        .send(Action::Scale(scale))
                        .expect("live demo action owner");
                })),
                None,
                order as i16,
            );
        }
        let admitted = add_label(cx.tree, root, "Admitted App scale S: waiting", 3);
        let requested = add_label(cx.tree, root, "Requested: none", 4);
        let density = add_label(
            cx.tree,
            root,
            "OS D / effective E: waiting for first frame",
            5,
        );
        add_label(
            cx.tree,
            root,
            "Scroll the clipped area below. Tab / Enter also activate Buttons.",
            6,
        );
        let scroll = cx.tree.add_child(
            root,
            Box::new(
                ScrollArea::new_unwired()
                    .with_scrollbars(true)
                    .enable_horizontal_scrollbar(false),
            ),
            None,
            7,
        );
        cx.tree.set_flex(scroll, FlexParams::flexible(1.0));
        let content = cx.tree.add_child(
            scroll,
            Box::new(
                Flex::column()
                    .with_padding(EdgeInsets::all(12.0))
                    .with_spacing(14.0),
            ),
            None,
            0,
        );
        cx.tree
            .node_mut(scroll)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(content);
        add_label(
            cx.tree,
            content,
            "One existing texture: plain Image and stretch NinePatch",
            0,
        );
        let pictures = cx.tree.add_child(
            content,
            Box::new(
                Flex::row()
                    .with_align_items(Alignment::Start)
                    .with_spacing(24.0),
            ),
            None,
            1,
        );
        let image = cx.tree.add_child(
            pictures,
            Box::new(ImagePreview {
                id: WidgetInternalId::new(),
                binding: None,
                refresh: self.refresh.clone(),
            }),
            None,
            0,
        );
        let sender = self.sender.clone();
        let nine_patch = cx.tree.add_child(
            pictures,
            Box::new(Button::with_text("NinePatch").on_click(move || {
                sender
                    .send(Action::Activate)
                    .expect("live demo action owner");
            })),
            None,
            1,
        );
        let clicks = add_label(cx.tree, content, "NinePatch activations: 0", 2);
        for row in 1..=16 {
            add_label(
                cx.tree,
                content,
                format!("Row {row:02} — text, image borders and hit geometry share the App scale."),
                2 + row,
            );
        }
        add_label(
            cx.tree,
            root,
            "Close the window to exit. Scale changes preserve the mounted view.",
            8,
        );
        self.nodes = Some(Nodes {
            admitted,
            requested,
            density,
            clicks,
            image,
            nine_patch,
        });
        &mut self.placeholder
    }

    fn take_rebuild_flag(&mut self) -> bool {
        self.nodes.is_none()
    }

    fn startup_ready(&self) -> Result<bool, String> {
        Ok(self.binding.is_some())
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let Some(nodes) = self.nodes else { return };
        while let Ok(action) = self.actions.try_recv() {
            match action {
                Action::Scale(scale) => {
                    self.rejection = cx
                        .app
                        .request_ui_scale(scale)
                        .err()
                        .map(|reason| format!("Rejected: {reason:?}"));
                    self.refresh.set(true);
                }
                Action::Activate => {
                    self.clicks += 1;
                    println!("app_ui_scale: NinePatch activations={}", self.clicks);
                }
            }
        }
        let primary = self.observer.latest().and_then(|receipt| {
            receipt
                .surfaces()
                .first()
                .map(|surface| surface.surface_id())
        });
        let status = cx.app.ui_scale_status(primary);
        let Some(tree) = cx.tree.as_deref_mut() else {
            return;
        };
        if let Some(request) = &self.request {
            if let Some(binding) = request.try_resolve().expect("demo image admission") {
                tree.node_mut(nodes.image)
                    .unwrap()
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<ImagePreview>()
                    .unwrap()
                    .binding = Some(binding.clone());
                tree.mark_paint_dirty(nodes.image);
                tree.node_mut(nodes.nine_patch)
                    .unwrap()
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<Button>()
                    .unwrap()
                    .set_appearance(nine_patch_style(&cx.app.theme, &binding), None)
                    .expect("existing admitted NinePatch Button profile");
                self.binding = Some(binding);
                self.request = None;
            }
        }
        set_label(
            tree,
            nodes.admitted,
            format!(
                "Admitted App scale S = {:.2}  |  revision {}",
                status.admitted.get(),
                status.admitted_revision
            ),
        );
        let requested = self
            .rejection
            .clone()
            .unwrap_or_else(|| match status.requested {
                Some(request) => format!(
                    "Requested {:.2}  |  #{} {:?}",
                    request.value.get(),
                    request.request_id,
                    request.outcome
                ),
                None => "Requested: none (initial configuration)".into(),
            });
        set_label(tree, nodes.requested, requested);
        let density = status
            .target
            .and_then(|target| target.effective)
            .map(|target| {
                format!(
                    "OS D = {:.2}  |  effective E = {:.2}  |  P = {} × {}  |  target rev {}",
                    target.platform_density,
                    target.effective_scale_factor,
                    target.physical_size.0,
                    target.physical_size.1,
                    target.admitted_revision,
                )
            })
            .unwrap_or_else(|| "OS D / effective E: waiting for first frame".into());
        set_label(tree, nodes.density, density);
        set_label(
            tree,
            nodes.clicks,
            format!("NinePatch activations: {}", self.clicks),
        );
        self.refresh.set(
            self.binding.is_none()
                || status
                    .requested
                    .is_some_and(|request| request.outcome == UiScaleRequestOutcome::Queued)
                || status.target.is_none_or(|target| {
                    target.output == UiScaleTargetOutput::Pending
                        || target.effective.is_none_or(|effective| {
                            effective.admitted_revision != status.admitted_revision
                        })
                }),
        );
        if self.previous != Some(status) {
            println!("app_ui_scale: {status:?}");
            self.previous = Some(status);
        }
    }
}

fn add_label(tree: &mut WidgetTree, parent: NodeId, text: impl Into<String>, order: i16) -> NodeId {
    tree.add_child(parent, Box::new(Label::new(text)), None, order)
}

fn set_label(tree: &mut WidgetTree, node: NodeId, text: String) {
    let label = tree
        .node_mut(node)
        .unwrap()
        .widget_mut()
        .as_any_mut()
        .downcast_mut::<Label>()
        .unwrap();
    if label.text != text {
        label.set_text(text);
        tree.mark_layout_dirty(node);
    }
}

fn nine_patch_style(theme: &Theme, binding: &ImageBinding) -> PreparedButtonStyle {
    let contract = ButtonAppearanceContract::standard();
    let mut values = theme.button_appearance_values(&contract);
    values.metrics = Some(ButtonAppearanceMetrics {
        padding: Padding::symmetric(48.0, 28.0),
    });
    for paint in [
        &mut values.resting,
        &mut values.hovered,
        &mut values.pressed,
        &mut values.disabled,
        &mut values.selected,
    ]
    .into_iter()
    .flatten()
    {
        paint.surface = AppearanceSurface::NinePatch(
            SurfaceNinePatch::new(
                AppearanceImageSource::Admitted(binding.clone()),
                RectF::new(0.0, 0.0, 1.0, 1.0),
                NinePatchSourceInsets::all(1.0 / 3.0),
                NinePatchDestinationInsets::new(12.0, 12.0, 12.0, 12.0),
                Color::WHITE,
            )
            .expect("existing normalized sample material"),
        );
        paint.content_color = Color::BLACK;
        paint.radius = 0.0;
        paint.underlay = None;
        paint.border = None;
        paint.overlay = None;
    }
    ButtonAppearanceConfiguration::new(
        AppearanceConfigurationId {
            owner: AppearanceConfigurationOwner::Product(ProductAppearanceOwner(1)),
            generation: 1,
        },
        contract,
        vec![ButtonAppearanceDefinition {
            selector: AppearanceSelector::button(),
            values,
        }],
        Vec::new(),
    )
    .expect("fixed demo appearance")
    .prepare_style(
        AppearanceSelector::button(),
        AppearanceFallbackPolicy::Reject,
    )
    .expect("supported admitted Button material")
}

struct ImagePreview {
    id: WidgetInternalId,
    binding: Option<ImageBinding>,
    refresh: Rc<Cell<bool>>,
}

impl WidgetDowncast for ImagePreview {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ImagePreview {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn layout(
        &mut self,
        _: &mut LayoutContext,
        constraints: Constraints,
        _: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(constraints.constrain(Size::new(96.0, 96.0)))
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        if let Some(binding) = &self.binding {
            cx.scene.push_admitted_image(
                rect,
                binding,
                RectF::new(0.0, 0.0, 1.0, 1.0),
                Color::WHITE,
                0,
                None,
            );
        }
    }
    fn needs_update(&self) -> bool {
        self.refresh.get()
    }
}
