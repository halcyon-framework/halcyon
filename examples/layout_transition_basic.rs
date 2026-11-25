// SPEC-HAL-078 Stage 3: Layout Transition Basic Demo
//
// Demonstrates layout transitions by toggling widget sizes.
// Layout transitions require:
// - Same TreeWidgetId across frames (no tree rebuild)
// - Rect changes between frames (size or position change)
// - Widget NOT inside scroll container (conflict denial)
//
// This demo shows:
// 1. Basic case: Collapsible panel that animates height changes
// 2. Nested case: Child inside parent that transitions (child carried by parent)
// 3. Scroll area case: Widgets inside scroll area do NOT transition (CONFLICT-1)

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{
    Event, EventPhase, FusedInputEvent, PointerButton, PointerEventKind,
};
use halcyon::core::layout::{Constraints, FlexParams, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{Color, RoundedCorners};
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::*;
use halcyon::replace_widget;
use halcyon::utils::id::WidgetInternalId;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== SPEC-HAL-078 Stage 3: Layout Transition Demo ===");
    println!();
    println!("INSTRUCTIONS:");
    println!("  Click 'Toggle Panel' button to expand/collapse the panel.");
    println!("  The panel and widgets below it should animate smoothly.");
    println!();
    println!("WHAT TO OBSERVE:");
    println!("  1. Panel height animates (not instant jump)");
    println!("  2. Widgets below panel slide up/down smoothly");
    println!("  3. Child inside panel is carried by parent (no double-move)");
    println!("  4. Widget inside scroll area does NOT animate (denied by CONFLICT-1)");
    println!();
    println!("Close the window to exit.");
    println!("===================================================\n");

    let app = App::new(AppConfig::new("Layout Transition Demo").with_size(600.0, 500.0));
    app.run(LayoutTransitionScreen::new());
}

// ============================================================================
// Screen Implementation
// ============================================================================

struct LayoutTransitionScreen {
    needs_rebuild: bool,
    first_build: bool,
    placeholder: Label,
}

impl LayoutTransitionScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            first_build: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for LayoutTransitionScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Only build the tree once. Subsequent frames reuse existing widgets.
        // This is CRITICAL for layout transitions - widgets must keep same TreeWidgetId.
        if !self.first_build {
            return &mut self.placeholder;
        }
        self.first_build = false;

        // Root column
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));
        cx.tree.set_flex(root, FlexParams::flexible(1.0));

        // Title
        cx.tree.add_child(
            root,
            Box::new(Label::new("SPEC-HAL-078: Layout Transition Demo")),
            None,
            0,
        );

        // Toggle button (wrapped in a row for centering)
        let button_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 1);

        // ToggleButton - a custom widget that controls the panel
        let toggle_button_node =
            cx.tree
                .add_child(button_row, Box::new(Label::new("placeholder")), None, 0);
        let toggle_label = cx.tree.add_child(
            toggle_button_node,
            Box::new(Label::new("Toggle Panel")),
            None,
            0,
        );
        replace_widget!(cx.tree, toggle_button_node, ToggleButton::new(toggle_label));

        // CollapsiblePanel - animates height when toggled
        let panel_node = cx.tree.add_child(
            root,
            Box::new(CollapsiblePanel::new(true)), // starts expanded
            None,
            2,
        );

        // Child inside the panel - should be carried by parent transition (no double-move)
        cx.tree.add_child(
            panel_node,
            Box::new(Label::new("Child inside panel (carried by parent)")),
            None,
            0,
        );

        // Widgets below the panel - these should slide up/down when panel collapses/expands
        cx.tree.add_child(
            root,
            Box::new(Label::new("Widget below panel - slides when panel changes")),
            None,
            3,
        );

        // Another widget below
        cx.tree.add_child(
            root,
            Box::new(Label::new("Another widget below - also slides")),
            None,
            4,
        );

        // Divider
        cx.tree.add_child(
            root,
            Box::new(Label::new("--- Scroll Area (transitions denied) ---")),
            None,
            5,
        );

        // Scroll area case - transitions should be DENIED for widgets inside
        // Because CONFLICT-1: inside_scroll_container = true
        let scroll_placeholder = ScrollArea::new(NodeId(0));
        let scroll_node = cx
            .tree
            .add_child(root, Box::new(scroll_placeholder), None, 6);
        cx.tree.set_flex(scroll_node, FlexParams::flexible(1.0));

        // Content inside scroll area
        let scroll_content = cx.tree.add_child(
            scroll_node,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            0,
        );

        // Add several items to scroll content
        for i in 0..10 {
            cx.tree.add_child(
                scroll_content,
                Box::new(Label::new(format!("Scroll item {} (no transition)", i + 1))),
                None,
                i as i16,
            );
        }

        // Update scroll area with correct content node
        replace_widget!(cx.tree, scroll_node, ScrollArea::new(scroll_content));

        // Help text at bottom
        cx.tree
            .add_child(root, Box::new(Label::new("Close the window to exit")), None, 7);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}

