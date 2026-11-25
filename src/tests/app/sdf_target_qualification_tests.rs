    use super::*;
    use crate::core::widget::Widget;
    use crate::renderer::custom_render_prepare::CustomRenderActivity;
    use crate::renderer::pixel_coverage::PhysicalExtent;
    use crate::renderer::sdf_material_status::{SdfMaterialKey, SdfMaterialRevision};
    use crate::widgets::{
        button::{Button, ButtonSdfRestingProfile},
        flex::Flex,
        label::Label,
    };

    #[test]
    fn app_qualifies_first_button_sdf_request_from_active_target_after_layout() {
        fn button(tree: &mut WidgetTree, node: NodeId) -> &mut Button {
            tree.node_mut(node)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Button>()
                .unwrap()
        }

        let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
            .expect("SDF target qualification requires a real renderer");
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 240.0, 100.0));
        let surface =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 240.0, 100.0), SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (480, 200), 2.0)
            .unwrap();
        assert!(renderer.target_has_active_extent(surface));
        let target = renderer.custom_render_target_snapshot(surface).unwrap();

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let owner_node = tree.add_child(root, Box::new(Button::with_text("Stage")), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("unchanged peer")), None, 0);
        let owner = tree.node(owner_node).unwrap().id;
        let root_owner = tree.node(root).unwrap().id;
        let peer_owner = tree.node(peer).unwrap().id;
        assert!(!tree.has_custom_render_participants());
        assert!(tree.presentation_inbox.is_none());
        button(&mut tree, owner_node)
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        assert!(
            button(&mut tree, owner_node)
                .sdf_request_snapshot()
                .is_none()
        );
        assert!(tree.snapshot_button_sdf_requests().is_empty());

        let theme = Theme::default();
        let domains = InteractionDomains::new();
        let focus_before = (
            domains.primary_state().focus_manager.focused(),
            domains.primary_state().keyboard_mode,
        );
        let icons = crate::core::icons::builtin_registry();
        let mut text = crate::platform::font_manager_text_engine::FontManagerTextEngine::new();
        let mut harness = RetainedSceneTestHarness::new();
        let mut frame = |tree: &mut WidgetTree, harness: &mut RetainedSceneTestHarness, target| {
            harness.frame_with_optional_custom_render_target(
                tree,
                root,
                Constraints::tight(Size::new(240.0, 100.0)),
                &theme,
                &mut text,
                &icons,
                target,
                Some((&domains, WindowId::PRIMARY)),
            )
        };
        frame(&mut tree, &mut harness, Some(target));
        let first = harness.cache.qualified_sdf_requests.as_ref().unwrap()[0].clone();
        assert_eq!(
            harness.cache.qualified_sdf_requests.as_ref().unwrap().len(),
            1
        );
        assert_eq!(first.request.owner(), owner);
        assert_eq!(
            first.request.request().profile,
            ButtonSdfRestingProfile::Light
        );
        assert_eq!(
            first.request.request().revision,
            SdfMaterialRevision::initial()
        );
        assert!(!button(&mut tree, owner_node).needs_update());
        let metrics = target
            .metrics_for(
                tree.get_absolute_paint_rect(owner_node).unwrap(),
                tree.get_viewport(owner_node).unwrap(),
            )
            .unwrap();
        assert_eq!(metrics.surface_id, surface);
        assert_eq!(metrics.scale_factor, 2.0);
        assert_eq!(
            metrics.target_physical_extent,
            PhysicalExtent::new(480, 200)
        );
        assert_eq!(metrics.target_logical_extent, Size::new(240.0, 100.0));
        assert!(matches!(
            metrics.activity,
            CustomRenderActivity::Active { .. }
        ));
        assert_eq!(
            first.key,
            SdfMaterialKey::new(
                tree.snapshot_tree_lifetime(),
                owner,
                surface,
                first.request.request().revision,
                metrics,
            )
            .unwrap()
            .with_composited_placement(metrics.final_logical_rect, metrics.final_logical_rect)
        );

        let geometry = |tree: &WidgetTree| {
            [root, owner_node, peer].map(|id| {
                let node = tree.node(id).unwrap();
                (
                    node.id,
                    node.widget.id(),
                    tree.get_absolute_paint_rect(id),
                    tree.get_viewport(id),
                    node.last_constraints,
                )
            })
        };
        let original_geometry = geometry(&tree);
        #[cfg(debug_assertions)]
        let original_epochs =
            [root, owner_node, peer].map(|id| tree.node(id).unwrap().layout_epoch);
        let locals = [root_owner, owner, peer_owner].map(|id| harness.local_fragment(id).unwrap());
        button(&mut tree, owner_node)
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        let same_target = renderer.custom_render_target_snapshot(surface).unwrap();
        assert_eq!(same_target, target);
        let equal_frame = frame(&mut tree, &mut harness, Some(same_target));
        assert!(!equal_frame.scene_built && !equal_frame.visual_changed);
        assert_eq!(
            harness.cache.qualified_sdf_requests.as_deref().unwrap(),
            &vec![first.clone()]
        );

        button(&mut tree, owner_node)
            .request_sdf_material(Some(ButtonSdfRestingProfile::Dark))
            .unwrap();
        frame(&mut tree, &mut harness, Some(same_target));
        let dark = harness.cache.qualified_sdf_requests.as_ref().unwrap()[0].clone();
        assert_ne!(dark.key, first.key);
        assert_eq!(
            dark.request.request().profile,
            ButtonSdfRestingProfile::Dark
        );
        assert_eq!(
            dark.request.request().revision,
            first
                .request
                .request()
                .revision
                .checked_increment()
                .unwrap()
        );
        assert!(!tree.is_current_button_sdf_request(&first.request));
        assert_eq!(geometry(&tree), original_geometry);
        #[cfg(debug_assertions)]
        assert_eq!(
            [root, owner_node, peer].map(|id| tree.node(id).unwrap().layout_epoch),
            original_epochs
        );
        for (id, local) in [root_owner, owner, peer_owner].into_iter().zip(&locals) {
            assert!(Arc::ptr_eq(local, &harness.local_fragment(id).unwrap()));
        }
        assert_eq!(
            (
                domains.primary_state().focus_manager.focused(),
                domains.primary_state().keyboard_mode
            ),
            focus_before
        );

        button(&mut tree, owner_node)
            .request_sdf_material(None)
            .unwrap();
        frame(&mut tree, &mut harness, Some(same_target));
        assert!(harness.cache.qualified_sdf_requests.is_none());
        assert!(!tree.is_current_button_sdf_request(&dark.request));
        button(&mut tree, owner_node)
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        frame(&mut tree, &mut harness, Some(same_target));
        let restored = harness.cache.qualified_sdf_requests.as_ref().unwrap()[0].clone();
        tree.remove_node(owner_node);
        assert!(!tree.is_current_button_sdf_request(&restored.request));
        frame(&mut tree, &mut harness, Some(same_target));
        assert!(harness.cache.qualified_sdf_requests.is_none());
    }
