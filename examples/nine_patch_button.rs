//! Native SPEC-R-019 acceptance through App's renderer and ordinary Buttons.
//! Run the compiled `target/debug/examples/nine_patch_button` executable.

use std::sync::mpsc::{self, Receiver, Sender};

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::{
    AppearanceConfigurationId, AppearanceConfigurationOwner, AppearanceFallbackPolicy,
    AppearanceImageSource, AppearanceSelector, AppearanceSurface, ButtonAppearanceConfiguration,
    ButtonAppearanceContract, ButtonAppearanceDefinition, ButtonAppearanceMetrics,
    PreparedButtonStyle, ProductAppearanceOwner,
};
use halcyon::core::layout::{Alignment, EdgeInsets, Padding};
use halcyon::core::math::RectF;
use halcyon::core::paint::{
    Color, NinePatchDestinationInsets, NinePatchSourceInsets, SurfaceNinePatch,
};
use halcyon::core::theme::Theme;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::NodeId;
use halcyon::renderer::image_binding::{ImageBinding, ImageBindingRequest};
use halcyon::widgets::{button::Button, flex::Flex, label::Label};

const COLORS: [[u8; 4]; 9] = [
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

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();
    let (sender, clicks) = mpsc::channel();
    App::new(
        AppConfig::new("Stretch NinePatch — native Button acceptance").with_size(800.0, 600.0),
    )
    .run(NinePatchDemo {
        placeholder: Label::new(""),
        nodes: None,
        request: None,
        sender,
        clicks,
        activation_count: 0,
    });
}

fn style(
    theme: &Theme,
    binding: &ImageBinding,
    padding: Padding,
    owner: ProductAppearanceOwner,
) -> PreparedButtonStyle {
    let material = |tint| {
        SurfaceNinePatch::new(
            AppearanceImageSource::Admitted(binding.clone()),
            RectF::new(0.0, 0.0, 1.0, 1.0),
            NinePatchSourceInsets::all(1.0 / 3.0),
            NinePatchDestinationInsets::new(60.0, 80.0, 40.0, 120.0),
            tint,
        )
        .expect("fixed normalized NinePatch value")
    };
    let contract = ButtonAppearanceContract::standard();
    let mut values = theme.button_appearance_values(&contract);
    values.metrics = Some(ButtonAppearanceMetrics { padding });
    for (paint, tint) in [
        (&mut values.resting, Color::WHITE),
        (
            &mut values.hovered,
            Color::from_linear(0.08, 1.0, 0.08, 1.0),
        ),
        (
            &mut values.pressed,
            Color::from_linear(0.08, 0.08, 1.0, 1.0),
        ),
        (
            &mut values.disabled,
            Color::from_linear(0.18, 0.18, 0.18, 1.0),
        ),
        (
            &mut values.selected,
            Color::from_linear(1.0, 0.08, 0.65, 1.0),
        ),
    ] {
        if let Some(paint) = paint.as_mut() {
            paint.surface = AppearanceSurface::NinePatch(material(tint));
            paint.radius = 0.0;
            paint.underlay = None;
            paint.border = None;
            paint.overlay = None;
        }
    }
    ButtonAppearanceConfiguration::new(
        AppearanceConfigurationId {
            owner: AppearanceConfigurationOwner::Product(owner),
            generation: 1,
        },
        contract,
        vec![ButtonAppearanceDefinition {
            selector: AppearanceSelector::button(),
            values,
        }],
        Vec::new(),
    )
    .expect("complete fixed Button configuration")
    .prepare_style(
        AppearanceSelector::button(),
        AppearanceFallbackPolicy::Reject,
    )
    .expect("complete fixed Button style")
}

struct NinePatchDemo {
    placeholder: Label,
    nodes: Option<([NodeId; 2], NodeId)>,
    request: Option<ImageBindingRequest>,
    sender: Sender<()>,
    clicks: Receiver<()>,
    activation_count: u64,
}

impl Screen for NinePatchDemo {
    fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
        let mut pixels = Vec::with_capacity(96 * 96 * 4);
        for y in 0..96 {
            for x in 0..96 {
                pixels.extend_from_slice(&COLORS[(y / 32) * 3 + x / 32]);
            }
        }
        self.request = Some(
            cx.app
                .gpu_resources()
                .expect("App owns the native renderer")
                .queue_image_rgba8_srgb(1, 96, 96, pixels)
                .expect("valid owned decoded RGBA8 pixels"),
        );
    }

    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.nodes.is_some() {
            return &mut self.placeholder;
        }
        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_align_items(Alignment::Start)
                .with_padding(EdgeInsets::all(28.0))
                .with_spacing(18.0),
        ));
        for (index, text) in [
            "Stretch NinePatch / resting: original colors; hover: green; pressed: blue.",
            "Top: red / green / blue. Middle: yellow / white / cyan.",
            "Bottom: magenta / gray / orange. Tab: focus; Enter / Space: activate.",
            "Normal / asymmetric borders: left 120, right 80, top 60, bottom 40 logical units.",
        ]
        .into_iter()
        .enumerate()
        {
            cx.tree
                .add_child(root, Box::new(Label::new(text)), None, index as i16);
        }
        let mut buttons = Vec::with_capacity(2);
        for index in 0..2 {
            if index == 1 {
                cx.tree.add_child(
                    root,
                    Box::new(Label::new(
                        "Small / both centers collapse; four corners remain.",
                    )),
                    None,
                    5,
                );
            }
            let sender = self.sender.clone();
            buttons.push(cx.tree.add_child(
                root,
                Box::new(Button::with_text("").on_click(move || {
                    sender.send(()).expect("live demo action owner");
                })),
                None,
                if index == 0 { 4 } else { 6 },
            ));
        }
        let status = cx.tree.add_child(
            root,
            Box::new(Label::new("Waiting for renderer-owned Image admission.")),
            None,
            7,
        );
        self.nodes = Some(([buttons[0], buttons[1]], status));
        &mut self.placeholder
    }

    fn take_rebuild_flag(&mut self) -> bool {
        self.nodes.is_none()
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let Some((buttons, status)) = self.nodes else {
            return;
        };
        let Some(tree) = cx.tree.as_deref_mut() else {
            return;
        };
        if let Some(request) = &self.request {
            if let Some(binding) = request
                .try_resolve()
                .expect("renderer-owned admission result")
            {
                for (node, (padding, owner)) in buttons.into_iter().zip([
                    (Padding::symmetric(180.0, 100.0), ProductAppearanceOwner(1)),
                    (Padding::symmetric(40.0, 20.0), ProductAppearanceOwner(2)),
                ]) {
                    tree.node_mut(node)
                        .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Button>())
                        .expect("ordinary retained Button")
                        .set_appearance(style(&cx.app.theme, &binding, padding, owner), None)
                        .expect("current admitted binding and zero own radius");
                }
                self.request = None;
                tree.node_mut(status)
                    .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Label>())
                    .expect("demo status Label")
                    .set_text(format!(
                        "Admitted {:?} / activations: 0",
                        binding.identity()
                    ));
                tree.mark_layout_dirty(status);
            }
        }
        while self.clicks.try_recv().is_ok() {
            self.activation_count += 1;
            tree.node_mut(status)
                .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Label>())
                .expect("demo status Label")
                .set_text(format!(
                    "Admitted NinePatch / activations: {}",
                    self.activation_count
                ));
            tree.mark_layout_dirty(status);
        }
    }
}
