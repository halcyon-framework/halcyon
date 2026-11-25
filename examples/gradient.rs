// SPEC-R-012-v2 Stage 7: Gradient Pipeline Demo — Gradient Catalog
//
// Reference sheet showing gradient capabilities: linear, radial,
// rounded corners, and practical UI use cases.
//
// Run: cargo run --example gradient

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{Color, ColorStop, RoundedCorners};
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::*;
use halcyon::utils::id::WidgetInternalId;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    println!("=== Gradient Demo — Gradient Catalog ===");
    println!("Reference sheet for gradient rendering.");
    println!("Close the window to exit");
    println!("=========================================\n");

    let app = App::new(AppConfig::new("Gradient Catalog").with_size(1000.0, 650.0));
    app.run(GradientDemoScreen::new());
}

// ============================================================================
// GradientCard: Configurable gradient sample
// ============================================================================

enum GradientType {
    Linear {
        from: (f32, f32),
        to: (f32, f32),
        stops: Vec<ColorStop>,
    },
    Radial {
        center: (f32, f32),
        edge: (f32, f32),
        stops: Vec<ColorStop>,
    },
    Vertical {
        top: Color,
        bottom: Color,
    },
}

struct GradientCard {
    id: WidgetInternalId,
    label: String,
    radius: f32,
    gradient: GradientType,
    card_width: f32,
    card_height: f32,
}

impl GradientCard {
    fn new(label: &str) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.to_string(),
            radius: 6.0,
            gradient: GradientType::Vertical {
                top: Color::from_hex(0xFFFFFF),
                bottom: Color::from_hex(0xCCCCCC),
            },
            card_width: 200.0,
            card_height: 100.0,
        }
    }

    fn with_radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    fn with_size(mut self, w: f32, h: f32) -> Self {
        self.card_width = w;
        self.card_height = h;
        self
    }

    fn with_linear(mut self, from: (f32, f32), to: (f32, f32), stops: Vec<ColorStop>) -> Self {
        self.gradient = GradientType::Linear { from, to, stops };
        self
    }

    fn with_radial(mut self, center: (f32, f32), edge: (f32, f32), stops: Vec<ColorStop>) -> Self {
        self.gradient = GradientType::Radial {
            center,
            edge,
            stops,
        };
        self
    }

    fn with_vertical(mut self, top: Color, bottom: Color) -> Self {
        self.gradient = GradientType::Vertical { top, bottom };
        self
    }
}

impl WidgetDowncast for GradientCard {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for GradientCard {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width.min(self.card_width);
        let h = constraints.max_height.min(self.card_height);
        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        match &self.gradient {
            GradientType::Linear { from, to, stops } => {
                cx.paint_linear_gradient(rect, self.radius, *from, *to, stops, 2);
            }
            GradientType::Radial {
                center,
                edge,
                stops,
            } => {
                cx.paint_radial_gradient(rect, self.radius, *center, *edge, stops, 2);
            }
            GradientType::Vertical { top, bottom } => {
                cx.paint_vertical_gradient(rect, self.radius, *top, *bottom, 2, None);
            }
        }

        // Border for visibility
        if self.radius > 0.0 {
            cx.scene.push_border_rounded_rect(
                rect,
                Color::from_srgb(0.0, 0.0, 0.0, 0.1),
                1.0,
                RoundedCorners::uniform(self.radius),
                3,
                None,
            );
        } else {
            cx.scene
                .push_border_rect(rect, Color::from_srgb(0.0, 0.0, 0.0, 0.1), 1.0, 3, None);
        }

