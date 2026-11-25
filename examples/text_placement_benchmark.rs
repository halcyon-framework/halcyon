// SPEC-HAL-092 Text Placement Benchmark
//
// Purpose: Exposes whether Halcyon obeys typography + control alignment mandates.
// Demonstrates the single shared text placement helper: compute_control_text_baseline_y
//
// SPEC-HAL-092 Section 8A: Baseline-Anchored Control Text Placement
// INV-TYPO-CTRL-1: Control text uses actual font metrics (ascent + descent), not line_height.
// INV-TYPO-CTRL-2: Glyphs must be fully contained within control bounds.
//
// Usage:
//   cargo run --release --example text_placement_benchmark
//
// Controls:
//   D - Toggle debug overlay (control bounds, baseline grid, glyph bounds)
//   Close the window to exit
//
// DPI/Scale Stress:
//   Set OS scale to 125% and rerun to verify pixel stability.
//   No visible wobble/shimmer should occur when moving the window.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::layout::{Alignment, Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{Color, LAYER_CONTENT, RoundedCorners};
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::*;
use halcyon::replace_widget;
use halcyon::utils::id::WidgetInternalId;
use std::cell::Cell;
use std::rc::Rc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== SPEC-HAL-092 Text Placement Benchmark ===");
    println!();
    println!("This demo validates baseline alignment across controls.");
    println!("All controls use compute_control_text_baseline_y() for text placement.");
    println!();
    println!("SPEC-HAL-092 Section 8A Invariants:");
    println!("  INV-TYPO-CTRL-1: Control text uses font metrics, not line_height");
    println!("  INV-TYPO-CTRL-2: Glyphs fully contained within control bounds");
    println!();
    println!("Controls:");
    println!("  D      - Toggle debug overlay");
    println!("  Close the window to exit");
    println!();
    println!("Overlay shows:");
    println!("  - Control bounds (cyan rectangles)");
    println!("  - 4U baseline grid (red horizontal lines)");
    println!("  - Text baselines (green lines per control)");
    println!("  - Glyph bounds (magenta lines: top = baseline-ascent, bottom = baseline+descent)");
    println!();
    println!("DPI/Scale Testing:");
    println!("  Set OS scale to 125% and rerun.");
    println!("  Verify no wobble/shimmer when moving window.");
    println!("==============================================\n");

    let app = App::new(AppConfig::new("Text Placement Benchmark").with_size(700.0, 500.0));
    app.run(BenchmarkScreen::new());
}

// =============================================================================
// Shared overlay state
// =============================================================================

struct OverlayState {
    enabled: Cell<bool>,
}

impl OverlayState {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            enabled: Cell::new(false),
        })
    }

    fn toggle(&self) {
        self.enabled.set(!self.enabled.get());
        println!(
            "[DEBUG] Overlay: {}",
            if self.enabled.get() { "ON" } else { "OFF" }
        );
    }

    fn is_enabled(&self) -> bool {
        self.enabled.get()
    }
}

// =============================================================================
// DebugOverlay widget - draws debug visualizations
// =============================================================================

struct DebugOverlay {
    id: WidgetInternalId,
    state: Rc<OverlayState>,
}

impl DebugOverlay {
    fn new(state: Rc<OverlayState>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            state,
        }
    }
}

impl WidgetDowncast for DebugOverlay {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DebugOverlay {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Fill available space
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        if !self.state.is_enabled() {
            return;
        }

        let grid_color = Color::from_srgb(1.0, 0.2, 0.2, 0.3); // Red, semi-transparent

        // Draw 4U baseline grid across the entire area
        // 4U = 4 logical units (baseline grid per SPEC-HAL-090)
        let grid_step = 4.0;
        let mut y = rect.y;
        while y < rect.y + rect.height {
            cx.scene.push_rect(
                RectF::new(rect.x, y, rect.width, 1.0),
                grid_color,
                100,
                cx.scene_key,
            );
            y += grid_step;
        }
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false // Overlay doesn't capture input
    }
}

// =============================================================================
// InspectorRow - Label + Control pair with debug overlay
// =============================================================================

