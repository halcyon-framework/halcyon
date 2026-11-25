//! HAL390 integration at the existing retained/native text/Metal owners.
#![cfg(all(target_os = "macos", debug_assertions))]
use super::_support::markdown_fonts as fonts;
use super::renderer_z_order::{read_compiled_surface, setup_with_size_and_scale};
use crate::{
    core::{
        app::{
            AppContext, AppRoot, RetainedSceneTestHarness, Screen, ScreenContext,
            ScreenFrameContext,
        },
        event::{
            FusedInputEvent, InteractionDomainState, Modifiers, PointerButton, PointerDevice,
            WindowId,
        },
        event_router::EventRouter,
        icon::IconRegistry,
        layout::{Constraints, FlexParams},
        math::{Point, Size},
        scene::{Color, SceneGraph, SceneNode, TextNode},
        text_selection::TextSelectionPolicy,
        theme::Theme,
        widget_tree::{NodeId, WidgetTree},
    },
    platform::font_manager::{FontManager, FontWeight},
    renderer::{DrawPlanBuilder, TextRealizationOutcome, mesh_cache::MeshCache},
    runtime::binding_registry::BindingRegistry,
    widgets::{
        flex::Flex,
        markdown::{
            MarkdownDocument, MarkdownLimits, MarkdownReadHandoff, MarkdownReadHandoffStatus,
            MarkdownSelectionRetention, MarkdownStyle, MarkdownView,
        },
        scroll_area::ScrollArea,
        text_field::TextField,
    },
};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc, time::Instant};

fn style() -> MarkdownStyle {
    MarkdownStyle::new("Arial", "Courier New")
        .unwrap()
        .with_size(16.0)
        .unwrap()
}
fn document(source: &str) -> Arc<MarkdownDocument> {
    MarkdownDocument::parse(source.into(), MarkdownLimits::default()).unwrap()
}
fn collect<'a>(nodes: &'a [SceneNode], out: &mut Vec<&'a TextNode>) {
    for node in nodes {
        match node {
            SceneNode::Text(t) => out.push(t),
            SceneNode::RetainedFragment(f) => collect(&f.children, out),
            SceneNode::Clip(c) => collect(&c.children, out),
            SceneNode::Scroll(s) => collect(&s.children, out),
            SceneNode::Layer(l) => collect(&l.children, out),
            _ => {}
        }
    }
}
fn assemble(
    source: &str,
    with_input: bool,
) -> (WidgetTree, NodeId, NodeId, NodeId, Option<NodeId>) {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let input = with_input.then(|| {
        let node = tree.add_child(root, Box::new(TextField::new()), None, 0);
        tree.set_flex(node, FlexParams::fixed(40.0));
        node
    });
    let scroll = tree.add_child(
        root,
        Box::new(ScrollArea::new_unwired().enable_horizontal_scrollbar(true)),
        None,
        0,
    );
    tree.set_flex(scroll, FlexParams::flexible(1.0));
    let view = tree.add_child(
        scroll,
        Box::new(
            MarkdownView::new(document(source), style())
                .with_selection_policy(TextSelectionPolicy::Enabled),
        ),
        None,
        0,
    );
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(view);
    (tree, root, scroll, view, input)
}
fn scene(frame: &crate::core::app::RetainedSceneTestFrame) -> SceneGraph {
    let mut scene = SceneGraph::new();
    scene.root = frame.scene_root.clone();
    scene.clear_color = Color::TRANSPARENT;
    scene
}
fn root_fragment(frame: &crate::core::app::RetainedSceneTestFrame) -> &Arc<[SceneNode]> {
    match &frame.scene_root[0] {
        SceneNode::RetainedFragment(fragment) => &fragment.children,
        other => panic!(
            "expected original retained root, got {}",
            other.kind() as u8
        ),
    }
}
fn heading_fragment(nodes: &[SceneNode]) -> Option<&Arc<[SceneNode]>> {
    for node in nodes {
        let children = match node {
            SceneNode::RetainedFragment(fragment) => {
                if fragment
                    .children
                    .iter()
                    .any(|n| matches!(n,SceneNode::Text(t) if t.text=="Heading"))
                {
                    return Some(&fragment.children);
                }
                &fragment.children[..]
            }
            SceneNode::Scroll(scroll) => &scroll.children[..],
            SceneNode::Clip(clip) => &clip.children[..],
            SceneNode::Layer(layer) => &layer.children[..],
            _ => continue,
        };
        if let Some(found) = heading_fragment(children) {
            return Some(found);
        }
    }
    None
}
fn evidence(name: &str, pixels: &[u8], width: u32, height: u32) {
    use sha2::{Digest, Sha256};
    let dir = std::path::PathBuf::from("target/evidence/hal390")
        .join(format!("metal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{name}.rgba")), pixels).unwrap();
    let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
    for p in pixels.chunks_exact(4) {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write(dir.join(format!("{name}.ppm")), ppm).unwrap();
    println!(
        "HAL390 {name} physical={width}x{height} RGBA8 SHA256={:x}",
        Sha256::digest(pixels)
    );
}

