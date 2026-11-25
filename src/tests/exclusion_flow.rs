use crate::core::layout::{Constraints, EdgeInsets};
use crate::core::math::RectF;
use crate::core::rect_authority::LayoutAuthorityGuard;
use crate::core::theme::Theme;
use crate::core::widget::LayoutContext;
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::widgets::exclusion_flow::{ExclusionFlow, SpaceMap};
use crate::widgets::fixed_spacer::FixedSpacer;

fn rect(x: f32, y: f32, width: f32, height: f32) -> RectF {
    RectF::new(x, y, width, height)
}

fn approx_eq(a: f32, b: f32) {
    assert!(
        (a - b).abs() < 0.1,
        "expected {a} to approximately equal {b}"
    );
}

fn intersects_any(rect: RectF, exclusions: &[RectF]) -> bool {
    exclusions
        .iter()
        .any(|exclusion| rect.intersects(exclusion))
}

fn build_tree(
    flow: ExclusionFlow,
    child_sizes: &[(f32, f32)],
    constraints: Constraints,
) -> (WidgetTree, NodeId, Vec<NodeId>) {
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    let root = tree.create_root(Box::new(flow), None, 0);
    let mut children = Vec::new();

    for &(width, height) in child_sizes {
        children.push(tree.add_child(root, Box::new(FixedSpacer::new(width, height)), None, 0));
    }

    {
        let _guard = LayoutAuthorityGuard::enter();
        let (mut widget, root_children, _rect) = tree.take_widget(root).unwrap();
        let result = {
            let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
            cx.set_current_node(root);
            widget.layout(&mut cx, constraints, &root_children)
        };
        tree.restore_widget(
            root,
            widget,
            RectF::new(0.0, 0.0, result.size.width, result.size.height),
        );
        tree.set_cached_size(root, result.size);
        tree.set_content_size(root, result.content_size);
    }

    (tree, root, children)
}

#[test]
fn spacemap_no_exclusions_returns_one_full_width_band() {
    let map = SpaceMap::new(rect(0.0, 0.0, 100.0, 50.0), &[]);

    assert_eq!(map.bands().len(), 1);
    approx_eq(map.bands()[0].y, 0.0);
    approx_eq(map.bands()[0].height, 50.0);
    assert_eq!(map.bands()[0].segments.len(), 1);
    approx_eq(map.bands()[0].segments[0].x, 0.0);
    approx_eq(map.bands()[0].segments[0].width, 100.0);
}

#[test]
fn spacemap_centered_exclusion_splits_only_affected_band() {
    let map = SpaceMap::new(
        rect(0.0, 0.0, 100.0, 100.0),
        &[rect(40.0, 25.0, 20.0, 50.0)],
    );

    assert_eq!(map.bands().len(), 3);
    assert_eq!(map.bands()[0].segments.len(), 1);
    assert_eq!(map.bands()[1].segments.len(), 2);
    assert_eq!(map.bands()[2].segments.len(), 1);
    approx_eq(map.bands()[1].segments[0].x, 0.0);
    approx_eq(map.bands()[1].segments[0].width, 40.0);
    approx_eq(map.bands()[1].segments[1].x, 60.0);
    approx_eq(map.bands()[1].segments[1].width, 40.0);
}

#[test]
fn spacemap_left_and_right_exclusions_narrow_affected_bands() {
    let map = SpaceMap::new(
        rect(0.0, 0.0, 120.0, 80.0),
        &[rect(0.0, 10.0, 20.0, 30.0), rect(100.0, 40.0, 20.0, 30.0)],
    );

    let left_band = map.bands().iter().find(|band| band.y >= 10.0).unwrap();
    approx_eq(left_band.segments[0].x, 20.0);
    approx_eq(left_band.segments[0].width, 100.0);

    let right_band = map.bands().iter().find(|band| band.y >= 40.0).unwrap();
    approx_eq(right_band.segments[0].x, 0.0);
    approx_eq(right_band.segments[0].width, 100.0);
}

#[test]
fn spacemap_overlapping_exclusions_do_not_produce_negative_segments() {
    let map = SpaceMap::new(
        rect(0.0, 0.0, 100.0, 100.0),
        &[rect(20.0, 20.0, 40.0, 60.0), rect(40.0, 20.0, 40.0, 60.0)],
    );

    for band in map.bands() {
        for segment in &band.segments {
            assert!(segment.width > 0.0);
        }
    }
}

#[test]
fn spacemap_full_width_exclusion_removes_all_segments_for_that_band() {
    let map = SpaceMap::new(
        rect(0.0, 0.0, 100.0, 100.0),
        &[rect(0.0, 25.0, 100.0, 50.0)],
    );

    let blocked = map.bands().iter().find(|band| band.y >= 25.0).unwrap();
    assert!(blocked.segments.is_empty());
}

#[test]
fn spacemap_ignores_outside_and_zero_area_exclusions() {
    let map = SpaceMap::new(
        rect(0.0, 0.0, 100.0, 50.0),
        &[rect(200.0, 0.0, 50.0, 50.0), rect(10.0, 10.0, 0.0, 20.0)],
    );

    assert_eq!(map.exclusions().len(), 0);
    assert_eq!(map.bands().len(), 1);
    approx_eq(map.bands()[0].segments[0].width, 100.0);
}

#[test]
fn spacemap_bands_and_segments_are_sorted() {
    let map = SpaceMap::new(
        rect(0.0, 0.0, 100.0, 100.0),
        &[rect(70.0, 40.0, 20.0, 20.0), rect(20.0, 40.0, 20.0, 20.0)],
    );

    for pair in map.bands().windows(2) {
        assert!(pair[0].y <= pair[1].y);
    }
    let split_band = map
        .bands()
        .iter()
        .find(|band| band.segments.len() > 1)
        .unwrap();
    for pair in split_band.segments.windows(2) {
        assert!(pair[0].x <= pair[1].x);
    }
}