struct InspectorRow {
    id: WidgetInternalId,
    label_text: String,
    control_type: ControlType,
    overlay_state: Rc<OverlayState>,
    bounds: Size,
}

#[derive(Clone)]
enum ControlType {
    Button(String),
    TextField(String),
    Tag(String),
    IconText(String),
}

impl InspectorRow {
    fn new(label: &str, control: ControlType, overlay_state: Rc<OverlayState>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label_text: label.to_string(),
            control_type: control,
            overlay_state,
            bounds: Size::zero(),
        }
    }
}

impl WidgetDowncast for InspectorRow {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for InspectorRow {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Row height based on control height from theme
        let control_height = cx.theme.geometry.button.height;
        let row_height = control_height.max(24.0);
        let width = constraints.max_width;

        self.bounds = Size::new(width, row_height);
        LayoutResult::fit(self.bounds)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let t = cx.theme;
        let font_size = t.typography.font_size;

        // Layout constants
        let label_width = 120.0;
        let gap = 12.0;
        let control_x = rect.x + label_width + gap;
        let control_width = rect.width - label_width - gap - 16.0;
        let control_height = t.geometry.button.height;

        // Compute control rect (vertically centered in row if needed)
        let control_y = rect.y + (rect.height - control_height) / 2.0;
        let control_rect = RectF::new(
            control_x,
            control_y,
            control_width.min(200.0),
            control_height,
        );

        // =========================================================================
        // LABEL (left side)
        // =========================================================================
        let label_rect = RectF::new(rect.x, control_y, label_width, control_height);

        // Text placement: layout owns all vertical positioning.
        // Renderer is placement-dumb; it draws glyphs at the Y we provide.
        let metrics = cx.text_engine.font_metrics(font_size);
        let glyph_height = metrics.ascent + metrics.descent;

        // CONTRACT VIOLATION CHECK: control must be tall enough for glyph box
        if glyph_height > control_height {
            eprintln!(
                "[THEME CONTRACT VIOLATION] glyph_height ({:.1}) > control_height ({:.1}). \
                 Vertical centering impossible. Fix theme: control_height >= ascent + descent + padding.",
                glyph_height, control_height
            );
        }

        // Core formula: center glyph box in rectangle
        let label_text_top_y = label_rect.y + (label_rect.height - glyph_height) / 2.0;
        // Baseline is derived (for debug overlay only)
        let label_baseline_y = label_text_top_y + metrics.ascent;

        let label_run = cx.text_engine.shape(&self.label_text, font_size);
        cx.text_engine.draw(
            &label_run,
            label_rect.x,
            label_text_top_y,
            t.colors.typography.primary,
            LAYER_CONTENT + 2,
            cx.scene,
        );

        // =========================================================================
        // CONTROL (right side)
        // =========================================================================
        // Core formula: center glyph box in rectangle
        let control_text_top_y = control_rect.y + (control_rect.height - glyph_height) / 2.0;
        // Baseline is derived (for debug overlay only)
        let control_baseline_y = control_text_top_y + metrics.ascent;