#[test]
fn markdown_native_retained_styles_and_metal_1x_2x() {
    let source = "# Heading\n\nPlain **bold**, *italic*, ***both***, ~~strike~~ and `inline` 看龍 אבג é.\n\n> Quote\n\n- [x] Native\n- [ ] Passive\n\n```rs\na\tb\n```\n\n| left | center | right |\n| :--- | :---: | ---: |\n| A | 看 | אבג |\n\n![alt](file:inert) [label](javascript:inert)";
    let assets = fonts();
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(680, 600, scale).expect("HAL390 real Metal required");
        renderer.admit_font_assets(&assets).unwrap();
        renderer.text_engine_mut().set_scale(scale);
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        let (mut tree, root, _, view, _) = assemble(source, false);
        let mut harness = RetainedSceneTestHarness::new();
        let icons = IconRegistry::new(&[]);
        let theme = Theme::default();
        let c = Constraints::tight(Size::new(680.0, 600.0));
        let frame = harness.frame(
            &mut tree,
            root,
            c,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        assert!(
            tree.node(view)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<MarkdownView>()
                .unwrap()
                .layout_failure()
                .is_none()
        );
        let mut texts = Vec::new();
        collect(&frame.scene_root, &mut texts);
        let bindings: Vec<_> = texts.iter().filter_map(|t| t.binding.as_ref()).collect();
        assert!(bindings.iter().any(|b| b.face().family_name() == "Arial"
            && b.face().weight() == FontWeight::Bold
            && !b.face().italic()));
        assert!(
            bindings
                .iter()
                .any(|b| b.face().family_name() == "Arial" && b.face().italic())
        );
        assert!(
            bindings
                .iter()
                .any(|b| b.face().family_name() == "Courier New")
        );
        assert!(
            bindings
                .iter()
                .any(|b| b.face().family_name() == "Arial Unicode MS")
        );
        assert!(
            !texts
                .iter()
                .any(|t| t.text.contains("**") || t.text.contains("javascript:"))
        );
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene(&frame))]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .unwrap();
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        assert!(
            compiled
                .text_realization(surface)
                .unwrap()
                .outcome()
                .is_complete()
        );
        let pixels = read_compiled_surface(&renderer, &compiled, surface).unwrap();
        let heading_ink = (0..(36.0 * scale) as usize)
            .flat_map(|y| {
                (0..(160.0 * scale) as usize).map(move |x| (y * (680.0 * scale) as usize + x) * 4)
            })
            .filter(|index| pixels[*index + 3] > 0)
            .count();
        assert!(
            heading_ink > 100,
            "real foreground glyph pixels in a region containing no background decoration"
        );
        let ink = pixels
            .chunks_exact(4)
            .filter(|p| p[3] > 0 && p[..3].iter().any(|v| *v > 100))
            .count();
        assert!(
            ink > 500,
            "actual glyph/geometry output, not compile metadata"
        );
        evidence(
            &format!("markdown-{scale}x"),
            &pixels,
            (680.0 * scale) as u32,
            (600.0 * scale) as u32,
        );
        let again = harness.frame(
            &mut tree,
            root,
            c,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        assert!(
            Arc::ptr_eq(
                heading_fragment(&frame.scene_root).unwrap(),
                heading_fragment(&again.scene_root).unwrap()
            ),
            "unchanged document reuses its exact local scene even while ancestor scrollbar composition changes"
        );
        println!(
            "HAL390 Metal={} DPR={scale} bound_text_nodes={} visible_ink={ink}",
            renderer.gpu_resources().adapter.get_info().name,
            texts.len()
        );
    }
}

