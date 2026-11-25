    use super::*;
    use crate::core::animation::{
        Animation, AnimationDescriptor, AnimationSample, Easing, Playback, RepeatCount,
        TransitionHandle,
    };
    use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
    use crate::utils::id::WidgetInternalId;
    use crate::widgets::screen_host::ScreenHost;

    struct AnimatedOwner {
        id: WidgetInternalId,
        handle: Option<TransitionHandle>,
    }

    impl AnimatedOwner {
        fn new() -> Self {
            Self {
                id: WidgetInternalId::new(),
                handle: None,
            }
        }
    }

    impl WidgetDowncast for AnimatedOwner {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for AnimatedOwner {
        fn id(&self) -> WidgetInternalId {
            self.id
        }
        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            constraints: Constraints,
            _children: &[NodeId],
        ) -> crate::core::layout::LayoutResult {
            crate::core::layout::LayoutResult::fit(Size::new(
                constraints.max_width,
                constraints.max_height,
            ))
        }
        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}
        fn hit_testable(&self) -> bool {
            false
        }
        fn update(&mut self, _dt: f32, cx: &mut UpdateContext<'_>) {
            if self.handle.is_none() {
                self.handle = Some(
                    cx.start_animation_descriptor(AnimationDescriptor::new(
                        Animation::new(0.0f32, 1.0, 1.0, Easing::Linear),
                        Playback::Loop(RepeatCount::Infinite),
                    ))
                    .unwrap()
                    .expect("canonical Widget Update receives animation authority"),
                );
            }
        }
    }

    fn handle(tree: &WidgetTree, node: NodeId) -> TransitionHandle {
        tree.node(node)
            .unwrap()
            .widget()
            .as_any()
            .downcast_ref::<AnimatedOwner>()
            .unwrap()
            .handle
            .unwrap()
    }

    #[test]
    fn timeline_contract_retained_replacement_cancels_predecessor_before_successor_update() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let node = tree.add_child(root, Box::new(AnimatedOwner::new()), None, 0);
        let owner = tree.node(node).unwrap().id();
        let theme = Theme::default();
        let icons = crate::core::icon::IconRegistry::new(&[]);
        let mut text = crate::platform::font_manager_text_engine::FontManagerTextEngine::new();
        let mut harness = RetainedSceneTestHarness::new();
        harness.advance_transition_time(0.0);
        harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(128.0, 32.0)),
            &theme,
            &mut text,
            &icons,
        );
        let predecessor = handle(&tree, node);
        assert_eq!(
            harness
                .transition_layer
                .timeline()
                .sample::<f32>(predecessor),
            AnimationSample::Active(0.0)
        );

        tree.replace_widget_only(node, Box::new(AnimatedOwner::new()));
        assert_eq!(tree.node(node).unwrap().id(), owner);
        // No intervening Timeline advance: the shared pre-Update lifetime cut
        // must cancel the dead instance before this successor starts.
        harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(128.0, 32.0)),
            &theme,
            &mut text,
            &icons,
        );
        let successor = handle(&tree, node);
        assert_ne!(predecessor, successor);
        assert_eq!(
            harness
                .transition_layer
                .timeline()
                .sample::<f32>(predecessor),
            AnimationSample::Cancelled
        );
        assert_eq!(
            harness.transition_layer.timeline().sample::<f32>(successor),
            AnimationSample::Active(0.0)
        );
        assert_eq!(harness.transition_layer.timeline().active_count(), 1);
        harness.advance_transition_time(0.25);
        assert_eq!(
            harness
                .transition_layer
                .timeline()
                .sample::<f32>(predecessor),
            AnimationSample::Missing
        );
        assert_eq!(
            harness.transition_layer.timeline().sample::<f32>(successor),
            AnimationSample::Active(0.25)
        );

        tree.remove_node(node);
        harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(128.0, 32.0)),
            &theme,
            &mut text,
            &icons,
        );
        assert_eq!(
            harness.transition_layer.timeline().sample::<f32>(successor),
            AnimationSample::Cancelled
        );
        assert!(!harness.transition_layer.has_active_animations());
        harness.advance_transition_time(0.5);
        assert_eq!(
            harness.transition_layer.timeline().sample::<f32>(successor),
            AnimationSample::Missing
        );
    }