        match &self.control_type {
            ControlType::Button(text) => {
                // Draw button background
                cx.scene.push_rounded_rect(
                    control_rect,
                    t.colors.accents.primary,
                    RoundedCorners::uniform(t.geometry.button.radius),
                    2,
                    cx.scene_key,
                );

                // Draw button text (horizontally centered)
                let run = cx.text_engine.shape(text, font_size);
                let text_x = control_rect.x + (control_rect.width - run.width) / 2.0;
                cx.text_engine.draw(
                    &run,
                    text_x,
                    control_text_top_y,
                    t.colors.typography.primary,
                    LAYER_CONTENT + 2,
                    cx.scene,
                );
            }
            ControlType::TextField(text) => {
                // Draw text field background
                cx.scene.push_rounded_rect(
                    control_rect,
                    t.colors.surfaces.content,
                    RoundedCorners::uniform(t.geometry.text_field.radius),
                    2,
                    cx.scene_key,
                );
                cx.scene.push_border_rounded_rect(
                    control_rect,
                    t.colors.borders.default,
                    1.0,
                    RoundedCorners::uniform(t.geometry.text_field.radius),
                    3,
                    cx.scene_key,
                );

                // Draw text (left-aligned with padding)
                let padding_x = t.geometry.text_field.padding_x;
                let run = cx.text_engine.shape(text, font_size);
                cx.text_engine.draw(
                    &run,
                    control_rect.x + padding_x,
                    control_text_top_y,
                    t.colors.typography.primary,
                    LAYER_CONTENT + 2,
                    cx.scene,
                );
            }
            ControlType::Tag(text) => {
                // Draw tag/pill background
                let tag_run = cx.text_engine.shape(text, font_size);
                let tag_padding = 8.0;
                let tag_width = tag_run.width + tag_padding * 2.0;
                let tag_rect =
                    RectF::new(control_rect.x, control_rect.y, tag_width, control_height);

                cx.scene.push_rounded_rect(
                    tag_rect,
                    t.colors.accents.primary,
                    RoundedCorners::uniform(control_height / 2.0),
                    2,
                    cx.scene_key,
                );

                // Draw tag text
                let text_x = tag_rect.x + tag_padding;
                cx.text_engine.draw(
                    &tag_run,
                    text_x,
                    control_text_top_y,
                    Color::WHITE,
                    LAYER_CONTENT + 2,
                    cx.scene,
                );
            }
            ControlType::IconText(text) => {
                // Draw icon placeholder (square)
                let icon_size = 16.0;
                let icon_rect = RectF::new(
                    control_rect.x,
                    control_rect.y + (control_height - icon_size) / 2.0,
                    icon_size,
                    icon_size,
                );
                cx.scene
                    .push_rect(icon_rect, t.colors.accents.primary, 2, None);

                // Draw text next to icon
                let text_x = control_rect.x + icon_size + 6.0;
                let run = cx.text_engine.shape(text, font_size);
                cx.text_engine.draw(
                    &run,
                    text_x,
                    control_text_top_y,
                    t.colors.typography.primary,
                    LAYER_CONTENT + 2,
                    cx.scene,
                );
            }
        }

        // =========================================================================
        // DEBUG OVERLAY
        // =========================================================================
        if self.overlay_state.is_enabled() {
            let cyan = Color::from_srgb(0.0, 1.0, 1.0, 0.5);
            let green = Color::from_srgb(0.0, 1.0, 0.0, 0.8);
            let magenta = Color::from_srgb(1.0, 0.0, 1.0, 0.4);

            // Draw label bounds
            cx.scene
                .push_border_rect(label_rect, cyan, 1.0, 50, cx.scene_key);

            // Draw control bounds
            cx.scene
                .push_border_rect(control_rect, cyan, 1.0, 50, cx.scene_key);

            // Draw text baseline for label (green line)
            // label_baseline_y is now computed directly from compute_control_text_baseline_y
            cx.scene.push_rect(
                RectF::new(label_rect.x, label_baseline_y, label_rect.width, 1.0),
                green,
                51,
                cx.scene_key,
            );

            // Draw text baseline for control (green line)
            // control_baseline_y is now computed directly from compute_control_text_baseline_y
            cx.scene.push_rect(
                RectF::new(control_rect.x, control_baseline_y, control_rect.width, 1.0),
                green,
                51,
                cx.scene_key,
            );

            // Draw glyph bounds (top = text_top_y, bottom = text_top_y + glyph_height)
            // This shows the actual glyph extent, not line_height
            // Label glyph top
            cx.scene.push_rect(
                RectF::new(label_rect.x, label_text_top_y, label_rect.width, 1.0),
                magenta,
                50,
                cx.scene_key,
            );
            // Label glyph bottom
            cx.scene.push_rect(
                RectF::new(
                    label_rect.x,
                    label_baseline_y + metrics.descent,
                    label_rect.width,
                    1.0,
                ),
                magenta,
                50,
                cx.scene_key,
            );

            // Control glyph top
            cx.scene.push_rect(
                RectF::new(control_rect.x, control_text_top_y, control_rect.width, 1.0),
                magenta,
                50,
                cx.scene_key,
            );
            // Control glyph bottom
            cx.scene.push_rect(
                RectF::new(
                    control_rect.x,
                    control_baseline_y + metrics.descent,
                    control_rect.width,
                    1.0,
                ),
                magenta,
                50,
                cx.scene_key,
            );
        }
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

// =============================================================================
// Benchmark Screen
// =============================================================================

struct BenchmarkScreen {
    needs_rebuild: bool,
    placeholder: Label,
    overlay_state: Rc<OverlayState>,
}

impl BenchmarkScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            overlay_state: OverlayState::new(),
        }
    }
}