        // Label
        let run = cx.text_engine.shape(&self.label, 11.0);
        cx.scene.push_text_with_backgrounds(
            &run,
            rect.x + 8.0,
            rect.y + rect.height - 18.0,
            Color::from_srgb(0.0, 0.0, 0.0, 0.7),
            vec![],
            4,
            None,
        );
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

// ============================================================================
// Demo Screen
// ============================================================================

struct GradientDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl GradientDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for GradientDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_spacing(4.0)
                .with_background(Color::from_hex(0xE8E8E8)),
        ));

        // Top margin
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(12.0)), None, 0);

        // === Row 1: Linear Gradients ===
        let h1 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 1);
        cx.tree
            .add_child(h1, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h1,
            Box::new(Label::new("Linear Gradients").with_size(13.0)),
            None,
            1,
        );

        let r1 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 2);
        cx.tree
            .add_child(r1, Box::new(FixedSpacer::horizontal(24.0)), None, 0);

        // Vertical linear
        cx.tree.add_child(
            r1,
            Box::new(GradientCard::new("Vertical").with_linear(
                (0.0, 0.0),
                (0.0, 1.0),
                vec![
                    ColorStop {
                        offset: 0.0,
                        color: Color::from_hex(0x4A90D9),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: Color::from_hex(0x1B4F8A),
                    },
                ],
            )),
            None,
            1,
        );

        // Horizontal linear
        cx.tree.add_child(
            r1,
            Box::new(GradientCard::new("Horizontal").with_linear(
                (0.0, 0.5),
                (1.0, 0.5),
                vec![
                    ColorStop {
                        offset: 0.0,
                        color: Color::from_hex(0xE74C3C),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: Color::from_hex(0xF39C12),
                    },
                ],
            )),
            None,
            2,
        );

        // Diagonal linear
        cx.tree.add_child(
            r1,
            Box::new(GradientCard::new("Diagonal").with_linear(
                (0.0, 0.0),
                (1.0, 1.0),
                vec![
                    ColorStop {
                        offset: 0.0,
                        color: Color::from_hex(0x8E44AD),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: Color::from_hex(0x3498DB),
                    },
                ],
            )),
            None,
            3,
        );

        // 4-stop rainbow
        cx.tree.add_child(
            r1,
            Box::new(GradientCard::new("Rainbow (4-stop)").with_linear(
                (0.0, 0.5),
                (1.0, 0.5),
                vec![
                    ColorStop {
                        offset: 0.0,
                        color: Color::from_hex(0xE74C3C),
                    },
                    ColorStop {
                        offset: 0.33,
                        color: Color::from_hex(0xF1C40F),
                    },
                    ColorStop {
                        offset: 0.66,
                        color: Color::from_hex(0x2ECC71),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: Color::from_hex(0x3498DB),
                    },
                ],
            )),
            None,
            4,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 3);

        // === Row 2: Radial Gradients ===
        let h2 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 4);
        cx.tree
            .add_child(h2, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h2,
            Box::new(Label::new("Radial Gradients").with_size(13.0)),
            None,
            1,
        );

        let r2 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 5);
        cx.tree
            .add_child(r2, Box::new(FixedSpacer::horizontal(24.0)), None, 0);

        // Center-out
        cx.tree.add_child(
            r2,
            Box::new(GradientCard::new("Center-out").with_radial(
                (0.5, 0.5),
                (1.0, 0.5),
                vec![
                    ColorStop {
                        offset: 0.0,
                        color: Color::from_hex(0xF1C40F),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: Color::from_hex(0xE67E22),
                    },
                ],
            )),
            None,
            1,
        );

        // Off-center
        cx.tree.add_child(
            r2,
            Box::new(GradientCard::new("Off-center").with_radial(
                (0.3, 0.3),
                (1.0, 1.0),
                vec![
                    ColorStop {
                        offset: 0.0,
                        color: Color::from_hex(0xFFFFFF),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: Color::from_hex(0x3498DB),
                    },
                ],
            )),
            None,
            2,
        );

        // Tight radius
        cx.tree.add_child(
            r2,
            Box::new(GradientCard::new("Tight radius").with_radial(
                (0.5, 0.5),
                (0.6, 0.5),
                vec![
                    ColorStop {
                        offset: 0.0,
                        color: Color::from_hex(0xFFFFFF),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: Color::from_hex(0x2C3E50),
                    },
                ],
            )),
            None,
            3,
        );

        // Wide radius
        cx.tree.add_child(
            r2,
            Box::new(GradientCard::new("Wide radius").with_radial(
                (0.5, 0.5),
                (1.5, 1.5),
                vec![
                    ColorStop {
                        offset: 0.0,
                        color: Color::from_hex(0xE74C3C),
                    },
                    ColorStop {
                        offset: 0.5,
                        color: Color::from_hex(0x8E44AD),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: Color::from_hex(0x2C3E50),
                    },
                ],
            )),
            None,
            4,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 6);

        // === Row 3: Rounded Gradient Rects ===
        let h3 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 7);
        cx.tree
            .add_child(h3, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h3,
            Box::new(Label::new("Rounded Corners").with_size(13.0)),
            None,
            1,
        );

        let r3 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 8);
        cx.tree
            .add_child(r3, Box::new(FixedSpacer::horizontal(24.0)), None, 0);

        let corner_stops = vec![
            ColorStop {
                offset: 0.0,
                color: Color::from_hex(0x6C5CE7),
            },
            ColorStop {
                offset: 1.0,
                color: Color::from_hex(0xA29BFE),
            },
        ];

        // r=0
        cx.tree.add_child(
            r3,
            Box::new(
                GradientCard::new("r=0 (sharp)")
                    .with_radius(0.0)
                    .with_linear((0.0, 0.0), (1.0, 1.0), corner_stops.clone()),
            ),
            None,
            1,
        );

        // r=6
        cx.tree.add_child(
            r3,
            Box::new(GradientCard::new("r=6").with_radius(6.0).with_linear(
                (0.0, 0.0),
                (1.0, 1.0),
                corner_stops.clone(),
            )),
            None,
            2,
        );

        // r=16
        cx.tree.add_child(
            r3,
            Box::new(GradientCard::new("r=16").with_radius(16.0).with_linear(
                (0.0, 0.0),
                (1.0, 1.0),
                corner_stops.clone(),
            )),
            None,
            3,
        );

        // Pill
        cx.tree.add_child(
            r3,
            Box::new(
                GradientCard::new("Pill (r=50)")
                    .with_radius(50.0)
                    .with_linear((0.0, 0.0), (1.0, 1.0), corner_stops),
            ),
            None,
            4,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 9);

        // === Row 4: Practical Use Cases ===
        let h4 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 10);
        cx.tree
            .add_child(h4, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h4,
            Box::new(Label::new("Practical Use Cases").with_size(13.0)),
            None,
            1,
        );

        let r4 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 11);
        cx.tree
            .add_child(r4, Box::new(FixedSpacer::horizontal(24.0)), None, 0);

        // Subtle button hover tint
        cx.tree.add_child(
            r4,
            Box::new(
                GradientCard::new("Button Tint")
                    .with_vertical(Color::from_hex(0xFFFFFF), Color::from_hex(0xF0F0F0)),
            ),
            None,
            1,
        );

        // Progress bar fill
        cx.tree.add_child(
            r4,
            Box::new(
                GradientCard::new("Progress Bar")
                    .with_radius(4.0)
                    .with_size(200.0, 100.0)
                    .with_linear(
                        (0.0, 0.0),
                        (1.0, 0.0),
                        vec![
                            ColorStop {
                                offset: 0.0,
                                color: Color::from_hex(0x27AE60),
                            },
                            ColorStop {
                                offset: 1.0,
                                color: Color::from_hex(0x2ECC71),
                            },
                        ],
                    ),
            ),
            None,
            2,
        );

        // Header bar background
        cx.tree.add_child(
            r4,
            Box::new(
                GradientCard::new("Header Bar")
                    .with_radius(0.0)
                    .with_linear(
                        (0.0, 0.0),
                        (0.0, 1.0),
                        vec![
                            ColorStop {
                                offset: 0.0,
                                color: Color::from_hex(0x2C3E50),
                            },
                            ColorStop {
                                offset: 1.0,
                                color: Color::from_hex(0x34495E),
                            },
                        ],
                    ),
            ),
            None,
            3,
        );

        // Sidebar gradient / spotlight
        cx.tree.add_child(
            r4,
            Box::new(GradientCard::new("Spotlight").with_radial(
                (0.3, 0.3),
                (1.0, 1.0),
                vec![
                    ColorStop {
                        offset: 0.0,
                        color: Color::from_srgb(1.0, 1.0, 1.0, 0.9),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: Color::from_srgb(1.0, 1.0, 1.0, 0.0),
                    },
                ],
            )),
            None,
            4,
        );

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
