// SPEC-R-015-v2 Demo: Border & Focus Ring Catalog
//
// Demonstrates FocusStyle::ShadowRing, BorderWidths, and RoundedCorners
// extensions. Custom demo widgets only — no production widgets modified.
//
// Run: cargo run --example border_focus

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{BorderWidths, Color, RoundedCorners};
use halcyon::core::style::{FOCUS_RING_SOFTNESS, FOCUS_RING_SPREAD, FocusStyle};
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::*;
use halcyon::utils::id::WidgetInternalId;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    println!("=== Border & Focus Ring Catalog ===");
    println!("SPEC-R-015-v2 infrastructure demo.");
    println!("Close the window to exit");
    println!("===================================\n");

    let app = App::new(AppConfig::new("Border & Focus Ring Catalog").with_size(1100.0, 750.0));
    app.run(BorderFocusDemoScreen::new());
}

// ============================================================================
// FocusRingCard: Box with one FocusStyle variant rendered
// ============================================================================

struct FocusRingCard {
    id: WidgetInternalId,
    label: String,
    focus_style: FocusStyle,
    box_radius: f32,
    card_width: f32,
    card_height: f32,
}

impl FocusRingCard {
    fn new(label: &str, focus_style: FocusStyle, box_radius: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.to_string(),
            focus_style,
            box_radius,
            card_width: 220.0,
            card_height: 80.0,
        }
    }

    fn with_size(mut self, w: f32, h: f32) -> Self {
        self.card_width = w;
        self.card_height = h;
        self
    }
}

impl WidgetDowncast for FocusRingCard {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for FocusRingCard {
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
        // Inset the actual box so the focus ring has room
        let inset = 10.0;
        let box_rect = RectF::new(
            rect.x + inset,
            rect.y + inset,
            rect.width - inset * 2.0,
            rect.height - inset * 2.0 - 16.0,
        );

        // White background box
        cx.scene.push_rounded_rect(
            box_rect,
            Color::from_hex(0xFFFFFF),
            RoundedCorners::uniform(self.box_radius),
            2,
            None,
        );

        // Subtle border
        cx.scene.push_border_rounded_rect(
            box_rect,
            Color::from_srgb(0.0, 0.0, 0.0, 0.15),
            1.0,
            RoundedCorners::uniform(self.box_radius),
            3,
            None,
        );

        // Focus style (rendered as if focused)
        self.focus_style.paint(cx.scene, box_rect, 1.0);

        // Label
        let run = cx.text_engine.shape(&self.label, 11.0);
        cx.scene.push_text_with_backgrounds(
            &run,
            rect.x + 8.0,
            rect.y + rect.height - 14.0,
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
// BorderWidthsBox: Manual per-side border rendering
// ============================================================================

struct BorderWidthsBox {
    id: WidgetInternalId,
    label: String,
    widths: BorderWidths,
    border_color: Color,
}

impl BorderWidthsBox {
    fn new(label: &str, widths: BorderWidths, border_color: Color) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.to_string(),
            widths,
            border_color,
        }
    }
}

impl WidgetDowncast for BorderWidthsBox {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for BorderWidthsBox {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width.min(220.0);
        let h = constraints.max_height.min(80.0);
        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let inset = 10.0;
        let box_rect = RectF::new(
            rect.x + inset,
            rect.y + inset,
            rect.width - inset * 2.0,
            rect.height - inset * 2.0 - 16.0,
        );

        // White background
        cx.scene
            .push_rect(box_rect, Color::from_hex(0xFFFFFF), 2, None);

        // Draw per-side borders as individual rects
        let bw = &self.widths;
        let c = self.border_color;

        // Top border
        if bw.top > 0.0 {
            cx.scene.push_rect(
                RectF::new(box_rect.x, box_rect.y, box_rect.width, bw.top),
                c,
                3,
                None,
            );
        }
        // Bottom border
        if bw.bottom > 0.0 {
            cx.scene.push_rect(
                RectF::new(
                    box_rect.x,
                    box_rect.y + box_rect.height - bw.bottom,
                    box_rect.width,
                    bw.bottom,
                ),
                c,
                3,
                None,
            );
        }
        // Left border
        if bw.left > 0.0 {
            cx.scene.push_rect(
                RectF::new(box_rect.x, box_rect.y, bw.left, box_rect.height),
                c,
                3,
                None,
            );
        }
        // Right border
        if bw.right > 0.0 {
            cx.scene.push_rect(
                RectF::new(
                    box_rect.x + box_rect.width - bw.right,
                    box_rect.y,
                    bw.right,
                    box_rect.height,
                ),
                c,
                3,
                None,
            );
        }

