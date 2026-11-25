// SPEC-R-013-v2 Stage 8: Shadow Pipeline Demo — Professional Desktop Shadow Catalog
//
// Reference sheet showing context-appropriate shadow usage for desktop UI.
// Demonstrates: popup shadows, directional light, recessed inputs,
// elevation scale, focus rings, and scroll edge hints.
//
// Run: cargo run --example shadow

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{Color, RoundedCorners, ShadowMode};
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::*;
use halcyon::utils::id::WidgetInternalId;

// ============================================================================
// Shadow constants — for future theme integration
// ============================================================================

// Elevation levels — consistent light direction (top-down)
// Format: (offset_x, offset_y, blur, alpha)
const SHADOW_TOOLTIP: (f32, f32, f32, f32) = (0.0, 1.0, 3.0, 0.10);
const SHADOW_DROPDOWN: (f32, f32, f32, f32) = (0.0, 2.0, 6.0, 0.15);
const SHADOW_MENU: (f32, f32, f32, f32) = (0.0, 2.0, 6.0, 0.18);
const SHADOW_DIALOG: (f32, f32, f32, f32) = (0.0, 4.0, 12.0, 0.22);
const SHADOW_FLOATING: (f32, f32, f32, f32) = (0.0, 6.0, 20.0, 0.28);

// Inner shadows — (blur, alpha)
const SHADOW_INPUT: (f32, f32) = (2.0, 0.06);
const SHADOW_TEXTAREA: (f32, f32) = (3.0, 0.08);

// Focus ring
const FOCUS_RING_SPREAD: f32 = 2.0;
const FOCUS_RING_SOFTNESS: f32 = 1.5;
const FOCUS_RING_ALPHA: f32 = 0.5;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    println!("=== Shadow Demo — Professional Desktop Shadow Catalog ===");
    println!("Reference sheet for context-appropriate shadow usage.");
    println!("Close the window to exit");
    println!("=========================================================\n");

    let app = App::new(AppConfig::new("Shadow Catalog").with_size(1100.0, 850.0));
    app.run(ShadowDemoScreen::new());
}

// ============================================================================
// ShadowCard: Configurable drop shadow card
// ============================================================================

struct ShadowCard {
    id: WidgetInternalId,
    label: String,
    radius: f32,
    shadow_offset: (f32, f32),
    shadow_spread: f32,
    shadow_blur: f32,
    shadow_color: Color,
    bg_color: Color,
    show_shadow: bool,
    border_color: Option<Color>,
}

impl ShadowCard {
    fn new(label: &str) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.to_string(),
            radius: 6.0,
            shadow_offset: (0.0, 2.0),
            shadow_spread: 0.0,
            shadow_blur: 6.0,
            shadow_color: Color::from_srgb(0.0, 0.0, 0.0, 0.15),
            bg_color: Color::from_hex(0xFFFFFF),
            show_shadow: true,
            border_color: None,
        }
    }

    /// Configure from an elevation tuple (offset_x, offset_y, blur, alpha).
    fn from_elevation(mut self, e: (f32, f32, f32, f32)) -> Self {
        self.shadow_offset = (e.0, e.1);
        self.shadow_blur = e.2;
        self.shadow_color = Color::from_srgb(0.0, 0.0, 0.0, e.3);
        self
    }

    fn with_offset(mut self, x: f32, y: f32) -> Self {
        self.shadow_offset = (x, y);
        self
    }

    #[allow(dead_code)]
    fn with_radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    fn with_no_shadow(mut self) -> Self {
        self.show_shadow = false;
        self
    }

    fn with_border(mut self, color: Color) -> Self {
        self.border_color = Some(color);
        self
    }
}

impl WidgetDowncast for ShadowCard {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ShadowCard {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width.min(150.0);
        let h = constraints.max_height.min(80.0);
        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Shadow (behind card)
        if self.show_shadow {
            cx.scene.push_shadow(
                rect,
                self.radius,
                ShadowMode::Drop,
                self.shadow_offset,
                self.shadow_spread,
                self.shadow_blur,
                self.shadow_color,
                1,
                None,
            );
        }

        // Card background
        if self.radius > 0.0 {
            cx.scene.push_rounded_rect(
                rect,
                self.bg_color,
                RoundedCorners::uniform(self.radius),
                2,
                None,
            );
        } else {
            cx.scene.push_rect(rect, self.bg_color, 2, None);
        }

        // Optional border
        if let Some(border_color) = self.border_color {
            if self.radius > 0.0 {
                cx.scene.push_border_rounded_rect(
                    rect,
                    border_color,
                    1.0,
                    RoundedCorners::uniform(self.radius),
                    3,
                    None,
                );
            } else {
                cx.scene.push_border_rect(rect, border_color, 1.0, 3, None);
            }
        }

