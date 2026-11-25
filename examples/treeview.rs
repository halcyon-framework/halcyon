// =============================================================================
// TreeView Stress Demo (SPEC-HAL-259)
// =============================================================================
//
// 5,000-node tree, depth up to 6, random expansion pattern.
//
// Proves:
// 1. No O(n²) paint paths (no stutter with 5000 nodes)
// 2. No resolve panic on repeated frames
// 3. Expand/collapse works correctly
// 4. Scroll works at scale
// 5. Per-column rendering with real text labels in tree column
//
// =============================================================================

use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::FlexParams;
use halcyon::core::widget::Widget;
use halcyon::engines::hierarchical::NodeMeta;
use halcyon::prelude::*;
use halcyon::widgets::flex::Flex;
use halcyon::widgets::label::Label;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::tabular_view::column::{Column, ColumnModel};
use halcyon::widgets::tree_view::TreeView;

const WINDOW_WIDTH: f32 = 800.0;
const WINDOW_HEIGHT: f32 = 600.0;

/// Total number of nodes to generate.
const NODE_COUNT: usize = 1000_000;

/// Maximum tree depth.
const MAX_DEPTH: u32 = 6;

/// Generate a stress-test tree with NODE_COUNT nodes, depth up to MAX_DEPTH.
///
/// Strategy: sequential allocation. Each node is a child of a "recent ancestor"
/// chosen by a deterministic pattern that produces a realistic tree shape.
fn build_stress_tree() -> Vec<NodeMeta> {
    let mut nodes = Vec::with_capacity(NODE_COUNT);

    // Node 0: root
    nodes.push(NodeMeta {
        node_id: 0,
        parent_index: None,
        depth: 0,
        has_children: true,
        label: "Root".to_string(),
    });

    // Track the most recent node at each depth for parent assignment.
    let mut latest_at_depth: [usize; (MAX_DEPTH + 1) as usize] = [0; (MAX_DEPTH + 1) as usize];
    latest_at_depth[0] = 0;

    for i in 1..NODE_COUNT {
        // Deterministic depth selection: use modular arithmetic for variety.
        // Pattern: mostly stay at depth 1-3, occasionally go deeper.
        let target_depth = match i % 17 {
            0 => 1,                      // reset to shallow
            1..=4 => 1,                  // depth 1
            5..=8 => 2,                  // depth 2
            9..=11 => 3,                 // depth 3
            12..=13 => 4.min(MAX_DEPTH), // depth 4
            14 => 5.min(MAX_DEPTH),      // depth 5
            15 => MAX_DEPTH,             // max depth
            _ => 2,                      // fallback
        };

        // Parent is the latest node at (target_depth - 1).
        // If target_depth is 0, parent is None (but we only have 1 root).
        let depth = target_depth.min(MAX_DEPTH);
        let parent_depth = if depth > 0 { depth - 1 } else { 0 };
        let parent_idx = latest_at_depth[parent_depth as usize];

        // Mark parent as having children.
        nodes[parent_idx].has_children = true;

        // Determine if this node will have children (nodes near the end don't).
        let has_children = depth < MAX_DEPTH && i < NODE_COUNT - 100;

        nodes.push(NodeMeta {
            node_id: i as u64,
            parent_index: Some(parent_idx),
            depth,
            has_children: false, // Will be set to true by future children
            label: format!("Node {}", i),
        });

        // Update latest_at_depth for this depth.
        latest_at_depth[depth as usize] = i;

        // Set has_children based on prediction (we fix parent above already).
        if has_children {
            nodes[i].has_children = false; // Children will mark us later
        }
    }

    // Second pass: mark has_children correctly.
    let mut has_child_flags = vec![false; NODE_COUNT];
    for node in &nodes {
        if let Some(parent) = node.parent_index {
            has_child_flags[parent] = true;
        }
    }
    for (i, node) in nodes.iter_mut().enumerate() {
        node.has_children = has_child_flags[i];
    }

    nodes
}

struct TreeViewDemoScreen {
    placeholder: Label,
    built: bool,
}

impl TreeViewDemoScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            built: false,
        }
    }
}