        // Label
        let run = cx.text_engine.shape(&self.label, 11.0);
        cx.scene.push_text_with_backgrounds(
            &run,
            rect.x + 8.0,
            rect.y + rect.height - 14.0,
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
// CornerRadiiBox: Per-corner radius showcase
// ============================================================================

struct CornerRadiiBox {
    id: WidgetInternalId,
    label: String,
    corners: RoundedCorners,
}

impl CornerRadiiBox {
    fn new(label: &str, corners: RoundedCorners) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.to_string(),
            corners,
        }
    }
}

impl WidgetDowncast for CornerRadiiBox {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for CornerRadiiBox {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width.min(220.0);
        let h = constraints.max_height.min(80.0);
        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let inset = 10.0;
        let box_rect = RectF::new(
            rect.x + inset,
            rect.y + inset,
            rect.width - inset * 2.0,
            rect.height - inset * 2.0 - 16.0,
        );

        // Fill with per-corner rounding
        cx.scene
            .push_rounded_rect(box_rect, Color::from_hex(0x4A90D9), self.corners, 2, None);

        // Border
        cx.scene.push_border_rounded_rect(
            box_rect,
            Color::from_srgb(0.0, 0.0, 0.0, 0.2),
            1.0,
            self.corners,
            3,
            None,
        );

        // Show max_radius and to_array info
        let info = format!(
            "max_r={:.0} arr={:?}",
            self.corners.max_radius(),
            self.corners.to_array()
        );
        let info_run = cx.text_engine.shape(&info, 9.0);
        cx.scene.push_text_with_backgrounds(
            &info_run,
            box_rect.x + 4.0,
            box_rect.y + box_rect.height - 12.0,
            Color::from_srgb(1.0, 1.0, 1.0, 0.8),
            vec![],
            4,
            None,
        );

        // Label
        let run = cx.text_engine.shape(&self.label, 11.0);
        cx.scene.push_text_with_backgrounds(
            &run,
            rect.x + 8.0,
            rect.y + rect.height - 14.0,
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

struct BorderFocusDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl BorderFocusDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for BorderFocusDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let accent = Color::from_srgb(0.26, 0.52, 0.96, 0.6);
        let accent_strong = Color::from_srgb(0.26, 0.52, 0.96, 0.8);

        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_spacing(4.0)
                .with_background(Color::from_hex(0xE8E8E8)),
        ));

        // Top margin
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(12.0)), None, 0);