#[test]
fn markdown_wide_code_table_scroll_and_resize() {
    let source = format!(
        "Prose wraps at the viewport width and never inherits the wide code extent.\n\n```\n\t{}\n```\n\n| {} | B | C |\n|---|---|---|\n| data | data | data |\n\nend",
        "abcdefghij".repeat(20),
        "LONG".repeat(20)
    );
    let assets = fonts();
    let (mut renderer, compositor, surface) = setup_with_size_and_scale(300, 220, 1.0).unwrap();
    renderer.admit_font_assets(&assets).unwrap();
    let (mut tree, root, scroll, view, _) = assemble(&source, false);
    let mut harness = RetainedSceneTestHarness::new();
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let frame = harness.frame(
        &mut tree,
        root,
        Constraints::tight(Size::new(300.0, 220.0)),
        &theme,
        renderer.text_engine_mut(),
        &icons,
    );
    let before = tree.get_content_size(view).unwrap();
    assert!(
        before.width > 1500.0 && before.height > 220.0,
        "natural code/table extent must reach scroll owner: {before:?}"
    );
    let mut router = EventRouter::new();
    let mut domains = InteractionDomainState::new();
    let mut bindings = BindingRegistry::new();
    router.route_input_event(
        &mut tree,
        FusedInputEvent::Scroll {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(120.0, 100.0),
            delta: (-5000.0, -5000.0),
            source: crate::core::event::ScrollSource::Wheel,
        },
        Some(root),
        renderer.text_engine_mut(),
        None,
        &mut domains,
        &mut bindings,
    );
    let scroll_widget = tree
        .node(scroll)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ScrollArea>()
        .unwrap();
    assert!(
        scroll_widget.target_scroll_x() > 1000.0 && scroll_widget.target_scroll_y() > 0.0,
        "both actual scroll axes remain reachable"
    );
    let after = harness.frame(
        &mut tree,
        root,
        Constraints::tight(Size::new(300.0, 220.0)),
        &theme,
        renderer.text_engine_mut(),
        &icons,
    );
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &HashMap::from([(surface, scene(&after))]),
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .unwrap();
    let compiled = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("native Compile must be ready");
    assert!(
        compiled
            .text_realization(surface)
            .unwrap()
            .outcome()
            .is_complete()
    );
    assert!(read_compiled_surface(&renderer, &compiled, surface).is_some());
    assert!(!Arc::ptr_eq(root_fragment(&frame), root_fragment(&after)));
    harness.frame(
        &mut tree,
        root,
        Constraints::tight(Size::new(500.0, 220.0)),
        &theme,
        renderer.text_engine_mut(),
        &icons,
    );
    assert!(
        tree.node(view)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<MarkdownView>()
            .unwrap()
            .layout_failure()
            .is_none()
    );
}

#[test]
fn markdown_degraded_keeps_same_surface_routed_input() {
    let assets = fonts();
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(360, 220, scale).unwrap();
        renderer.admit_font_assets(&assets).unwrap();
        renderer.text_engine_mut().set_scale(scale);
        let (mut tree, root, _, _, input) = assemble(
            "# Healthy\n\nUncovered \u{10ffff} and **supported** source.",
            true,
        );
        let input = input.unwrap();
        let mut harness = RetainedSceneTestHarness::new();
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let c = Constraints::tight(Size::new(360.0, 220.0));
        let first = harness.frame(
            &mut tree,
            root,
            c,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let point = tree.get_absolute_paint_rect(input).unwrap();
        let mut router = EventRouter::new();
        let mut domains = InteractionDomainState::new();
        let mut bindings = BindingRegistry::new();
        for event in [
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(point.x + 10.0, point.y + point.height * 0.5),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                text: "x".into(),
                modifiers: Modifiers::default(),
            },
        ] {
            router.route_input_event(
                &mut tree,
                event,
                Some(root),
                renderer.text_engine_mut(),
                None,
                &mut domains,
                &mut bindings,
            );
        }
        assert_eq!(
            tree.node(input)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<TextField>()
                .unwrap()
                .text(),
            "x"
        );
        let next = harness.frame(
            &mut tree,
            root,
            c,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        assert!(!Arc::ptr_eq(root_fragment(&first), root_fragment(&next)));
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene(&next))]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .unwrap();
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let outcome = compiled.text_realization(surface).unwrap().outcome();
        assert!(
            matches!(outcome,TextRealizationOutcome::Degraded {missing_cluster_count,..} if *missing_cluster_count==1)
        );
        assert!(!outcome.is_complete() && outcome.is_renderable());
        let pixels = read_compiled_surface(&renderer, &compiled, surface).unwrap();
        assert!(pixels.chunks_exact(4).filter(|p| p[3] > 0).count() > 300);
        evidence(
            &format!("markdown-degraded-input-{scale}x"),
            &pixels,
            (360.0 * scale) as u32,
            (220.0 * scale) as u32,
        );
    }
}

