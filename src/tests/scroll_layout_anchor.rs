use crate::core::app::{
    layout_tree, post_layout_resolve_scroll_anchors, test_begin_frame_layout_tracking,
};
use crate::core::event::ScrollAlign;
use crate::core::layout::{Alignment, Constraints, FlexParams, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::ordered_selection::SelectionMode;
use crate::core::rect_authority::LayoutAuthorityGuard;
use crate::core::theme::Theme;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::utils::id::WidgetInternalId;
use crate::widgets::flex::Flex;
use crate::widgets::list::List;
use crate::widgets::scroll_area::{LayoutScrollAnchorIntent, ScrollArea};

struct AnchorTestWidget {
    id: WidgetInternalId,
    logical_height: f32,
    requested_scroll_y: Option<f32>,
}

impl AnchorTestWidget {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            logical_height: 0.0,
            requested_scroll_y: None,
        }
    }
}

impl WidgetDowncast for AnchorTestWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for AnchorTestWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let size = constraints.constrain(Size::zero());
        LayoutResult::new(
            size,
            Size::new(size.width, size.height.max(self.logical_height)),
        )
    }

    fn take_requested_scroll_y(&mut self) -> Option<f32> {
        self.requested_scroll_y.take()
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

fn build_tree(
    content_height: f32,
    scroll_y: f32,
    region_y: f32,
    region_height: f32,
) -> (WidgetTree, NodeId, NodeId, NodeId) {
    let mut tree = WidgetTree::new();
    let scroll_id = tree.create_root(Box::new(ScrollArea::new(NodeId(1))), None, 0);
    let region_id = tree.add_child(scroll_id, Box::new(AnchorTestWidget::new()), None, 0);
    let anchor_id = tree.add_child(region_id, Box::new(AnchorTestWidget::new()), None, 0);

    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(scroll_id, RectF::new(0.0, 0.0, 300.0, 200.0));
        tree.set_rect(region_id, RectF::new(0.0, region_y, 300.0, region_height));
        tree.set_rect(anchor_id, RectF::new(0.0, 0.0, 300.0, 24.0));
    }

    let scroll = tree
        .node_mut(scroll_id)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap();
    scroll.test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, content_height),
    );
    scroll.jump_to_y(scroll_y);

    (tree, scroll_id, region_id, anchor_id)
}

