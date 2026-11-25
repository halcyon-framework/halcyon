//! Ordinary consumer API: no private renderer or test-harness imports.
use halcyon::{core::text_selection::TextSelectionPolicy, widgets::markdown::*};
use std::sync::Arc;

#[test]
fn markdown_public_document_view_and_explicit_copy_values() {
    let source: Arc<str> = "# Reader\n\n**native**\n\n```\ncode\tvalue\n```".into();
    let doc = MarkdownDocument::parse(source.clone(), MarkdownLimits::default()).unwrap();
    let style = MarkdownStyle::new("Arial", "Courier New")
        .unwrap()
        .with_size(18.0)
        .unwrap();
    let reader =
        MarkdownView::new(doc.clone(), style).with_selection_policy(TextSelectionPolicy::Enabled);
    assert!(Arc::ptr_eq(reader.document(), &doc));
    assert_eq!(reader.document().source(), source.as_ref());
    assert_eq!(doc.reading_text(), "Reader\n\nnative\n\ncode\tvalue\n");
    assert_eq!(doc.code_literal(0), Some("code\tvalue\n"));
    assert!(reader.layout_failure().is_none());
    let changed =
        MarkdownDocument::parse("new revision".into(), MarkdownLimits::default()).unwrap();
    assert!(!Arc::ptr_eq(&doc, &changed));
    assert!(matches!(
        MarkdownDocument::parse_with_cancel("x".into(), MarkdownLimits::default(), || true),
        Err(MarkdownPrepareError::Canceled)
    ));
}

