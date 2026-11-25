//! Native visual acceptance for the SPEC-HAL-012/328 Button appearance path.
//! Run from the repository root: `cargo run --example button_appearance`.

use std::sync::mpsc::{self, Receiver, Sender};

use halcyon::core::animation::Easing;
use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::{
    AppearanceConfigurationId, AppearanceConfigurationOwner, AppearanceFallbackPolicy,
    AppearanceSelector, AppearanceStroke, AppearanceSurface, ButtonAppearanceConfiguration,
    ButtonAppearanceContract, ButtonAppearanceDefinition, ButtonAppearanceMetrics,
    ButtonPaintValues, PreparedButtonStyle, ProductAppearanceOwner,
};
use halcyon::core::layout::{EdgeInsets, FlexParams, Padding};
use halcyon::core::paint::Color;
use halcyon::core::presentation::{
    ExchangeRecipe, PresentationAxis, PresentationComposition, PresentationConfiguration,
    PresentationLimits, PresentationMotionPolicy, PresentationOutcome, PresentationRequest,
    PresentationSettlement, PresentationSource, PresentationTicket,
};
use halcyon::core::theme::Theme;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::utils::id::TreeWidgetId;
use halcyon::widgets::{button::Button, flex::Flex, label::Label};

fn main() -> Result<(), String> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();
    let theme = Theme::default();
    let screen = AppearanceDemo::new(&theme)?;
    App::new(
        AppConfig::new("Button appearance — native acceptance")
            .with_size(960.0, 660.0)
            .with_theme(theme),
    )
    .run(screen);
    Ok(())
}

