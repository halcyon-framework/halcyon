use crate::core::math::RectF;
use crate::model::tooltip::{TooltipManager, TooltipPayload, TooltipPlacement, TooltipTiming};
use crate::utils::id::TreeWidgetId;

fn tooltip_text(manager: &TooltipManager) -> Option<&str> {
    match manager.content() {
        Some(TooltipPayload::Text(text)) => Some(text.as_str()),
        None => None,
    }
}

#[test]
fn tooltip_manager_distinguishes_regions_owned_by_the_same_widget() {
    let mut manager = TooltipManager::new(TooltipTiming {
        show_delay_ms: 0,
        hide_delay_ms: 0,
        reshow_window_ms: 100,
    });
    let widget_id = TreeWidgetId::from_raw(7);
    let first_rect = RectF::new(0.0, 0.0, 40.0, 20.0);
    let second_rect = RectF::new(44.0, 0.0, 40.0, 20.0);

    manager.hover_enter(
        widget_id,
        first_rect,
        TooltipPayload::Text("first".into()),
        TooltipPlacement::Auto,
        1_000,
    );
    assert!(manager.tick(0.0, 1_000));
    assert_eq!(tooltip_text(&manager), Some("first"));

    manager.hover_enter(
        widget_id,
        second_rect,
        TooltipPayload::Text("second".into()),
        TooltipPlacement::Auto,
        1_001,
    );
    assert!(manager.tick(0.0, 1_001));
    assert_eq!(manager.trigger_rect(), Some(second_rect));
    assert_eq!(tooltip_text(&manager), Some("second"));
}

#[test]
fn tooltip_manager_refreshes_content_for_an_unchanged_trigger_region() {
    let mut manager = TooltipManager::new(TooltipTiming {
        show_delay_ms: 0,
        hide_delay_ms: 0,
        reshow_window_ms: 100,
    });
    let widget_id = TreeWidgetId::from_raw(8);
    let rect = RectF::new(0.0, 0.0, 40.0, 20.0);

    manager.hover_enter(
        widget_id,
        rect,
        TooltipPayload::Text("old".into()),
        TooltipPlacement::Auto,
        1_000,
    );
    assert!(manager.tick(0.0, 1_000));

    manager.hover_enter(
        widget_id,
        rect,
        TooltipPayload::Text("new".into()),
        TooltipPlacement::Below,
        1_001,
    );
    assert_eq!(tooltip_text(&manager), Some("new"));
    assert_eq!(manager.placement(), TooltipPlacement::Below);
}