        // Label
        let run = cx.text_engine.shape(&self.label, 11.0);
        cx.scene.push_text_with_backgrounds(
            &run,
            rect.x + 10.0,
            rect.y + 12.0,
            Color::from_hex(0x333333),
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
// InputBox: Recessed inner shadow for input fields
// ============================================================================

struct InputBox {
    id: WidgetInternalId,
    label: String,
    radius: f32,
    blur: f32,
    alpha: f32,
    warn: bool,
}

impl InputBox {
    fn new(label: &str, blur: f32, alpha: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.to_string(),
            radius: 4.0,
            blur,
            alpha,
            warn: false,
        }
    }

    fn with_radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    fn with_warn(mut self) -> Self {
        self.warn = true;
        self
    }
}

impl WidgetDowncast for InputBox {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for InputBox {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width.min(150.0);
        let h = constraints.max_height.min(44.0);
        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // White background (input field)
        cx.scene.push_rounded_rect(
            rect,
            Color::from_hex(0xFFFFFF),
            RoundedCorners::uniform(self.radius),
            2,
            None,
        );

        // Border
        let border_color = if self.warn {
            Color::from_hex(0xCC4444)
        } else {
            Color::from_hex(0xCCCCCC)
        };
        cx.scene.push_border_rounded_rect(
            rect,
            border_color,
            1.0,
            RoundedCorners::uniform(self.radius),
            3,
            None,
        );

        // Inner shadow (recessed)
        cx.scene.push_shadow(
            rect,
            self.radius,
            ShadowMode::Inner,
            (0.0, 1.0),
            0.0,
            self.blur,
            Color::from_srgb(0.0, 0.0, 0.0, self.alpha),
            4,
            None,
        );

        // Label
        let label_color = if self.warn {
            Color::from_hex(0xCC2222)
        } else {
            Color::from_hex(0x666666)
        };
        let run = cx.text_engine.shape(&self.label, 11.0);
        cx.scene.push_text_with_backgrounds(
            &run,
            rect.x + 10.0,
            rect.y + 14.0,
            label_color,
            vec![],
            5,
            None,
        );
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

// ============================================================================
// FocusRingBox: Focus ring via ShadowMode::Ring
// ============================================================================

struct FocusRingBox {
    id: WidgetInternalId,
    label: String,
    ring_color: Color,
}

impl FocusRingBox {
    fn new(label: &str, ring_color: Color) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.to_string(),
            ring_color,
        }
    }
}

impl WidgetDowncast for FocusRingBox {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for FocusRingBox {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width.min(140.0);
        let h = constraints.max_height.min(50.0);
        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let radius = 6.0;

        // Background
        cx.scene.push_rounded_rect(
            rect,
            Color::from_hex(0xFFFFFF),
            RoundedCorners::uniform(radius),
            2,
            None,
        );

        // Border
        cx.scene.push_border_rounded_rect(
            rect,
            Color::from_hex(0xCCCCCC),
            1.0,
            RoundedCorners::uniform(radius),
            3,
            None,
        );

        // Focus ring
        cx.scene.push_shadow(
            rect,
            radius,
            ShadowMode::Ring,
            (0.0, 0.0),
            FOCUS_RING_SPREAD,
            FOCUS_RING_SOFTNESS,
            self.ring_color,
            4,
            None,
        );

        // Label
        let run = cx.text_engine.shape(&self.label, 11.0);
        cx.scene.push_text_with_backgrounds(
            &run,
            rect.x + 10.0,
            rect.y + 16.0,
            Color::from_hex(0x333333),
            vec![],
            5,
            None,
        );
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

// ============================================================================
// ScrollEdgeBox: Inner shadow simulating clipped content above
// ============================================================================

struct ScrollEdgeBox {
    id: WidgetInternalId,
}

impl ScrollEdgeBox {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for ScrollEdgeBox {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ScrollEdgeBox {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width.min(140.0);
        let h = constraints.max_height.min(50.0);
        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let radius = 4.0;

        // White background
        cx.scene.push_rounded_rect(
            rect,
            Color::from_hex(0xFFFFFF),
            RoundedCorners::uniform(radius),
            2,
            None,
        );

        // Border
        cx.scene.push_border_rounded_rect(
            rect,
            Color::from_hex(0xDDDDDD),
            1.0,
            RoundedCorners::uniform(radius),
            3,
            None,
        );