        // === Row 1: Focus Ring Comparison ===
        let h1 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 1);
        cx.tree
            .add_child(h1, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h1,
            Box::new(Label::new("Focus Ring Comparison — Old vs New").with_size(13.0)),
            None,
            1,
        );

        let r1 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 2);
        cx.tree
            .add_child(r1, Box::new(FixedSpacer::horizontal(24.0)), None, 0);

        // StrokeRect (current Button pattern)
        cx.tree.add_child(
            r1,
            Box::new(FocusRingCard::new(
                "StrokeRect (Button)",
                FocusStyle::stroke_rect(1.5, accent_strong, 6.0),
                6.0,
            )),
            None,
            1,
        );

        // FillExpandedRect (current Checkbox pattern)
        cx.tree.add_child(
            r1,
            Box::new(FocusRingCard::new(
                "FillExpandedRect (Checkbox)",
                FocusStyle::fill_expanded(2.0, accent, 6.0, 1),
                6.0,
            )),
            None,
            2,
        );

        // ShadowRing (new R-015-v2)
        cx.tree.add_child(
            r1,
            Box::new(FocusRingCard::new(
                "ShadowRing (new)",
                FocusStyle::shadow_ring(FOCUS_RING_SPREAD, FOCUS_RING_SOFTNESS, accent, 6.0),
                6.0,
            )),
            None,
            3,
        );

        // ShadowRing variant (wider, different color)
        cx.tree.add_child(
            r1,
            Box::new(FocusRingCard::new(
                "ShadowRing (warm)",
                FocusStyle::shadow_ring(3.0, 2.0, Color::from_srgb(0.96, 0.52, 0.26, 0.6), 6.0),
                6.0,
            )),
            None,
            4,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 3);

        // === Row 2: ShadowRing Variations ===
        let h2 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 4);
        cx.tree
            .add_child(h2, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h2,
            Box::new(Label::new("ShadowRing Variations").with_size(13.0)),
            None,
            1,
        );

        let r2 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 5);
        cx.tree
            .add_child(r2, Box::new(FixedSpacer::horizontal(24.0)), None, 0);

        // Tight ring
        cx.tree.add_child(
            r2,
            Box::new(FocusRingCard::new(
                "Tight (1.0, 0.8)",
                FocusStyle::shadow_ring(1.0, 0.8, accent, 6.0),
                6.0,
            )),
            None,
            1,
        );

        // Default ring
        cx.tree.add_child(
            r2,
            Box::new(FocusRingCard::new(
                "Default (2.0, 1.5)",
                FocusStyle::shadow_ring(2.0, 1.5, accent, 6.0),
                6.0,
            )),
            None,
            2,
        );

        // Wide ring
        cx.tree.add_child(
            r2,
            Box::new(FocusRingCard::new(
                "Wide (3.0, 2.5)",
                FocusStyle::shadow_ring(3.0, 2.5, accent, 6.0),
                6.0,
            )),
            None,
            3,
        );

        // Soft glow
        cx.tree.add_child(
            r2,
            Box::new(FocusRingCard::new(
                "Soft glow (4.0, 4.0)",
                FocusStyle::shadow_ring(4.0, 4.0, accent, 6.0),
                6.0,
            )),
            None,
            4,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 6);

        // === Row 3: ShadowRing on Different Shapes ===
        let h3 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 7);
        cx.tree
            .add_child(h3, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h3,
            Box::new(Label::new("ShadowRing on Different Shapes").with_size(13.0)),
            None,
            1,
        );

        let r3 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 8);
        cx.tree
            .add_child(r3, Box::new(FixedSpacer::horizontal(24.0)), None, 0);

        // Sharp rect (r=0)
        cx.tree.add_child(
            r3,
            Box::new(FocusRingCard::new(
                "Sharp (r=0)",
                FocusStyle::shadow_ring(FOCUS_RING_SPREAD, FOCUS_RING_SOFTNESS, accent, 0.0),
                0.0,
            )),
            None,
            1,
        );

        // Small radius (r=4)
        cx.tree.add_child(
            r3,
            Box::new(FocusRingCard::new(
                "Small (r=4)",
                FocusStyle::shadow_ring(FOCUS_RING_SPREAD, FOCUS_RING_SOFTNESS, accent, 4.0),
                4.0,
            )),
            None,
            2,
        );

        // Medium radius (r=8)
        cx.tree.add_child(
            r3,
            Box::new(FocusRingCard::new(
                "Medium (r=8)",
                FocusStyle::shadow_ring(FOCUS_RING_SPREAD, FOCUS_RING_SOFTNESS, accent, 8.0),
                8.0,
            )),
            None,
            3,
        );

        // Circle (r=25 on 50x50 box)
        cx.tree.add_child(
            r3,
            Box::new(
                FocusRingCard::new(
                    "Circle (r=25)",
                    FocusStyle::shadow_ring(FOCUS_RING_SPREAD, FOCUS_RING_SOFTNESS, accent, 25.0),
                    25.0,
                )
                .with_size(90.0, 90.0),
            ),
            None,
            4,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 9);

        // === Row 4: Per-Side Borders (BorderWidths showcase) ===
        let h4 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 10);
        cx.tree
            .add_child(h4, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h4,
            Box::new(Label::new("Per-Side Borders (BorderWidths)").with_size(13.0)),
            None,
            1,
        );

        let r4 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 11);
        cx.tree
            .add_child(r4, Box::new(FixedSpacer::horizontal(24.0)), None, 0);

        let border_color = Color::from_hex(0x333333);

        // Uniform 1px border
        cx.tree.add_child(
            r4,
            Box::new(BorderWidthsBox::new(
                "Uniform 1px",
                BorderWidths::uniform(1.0),
                border_color,
            )),
            None,
            1,
        );

        // Bottom-only 2px border
        cx.tree.add_child(
            r4,
            Box::new(BorderWidthsBox::new(
                "Bottom-only 2px",
                BorderWidths {
                    top: 0.0,
                    right: 0.0,
                    bottom: 2.0,
                    left: 0.0,
                },
                border_color,
            )),
            None,
            2,
        );

        // Left-only 3px accent border
        cx.tree.add_child(
            r4,
            Box::new(BorderWidthsBox::new(
                "Left 3px accent",
                BorderWidths {
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                    left: 3.0,
                },
                Color::from_hex(0x4A90D9),
            )),
            None,
            3,
        );

        // Top+bottom 1px borders
        cx.tree.add_child(
            r4,
            Box::new(BorderWidthsBox::new(
                "Top+Bottom 1px",
                BorderWidths {
                    top: 1.0,
                    right: 0.0,
                    bottom: 1.0,
                    left: 0.0,
                },
                border_color,
            )),
            None,
            4,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 12);

        // === Row 5: Corner Radius Showcase ===
        let h5 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 13);
        cx.tree
            .add_child(h5, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h5,
            Box::new(Label::new("Corner Radius Showcase").with_size(13.0)),
            None,
            1,
        );

        let r5 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 14);
        cx.tree
            .add_child(r5, Box::new(FixedSpacer::horizontal(24.0)), None, 0);

        // Uniform corners
        cx.tree.add_child(
            r5,
            Box::new(CornerRadiiBox::new(
                "Uniform (r=8)",
                RoundedCorners::uniform(8.0),
            )),
            None,
            1,
        );

        // Top-rounded only
        cx.tree.add_child(
            r5,
            Box::new(CornerRadiiBox::new(
                "Top-rounded",
                RoundedCorners::new(8.0, 8.0, 0.0, 0.0),
            )),
            None,
            2,
        );

        // Left-rounded only
        cx.tree.add_child(
            r5,
            Box::new(CornerRadiiBox::new(
                "Left-rounded",
                RoundedCorners::new(8.0, 0.0, 0.0, 8.0),
            )),
            None,
            3,
        );

        // Mixed radii
        cx.tree.add_child(
            r5,
            Box::new(CornerRadiiBox::new(
                "Mixed (4,12,0,8)",
                RoundedCorners::new(4.0, 12.0, 0.0, 8.0),
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