fn scroll(tree: &WidgetTree, scroll_id: NodeId) -> &ScrollArea {
    tree.node(scroll_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ScrollArea>()
        .unwrap()
}

fn scroll_mut(tree: &mut WidgetTree, scroll_id: NodeId) -> &mut ScrollArea {
    tree.node_mut(scroll_id)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
}

fn mount_scroll_content(content: Box<dyn Widget>) -> (WidgetTree, NodeId, NodeId) {
    let mut tree = WidgetTree::new();
    let scroll_id = tree.create_root(Box::new(ScrollArea::new_unwired()), None, 0);
    let content_id = tree.add_child(scroll_id, content, None, 0);
    scroll_mut(&mut tree, scroll_id).wire_content(content_id);
    (tree, scroll_id, content_id)
}

fn layout_scroll_fixture(tree: &mut WidgetTree, root: NodeId, size: Size) {
    test_begin_frame_layout_tracking();
    let _authority = LayoutAuthorityGuard::enter();
    layout_tree(
        tree,
        root,
        Constraints::tight(size),
        &Theme::default(),
        &mut FontManagerTextEngine::new(),
        None,
    );
}

#[test]
fn post_layout_realize_guard_zero_nonresolver_applies_direct_request() {
    let mut content = AnchorTestWidget::new();
    content.logical_height = 120.0;
    content.requested_scroll_y = Some(42.0);
    let (mut tree, scroll_id, content_id) = mount_scroll_content(Box::new(content));
    layout_scroll_fixture(&mut tree, scroll_id, Size::zero());
    assert_eq!(tree.get_rect(scroll_id), Some(RectF::zero()));
    assert_eq!(tree.get_rect(content_id), Some(RectF::zero()));
    assert_eq!(scroll(&tree, scroll_id).content_extent_height(), 120.0);
    assert_eq!(scroll(&tree, scroll_id).target_scroll_y(), 0.0);
    post_layout_resolve_scroll_anchors(&mut tree);
    assert_eq!(scroll(&tree, scroll_id).target_scroll_y(), 42.0);

    // Applying the direct request clears anchor mode. A later dirty anchor
    // distinguishes a repeated pass from reapplying that consumed request.
    scroll_mut(&mut tree, scroll_id).set_anchor_at_viewport_y(1, 0.0);
    scroll_mut(&mut tree, scroll_id).mark_anchor_dirty();
    post_layout_resolve_scroll_anchors(&mut tree);
    assert_eq!(tree.get_rect(content_id), Some(RectF::zero()));
    assert_eq!(scroll(&tree, scroll_id).target_scroll_y(), 42.0);
    assert!(scroll(&tree, scroll_id).is_anchor_dirty());
}

#[test]
fn post_layout_realize_guard_realized_nonresolver_applies_direct_request() {
    let mut content = AnchorTestWidget::new();
    content.logical_height = 120.0;
    content.requested_scroll_y = Some(36.0);
    let (mut tree, scroll_id, content_id) = mount_scroll_content(Box::new(content));
    layout_scroll_fixture(&mut tree, scroll_id, Size::new(240.0, 80.0));
    assert_eq!(
        tree.get_rect(content_id),
        Some(RectF::new(0.0, 0.0, 240.0, 80.0))
    );

    post_layout_resolve_scroll_anchors(&mut tree);
    assert_eq!(scroll(&tree, scroll_id).target_scroll_y(), 36.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "INV-REALIZE-ORDER-1 VIOLATION")]
fn post_layout_realize_guard_rejects_unrealized_list() {
    let mut list = List::new(SelectionMode::Single);
    list.set_item_count(1);
    let (mut tree, _, content_id) = mount_scroll_content(Box::new(list));
    assert_eq!(tree.get_rect(content_id), Some(RectF::zero()));
    post_layout_resolve_scroll_anchors(&mut tree);
}

#[test]
fn post_layout_realize_guard_resolves_realized_list() {
    let mut list = List::new(SelectionMode::Single);
    list.set_item_count(10);
    list.set_item_height(20.0);
    let (mut tree, scroll_id, content_id) = mount_scroll_content(Box::new(list));
    layout_scroll_fixture(&mut tree, scroll_id, Size::new(240.0, 80.0));
    assert_eq!(
        tree.get_rect(content_id),
        Some(RectF::new(0.0, 0.0, 240.0, 80.0))
    );
    scroll_mut(&mut tree, scroll_id).set_anchor_at_viewport_y(4, 10.0);
    scroll_mut(&mut tree, scroll_id).mark_anchor_dirty();

    for _ in 0..2 {
        post_layout_resolve_scroll_anchors(&mut tree);
        assert_eq!(scroll(&tree, scroll_id).logical_offset().y, 70.0);
        assert!(!scroll(&tree, scroll_id).is_anchor_dirty());
    }
}

#[test]
#[should_panic(expected = "post-layout anchors: invalid ScrollArea content pair")]
fn post_layout_realize_guard_rejects_missing_wired_content() {
    let (mut tree, scroll_id, content_id) = mount_scroll_content(Box::new(AnchorTestWidget::new()));
    layout_scroll_fixture(&mut tree, scroll_id, Size::new(240.0, 80.0));
    tree.remove_node(content_id);
    post_layout_resolve_scroll_anchors(&mut tree);
}

#[test]
#[should_panic(expected = "post-layout anchors: wired content is not a child of its ScrollArea")]
fn post_layout_realize_guard_rejects_reused_content_slot_under_another_parent() {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(AnchorTestWidget::new()), None, 0);
    let scroll_id = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
    let content_id = tree.add_child(scroll_id, Box::new(AnchorTestWidget::new()), None, 0);
    scroll_mut(&mut tree, scroll_id).wire_content(content_id);
    tree.remove_node(content_id);
    let reused = tree.add_child(root, Box::new(AnchorTestWidget::new()), None, 0);
    assert_eq!(reused, content_id);
    layout_scroll_fixture(&mut tree, reused, Size::new(240.0, 80.0));
    post_layout_resolve_scroll_anchors(&mut tree);
}