impl Screen for TreeViewDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;

        // Create TreeView.
        let mut tree_view = TreeView::new();

        // Set tree data.
        let nodes = build_stress_tree();
        println!(
            "Generated {} tree nodes (max depth {})",
            nodes.len(),
            MAX_DEPTH
        );

        // Count nodes with children.
        let expandable = nodes.iter().filter(|n| n.has_children).count();
        println!("Expandable nodes: {}", expandable);

        tree_view.set_tree_data(nodes);

        // Set columns: tree column + 3 wide data columns (forces h-overflow for scroll testing).
        let mut col_model = ColumnModel::new();
        let tree_col = Column::new(400.0);
        let data_col_a = Column::new(300.0);
        let data_col_b = Column::new(300.0);
        let data_col_c = Column::new(300.0);
        let tree_col_id = tree_col.id;
        let data_col_a_id = data_col_a.id;
        let data_col_b_id = data_col_b.id;
        let data_col_c_id = data_col_c.id;
        col_model.push(tree_col);
        col_model.push(data_col_a);
        col_model.push(data_col_b);
        col_model.push(data_col_c);
        tree_view.set_columns(col_model);
        tree_view.set_tree_column(tree_col_id);
        tree_view.set_column_title(tree_col_id, "Name");
        tree_view.set_column_title(data_col_a_id, "Details");
        tree_view.set_column_title(data_col_b_id, "Size");
        tree_view.set_column_title(data_col_c_id, "Modified");

        tree_view.set_footer_height(24.0);

        // Wire horizontal scroll link for header alignment with body scroll.
        let hscroll_link = tree_view.horizontal_scroll_link();
        let hscroll_freeze = tree_view.horizontal_scroll_freeze();

        // Create body and footer widgets BEFORE moving tree_view (shares Rc state).
        let body_widget = tree_view.create_body_widget();
        let footer_widget = tree_view.create_footer_widget();

        // Build widget tree:
        // Root (Flex column)
        //   └─ TreeView (flexible, fills remaining space)
        //        └─ ScrollArea (scroll state, scrollbars, clipping, smooth scroll)
        //             └─ TreeViewBody (row content, expand/collapse)
        let tree_root = cx.tree.root().expect("Tree must have root");
        let root = cx
            .tree
            .add_child(tree_root, Box::new(Flex::column()), None, 0);

        // Add TreeView as child of Flex.
        let tree_view_id = cx.tree.add_child(root, Box::new(tree_view), None, 0);
        cx.tree.set_flex(tree_view_id, FlexParams::flexible(1.0));

        // Add ScrollArea (unwired) as child of TreeView.
        // Enable horizontal scrollbar and wire scroll link for header alignment.
        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze)
            .with_content_owned_vertical_translation();
        let scroll_area_id = cx
            .tree
            .add_child(tree_view_id, Box::new(scroll_area), None, 0);

        // Add body widget as child of ScrollArea.
        let body_id = cx
            .tree
            .add_child(scroll_area_id, Box::new(body_widget), None, 0);

        // Wire ScrollArea to its content.
        if let Some(node) = cx.tree.node_mut(scroll_area_id) {
            if let Some(scroll_area_widget) =
                node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>()
            {
                scroll_area_widget.wire_content(body_id);
            }
        }

        // Add footer as second child of TreeView.
        cx.tree
            .add_child(tree_view_id, Box::new(footer_widget), None, 0);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        println!("=== TreeView Stress Demo (SPEC-HAL-259) ===");
        println!();
        println!("Tree: {} nodes, max depth {}", NODE_COUNT, MAX_DEPTH);
        println!();
        println!("Verification:");
        println!("  [ ] No stutter during scroll");
        println!("  [ ] No resolve panic on repeated frames");
        println!("  [ ] Expand/collapse works");
        println!("  [ ] Guide lines render correctly");
        println!("  [ ] Disclosure triangles visible (not rects)");
        println!("  [ ] Real text labels in tree column");
        println!("  [ ] Non-tree columns show placeholder rects");
        println!();
        println!("Close window to exit.");
        println!("==========================================");
    }

    fn mark_rebuild(&mut self) {}

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(
        AppConfig::new("TreeView Stress Demo (SPEC-HAL-259)")
            .with_size(WINDOW_WIDTH, WINDOW_HEIGHT),
    );
    app.run(TreeViewDemoScreen::new());
}