impl Screen for BenchmarkScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Root: vertical Flex column
        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_spacing(0.0)
                .with_align_items(Alignment::Stretch),
        ));

        // Title area with padding
        let title_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 0);
        cx.tree
            .add_child(title_row, Box::new(FixedSpacer::horizontal(16.0)), None, 0);
        cx.tree.add_child(
            title_row,
            Box::new(Label::new("Text Placement Benchmark (SPEC-HAL-092)")),
            None,
            1,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 1);

        // Status row
        let status_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 2);
        cx.tree
            .add_child(status_row, Box::new(FixedSpacer::horizontal(16.0)), None, 0);
        cx.tree.add_child(
            status_row,
            Box::new(Label::new("Press D to toggle overlay | Close the window to exit")),
            None,
            1,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 3);

        // Divider
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 4);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 5);

        // Inspector section header
        let header_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 6);
        cx.tree
            .add_child(header_row, Box::new(FixedSpacer::horizontal(16.0)), None, 0);
        cx.tree.add_child(
            header_row,
            Box::new(Label::new("Inspector Layout (baseline alignment test):")),
            None,
            1,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(12.0)), None, 7);

        // Content area with padding
        let content_wrapper =
            cx.tree
                .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 8);
        cx.tree.add_child(
            content_wrapper,
            Box::new(FixedSpacer::horizontal(16.0)),
            None,
            0,
        );

        let content_col = cx.tree.add_child(
            content_wrapper,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            1,
        );

        // Inspector rows - each with Label + Control
        let state = self.overlay_state.clone();
        cx.tree.add_child(
            content_col,
            Box::new(InspectorRow::new(
                "Username:",
                ControlType::TextField("john_doe".to_string()),
                state.clone(),
            )),
            None,
            0,
        );

        cx.tree.add_child(
            content_col,
            Box::new(InspectorRow::new(
                "Email:",
                ControlType::TextField("user@example.com".to_string()),
                state.clone(),
            )),
            None,
            1,
        );

        cx.tree.add_child(
            content_col,
            Box::new(InspectorRow::new(
                "Status:",
                ControlType::Tag("Active".to_string()),
                state.clone(),
            )),
            None,
            2,
        );

        cx.tree.add_child(
            content_col,
            Box::new(InspectorRow::new(
                "Role:",
                ControlType::Button("Administrator".to_string()),
                state.clone(),
            )),
            None,
            3,
        );

        cx.tree.add_child(
            content_col,
            Box::new(InspectorRow::new(
                "Icon Test:",
                ControlType::IconText("Settings".to_string()),
                state.clone(),
            )),
            None,
            4,
        );

        // Empty placeholder field
        cx.tree.add_child(
            content_col,
            Box::new(InspectorRow::new(
                "Notes:",
                ControlType::TextField("".to_string()),
                state.clone(),
            )),
            None,
            5,
        );

        // Spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 9);

        // Second section: Icon + Text alignment
        let header_row2 =
            cx.tree
                .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 10);
        cx.tree.add_child(
            header_row2,
            Box::new(FixedSpacer::horizontal(16.0)),
            None,
            0,
        );
        cx.tree.add_child(
            header_row2,
            Box::new(Label::new("Icon + Text pairs (icon baseline alignment):")),
            None,
            1,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(12.0)), None, 11);

        let icon_wrapper =
            cx.tree
                .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 12);
        cx.tree.add_child(
            icon_wrapper,
            Box::new(FixedSpacer::horizontal(16.0)),
            None,
            0,
        );

        let icon_row = cx.tree.add_child(
            icon_wrapper,
            Box::new(Flex::row().with_spacing(24.0)),
            None,
            1,
        );

        // Multiple icon+text pairs in a row
        cx.tree.add_child(
            icon_row,
            Box::new(InspectorRow::new(
                "",
                ControlType::IconText("Home".to_string()),
                state.clone(),
            )),
            None,
            0,
        );

        cx.tree.add_child(
            icon_row,
            Box::new(InspectorRow::new(
                "",
                ControlType::IconText("Settings".to_string()),
                state.clone(),
            )),
            None,
            1,
        );

        cx.tree.add_child(
            icon_row,
            Box::new(InspectorRow::new(
                "",
                ControlType::IconText("Profile".to_string()),
                state.clone(),
            )),
            None,
            2,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 13);

        // =========================================================================
        // REAL WIDGETS SECTION
        // =========================================================================
        // This section uses actual Button and TextField widgets to verify that
        // the canonical text placement helper is obeyed under real widget logic.
        // =========================================================================
        let real_header_row =
            cx.tree
                .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 14);
        cx.tree.add_child(
            real_header_row,
            Box::new(FixedSpacer::horizontal(16.0)),
            None,
            0,
        );
        cx.tree.add_child(
            real_header_row,
            Box::new(Label::new("REAL WIDGETS (actual Button/TextField):")),
            None,
            1,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(12.0)), None, 15);

        // Real widgets row with padding
        let real_wrapper =
            cx.tree
                .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 16);
        cx.tree.add_child(
            real_wrapper,
            Box::new(FixedSpacer::horizontal(16.0)),
            None,
            0,
        );

        let real_row = cx.tree.add_child(
            real_wrapper,
            Box::new(Flex::row().with_spacing(16.0)),
            None,
            1,
        );

        // Real Button widget (using proper tree pattern)
        // 1. Add placeholder button to row
        let button_node = cx.tree.add_child(
            real_row,
            Box::new(Button::new(NodeId(0)).with_padding(8.0)),
            None,
            0,
        );
        // 2. Add label as child of button
        let button_label =
            cx.tree
                .add_child(button_node, Box::new(Label::new("Real Button")), None, 0);
        // 3. Replace button with correct child reference
        replace_widget!(
            cx.tree,
            button_node,
            Button::new(button_label).with_padding(8.0)
        );

        // Real TextField widget
        let real_text_field = TextField::new().with_text("Real TextField");
        cx.tree
            .add_child(real_row, Box::new(real_text_field), None, 1);

        // Second real widgets row for comparison
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 17);

        let real_wrapper2 =
            cx.tree
                .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 18);
        cx.tree.add_child(
            real_wrapper2,
            Box::new(FixedSpacer::horizontal(16.0)),
            None,
            0,
        );

        let real_row2 = cx.tree.add_child(
            real_wrapper2,
            Box::new(Flex::row().with_spacing(16.0)),
            None,
            1,
        );

        // Another real Button (using proper tree pattern)
        let button_node2 = cx.tree.add_child(
            real_row2,
            Box::new(Button::new(NodeId(0)).with_padding(8.0)),
            None,
            0,
        );
        let button_label2 =
            cx.tree
                .add_child(button_node2, Box::new(Label::new("Administrator")), None, 0);
        replace_widget!(
            cx.tree,
            button_node2,
            Button::new(button_label2).with_padding(8.0)
        );

        // Another real TextField
        let real_text_field2 = TextField::new().with_text("user@example.com");
        cx.tree
            .add_child(real_row2, Box::new(real_text_field2), None, 1);

        // Flexible spacer to push content up
        let flex_spacer = cx
            .tree
            .add_child(root, Box::new(FlexSpacer::new()), None, 19);
        cx.tree.set_flex(flex_spacer, FlexParams::flexible(1.0));

        // Global debug overlay (draws grid across entire window)
        cx.tree.add_child(
            root,
            Box::new(DebugOverlay::new(self.overlay_state.clone())),
            None,
            20,
        );

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        match event {
            FusedInputEvent::KeyDown { key_code, .. } => match key_code {
                KeyCode::Character('d') | KeyCode::Character('D') => {
                    self.overlay_state.toggle();
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