#[test]
fn exclusion_flow_no_exclusions_behaves_like_simple_ordered_wrap() {
    let flow = ExclusionFlow::new().with_spacing(10.0);
    let (tree, _root, children) = build_tree(
        flow,
        &[(80.0, 20.0), (80.0, 20.0), (80.0, 20.0)],
        Constraints::new(0.0, 180.0, 0.0, 200.0),
    );

    let first = tree.get_rect(children[0]).unwrap();
    let second = tree.get_rect(children[1]).unwrap();
    let third = tree.get_rect(children[2]).unwrap();

    approx_eq(first.x, 0.0);
    approx_eq(first.y, 0.0);
    approx_eq(second.x, 90.0);
    approx_eq(second.y, 0.0);
    approx_eq(third.x, 0.0);
    approx_eq(third.y, 30.0);
}

#[test]
fn exclusion_flow_preserves_order_and_avoids_exclusions() {
    let exclusion = rect(90.0, 0.0, 80.0, 70.0);
    let flow = ExclusionFlow::new()
        .with_spacing(10.0)
        .with_exclusion(exclusion);
    let (tree, _root, children) = build_tree(
        flow,
        &[(70.0, 20.0), (70.0, 20.0), (70.0, 20.0), (70.0, 20.0)],
        Constraints::new(0.0, 240.0, 0.0, 240.0),
    );

    let rects: Vec<_> = children
        .iter()
        .map(|&child| tree.get_rect(child).unwrap())
        .collect();

    for rect in &rects {
        assert!(!intersects_any(*rect, &[exclusion]));
    }
    for pair in rects.windows(2) {
        assert!(
            pair[0].y < pair[1].bottom() + 0.1,
            "source order should remain visually monotonic"
        );
    }
}

#[test]
fn exclusion_flow_child_rects_do_not_overlap_each_other() {
    let flow = ExclusionFlow::new()
        .with_spacing(5.0)
        .with_exclusion(rect(60.0, 0.0, 60.0, 40.0));
    let (tree, _root, children) = build_tree(
        flow,
        &[(50.0, 20.0), (50.0, 20.0), (50.0, 20.0), (50.0, 20.0)],
        Constraints::new(0.0, 180.0, 0.0, 200.0),
    );

    let rects: Vec<_> = children
        .iter()
        .map(|&child| tree.get_rect(child).unwrap())
        .collect();

    for (index, rect) in rects.iter().enumerate() {
        for other in rects.iter().skip(index + 1) {
            assert!(!rect.intersects(other), "{rect:?} overlapped {other:?}");
        }
    }
}

#[test]
fn exclusion_flow_oversized_child_clamps_to_widest_segment() {
    let flow = ExclusionFlow::new();
    let (tree, _root, children) = build_tree(
        flow,
        &[(200.0, 20.0)],
        Constraints::new(0.0, 120.0, 0.0, 120.0),
    );

    let child = tree.get_rect(children[0]).unwrap();
    approx_eq(child.width, 120.0);
}

#[test]
fn exclusion_flow_tall_child_is_pushed_below_collision() {
    let exclusion = rect(70.0, 20.0, 70.0, 80.0);
    let flow = ExclusionFlow::new().with_exclusion(exclusion);
    let (tree, _root, children) = build_tree(
        flow,
        &[(60.0, 90.0), (80.0, 20.0)],
        Constraints::new(0.0, 180.0, 0.0, 220.0),
    );

    let tall = tree.get_rect(children[0]).unwrap();
    let second = tree.get_rect(children[1]).unwrap();

    assert!(!tall.intersects(&exclusion));
    assert!(!second.intersects(&exclusion));
}

#[test]
fn exclusion_flow_finite_viewport_reports_larger_content_size() {
    let flow = ExclusionFlow::new().with_spacing(10.0);
    let (tree, root, _children) = build_tree(
        flow,
        &[(80.0, 30.0), (80.0, 30.0), (80.0, 30.0), (80.0, 30.0)],
        Constraints::new(0.0, 100.0, 0.0, 80.0),
    );

    let root_size = tree.get_cached_size(root).unwrap();
    let content_size = tree.get_content_size(root).unwrap();

    approx_eq(root_size.height, 80.0);
    assert!(
        content_size.height > root_size.height,
        "content height should exceed finite viewport"
    );
}

#[test]
fn exclusion_flow_resize_recomputes_placement_deterministically() {
    let exclusion = rect(90.0, 0.0, 60.0, 60.0);
    let flow = ExclusionFlow::new()
        .with_spacing(10.0)
        .with_padding(EdgeInsets::zero())
        .with_exclusion(exclusion);

    let (narrow_tree, _root, narrow_children) = build_tree(
        flow,
        &[(70.0, 20.0), (70.0, 20.0), (70.0, 20.0)],
        Constraints::new(0.0, 180.0, 0.0, 200.0),
    );

    let wide_flow = ExclusionFlow::new()
        .with_spacing(10.0)
        .with_padding(EdgeInsets::zero())
        .with_exclusion(exclusion);
    let (wide_tree, _root, wide_children) = build_tree(
        wide_flow,
        &[(70.0, 20.0), (70.0, 20.0), (70.0, 20.0)],
        Constraints::new(0.0, 260.0, 0.0, 200.0),
    );

    let narrow_last = narrow_tree.get_rect(narrow_children[2]).unwrap();
    let wide_last = wide_tree.get_rect(wide_children[2]).unwrap();

    assert!(
        wide_last.y <= narrow_last.y,
        "wider layout should not push the ordered flow lower"
    );
    assert!(!wide_last.intersects(&exclusion));
}