fn mount_scroll_removal_fixture() -> (WidgetTree, NodeId, NodeId, NodeId) {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(Flex::column().with_align_items(Alignment::Stretch)),
        None,
        0,
    );
    let branch = tree.add_child(
        root,
        Box::new(Flex::column().with_align_items(Alignment::Stretch)),
        None,
        0,
    );
    tree.set_flex(branch, FlexParams::fixed(80.0));
    let scroll_id = tree.add_child(branch, Box::new(ScrollArea::new_unwired()), None, 0);
    tree.set_flex(scroll_id, FlexParams::fixed(80.0));
    let content_id = tree.add_child(scroll_id, Box::new(AnchorTestWidget::new()), None, 0);
    scroll_mut(&mut tree, scroll_id).wire_content(content_id);
    (tree, branch, scroll_id, content_id)
}

fn layout_and_seal_scroll_fixture(tree: &mut WidgetTree, scroll_id: NodeId) {
    let root = tree.root().unwrap();
    layout_scroll_fixture(tree, root, Size::new(240.0, 80.0));
    assert!(
        scroll(tree, scroll_id)
            .deferred_wiring()
            .unwrap()
            .is_sealed()
    );
}

#[test]
fn sealed_scroll_content_removal_refuses_before_mutation() {
    let (mut tree, _, scroll_id, content_id) = mount_scroll_removal_fixture();
    layout_and_seal_scroll_fixture(&mut tree, scroll_id);
    let owner = tree.node(content_id).unwrap().id();
    let mount = tree.concrete_mount(owner).unwrap();
    let scroll_owner = tree.node(scroll_id).unwrap().id();
    let scroll_mount = tree.concrete_mount(scroll_owner).unwrap();
    let generation = tree.structure_generation();
    let rect = Some(RectF::new(0.0, 0.0, 240.0, 80.0));
    assert_eq!(tree.get_rect(content_id), rect);

    for _ in 0..2 {
        let refusal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tree.remove_node(content_id);
        }))
        .expect_err("sealed wired content removal must fail");
        let diagnostic = format!(
            "HAL-154: cannot remove wired content {content_id:?} of sealed ScrollArea {scroll_id:?}"
        );
        assert_eq!(refusal.downcast_ref::<String>(), Some(&diagnostic));
        assert_eq!(tree.structure_generation(), generation);
        assert_eq!(tree.find_by_id(owner), Some(content_id));
        assert_eq!(tree.node(content_id).unwrap().id(), owner);
        assert_eq!(tree.node(content_id).unwrap().parent(), Some(scroll_id));
        assert_eq!(tree.concrete_mount(owner).as_ref(), Some(&mount));
        assert!(mount.is_live());
        assert_eq!(
            tree.concrete_mount(scroll_owner).as_ref(),
            Some(&scroll_mount)
        );
        assert!(scroll_mount.is_live());
        assert_eq!(tree.children(scroll_id), vec![content_id]);
        assert_eq!(tree.get_rect(content_id), rect);
        assert_eq!(scroll(&tree, scroll_id).content(), content_id);
        assert_eq!(tree.scroll_anchor_nodes(), &[scroll_id]);
        assert!(tree.drain_pending_removals().is_empty());
        assert!(tree.drain_pending_node_removals().is_empty());
    }

    let sibling = tree.add_child(scroll_id, Box::new(AnchorTestWidget::new()), None, 0);
    assert_ne!(sibling, content_id);
    tree.remove_node(sibling);
    assert!(tree.node(sibling).is_none());
    assert_eq!(tree.children(scroll_id), vec![content_id]);
    assert_eq!(tree.concrete_mount(owner).as_ref(), Some(&mount));
}