fn product_style(theme: &Theme) -> Result<PreparedButtonStyle, String> {
    let contract = ButtonAppearanceContract::standard();
    let mut values = theme.button_appearance_values(&contract);
    let paint = |surface| ButtonPaintValues {
        surface: AppearanceSurface::Solid(surface),
        radius: 16.0,
        content_color: Color::from_srgb(1.0, 1.0, 1.0, 1.0),
        underlay: None,
        border: Some(AppearanceStroke {
            color: Color::from_srgb(0.70, 0.85, 1.0, 1.0),
            width: 2.0,
        }),
        overlay: None,
    };
    values.metrics = Some(ButtonAppearanceMetrics {
        padding: Padding::symmetric(32.0, 20.0),
    });
    values.resting = Some(paint(Color::from_srgb(0.08, 0.24, 0.65, 1.0)));
    values.hovered = Some(paint(Color::from_srgb(0.02, 0.48, 0.46, 1.0)));
    values.pressed = Some(paint(Color::from_srgb(0.64, 0.08, 0.34, 1.0)));
    values.focus = Some(AppearanceStroke {
        color: Color::from_srgb(1.0, 0.90, 0.12, 1.0),
        width: 4.0,
    });
    let configuration = ButtonAppearanceConfiguration::new(
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
    .map_err(|error| format!("Invalid demo configuration: {error:?}"))?;
    configuration
        .prepare_style(
            AppearanceSelector::button(),
            AppearanceFallbackPolicy::Reject,
        )
        .map_err(|error| format!("Incomplete demo configuration: {error:?}"))
}

enum Action {
    ToggleSkin,
    ActivateTarget,
}

#[derive(Clone, Copy)]
struct DemoNodes {
    target: NodeId,
    target_id: TreeWidgetId,
    status: NodeId,
    help: [NodeId; 2],
}

struct AppearanceDemo {
    placeholder: Label,
    nodes: Option<DemoNodes>,
    product_style: PreparedButtonStyle,
    product_active: bool,
    activations: u64,
    actions: Receiver<Action>,
    sender: Sender<Action>,
    presentation_cause: u64,
    presentation: Option<PresentationTicket>,
}

impl AppearanceDemo {
    fn new(theme: &Theme) -> Result<Self, String> {
        let (sender, actions) = mpsc::channel();
        Ok(Self {
            placeholder: Label::new(""),
            nodes: None,
            product_style: product_style(theme)?,
            product_active: false,
            activations: 0,
            actions,
            sender,
            presentation_cause: 0,
            presentation: None,
        })
    }

    fn status(&self, id: TreeWidgetId) -> String {
        let skin = if self.product_active {
            "Product"
        } else {
            "Framework"
        };
        format!(
            "Target: {skin} | WidgetId: {} (unchanged) | Activations: {}",
            id.0, self.activations
        )
    }
}

impl Screen for AppearanceDemo {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.nodes.is_some() {
            return &mut self.placeholder;
        }
        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_padding(EdgeInsets::all(24.0))
                .with_spacing(16.0),
        ));
        add_label(cx.tree, root, "Button appearance / native renderer", 0);
        add_label(
            cx.tree,
            root,
            "Tab / Shift+Tab: focus. Hold Enter or Space: press. Release: activate.",
            1,
        );
        add_label(
            cx.tree,
            root,
            "Product: blue at rest, teal on hover, magenta when pressed; yellow focus outline.",
            2,
        );

        let comparison = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(48.0)), None, 3);
        let framework = column(cx.tree, comparison, 0);
        add_label(cx.tree, framework, "Framework / default", 0);
        cx.tree
            .add_child(framework, Box::new(Button::with_text("Activate")), None, 1);
        add_label(cx.tree, framework, "Unmodified Theme values and padding", 2);

        let product = column(cx.tree, comparison, 1);
        add_label(cx.tree, product, "Product / solid configuration", 0);
        let mut product_button = Button::with_text("Activate");
        product_button
            .set_appearance(self.product_style.clone(), None)
            .expect("the fixed solid demo style must be admitted by Button");
        cx.tree
            .add_child(product, Box::new(product_button), None, 1);
        add_label(cx.tree, product, "32 horizontal / 20 vertical padding", 2);

        add_label(
            cx.tree,
            root,
            "Runtime replacement / one retained Button",
            4,
        );
        let sender = self.sender.clone();
        cx.tree.add_child(
            root,
            Box::new(
                Button::with_text("Switch target: Framework / Product").on_click(move || {
                    if let Err(error) = sender.send(Action::ToggleSkin) {
                        tracing::error!(%error, "demo action receiver was dropped");
                    }
                }),
            ),
            None,
            5,
        );
        let runtime = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(48.0)), None, 6);
        let target_column = column(cx.tree, runtime, 0);
        add_label(cx.tree, target_column, "Switchable target", 0);
        let sender = self.sender.clone();
        let target = cx.tree.add_child(
            target_column,
            Box::new(Button::with_text("Activate").on_click(move || {
                if let Err(error) = sender.send(Action::ActivateTarget) {
                    tracing::error!(%error, "demo action receiver was dropped");
                }
            })),
            None,
            1,
        );
        let reference = column(cx.tree, runtime, 1);
        add_label(cx.tree, reference, "Unchanged / Framework reference", 0);
        cx.tree
            .add_child(reference, Box::new(Button::with_text("Activate")), None, 1);

        let target_id = cx
            .tree
            .widget_id_for_node(target)
            .expect("newly mounted target has an ID");
        let status = add_label(cx.tree, root, self.status(target_id), 7);
        let first_help = add_label(
            cx.tree,
            root,
            "Switch the target skin; its padding changes its size, not its identity or activation.",
            8,
        );
        let second_help = add_label(
            cx.tree,
            root,
            "Compare with the unchanged reference. Close the window to exit.",
            9,
        );
        self.nodes = Some(DemoNodes {
            target,
            target_id,
            status,
            help: [first_help, second_help],
        });
        // Application-owned finite policy, not framework motion defaults. These
        // bounds include conservative repeated charges of the shared font lease.
        // The engine's custody/retirement tests measure the solid/text profile.
        cx.tree
            .configure_presentation(PresentationConfiguration {
                owner: cx.tree.widget_id_for_node(root).expect("mounted root"),
                sources: [target, status, first_help, second_help]
                    .map(|node| {
                        PresentationSource::Widget(
                            cx.tree.widget_id_for_node(node).expect("mounted source"),
                        )
                    })
                    .to_vec(),
                policy: PresentationMotionPolicy {
                    enabled: true,
                    allow_nonessential_motion: true,
                    limits: PresentationLimits {
                        groups: 1,
                        queued_requests: 8,
                        sources: 4,
                        source_widgets: 16,
                        occurrences: 16,
                        generations: 32,
                        nodes: 2048,
                        depth: 24,
                        dependency_edges: 4096,
                        cpu_bytes: 512 * 1024 * 1024,
                        text_bytes: 65536,
                        font_bytes: 512 * 1024 * 1024,
                        resource_bytes: 0,
                        in_flight_bytes: 512 * 1024 * 1024,
                        capture_nodes_per_frame: 4096,
                        max_duration_secs: 1.0,
                        max_historical_age_ms: 4000,
                    },
                },
            })
            .expect("finite demo source custody");
        &mut self.placeholder
    }

    fn take_rebuild_flag(&mut self) -> bool {
        self.nodes.is_none()
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        if let Some(tree) = cx.tree.as_deref_mut() {
            self.update(tree);
        }
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        let Some(nodes) = self.nodes else { return };
        let mut changed = false;
        // Callbacks only enqueue actions. Existing screen update authority owns
        // value replacement; Button classifies its padding/paint invalidation.
        while let Ok(action) = self.actions.try_recv() {
            assert_eq!(tree.widget_id_for_node(nodes.target), Some(nodes.target_id));
            match action {
                Action::ToggleSkin => {
                    self.presentation_cause = self
                        .presentation_cause
                        .checked_add(1)
                        .expect("demo cause exhausted");
                    let replaces = self.presentation.and_then(|ticket| {
                        tree.presentation_receipts()
                            .iter()
                            .find(|receipt| receipt.ticket == ticket)
                            .filter(|receipt| {
                                matches!(
                                    receipt.outcome,
                                    PresentationOutcome::Reserved | PresentationOutcome::Admitted
                                )
                            })
                            .map(|_| ticket.group)
                    });
                    let source = |node| {
                        PresentationSource::Widget(
                            tree.widget_id_for_node(node).expect("retained source"),
                        )
                    };
                    let request = PresentationRequest {
                        cause: self.presentation_cause,
                        composition: PresentationComposition::Exchange(ExchangeRecipe {
                            replacement: source(nodes.target),
                            displaced: [nodes.status, nodes.help[0], nodes.help[1]]
                                .map(source)
                                .to_vec(),
                            axis: PresentationAxis::Vertical,
                        }),
                        duration_secs: 0.36,
                        easing: Easing::Linear,
                        replaces,
                        settlement: PresentationSettlement::Canonical,
                    };
                    match tree.request_presentation(request) {
                        Ok(ticket) => self.presentation = Some(ticket),
                        Err(reason) => {
                            tracing::warn!(?reason, "Exchange settled to canonical appearance")
                        }
                    }
                    let button = tree
                        .node_mut(nodes.target)
                        .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Button>())
                        .expect("the retained demo target remains an ordinary Button");
                    if self.product_active {
                        button.use_default_appearance();
                    } else {
                        button
                            .set_appearance(self.product_style.clone(), None)
                            .expect("the fixed solid demo style must remain admitted");
                    }
                    self.product_active = !self.product_active;
                }
                Action::ActivateTarget => self.activations += 1,
            }
            changed = true;
        }
        if changed {
            let status = self.status(nodes.target_id);
            let label = tree
                .node_mut(nodes.status)
                .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Label>())
                .expect("the retained demo status remains a Label");
            if label.text != status {
                label.set_text(status);
                tree.mark_layout_dirty(nodes.status);
            }
        }
    }
}

fn add_label(tree: &mut WidgetTree, parent: NodeId, text: impl Into<String>, order: i16) -> NodeId {
    tree.add_child(parent, Box::new(Label::new(text)), None, order)
}

fn column(tree: &mut WidgetTree, parent: NodeId, order: i16) -> NodeId {
    let node = tree.add_child(
        parent,
        Box::new(Flex::column().with_spacing(12.0)),
        None,
        order,
    );
    tree.set_flex(
        node,
        FlexParams {
            grow: 0.0,
            shrink: 0.0,
            basis: 340.0,
        },
    );
    node
}