        // Scroll edge shadow — inner shadow biased upward via offset(0, -2)
        // Simulates clipped content above the visible area
        cx.scene.push_shadow(
            rect,
            radius,
            ShadowMode::Inner,
            (0.0, -2.0),
            0.0,
            4.0,
            Color::from_srgb(0.0, 0.0, 0.0, 0.10),
            4,
            None,
        );

        // Simulated text content lines
        let line_color = Color::from_hex(0xDDDDDD);
        for i in 0..3 {
            let ly = rect.y + 12.0 + i as f32 * 10.0;
            let lw = if i == 2 { 60.0 } else { 90.0 };
            cx.scene
                .push_rect(RectF::new(rect.x + 10.0, ly, lw, 3.0), line_color, 3, None);
        }

        // Label at bottom
        let run = cx.text_engine.shape("Scroll Edge", 9.0);
        cx.scene.push_text_with_backgrounds(
            &run,
            rect.x + 10.0,
            rect.y + rect.height - 14.0,
            Color::from_hex(0x999999),
            vec![],
            5,
            None,
        );
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

// ============================================================================
// CompositeCard: Drop shadow + inner shadow content area
// ============================================================================

struct CompositeCard {
    id: WidgetInternalId,
}

impl CompositeCard {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for CompositeCard {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for CompositeCard {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width.min(160.0);
        let h = constraints.max_height.min(70.0);
        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let outer_radius = 6.0;

        // Outer drop shadow (card elevation)
        cx.scene.push_shadow(
            rect,
            outer_radius,
            ShadowMode::Drop,
            (0.0, 2.0),
            0.0,
            6.0,
            Color::from_srgb(0.0, 0.0, 0.0, 0.15),
            1,
            None,
        );

        // Card background
        cx.scene.push_rounded_rect(
            rect,
            Color::from_hex(0xFFFFFF),
            RoundedCorners::uniform(outer_radius),
            2,
            None,
        );

        // Inner content well (inset)
        let inset = 8.0;
        let well = RectF::new(
            rect.x + inset,
            rect.y + 22.0,
            rect.width - inset * 2.0,
            rect.height - 22.0 - inset,
        );
        let inner_radius = 3.0;

        // Content well background
        cx.scene.push_rounded_rect(
            well,
            Color::from_hex(0xF8F8F8),
            RoundedCorners::uniform(inner_radius),
            3,
            None,
        );

        // Inner shadow on content well (recessed)
        cx.scene.push_shadow(
            well,
            inner_radius,
            ShadowMode::Inner,
            (0.0, 1.0),
            0.0,
            2.0,
            Color::from_srgb(0.0, 0.0, 0.0, 0.06),
            4,
            None,
        );

        // Title label
        let run = cx.text_engine.shape("Combined", 11.0);
        cx.scene.push_text_with_backgrounds(
            &run,
            rect.x + 10.0,
            rect.y + 6.0,
            Color::from_hex(0x333333),
            vec![],
            5,
            None,
        );

        // Content placeholder lines
        let line_color = Color::from_hex(0xDDDDDD);
        for i in 0..2 {
            let ly = well.y + 8.0 + i as f32 * 10.0;
            let lw = if i == 1 { 80.0 } else { 110.0 };
            cx.scene
                .push_rect(RectF::new(well.x + 6.0, ly, lw, 3.0), line_color, 3, None);
        }
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

// ============================================================================
// Demo Screen
// ============================================================================

struct ShadowDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl ShadowDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for ShadowDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_spacing(4.0)
                .with_background(Color::from_hex(0xE8E8E8)),
        ));

        // Top margin
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(12.0)), None, 0);

        // === Row 1: Popup / Overlay ===
        let h1 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 1);
        cx.tree
            .add_child(h1, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h1,
            Box::new(Label::new("Popup / Overlay").with_size(13.0)),
            None,
            1,
        );

        let r1 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 2);
        cx.tree
            .add_child(r1, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            r1,
            Box::new(ShadowCard::new("Tooltip").from_elevation(SHADOW_TOOLTIP)),
            None,
            1,
        );
        cx.tree.add_child(
            r1,
            Box::new(ShadowCard::new("Dropdown").from_elevation(SHADOW_DROPDOWN)),
            None,
            2,
        );
        cx.tree.add_child(
            r1,
            Box::new(ShadowCard::new("Context Menu").from_elevation(SHADOW_MENU)),
            None,
            3,
        );
        cx.tree.add_child(
            r1,
            Box::new(ShadowCard::new("Modal Dialog").from_elevation(SHADOW_DIALOG)),
            None,
            4,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 3);

        // === Row 2: Directional Light ===
        let h2 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 4);
        cx.tree
            .add_child(h2, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h2,
            Box::new(Label::new("Directional Light").with_size(13.0)),
            None,
            1,
        );