#[test]
fn sealed_scroll_content_removal_allows_subtree_teardown() {
    for remove_ancestor in [false, true] {
        let (mut tree, branch, scroll_id, content_id) = mount_scroll_removal_fixture();
        layout_and_seal_scroll_fixture(&mut tree, scroll_id);
        let scroll_owner = tree.node(scroll_id).unwrap().id();
        let content_owner = tree.node(content_id).unwrap().id();
        let scroll_mount = tree.concrete_mount(scroll_owner).unwrap();
        let content_mount = tree.concrete_mount(content_owner).unwrap();

        tree.remove_node(if remove_ancestor { branch } else { scroll_id });

        assert_eq!(tree.node(branch).is_none(), remove_ancestor);
        assert!(tree.node(scroll_id).is_none());
        assert!(tree.node(content_id).is_none());
        assert!(tree.find_by_id(scroll_owner).is_none());
        assert!(tree.find_by_id(content_owner).is_none());
        assert!(!scroll_mount.is_live());
        assert!(!content_mount.is_live());
        assert!(tree.scroll_anchor_nodes().is_empty());
    }
}

#[test]
fn sealed_scroll_content_removal_allows_unsealed_and_empty_hosts() {
    let (mut tree, branch, scroll_id, content_id) = mount_scroll_removal_fixture();
    assert!(
        !scroll(&tree, scroll_id)
            .deferred_wiring()
            .unwrap()
            .is_sealed()
    );
    assert_eq!(scroll(&tree, scroll_id).content(), content_id);
    tree.remove_node(content_id);
    assert!(tree.node(content_id).is_none());
    assert!(tree.children(scroll_id).is_empty());
    tree.remove_node(scroll_id);

    let empty = tree.add_child(branch, Box::new(ScrollArea::empty()), None, 0);
    tree.set_flex(empty, FlexParams::fixed(80.0));
    let child = tree.add_child(empty, Box::new(AnchorTestWidget::new()), None, 0);
    layout_and_seal_scroll_fixture(&mut tree, empty);
    assert_eq!(scroll(&tree, empty).sealed_wired_content(), None);
    tree.remove_node(child);
    assert!(tree.node(child).is_none());
    assert!(tree.children(empty).is_empty());
    tree.remove_node(empty);
    assert!(tree.node(empty).is_none());
}

#[test]
fn sealed_scroll_content_replacement_keeps_logical_child_and_resolves_anchor() {
    let (mut tree, _, scroll_id, content_id) = mount_scroll_removal_fixture();
    layout_and_seal_scroll_fixture(&mut tree, scroll_id);
    let owner = tree.node(content_id).unwrap().id();
    let previous_mount = tree.concrete_mount(owner).unwrap();

    let mut list = List::new(SelectionMode::Single);
    list.set_item_count(10);
    list.set_item_height(20.0);
    tree.replace_widget_only(content_id, Box::new(list));

    assert_eq!(tree.find_by_id(owner), Some(content_id));
    assert_eq!(tree.node(content_id).unwrap().id(), owner);
    assert_eq!(tree.node(content_id).unwrap().parent(), Some(scroll_id));
    assert_eq!(scroll(&tree, scroll_id).content(), content_id);
    let current_mount = tree.concrete_mount(owner).unwrap();
    assert_ne!(previous_mount, current_mount);
    assert!(!previous_mount.is_live());
    assert!(current_mount.is_live());

    layout_and_seal_scroll_fixture(&mut tree, scroll_id);
    assert_eq!(
        tree.get_rect(content_id),
        Some(RectF::new(0.0, 0.0, 240.0, 80.0))
    );
    scroll_mut(&mut tree, scroll_id).set_anchor_at_viewport_y(4, 10.0);
    scroll_mut(&mut tree, scroll_id).mark_anchor_dirty();
    post_layout_resolve_scroll_anchors(&mut tree);
    assert_eq!(scroll(&tree, scroll_id).logical_offset().y, 70.0);
    assert!(!scroll(&tree, scroll_id).is_anchor_dirty());
}

