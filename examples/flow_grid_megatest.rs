// FlowGrid Megatest - Multiple constraint behaviors demonstration
// Tests various FlowGrid and Container constraint combinations
// SPEC-HAL-018: Migrated to App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::layout::FlexParams;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::replace_widget;

const GUTTER: f32 = 8.0;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== FlowGrid Megatest ===");
    println!("Demonstrates 5 different constraint behaviors:");
    println!("1. Responsive Elastic - items stretch to fill");
    println!("2. Fixed Tiles - fixed width AND height");
    println!("3. Fixed Width Only - natural height from content");
    println!("4. Min Width Only - standard FlowGrid behavior");
    println!("5. Mixed Constraints - various combinations");
    println!("Resize window to see different behaviors");
    println!("===========================");

    let mut config =
        AppConfig::new("FlowGrid Megatest - Constraint Behaviors").with_size(900.0, 800.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let app = App::new(config);
    app.run(FlowGridMegatestScreen::new());
}

struct FlowGridMegatestScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl FlowGridMegatestScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for FlowGridMegatestScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Root ScrollArea with padding container
        let scroll_area = cx.mount_child(Box::new(ScrollArea::new(NodeId(0))));
        let tree = &mut cx.tree;

        // Create vertical Flex::column to stack all sections
        let main_column = tree.add_child(
            scroll_area,
            Box::new(Flex::column().with_spacing(16.0)),
            None,
            0,
        );

        // ========================================================================
        // SECTION 1: Responsive Elastic FlowGrid
        // ========================================================================
        add_section_header(tree, main_column, "Section 1 - Responsive Elastic FlowGrid");

        let section1_grid = tree.add_child(
            main_column,
            Box::new(FlowGrid::new(100.0).with_spacing(GUTTER)),
            None,
            0,
        );

        for i in 1..=8 {
            add_elastic_button(tree, section1_grid, &format!("Elastic {}", i), i);
        }

        // ========================================================================
        // SECTION 2: Fixed Tile Grid (fixed width AND height)
        // ========================================================================
        add_section_header(tree, main_column, "Section 2 - Fixed Tile Grid (100x60)");

        let section2_grid = tree.add_child(
            main_column,
            Box::new(
                Flex::row()
                    .with_wrap()
                    .with_spacing(GUTTER)
                    .with_align_items(Alignment::Start),
            ),
            None,
            0,
        );

        for i in 1..=10 {
            add_fixed_tile(tree, section2_grid, &format!("Tile {}", i), 100.0, 60.0, i);
        }

        // ========================================================================
        // SECTION 3: Fixed Width Only (natural height from content)
        // ========================================================================
        add_section_header(tree, main_column, "Section 3 - Fixed Width Only (120px)");

        let section3_grid = tree.add_child(
            main_column,
            Box::new(
                Flex::row()
                    .with_wrap()
                    .with_spacing(GUTTER)
                    .with_align_items(Alignment::Start),
            ),
            None,
            0,
        );

        for i in 1..=8 {
            add_fixed_width_button(tree, section3_grid, &format!("Width {}", i), 120.0, i);
        }

        // ========================================================================
        // SECTION 4: Min Width Only (standard FlowGrid behavior)
        // ========================================================================
        add_section_header(tree, main_column, "Section 4 - Min Width Only (80px basis)");

        let section4_grid = tree.add_child(
            main_column,
            Box::new(FlowGrid::new(80.0).with_spacing(GUTTER)),
            None,
            0,
        );

        for i in 1..=12 {
            add_elastic_button(tree, section4_grid, &format!("Min {}", i), i + 100);
        }

        // ========================================================================
        // SECTION 5: Mixed Constraints Grid
        // ========================================================================
        add_section_header(tree, main_column, "Section 5 - Mixed Constraints");

        let section5_grid = tree.add_child(
            main_column,
            Box::new(
                Flex::row()
                    .with_wrap()
                    .with_spacing(GUTTER)
                    .with_align_items(Alignment::Start),
            ),
            None,
            0,
        );

        // Fixed width items
        add_fixed_width_button(tree, section5_grid, "FW 100", 100.0, 201);
        add_fixed_width_button(tree, section5_grid, "FW 150", 150.0, 202);

        // Fixed height items (with grow)
        add_fixed_height_button(tree, section5_grid, "FH 40", 40.0, 203);
        add_fixed_height_button(tree, section5_grid, "FH 60", 60.0, 204);

        // Fixed tile items
        add_fixed_tile(tree, section5_grid, "80x50", 80.0, 50.0, 205);
        add_fixed_tile(tree, section5_grid, "120x70", 120.0, 70.0, 206);

        // Elastic items with basis
        add_elastic_with_basis(tree, section5_grid, "Grow 1", 100.0, 1.0, 207);
        add_elastic_with_basis(tree, section5_grid, "Grow 2", 100.0, 2.0, 208);

        // Update scroll_area with correct content NodeId
        replace_widget!(tree, scroll_area, ScrollArea::new(main_column));

        &mut self.placeholder
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}

// Helper: Add section header label
fn add_section_header(tree: &mut WidgetTree, parent: NodeId, text: &str) {
    let header_container = tree.add_child(
        parent,
        Box::new(
            Container::new(NodeId(0), 8.0)
                .with_background(Color::from_hex(0x333333))
                .with_radius(4.0),
        ),
        None,
        0,
    );

    let header_label = tree.add_child(
        header_container,
        Box::new(
            Label::new(text)
                .with_size(16.0)
                .with_color(Color::from_hex(0xffffff)),
        ),
        None,
        0,
    );

    // Update container with correct child
    replace_widget!(
        tree,
        header_container,
        Container::new(header_label, 8.0)
            .with_background(Color::from_hex(0x333333))
            .with_radius(4.0)
    );
}

