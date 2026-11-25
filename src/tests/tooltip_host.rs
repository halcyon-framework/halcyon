use std::collections::HashMap;

use crate::core::animation::Timeline;
use crate::core::app::{layout_tree, test_begin_frame_layout_tracking};
use crate::core::event::KeyboardAuthorityMode;
use crate::core::icons::builtin_registry;
use crate::core::layout::Constraints;
use crate::core::math::RectF;
use crate::core::rect_authority::{
    LayoutAuthorityGuard, enter_paint_authority, exit_paint_authority,
};
use crate::core::scene::SceneBuilder;
use crate::core::theme::Theme;
use crate::core::widget::{PaintContext, Widget};
use crate::core::widget_tree::{OverlayHitRegion, WidgetTree};
use crate::model::tooltip::{TooltipPayload, TooltipPlacement};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::widgets::TooltipHost;
use crate::widgets::label::Label;

#[test]
fn consumer_contract_tooltip_host_preserves_child_layout_and_registers_owned_trigger() {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(TooltipHost::new("Description").with_placement(TooltipPlacement::Below)),
        None,
        0,
    );
    let child = tree.add_child(root, Box::new(Label::new("Target")), None, 0);
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    test_begin_frame_layout_tracking();
    {
        let _guard = LayoutAuthorityGuard::enter();
        layout_tree(
            &mut tree,
            root,
            Constraints::new(0.0, 200.0, 0.0, 100.0),
            &theme,
            &mut text_engine,
            None,
        );
    }

    let root_rect = tree.get_rect(root).unwrap();
    let child_rect = tree.get_rect(child).unwrap();
    assert_eq!(child_rect.x, 0.0);
    assert_eq!(child_rect.y, 0.0);
    assert_eq!(child_rect.width, root_rect.width);
    assert_eq!(child_rect.height, root_rect.height);

    let (widget, children, _) = tree.take_widget(root).unwrap();
    assert!(!widget.hit_testable());
    let mut scene = SceneBuilder::new();
    let icon_registry = builtin_registry();
    let timeline = Timeline::new();
    let interpolated_rects = HashMap::new();
    let mut overlay_regions: Vec<OverlayHitRegion> = Vec::new();
    let mut tooltip_triggers = Vec::new();
    {
        let mut paint_cx = PaintContext::new(
            &tree,
            &mut scene,
            &theme,
            &mut text_engine,
            &icon_registry,
            0,
            &timeline,
            &interpolated_rects,
            &mut overlay_regions,
            &mut tooltip_triggers,
            1.0,
            None,
            KeyboardAuthorityMode::Navigation,
        );
        paint_cx.set_current_node(root);
        enter_paint_authority();
        widget.paint(&mut paint_cx, root_rect, &children);
        exit_paint_authority();
    }

    assert_eq!(tooltip_triggers.len(), 1);
    let trigger = &tooltip_triggers[0];
    assert_eq!(trigger.trigger_rect, root_rect);
    assert_eq!(trigger.placement, TooltipPlacement::Below);
    match &trigger.content {
        TooltipPayload::Text(text) => assert_eq!(text, "Description"),
    }
}