        let r2 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 5);
        cx.tree
            .add_child(r2, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            r2,
            Box::new(ShadowCard::new("Top-down").from_elevation(SHADOW_DROPDOWN)),
            None,
            1,
        );
        cx.tree.add_child(
            r2,
            Box::new(
                ShadowCard::new("Upper-left")
                    .from_elevation(SHADOW_DROPDOWN)
                    .with_offset(1.0, 2.0),
            ),
            None,
            2,
        );
        cx.tree.add_child(
            r2,
            Box::new(
                ShadowCard::new("Upper-right")
                    .from_elevation(SHADOW_DROPDOWN)
                    .with_offset(-1.0, 2.0),
            ),
            None,
            3,
        );
        cx.tree.add_child(
            r2,
            Box::new(
                ShadowCard::new("Ambient")
                    .from_elevation(SHADOW_DROPDOWN)
                    .with_offset(0.0, 0.0),
            ),
            None,
            4,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 6);

        // === Row 3: Recessed Inputs ===
        let h3 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 7);
        cx.tree
            .add_child(h3, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h3,
            Box::new(Label::new("Recessed Inputs").with_size(13.0)),
            None,
            1,
        );

        let r3 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 8);
        cx.tree
            .add_child(r3, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            r3,
            Box::new(InputBox::new("TextField", SHADOW_INPUT.0, SHADOW_INPUT.1)),
            None,
            1,
        );
        cx.tree.add_child(
            r3,
            Box::new(InputBox::new(
                "TextArea",
                SHADOW_TEXTAREA.0,
                SHADOW_TEXTAREA.1,
            )),
            None,
            2,
        );
        cx.tree.add_child(
            r3,
            Box::new(InputBox::new("Search Box", 2.0, 0.05).with_radius(12.0)),
            None,
            3,
        );
        cx.tree.add_child(
            r3,
            Box::new(InputBox::new("Don't do this", 6.0, 0.25).with_warn()),
            None,
            4,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 9);

        // === Row 4: Elevation Scale ===
        let h4 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 10);
        cx.tree
            .add_child(h4, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h4,
            Box::new(Label::new("Elevation Scale").with_size(13.0)),
            None,
            1,
        );

        let r4 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 11);
        cx.tree
            .add_child(r4, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            r4,
            Box::new(
                ShadowCard::new("Level 0 (flat)")
                    .with_no_shadow()
                    .with_border(Color::from_hex(0xDDDDDD)),
            ),
            None,
            1,
        );
        cx.tree.add_child(
            r4,
            Box::new(ShadowCard::new("Level 1 (toolbar)").from_elevation((0.0, 1.0, 2.0, 0.08))),
            None,
            2,
        );
        cx.tree.add_child(
            r4,
            Box::new(ShadowCard::new("Level 2 (dropdown)").from_elevation(SHADOW_DROPDOWN)),
            None,
            3,
        );
        cx.tree.add_child(
            r4,
            Box::new(ShadowCard::new("Level 3 (dialog)").from_elevation(SHADOW_DIALOG)),
            None,
            4,
        );
        cx.tree.add_child(
            r4,
            Box::new(ShadowCard::new("Level 4 (floating)").from_elevation(SHADOW_FLOATING)),
            None,
            5,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 12);

        // === Row 5: Focus Ring + Scroll Edge ===
        let h5 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 13);
        cx.tree
            .add_child(h5, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            h5,
            Box::new(Label::new("Focus Ring + Scroll Edge").with_size(13.0)),
            None,
            1,
        );

        let r5 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(20.0)), None, 14);
        cx.tree
            .add_child(r5, Box::new(FixedSpacer::horizontal(24.0)), None, 0);
        cx.tree.add_child(
            r5,
            Box::new(FocusRingBox::new(
                "Blue Focus",
                Color::from_srgb(0.26, 0.52, 0.96, FOCUS_RING_ALPHA),
            )),
            None,
            1,
        );
        cx.tree
            .add_child(r5, Box::new(ScrollEdgeBox::new()), None, 2);
        cx.tree
            .add_child(r5, Box::new(CompositeCard::new()), None, 3);
        cx.tree.add_child(
            r5,
            Box::new(FocusRingBox::new(
                "Red Ring",
                Color::from_srgb(0.9, 0.2, 0.2, 0.5),
            )),
            None,
            4,
        );
        cx.tree.add_child(
            r5,
            Box::new(FocusRingBox::new(
                "Gold Ring",
                Color::from_srgb(0.85, 0.65, 0.1, 0.6),
            )),
            None,
            5,
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