#[derive(Default)]
struct ReadingScreenState {
    node: Option<NodeId>,
    saved: Option<MarkdownReadHandoff>,
    views: Option<NodeId>,
    activations: usize,
    pending_builds: usize,
    enters: usize,
    leaves: usize,
}
struct ReadingScreen {
    document: Arc<MarkdownDocument>,
    state: Rc<RefCell<ReadingScreenState>>,
}
impl Screen for ReadingScreen {
    fn on_enter(&mut self, _: &mut ScreenContext<'_>) {
        self.state.borrow_mut().enters += 1;
    }
    fn on_leave(&mut self, _: &mut ScreenContext<'_>) {
        self.state.borrow_mut().leaves += 1;
    }
    fn build<'a>(
        &'a mut self,
        cx: &'a mut ScreenFrameContext<'a>,
    ) -> &'a mut dyn crate::core::widget::Widget {
        let root = cx.mount_child(Box::new(Flex::column()));
        let mut reader = MarkdownView::new(self.document.clone(), style())
            .with_selection_policy(TextSelectionPolicy::Enabled)
            .with_selection_retention(MarkdownSelectionRetention::RetainOnBlur);
        let mut state = self.state.borrow_mut();
        if let Some(saved) = &state.saved {
            reader.try_stage_read_handoff(saved).unwrap();
            assert_eq!(
                reader.read_handoff_status(),
                MarkdownReadHandoffStatus::Pending
            );
            state.pending_builds += 1;
        }
        state.node = Some(cx.tree.add_child(root, Box::new(reader), None, 0));
        let clicked = self.state.clone();
        state.views = Some(
            cx.tree.add_child(
                root,
                Box::new(
                    crate::widgets::button::Button::with_text("Views")
                        .on_click(move || clicked.borrow_mut().activations += 1),
                ),
                None,
                0,
            ),
        );
        drop(state);
        cx.tree.node_mut(root).unwrap().widget_mut()
    }
    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let mut state = self.state.borrow_mut();
        if let Some(reader) = cx.tree.as_deref().and_then(|tree| {
            state
                .node
                .and_then(|node| tree.node(node))
                .and_then(|node| node.widget().as_any().downcast_ref::<MarkdownView>())
        }) {
            match reader.read_handoff_status() {
                MarkdownReadHandoffStatus::Pending => {}
                MarkdownReadHandoffStatus::Rejected(error) => panic!("reader rejected {error:?}"),
                _ => state.saved = Some(reader.capture_read_handoff().unwrap()),
            }
        }
    }
}
struct CoverScreen;
impl Screen for CoverScreen {
    fn build<'a>(
        &'a mut self,
        cx: &'a mut ScreenFrameContext<'a>,
    ) -> &'a mut dyn crate::core::widget::Widget {
        let node = cx.mount_child(Box::new(crate::widgets::label::Label::new("Covered")));
        cx.tree.node_mut(node).unwrap().widget_mut()
    }
}
fn reader(tree: &WidgetTree, node: NodeId) -> &MarkdownView {
    tree.node(node)
        .unwrap()
        .widget()
        .as_any()
        .downcast_ref()
        .unwrap()
}
fn routed(
    tree: &mut WidgetTree,
    root: NodeId,
    engine: &mut dyn crate::core::text_engine::TextEngine,
    router: &mut EventRouter,
    domains: &mut InteractionDomainState,
    bindings: &mut BindingRegistry,
    event: FusedInputEvent,
) {
    router.route_input_event(tree, event, Some(root), engine, None, domains, bindings);
}
fn reading_point(
    frame: &crate::core::app::RetainedSceneTestFrame,
    engine: &mut dyn crate::core::text_engine::TextEngine,
    offset: usize,
) -> Point {
    let mut texts = Vec::new();
    collect(&frame.scene_root, &mut texts);
    let first = texts
        .iter()
        .position(|text| text.text.starts_with("Alpha beta"))
        .unwrap();
    let mut start = 0;
    for text in &texts[first..] {
        if offset < start + text.text.len() {
            let glyph = engine
                .shape_bound_text(&text.text, text.font_size, text.binding.as_ref().unwrap())
                .unwrap();
            let x = glyph
                .clusters
                .iter()
                .find(|cluster| cluster.source.start == offset - start)
                .unwrap()
                .x_start;
            return Point::new(text.position.x + x, text.position.y + glyph.height * 0.5);
        }
        start += text.text.len();
    }
    panic!("requested routed point outside actual scene text");
}