#[test]
fn tooltip_projection_uses_current_declarations_before_retained_scene_cutoff() {
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::icon::IconRegistry;
    use crate::core::layout::{Constraints, LayoutResult};
    use crate::core::math::{Point, Size};
    use crate::core::rect_authority::UpdateAuthorityGuard;
    use crate::core::scene::SceneNode;
    use crate::core::theme::Theme;
    use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
    use crate::core::widget_tree::{NodeId, WidgetTree};
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::utils::id::WidgetInternalId;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::sync::Arc;

    struct TriggerProbe {
        id: WidgetInternalId,
        declaration: Rc<RefCell<Option<(String, TooltipPlacement)>>>,
        layouts: Rc<Cell<usize>>,
    }

    impl WidgetDowncast for TriggerProbe {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for TriggerProbe {
        fn id(&self) -> WidgetInternalId {
            self.id
        }
        fn layout(
            &mut self,
            _: &mut LayoutContext,
            constraints: Constraints,
            _: &[NodeId],
        ) -> LayoutResult {
            self.layouts.set(self.layouts.get() + 1);
            LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
        }
        fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
            if let Some((text, placement)) = self.declaration.borrow().as_ref() {
                cx.register_tooltip_at(
                    RectF::new(
                        rect.x + rect.width * 0.2,
                        rect.y + rect.height * 0.2,
                        rect.width * 0.5,
                        rect.height * 0.25,
                    ),
                    TooltipPayload::Text(text.clone()),
                    *placement,
                );
            }
        }
    }

    fn tooltip_position(nodes: &[SceneNode], expected: &str) -> Option<Point> {
        nodes.iter().find_map(|node| match node {
            SceneNode::Text(text) if text.text == expected => Some(text.position),
            SceneNode::RetainedFragment(fragment) => tooltip_position(&fragment.children, expected),
            SceneNode::Clip(clip) => tooltip_position(&clip.children, expected),
            SceneNode::Layer(layer) => tooltip_position(&layer.children, expected),
            SceneNode::Scroll(scroll) => tooltip_position(&scroll.children, expected),
            _ => None,
        })
    }

    let declaration = Rc::new(RefCell::new(Some((
        "old".to_string(),
        TooltipPlacement::Above,
    ))));
    let layouts = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(TriggerProbe {
            id: WidgetInternalId::new(),
            declaration: declaration.clone(),
            layouts: layouts.clone(),
        }),
        None,
        0,
    );
    let owner = tree.widget_id_for_node(root).unwrap();
    let mut harness = RetainedSceneTestHarness::new();
    let mut manager = TooltipManager::new(TooltipTiming {
        show_delay_ms: 500,
        hide_delay_ms: 0,
        reshow_window_ms: 0,
    });
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let pointer = Point::new(160.0, 110.0);
    let constraints = Constraints::tight(Size::new(640.0, 480.0));

    let (first, hover_work) = harness.frame_with_tooltips(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text,
        &icons,
        &mut manager,
        pointer,
        1.0,
    );
    assert!(first.scene_built && hover_work);
    assert!(!manager.is_visible());
    let (entered, hover_work) = harness.frame_with_tooltips(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text,
        &icons,
        &mut manager,
        pointer,
        1.0,
    );
    assert!(!entered.scene_built && hover_work);
    assert!(
        !manager.is_visible(),
        "pre-entry idle time must not satisfy dwell"
    );
    let (shown, _) = harness.frame_with_tooltips(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text,
        &icons,
        &mut manager,
        pointer,
        0.5,
    );
    let old_position = tooltip_position(&shown.scene_root, "old").unwrap();
    let old_layouts = layouts.get();
    {
        let _update = UpdateAuthorityGuard::enter();
        *declaration.borrow_mut() = Some(("fresh placement".to_string(), TooltipPlacement::Below));
        tree.mark_paint_dirty(root);
    }
    let (changed, hover_work) = harness.frame_with_tooltips(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text,
        &icons,
        &mut manager,
        pointer,
        0.0,
    );
    assert!(changed.scene_built && changed.visual_changed && hover_work);
    assert_eq!(layouts.get(), old_layouts);
    assert!(tooltip_position(&changed.scene_root, "old").is_none());
    let new_position = tooltip_position(&changed.scene_root, "fresh placement").unwrap();
    let trigger = &changed.tooltip_triggers[0];
    assert_eq!(
        new_position.y,
        trigger.trigger_rect.y + trigger.trigger_rect.height + theme.tooltip.gap + 4.0
    );
    assert_ne!(new_position, old_position);
    let fragment = harness.local_fragment(owner).unwrap();
    let (clean, hover_work) = harness.frame_with_tooltips(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text,
        &icons,
        &mut manager,
        pointer,
        0.0,
    );
    assert!(!clean.scene_built && !clean.visual_changed && !hover_work);
    assert!(Arc::ptr_eq(
        &fragment,
        &harness.local_fragment(owner).unwrap()
    ));

    let resized = Constraints::tight(Size::new(480.0, 360.0));
    let (moved, hover_work) = harness.frame_with_tooltips(
        &mut tree,
        root,
        resized,
        &theme,
        &mut text,
        &icons,
        &mut manager,
        pointer,
        0.0,
    );
    assert!(moved.scene_built && moved.visual_changed && hover_work);
    assert!(tooltip_position(&moved.scene_root, "fresh placement").is_none());
    assert!(moved.tooltip_triggers[0].trigger_rect.contains(pointer));
    let (waiting, hover_work) = harness.frame_with_tooltips(
        &mut tree,
        root,
        resized,
        &theme,
        &mut text,
        &icons,
        &mut manager,
        pointer,
        0.5,
    );
    assert!(!waiting.scene_built && hover_work);
    assert!(!manager.is_visible());
    let (reshown, hover_work) = harness.frame_with_tooltips(
        &mut tree,
        root,
        resized,
        &theme,
        &mut text,
        &icons,
        &mut manager,
        pointer,
        0.5,
    );
    assert!(!hover_work);
    let resized_position = tooltip_position(&reshown.scene_root, "fresh placement").unwrap();
    assert_ne!(resized_position, new_position);
    {
        let _update = UpdateAuthorityGuard::enter();
        *declaration.borrow_mut() = None;
        tree.mark_paint_dirty(root);
    }
    let (removed, hover_work) = harness.frame_with_tooltips(
        &mut tree,
        root,
        resized,
        &theme,
        &mut text,
        &icons,
        &mut manager,
        pointer,
        0.0,
    );
    assert!(removed.scene_built && removed.visual_changed && hover_work);
    assert!(removed.tooltip_triggers.is_empty());
    assert!(tooltip_position(&removed.scene_root, "fresh placement").is_none());
    assert!(manager.find_trigger_at(pointer.x, pointer.y).is_none());
    let (settled, hover_work) = harness.frame_with_tooltips(
        &mut tree,
        root,
        resized,
        &theme,
        &mut text,
        &icons,
        &mut manager,
        pointer,
        0.0,
    );
    assert!(!settled.scene_built && !settled.visual_changed && !hover_work);
    assert!(!manager.is_visible());
}