// ============================================================================
// ToggleButton - Sends toggle signal to sibling CollapsiblePanel
// ============================================================================

struct ToggleButton {
    id: WidgetInternalId,
    #[allow(dead_code)]
    label_child: NodeId,
    pressed: bool,
}

impl ToggleButton {
    fn new(label_child: NodeId) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label_child,
            pressed: false,
        }
    }
}

impl WidgetDowncast for ToggleButton {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ToggleButton {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // Layout child label
        let child_size = if !children.is_empty() {
            cx.layout_child(children[0], constraints)
        } else {
            Size::new(100.0, 24.0)
        };

        // Add padding
        let padding = 12.0;
        let size = Size::new(
            child_size.width + padding * 2.0,
            child_size.height + padding * 2.0,
        );

        // Position child centered
        if !children.is_empty() {
            cx.set_child_rect(
                children[0],
                RectF::new(padding, padding, child_size.width, child_size.height),
            );
        }

        LayoutResult::fit(constraints.constrain(size))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Draw button background
        let color = if self.pressed {
            Color::from_srgb(0.2, 0.5, 0.8, 1.0)
        } else {
            Color::from_srgb(0.3, 0.6, 0.9, 1.0)
        };
        cx.scene
            .push_rounded_rect(rect, color, RoundedCorners::uniform(4.0), 2, cx.scene_key);

        // Draw border
        cx.scene.push_border_rounded_rect(
            rect,
            Color::from_srgb(0.1, 0.3, 0.5, 1.0),
            2.0,
            RoundedCorners::uniform(4.0),
            3,
            cx.scene_key,
        );
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Only handle events in Target phase
        if phase != EventPhase::Target {
            return false;
        }

        match event {
            Event::Pointer(pe) => match pe.kind {
                PointerEventKind::Down => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    self.pressed = true;
                    if let Some(button) = pe.button {
                        cx.capture_pointer(button);
                    }
                    true
                }
                PointerEventKind::Up => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    if self.pressed {
                        self.pressed = false;
                        // Signal toggle to the panel via layout invalidation
                        // The CollapsiblePanel will toggle on next frame
                        TOGGLE_SIGNAL.store(true, std::sync::atomic::Ordering::SeqCst);
                        cx.invalidate_layout();
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
            _ => false,
        }
    }
}

// Global toggle signal (simple approach for demo)
static TOGGLE_SIGNAL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// ============================================================================
// CollapsiblePanel - Animates height on toggle
// ============================================================================

struct CollapsiblePanel {
    id: WidgetInternalId,
    expanded: bool,
}

impl CollapsiblePanel {
    fn new(expanded: bool) -> Self {
        Self {
            id: WidgetInternalId::new(),
            expanded,
        }
    }
}

impl WidgetDowncast for CollapsiblePanel {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for CollapsiblePanel {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // Check for toggle signal
        if TOGGLE_SIGNAL.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.expanded = !self.expanded;
            println!(
                "[CollapsiblePanel] Toggled to {}",
                if self.expanded {
                    "EXPANDED"
                } else {
                    "COLLAPSED"
                }
            );
        }

        // Height depends on expanded state
        let target_height = if self.expanded { 120.0 } else { 40.0 };
        let width = constraints.max_width.min(400.0);

        // Layout children
        let padding = 8.0;
        let child_constraints = Constraints::new(
            0.0,
            width - padding * 2.0,
            0.0,
            target_height - padding * 2.0,
        );

        let mut y_offset = padding;
        for &child in children {
            let child_size = cx.layout_child(child, child_constraints);
            cx.set_child_rect(
                child,
                RectF::new(padding, y_offset, child_size.width, child_size.height),
            );
            y_offset += child_size.height + 4.0;
        }

        let size = Size::new(width, target_height);
        LayoutResult::fit(constraints.constrain(size))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Draw panel background
        let bg_color = if self.expanded {
            Color::from_srgb(0.2, 0.4, 0.3, 1.0)
        } else {
            Color::from_srgb(0.3, 0.3, 0.3, 1.0)
        };
        cx.scene.push_rounded_rect(
            rect,
            bg_color,
            RoundedCorners::uniform(4.0),
            2,
            cx.scene_key,
        );

        // Draw border
        cx.scene.push_border_rounded_rect(
            rect,
            Color::from_srgb(0.4, 0.6, 0.5, 1.0),
            2.0,
            RoundedCorners::uniform(4.0),
            3,
            cx.scene_key,
        );
    }
}