fn select_reading_interval(
    tree: &mut WidgetTree,
    root: NodeId,
    engine: &mut dyn crate::core::text_engine::TextEngine,
    router: &mut EventRouter,
    domains: &mut InteractionDomainState,
    bindings: &mut BindingRegistry,
    anchor: Point,
    head: Point,
) {
    for event in [
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: anchor,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: head,
            delta: (head.x - anchor.x, head.y - anchor.y),
            device: PointerDevice::Mouse,
            buttons: crate::core::event::PointerButtonsState {
                primary: true,
                ..Default::default()
            },
        },
        FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: head,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        },
    ] {
        routed(tree, root, engine, router, domains, bindings, event);
    }
}

#[test]
fn markdown_read_handoff_screen_return_first_paint_metal_1x_2x() {
    use crate::core::{
        event::{Event, EventPhase, KeyboardAuthorityMode, TextInputEvent},
        widget::{EventContext, Widget},
    };
    let doc = document(
        "Alpha beta gamma 看 é delta epsilon zeta eta theta.\n\nSecond **bold** paragraph with 看 and é, enough text to reflow on return.",
    );
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(520, 300, scale).unwrap();
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        renderer.admit_font_assets(&fonts()).unwrap();
        renderer.text_engine_mut().set_scale(1.0);
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let state = Rc::new(RefCell::new(ReadingScreenState::default()));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let mut app = AppRoot::new(AppContext::new(theme.clone()), root);
        app.push_screen(Box::new(ReadingScreen {
            document: doc.clone(),
            state: state.clone(),
        }));
        app.rebuild_active_view(&mut tree);
        let mut harness = RetainedSceneTestHarness::new();
        let first = harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(520.0, 300.0)),
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let original = state.borrow().node.unwrap();
        let original_mount = tree
            .concrete_mount(tree.node(original).unwrap().id())
            .unwrap();
        let anchor = reading_point(&first, renderer.text_engine_mut(), 24);
        let head = reading_point(&first, renderer.text_engine_mut(), 6);
        let mut router = EventRouter::new();
        let mut domains = InteractionDomainState::new();
        let mut bindings = BindingRegistry::new();
        select_reading_interval(
            &mut tree,
            root,
            renderer.text_engine_mut(),
            &mut router,
            &mut domains,
            &mut bindings,
            anchor,
            head,
        );
        let source = reader(&tree, original).capture_read_handoff().unwrap();
        assert_eq!(
            (source.selection_anchor_byte(), source.selection_head_byte()),
            (24, 6)
        );
        assert!(Arc::ptr_eq(source.document(), &doc));
        assert_eq!(format!("{source:?}"), "MarkdownReadHandoff { .. }");
        let views = state.borrow().views.unwrap();
        let rect = tree.get_absolute_paint_rect(views).unwrap();
        let point = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
        for event in [
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: point,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: point,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
        ] {
            routed(
                &mut tree,
                root,
                renderer.text_engine_mut(),
                &mut router,
                &mut domains,
                &mut bindings,
                event,
            );
        }
        assert_eq!(
            state.borrow().activations,
            1,
            "ordinary Views Button activates once"
        );
        assert_eq!(
            domains.focus_manager.focused(),
            Some(tree.node(views).unwrap().id())
        );
        let after_views = reader(&tree, original).capture_read_handoff().unwrap();
        assert_eq!(
            (
                after_views.selection_anchor_byte(),
                after_views.selection_head_byte()
            ),
            (24, 6)
        );
        app.flush_pending_actions_all(&mut tree);
        assert_eq!(
            state
                .borrow()
                .saved
                .as_ref()
                .unwrap()
                .selection_anchor_byte(),
            24
        );
        app.push_screen(Box::new(CoverScreen));
        app.rebuild_active_view(&mut tree);
        assert!(!original_mount.is_live());
        app.pop_screen().unwrap();
        app.rebuild_active_view(&mut tree);
        let target = state.borrow().node.unwrap();
        assert_eq!(
            reader(&tree, target).read_handoff_status(),
            MarkdownReadHandoffStatus::Pending
        );
        assert!(reader(&tree, target).capture_read_handoff().is_err());
        assert_ne!(
            tree.concrete_mount(tree.node(target).unwrap().id())
                .unwrap(),
            original_mount
        );
        // Same original App cut, before the target's first Layout or Paint.
        renderer.text_engine_mut().set_scale(scale);
        tree.process_text_field_handoffs(renderer.text_engine_mut(), &theme);
        assert_eq!(
            reader(&tree, target).read_handoff_status(),
            MarkdownReadHandoffStatus::Applied
        );
        assert_eq!(
            reader(&tree, target).capture_read_handoff().unwrap_err(),
            crate::widgets::markdown::MarkdownReadHandoffError::LayoutUnavailable
        );
        let next = harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(230.0, 300.0)),
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let applied = reader(&tree, target).capture_read_handoff().unwrap();
        assert_eq!(
            (
                applied.selection_anchor_byte(),
                applied.selection_head_byte()
            ),
            (24, 6)
        );
        let mut texts = Vec::new();
        collect(&next.scene_root, &mut texts);
        let backgrounds: usize = texts.iter().map(|text| text.backgrounds.len()).sum();
        assert!(
            backgrounds > 0,
            "restored highlight must exist in the FIRST painted scene"
        );
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene(&next))]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .unwrap();
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        assert!(
            compiled
                .text_realization(surface)
                .unwrap()
                .outcome()
                .is_complete()
        );
        let highlighted = read_compiled_surface(&renderer, &compiled, surface).unwrap();
        evidence(
            &format!("markdown-read-handoff-first-paint-{scale}x"),
            &highlighted,
            (520.0 * scale) as u32,
            (300.0 * scale) as u32,
        );
        // Copy through the original widget command into an isolated clipboard.
        let mut clipboard = crate::platform::clipboard::Clipboard::new().unwrap();
        clipboard.use_memory_transport_for_test("untouched");
        let owner = tree.node(target).unwrap().id();
        let tree_ptr = &mut tree as *mut WidgetTree;
        {
            let _phase2 = crate::core::rect_authority::UpdateAuthorityGuard::enter();
            let mut cx = EventContext::new(
                owner,
                tree_ptr,
                0,
                &mut clipboard,
                Some(renderer.text_engine_mut()),
                None,
                true,
                false,
                KeyboardAuthorityMode::Capture,
            );
            let widget = tree.node_mut(target).unwrap().widget_mut();
            assert!(widget.handle_event(
                &mut cx,
                &Event::Text(TextInputEvent {
                    text: "c".into(),
                    modifiers: Modifiers {
                        meta: true,
                        ctrl: true,
                        ..Default::default()
                    }
                }),
                EventPhase::Target
            ));
        }
        assert_eq!(
            clipboard.get_text().as_deref(),
            Some(&doc.reading_text()[6..24])
        );
        // A repeated owner cut cannot restore over later real selection/clear.
        assert!(
            tree.node_mut(target)
                .unwrap()
                .widget_mut()
                .clear_read_only_text_selection()
        );
        tree.mark_paint_dirty(target);
        for _ in 0..3 {
            tree.process_text_field_handoffs(renderer.text_engine_mut(), &theme);
        }
        let cleared = reader(&tree, target).capture_read_handoff().unwrap();
        assert_eq!(
            (
                cleared.selection_anchor_byte(),
                cleared.selection_head_byte()
            ),
            (0, 0)
        );
        let plain = harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(230.0, 300.0)),
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let mut plain_texts = Vec::new();
        collect(&plain.scene_root, &mut plain_texts);
        assert!(
            !plain_texts.is_empty(),
            "clear must paint the same actual document, not an empty unchanged-frame scene"
        );
        assert_eq!(
            texts
                .iter()
                .map(|text| (&text.text, text.position))
                .collect::<Vec<_>>(),
            plain_texts
                .iter()
                .map(|text| (&text.text, text.position))
                .collect::<Vec<_>>(),
            "explicit clear preserves actual text and geometry"
        );
        assert_eq!(
            plain_texts
                .iter()
                .map(|text| text.backgrounds.len())
                .sum::<usize>(),
            0
        );
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene(&plain))]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .unwrap();
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let unselected = read_compiled_surface(&renderer, &compiled, surface).unwrap();
        assert_ne!(
            highlighted, unselected,
            "actual Metal highlight output must differ from explicit clear"
        );
        let a = reading_point(&plain, renderer.text_engine_mut(), 6);
        select_reading_interval(
            &mut tree,
            root,
            renderer.text_engine_mut(),
            &mut router,
            &mut domains,
            &mut bindings,
            a,
            a,
        );
        let collapsed = reader(&tree, target).capture_read_handoff().unwrap();
        assert_eq!(
            (
                collapsed.selection_anchor_byte(),
                collapsed.selection_head_byte()
            ),
            (6, 6)
        );
        let mut collapsed_target = MarkdownView::new(doc.clone(), style())
            .with_selection_policy(TextSelectionPolicy::Enabled)
            .with_selection_retention(MarkdownSelectionRetention::RetainOnBlur);
        collapsed_target.try_stage_read_handoff(&collapsed).unwrap();
        let mut collapsed_tree = WidgetTree::new();
        let collapsed_root = collapsed_tree.create_root(Box::new(collapsed_target), None, 0);
        assert_eq!(
            reader(&collapsed_tree, collapsed_root).read_handoff_status(),
            MarkdownReadHandoffStatus::Pending
        );
        collapsed_tree.process_text_field_handoffs(renderer.text_engine_mut(), &theme);
        assert_eq!(
            reader(&collapsed_tree, collapsed_root).read_handoff_status(),
            MarkdownReadHandoffStatus::Applied
        );
        let mut collapsed_harness = RetainedSceneTestHarness::new();
        collapsed_harness.frame(
            &mut collapsed_tree,
            collapsed_root,
            Constraints::tight(Size::new(180.0, 300.0)),
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let restored_collapsed = reader(&collapsed_tree, collapsed_root)
            .capture_read_handoff()
            .unwrap();
        assert_eq!(
            (
                restored_collapsed.selection_anchor_byte(),
                restored_collapsed.selection_head_byte()
            ),
            (6, 6)
        );
        assert_eq!(
            (
                state.borrow().enters,
                state.borrow().leaves,
                state.borrow().pending_builds
            ),
            (2, 1, 1)
        );
        println!(
            "HAL390 reading handoff DPR={scale} reverse24->6 Pending->Applied before first Layout/Paint; real Screen cover/return; Complete Metal highlight spans={backgrounds}; exact isolated Copy; continued collapsed6->6; retired source/repeated cut safe"
        );
    }
}