#[cfg(target_os = "macos")]
#[test]
fn markdown_public_screen_cover_return_stages_exact_reading_value() {
    use halcyon::{
        core::{
            app::{
                AppContext, AppRoot, EngineTicker, Screen, ScreenContext, ScreenFrameContext,
                build_scene_with_appearance,
            },
            event::{
                FusedInputEvent, InteractionDomainState, InteractionDomains, Modifiers,
                PointerButton, PointerDevice, WindowId,
            },
            event_router::EventRouter,
            icon::IconRegistry,
            layout::Constraints,
            layout_transition::ActiveLayoutTransitions,
            math::{Point, Size},
            theme::Theme,
            transition::TransitionLayer,
            widget::Widget,
            widget_tree::{NodeId, WidgetTree},
        },
        platform::{font_manager::FontManager, font_manager_text_engine::FontManagerTextEngine},
        runtime::binding_registry::BindingRegistry,
        widgets::{flex::Flex, label::Label},
    };
    use std::{cell::RefCell, rc::Rc, time::Instant};
    #[derive(Default)]
    struct Saved {
        node: Option<NodeId>,
        value: Option<MarkdownReadHandoff>,
        enters: usize,
        leaves: usize,
    }
    struct Reader {
        doc: Arc<MarkdownDocument>,
        saved: Rc<RefCell<Saved>>,
    }
    impl Screen for Reader {
        fn on_enter(&mut self, _: &mut ScreenContext<'_>) {
            self.saved.borrow_mut().enters += 1;
        }
        fn on_leave(&mut self, _: &mut ScreenContext<'_>) {
            self.saved.borrow_mut().leaves += 1;
        }
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let root = cx.mount_child(Box::new(Flex::column()));
            let mut reader = MarkdownView::new(
                self.doc.clone(),
                MarkdownStyle::new("Arial", "Courier New").unwrap(),
            )
            .with_selection_policy(TextSelectionPolicy::Enabled)
            .with_selection_retention(MarkdownSelectionRetention::RetainOnBlur);
            if let Some(value) = &self.saved.borrow().value {
                reader.try_stage_read_handoff(value).unwrap();
            }
            self.saved.borrow_mut().node = Some(cx.tree.add_child(root, Box::new(reader), None, 0));
            cx.tree.node_mut(root).unwrap().widget_mut()
        }
        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            let mut saved = self.saved.borrow_mut();
            let view = cx
                .tree
                .as_deref()
                .unwrap()
                .node(saved.node.unwrap())
                .unwrap()
                .widget()
                .as_any()
                .downcast_ref::<MarkdownView>()
                .unwrap();
            if view.read_handoff_status() != MarkdownReadHandoffStatus::Pending {
                saved.value = Some(view.capture_read_handoff().unwrap());
            }
        }
    }
    struct Cover;
    impl Screen for Cover {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let root = cx.mount_child(Box::new(Label::new("cover")));
            cx.tree.node_mut(root).unwrap().widget_mut()
        }
    }
    let mut fonts = FontManager::new();
    let primary = fonts
        .load_font_file("/System/Library/Fonts/Supplemental/Arial.ttf")
        .unwrap();
    fonts
        .load_font_file("/System/Library/Fonts/Supplemental/Courier New.ttf")
        .unwrap();
    let mut text = FontManagerTextEngine::with_face_catalog(fonts.face_catalog(), primary).unwrap();
    let doc =
        MarkdownDocument::parse("Alpha beta gamma".into(), MarkdownLimits::default()).unwrap();
    let saved = Rc::new(RefCell::new(Saved::default()));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let mut app = AppRoot::new(AppContext::new(Theme::default()), root);
    app.push_screen(Box::new(Reader {
        doc: doc.clone(),
        saved: saved.clone(),
    }));
    app.rebuild_active_view(&mut tree);
    let mut ticker = EngineTicker::new(Instant::now());
    let timing = ticker.begin_frame();
    let mut transitions = TransitionLayer::new();
    let mut layout = ActiveLayoutTransitions::new();
    tree.begin_layout_transition_frame();
    let _scene = build_scene_with_appearance(
        &mut tree,
        root,
        Constraints::tight(Size::new(400.0, 160.0)),
        &app.context.theme,
        &mut text,
        &IconRegistry::new(&[]),
        ticker.frame_time_ms(timing.now),
        timing.dt.as_secs_f32(),
        &mut transitions,
        &mut layout,
        None,
        1.0,
        None,
        None,
        false,
        &mut Vec::new(),
        None,
        &InteractionDomains::new(),
        WindowId::PRIMARY,
    );
    let node = saved.borrow().node.unwrap();
    let rect = tree.get_absolute_paint_rect(node).unwrap();
    let mut router = EventRouter::new();
    let mut domain = InteractionDomainState::new();
    let mut bindings = BindingRegistry::new();
    for event in [
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(rect.x + 2.0, rect.y + 8.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
        FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(rect.x + 2.0, rect.y + 8.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        },
        FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: "a".into(),
            modifiers: Modifiers {
                meta: true,
                ctrl: true,
                ..Default::default()
            },
        },
    ] {
        router.route_input_event(
            &mut tree,
            event,
            Some(root),
            &mut text,
            None,
            &mut domain,
            &mut bindings,
        );
    }
    app.flush_pending_actions_all(&mut tree);
    let original = saved.borrow().value.clone().unwrap();
    assert_eq!(
        (
            original.selection_anchor_byte(),
            original.selection_head_byte()
        ),
        (0, doc.reading_text().len())
    );
    assert!(Arc::ptr_eq(original.document(), &doc));
    assert_eq!(format!("{original:?}"), "MarkdownReadHandoff { .. }");
    fn send_sync<T: Send + Sync>() {}
    send_sync::<MarkdownReadHandoff>();
    app.push_screen(Box::new(Cover));
    app.rebuild_active_view(&mut tree);
    app.pop_screen().unwrap();
    app.rebuild_active_view(&mut tree);
    let borrowed = saved.borrow();
    let target = tree
        .node(borrowed.node.unwrap())
        .unwrap()
        .widget()
        .as_any()
        .downcast_ref::<MarkdownView>()
        .unwrap();
    assert_eq!(
        target.read_handoff_status(),
        MarkdownReadHandoffStatus::Pending
    );
    assert!(Arc::ptr_eq(target.document(), &doc));
    assert_eq!((borrowed.enters, borrowed.leaves), (2, 1));
    assert_eq!(
        borrowed.value.as_ref().unwrap().selection_head_byte(),
        original.selection_head_byte()
    );
    // Ordinary callers cannot publish. Internal first-Paint/Metal proof exercises
    // the original engine-owned cut; this public target deliberately stops Pending.
}