#[test]
fn collapse_preserves_header_with_transient_tail_slack() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);

    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));

    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 450.0),
    );

    post_layout_resolve_scroll_anchors(&mut tree);

    let scroll = scroll(&tree, scroll_id);
    assert!((scroll.target_scroll_y() - 250.0).abs() < 0.01);
    assert!((scroll.test_tail_slack_y() - 150.0).abs() < 0.01);
    assert!((scroll.visual_scroll_y() - 400.0).abs() < 0.01);
    assert!((scroll.content_extent_height() - 450.0).abs() < 0.01);
}

#[test]
fn expand_minimally_reveals_region_without_slack() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(450.0, 250.0, 300.0, 24.0);

    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::ExpandRevealRegion,
    ));

    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 300.0, 300.0, 200.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 550.0),
    );

    post_layout_resolve_scroll_anchors(&mut tree);

    let scroll = scroll(&tree, scroll_id);
    assert!((scroll.target_scroll_y() - 300.0).abs() < 0.01);
    assert!(scroll.test_tail_slack_y().abs() < 0.01);
    assert!((scroll.visual_scroll_y() - 300.0).abs() < 0.01);
}

#[test]
fn content_growth_absorbs_slack_without_moving_effective_offset() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));

    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 450.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 550.0),
    );
    assert!((scroll(&tree, scroll_id).target_scroll_y() - 350.0).abs() < 0.01);
    assert!((scroll(&tree, scroll_id).test_tail_slack_y() - 50.0).abs() < 0.01);
    assert!((scroll(&tree, scroll_id).visual_scroll_y() - 400.0).abs() < 0.01);

    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 600.0),
    );
    assert!((scroll(&tree, scroll_id).target_scroll_y() - 400.0).abs() < 0.01);
    assert!(scroll(&tree, scroll_id).test_tail_slack_y().abs() < 0.01);
    assert!((scroll(&tree, scroll_id).visual_scroll_y() - 400.0).abs() < 0.01);
}

#[test]
fn committed_programmatic_scroll_retires_slack() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));

    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 450.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    scroll_mut(&mut tree, scroll_id).scroll_to_y(200.0);
    assert!(scroll(&tree, scroll_id).test_tail_slack_y().abs() < 0.01);
    assert!((scroll(&tree, scroll_id).visual_scroll_y() - 250.0).abs() < 0.01);
    assert!((scroll(&tree, scroll_id).target_scroll_y() - 200.0).abs() < 0.01);
}

#[test]
fn collapse_with_true_range_available_needs_no_slack() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(1_000.0, 400.0, 400.0, 200.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));

    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 850.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    assert!((scroll(&tree, scroll_id).target_scroll_y() - 400.0).abs() < 0.01);
    assert!(scroll(&tree, scroll_id).test_tail_slack_y().abs() < 0.01);
}

#[test]
fn oversized_expand_keeps_header_at_scrollport_top_without_slack() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 24.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::ExpandRevealRegion,
    ));

    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 500.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 900.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    assert!((scroll(&tree, scroll_id).target_scroll_y() - 400.0).abs() < 0.01);
    assert!(scroll(&tree, scroll_id).test_tail_slack_y().abs() < 0.01);
}

#[test]
fn slack_does_not_change_true_extent_or_max_scroll() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));
    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 450.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    let scroll = scroll(&tree, scroll_id);
    assert!((scroll.content_extent_height() - 450.0).abs() < 0.01);
    assert!((scroll.test_max_scroll_y() - 250.0).abs() < 0.01);
    assert!((scroll.target_scroll_y() - 250.0).abs() < 0.01);
    assert!((scroll.visual_scroll_y() - 400.0).abs() < 0.01);
    assert!((Widget::scroll_offset(scroll).unwrap().1 - 400.0).abs() < 0.01);
}