#[test]
fn markdown_read_handoff_routed_blur_and_concrete_mount_lifetime() {
    use crate::{
        platform::font_manager_text_engine::FontManagerTextEngine,
        widgets::text_field::TextFieldHandoffStatus,
    };
    let mut fonts_manager = FontManager::new();
    let mut primary = None;
    for asset in fonts() {
        primary.get_or_insert(fonts_manager.register_font_asset(asset).unwrap());
    }
    let mut engine =
        FontManagerTextEngine::with_face_catalog(fonts_manager.face_catalog(), primary.unwrap())
            .unwrap();
    let doc = document("Alpha beta gamma delta epsilon.\n\nAnother paragraph.");
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let c = Constraints::tight(Size::new(480.0, 220.0));
    for retention in [
        MarkdownSelectionRetention::ClearOnBlur,
        MarkdownSelectionRetention::RetainOnBlur,
    ] {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let node = tree.add_child(
            root,
            Box::new(
                MarkdownView::new(doc.clone(), style())
                    .with_selection_policy(TextSelectionPolicy::Enabled)
                    .with_selection_retention(retention),
            ),
            None,
            0,
        );
        let clicked = Rc::new(std::cell::Cell::new(0));
        let activated = clicked.clone();
        let other = tree.add_child(
            root,
            Box::new(
                crate::widgets::button::Button::with_text("Views")
                    .on_click(move || activated.set(activated.get() + 1)),
            ),
            None,
            0,
        );
        let mut harness = RetainedSceneTestHarness::new();
        let first = harness.frame(&mut tree, root, c, &theme, &mut engine, &icons);
        let a = reading_point(&first, &mut engine, 17);
        let b = reading_point(&first, &mut engine, 6);
        let mut router = EventRouter::new();
        let mut domains = InteractionDomainState::new();
        let mut bindings = BindingRegistry::new();
        select_reading_interval(
            &mut tree,
            root,
            &mut engine,
            &mut router,
            &mut domains,
            &mut bindings,
            a,
            b,
        );
        assert_eq!(
            reader(&tree, node)
                .capture_read_handoff()
                .unwrap()
                .selection_anchor_byte(),
            17
        );
        let original_owner = tree.node(node).unwrap().id();
        let background = Point::new(470.0, 210.0);
        for event in [
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: background,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: background,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
        ] {
            routed(
                &mut tree,
                root,
                &mut engine,
                &mut router,
                &mut domains,
                &mut bindings,
                event,
            );
        }
        assert_eq!(
            domains.focus_manager.focused(),
            Some(original_owner),
            "nonfocusable background does not fabricate Blur/focus"
        );
        let background_selection = reader(&tree, node).capture_read_handoff().unwrap();
        let expected_background = if retention == MarkdownSelectionRetention::RetainOnBlur {
            (17, 6)
        } else {
            (0, 0)
        };
        assert_eq!(
            (
                background_selection.selection_anchor_byte(),
                background_selection.selection_head_byte()
            ),
            expected_background
        );
        select_reading_interval(
            &mut tree,
            root,
            &mut engine,
            &mut router,
            &mut domains,
            &mut bindings,
            a,
            b,
        );
        let other_id = tree.node(other).unwrap().id();
        let rect = tree.get_absolute_paint_rect(other).unwrap();
        let point = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
        for event in [
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: point,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: point,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
        ] {
            routed(
                &mut tree,
                root,
                &mut engine,
                &mut router,
                &mut domains,
                &mut bindings,
                event,
            );
        }
        assert_eq!(
            clicked.get(),
            1,
            "normal outside Button activation is not consumed by retention"
        );
        let saved = reader(&tree, node).capture_read_handoff().unwrap();
        let expected = if retention == MarkdownSelectionRetention::RetainOnBlur {
            (17, 6)
        } else {
            (0, 0)
        };
        assert_eq!(
            (saved.selection_anchor_byte(), saved.selection_head_byte()),
            expected
        );
        assert_eq!(domains.focus_manager.focused(), Some(other_id));
        if retention == MarkdownSelectionRetention::ClearOnBlur {
            continue;
        }
        let fresh = |value: &MarkdownReadHandoff| {
            let mut target = MarkdownView::new(doc.clone(), style())
                .with_selection_policy(TextSelectionPolicy::Enabled)
                .with_selection_retention(MarkdownSelectionRetention::RetainOnBlur);
            target.try_stage_read_handoff(value).unwrap();
            target
        };
        let pending = tree.add_child(root, Box::new(fresh(&saved)), None, 0);
        let retired = tree
            .concrete_mount(tree.node(pending).unwrap().id())
            .unwrap();
        tree.replace_widget_only(
            pending,
            Box::new(
                MarkdownView::new(doc.clone(), style())
                    .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
        );
        assert!(!retired.is_live());
        tree.process_text_field_handoffs(&mut engine, &theme);
        assert_eq!(
            reader(&tree, pending).read_handoff_status(),
            MarkdownReadHandoffStatus::NotRequested
        );
        tree.replace_widget_only(pending, Box::new(fresh(&saved)));
        let removed = tree
            .concrete_mount(tree.node(pending).unwrap().id())
            .unwrap();
        tree.remove_node(pending);
        assert!(!removed.is_live());
        let reused = tree.add_child(
            root,
            Box::new(MarkdownView::new(doc.clone(), style())),
            None,
            0,
        );
        assert_eq!(reused, pending);
        tree.process_text_field_handoffs(&mut engine, &theme);
        assert_eq!(
            reader(&tree, reused).read_handoff_status(),
            MarkdownReadHandoffStatus::NotRequested
        );
        // Both explicit owner kinds share this same one-shot lazy index.
        let field_saved = TextField::new()
            .with_text("editor unchanged")
            .capture_edit_handoff()
            .unwrap();
        let mut field = TextField::new();
        field.try_stage_edit_handoff(&field_saved).unwrap();
        let field_node = tree.add_child(root, Box::new(field), None, 0);
        tree.replace_widget_only(reused, Box::new(fresh(&saved)));
        tree.process_text_field_handoffs(&mut engine, &theme);
        assert_eq!(
            reader(&tree, reused).read_handoff_status(),
            MarkdownReadHandoffStatus::Applied
        );
        let field = tree
            .node(field_node)
            .unwrap()
            .widget()
            .as_any()
            .downcast_ref::<TextField>()
            .unwrap();
        assert_eq!(field.text(), "editor unchanged");
        assert_eq!(field.edit_handoff_status(), TextFieldHandoffStatus::Applied);
        harness.frame(&mut tree, root, c, &theme, &mut engine, &icons);
        let applied = reader(&tree, reused).capture_read_handoff().unwrap();
        assert_eq!(
            (
                applied.selection_anchor_byte(),
                applied.selection_head_byte()
            ),
            (17, 6)
        );
        // Active pointer selection is not capturable; finishing that actual
        // routed gesture restores ordinary capture without synthetic restore.
        routed(
            &mut tree,
            root,
            &mut engine,
            &mut router,
            &mut domains,
            &mut bindings,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: b,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 2,
            },
        );
        assert_eq!(
            reader(&tree, node).capture_read_handoff().unwrap_err(),
            crate::widgets::markdown::MarkdownReadHandoffError::SelectionInProgress
        );
        routed(
            &mut tree,
            root,
            &mut engine,
            &mut router,
            &mut domains,
            &mut bindings,
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: b,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
        );
        assert!(reader(&tree, node).capture_read_handoff().is_ok());
        println!(
            "HAL390 actual routed reverse/word drag; blur retention; replacement/removal/reused slot; mixed TextField+Markdown owner cut verified"
        );
    }
}
