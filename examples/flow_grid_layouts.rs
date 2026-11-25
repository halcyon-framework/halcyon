// FlowGrid Layouts - Demonstrates horizontal section arrangements
// Tests FlowGrids appearing side-by-side using Flex row + wrap
// SPEC-HAL-018: Migrated to App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::layout::{FlexParams, Justification};
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::replace_widget;

const GUTTER: f32 = 8.0;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== FlowGrid Layouts ===");
    println!("Demonstrates horizontal FlowGrid arrangements:");
    println!("1. Two FlowGrids side-by-side");
    println!("2. Triple section row (wraps when narrow)");
    println!("3. One wide + one narrow (proportional)");
    println!("4. Nested FlowGrid inside row");
    println!("5. FlowGrid-of-FlowGrids");
    println!("Resize window to see wrap behavior");
    println!("=========================");

    let mut config =
        AppConfig::new("FlowGrid Layouts - Horizontal Arrangements").with_size(1000.0, 900.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let app = App::new(config);
    app.run(FlowGridLayoutsScreen::new());
}

struct FlowGridLayoutsScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl FlowGridLayoutsScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for FlowGridLayoutsScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Root: ScrollArea
        let scroll_area = cx.mount_child(Box::new(ScrollArea::new(NodeId(0))));
        let tree = &mut cx.tree;

        // Main vertical column to stack all sections
        let main_column = tree.add_child(
            scroll_area,
            Box::new(Flex::column().with_spacing(16.0)),
            None,
            0,
        );

        // ========================================================================
        // SECTION 1: Two FlowGrids Side-by-Side
        // ========================================================================
        add_section_header(tree, main_column, "Section 1 - Horizontal Pair");

        let section1_row = tree.add_child(
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

        // Left: Elastic FlowGrid
        let left_container = tree.add_child(
            section1_row,
            Box::new(
                Container::new(NodeId(0), 4.0)
                    .with_background(Color::from_hex(0x2a2a2a))
                    .with_radius(4.0),
            ),
            None,
            0,
        );

        let left_grid = tree.add_child(
            left_container,
            Box::new(FlowGrid::new(80.0).with_spacing(4.0)),
            None,
            0,
        );

        for i in 1..=6 {
            add_elastic_button(tree, left_grid, &format!("L{}", i), i);
        }

        // Update left container
        replace_widget!(
            tree,
            left_container,
            Container::new(left_grid, 4.0)
                .with_background(Color::from_hex(0x2a2a2a))
                .with_radius(4.0)
        );

        tree.set_flex(
            left_container,
            FlexParams {
                grow: 1.0,
                shrink: 1.0,
                basis: 200.0,
            },
        );

        // Right: Fixed tile grid
        let right_container = tree.add_child(
            section1_row,
            Box::new(
                Container::new(NodeId(0), 4.0)
                    .with_background(Color::from_hex(0x2a3a2a))
                    .with_radius(4.0),
            ),
            None,
            0,
        );

        let right_grid = tree.add_child(
            right_container,
            Box::new(
                Flex::row()
                    .with_wrap()
                    .with_spacing(4.0)
                    .with_justify_content(Justification::Center),
            ),
            None,
            0,
        );

        for i in 1..=4 {
            add_fixed_tile(tree, right_grid, &format!("R{}", i), 80.0, 50.0, 100 + i);
        }

        // Update right container
        replace_widget!(
            tree,
            right_container,
            Container::new(right_grid, 4.0)
                .with_background(Color::from_hex(0x2a3a2a))
                .with_radius(4.0)
        );

        tree.set_flex(
            right_container,
            FlexParams {
                grow: 0.0,
                shrink: 0.0,
                basis: 180.0,
            },
        );

        // ========================================================================
        // SECTION 2: Triple Section Row (wraps when narrow)
        // ========================================================================
        add_section_header(tree, main_column, "Section 2 - Triple Row (Wraps)");

        let section2_row = tree.add_child(
            main_column,
            Box::new(
                Flex::row()
                    .with_wrap()
                    .with_spacing(GUTTER)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            0,
        );

        // Three FlowGrids with min width 250px each
        let colors = [0x2a2a3a, 0x3a2a2a, 0x2a3a3a];
        for (idx, &bg_color) in colors.iter().enumerate() {
            let panel = tree.add_child(
                section2_row,
                Box::new(
                    Container::new(NodeId(0), 4.0)
                        .with_background(Color::from_hex(bg_color))
                        .with_radius(4.0),
                ),
                None,
                0,
            );

            let grid = tree.add_child(
                panel,
                Box::new(FlowGrid::new(70.0).with_spacing(4.0)),
                None,
                0,
            );

            for i in 1..=4 {
                add_elastic_button(
                    tree,
                    grid,
                    &format!("P{}-{}", idx + 1, i),
                    200 + idx * 10 + i,
                );
            }

            // Update panel
            replace_widget!(
                tree,
                panel,
                Container::new(grid, 4.0)
                    .with_background(Color::from_hex(bg_color))
                    .with_radius(4.0)
            );

            tree.set_flex(
                panel,
                FlexParams {
                    grow: 1.0,
                    shrink: 1.0,
                    basis: 250.0,
                },
            );
        }

        // ========================================================================
        // SECTION 3: One Wide + One Narrow
        // ========================================================================
        add_section_header(tree, main_column, "Section 3 - Wide + Narrow");

        let section3_row = tree.add_child(
            main_column,
            Box::new(
                Flex::row()
                    .with_wrap()
                    .with_spacing(GUTTER)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            0,
        );

        // Wide section: grows to fill
        let wide_container = tree.add_child(
            section3_row,
            Box::new(
                Container::new(NodeId(0), 4.0)
                    .with_background(Color::from_hex(0x3a3a2a))
                    .with_radius(4.0),
            ),
            None,
            0,
        );

        let wide_grid = tree.add_child(
            wide_container,
            Box::new(FlowGrid::new(90.0).with_spacing(4.0)),
            None,
            0,
        );

        for i in 1..=8 {
            add_elastic_button(tree, wide_grid, &format!("Wide {}", i), 300 + i);
        }

        replace_widget!(
            tree,
            wide_container,
            Container::new(wide_grid, 4.0)
                .with_background(Color::from_hex(0x3a3a2a))
                .with_radius(4.0)
        );

        tree.set_flex(
            wide_container,
            FlexParams {
                grow: 1.0,
                shrink: 1.0,
                basis: 300.0,
            },
        );

        // Narrow section: fixed 200px width
        let narrow_container = tree.add_child(
            section3_row,
            Box::new(
                Container::new(NodeId(0), 4.0)
                    .with_background(Color::from_hex(0x2a3a4a))
                    .with_radius(4.0)
                    .with_width(200.0),
            ),
            None,
            0,
        );

        let narrow_grid = tree.add_child(
            narrow_container,
            Box::new(FlowGrid::new(80.0).with_spacing(4.0)),
            None,
            0,
        );

        for i in 1..=3 {
            add_elastic_button(tree, narrow_grid, &format!("N{}", i), 350 + i);
        }

        replace_widget!(
            tree,
            narrow_container,
            Container::new(narrow_grid, 4.0)
                .with_background(Color::from_hex(0x2a3a4a))
                .with_radius(4.0)
                .with_width(200.0)
        );

        // ========================================================================
        // SECTION 4: Nested FlowGrid inside Row
        // ========================================================================
        add_section_header(tree, main_column, "Section 4 - Nested FlowGrids");

        let section4_row = tree.add_child(
            main_column,
            Box::new(
                Flex::row()
                    .with_wrap()
                    .with_spacing(GUTTER)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            0,
        );

        // Two nested FlowGrid containers
        for outer_idx in 1..=2 {
            let outer_container = tree.add_child(
                section4_row,
                Box::new(
                    Container::new(NodeId(0), 8.0)
                        .with_background(Color::from_hex(if outer_idx == 1 {
                            0x2a2a4a
                        } else {
                            0x4a2a2a
                        }))
                        .with_radius(6.0),
                ),
                None,
                0,
            );

            // Inner column with label + FlowGrid
            let inner_column = tree.add_child(
                outer_container,
                Box::new(Flex::column().with_spacing(4.0)),
                None,
                0,
            );

            // Add inner label
            tree.add_child(
                inner_column,
                Box::new(
                    Label::new(&format!("Nested {}", outer_idx))
                        .with_size(12.0)
                        .with_color(Color::from_hex(0xaaaaaa)),
                ),
                None,
                0,
            );

            let inner_grid = tree.add_child(
                inner_column,
                Box::new(FlowGrid::new(60.0).with_spacing(4.0)),
                None,
                0,
            );

            for i in 1..=6 {
                add_elastic_button(
                    tree,
                    inner_grid,
                    &format!("{}-{}", outer_idx, i),
                    400 + outer_idx * 10 + i,
                );
            }

            // Update outer container
            replace_widget!(
                tree,
                outer_container,
                Container::new(inner_column, 8.0)
                    .with_background(Color::from_hex(if outer_idx == 1 {
                        0x2a2a4a
                    } else {
                        0x4a2a2a
                    }))
                    .with_radius(6.0)
            );

            tree.set_flex(
                outer_container,
                FlexParams {
                    grow: 1.0,
                    shrink: 1.0,
                    basis: 200.0,
                },
            );
        }

        // ========================================================================
        // SECTION 5: FlowGrid-of-FlowGrids
        // ========================================================================
        add_section_header(tree, main_column, "Section 5 - FlowGrid of FlowGrids");

        let section5_grid = tree.add_child(
            main_column,
            Box::new(FlowGrid::new(200.0).with_spacing(GUTTER)),
            None,
            0,
        );

        // Each item in the outer FlowGrid is itself a FlowGrid container
        for outer_idx in 1..=4 {
            let item_container = tree.add_child(
                section5_grid,
                Box::new(
                    Container::new(NodeId(0), 6.0)
                        .with_background(Color::from_hex(0x3a3a3a))
                        .with_radius(4.0),
                ),
                None,
                0,
            );

            let inner_column = tree.add_child(
                item_container,
                Box::new(Flex::column().with_spacing(4.0)),
                None,
                0,
            );

            // Label for inner grid
            tree.add_child(
                inner_column,
                Box::new(
                    Label::new(&format!("Grid {}", outer_idx))
                        .with_size(11.0)
                        .with_color(Color::from_hex(0x888888)),
                ),
                None,
                0,
            );

            let inner_grid = tree.add_child(
                inner_column,
                Box::new(FlowGrid::new(50.0).with_spacing(2.0)),
                None,
                0,
            );

            for i in 1..=4 {
                add_small_button(
                    tree,
                    inner_grid,
                    &format!("{}", i),
                    500 + outer_idx * 10 + i,
                );
            }

            // Update item container
            replace_widget!(
                tree,
                item_container,
                Container::new(inner_column, 6.0)
                    .with_background(Color::from_hex(0x3a3a3a))
                    .with_radius(4.0)
            );
        }

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
        Box::new(Button::new(NodeId(0)).with_padding(6.0)),
        None,
        0,
    );

    let label = tree.add_child(
        button,
        Box::new(Label::new(text).with_size(11.0).with_color(Color::WHITE)),
        None,
        0,
    );

    replace_widget!(
        tree,
        button,
        Button::new(label).with_padding(6.0).on_click(move || {
            println!("CLICK: {} (id={})", text_owned, id);
        })
    );

    replace_widget!(
        tree,
        container,
        Container::new(button, 0.0).with_height(32.0)
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
        Box::new(Label::new(text).with_size(10.0).with_color(Color::WHITE)),
        None,
        0,
    );

    replace_widget!(
        tree,
        button,
        Button::new(label).with_padding(4.0).on_click(move || {
            println!("CLICK: {} (id={})", text_owned, id);
        })
    );

    replace_widget!(
        tree,
        container,
        Container::new(button, 0.0)
            .with_width(width)
            .with_height(height)
    );
}

// Helper: Add small button for nested grids
fn add_small_button(tree: &mut WidgetTree, parent: NodeId, text: &str, id: usize) {
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
        Box::new(Label::new(text).with_size(10.0).with_color(Color::WHITE)),
        None,
        0,
    );

    replace_widget!(
        tree,
        button,
        Button::new(label).with_padding(4.0).on_click(move || {
            println!("CLICK: {} (id={})", text_owned, id);
        })
    );

    replace_widget!(
        tree,
        container,
        Container::new(button, 0.0).with_height(28.0)
    );
}