#[test]
fn no_op_focus_reveal_preserves_slack() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));
    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 450.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    scroll_mut(&mut tree, scroll_id)
        .resolve_reveal_rect(RectF::new(0.0, 410.0, 100.0, 20.0), ScrollAlign::Nearest);
    assert!((scroll(&tree, scroll_id).test_tail_slack_y() - 150.0).abs() < 0.01);
    assert!((scroll(&tree, scroll_id).visual_scroll_y() - 400.0).abs() < 0.01);
}

#[test]
fn viewport_resize_retires_slack() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));
    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 450.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);
    assert!(scroll(&tree, scroll_id).test_tail_slack_y() > 0.0);

    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(320.0, 200.0),
        Size::new(320.0, 200.0),
        Size::new(320.0, 450.0),
    );
    assert!(scroll(&tree, scroll_id).test_tail_slack_y().abs() < 0.01);
    assert!((scroll(&tree, scroll_id).visual_scroll_y() - 250.0).abs() < 0.01);
}

#[test]
fn top_prepend_and_trim_compensate_preserved_effective_offset_once() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));
    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 450.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    scroll_mut(&mut tree, scroll_id).prepend_content_height(50.0);
    assert!((scroll(&tree, scroll_id).target_scroll_y() - 300.0).abs() < 0.01);
    assert!((scroll(&tree, scroll_id).test_tail_slack_y() - 150.0).abs() < 0.01);
    assert!((scroll(&tree, scroll_id).visual_scroll_y() - 450.0).abs() < 0.01);

    scroll_mut(&mut tree, scroll_id).trim_content_top(30.0);
    assert!((scroll(&tree, scroll_id).target_scroll_y() - 270.0).abs() < 0.01);
    assert!((scroll(&tree, scroll_id).test_tail_slack_y() - 150.0).abs() < 0.01);
    assert!((scroll(&tree, scroll_id).visual_scroll_y() - 420.0).abs() < 0.01);
}

#[test]
fn active_scroll_motion_rejects_transaction_without_mutation() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);
    scroll_mut(&mut tree, scroll_id).scroll_to_y(350.0);

    assert!(!tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));
    assert!(!scroll(&tree, scroll_id).test_has_pending_layout_anchor());
    assert!((scroll(&tree, scroll_id).target_scroll_y() - 350.0).abs() < 0.01);
}

#[test]
fn missing_anchor_cancels_pending_transaction_cleanly() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));
    tree.remove_node(anchor_id);

    post_layout_resolve_scroll_anchors(&mut tree);
    assert!(!scroll(&tree, scroll_id).test_has_pending_layout_anchor());
    assert!(scroll(&tree, scroll_id).test_tail_slack_y().abs() < 0.01);
}

#[test]
fn layout_transaction_retires_ordinary_anchor_authority() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);
    scroll_mut(&mut tree, scroll_id).set_anchor_at_viewport_y(42, 10.0);
    assert!(scroll(&tree, scroll_id).test_has_ordinary_anchor());

    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));
    assert!(!scroll(&tree, scroll_id).test_has_ordinary_anchor());
    assert!(scroll(&tree, scroll_id).test_has_pending_layout_anchor());
}

#[test]
fn horizontal_overlay_occlusion_reduces_expand_reveal_height() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(450.0, 250.0, 300.0, 24.0);
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 188.0),
        Size::new(300.0, 450.0),
    );
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::ExpandRevealRegion,
    ));

    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 300.0, 300.0, 180.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 188.0),
        Size::new(300.0, 550.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    assert!((scroll(&tree, scroll_id).target_scroll_y() - 292.0).abs() < 0.01);
    assert!(scroll(&tree, scroll_id).test_tail_slack_y().abs() < 0.01);
}