// Helper: Add elastic button (for FlowGrid - stretches to fill)
fn add_elastic_button(tree: &mut WidgetTree, parent: NodeId, text: &str, id: usize) {
    let text_owned = text.to_string();
    let container = tree.add_child(parent, Box::new(Container::new(NodeId(0), 0.0)), None, 0);

    let button = tree.add_child(
        container,
        Box::new(Button::new(NodeId(0)).with_padding(8.0)),
        None,
        0,
    );

    let label = tree.add_child(
        button,
        Box::new(Label::new(text).with_size(12.0).with_color(Color::WHITE)),
        None,
        0,
    );

    // Update button with correct child
    replace_widget!(
        tree,
        button,
        Button::new(label).with_padding(8.0).on_click(move || {
            println!("CLICK: {} (id={})", text_owned, id);
        })
    );

    // Update container with button and fixed height for uniform look
    replace_widget!(
        tree,
        container,
        Container::new(button, 0.0).with_height(40.0)
    );
}

// Helper: Add fixed tile (fixed width AND height)
fn add_fixed_tile(
    tree: &mut WidgetTree,
    parent: NodeId,
    text: &str,
    width: f32,
    height: f32,
    id: usize,
) {
    let text_owned = text.to_string();
    let container = tree.add_child(parent, Box::new(Container::new(NodeId(0), 0.0)), None, 0);

    let button = tree.add_child(
        container,
        Box::new(Button::new(NodeId(0)).with_padding(4.0)),
        None,
        0,
    );

    let label = tree.add_child(
        button,
        Box::new(Label::new(text).with_size(11.0).with_color(Color::WHITE)),
        None,
        0,
    );

    // Update button with correct child
    replace_widget!(
        tree,
        button,
        Button::new(label).with_padding(4.0).on_click(move || {
            println!("CLICK: {} (id={})", text_owned, id);
        })
    );

    // Update container with fixed width AND height
    replace_widget!(
        tree,
        container,
        Container::new(button, 0.0)
            .with_width(width)
            .with_height(height)
    );
}

// Helper: Add fixed width button (natural height)
fn add_fixed_width_button(
    tree: &mut WidgetTree,
    parent: NodeId,
    text: &str,
    width: f32,
    id: usize,
) {
    let text_owned = text.to_string();
    let container = tree.add_child(parent, Box::new(Container::new(NodeId(0), 0.0)), None, 0);

    let button = tree.add_child(
        container,
        Box::new(Button::new(NodeId(0)).with_padding(8.0)),
        None,
        0,
    );

    let label = tree.add_child(
        button,
        Box::new(Label::new(text).with_size(12.0).with_color(Color::WHITE)),
        None,
        0,
    );

    // Update button with correct child
    replace_widget!(
        tree,
        button,
        Button::new(label).with_padding(8.0).on_click(move || {
            println!("CLICK: {} (id={})", text_owned, id);
        })
    );

    // Update container with fixed width only
    replace_widget!(
        tree,
        container,
        Container::new(button, 0.0).with_width(width)
    );
}

// Helper: Add fixed height button (with grow for width)
fn add_fixed_height_button(
    tree: &mut WidgetTree,
    parent: NodeId,
    text: &str,
    height: f32,
    id: usize,
) {
    let text_owned = text.to_string();
    let container = tree.add_child(parent, Box::new(Container::new(NodeId(0), 0.0)), None, 0);

    let button = tree.add_child(
        container,
        Box::new(Button::new(NodeId(0)).with_padding(8.0)),
        None,
        0,
    );

    let label = tree.add_child(
        button,
        Box::new(Label::new(text).with_size(12.0).with_color(Color::WHITE)),
        None,
        0,
    );

    // Update button with correct child
    replace_widget!(
        tree,
        button,
        Button::new(label).with_padding(8.0).on_click(move || {
            println!("CLICK: {} (id={})", text_owned, id);
        })
    );

    // Update container with fixed height only
    replace_widget!(
        tree,
        container,
        Container::new(button, 0.0).with_height(height)
    );

    // Set flex params for grow behavior
    tree.set_flex(
        container,
        FlexParams {
            grow: 1.0,
            shrink: 1.0,
            basis: 80.0,
        },
    );
}

// Helper: Add elastic button with custom basis and grow
fn add_elastic_with_basis(
    tree: &mut WidgetTree,
    parent: NodeId,
    text: &str,
    basis: f32,
    grow: f32,
    id: usize,
) {
    let text_owned = text.to_string();
    let container = tree.add_child(parent, Box::new(Container::new(NodeId(0), 0.0)), None, 0);

    let button = tree.add_child(
        container,
        Box::new(Button::new(NodeId(0)).with_padding(8.0)),
        None,
        0,
    );

    let label = tree.add_child(
        button,
        Box::new(Label::new(text).with_size(12.0).with_color(Color::WHITE)),
        None,
        0,
    );

    // Update button with correct child
    replace_widget!(
        tree,
        button,
        Button::new(label).with_padding(8.0).on_click(move || {
            println!("CLICK: {} (id={})", text_owned, id);
        })
    );

    // Update container with fixed height
    replace_widget!(
        tree,
        container,
        Container::new(button, 0.0).with_height(40.0)
    );

    // Set custom flex params
    tree.set_flex(
        container,
        FlexParams {
            grow,
            shrink: 1.0,
            basis,
        },
    );
}