#[test]
fn resize_between_request_and_resolution_cancels_transaction() {
    let (mut tree, scroll_id, region_id, anchor_id) = build_tree(600.0, 400.0, 400.0, 200.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));

    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(320.0, 200.0),
        Size::new(320.0, 200.0),
        Size::new(320.0, 450.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    assert!(!scroll(&tree, scroll_id).test_has_pending_layout_anchor());
    assert!(scroll(&tree, scroll_id).test_tail_slack_y().abs() < 0.01);
}

#[test]
fn nested_scroll_areas_mutate_only_nearest_owner() {
    let mut tree = WidgetTree::new();
    let outer_id = tree.create_root(Box::new(ScrollArea::new(NodeId(1))), None, 0);
    let outer_content = tree.add_child(outer_id, Box::new(AnchorTestWidget::new()), None, 0);
    let inner_id = tree.add_child(outer_content, Box::new(ScrollArea::new(NodeId(3))), None, 0);
    let region_id = tree.add_child(inner_id, Box::new(AnchorTestWidget::new()), None, 0);
    let anchor_id = tree.add_child(region_id, Box::new(AnchorTestWidget::new()), None, 0);

    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(outer_id, RectF::new(0.0, 0.0, 300.0, 200.0));
        tree.set_rect(outer_content, RectF::new(0.0, 0.0, 300.0, 1_000.0));
        tree.set_rect(inner_id, RectF::new(0.0, 100.0, 300.0, 200.0));
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 200.0));
        tree.set_rect(anchor_id, RectF::new(0.0, 0.0, 300.0, 24.0));
    }

    scroll_mut(&mut tree, outer_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 1_000.0),
    );
    scroll_mut(&mut tree, outer_id).jump_to_y(100.0);
    scroll_mut(&mut tree, inner_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 600.0),
    );
    scroll_mut(&mut tree, inner_id).jump_to_y(400.0);

    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));
    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, inner_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 450.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    assert!((scroll(&tree, outer_id).target_scroll_y() - 100.0).abs() < 0.01);
    assert!(scroll(&tree, outer_id).test_tail_slack_y().abs() < 0.01);
    assert!((scroll(&tree, inner_id).target_scroll_y() - 250.0).abs() < 0.01);
    assert!((scroll(&tree, inner_id).test_tail_slack_y() - 150.0).abs() < 0.01);
}

#[test]
fn content_owned_vertical_translation_consumes_effective_offset_once() {
    let mut tree = WidgetTree::new();
    let scroll_id = tree.create_root(
        Box::new(ScrollArea::new(NodeId(1)).with_content_owned_vertical_translation()),
        None,
        0,
    );
    let region_id = tree.add_child(scroll_id, Box::new(AnchorTestWidget::new()), None, 0);
    let anchor_id = tree.add_child(region_id, Box::new(AnchorTestWidget::new()), None, 0);
    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(scroll_id, RectF::new(0.0, 0.0, 300.0, 200.0));
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 200.0));
        tree.set_rect(anchor_id, RectF::new(0.0, 0.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 600.0),
    );
    scroll_mut(&mut tree, scroll_id).jump_to_y(400.0);
    assert!(tree.begin_layout_scroll_anchor(
        anchor_id,
        region_id,
        LayoutScrollAnchorIntent::CollapsePreserveHeader,
    ));
    {
        let _authority = LayoutAuthorityGuard::enter();
        tree.set_rect(region_id, RectF::new(0.0, 400.0, 300.0, 24.0));
    }
    scroll_mut(&mut tree, scroll_id).test_set_final_layout_anchor_metrics(
        Size::new(300.0, 200.0),
        Size::new(300.0, 200.0),
        Size::new(300.0, 450.0),
    );
    post_layout_resolve_scroll_anchors(&mut tree);

    let scroll = scroll(&tree, scroll_id);
    assert!((scroll.visual_scroll_y() - 400.0).abs() < 0.01);
    assert!(Widget::scroll_offset(scroll).unwrap().1.abs() < 0.01);
}
