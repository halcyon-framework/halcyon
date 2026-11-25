use crate::core::app::{
    ContinuousFramePacer, RetainedSceneTestHarness, current_layout_epoch, layout_tree,
    post_layout_sync_scroll_to_lists, test_begin_frame_layout_tracking,
};
use crate::core::icon::IconRegistry;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::Color;
use crate::core::rect_authority::{LayoutAuthorityGuard, UpdateAuthorityGuard};
use crate::core::scene::SceneBuilder;
use crate::core::theme::Theme;
use crate::core::widget::{
    LayoutContext, PaintContext, ScrollSyncInvalidation, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::renderer::batcher::test_flatten_counts;
use crate::utils::id::WidgetInternalId;
use crate::widgets::scroll_area::ScrollArea;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// Local authoring geometry only; clips and scrolling are the real tree owners.
struct ChildClipSourceLayout {
    id: WidgetInternalId,
    size: Size,
    placements: Vec<RectF>,
    color: Option<Color>,
}
impl ChildClipSourceLayout {
    fn new(size: Size, placements: Vec<RectF>, color: Option<Color>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            size,
            placements,
            color,
        }
    }
}
impl WidgetDowncast for ChildClipSourceLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl Widget for ChildClipSourceLayout {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        self.color.is_some()
    }
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        k: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        for (&child, &r) in children.iter().zip(&self.placements) {
            cx.layout_child(child, Constraints::tight(r.size()));
            cx.set_child_rect(child, r);
        }
        LayoutResult::new(k.constrain(self.size), self.size)
    }
    fn paint(&self, cx: &mut PaintContext, r: RectF, _: &[NodeId]) {
        if let Some(color) = self.color {
            cx.scene.push_rect(r, color, 6, Some(self.id.0.into()));
        }
    }
}

#[test]
fn child_clip_source_nested_scroll_offscreen_retention_visible_and_hit_parity() {
    use crate::compositor::{Compositor, SceneKey};
    use crate::core::scene::SceneGraph;
    use crate::core::widget_tree::OverflowMode;
    use crate::renderer::{DrawPlanBuilder, batcher::PrimitiveData, mesh_cache::MeshCache};
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(ChildClipSourceLayout::new(
            Size::new(120.0, 90.0),
            vec![RectF::new(-10.0, 5.0, 100.0, 70.0)],
            None,
        )),
        None,
        0,
    );
    let outer = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
    let content = tree.add_child(
        outer,
        Box::new(ChildClipSourceLayout::new(
            Size::new(180.0, 280.0),
            vec![
                RectF::new(20.0, 130.0, 100.0, 80.0),
                RectF::new(10.0, 145.0, 1.0, 1.0),
            ],
            None,
        )),
        None,
        0,
    );
    tree.node_mut(outer)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(content);
    let host = tree.add_child(
        content,
        Box::new(ChildClipSourceLayout::new(
            Size::new(100.0, 80.0),
            vec![RectF::new(10.0, 10.0, 80.0, 50.0)],
            None,
        )),
        None,
        0,
    );
    tree.set_overflow(host, OverflowMode::Clip);
    let inner = tree.add_child(host, Box::new(ScrollArea::new_unwired()), None, 0);
    let inner_content = tree.add_child(
        inner,
        Box::new(ChildClipSourceLayout::new(
            Size::new(160.0, 140.0),
            vec![RectF::new(5.0, 45.0, 120.0, 30.0)],
            None,
        )),
        None,
        0,
    );
    tree.node_mut(inner)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(inner_content);
    let leaf_color = Color::from_hex(0xA731D9);
    let leaf = tree.add_child(
        inner_content,
        Box::new(ChildClipSourceLayout::new(
            Size::new(120.0, 30.0),
            vec![],
            Some(leaf_color),
        )),
        None,
        0,
    );
    let visible = tree.add_child(
        content,
        Box::new(ChildClipSourceLayout::new(
            Size::new(1.0, 1.0),
            vec![RectF::new(20.0, 0.0, 50.0, 20.0)],
            None,
        )),
        None,
        1,
    );
    let overflow = tree.add_child(
        visible,
        Box::new(ChildClipSourceLayout::new(
            Size::new(50.0, 20.0),
            vec![],
            Some(Color::WHITE),
        )),
        None,
        0,
    );
    let mut harness = RetainedSceneTestHarness::new();
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new_with_system_ui();
    let icons = IconRegistry::new(&[]);
    let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 120.0, 90.0));
    let surface = compositor.create_surface(0, RectF::new(0.0, 0.0, 120.0, 90.0), SceneKey::BASE);
    let mut frame = |tree: &mut WidgetTree, harness: &mut RetainedSceneTestHarness| {
        harness.frame(
            tree,
            root,
            Constraints::tight(Size::new(120.0, 90.0)),
            &theme,
            &mut text,
            &icons,
        )
    };
    frame(&mut tree, &mut harness);
    let raw = tree.get_absolute_paint_rect(host).unwrap();
    assert!(raw.y > 90.0, "source begins beyond the window");
    assert_eq!(tree.child_clip_source(host), Some(raw));
    let before = harness
        .local_fragment(tree.node(leaf).unwrap().id())
        .unwrap();
    let visible_before = harness
        .local_fragment(tree.node(overflow).unwrap().id())
        .unwrap();
    tree.node_mut(outer)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .scroll_to(0.0, 100.0);
    tree.node_mut(inner)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .scroll_to(0.0, 20.0);
    let f = frame(&mut tree, &mut harness);
    assert!(Arc::ptr_eq(
        &before,
        &harness
            .local_fragment(tree.node(leaf).unwrap().id())
            .unwrap()
    ));
    assert!(Arc::ptr_eq(
        &visible_before,
        &harness
            .local_fragment(tree.node(overflow).unwrap().id())
            .unwrap()
    ));
    assert_eq!(tree.get_absolute_paint_rect(host), Some(raw));
    assert_eq!(tree.cumulative_scroll_offset(leaf), (0.0, 120.0));
    let expected = RectF::new(20.0, 45.0, 70.0, 30.0);
    assert_eq!(
        tree.own_presentation_geometry(leaf, 1.0).unwrap().clip,
        expected
    );
    let mut scene = SceneGraph::new();
    scene.root = f.scene_root;
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &std::collections::HashMap::from([(surface, scene)]),
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .unwrap();
    let primitive = plan.surface_data[&surface]
        .primitives
        .iter()
        .find(|p| matches!(&p.data,PrimitiveData::Rect{color,..} if *color==leaf_color.into()))
        .unwrap();
    assert_eq!(primitive.clip_state.rect, expected);
    assert_eq!(tree.child_clip_source(visible), None);
    let viewport = RectF::new(0.0, 0.0, 120.0, 90.0);
    tree.rebuild_hit_test_grid(viewport);
    for (x, y, target) in [
        (30.0, 72.0, Some(leaf)),
        (40.0, 55.0, Some(overflow)),
        (30.0, 80.0, None),
    ] {
        let canonical = tree.hit_test_canonical(x, y);
        let accelerated = tree.hit_test(x, y);
        assert_eq!(
            canonical.as_ref().map(|h| h.widget_id),
            target.map(|id| tree.node(id).unwrap().id())
        );
        assert_eq!(
            canonical.as_ref().map(|h| h.widget_id),
            accelerated.as_ref().map(|h| h.widget_id)
        );
    }
    tree.set_overflow(visible, OverflowMode::Clip);
    frame(&mut tree, &mut harness);
    tree.rebuild_hit_test_grid(viewport);
    assert_ne!(
        tree.hit_test_canonical(40.0, 55.0).map(|h| h.widget_id),
        Some(tree.node(overflow).unwrap().id())
    );
    assert_eq!(
        tree.hit_test_canonical(40.0, 55.0).map(|h| h.widget_id),
        tree.hit_test(40.0, 55.0).map(|h| h.widget_id)
    );
}

#[test]
fn nested_button_paint_and_hit_geometry_keep_the_bottom_edge_exclusive() {
    use crate::core::event::{InteractionDomains, WindowId};
    use crate::widgets::{
        button::Button, container::Container, fixed_spacer::FixedSpacer, flex::Flex, label::Label,
    };

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column().with_spacing(0.0)), None, 0);
    tree.add_child(root, Box::new(FixedSpacer::vertical(100.0)), None, 0);
    let container = tree.add_child(root, Box::new(Container::empty()), None, 0);
    let row = tree.add_child(container, Box::new(Flex::row().with_spacing(10.0)), None, 0);
    let button = tree.add_child(row, Box::new(Label::new("placeholder")), None, 0);
    let label = tree.add_child(button, Box::new(Label::new("Click Me")), None, 0);
    tree.replace_widget_only(button, Box::new(Button::new(label).with_padding(8.0)));
    tree.replace_widget_only(container, Box::new(Container::new(row, 20.0)));
    tree.add_child(root, Box::new(FixedSpacer::vertical(50.0)), None, 0);

    let mut harness = RetainedSceneTestHarness::new();
    let frame = harness.frame_with_appearance(
        &mut tree,
        root,
        Constraints::loose(Size::new(800.0, 600.0)),
        &Theme::dark(),
        &mut FontManagerTextEngine::new(),
        &IconRegistry::new(&[]),
        &InteractionDomains::new(),
        WindowId::PRIMARY,
    );
    assert!(frame.scene_built);
    let owner = tree.node(button).unwrap().id();
    assert!(!harness.local_fragment(owner).unwrap().is_empty());

    for (node, expected) in [
        (container, (0.0, 100.0)),
        (row, (20.0, 20.0)),
        (button, (0.0, 0.0)),
    ] {
        let rect = tree.get_rect(node).unwrap();
        assert_eq!((rect.x, rect.y), expected);
    }
    let painted = tree.get_absolute_paint_rect(button).unwrap();
    assert_eq!((painted.x, painted.y), (20.0, 120.0));
    let hit = tree.hit_test(25.0, 125.0).expect("inside the mounted button");
    assert_eq!(hit.widget_id, owner);
    assert_eq!(hit.local_pos, (5.0, 5.0));
    assert_eq!(
        hit.absolute_bounds,
        (painted.x, painted.y, painted.width, painted.height)
    );
    for y in [160.0, 200.0] {
        let hit = tree.hit_test(25.0, y);
        assert!(!hit.is_some_and(|hit| hit.widget_id == owner));
    }

    let center_x = painted.x + painted.width / 2.0;
    let bottom = painted.y + painted.height;
    for (offset, expected) in [
        (-1.0, true),
        (0.0, false),
        (1.0, false),
        (2.0, false),
        (5.0, false),
    ] {
        let hit = tree.hit_test(center_x, bottom + offset);
        assert_eq!(
            hit.is_some_and(|hit| hit.widget_id == owner),
            expected,
            "button hit at bottom-edge offset {offset}"
        );
    }
}

#[test]
fn child_clip_source_transported_main_affordance_keeps_scope_and_retained_arcs() {
    workspace_preview_composition_retains_handle_output_and_real_control_order();
}

// Layout-only host plus declarative decoration; controls below it are real widgets.
struct TypographyLayout {
    id: WidgetInternalId,
}

impl WidgetDowncast for TypographyLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TypographyLayout {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        for (row, &child) in children.iter().enumerate() {
            let size = cx.layout_child(
                child,
                Constraints {
                    min_width: 0.0,
                    max_width: constraints.max_width - 40.0,
                    min_height: 0.0,
                    max_height: 60.0,
                },
            );
            cx.set_child_rect(
                child,
                RectF::new(20.0, 20.0 + row as f32 * 70.0, size.width, size.height),
            );
        }
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        cx.scene.push_rect(rect, Color::BLACK, 0, None);
    }
    fn hit_testable(&self) -> bool {
        false
    }
}

pub(super) struct UiTypographyFixture {
    pub(super) tree: WidgetTree,
    pub(super) root: NodeId,
    pub(super) label: NodeId,
    pub(super) standalone: NodeId,
    pub(super) child_button: NodeId,
    pub(super) child_label: NodeId,
    harness: RetainedSceneTestHarness,
    domains: crate::core::event::InteractionDomains,
    theme: Theme,
}

impl UiTypographyFixture {
    pub(super) fn new(
        regular: crate::core::paragraph::TextStyle,
        bold: crate::core::paragraph::TextStyle,
    ) -> Self {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(TypographyLayout {
                id: WidgetInternalId::new(),
            }),
            None,
            0,
        );
        let label = tree.add_child(
            root,
            Box::new(
                crate::widgets::label::Label::new("AV office Привет e\u{301}")
                    .with_color(Color::WHITE)
                    .with_typography(regular)
                    .unwrap(),
            ),
            None,
            0,
        );
        let standalone = tree.add_child(
            root,
            Box::new(
                crate::widgets::button::Button::with_text("AV office Привет")
                    .with_typography(bold.clone())
                    .unwrap(),
            ),
            None,
            0,
        );
        let child_button = tree.add_child(
            root,
            Box::new(crate::widgets::button::Button::with_text("")),
            None,
            0,
        );
        let child_label = tree.add_child(
            child_button,
            Box::new(
                crate::widgets::label::Label::new("AV office Привет")
                    .with_typography(bold)
                    .unwrap(),
            ),
            None,
            0,
        );
        let child = tree
            .node_mut(child_button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<crate::widgets::button::Button>()
            .unwrap();
        child.child = Some(child_label);
        child.text = None;
        tree.add_child(
            root,
            Box::new(
                crate::widgets::label::Label::new("unchanged sibling").with_color(Color::WHITE),
            ),
            None,
            0,
        );
        Self {
            tree,
            root,
            label,
            standalone,
            child_button,
            child_label,
            harness: RetainedSceneTestHarness::new(),
            domains: crate::core::event::InteractionDomains::new(),
            theme: Theme::default(),
        }
    }

    pub(super) fn frame(
        &mut self,
        text: &mut dyn crate::core::text_engine::TextEngine,
        size: Size,
    ) -> crate::core::app::RetainedSceneTestFrame {
        self.harness.frame_with_appearance(
            &mut self.tree,
            self.root,
            Constraints {
                min_width: 0.0,
                max_width: size.width,
                min_height: 0.0,
                max_height: size.height,
            },
            &self.theme,
            text,
            &IconRegistry::new(&[]),
            &self.domains,
            crate::core::event::WindowId::PRIMARY,
        )
    }

    pub(super) fn change_label_style(&mut self, style: crate::core::paragraph::TextStyle) -> bool {
        let _phase = UpdateAuthorityGuard::enter();
        let mut cx = UpdateContext::new(None);
        cx.begin_widget_update_for_owner(self.tree.widget_id_for_node(self.label).unwrap());
        let changed = self
            .tree
            .node_mut(self.label)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<crate::widgets::label::Label>()
            .unwrap()
            .set_typography(Some(style), &mut cx)
            .unwrap();
        assert_eq!(cx.take_widget_invalidations(), (changed, false, changed));
        if changed {
            self.tree.mark_layout_dirty(self.label);
            self.tree.mark_paint_dirty(self.label);
        }
        changed
    }
}

pub(super) fn ui_typography_texts(
    nodes: &[crate::core::scene::SceneNode],
) -> Vec<crate::core::scene::TextNode> {
    use crate::core::scene::SceneNode;
    let mut result = Vec::new();
    for node in nodes {
        match node {
            SceneNode::Text(text) => result.push(text.clone()),
            SceneNode::RetainedFragment(fragment) => {
                result.extend(ui_typography_texts(&fragment.children))
            }
            SceneNode::Clip(clip) => result.extend(ui_typography_texts(&clip.children)),
            SceneNode::Scroll(scroll) => result.extend(ui_typography_texts(&scroll.children)),
            SceneNode::Layer(layer) => result.extend(ui_typography_texts(&layer.children)),
            _ => {}
        }
    }
    result
}

#[cfg(target_os = "macos")]
pub(super) fn ui_typography_font_assets() -> Vec<crate::platform::font_manager::FontAsset> {
    [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    ]
    .iter()
    .map(|path| {
        let data = Arc::new(
            std::fs::read(path).expect("existing static system font required for typography proof"),
        );
        let face = ttf_parser::Face::parse(&data, 0).unwrap();
        assert!(
            face.tables().fvar.is_none(),
            "do not fake multiple weights from one variable face"
        );
        let identity =
            crate::platform::font_manager_text_engine::FontIdentity::from_font_data(&data, "Arial");
        crate::platform::font_manager::FontAsset {
            family_name: identity.family_name,
            weight: identity.weight,
            italic: identity.italic,
            data,
        }
    })
    .collect()
}

fn ui_typography_text_arc(
    nodes: &[crate::core::scene::SceneNode],
    target: &str,
) -> Option<Arc<[crate::core::scene::SceneNode]>> {
    use crate::core::scene::SceneNode;
    for node in nodes {
        match node {
            SceneNode::RetainedFragment(fragment) => {
                if fragment
                    .children
                    .iter()
                    .any(|node| matches!(node, SceneNode::Text(text) if text.text == target))
                {
                    return Some(fragment.children.clone());
                }
                if let Some(found) = ui_typography_text_arc(&fragment.children, target) {
                    return Some(found);
                }
            }
            SceneNode::Clip(clip) => {
                if let Some(found) = ui_typography_text_arc(&clip.children, target) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn ui_typography_controls_change_first_successor_and_reuse_sibling() {
    use crate::core::paragraph::TextStyle;
    let mut engine = FontManagerTextEngine::new();
    let regular = TextStyle::default()
        .with_size(22.0)
        .with_cluster_gap(2.0)
        .unwrap();
    let mut fixture = UiTypographyFixture::new(regular.clone(), regular.clone());
    let size = Size::new(720.0, 480.0);
    let first = fixture.frame(&mut engine, size);
    assert!(first.scene_built);
    let texts = ui_typography_texts(&first.scene_root);
    assert_eq!(texts.len(), 4);
    let a = fixture.tree.get_cached_size(fixture.standalone).unwrap();
    let b = fixture.tree.get_cached_size(fixture.child_button).unwrap();
    assert_eq!(
        a, b,
        "standalone and public child projection have identical requested text measurement"
    );
    let buttons: Vec<_> = texts
        .iter()
        .filter(|text| text.text == "AV office Привет")
        .collect();
    assert_eq!(buttons.len(), 2);
    assert_eq!(buttons[0].binding, buttons[1].binding);
    assert_eq!(buttons[0].font_size, 22.0);
    assert_eq!(buttons[1].font_size, 22.0);
    let sibling = ui_typography_text_arc(&first.scene_root, "unchanged sibling").unwrap();
    assert!(!fixture.frame(&mut engine, size).scene_built);
    let owner = fixture.tree.widget_id_for_node(fixture.label).unwrap();
    let next = regular
        .clone()
        .with_size(28.0)
        .with_cluster_gap(5.0)
        .unwrap();
    assert!(fixture.change_label_style(next.clone()));
    let successor = fixture.frame(&mut engine, size);
    assert!(successor.scene_built);
    assert_eq!(fixture.tree.widget_id_for_node(fixture.label), Some(owner));
    let text = ui_typography_texts(&successor.scene_root)
        .into_iter()
        .find(|text| text.text.starts_with("AV office Привет e"))
        .unwrap();
    assert_eq!(text.font_size, 28.0);
    assert_eq!(text.binding.as_ref().unwrap().cluster_gap().value(), 5.0);
    assert!(Arc::ptr_eq(
        &sibling,
        &ui_typography_text_arc(&successor.scene_root, "unchanged sibling").unwrap()
    ));
    assert!(!fixture.change_label_style(next));
    assert!(!fixture.frame(&mut engine, size).scene_built);
}

#[derive(Clone, Copy)]
enum GeometryLayoutDispatch {
    Cached,
    Full,
    TightRelayout,
    ConstrainedRelayout,
}

struct RetainedGeometryProbe {
    id: WidgetInternalId,
    size: Size,
    layouts: Rc<Cell<u32>>,
    paints: Rc<Cell<u32>>,
    requested_marker: u32,
    laid_out_marker: u32,
    sidecars: bool,
    dispatch: GeometryLayoutDispatch,
}

impl RetainedGeometryProbe {
    fn new(layouts: Rc<Cell<u32>>, paints: Rc<Cell<u32>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            size: Size::new(80.0, 40.0),
            layouts,
            paints,
            requested_marker: 0,
            laid_out_marker: 0,
            sidecars: false,
            dispatch: GeometryLayoutDispatch::Cached,
        }
    }

    fn key(&self) -> crate::core::scene_identity::SceneNodeKey {
        crate::core::scene_identity::SceneNodeKey::new(self.id.0)
    }
}

impl WidgetDowncast for RetainedGeometryProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for RetainedGeometryProbe {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        self.layouts.set(self.layouts.get() + 1);
        self.laid_out_marker = self.requested_marker + u32::from(constraints.max_width > 350.0);
        for &child in children {
            let size = Size::new(80.0, 40.0);
            let tight = Constraints::tight(size);
            let result = match self.dispatch {
                GeometryLayoutDispatch::Cached => cx.layout_child(child, tight),
                GeometryLayoutDispatch::Full => cx.layout_child_full(child, tight).size,
                GeometryLayoutDispatch::TightRelayout => {
                    cx.relayout_child_with_size(child, size);
                    cx.tree.get_cached_size(child).unwrap()
                }
                GeometryLayoutDispatch::ConstrainedRelayout => {
                    cx.relayout_child_with_constraints(child, tight);
                    cx.tree.get_cached_size(child).unwrap()
                }
            };
            cx.set_child_rect(child, RectF::new(20.0, 20.0, result.width, result.height));
        }
        LayoutResult::fit(constraints.constrain(self.size))
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        self.paints.set(self.paints.get() + 1);
        let color = if self.laid_out_marker == 0 {
            Color::from_hex(0x804000)
        } else {
            Color::from_hex(0x408000)
        };
        cx.scene.push_rect(rect, color, 2, Some(self.key()));
        if self.sidecars {
            cx.register_tooltip(
                rect,
                crate::model::tooltip::TooltipPayload::Text("geometry probe".into()),
            );
        }
    }
    fn paint_overlay(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        owner: crate::utils::id::TreeWidgetId,
    ) {
        if self.sidecars {
            cx.register_overlay_hit_region(crate::core::widget_tree::OverlayHitRegion {
                abs_rect: rect,
                owner_abs_rect: rect,
                widget_id: owner,
            });
        }
    }
}

type GeometryScenePrimitive = (
    Option<crate::core::scene_identity::SceneNodeKey>,
    RectF,
    crate::core::scene::Color,
    u32,
    Vec<(RectF, Option<crate::core::scene::RoundedCorners>)>,
    Vec<(f32, f32)>,
);

fn geometry_scene_primitives(
    nodes: &[crate::core::scene::SceneNode],
) -> Vec<GeometryScenePrimitive> {
    use crate::core::scene::SceneNode;
    fn collect(
        nodes: &[SceneNode],
        clips: &mut Vec<(RectF, Option<crate::core::scene::RoundedCorners>)>,
        scrolls: &mut Vec<(f32, f32)>,
        out: &mut Vec<GeometryScenePrimitive>,
    ) {
        for node in nodes {
            match node {
                SceneNode::RetainedFragment(fragment) => {
                    assert!(fragment.control_subtree_member.is_none());
                    collect(&fragment.children, clips, scrolls, out);
                }
                SceneNode::Clip(clip) => {
                    clips.push((clip.rect, clip.rounded));
                    collect(&clip.children, clips, scrolls, out);
                    clips.pop();
                }
                SceneNode::Scroll(scroll) => {
                    scrolls.push(scroll.offset);
                    collect(&scroll.children, clips, scrolls, out);
                    scrolls.pop();
                }
                SceneNode::Rect(rect) => out.push((
                    rect.key,
                    rect.rect,
                    rect.color,
                    rect.z_index,
                    clips.clone(),
                    scrolls.clone(),
                )),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    collect(nodes, &mut Vec::new(), &mut Vec::new(), &mut out);
    out
}

fn geometry_local_arc(
    nodes: &[crate::core::scene::SceneNode],
    key: crate::core::scene_identity::SceneNodeKey,
) -> Arc<[crate::core::scene::SceneNode]> {
    use crate::core::scene::SceneNode;
    fn find(
        nodes: &[SceneNode],
        key: crate::core::scene_identity::SceneNodeKey,
    ) -> Option<Arc<[SceneNode]>> {
        for node in nodes {
            let found =
                match node {
                    SceneNode::RetainedFragment(fragment) => {
                        if fragment.children.iter().any(
                            |node| matches!(node, SceneNode::Rect(rect) if rect.key == Some(key)),
                        ) {
                            return Some(fragment.children.clone());
                        }
                        find(&fragment.children, key)
                    }
                    SceneNode::Clip(clip) => find(&clip.children, key),
                    SceneNode::Scroll(scroll) => find(&scroll.children, key),
                    _ => None,
                };
            if found.is_some() {
                return found;
            }
        }
        None
    }
    find(nodes, key).expect("mounted probe local Arc")
}

fn geometry_probe(tree: &mut WidgetTree, node: NodeId) -> &mut RetainedGeometryProbe {
    tree.node_mut(node)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}

fn geometry_layout(tree: &mut WidgetTree, node: NodeId) -> &mut ControlOrderLayout {
    tree.node_mut(node)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}

fn geometry_assert_fresh_equivalence(
    tree: &mut WidgetTree,
    root: NodeId,
    constraints: Constraints,
    text: &mut FontManagerTextEngine,
    retained: &crate::core::app::RetainedSceneTestFrame,
) {
    fn canonical(nodes: &[crate::core::scene::SceneNode]) -> Vec<crate::core::scene::SceneNode> {
        use crate::core::scene::SceneNode;
        let mut result = Vec::new();
        for node in nodes {
            match node {
                SceneNode::RetainedFragment(fragment) => {
                    assert!(fragment.control_subtree_member.is_none());
                    result.extend(canonical(&fragment.children));
                }
                SceneNode::Clip(clip) => {
                    let mut clip = clip.clone();
                    clip.children = canonical(&clip.children);
                    result.push(SceneNode::Clip(clip));
                }
                SceneNode::Scroll(scroll) => {
                    let mut scroll = scroll.clone();
                    scroll.children = canonical(&scroll.children);
                    result.push(SceneNode::Scroll(scroll));
                }
                SceneNode::Layer(layer) => {
                    let mut layer = layer.clone();
                    layer.children = canonical(&layer.children);
                    result.push(SceneNode::Layer(layer));
                }
                other => result.push(other.clone()),
            }
        }
        result
    }
    let fresh = RetainedSceneTestHarness::new().frame(
        tree,
        root,
        constraints,
        &Theme::default(),
        text,
        &IconRegistry::new(&[]),
    );
    assert_eq!(
        geometry_scene_primitives(&retained.scene_root),
        geometry_scene_primitives(&fresh.scene_root)
    );
    assert_eq!(
        format!("{:?}", canonical(&retained.scene_root)),
        format!("{:?}", canonical(&fresh.scene_root))
    );
    let sidecars = |frame: &crate::core::app::RetainedSceneTestFrame| {
        frame
            .tooltip_triggers
            .iter()
            .map(|trigger| {
                (
                    trigger.widget_id,
                    trigger.trigger_rect,
                    trigger.content.clone(),
                    trigger.placement,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(sidecars(retained), sidecars(&fresh));
}

#[test]
fn ui_typography_geometry_moved_skipped_subtree_updates_paint_and_sidecars() {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(ControlOrderLayout::new(
            Size::new(320.0, 180.0),
            vec![
                RectF::new(20.0, 20.0, 100.0, 80.0),
                RectF::new(220.0, 20.0, 80.0, 40.0),
            ],
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        )),
        None,
        0,
    );
    let branch_layouts = Rc::new(Cell::new(0));
    let branch = tree.add_child(
        root,
        Box::new(ControlOrderLayout::new(
            Size::new(100.0, 80.0),
            vec![RectF::new(0.0, 0.0, 80.0, 40.0)],
            branch_layouts.clone(),
            Rc::new(Cell::new(0)),
        )),
        None,
        0,
    );
    let layouts = Rc::new(Cell::new(0));
    let paints = Rc::new(Cell::new(0));
    let mut probe = RetainedGeometryProbe::new(layouts.clone(), paints.clone());
    probe.sidecars = true;
    let key = probe.key();
    let child = tree.add_child(branch, Box::new(probe), None, 0);
    let sibling_paints = Rc::new(Cell::new(0));
    let sibling_layouts = Rc::new(Cell::new(0));
    let sibling = RetainedGeometryProbe::new(sibling_layouts.clone(), sibling_paints.clone());
    let sibling_key = sibling.key();
    tree.add_child(root, Box::new(sibling), None, 0);
    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let mut harness = RetainedSceneTestHarness::new();
    let mut text = FontManagerTextEngine::new();
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let sibling_arc = geometry_local_arc(&first.scene_root, sibling_key);
    let child_arc = geometry_local_arc(&first.scene_root, key);
    let owner = tree.widget_id_for_node(child).unwrap();
    assert_eq!(
        first.tooltip_triggers[0].trigger_rect,
        RectF::new(20.0, 20.0, 80.0, 40.0)
    );
    {
        let _update = UpdateAuthorityGuard::enter();
        geometry_layout(&mut tree, root).slots[0].x = 100.0;
        tree.mark_layout_dirty(root);
    }
    let moved = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(moved.scene_built);
    assert_eq!(branch_layouts.get(), 1);
    assert_eq!(layouts.get(), 1);
    assert_eq!(paints.get(), 2);
    assert_eq!(sibling_layouts.get(), 1);
    assert_eq!(sibling_paints.get(), 1);
    assert!(Arc::ptr_eq(
        &sibling_arc,
        &geometry_local_arc(&moved.scene_root, sibling_key)
    ));
    assert!(!Arc::ptr_eq(
        &child_arc,
        &geometry_local_arc(&moved.scene_root, key)
    ));
    assert_eq!(
        tree.get_absolute_paint_rect(child),
        Some(RectF::new(100.0, 20.0, 80.0, 40.0))
    );
    assert_eq!(
        tree.hit_test_canonical(110.0, 30.0).unwrap().widget_id,
        owner
    );
    let (hit, origin) = tree.hit_test_for_routing(110.0, 30.0).unwrap();
    assert_eq!(hit.widget_id, owner);
    assert!(matches!(
        origin,
        crate::core::event::PointerRouteOrigin::Overlay
    ));
    assert!(tree.hit_test(30.0, 30.0).is_none());
    assert_eq!(moved.tooltip_triggers[0].widget_id, owner);
    assert_eq!(
        moved.tooltip_triggers[0].trigger_rect,
        tree.get_absolute_paint_rect(child).unwrap()
    );
    assert_eq!(
        geometry_scene_primitives(&moved.scene_root)
            .iter()
            .find(|p| p.0 == Some(key))
            .unwrap()
            .1,
        tree.get_absolute_paint_rect(child).unwrap()
    );
    geometry_assert_fresh_equivalence(&mut tree, root, constraints, &mut text, &moved);
    assert!(
        !harness
            .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
            .scene_built
    );
}

#[test]
fn ui_typography_geometry_overflow_setter_updates_clean_tree() {
    use crate::core::widget_tree::OverflowMode;
    let mut tree = WidgetTree::new();
    let mut root_slots = vec![
        RectF::new(20.0, 20.0, 100.0, 80.0),
        RectF::new(220.0, 20.0, 80.0, 40.0),
        RectF::new(340.0, 20.0, 20.0, 20.0),
        RectF::new(120.0, 110.0, 60.0, 60.0),
        RectF::new(220.0, 100.0, 80.0, 60.0),
    ];
    root_slots.extend((0..130).map(|i| RectF::new(400.0, i as f32, 10.0, 10.0)));
    let root = tree.create_root(
        Box::new(ControlOrderLayout::new(
            Size::new(320.0, 180.0),
            root_slots,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        )),
        None,
        0,
    );
    let mut branch_layout = ControlOrderLayout::new(
        Size::new(100.0, 80.0),
        vec![RectF::new(70.0, 20.0, 80.0, 40.0)],
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
    );
    branch_layout.hit_target = true;
    let branch = tree.add_child(root, Box::new(branch_layout), None, 0);
    let layouts = Rc::new(Cell::new(0));
    let paints = Rc::new(Cell::new(0));
    let probe = RetainedGeometryProbe::new(layouts.clone(), paints.clone());
    let key = probe.key();
    let child = tree.add_child(branch, Box::new(probe), None, 0);
    let sibling_layouts = Rc::new(Cell::new(0));
    let sibling_paints = Rc::new(Cell::new(0));
    let sibling = RetainedGeometryProbe::new(sibling_layouts.clone(), sibling_paints.clone());
    let sibling_key = sibling.key();
    let sibling_node = tree.add_child(root, Box::new(sibling), None, 0);

    // This Visible ancestor is wholly outside the root, unlike its children.
    let outside = tree.add_child(
        root,
        Box::new(ControlOrderLayout::new(
            Size::new(20.0, 20.0),
            vec![RectF::new(-320.0, 100.0, 80.0, 40.0); 2],
            Rc::default(),
            Rc::default(),
        )),
        None,
        0,
    );
    tree.add_child(
        outside,
        Box::new(RetainedGeometryProbe::new(Rc::default(), Rc::default())),
        None,
        0,
    );
    let front = tree.add_child(
        outside,
        Box::new(RetainedGeometryProbe::new(Rc::default(), Rc::default())),
        None,
        1,
    );

    let mut clipping_layout = ControlOrderLayout::new(
        Size::new(60.0, 60.0),
        vec![RectF::new(40.0, 10.0, 80.0, 40.0)],
        Rc::default(),
        Rc::default(),
    );
    clipping_layout.clip = true;
    let clipping = tree.add_child(root, Box::new(clipping_layout), None, 0);
    let clipped_child = tree.add_child(
        clipping,
        Box::new(RetainedGeometryProbe::new(Rc::default(), Rc::default())),
        None,
        0,
    );

    let mut scroll_layout = ControlOrderLayout::new(
        Size::new(80.0, 60.0),
        vec![RectF::new(0.0, 0.0, 160.0, 60.0)],
        Rc::default(),
        Rc::default(),
    );
    scroll_layout.scroll = Some((16.0, 0.0));
    let scrollport = tree.add_child(root, Box::new(scroll_layout), None, 0);
    let scroll_content = tree.add_child(
        scrollport,
        Box::new(ControlOrderLayout::new(
            Size::new(160.0, 60.0),
            vec![RectF::new(0.0, 10.0, 160.0, 40.0)],
            Rc::default(),
            Rc::default(),
        )),
        None,
        0,
    );
    let scrolled_probe = RetainedGeometryProbe::new(Rc::default(), Rc::default());
    let scrolled_key = scrolled_probe.key();
    let scrolled_child = tree.add_child(scroll_content, Box::new(scrolled_probe), None, 0);
    for _ in 0..130 {
        tree.add_child(
            root,
            Box::new(RetainedGeometryProbe::new(Rc::default(), Rc::default())),
            None,
            0,
        );
    }
    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let mut harness = RetainedSceneTestHarness::new();
    let mut text = FontManagerTextEngine::new();
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let sibling_arc = geometry_local_arc(&first.scene_root, sibling_key);
    let owner = tree.widget_id_for_node(child).unwrap();
    let sibling_owner = tree.widget_id_for_node(sibling_node).unwrap();
    let branch_owner = tree.widget_id_for_node(branch).unwrap();
    let front_owner = tree.widget_id_for_node(front).unwrap();
    let clipped_owner = tree.widget_id_for_node(clipped_child).unwrap();
    let scrolled_owner = tree.widget_id_for_node(scrolled_child).unwrap();
    let rect = RectF::new(90.0, 40.0, 80.0, 40.0);
    assert_eq!(tree.get_absolute_paint_rect(child), Some(rect));
    assert_eq!(tree.get_viewport(child), Some(rect));
    assert_eq!(tree.get_viewport(outside), Some(RectF::zero()));
    assert_eq!(
        tree.get_absolute_paint_rect(front),
        Some(RectF::new(20.0, 120.0, 80.0, 40.0))
    );
    assert!(tree.node_count() > 128);
    let viewport = RectF::new(0.0, 0.0, 320.0, 180.0);
    tree.rebuild_hit_test_grid(viewport);
    assert!(tree.has_current_hit_test_grid());
    assert_eq!(tree.hit_test(150.0, 60.0).unwrap().widget_id, owner);
    let compare = |tree: &WidgetTree, x, y, expected| {
        assert!(tree.has_current_hit_test_grid());
        let accelerated = tree.hit_test(x, y);
        let canonical = tree.hit_test_canonical(x, y);
        assert_eq!(
            accelerated
                .as_ref()
                .map(|hit| (hit.widget_id, hit.local_pos, hit.absolute_bounds)),
            canonical
                .as_ref()
                .map(|hit| (hit.widget_id, hit.local_pos, hit.absolute_bounds))
        );
        assert_eq!(accelerated.map(|hit| hit.widget_id), expected);
    };
    let check_boundaries = |tree: &WidgetTree| {
        compare(tree, 40.0, 30.0, Some(branch_owner));
        compare(tree, 180.0, 60.0, None);
        compare(tree, 40.0, 140.0, Some(front_owner));
        compare(tree, 170.0, 130.0, Some(clipped_owner));
        compare(tree, 190.0, 130.0, None);
        compare(tree, 260.0, 130.0, Some(scrolled_owner));
        compare(tree, 310.0, 130.0, None);
        compare(tree, -1.0, 60.0, None);
        compare(tree, 330.0, 60.0, None);
        let hit = tree.hit_test(40.0, 140.0).unwrap();
        assert_eq!(hit.local_pos, (20.0, 20.0));
        assert_eq!(hit.absolute_bounds, (20.0, 120.0, 80.0, 40.0));
        let hit = tree.hit_test(260.0, 130.0).unwrap();
        assert_eq!(hit.local_pos, (56.0, 20.0));
        assert_eq!(hit.absolute_bounds, (220.0, 110.0, 160.0, 40.0));
    };
    check_boundaries(&tree);
    let primitive = geometry_scene_primitives(&first.scene_root)
        .into_iter()
        .find(|primitive| primitive.0 == Some(scrolled_key))
        .unwrap();
    assert_eq!(primitive.5, vec![(16.0, 0.0)]);
    assert!(
        primitive
            .4
            .contains(&(RectF::new(220.0, 100.0, 80.0, 60.0), None))
    );
    compare(&tree, 110.0, 60.0, Some(owner));
    compare(&tree, 150.0, 60.0, Some(owner));
    let hit = tree.hit_test(150.0, 60.0).unwrap();
    assert_eq!(hit.local_pos, (60.0, 20.0));
    assert_eq!(hit.absolute_bounds, (90.0, 40.0, 80.0, 40.0));
    assert!(!tree.node(root).unwrap().dirty_layout);
    let original_geometry = [root, branch, child, sibling_node].map(|node| {
        (
            tree.get_rect(node),
            tree.get_absolute_paint_rect(node),
            tree.get_cached_size(node),
            tree.node(node).unwrap().last_constraints,
        )
    });

    for (step, overflow) in [OverflowMode::Clip, OverflowMode::Visible]
        .into_iter()
        .enumerate()
    {
        {
            let _update = UpdateAuthorityGuard::enter();
            tree.set_overflow(branch, overflow);
        }
        let changed = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(changed.scene_built);
        assert!(!tree.node(root).unwrap().dirty_layout);
        assert_eq!(
            [root, branch, child, sibling_node].map(|node| {
                (
                    tree.get_rect(node),
                    tree.get_absolute_paint_rect(node),
                    tree.get_cached_size(node),
                    tree.node(node).unwrap().last_constraints,
                )
            }),
            original_geometry
        );
        let viewport = if overflow == OverflowMode::Clip {
            RectF::new(90.0, 40.0, 30.0, 40.0)
        } else {
            rect
        };
        assert_eq!(tree.get_viewport(child), Some(viewport));
        tree.rebuild_hit_test_grid(RectF::new(0.0, 0.0, 320.0, 180.0));
        assert!(tree.has_current_hit_test_grid());
        assert_eq!(tree.hit_test(110.0, 60.0).unwrap().widget_id, owner);
        assert_eq!(
            tree.hit_test(150.0, 60.0).map(|hit| hit.widget_id),
            (overflow == OverflowMode::Visible).then_some(owner)
        );
        compare(&tree, 110.0, 60.0, Some(owner));
        compare(
            &tree,
            150.0,
            60.0,
            (overflow == OverflowMode::Visible).then_some(owner),
        );
        check_boundaries(&tree);
        assert_eq!(
            tree.hit_test_canonical(150.0, 60.0)
                .map(|hit| hit.widget_id),
            (overflow == OverflowMode::Visible).then_some(owner)
        );
        let primitive = geometry_scene_primitives(&changed.scene_root)
            .into_iter()
            .find(|primitive| primitive.0 == Some(key))
            .unwrap();
        assert_eq!(primitive.1, rect);
        assert_eq!(
            primitive
                .4
                .contains(&(RectF::new(20.0, 20.0, 100.0, 80.0), None)),
            overflow == OverflowMode::Clip
        );
        assert_eq!(layouts.get(), 1, "the child remains a skipped layout");
        assert_eq!(paints.get(), step as u32 + 2);
        assert_eq!(sibling_layouts.get(), 1);
        assert_eq!(sibling_paints.get(), 1);
        assert!(Arc::ptr_eq(
            &sibling_arc,
            &geometry_local_arc(&changed.scene_root, sibling_key)
        ));
        let child_arc = geometry_local_arc(&changed.scene_root, key);
        assert!(Arc::ptr_eq(
            &child_arc,
            &harness
                .local_fragment(owner)
                .expect("mounted child local Arc")
        ));
        assert!(Arc::ptr_eq(
            &sibling_arc,
            &harness
                .local_fragment(sibling_owner)
                .expect("mounted sibling local Arc")
        ));
        {
            let _update = UpdateAuthorityGuard::enter();
            tree.set_overflow(branch, overflow);
        }
        assert!(!tree.node(root).unwrap().dirty_layout);
        tree.rebuild_hit_test_grid(RectF::new(0.0, 0.0, 320.0, 180.0));
        assert!(tree.has_current_hit_test_grid());
        let unchanged = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(!unchanged.scene_built);
        assert!(unchanged.scene_root.is_empty());
        tree.rebuild_hit_test_grid(RectF::new(0.0, 0.0, 320.0, 180.0));
        assert!(tree.has_current_hit_test_grid());
        compare(&tree, 110.0, 60.0, Some(owner));
        compare(
            &tree,
            150.0,
            60.0,
            (overflow == OverflowMode::Visible).then_some(owner),
        );
        assert_eq!(paints.get(), step as u32 + 2);
        assert_eq!(sibling_paints.get(), 1);
        assert!(Arc::ptr_eq(
            &child_arc,
            &harness
                .local_fragment(owner)
                .expect("mounted child local Arc")
        ));
        assert!(Arc::ptr_eq(
            &sibling_arc,
            &harness
                .local_fragment(sibling_owner)
                .expect("mounted sibling local Arc")
        ));
    }
}

#[test]
fn analytic_surface_actual_retained_clip_change_material_idle_and_unmount() {
    use crate::compositor::{Compositor, SceneKey};
    use crate::core::paint::{RoundedCorners, SurfaceAnalytic, analytic_surface_tests::fixture};
    use crate::core::scene::{SceneBuilder, SceneGraph, SceneNode};
    use crate::core::scene_identity::SceneNodeKey;
    use crate::renderer::{
        DrawPlanBuilder, draw_plan_builder::DrawPlanBuildError, mesh_cache::MeshCache,
    };
    use std::collections::HashMap;
    struct Face {
        id: WidgetInternalId,
        paints: Rc<Cell<u32>>,
        material: SurfaceAnalytic,
        key: SceneNodeKey,
    }
    impl WidgetDowncast for Face {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }
    impl Widget for Face {
        fn id(&self) -> WidgetInternalId {
            self.id
        }
        fn hit_testable(&self) -> bool {
            false
        }
        fn layout(&mut self, _: &mut LayoutContext, c: Constraints, _: &[NodeId]) -> LayoutResult {
            LayoutResult::fit(c.constrain(Size::new(40.0, 30.0)))
        }
        fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
            self.paints.set(self.paints.get() + 1);
            cx.scene
                .push_analytic_surface(
                    rect,
                    RoundedCorners::uniform(8.0),
                    &self.material,
                    1,
                    Some(self.key),
                )
                .unwrap();
        }
    }
    let size = Size::new(120.0, 100.0);
    let bounds = RectF::new(0.0, 0.0, 120.0, 100.0);
    let mut host = ControlOrderLayout::new(
        size,
        vec![RectF::new(20.0, 20.0, 40.0, 30.0)],
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
    );
    host.geometry_clip = Some(GeometryClipState {
        bounds,
        rounded: None,
        pending: None,
        updates: 0,
    });
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(host), None, 0);
    let paints = Rc::new(Cell::new(0));
    let key = SceneNodeKey::new(386);
    let child = tree.add_child(
        root,
        Box::new(Face {
            id: WidgetInternalId::new(),
            paints: paints.clone(),
            material: fixture(1.0, 0.2, 0.6),
            key,
        }),
        None,
        0,
    );
    let owner = tree.widget_id_for_node(child).unwrap();
    let mut harness = RetainedSceneTestHarness::new();
    let mut text = FontManagerTextEngine::new();
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let constraints = Constraints::tight(size);
    let mut compositor = Compositor::new(bounds);
    let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
    let mut builder = DrawPlanBuilder::new();
    let mut mesh = MeshCache::new();
    let build = |builder: &mut DrawPlanBuilder, mesh: &mut MeshCache, nodes: Vec<SceneNode>| {
        let mut scene = SceneGraph::new();
        scene.root = nodes;
        builder.build_multi(
            &HashMap::from([(surface, scene)]),
            compositor.snapshot_multi(),
            mesh,
            |_| 1.0,
            0,
        )
    };
    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let local = harness.local_fragment(owner).unwrap();
    assert!(matches!(local.as_ref(), [SceneNode::AnalyticSurface(_)]));
    assert!(build(&mut builder, &mut mesh, first.scene_root).is_ok());
    for rounded in [
        Some(RoundedCorners::uniform(12.0)),
        Some(RoundedCorners::uniform(0.0)),
        None,
    ] {
        {
            let _update = UpdateAuthorityGuard::enter();
            geometry_layout(&mut tree, root)
                .geometry_clip
                .as_mut()
                .unwrap()
                .rounded = rounded;
            tree.mark_layout_dirty(root);
        }
        let frame = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert_eq!(paints.get(), 1);
        assert!(Arc::ptr_eq(&local, &harness.local_fragment(owner).unwrap()));
        assert!(build(&mut builder, &mut mesh, frame.scene_root.clone()).is_ok());
        if rounded.is_some_and(|r| r.tl > 0.0) {
            let mut parent = SceneBuilder::new();
            parent.begin_clip(
                bounds,
                Some(crate::core::scene::RoundedCorners::uniform(4.0)),
            );
            parent.push_retained_fragment(Arc::from(frame.scene_root));
            parent.end_clip();
            assert!(matches!(
                build(&mut builder, &mut mesh, parent.build().root),
                Err(DrawPlanBuildError::UnsupportedAnalyticSurfaceClip {
                    rounded_clip_depth: 2,
                    ..
                })
            ));
        }
    }
    assert!(
        !harness
            .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
            .scene_built
    );
    assert_eq!(
        paints.get(),
        1,
        "static material adds no update demand or idle Paint"
    );
    {
        let _update = UpdateAuthorityGuard::enter();
        tree.node_mut(child)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Face>()
            .unwrap()
            .material = fixture(1.0, 0.0, -0.6);
        tree.mark_paint_dirty(child);
    }
    let changed = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(changed.scene_built);
    assert_eq!(paints.get(), 2);
    assert!(!Arc::ptr_eq(
        &local,
        &harness.local_fragment(owner).unwrap()
    ));
    tree.rebuild_hit_test_grid(bounds);
    assert!(
        tree.hit_test(35.0, 35.0).is_none(),
        "paint material has no input owner"
    );
    {
        let _update = UpdateAuthorityGuard::enter();
        tree.remove_node(child);
        geometry_layout(&mut tree, root).slots.clear();
        tree.mark_layout_dirty(root);
    }
    let removed = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(harness.local_fragment(owner).is_none());
    assert!(
        build(&mut builder, &mut mesh, removed.scene_root)
            .unwrap()
            .surface_scene(surface)
            .unwrap()
            .primitives
            .is_empty()
    );
}

#[test]
fn analytic_surface_micrograin_retained_identity_motion_idle_and_unmount() {
    use crate::core::paint::{
        RoundedCorners, SurfaceAnalytic, SurfaceMicrograin, analytic_surface_tests::fixture,
    };
    use crate::core::scene::SceneNode;
    struct Face {
        id: WidgetInternalId,
        material: SurfaceAnalytic,
        paints: Rc<Cell<u32>>,
    }
    impl WidgetDowncast for Face {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }
    impl Widget for Face {
        fn id(&self) -> WidgetInternalId {
            self.id
        }
        fn hit_testable(&self) -> bool {
            false
        }
        fn layout(&mut self, _: &mut LayoutContext, c: Constraints, _: &[NodeId]) -> LayoutResult {
            LayoutResult::fit(c.constrain(Size::new(40.0, 30.0)))
        }
        fn paint(&self, cx: &mut PaintContext, r: RectF, _: &[NodeId]) {
            self.paints.set(self.paints.get() + 1);
            cx.scene
                .push_analytic_surface(r, RoundedCorners::uniform(8.0), &self.material, 1, None)
                .unwrap();
        }
    }
    let size = Size::new(180.0, 120.0);
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(ControlOrderLayout::new(
            size,
            vec![
                RectF::new(10.0, 10.0, 40.0, 30.0),
                RectF::new(65.0, 10.0, 100.0, 30.0),
            ],
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        )),
        None,
        0,
    );
    let original = fixture(1.0, 0.2, 0.6);
    let paints = Rc::new(Cell::new(0));
    let child = tree.add_child(
        root,
        Box::new(Face {
            id: WidgetInternalId::new(),
            material: original.clone(),
            paints: paints.clone(),
        }),
        None,
        0,
    );
    let caption = tree.add_child(
        root,
        Box::new(crate::widgets::label::Label::new("Sharp text")),
        None,
        0,
    );
    let owner = tree.widget_id_for_node(child).unwrap();
    let caption_owner = tree.widget_id_for_node(caption).unwrap();
    let mut harness = RetainedSceneTestHarness::new();
    let mut text = FontManagerTextEngine::new();
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let constraints = Constraints::tight(size);
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let caption_fragment = harness.local_fragment(caption_owner).unwrap();
    let first = harness.local_fragment(owner).unwrap();
    let zero = original.clone().with_micrograin(Some(
        SurfaceMicrograin::try_new(3.0, 0.0, u32::MAX).unwrap(),
    ));
    assert_eq!(
        zero, original,
        "zero is not a material change requiring invalidation"
    );
    assert!(
        !harness
            .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
            .scene_built
    );
    assert!(Arc::ptr_eq(&first, &harness.local_fragment(owner).unwrap()));
    let changes = [
        Some((0.75, 2.0 / 255.0, 387)),
        Some((1.5, 2.0 / 255.0, 387)),
        Some((1.5, 1.0 / 255.0, 387)),
        Some((1.5, 1.0 / 255.0, 305419896)),
        None,
    ];
    let mut materials: Vec<_> = changes.into_iter().map(|value| original.clone().with_micrograin(
            value.map(|(s, a, seed)| SurfaceMicrograin::try_new(s, a, seed).unwrap()),
        )).collect();
    use crate::core::paint::analytic_surface_tests::{coating,emission};
    materials.extend([
        original.clone().with_glass_optics(Some(coating(0.6))),
        original.clone().with_glass_optics(Some(coating(-0.6))).with_surface_emission(Some(emission())),
        fixture(0.0,0.0,0.6).with_surface_emission(Some(emission())),
        original.clone(),
    ]);
    for (i, material) in materials.into_iter().enumerate() {
        let before = harness.local_fragment(owner).unwrap();
        {
            let _update = UpdateAuthorityGuard::enter();
            let face = tree
                .node_mut(child)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Face>()
                .unwrap();
            assert_ne!(face.material, material);
            face.material = material.clone();
            tree.mark_paint_dirty(child);
        }
        assert!(
            harness
                .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
                .scene_built
        );
        let after = harness.local_fragment(owner).unwrap();
        assert!(!Arc::ptr_eq(&before, &after));
        let [SceneNode::AnalyticSurface(node)] = after.as_ref() else {
            panic!("ordinary retained analytic node")
        };
        assert_eq!(node.material(), &material);
        assert_eq!(node.rect(),RectF::new(10.0,10.0,40.0,30.0),"halo changes raster support, never canonical geometry");
        assert_eq!(paints.get(), i as u32 + 2);
        assert!(
            Arc::ptr_eq(
                &caption_fragment,
                &harness.local_fragment(caption_owner).unwrap()
            ),
            "grain changes only the face fragment, not sharp foreground text"
        );
        assert!(
            !harness
                .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
                .scene_built
        );
    }
    {
        let _update = UpdateAuthorityGuard::enter();
        geometry_layout(&mut tree, root).slots[0] = RectF::new(13.375, 12.625, 49.0, 35.0);
        tree.mark_layout_dirty(root);
    }
    assert!(
        harness
            .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
            .scene_built
    );
    assert!(Arc::ptr_eq(
        &caption_fragment,
        &harness.local_fragment(caption_owner).unwrap()
    ));
    {
        let _update = UpdateAuthorityGuard::enter();
        tree.remove_node(child);
        geometry_layout(&mut tree, root).slots.remove(0);
        tree.mark_layout_dirty(root);
    }
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(harness.local_fragment(owner).is_none());
    assert!(
        !harness
            .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
            .scene_built
    );
}

#[test]
fn local_ambient_glow_actual_retained_clip_change_idle_and_unmount() {
    use crate::compositor::{Compositor, SceneKey};
    use crate::core::math::Point;
    use crate::core::paint::{LocalAmbientGlowAppearance, LocalGlowLobe, RoundedCorners};
    use crate::core::scene::{SceneGraph, SceneNode};
    use crate::core::scene_identity::SceneNodeKey;
    use crate::renderer::draw_plan_builder::{DrawPlanBuildError, DrawPlanBuilder};
    use crate::renderer::mesh_cache::MeshCache;
    use std::collections::HashMap;

    struct Glow {
        id: WidgetInternalId,
        paints: Rc<Cell<u32>>,
        appearance: LocalAmbientGlowAppearance,
        key: SceneNodeKey,
    }
    impl WidgetDowncast for Glow {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }
    impl Widget for Glow {
        fn id(&self) -> WidgetInternalId {
            self.id
        }
        fn hit_testable(&self) -> bool {
            false
        }
        fn layout(&mut self, _: &mut LayoutContext, c: Constraints, _: &[NodeId]) -> LayoutResult {
            LayoutResult::fit(c.constrain(Size::new(40.0, 30.0)))
        }
        fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
            self.paints.set(self.paints.get() + 1);
            cx.scene
                .push_local_ambient_glow(rect, self.appearance, 1, Some(self.key))
                .unwrap();
        }
    }
    let size = Size::new(120.0, 100.0);
    let bounds = RectF::new(0.0, 0.0, size.width, size.height);
    let mut host = ControlOrderLayout::new(
        size,
        vec![RectF::new(20.0, 20.0, 40.0, 30.0)],
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
    );
    host.geometry_clip = Some(GeometryClipState {
        bounds,
        rounded: None,
        pending: None,
        updates: 0,
    });
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(host), None, 0);
    let paints = Rc::new(Cell::new(0));
    let key = SceneNodeKey::new(388);
    let appearance = LocalAmbientGlowAppearance::try_new(&[LocalGlowLobe::try_new(
        Point::new(0.5, 1.0),
        Point::new(20.0, 15.0),
        Point::new(1.0, 0.0),
        Color::WHITE,
        0.8,
    )
    .unwrap()])
    .unwrap();
    let child = tree.add_child(
        root,
        Box::new(Glow {
            id: WidgetInternalId::new(),
            paints: paints.clone(),
            appearance,
            key,
        }),
        None,
        0,
    );
    let owner = tree.widget_id_for_node(child).unwrap();
    let mut harness = RetainedSceneTestHarness::new();
    let mut text = FontManagerTextEngine::new();
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let constraints = Constraints::tight(size);
    let mut compositor = Compositor::new(bounds);
    let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
    let mut builder = DrawPlanBuilder::new();
    let mut mesh = MeshCache::new();
    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let local = harness.local_fragment(owner).unwrap();
    assert!(matches!(local.as_ref(), [SceneNode::LocalAmbientGlow(_)]));
    let build = |builder: &mut DrawPlanBuilder, mesh: &mut MeshCache, nodes: Vec<SceneNode>| {
        let mut scene = SceneGraph::new();
        scene.root = nodes;
        builder.build_multi(
            &HashMap::from([(surface, scene)]),
            compositor.snapshot_multi(),
            mesh,
            |_| 1.0,
            0,
        )
    };
    assert!(build(&mut builder, &mut mesh, first.scene_root).is_ok());
    for (rounded, refused) in [
        (Some(RoundedCorners::uniform(12.0)), true),
        (Some(RoundedCorners::uniform(0.0)), false),
        (None, false),
    ] {
        {
            let _update = UpdateAuthorityGuard::enter();
            geometry_layout(&mut tree, root)
                .geometry_clip
                .as_mut()
                .unwrap()
                .rounded = rounded;
            tree.mark_layout_dirty(root);
        }
        let frame = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert_eq!(
            paints.get(),
            1,
            "clip-only composition must reuse leaf Paint"
        );
        assert!(Arc::ptr_eq(&local, &harness.local_fragment(owner).unwrap()));
        let result = build(&mut builder, &mut mesh, frame.scene_root);
        if refused {
            assert!(
                matches!(result, Err(DrawPlanBuildError::UnsupportedLocalAmbientGlowClip { surface_id, node_key, rounded_clip_depth: 1 }) if surface_id == surface && node_key == Some(key))
            );
        } else {
            assert_eq!(
                result
                    .unwrap()
                    .surface_scene(surface)
                    .unwrap()
                    .primitives
                    .len(),
                1
            );
        }
    }
    tree.rebuild_hit_test_grid(bounds);
    assert!(
        tree.hit_test(40.0, 55.0).is_none(),
        "halo creates no input owner"
    );
    assert!(
        !harness
            .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
            .scene_built
    );
    assert_eq!(paints.get(), 1, "static glow adds no idle Paint");
    {
        let _update = UpdateAuthorityGuard::enter();
        tree.remove_node(child);
        geometry_layout(&mut tree, root).slots.clear();
        tree.mark_layout_dirty(root);
    }
    let removed = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(harness.local_fragment(owner).is_none());
    assert!(
        build(&mut builder, &mut mesh, removed.scene_root)
            .unwrap()
            .surface_scene(surface)
            .unwrap()
            .primitives
            .is_empty()
    );
}

#[test]
fn ui_typography_geometry_inherited_viewport_and_clip_update_first_successor() {
    use crate::core::widget_tree::OverflowMode;
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(ControlOrderLayout::new(
            Size::new(320.0, 180.0),
            vec![
                RectF::new(20.0, 20.0, 100.0, 80.0),
                RectF::new(220.0, 20.0, 80.0, 40.0),
            ],
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        )),
        None,
        0,
    );
    let branch = tree.add_child(
        root,
        Box::new(ControlOrderLayout::new(
            Size::new(100.0, 80.0),
            vec![RectF::new(0.0, 0.0, 80.0, 40.0)],
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        )),
        None,
        0,
    );
    let layouts = Rc::new(Cell::new(0));
    let paints = Rc::new(Cell::new(0));
    let probe = RetainedGeometryProbe::new(layouts.clone(), paints.clone());
    let key = probe.key();
    let child = tree.add_child(branch, Box::new(probe), None, 0);
    let sibling_paints = Rc::new(Cell::new(0));
    let sibling = RetainedGeometryProbe::new(Rc::new(Cell::new(0)), sibling_paints.clone());
    let sibling_key = sibling.key();
    tree.add_child(root, Box::new(sibling), None, 0);
    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let mut harness = RetainedSceneTestHarness::new();
    let mut text = FontManagerTextEngine::new();
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let sibling_arc = geometry_local_arc(&first.scene_root, sibling_key);
    let rect = tree.get_absolute_paint_rect(child).unwrap();
    {
        let _update = UpdateAuthorityGuard::enter();
        geometry_layout(&mut tree, root).slots[0].width = 60.0;
        tree.set_overflow(branch, OverflowMode::Clip);
        tree.mark_layout_dirty(root);
    }
    let clipped = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert_eq!(tree.get_absolute_paint_rect(child), Some(rect));
    assert_eq!(
        tree.get_viewport(child),
        Some(RectF::new(20.0, 20.0, 60.0, 40.0))
    );
    assert_eq!(layouts.get(), 1);
    assert_eq!(
        paints.get(),
        2,
        "changed inherited viewport repaints the skipped leaf"
    );
    assert_eq!(sibling_paints.get(), 1);
    assert!(Arc::ptr_eq(
        &sibling_arc,
        &geometry_local_arc(&clipped.scene_root, sibling_key)
    ));
    let child_arc = geometry_local_arc(&clipped.scene_root, key);
    {
        let _update = UpdateAuthorityGuard::enter();
        geometry_layout(&mut tree, branch).geometry_clip = Some(GeometryClipState {
            bounds: RectF::new(0.0, 0.0, 30.0, 80.0),
            rounded: Some(crate::core::paint::RoundedCorners::uniform(8.0)),
            pending: None,
            updates: 0,
        });
        tree.mark_layout_dirty(branch);
    }
    let rounded = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert_eq!(tree.get_absolute_paint_rect(child), Some(rect));
    assert_eq!(tree.get_cached_size(child), Some(rect.size()));
    assert_eq!(layouts.get(), 1);
    assert_eq!(paints.get(), 2);
    assert!(Arc::ptr_eq(
        &child_arc,
        &geometry_local_arc(&rounded.scene_root, key)
    ));
    assert!(Arc::ptr_eq(
        &sibling_arc,
        &geometry_local_arc(&rounded.scene_root, sibling_key)
    ));
    let primitive = geometry_scene_primitives(&rounded.scene_root)
        .into_iter()
        .find(|p| p.0 == Some(key))
        .unwrap();
    assert!(primitive.4.contains(&(
        RectF::new(20.0, 20.0, 30.0, 80.0),
        Some(crate::core::scene::RoundedCorners::uniform(8.0))
    )));
    geometry_assert_fresh_equivalence(&mut tree, root, constraints, &mut text, &rounded);
    assert!(
        !harness
            .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
            .scene_built
    );
}

#[test]
fn ui_typography_geometry_executed_same_size_owner_repaints() {
    let mut tree = WidgetTree::new();
    let layouts = Rc::new(Cell::new(0));
    let paints = Rc::new(Cell::new(0));
    let root_probe = RetainedGeometryProbe::new(layouts.clone(), paints.clone());
    let key = root_probe.key();
    let root = tree.create_root(Box::new(root_probe), None, 0);
    let mut harness = RetainedSceneTestHarness::new();
    let mut text = FontManagerTextEngine::new();
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let constraints = |width| Constraints {
        min_width: 0.0,
        max_width: width,
        min_height: 0.0,
        max_height: 180.0,
    };
    let first = harness.frame(
        &mut tree,
        root,
        constraints(320.0),
        &theme,
        &mut text,
        &icons,
    );
    let rect = tree.get_absolute_paint_rect(root);
    assert!(!tree.node(root).unwrap().dirty_paint);
    let successor = harness.frame(
        &mut tree,
        root,
        constraints(400.0),
        &theme,
        &mut text,
        &icons,
    );
    assert_eq!(tree.get_absolute_paint_rect(root), rect);
    assert_eq!((layouts.get(), paints.get()), (2, 2));
    assert_ne!(
        geometry_scene_primitives(&first.scene_root)[0].2,
        geometry_scene_primitives(&successor.scene_root)[0].2
    );
    assert!(!Arc::ptr_eq(
        &geometry_local_arc(&first.scene_root, key),
        &geometry_local_arc(&successor.scene_root, key)
    ));
    geometry_assert_fresh_equivalence(&mut tree, root, constraints(400.0), &mut text, &successor);

    for dispatch in [
        GeometryLayoutDispatch::Cached,
        GeometryLayoutDispatch::Full,
        GeometryLayoutDispatch::TightRelayout,
        GeometryLayoutDispatch::ConstrainedRelayout,
    ] {
        let mut tree = WidgetTree::new();
        let mut host = RetainedGeometryProbe::new(Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        host.dispatch = dispatch;
        let root = tree.create_root(Box::new(host), None, 0);
        let layouts = Rc::new(Cell::new(0));
        let paints = Rc::new(Cell::new(0));
        let probe = RetainedGeometryProbe::new(layouts.clone(), paints.clone());
        let key = probe.key();
        let child = tree.add_child(root, Box::new(probe), None, 0);
        let constraints = Constraints::tight(Size::new(320.0, 180.0));
        let mut harness = RetainedSceneTestHarness::new();
        let mut text = FontManagerTextEngine::new();
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        let rect = tree.get_absolute_paint_rect(child).unwrap();
        let old_arc = geometry_local_arc(&first.scene_root, key);
        {
            let _update = UpdateAuthorityGuard::enter();
            geometry_probe(&mut tree, child).requested_marker = 1;
            tree.mark_layout_dirty(root);
            if matches!(dispatch, GeometryLayoutDispatch::Cached) {
                tree.mark_layout_dirty(child);
            }
        }
        assert!(
            !tree.node(child).unwrap().dirty_paint
                || matches!(dispatch, GeometryLayoutDispatch::Cached)
        );
        let successor = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert_eq!(tree.get_absolute_paint_rect(child), Some(rect));
        assert_eq!(layouts.get(), 2);
        assert_eq!(paints.get(), 2);
        assert!(!Arc::ptr_eq(
            &old_arc,
            &geometry_local_arc(&successor.scene_root, key)
        ));
        let before = geometry_scene_primitives(&first.scene_root)
            .into_iter()
            .find(|p| p.0 == Some(key))
            .unwrap();
        let after = geometry_scene_primitives(&successor.scene_root)
            .into_iter()
            .find(|p| p.0 == Some(key))
            .unwrap();
        assert_ne!(before.2, after.2);
        assert_eq!(before.1, after.1);
        geometry_assert_fresh_equivalence(&mut tree, root, constraints, &mut text, &successor);
        assert!(
            !harness
                .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
                .scene_built
        );
    }
}

#[test]
fn ui_typography_geometry_scroll_wrapper_preserves_clean_content() {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(ControlOrderLayout::new(
            Size::new(320.0, 180.0),
            vec![RectF::new(20.0, 20.0, 100.0, 80.0)],
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        )),
        None,
        0,
    );
    let scroll = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
    let content = tree.add_child(
        scroll,
        Box::new(ControlOrderLayout::new(
            Size::new(100.0, 240.0),
            vec![RectF::new(0.0, 30.0, 80.0, 40.0)],
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        )),
        None,
        0,
    );
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(content);
    let layouts = Rc::new(Cell::new(0));
    let paints = Rc::new(Cell::new(0));
    let probe = RetainedGeometryProbe::new(layouts.clone(), paints.clone());
    let key = probe.key();
    let child = tree.add_child(content, Box::new(probe), None, 0);
    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let mut harness = RetainedSceneTestHarness::new();
    let mut text = FontManagerTextEngine::new();
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let child_arc = geometry_local_arc(&first.scene_root, key);
    let rect = tree.get_absolute_paint_rect(child).unwrap();
    let epoch = tree.node(child).unwrap().layout_epoch;
    {
        let _update = UpdateAuthorityGuard::enter();
        tree.node_mut(scroll)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .scroll_to_y(120.0);
        tree.update_scroll_animations(1.0);
    }
    let scroll_area = tree
        .node(scroll)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ScrollArea>()
        .unwrap();
    assert_eq!(scroll_area.target_scroll_y(), 120.0);
    assert_eq!(scroll_area.visual_scroll_y(), 30.0);
    let successor = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(successor.scene_built);
    assert_eq!(layouts.get(), 1);
    assert_eq!(paints.get(), 1);
    assert_eq!(tree.node(child).unwrap().layout_epoch, epoch);
    assert_eq!(tree.get_absolute_paint_rect(child), Some(rect));
    assert!(Arc::ptr_eq(
        &child_arc,
        &geometry_local_arc(&successor.scene_root, key)
    ));
    let primitive = geometry_scene_primitives(&successor.scene_root)
        .into_iter()
        .find(|p| p.0 == Some(key))
        .unwrap();
    assert_eq!(primitive.5, vec![(0.0, 30.0)]);
    assert_eq!(
        tree.hit_test_canonical(30.0, 30.0).unwrap().widget_id,
        tree.widget_id_for_node(child).unwrap()
    );
    geometry_assert_fresh_equivalence(&mut tree, root, constraints, &mut text, &successor);
}

struct ControlOrderLayout {
    id: WidgetInternalId,
    size: Size,
    slots: Vec<RectF>,
    clip: bool,
    hit_target: bool,
    scroll: Option<(f32, f32)>,
    path: Option<(Color, u32)>,
    overlay: bool,
    layouts: Rc<Cell<u32>>,
    paints: Rc<Cell<u32>>,
    requested_scroll: Option<f32>,
    geometry_clip: Option<GeometryClipState>,
}

struct GeometryClipState {
    bounds: RectF,
    rounded: Option<crate::core::paint::RoundedCorners>,
    pending: Option<(RectF, Option<crate::core::paint::RoundedCorners>)>,
    updates: u32,
}

impl ControlOrderLayout {
    fn new(size: Size, slots: Vec<RectF>, layouts: Rc<Cell<u32>>, paints: Rc<Cell<u32>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            size,
            slots,
            clip: false,
            hit_target: false,
            scroll: None,
            path: None,
            overlay: false,
            layouts,
            paints,
            requested_scroll: None,
            geometry_clip: None,
        }
    }
}

impl WidgetDowncast for ControlOrderLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ControlOrderLayout {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        self.hit_target
    }
    fn scroll_offset(&self) -> Option<(f32, f32)> {
        self.scroll
    }
    fn has_requested_scroll(&self) -> bool {
        self.requested_scroll.is_some()
    }
    fn take_requested_scroll_y(&mut self) -> Option<f32> {
        self.requested_scroll.take()
    }
    fn clip_bounds(&self) -> Option<RectF> {
        if let Some(clip) = &self.geometry_clip {
            return Some(clip.bounds);
        }
        self.clip
            .then_some(RectF::new(0.0, 0.0, self.size.width, self.size.height))
    }
    fn clip_bounds_rounded(&self) -> Option<crate::core::paint::RoundedCorners> {
        self.geometry_clip.as_ref().and_then(|clip| clip.rounded)
    }
    fn update(&mut self, _: f32, cx: &mut UpdateContext) {
        if let Some(clip) = &mut self.geometry_clip {
            clip.updates += 1;
            if let Some((bounds, rounded)) = clip.pending.take() {
                clip.bounds = bounds;
                clip.rounded = rounded;
                cx.invalidate_paint();
            }
        }
    }
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        self.layouts.set(self.layouts.get() + 1);
        assert_eq!(children.len(), self.slots.len());
        for (&child, &rect) in children.iter().zip(&self.slots) {
            let size = cx.layout_child(
                child,
                Constraints::tight(Size::new(rect.width, rect.height)),
            );
            assert_eq!(size, rect.size(), "child must honor its tight slot");
            cx.set_child_rect(child, RectF::new(rect.x, rect.y, size.width, size.height));
        }
        LayoutResult {
            size: constraints.constrain(self.size),
            content_size: self.size,
            viewport_size: None,
            scroll_offset: self.scroll,
        }
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        self.paints.set(self.paints.get() + 1);
        if let Some((color, z)) = self.path {
            cx.scene.push_path(
                crate::renderer::path::Path::rounded_rect(
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                    0.0,
                ),
                color,
                z,
                None,
            );
        }
    }
    fn paint_overlay(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        owner: crate::utils::id::TreeWidgetId,
    ) {
        if self.overlay {
            cx.scene.push_rect(
                rect,
                Color::from_hex(0x00FFFF),
                crate::core::paint::LAYER_OVERLAY + 8,
                None,
            );
            cx.register_overlay_hit_region(crate::core::widget_tree::OverlayHitRegion {
                abs_rect: rect,
                owner_abs_rect: rect,
                widget_id: owner,
            });
        }
    }
}

pub(super) struct ControlOrderFixture {
    pub(super) tree: WidgetTree,
    pub(super) root: NodeId,
    pub(super) host: NodeId,
    pub(super) members: [NodeId; 3],
    pub(super) buttons: [NodeId; 2],
    pub(super) labels: [NodeId; 2],
    pub(super) overlay: NodeId,
    pub(super) external: NodeId,
    pub(super) domains: crate::core::event::InteractionDomains,
    router: crate::core::event_router::EventRouter,
    bindings: crate::runtime::binding_registry::BindingRegistry,
    clicks: [Rc<Cell<u32>>; 2],
    harness: RetainedSceneTestHarness,
    text: FontManagerTextEngine,
    layouts: Rc<Cell<u32>>,
    paints: Rc<Cell<u32>>,
    scroller: Option<NodeId>,
    fusion: crate::core::input_fusion::InputFusion,
    input_time: Instant,
}

impl ControlOrderFixture {
    pub(super) fn new(binding: Option<&crate::renderer::image_binding::ImageBinding>) -> Self {
        Self::with_workspace_scroll(binding, false)
    }

    fn with_workspace_scroll(
        binding: Option<&crate::renderer::image_binding::ImageBinding>,
        scrolled: bool,
    ) -> Self {
        use crate::core::widget_tree::ControlSubtreePlacement as Placement;
        let mut tree = WidgetTree::new();
        let layouts = Rc::new(Cell::new(0));
        let paints = Rc::new(Cell::new(0));
        let clicks = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
        let earlier_clicks = clicks[0].clone();
        let later_clicks = clicks[1].clone();
        let layout =
            |size, slots| ControlOrderLayout::new(size, slots, layouts.clone(), paints.clone());
        let root = tree.create_root(
            Box::new(layout(
                Size::new(320.0, 220.0),
                vec![
                    RectF::new(8.0, 8.0, 240.0, 168.0),
                    RectF::new(224.0, 120.0, 80.0, 64.0),
                    RectF::new(180.0, 112.0, 60.0, 40.0),
                ],
            )),
            None,
            0,
        );
        let mut workspace = layout(
            if scrolled {
                Size::new(304.0, 224.0)
            } else {
                Size::new(240.0, 168.0)
            },
            vec![
                RectF::new(0.0, 24.0, 200.0, 112.0),
                RectF::new(0.0, 24.0, 200.0, 112.0),
                RectF::new(208.0, 96.0, 96.0, 64.0),
            ],
        );
        workspace.clip = true;
        let scroller = scrolled.then(|| {
            tree.add_child(
                root,
                Box::new(ScrollArea::new_unwired().enable_horizontal_scrollbar(true)),
                None,
                0,
            )
        });
        let host = tree.add_child(scroller.unwrap_or(root), Box::new(workspace), None, 0);
        if let Some(scroller) = scroller {
            tree.node_mut(scroller)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<ScrollArea>()
                .unwrap()
                .wire_content(host);
        }
        let a = tree.add_child(
            host,
            Box::new(layout(
                Size::new(200.0, 112.0),
                vec![
                    RectF::new(16.0, 16.0, 160.0, 64.0),
                    RectF::new(24.0, 40.0, 140.0, 24.0),
                    RectF::new(36.0, 64.0, 40.0, 16.0),
                ],
            )),
            None,
            0,
        );
        let b = tree.add_child(
            host,
            Box::new(layout(
                Size::new(200.0, 112.0),
                vec![
                    RectF::new(0.0, 0.0, 200.0, 96.0),
                    RectF::new(24.0, 64.0, 140.0, 24.0),
                ],
            )),
            None,
            0,
        );
        let mut preview = layout(Size::new(96.0, 64.0), vec![]);
        preview.path = Some((Color::from_hex(0xFF00FF), 790));
        let preview = tree.add_child(host, Box::new(preview), None, 0);
        let buttons = [
            tree.add_child(
                a,
                Box::new(
                    crate::widgets::button::Button::with_text("EARLIER")
                        .on_click(move || earlier_clicks.set(earlier_clicks.get() + 1)),
                ),
                None,
                0,
            ),
            tree.add_child(
                b,
                Box::new(
                    crate::widgets::button::Button::with_text("LATER")
                        .on_click(move || later_clicks.set(later_clicks.get() + 1)),
                ),
                None,
                0,
            ),
        ];
        let labels = [
            tree.add_child(
                a,
                Box::new(
                    crate::widgets::label::Label::new("EARLIER LABEL")
                        .with_color(Color::WHITE)
                        .with_selection_policy(
                            crate::core::text_selection::TextSelectionPolicy::Enabled,
                        ),
                ),
                None,
                0,
            ),
            tree.add_child(
                b,
                Box::new(crate::widgets::label::Label::new("LATER LABEL").with_color(Color::WHITE)),
                None,
                0,
            ),
        ];
        let mut decoration = layout(Size::new(40.0, 16.0), vec![]);
        decoration.path = Some((Color::from_hex(0xFF00FF), 700));
        let decoration = tree.add_child(a, Box::new(decoration), None, 0);
        let external = tree.add_child(
            root,
            Box::new(layout(
                Size::new(80.0, 64.0),
                vec![RectF::new(0.0, 0.0, 80.0, 64.0)],
            )),
            None,
            0,
        );
        let external_button = tree.add_child(
            external,
            Box::new(crate::widgets::button::Button::with_text("TOP")),
            None,
            0,
        );
        let mut overlay = layout(Size::new(60.0, 40.0), vec![]);
        overlay.overlay = true;
        let overlay = tree.add_child(root, Box::new(overlay), None, 0);
        for (index, node) in buttons.into_iter().chain([external_button]).enumerate() {
            let button = tree
                .node_mut(node)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<crate::widgets::button::Button>()
                .unwrap();
            let contract = crate::core::appearance::ButtonAppearanceContract::standard();
            let mut values =
                button_product_values(&contract, crate::core::layout::Padding::all(8.0));
            let paint = crate::core::appearance::ButtonPaintValues {
                surface: crate::core::appearance::AppearanceSurface::Solid(Color::from_hex(
                    [0x2020C0, 0x008000, 0xC00000][index],
                )),
                radius: 0.0,
                content_color: Color::WHITE,
                underlay: binding.map(|binding| {
                    crate::core::appearance::AppearanceSurface::Image(
                        crate::core::paint::SurfaceImage {
                            source: crate::core::appearance::AppearanceImageSource::Admitted(
                                binding.clone(),
                            ),
                            uv_rect: RectF::new(0.0, 0.0, 1.0, 1.0),
                            tint: Color::WHITE,
                        },
                    )
                }),
                border: None,
                overlay: None,
            };
            values.resting = Some(paint.clone());
            values.hovered = Some(paint.clone());
            values.pressed = Some(paint.clone());
            values.disabled = Some(paint);
            values.focus = Some(crate::core::appearance::AppearanceStroke {
                color: Color::from_hex(0xFF0000),
                width: 3.0,
            });
            button
                .set_appearance(
                    button_product_style(contract, values, 3590 + index as u64).unwrap(),
                    None,
                )
                .unwrap();
        }
        tree.set_control_subtree_order(
            root,
            &[scroller.unwrap_or(host), external, overlay],
            Placement::Main { z_index: 50 },
        )
        .unwrap();
        tree.set_control_subtree_order(host, &[a, b, preview], Placement::Main { z_index: 200 })
            .unwrap();
        tree.set_control_subtree_order(
            a,
            &[buttons[0], labels[0], decoration],
            Placement::Main { z_index: 800 },
        )
        .unwrap();
        tree.set_control_subtree_order(
            external,
            &[external_button],
            Placement::Main { z_index: 700 },
        )
        .unwrap();
        let mut domains = crate::core::event::InteractionDomains::new();
        domains
            .primary_state_mut()
            .focus_manager
            .set_focus(Some(tree.widget_id_for_node(buttons[0]).unwrap()));
        Self {
            tree,
            root,
            host,
            members: [a, b, preview],
            buttons,
            labels,
            overlay,
            external,
            domains,
            router: crate::core::event_router::EventRouter::new(),
            bindings: crate::runtime::binding_registry::BindingRegistry::new(),
            clicks,
            harness: RetainedSceneTestHarness::new(),
            text: FontManagerTextEngine::new(),
            layouts,
            paints,
            scroller,
            fusion: crate::core::input_fusion::InputFusion::new(),
            input_time: Instant::now(),
        }
    }

    pub(super) fn frame(&mut self) -> crate::core::app::RetainedSceneTestFrame {
        self.harness.frame_with_appearance(
            &mut self.tree,
            self.root,
            Constraints::tight(Size::new(320.0, 220.0)),
            &Theme::default(),
            &mut self.text,
            &IconRegistry::new(&[]),
            &self.domains,
            crate::core::event::WindowId::PRIMARY,
        )
    }

    pub(super) fn reverse(&mut self) {
        self.tree
            .set_control_subtree_order(
                self.host,
                &[self.members[1], self.members[0], self.members[2]],
                crate::core::widget_tree::ControlSubtreePlacement::Main { z_index: 200 },
            )
            .unwrap();
    }

    pub(super) fn image_dependency_counts(&self) -> Option<(usize, usize)> {
        self.harness.image_dependency_counts()
    }

    pub(super) fn click_counts(&self) -> [u32; 2] {
        [self.clicks[0].get(), self.clicks[1].get()]
    }

    fn pointer(&mut self, kind: crate::core::event::PointerEventKind, x: f32, y: f32) {
        use crate::core::event::{
            FusedInputEvent, PointerButton, PointerDevice, PointerEventKind, WindowId,
        };
        let window_id = WindowId::PRIMARY;
        let time = Instant::now();
        let pos = crate::core::math::Point::new(x, y);
        let device = PointerDevice::Mouse;
        let event = match kind {
            PointerEventKind::Down => FusedInputEvent::PointerDown {
                window_id,
                time,
                pos,
                device,
                button: PointerButton::Primary,
                click_count: 1,
            },
            PointerEventKind::Up => FusedInputEvent::PointerUp {
                window_id,
                time,
                pos,
                device,
                button: PointerButton::Primary,
            },
            _ => unreachable!("control-order fixture routes only Down and Up"),
        };
        assert!(self.domains.should_route_event(&event));
        self.router.route_input_event(
            &mut self.tree,
            event,
            Some(self.root),
            &mut self.text,
            None,
            self.domains.resolve_window_mut(window_id).unwrap(),
            &mut self.bindings,
        );
    }

    pub(super) fn select_label(&mut self) {
        use crate::core::event::{
            Event, EventPhase, KeyboardAuthorityMode, Modifiers, TextInputEvent,
        };
        let owner = self.tree.widget_id_for_node(self.labels[0]).unwrap();
        let tree_ptr = &mut self.tree as *mut WidgetTree;
        let mut cx = crate::core::widget::EventContext::new(
            owner,
            tree_ptr,
            0,
            std::ptr::null_mut(),
            Some(&mut self.text),
            None,
            false,
            false,
            KeyboardAuthorityMode::Navigation,
        );
        let event = Event::Text(TextInputEvent {
            text: "a".into(),
            modifiers: Modifiers {
                ctrl: true,
                meta: true,
                ..Modifiers::default()
            },
        });
        assert!(
            self.tree
                .node_mut(self.labels[0])
                .unwrap()
                .widget
                .handle_event(&mut cx, &event, EventPhase::Target)
        );
        self.tree.mark_paint_dirty(self.labels[0]);
    }

    pub(super) fn workspace_preview(
        binding: Option<&crate::renderer::image_binding::ImageBinding>,
        scrolled: bool,
    ) -> (Self, NodeId) {
        use crate::core::widget_tree::ControlSubtreePlacement;
        let mut f = Self::with_workspace_scroll(binding, scrolled);
        let a = f.members[0];
        let handle = f
            .tree
            .add_child(a, Box::new(WorkspacePreviewHandle::new(binding)), None, 0);
        let owner = f.tree.widget_id_for_node(handle).unwrap();
        f.tree
            .node_mut(handle)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<WorkspacePreviewHandle>()
            .unwrap()
            .owner = Some(owner);
        let a_layout = f
            .tree
            .node_mut(a)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ControlOrderLayout>()
            .unwrap();
        a_layout.slots.push(RectF::new(16.0, 96.0, 168.0, 16.0));
        a_layout.clip = true;
        let children = f.tree.children(a);
        f.tree
            .set_control_subtree_order(
                a,
                &children,
                ControlSubtreePlacement::Main {
                    z_index: crate::core::paint::LAYER_CONTENT,
                },
            )
            .unwrap();
        f.tree
            .set_control_subtree_order(
                f.host,
                &f.members,
                ControlSubtreePlacement::Main {
                    z_index: crate::core::paint::LAYER_CONTENT,
                },
            )
            .unwrap();
        // Exercise an actual current accelerated index, including noninteractive decoration.
        let dummy_parent = f.members[2];
        for _ in 0..130 {
            f.tree.add_child(
                dummy_parent,
                Box::new(ControlOrderLayout::new(
                    Size::new(1.0, 1.0),
                    vec![],
                    f.layouts.clone(),
                    f.paints.clone(),
                )),
                None,
                0,
            );
        }
        f.tree
            .node_mut(dummy_parent)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ControlOrderLayout>()
            .unwrap()
            .slots = vec![RectF::new(0.0, 0.0, 1.0, 1.0); 130];
        f.tree
            .set_control_subtree_main_affordances(f.host, &[handle])
            .unwrap();
        assert!(f.frame().scene_built);
        if let Some(scroller) = f.scroller {
            f.tree
                .node_mut(scroller)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<ScrollArea>()
                .unwrap()
                .scroll_to(16.0, 24.0);
            f.tree.mark_paint_dirty(scroller);
            assert!(f.frame().scene_built);
        }
        (f, handle)
    }

    pub(super) fn fused_button(&mut self, down: bool, x: f32, y: f32) {
        use crate::core::event::{
            PointerButton, PointerDevice, RawButtonState, RawInputEvent, WindowId,
        };
        // Distinct physical presses retain normal click-history semantics.
        self.input_time += Duration::from_secs(1);
        self.fused_raw(RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time: self.input_time,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state: if down {
                RawButtonState::Pressed
            } else {
                RawButtonState::Released
            },
            position: crate::core::math::Point::new(x, y),
        });
    }

    pub(super) fn fused_move(&mut self, x: f32, y: f32) {
        use crate::core::event::{PointerDevice, RawInputEvent, WindowId};
        self.input_time += Duration::from_millis(16);
        self.fused_raw(RawInputEvent::PointerMoved {
            window_id: WindowId::PRIMARY,
            time: self.input_time,
            device: PointerDevice::Mouse,
            position: crate::core::math::Point::new(x, y),
            delta: (0.0, 0.0),
        });
    }

    fn fused_raw(&mut self, raw: crate::core::event::RawInputEvent) {
        let mut events = Vec::new();
        self.fusion.process_raw(raw, &mut events);
        self.fusion.flush(&mut events);
        self.route_fused(events);
    }

    fn route_fused(&mut self, events: Vec<crate::core::event::FusedInputEvent>) {
        for event in events {
            let window = crate::core::event::WindowId::PRIMARY;
            assert!(self.domains.should_route_event(&event));
            self.router.route_input_event(
                &mut self.tree,
                event,
                Some(self.root),
                &mut self.text,
                None,
                self.domains.resolve_window_mut(window).unwrap(),
                &mut self.bindings,
            );
        }
    }

    pub(super) fn cancel_preview(&mut self) {
        let mut events = Vec::new();
        self.fusion.cancel_drag(&mut events);
        self.fusion.flush(&mut events);
        assert!(events.iter().any(|event| matches!(
            event,
            crate::core::event::FusedInputEvent::DragEnd {
                cancelled: true,
                ..
            }
        )));
        self.route_fused(events);
    }

    pub(super) fn preview_keys(
        &self,
        handle: NodeId,
    ) -> Vec<crate::core::scene_identity::SceneNodeKey> {
        vec![self.preview_handle(handle).key()]
    }

    fn preview_handle(&self, handle: NodeId) -> &WorkspacePreviewHandle {
        self.tree
            .node(handle)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    pub(super) fn preview_arc(&self, handle: NodeId) -> Arc<[crate::core::scene::SceneNode]> {
        self.harness
            .overlay_fragment(self.tree.widget_id_for_node(handle).unwrap())
            .unwrap()
    }

    pub(super) fn assert_preview_capture(&self, handle: NodeId) {
        use crate::core::event::PointerId;
        let domain = self.domains.primary_state();
        let owner = self.tree.widget_id_for_node(handle).unwrap();
        assert_eq!(
            domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(owner)
        );
        assert!(domain.gesture_registry.has_session(PointerId::MOUSE));
        assert_eq!(self.preview_handle(handle).gesture_commits, 1);
        assert!(self.preview_handle(handle).preview.is_some());
        assert_eq!(self.preview_handle(handle).product_commits, 0);
    }

    pub(super) fn assert_preview_cancelled(&mut self, handle: NodeId) {
        use crate::core::event::PointerId;
        let domain = self.domains.primary_state_mut();
        assert_eq!(
            domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
        assert_eq!(domain.pointer_capture.captured_widget, None);
        assert!(!domain.gesture_registry.has_session(PointerId::MOUSE));
        assert!(
            domain
                .gesture_registry
                .candidates(PointerId::MOUSE)
                .is_empty()
        );
        assert!(
            domain
                .gesture_registry
                .take_commit_requests(PointerId::MOUSE)
                .is_empty()
        );
        let handle = self.preview_handle(handle);
        assert!(handle.preview.is_none());
        assert!(handle.down.is_none());
        assert_eq!(handle.product_commits, 0);
        assert_eq!(
            (
                handle.candidate_cancels,
                handle.capture_cancels,
                handle.rollbacks
            ),
            (1, 1, 1)
        );
    }

    pub(super) fn fresh_preview_scene(&mut self) -> crate::core::scene::SceneGraph {
        let frame = RetainedSceneTestHarness::new().frame_with_appearance(
            &mut self.tree,
            self.root,
            Constraints::tight(Size::new(320.0, 220.0)),
            &Theme::default(),
            &mut self.text,
            &IconRegistry::new(&[]),
            &self.domains,
            crate::core::event::WindowId::PRIMARY,
        );
        assert!(frame.scene_built);
        let mut builder = SceneBuilder::new();
        builder.push_retained_fragment(frame.scene_root.into());
        builder.build()
    }
}

struct WorkspacePreviewHandle {
    id: WidgetInternalId,
    owner: Option<crate::utils::id::TreeWidgetId>,
    down: Option<crate::core::math::Point>,
    preview: Option<crate::core::math::Point>,
    binding: Option<crate::renderer::image_binding::ImageBinding>,
    local_paints: Cell<u32>,
    overlay_paints: Cell<u32>,
    gesture_commits: u32,
    candidate_cancels: u32,
    capture_cancels: u32,
    rollbacks: u32,
    product_commits: u32,
    forbidden_overlay_sidecar: bool,
}

impl WorkspacePreviewHandle {
    fn new(binding: Option<&crate::renderer::image_binding::ImageBinding>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            owner: None,
            down: None,
            preview: None,
            binding: binding.cloned(),
            local_paints: Cell::new(0),
            overlay_paints: Cell::new(0),
            gesture_commits: 0,
            candidate_cancels: 0,
            capture_cancels: 0,
            rollbacks: 0,
            product_commits: 0,
            forbidden_overlay_sidecar: false,
        }
    }
    fn key(&self) -> crate::core::scene_identity::SceneNodeKey {
        crate::core::scene_identity::SceneNodeKey::new(self.id.0)
    }
    fn rollback(&mut self, cx: &mut crate::core::widget::EventContext) {
        self.down = None;
        if self.preview.take().is_some() {
            self.rollbacks += 1;
            cx.invalidate_paint();
        }
    }
}

impl WidgetDowncast for WorkspacePreviewHandle {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for WorkspacePreviewHandle {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn clip_bounds(&self) -> Option<RectF> {
        // Its child-only clip does not enclose its post-child overlay.
        Some(RectF::new(0.0, 0.0, 1.0, 1.0))
    }
    fn layout(
        &mut self,
        _: &mut LayoutContext,
        constraints: Constraints,
        _: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(constraints.constrain(Size::new(168.0, 16.0)))
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        self.local_paints.set(self.local_paints.get() + 1);
        cx.register_overlay_hit_region(crate::core::widget_tree::OverlayHitRegion {
            abs_rect: RectF::zero(),
            owner_abs_rect: rect,
            widget_id: self.owner.unwrap(),
        });
    }
    fn paint_overlay(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        owner: crate::utils::id::TreeWidgetId,
    ) {
        self.overlay_paints.set(self.overlay_paints.get() + 1);
        if self.forbidden_overlay_sidecar {
            cx.register_overlay_hit_region(crate::core::widget_tree::OverlayHitRegion {
                abs_rect: rect,
                owner_abs_rect: rect,
                widget_id: owner,
            });
        }
        let Some(delta) = self.preview else {
            return;
        };
        let outline = RectF::new(rect.x + delta.x, rect.y + delta.y, 200.0, 96.0);
        cx.scene.begin_clip(outline, None);
        let edges = [
            RectF::new(outline.x, outline.y, outline.width, 8.0),
            RectF::new(
                outline.x,
                outline.y + outline.height - 8.0,
                outline.width,
                8.0,
            ),
            RectF::new(outline.x, outline.y, 8.0, outline.height),
            RectF::new(
                outline.x + outline.width - 8.0,
                outline.y,
                8.0,
                outline.height,
            ),
        ];
        for (part, edge) in edges.into_iter().enumerate() {
            cx.scene.push_rect(
                edge,
                Color::from_hex(0xFFFF00),
                crate::core::paint::LAYER_CONTENT,
                (part == 0).then_some(self.key()),
            );
        }
        if let Some(binding) = &self.binding {
            cx.scene.push_admitted_image(
                RectF::new(outline.x + 16.0, outline.y + 16.0, 8.0, 8.0),
                binding,
                RectF::new(0.0, 0.0, 1.0, 1.0),
                Color::WHITE,
                crate::core::paint::LAYER_CONTENT + 1,
                None,
            );
        }
        cx.scene.end_clip();
    }
    fn gesture_surface(&self) -> &'static [crate::core::event::GestureKind] {
        &[crate::core::event::GestureKind::Drag]
    }
    fn handle_event(
        &mut self,
        cx: &mut crate::core::widget::EventContext,
        event: &crate::core::event::Event,
        phase: crate::core::event::EventPhase,
    ) -> bool {
        use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
        if phase != EventPhase::Target {
            return false;
        }
        let Event::Pointer(pointer) = event else {
            return false;
        };
        match pointer.kind {
            PointerEventKind::Down if pointer.button == Some(PointerButton::Primary) => {
                self.down = Some(crate::core::math::Point::new(
                    pointer.local_pos.0,
                    pointer.local_pos.1,
                ));
                true
            }
            PointerEventKind::DragStart if self.down.is_some() => {
                cx.request_gesture_commit();
                true
            }
            PointerEventKind::Move if self.preview.is_some() => {
                let down = self.down.unwrap();
                self.preview = Some(crate::core::math::Point::new(
                    pointer.local_pos.0 - down.x,
                    pointer.local_pos.1 - down.y,
                ));
                cx.invalidate_paint();
                true
            }
            PointerEventKind::Up if pointer.button == Some(PointerButton::Primary) => {
                self.down = None;
                if self.preview.take().is_some() {
                    self.product_commits += 1;
                    cx.invalidate_paint();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
    fn on_gesture_commit(
        &mut self,
        cx: &mut crate::core::widget::EventContext,
        kind: crate::core::event::GestureKind,
    ) {
        assert_eq!(kind, crate::core::event::GestureKind::Drag);
        assert!(self.down.is_some());
        self.gesture_commits += 1;
        self.preview = Some(crate::core::math::Point::new(0.0, 0.0));
        cx.invalidate_paint();
    }
    fn on_gesture_cancel(
        &mut self,
        cx: &mut crate::core::widget::EventContext,
        kind: crate::core::event::GestureKind,
    ) {
        assert_eq!(kind, crate::core::event::GestureKind::Drag);
        self.candidate_cancels += 1;
        self.rollback(cx);
    }
    fn on_pointer_capture_cancel(
        &mut self,
        cx: &mut crate::core::widget::EventContext,
        _: crate::core::event::PointerId,
        _: crate::core::event::CancellationReason,
    ) {
        assert!(!cx.has_pointer_capture());
        self.capture_cancels += 1;
        self.rollback(cx);
    }
    fn on_unmount(&mut self) {
        self.down = None;
        self.preview = None;
    }
}

struct GeometryPreviewHandle {
    inner: WorkspacePreviewHandle,
    dependency: crate::core::widget::AbsoluteDragGeometryDependency,
    captured_appearance: bool,
    updates: u32,
    settlements: u32,
    invalidate_on_update: bool,
}

struct GeometryConsumedPressScreen {
    widget: ControlOrderLayout,
    downs: Rc<Cell<u32>>,
    ups: Rc<Cell<u32>>,
}

impl crate::core::app::Screen for GeometryConsumedPressScreen {
    fn build<'a>(
        &'a mut self,
        _: &'a mut crate::core::app::ScreenFrameContext<'a>,
    ) -> &'a mut dyn Widget {
        &mut self.widget
    }
    fn handle_input(
        &mut self,
        event: &crate::core::event::FusedInputEvent,
        _: &mut crate::core::app::ScreenContext<'_>,
    ) -> bool {
        match event {
            crate::core::event::FusedInputEvent::PointerDown { .. } => {
                self.downs.set(self.downs.get() + 1);
                true
            }
            crate::core::event::FusedInputEvent::PointerUp { .. } => {
                self.ups.set(self.ups.get() + 1);
                true
            }
            _ => false,
        }
    }
}

fn geometry_consuming_screen(
    f: &ControlOrderFixture,
) -> (crate::core::app::AppRoot, Rc<Cell<u32>>, Rc<Cell<u32>>) {
    let downs = Rc::new(Cell::new(0));
    let ups = Rc::new(Cell::new(0));
    let mut app =
        crate::core::app::AppRoot::new(crate::core::app::AppContext::new(Theme::default()), f.root);
    app.screens.push(Box::new(GeometryConsumedPressScreen {
        widget: ControlOrderLayout::new(
            Size::new(1.0, 1.0),
            vec![],
            f.layouts.clone(),
            f.paints.clone(),
        ),
        downs: downs.clone(),
        ups: ups.clone(),
    }));
    (app, downs, ups)
}

impl WidgetDowncast for GeometryPreviewHandle {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for GeometryPreviewHandle {
    fn id(&self) -> WidgetInternalId {
        self.inner.id()
    }
    fn clip_bounds(&self) -> Option<RectF> {
        self.inner.clip_bounds()
    }
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        self.inner.layout(cx, constraints, children)
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId]) {
        self.inner.paint(cx, rect, children);
    }
    fn paint_overlay(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        owner: crate::utils::id::TreeWidgetId,
    ) {
        self.inner.paint_overlay(cx, rect, owner);
    }
    fn gesture_surface(&self) -> &'static [crate::core::event::GestureKind] {
        self.inner.gesture_surface()
    }
    fn absolute_drag_geometry_dependency(
        &self,
    ) -> Option<crate::core::widget::AbsoluteDragGeometryDependency> {
        Some(self.dependency)
    }
    fn update(&mut self, _: f32, cx: &mut UpdateContext) {
        self.updates += 1;
        self.captured_appearance = cx.appearance_facts().unwrap().pointer_captured();
        if self.invalidate_on_update {
            self.invalidate_on_update = false;
            cx.invalidate_layout();
        }
    }
    fn settle_absolute_drag_geometry_appearance(
        &mut self,
        facts: crate::core::appearance::FrameworkAppearanceFacts,
    ) {
        self.settlements += 1;
        self.captured_appearance = facts.pointer_captured();
    }
    fn handle_event(
        &mut self,
        cx: &mut crate::core::widget::EventContext,
        event: &crate::core::event::Event,
        phase: crate::core::event::EventPhase,
    ) -> bool {
        self.inner.handle_event(cx, event, phase)
    }
    fn on_gesture_commit(
        &mut self,
        cx: &mut crate::core::widget::EventContext,
        kind: crate::core::event::GestureKind,
    ) {
        self.inner.on_gesture_commit(cx, kind);
    }
    fn on_gesture_cancel(
        &mut self,
        cx: &mut crate::core::widget::EventContext,
        kind: crate::core::event::GestureKind,
    ) {
        self.inner.on_gesture_cancel(cx, kind);
    }
    fn on_pointer_capture_cancel(
        &mut self,
        cx: &mut crate::core::widget::EventContext,
        pointer: crate::core::event::PointerId,
        reason: crate::core::event::CancellationReason,
    ) {
        self.inner.on_pointer_capture_cancel(cx, pointer, reason);
    }
    fn on_unmount(&mut self) {
        self.inner.on_unmount();
    }
}

impl ControlOrderFixture {
    fn geometry_preview(scrolled: bool) -> (Self, NodeId) {
        let (mut f, handle) = Self::workspace_preview(None, scrolled);
        let owner = f.tree.widget_id_for_node(handle).unwrap();
        let mut inner = WorkspacePreviewHandle::new(None);
        inner.owner = Some(owner);
        f.tree.replace_widget_only(
            handle,
            Box::new(GeometryPreviewHandle {
                inner,
                dependency: crate::core::widget::AbsoluteDragGeometryDependency {
                    viewport_owner: f.scroller.unwrap_or(f.host),
                    reflow_root: f.scroller.unwrap_or(f.host),
                },
                captured_appearance: false,
                updates: 0,
                settlements: 0,
                invalidate_on_update: false,
            }),
        );
        f.tree.mark_layout_dirty(handle);
        (f, handle)
    }

    fn geometry_handle(&self, handle: NodeId) -> &GeometryPreviewHandle {
        self.tree
            .node(handle)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    fn geometry_raw(
        &mut self,
        raw: crate::core::event::RawInputEvent,
    ) -> Vec<crate::core::event::SessionQualifiedInput> {
        let window = crate::core::event::WindowId::PRIMARY;
        let domain = self.domains.domain_for_window(window).unwrap();
        let mut events = Vec::new();
        self.fusion.process_raw_with_gesture_sessions(
            raw,
            domain,
            &mut self.domains.primary_state_mut().gesture_registry,
            &self.tree,
            None,
            &mut events,
        );
        self.fusion.flush_with_gesture_sessions(&mut events);
        events
    }

    fn geometry_route(&mut self, events: Vec<crate::core::event::SessionQualifiedInput>) {
        for event in events {
            assert!(self.domains.should_route_event(event.event()));
            self.router.route_session_input_event(
                &mut self.tree,
                event,
                Some(self.root),
                &mut self.text,
                None,
                self.domains.primary_state_mut(),
                &mut self.bindings,
            );
        }
    }

    fn geometry_button(&mut self, down: bool, x: f32, y: f32) {
        use crate::core::event::{
            PointerButton, PointerDevice, RawButtonState, RawInputEvent, WindowId,
        };
        self.input_time += Duration::from_secs(1);
        let events = self.geometry_raw(RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time: self.input_time,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state: if down {
                RawButtonState::Pressed
            } else {
                RawButtonState::Released
            },
            position: crate::core::math::Point::new(x, y),
        });
        self.geometry_route(events);
    }

    fn geometry_move(&mut self, x: f32, y: f32) {
        use crate::core::event::{PointerDevice, RawInputEvent, WindowId};
        self.input_time += Duration::from_millis(16);
        let events = self.geometry_raw(RawInputEvent::PointerMoved {
            window_id: WindowId::PRIMARY,
            time: self.input_time,
            device: PointerDevice::Mouse,
            position: crate::core::math::Point::new(x, y),
            delta: (0.0, 0.0),
        });
        self.geometry_route(events);
    }

    fn geometry_frame(
        &mut self,
        ticker: &mut crate::core::app::EngineTicker,
        size: Size,
        pending_focus: Option<crate::utils::id::TreeWidgetId>,
    ) -> crate::core::app::RetainedSceneTestFrame {
        self.input_time += Duration::from_millis(16);
        let frame_time = ticker.begin_frame_at(self.input_time);
        let mut events = Vec::new();
        self.fusion
            .process_frame_with_gesture_sessions(&frame_time, ticker, &mut events);
        self.geometry_route(events);
        self.harness.frame_with_gesture_sessions(
            &mut self.tree,
            self.root,
            Constraints::tight(size),
            &Theme::default(),
            &mut self.text,
            &IconRegistry::new(&[]),
            &frame_time,
            pending_focus,
            &mut self.domains,
            crate::core::event::WindowId::PRIMARY,
            &mut self.router,
            &mut self.fusion,
        )
    }

    fn start_geometry_preview(
        &mut self,
        handle: NodeId,
        ticker: &mut crate::core::app::EngineTicker,
    ) -> crate::core::app::RetainedSceneTestFrame {
        use crate::core::event::PointerId;
        assert!(
            self.geometry_frame(ticker, Size::new(320.0, 220.0), None)
                .scene_built
        );
        let rect = self.tree.get_absolute_paint_rect(handle).unwrap();
        let scroll = self.tree.cumulative_scroll_offset(handle);
        let point = (rect.x - scroll.0 + 4.0, rect.y - scroll.1 + 8.0);
        assert_eq!(
            self.tree.hit_test(point.0, point.1).unwrap().widget_id,
            self.tree.widget_id_for_node(handle).unwrap()
        );
        self.geometry_button(true, point.0, point.1);
        assert!(self.geometry_handle(handle).inner.preview.is_none());
        self.geometry_move(point.0 + 16.0, point.1 - 16.0);
        self.geometry_move(point.0 + 48.0, point.1 - 48.0);
        assert_eq!(
            self.domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            self.tree.widget_id_for_node(handle)
        );
        let frame = self.geometry_frame(ticker, Size::new(320.0, 220.0), None);
        assert!(frame.scene_built);
        assert!(!self.tree.node(handle).unwrap().dirty_paint);
        assert_eq!(workspace_preview_outline_count(&frame.scene_root), 4);
        assert!(self.geometry_handle(handle).captured_appearance);
        frame
    }

    fn assert_geometry_cancelled(
        &mut self,
        handle: NodeId,
        frame: &crate::core::app::RetainedSceneTestFrame,
    ) {
        use crate::core::event::PointerId;
        assert!(
            frame.scene_built,
            "first affected frame must contain the cancelled result"
        );
        assert_eq!(workspace_preview_outline_count(&frame.scene_root), 0);
        let state = self.domains.primary_state_mut();
        assert!(!state.gesture_registry.has_session(PointerId::MOUSE));
        assert!(
            state
                .gesture_registry
                .candidates(PointerId::MOUSE)
                .is_empty()
        );
        assert!(
            state
                .gesture_registry
                .take_commit_requests(PointerId::MOUSE)
                .is_empty()
        );
        assert_eq!(
            state
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
        assert_eq!(state.pointer_capture.captured_widget, None);
        let probe = self.geometry_handle(handle);
        assert_eq!(
            (
                probe.inner.candidate_cancels,
                probe.inner.capture_cancels,
                probe.inner.rollbacks
            ),
            (1, 1, 1)
        );
        assert_eq!(probe.inner.product_commits, 0);
        assert!(probe.inner.preview.is_none() && probe.inner.down.is_none());
        assert!(!probe.captured_appearance);
        assert_eq!(probe.settlements, 1);
        assert!(self.tree.geometry_protected_owners.is_empty());
    }
}

#[test]
fn geometry_dependent_absolute_viewport_change_clears_first_retained_preview() {
    let (mut f, handle) = ControlOrderFixture::geometry_preview(true);
    let mut ticker = crate::core::app::EngineTicker::new(Instant::now());
    let live = f.start_geometry_preview(handle, &mut ticker);
    let owner = f.tree.widget_id_for_node(handle).unwrap();
    let key = f.geometry_handle(handle).inner.key();
    let old_arc = f.preview_arc(handle);
    assert_eq!(workspace_preview_key_count(&live.scene_root, &[key]), 1);
    let handle_bounds = f.tree.get_absolute_paint_rect(handle).unwrap();
    let focus = f.domains.primary_state().focus_manager.focused();
    let updates = f.geometry_handle(handle).updates;
    let epoch = current_layout_epoch();
    f.input_time += Duration::from_millis(16);
    let old_release = f.geometry_raw(crate::core::event::RawInputEvent::PointerButton {
        window_id: crate::core::event::WindowId::PRIMARY,
        time: f.input_time,
        device: crate::core::event::PointerDevice::Mouse,
        button: crate::core::event::PointerButton::Primary,
        state: crate::core::event::RawButtonState::Released,
        position: crate::core::math::Point::new(64.0, 64.0),
    });
    // Real workspace viewport allocation changes; the positioned handle's own box does not.
    f.tree
        .node_mut(f.root)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ControlOrderLayout>()
        .unwrap()
        .slots[0]
        .width = 224.0;
    f.tree.mark_layout_dirty(f.root);
    let cancelled = f.geometry_frame(&mut ticker, Size::new(304.0, 220.0), None);
    f.assert_geometry_cancelled(handle, &cancelled);
    assert_eq!(f.tree.widget_id_for_node(handle), Some(owner));
    assert_eq!(
        f.tree.get_absolute_paint_rect(handle).unwrap(),
        handle_bounds
    );
    assert_eq!(f.geometry_handle(handle).updates, updates + 1);
    assert_eq!(current_layout_epoch(), epoch + 1);
    assert_eq!(f.domains.primary_state().focus_manager.focused(), focus);
    assert_eq!(
        workspace_preview_arc_count(&cancelled.scene_root, &old_arc),
        0
    );
    assert!(!Arc::ptr_eq(&old_arc, &f.preview_arc(handle)));
    // A late physical release carries the retired press; it cannot commit or recapture.
    let (mut app, downs, ups) = geometry_consuming_screen(&f);
    for event in old_release {
        assert!(crate::core::app::dispatch_session_input_to_app_root(
            &mut app,
            &mut f.tree,
            &mut f.router,
            f.domains.primary_state_mut(),
            &event,
            &mut f.text
        ));
    }
    assert_eq!(
        (downs.get(), ups.get()),
        (0, 0),
        "retired protected terminals never reach Screen"
    );
    assert_eq!(f.geometry_handle(handle).inner.product_commits, 0);
    assert_eq!(f.click_counts(), [0, 0]);
}

#[test]
fn geometry_dependent_absolute_update_invalidity_settles_before_first_scene() {
    let (mut f, handle) = ControlOrderFixture::geometry_preview(false);
    let mut ticker = crate::core::app::EngineTicker::new(Instant::now());
    f.start_geometry_preview(handle, &mut ticker);
    let updates = f.geometry_handle(handle).updates;
    f.tree
        .node_mut(handle)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<GeometryPreviewHandle>()
        .unwrap()
        .invalidate_on_update = true;
    let epoch = current_layout_epoch();
    let cancelled = f.geometry_frame(&mut ticker, Size::new(320.0, 220.0), None);
    f.assert_geometry_cancelled(handle, &cancelled);
    assert_eq!(
        f.geometry_handle(handle).updates,
        updates + 1,
        "terminal settlement cannot run another Update"
    );
    assert_eq!(current_layout_epoch(), epoch + 1);
    f.geometry_button(false, 80.0, 60.0);

    // These are the actual existing post-layout producers, queried without consuming them early.
    for effect in 0..3 {
        let (mut f, handle) = ControlOrderFixture::geometry_preview(true);
        let mut ticker = crate::core::app::EngineTicker::new(Instant::now());
        if effect == 2 {
            f.tree
                .node_mut(f.host)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<ControlOrderLayout>()
                .unwrap()
                .slots[2]
                .y = 144.0;
            f.tree.mark_layout_dirty(f.host);
        }
        f.start_geometry_preview(handle, &mut ticker);
        let scroller = f.scroller.unwrap();
        let pending_focus = match effect {
            0 => {
                f.tree
                    .node_mut(f.host)
                    .unwrap()
                    .widget
                    .as_any_mut()
                    .downcast_mut::<ControlOrderLayout>()
                    .unwrap()
                    .requested_scroll = Some(40.0);
                None
            }
            1 => {
                assert!(f.tree.begin_layout_scroll_anchor(
                    handle,
                    f.members[0],
                    crate::widgets::scroll_area::LayoutScrollAnchorIntent::ExpandRevealRegion
                ));
                assert!(
                    f.tree
                        .node(scroller)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<ScrollArea>()
                        .unwrap()
                        .pending_layout_anchor()
                        .is_some()
                );
                None
            }
            _ => Some(f.tree.widget_id_for_node(f.members[2]).unwrap()),
        };
        let updates = f.geometry_handle(handle).updates;
        let cancelled = f.geometry_frame(&mut ticker, Size::new(320.0, 220.0), pending_focus);
        f.assert_geometry_cancelled(handle, &cancelled);
        assert_eq!(f.geometry_handle(handle).updates, updates + 1);
    }
}

#[test]
fn geometry_dependent_absolute_native_completed_transition_preserves_drag() {
    use crate::core::event::{
        InteractionDomains, PointerButton, PointerDevice, PointerId, RawButtonState, RawInputEvent,
        WindowId,
    };
    use crate::core::math::Point;

    for completed in [true, false] {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(crate::widgets::screen_host::ScreenHost::new()),
            None,
            0,
        );
        let canvas = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
        let workspace = tree.add_child(
            canvas,
            Box::new(ControlOrderLayout::new(
                Size::new(320.0, 220.0),
                vec![RectF::new(16.0, 24.0, 168.0, 16.0)],
                Rc::new(Cell::new(0)),
                Rc::new(Cell::new(0)),
            )),
            None,
            0,
        );
        tree.node_mut(canvas)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(workspace);
        let handle = tree.add_child(
            workspace,
            Box::new(GeometryPreviewHandle {
                inner: WorkspacePreviewHandle::new(None),
                dependency: crate::core::widget::AbsoluteDragGeometryDependency {
                    viewport_owner: canvas,
                    reflow_root: canvas,
                },
                captured_appearance: false,
                updates: 0,
                settlements: 0,
                invalidate_on_update: false,
            }),
            None,
            0,
        );
        let owner = tree.widget_id_for_node(handle).unwrap();
        tree.node_mut(handle)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<GeometryPreviewHandle>()
            .unwrap()
            .inner
            .owner = Some(owner);
        let root_owner = tree.widget_id_for_node(root).unwrap();
        let mut harness = RetainedSceneTestHarness::new();
        let mut text = FontManagerTextEngine::new();
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let constraints = Constraints::tight(Size::new(320.0, 220.0));

        let mut domains = InteractionDomains::new();

        // Mirror native bootstrap: project the fresh host, then swap before its first Layout.
        {
            let _layout = crate::core::rect_authority::LayoutAuthorityGuard::enter();
            crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
            crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }
        tree.begin_layout_transition_frame();
        assert!(
            harness
                .frame_with_appearance(
                    &mut tree,
                    root,
                    constraints,
                    &theme,
                    &mut text,
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                )
                .scene_built
        );
        assert!(
            harness.interpolated_rect(root_owner).is_some(),
            "the native-order setup must pass real authorization and bind the host transition"
        );
        harness.advance_transition_time(if completed { 1.0 } else { 0.1 });
        assert_eq!(harness.interpolated_rect(root_owner).is_none(), completed);

        let mut router = crate::core::event_router::EventRouter::new();
        let mut fusion = crate::core::input_fusion::InputFusion::new();
        let mut bindings = crate::runtime::binding_registry::BindingRegistry::new();
        let window = WindowId::PRIMARY;
        let domain = domains.domain_for_window(window).unwrap();
        let rect = tree.get_absolute_paint_rect(handle).unwrap();
        let down = Point::new(rect.x + 4.0, rect.y + 8.0);
        assert_eq!(tree.hit_test(down.x, down.y).unwrap().widget_id, owner);
        let start = Instant::now();
        let mut ticker = crate::core::app::EngineTicker::new(start);
        for step in 0..4 {
            let position = match step {
                0 => down,
                1 => Point::new(down.x + 16.0, down.y - 16.0),
                _ => Point::new(down.x + 48.0, down.y - 48.0),
            };
            let time = start + Duration::from_millis(step * 16);
            let raw = if step == 0 || step == 3 {
                RawInputEvent::PointerButton {
                    window_id: window,
                    time,
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                    state: if step == 0 {
                        RawButtonState::Pressed
                    } else {
                        RawButtonState::Released
                    },
                    position,
                }
            } else {
                RawInputEvent::PointerMoved {
                    window_id: window,
                    time,
                    device: PointerDevice::Mouse,
                    position,
                    delta: (0.0, 0.0),
                }
            };
            let mut events = Vec::new();
            fusion.process_raw_with_gesture_sessions(
                raw,
                domain,
                &mut domains.primary_state_mut().gesture_registry,
                &tree,
                None,
                &mut events,
            );
            fusion.flush_with_gesture_sessions(&mut events);
            for event in events {
                router.route_session_input_event(
                    &mut tree,
                    event,
                    Some(root),
                    &mut text,
                    None,
                    domains.primary_state_mut(),
                    &mut bindings,
                );
                harness.checkpoint_gesture_geometry(
                    &mut tree,
                    &mut router,
                    domains.primary_state_mut(),
                );
            }
            let widget = tree
                .node(handle)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<GeometryPreviewHandle>()
                .unwrap();
            if completed {
                assert_eq!(
                    widget.inner.candidate_cancels, 0,
                    "a completed host transition is not a new causal geometry effect"
                );
                if step == 1 || step == 2 {
                    assert_eq!(
                        domains
                            .primary_state()
                            .pointer_capture_manager
                            .current_capture(PointerId::MOUSE),
                        Some(owner)
                    );
                    assert!(widget.inner.preview.is_some());
                    assert_eq!(widget.inner.product_commits, 0);
                }
            } else {
                assert_eq!(widget.inner.product_commits, 0);
                assert_eq!(
                    widget.inner.candidate_cancels, 1,
                    "a genuinely running ancestor transition still retires the qualified press"
                );
            }
            if step == 2 && completed {
                let frame_time = ticker.begin_frame_at(time);
                let frame = harness.frame_with_gesture_sessions(
                    &mut tree,
                    root,
                    constraints,
                    &theme,
                    &mut text,
                    &icons,
                    &frame_time,
                    None,
                    &mut domains,
                    window,
                    &mut router,
                    &mut fusion,
                );
                assert!(frame.scene_built);
                assert!(
                    tree.node(handle)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<GeometryPreviewHandle>()
                        .unwrap()
                        .captured_appearance
                );
            }
        }
        let widget = tree
            .node(handle)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<GeometryPreviewHandle>()
            .unwrap();
        assert_eq!(widget.inner.product_commits, u32::from(completed));
        assert!(widget.inner.preview.is_none());
        assert!(
            !domains
                .primary_state()
                .pointer_capture_manager
                .is_any_captured()
        );
        assert!(
            !domains
                .primary_state()
                .gesture_registry
                .has_session(PointerId::MOUSE)
        );
    }
}

#[test]
fn geometry_dependent_absolute_paint_only_work_preserves_live_preview() {
    use crate::core::event::PointerId;
    let (mut f, handle) = ControlOrderFixture::geometry_preview(false);
    let mut ticker = crate::core::app::EngineTicker::new(Instant::now());
    f.start_geometry_preview(handle, &mut ticker);
    let session = f
        .domains
        .primary_state()
        .gesture_registry
        .session_id(PointerId::MOUSE);
    let owner = f.tree.widget_id_for_node(handle).unwrap();
    let arc = f.preview_arc(handle);
    let paints = f.geometry_handle(handle).inner.overlay_paints.get();
    let epoch = current_layout_epoch();
    f.tree.mark_paint_dirty(f.external);
    let unchanged = f.geometry_frame(&mut ticker, Size::new(320.0, 220.0), None);
    assert_eq!(current_layout_epoch(), epoch);
    assert!(Arc::ptr_eq(&arc, &f.preview_arc(handle)));
    assert_eq!(f.geometry_handle(handle).inner.overlay_paints.get(), paints);
    if unchanged.scene_built {
        assert_eq!(workspace_preview_outline_count(&unchanged.scene_root), 4);
    }
    assert_eq!(
        f.domains
            .primary_state()
            .gesture_registry
            .session_id(PointerId::MOUSE),
        session
    );
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(owner)
    );
    assert_eq!(f.geometry_handle(handle).settlements, 0);
    // An unrelated origin may propagate layout dirtiness through root without invalidating the declared domain.
    f.tree.mark_layout_dirty(f.external);
    let unrelated = f.geometry_frame(&mut ticker, Size::new(320.0, 220.0), None);
    assert!(unrelated.scene_built);
    assert_eq!(workspace_preview_outline_count(&unrelated.scene_root), 4);
    assert_eq!(
        f.domains
            .primary_state()
            .gesture_registry
            .session_id(PointerId::MOUSE),
        session
    );
    f.geometry_move(84.0, 68.0);
    let moved = f.geometry_frame(&mut ticker, Size::new(320.0, 220.0), None);
    assert!(moved.scene_built);
    assert_eq!(workspace_preview_outline_count(&moved.scene_root), 4);
    assert_eq!(f.geometry_handle(handle).inner.candidate_cancels, 0);
    f.geometry_button(false, 84.0, 68.0);
    assert_eq!(f.geometry_handle(handle).inner.product_commits, 1);

    let (mut f, handle) = ControlOrderFixture::geometry_preview(true);
    let mut ticker = crate::core::app::EngineTicker::new(Instant::now());
    f.start_geometry_preview(handle, &mut ticker);
    let visible_focus = f.tree.widget_id_for_node(f.buttons[1]).unwrap();
    let session = f
        .domains
        .primary_state()
        .gesture_registry
        .session_id(PointerId::MOUSE);
    let visible = f.geometry_frame(&mut ticker, Size::new(320.0, 220.0), Some(visible_focus));
    assert!(visible.scene_built);
    assert_eq!(workspace_preview_outline_count(&visible.scene_root), 4);
    assert_eq!(
        f.domains
            .primary_state()
            .gesture_registry
            .session_id(PointerId::MOUSE),
        session,
        "an already visible automatic reveal is not an effect"
    );

    // AppRoot-consumed, unenrolled Down retains its ordinary Screen Up delivery.
    let (mut f, _) = ControlOrderFixture::geometry_preview(false);
    let mut ticker = crate::core::app::EngineTicker::new(Instant::now());
    assert!(
        f.geometry_frame(&mut ticker, Size::new(320.0, 220.0), None)
            .scene_built
    );
    let (mut app, downs, ups) = geometry_consuming_screen(&f);
    for down in [true, false] {
        f.input_time += Duration::from_secs(1);
        let events = f.geometry_raw(crate::core::event::RawInputEvent::PointerButton {
            window_id: crate::core::event::WindowId::PRIMARY,
            time: f.input_time,
            device: crate::core::event::PointerDevice::Mouse,
            button: crate::core::event::PointerButton::Primary,
            state: if down {
                crate::core::event::RawButtonState::Pressed
            } else {
                crate::core::event::RawButtonState::Released
            },
            position: crate::core::math::Point::new(28.0, 136.0),
        });
        for event in events {
            assert!(crate::core::app::dispatch_session_input_to_app_root(
                &mut app,
                &mut f.tree,
                &mut f.router,
                f.domains.primary_state_mut(),
                &event,
                &mut f.text
            ));
        }
        assert!(
            !f.domains
                .primary_state()
                .gesture_registry
                .has_session(PointerId::MOUSE)
        );
        assert!(
            !f.domains
                .primary_state()
                .pointer_capture_manager
                .is_any_captured()
        );
        assert!(f.tree.geometry_protected_owners.is_empty());
    }
    assert_eq!((downs.get(), ups.get()), (1, 1));
    assert_eq!(f.click_counts(), [0, 0]);
}

#[test]
fn geometry_dependent_absolute_changed_clip_retires_before_first_scene() {
    use crate::core::event::PointerId;
    use crate::core::paint::RoundedCorners;

    for rounded_only in [false, true] {
        let (mut f, handle) = ControlOrderFixture::geometry_preview(false);
        let initial_clip = RectF::new(0.0, 0.0, 240.0, 168.0);
        let initial_rounded = Some(RoundedCorners::uniform(4.0));
        f.tree
            .node_mut(f.host)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ControlOrderLayout>()
            .unwrap()
            .geometry_clip = Some(GeometryClipState {
            bounds: initial_clip,
            rounded: initial_rounded,
            pending: None,
            updates: 0,
        });
        let mut ticker = crate::core::app::EngineTicker::new(Instant::now());
        let live = f.start_geometry_preview(handle, &mut ticker);
        let owner = f.tree.widget_id_for_node(handle).unwrap();
        let host_mount = f.tree.widget_id_for_node(f.host).unwrap();
        let host_bounds = f.tree.get_absolute_paint_rect(f.host).unwrap();
        let handle_bounds = f.tree.get_absolute_paint_rect(handle).unwrap();
        let session = f
            .domains
            .primary_state()
            .gesture_registry
            .session_id(PointerId::MOUSE);
        assert!(session.is_some());
        let focus = f.domains.primary_state().focus_manager.focused();
        let keyboard = f.domains.primary_state().keyboard_mode;
        let key = f.geometry_handle(handle).inner.key();
        let arc = f.preview_arc(handle);
        let paints = f.geometry_handle(handle).inner.overlay_paints.get();
        let epoch = current_layout_epoch();
        assert_eq!(workspace_preview_key_count(&live.scene_root, &[key]), 1);

        // The same declaration can request paint without invalidating the basis.
        f.tree
            .node_mut(f.host)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ControlOrderLayout>()
            .unwrap()
            .geometry_clip
            .as_mut()
            .unwrap()
            .pending = Some((initial_clip, initial_rounded));
        let updates = f.geometry_handle(handle).updates;
        let unchanged = f.geometry_frame(&mut ticker, Size::new(320.0, 220.0), None);
        assert_eq!(current_layout_epoch(), epoch);
        assert_eq!(f.geometry_handle(handle).updates, updates + 1);
        assert_eq!(
            f.domains
                .primary_state()
                .gesture_registry
                .session_id(PointerId::MOUSE),
            session
        );
        assert_eq!(
            f.domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(owner)
        );
        assert_eq!(
            f.domains.primary_state().pointer_capture.captured_widget,
            Some(owner)
        );
        assert_eq!(workspace_preview_outline_count(&unchanged.scene_root), 4);
        assert_eq!(
            workspace_preview_key_count(&unchanged.scene_root, &[key]),
            1
        );
        assert!(Arc::ptr_eq(&arc, &f.preview_arc(handle)));
        let probe = f.geometry_handle(handle);
        assert_eq!(probe.inner.overlay_paints.get(), paints);
        assert!(probe.captured_appearance && probe.inner.preview.is_some());
        assert_eq!(
            (
                probe.inner.candidate_cancels,
                probe.inner.capture_cancels,
                probe.settlements
            ),
            (0, 0, 0)
        );
        assert!(!f.tree.node(handle).unwrap().dirty_paint);

        let changed_clip = if rounded_only {
            initial_clip
        } else {
            RectF::new(0.0, 0.0, 240.0, 164.0)
        };
        let changed_rounded = if rounded_only {
            Some(RoundedCorners::uniform(12.0))
        } else {
            initial_rounded
        };
        let clip = f
            .tree
            .node_mut(f.host)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ControlOrderLayout>()
            .unwrap()
            .geometry_clip
            .as_mut()
            .unwrap();
        let clip_updates = clip.updates;
        clip.pending = Some((changed_clip, changed_rounded));
        let updates = f.geometry_handle(handle).updates;
        let cancelled = f.geometry_frame(&mut ticker, Size::new(320.0, 220.0), None);
        f.assert_geometry_cancelled(handle, &cancelled);
        assert_eq!(
            current_layout_epoch(),
            epoch,
            "clip retirement adds no layout"
        );
        assert_eq!(f.geometry_handle(handle).updates, updates + 1);
        assert_eq!(f.tree.widget_id_for_node(f.host), Some(host_mount));
        assert_eq!(f.tree.widget_id_for_node(handle), Some(owner));
        assert_eq!(f.tree.get_absolute_paint_rect(f.host), Some(host_bounds));
        assert_eq!(f.tree.get_absolute_paint_rect(handle), Some(handle_bounds));
        let host = f.tree.node(f.host).unwrap();
        assert_eq!(host.widget.clip_bounds(), Some(changed_clip));
        assert_eq!(host.widget.clip_bounds_rounded(), changed_rounded);
        assert_eq!(
            host.widget
                .as_any()
                .downcast_ref::<ControlOrderLayout>()
                .unwrap()
                .geometry_clip
                .as_ref()
                .unwrap()
                .updates,
            clip_updates + 1
        );
        assert_eq!(f.domains.primary_state().focus_manager.focused(), focus);
        assert_eq!(f.domains.primary_state().keyboard_mode, keyboard);
        assert_eq!(f.click_counts(), [0, 0]);
        assert_eq!(workspace_preview_arc_count(&cancelled.scene_root, &arc), 0);
        assert!(!Arc::ptr_eq(&arc, &f.preview_arc(handle)));
        assert_eq!(
            f.geometry_handle(handle).inner.overlay_paints.get(),
            paints + 1
        );
        assert_eq!(
            workspace_preview_key_count(&cancelled.scene_root, &[key]),
            0
        );
    }
}

pub(super) fn control_order_member_fragments(
    nodes: &[crate::core::scene::SceneNode],
    host: crate::utils::id::TreeWidgetId,
) -> Vec<Arc<[crate::core::scene::SceneNode]>> {
    use crate::core::scene::SceneNode;
    let mut out = Vec::new();
    for node in nodes {
        match node {
            SceneNode::RetainedFragment(value) => {
                if value
                    .control_subtree_member
                    .is_some_and(|member| member.scope == host)
                {
                    out.push(value.children.clone());
                }
                out.extend(control_order_member_fragments(&value.children, host));
            }
            SceneNode::Clip(value) => {
                out.extend(control_order_member_fragments(&value.children, host))
            }
            SceneNode::Scroll(value) => {
                out.extend(control_order_member_fragments(&value.children, host))
            }
            SceneNode::Layer(value) => {
                out.extend(control_order_member_fragments(&value.children, host))
            }
            _ => {}
        }
    }
    out
}

pub(super) fn workspace_preview_key_count(
    nodes: &[crate::core::scene::SceneNode],
    keys: &[crate::core::scene_identity::SceneNodeKey],
) -> usize {
    use crate::core::scene::SceneNode;
    nodes
        .iter()
        .map(|node| {
            usize::from(node.key().is_some_and(|key| keys.contains(&key)))
                + match node {
                    SceneNode::RetainedFragment(value) => {
                        workspace_preview_key_count(&value.children, keys)
                    }
                    SceneNode::Clip(value) => workspace_preview_key_count(&value.children, keys),
                    SceneNode::Scroll(value) => workspace_preview_key_count(&value.children, keys),
                    SceneNode::Layer(value) => workspace_preview_key_count(&value.children, keys),
                    _ => 0,
                }
        })
        .sum()
}

pub(super) fn workspace_preview_outline_count(nodes: &[crate::core::scene::SceneNode]) -> usize {
    use crate::core::scene::SceneNode;
    nodes
        .iter()
        .map(|node| match node {
            SceneNode::Rect(value) => usize::from(value.color == Color::from_hex(0xFFFF00).into()),
            SceneNode::RetainedFragment(value) => workspace_preview_outline_count(&value.children),
            SceneNode::Clip(value) => workspace_preview_outline_count(&value.children),
            SceneNode::Scroll(value) => workspace_preview_outline_count(&value.children),
            SceneNode::Layer(value) => workspace_preview_outline_count(&value.children),
            _ => 0,
        })
        .sum()
}

fn workspace_preview_arc_count(
    nodes: &[crate::core::scene::SceneNode],
    arc: &Arc<[crate::core::scene::SceneNode]>,
) -> usize {
    use crate::core::scene::SceneNode;
    nodes
        .iter()
        .map(|node| match node {
            SceneNode::RetainedFragment(value) => {
                usize::from(Arc::ptr_eq(&value.children, arc))
                    + workspace_preview_arc_count(&value.children, arc)
            }
            SceneNode::Clip(value) => workspace_preview_arc_count(&value.children, arc),
            SceneNode::Scroll(value) => workspace_preview_arc_count(&value.children, arc),
            SceneNode::Layer(value) => workspace_preview_arc_count(&value.children, arc),
            _ => 0,
        })
        .sum()
}

fn workspace_preview_scope_path(
    nodes: &[crate::core::scene::SceneNode],
    key: crate::core::scene_identity::SceneNodeKey,
    members: &mut Vec<crate::core::paint::ControlSubtreeMember>,
    clips: &mut Vec<RectF>,
    scroll: (f32, f32),
) -> Option<(
    Vec<crate::core::paint::ControlSubtreeMember>,
    Vec<RectF>,
    (f32, f32),
)> {
    use crate::core::scene::SceneNode;
    for node in nodes {
        if node.key() == Some(key) {
            return Some((members.clone(), clips.clone(), scroll));
        }
        let found = match node {
            SceneNode::RetainedFragment(value) => {
                if let Some(member) = value.control_subtree_member {
                    members.push(member);
                }
                let found =
                    workspace_preview_scope_path(&value.children, key, members, clips, scroll);
                if value.control_subtree_member.is_some() {
                    members.pop();
                }
                found
            }
            SceneNode::Clip(value) => {
                clips.push(value.rect);
                let found =
                    workspace_preview_scope_path(&value.children, key, members, clips, scroll);
                clips.pop();
                found
            }
            SceneNode::Scroll(value) => workspace_preview_scope_path(
                &value.children,
                key,
                members,
                clips,
                (scroll.0 + value.offset.0, scroll.1 + value.offset.1),
            ),
            SceneNode::Layer(value) => {
                workspace_preview_scope_path(&value.children, key, members, clips, scroll)
            }
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

#[test]
fn workspace_preview_composition_retains_handle_output_and_real_control_order() {
    use crate::core::event::PointerId;
    use crate::core::paint::{LAYER_CONTENT, LAYER_MAIN_AFFORDANCE};
    use crate::core::widget_tree::{
        ControlSubtreeAffordanceError, ControlSubtreeOrderError, ControlSubtreePlacement,
    };
    let (mut f, handle) = ControlOrderFixture::workspace_preview(None, true);
    let top = f.tree.widget_id_for_node(f.buttons[1]).unwrap();
    let source = f.tree.widget_id_for_node(handle).unwrap();
    let host = f.tree.widget_id_for_node(f.host).unwrap();
    let root = f.tree.widget_id_for_node(f.root).unwrap();
    let keys = f.preview_keys(handle);
    let viewport = RectF::new(0.0, 0.0, 320.0, 220.0);
    f.tree.rebuild_hit_test_grid(viewport);
    assert!(f.tree.has_current_hit_test_grid());
    assert_eq!(f.tree.hit_test(64.0, 64.0).unwrap().widget_id, top);
    f.fused_button(true, 64.0, 64.0);
    assert_eq!(f.click_counts(), [0, 0]);
    f.fused_button(false, 64.0, 64.0);
    assert_eq!(f.click_counts(), [0, 1]);
    assert!(f.frame().scene_built);
    f.fused_button(true, 16.0, 112.0);
    assert!(f.preview_handle(handle).preview.is_none());
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    f.fused_move(32.0, 96.0);
    f.fused_move(64.0, 64.0);
    f.assert_preview_capture(handle);
    let live = f.frame();
    assert!(live.scene_built);
    assert!(!f.tree.node(handle).unwrap().dirty_paint);
    assert_eq!(workspace_preview_key_count(&live.scene_root, &keys), 1);
    assert_eq!(workspace_preview_outline_count(&live.scene_root), 4);
    let arc = f.preview_arc(handle);
    assert_eq!(workspace_preview_arc_count(&live.scene_root, &arc), 1);
    let (members, clips, scroll) = workspace_preview_scope_path(
        &live.scene_root,
        keys[0],
        &mut Vec::new(),
        &mut Vec::new(),
        (0.0, 0.0),
    )
    .unwrap();
    assert_eq!(
        members.len(),
        2,
        "intervening member prefixes are replaced, outer prefix survives"
    );
    assert_eq!((members[0].scope, members[0].member_index), (root, 0));
    assert_eq!(
        (
            members[1].scope,
            members[1].z_index,
            members[1].member_index
        ),
        (host, LAYER_MAIN_AFFORDANCE, 0)
    );
    assert_eq!(scroll, (16.0, 24.0));
    assert!(
        clips.contains(&RectF::new(8.0, 32.0, 200.0, 112.0)),
        "intermediate bar clip retained"
    );
    assert!(
        !clips.iter().any(|clip| clip.size() == Size::new(1.0, 1.0)),
        "source child-only clip is outside its overlay"
    );
    f.tree.rebuild_hit_test_grid(viewport);
    let canonical = f.tree.hit_test_canonical(64.0, 64.0).unwrap();
    let accelerated = f.tree.hit_test(64.0, 64.0).unwrap();
    assert_eq!(canonical.widget_id, top);
    assert_eq!(accelerated.widget_id, top);
    assert_eq!(canonical.local_pos, accelerated.local_pos);
    assert_eq!(
        f.click_counts(),
        [0, 1],
        "captured movement did not activate the hit-query winner"
    );

    let paints = (
        f.preview_handle(handle).local_paints.get(),
        f.preview_handle(handle).overlay_paints.get(),
    );
    let layouts = f.layouts.get();
    let epoch = current_layout_epoch();
    let dependencies = f.image_dependency_counts();
    f.tree
        .set_control_subtree_main_affordances(f.host, &[handle])
        .unwrap();
    assert!(f.tree.has_current_hit_test_grid());
    assert!(!f.tree.node(handle).unwrap().dirty_composition);
    assert_eq!(
        f.tree.clear_control_subtree_order(f.host),
        Err(ControlSubtreeOrderError::InvalidMainPlacement)
    );
    assert_eq!(
        f.tree.set_control_subtree_order(
            f.host,
            &f.members,
            ControlSubtreePlacement::Main { z_index: 200 }
        ),
        Err(ControlSubtreeOrderError::InvalidMainPlacement)
    );
    assert_eq!(
        f.tree
            .set_control_subtree_main_affordances(f.members[0], &[handle]),
        Err(ControlSubtreeAffordanceError::SourceAlreadyAssigned)
    );
    for destination in [None, Some(f.members[0]), Some(f.host)] {
        // Normal frame cleanup uses node_mut; measure declaration invalidation separately.
        f.tree.rebuild_hit_test_grid(viewport);
        assert!(f.tree.has_current_hit_test_grid());
        f.tree
            .clear_control_subtree_main_affordances(f.host)
            .unwrap();
        f.tree
            .clear_control_subtree_main_affordances(f.members[0])
            .unwrap();
        if let Some(destination) = destination {
            f.tree
                .set_control_subtree_main_affordances(destination, &[handle])
                .unwrap();
        }
        assert!(
            f.tree.has_current_hit_test_grid(),
            "decorative composition keeps the current input index"
        );
        assert!(!f.tree.node(handle).unwrap().dirty_layout);
        assert!(!f.tree.node(handle).unwrap().dirty_paint);
        let changed = f.frame();
        assert!(changed.scene_built);
        assert!(Arc::ptr_eq(&arc, &f.preview_arc(handle)));
        assert_eq!(workspace_preview_arc_count(&changed.scene_root, &arc), 1);
        assert_eq!(workspace_preview_key_count(&changed.scene_root, &keys), 1);
        assert_eq!(workspace_preview_outline_count(&changed.scene_root), 4);
        assert_eq!(
            (
                f.preview_handle(handle).local_paints.get(),
                f.preview_handle(handle).overlay_paints.get()
            ),
            paints
        );
        assert_eq!(f.layouts.get(), layouts);
        assert_eq!(current_layout_epoch(), epoch);
        assert_eq!(f.image_dependency_counts(), dependencies);
        f.assert_preview_capture(handle);
    }
    f.tree
        .set_control_subtree_order(
            f.root,
            &[f.external, f.scroller.unwrap(), f.overlay],
            ControlSubtreePlacement::Main { z_index: 50 },
        )
        .unwrap();
    let outer = f.frame();
    assert!(outer.scene_built);
    assert!(Arc::ptr_eq(&arc, &f.preview_arc(handle)));
    assert_eq!(
        (
            f.preview_handle(handle).local_paints.get(),
            f.preview_handle(handle).overlay_paints.get()
        ),
        paints
    );
    let (members, _, _) = workspace_preview_scope_path(
        &outer.scene_root,
        keys[0],
        &mut Vec::new(),
        &mut Vec::new(),
        (0.0, 0.0),
    )
    .unwrap();
    assert_eq!((members[0].scope, members[0].member_index), (root, 1));
    f.cancel_preview();
    f.assert_preview_cancelled(handle);
    let cancelled = f.frame();
    assert!(
        cancelled.scene_built,
        "first canonical frame after fused cancellation"
    );
    assert_eq!(workspace_preview_key_count(&cancelled.scene_root, &keys), 0);
    assert_eq!(workspace_preview_outline_count(&cancelled.scene_root), 0);
    f.fused_button(false, 64.0, 64.0);
    assert_eq!(f.click_counts(), [0, 1]);
    f.fused_button(true, 64.0, 64.0);
    assert_eq!(f.click_counts(), [0, 1]);
    f.fused_button(false, 64.0, 64.0);
    assert_eq!(f.click_counts(), [0, 2]);
    f.tree
        .set_control_subtree_order(
            f.host,
            &[f.members[1], f.members[0], f.members[2]],
            ControlSubtreePlacement::Main {
                z_index: LAYER_CONTENT,
            },
        )
        .unwrap();
    assert!(f.frame().scene_built);
    f.fused_button(true, 64.0, 32.0);
    assert_eq!(f.click_counts(), [0, 2]);
    f.fused_button(false, 64.0, 32.0);
    assert_eq!(f.click_counts(), [1, 2]);
    f.domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(top));
    f.select_label();
    assert!(f.frame().scene_built);
    assert_eq!(
        f.tree.hit_test_canonical(64.0, 32.0).unwrap().widget_id,
        f.tree.widget_id_for_node(f.buttons[0]).unwrap()
    );
    assert_eq!(f.click_counts(), [1, 2]);

    f.tree.remove_node(handle);
    let removed = f.tree.drain_pending_removals();
    assert!(removed.contains(&source));
    f.router
        .on_widgets_destroyed(&removed, f.domains.primary_state_mut());
    let a = f.members[0];
    f.tree
        .node_mut(a)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ControlOrderLayout>()
        .unwrap()
        .slots
        .pop();
    let replacement = f
        .tree
        .add_child(a, Box::new(WorkspacePreviewHandle::new(None)), None, 0);
    let replacement_owner = f.tree.widget_id_for_node(replacement).unwrap();
    assert_ne!(source, replacement_owner);
    f.tree
        .node_mut(replacement)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<WorkspacePreviewHandle>()
        .unwrap()
        .owner = Some(replacement_owner);
    f.tree
        .node_mut(a)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ControlOrderLayout>()
        .unwrap()
        .slots
        .push(RectF::new(16.0, 96.0, 168.0, 16.0));
    let children = f.tree.children(a);
    f.tree
        .set_control_subtree_order(
            a,
            &children,
            ControlSubtreePlacement::Main {
                z_index: LAYER_CONTENT,
            },
        )
        .unwrap();
    assert!(f.tree.find_by_id(source).is_none());
    assert!(
        f.tree
            .control_subtree_main_affordance(replacement)
            .is_none()
    );
    let replaced = f.frame();
    assert!(replaced.scene_built);
    assert_eq!(workspace_preview_key_count(&replaced.scene_root, &keys), 0);

    // A cached overlay-origin sidecar is ineligible; ordinary local sidecars above were accepted.
    let (mut invalid, handle) = ControlOrderFixture::workspace_preview(None, false);
    invalid
        .tree
        .clear_control_subtree_main_affordances(invalid.host)
        .unwrap();
    invalid
        .tree
        .node_mut(handle)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<WorkspacePreviewHandle>()
        .unwrap()
        .forbidden_overlay_sidecar = true;
    invalid.tree.mark_paint_dirty(handle);
    assert!(invalid.frame().scene_built);
    let paints = invalid.preview_handle(handle).overlay_paints.get();
    invalid
        .tree
        .set_control_subtree_main_affordances(invalid.host, &[handle])
        .unwrap();
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| invalid.frame()));
    let rejection = rejected
        .err()
        .expect("cached ineligible overlay must be rejected");
    let message = rejection
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| rejection.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(message.contains("INV-359-MAIN-AFFORDANCE-OUTPUT"));
    assert_eq!(
        invalid.preview_handle(handle).overlay_paints.get(),
        paints,
        "cached evidence rejects without repainting the source"
    );
    invalid
        .tree
        .clear_control_subtree_main_affordances(invalid.host)
        .unwrap();
    assert!(
        invalid.frame().scene_built,
        "ordinary unexported output is restored without eligibility rejection"
    );
}

#[test]
fn control_subtree_order_real_controls_reorder_retains_local_fragments() {
    use crate::core::widget_tree::{
        ControlSubtreeOrderError as Error, ControlSubtreePlacement as Placement,
    };
    let mut f = ControlOrderFixture::new(None);
    let first = f.frame();
    assert!(first.scene_built);
    let later = f.tree.widget_id_for_node(f.buttons[1]).unwrap();
    assert_eq!(f.tree.hit_test(80.0, 60.0).unwrap().widget_id, later);
    f.select_label();
    let selected = f.frame();
    assert!(selected.scene_built);
    assert_eq!(
        f.tree.hit_test(80.0, 60.0).unwrap().widget_id,
        later,
        "selection does not raise an earlier member"
    );
    let host_owner = f.tree.widget_id_for_node(f.host).unwrap();
    let before = control_order_member_fragments(&selected.scene_root, host_owner);
    assert_eq!(before.len(), 3);
    let epochs: Vec<_> = f
        .tree
        .iter_nodes()
        .map(|(id, node)| (id, node.id, node.layout_epoch))
        .collect();
    let counts = (f.layouts.get(), f.paints.get());
    let placement = Placement::Main { z_index: 200 };
    for (members, placement, error) in [
        (
            vec![f.members[0], f.members[0], f.members[2]],
            placement,
            Error::DuplicateMember,
        ),
        (
            vec![NodeId(usize::MAX), f.members[1], f.members[2]],
            placement,
            Error::NotDirectChild,
        ),
        (
            vec![f.members[0], f.buttons[1], f.members[2]],
            placement,
            Error::NotDirectChild,
        ),
        (vec![f.members[0]], placement, Error::IncompleteMemberSet),
        (
            f.members.to_vec(),
            Placement::Main {
                z_index: crate::core::paint::LAYER_OVERLAY,
            },
            Error::InvalidMainPlacement,
        ),
    ] {
        assert_eq!(
            f.tree
                .set_control_subtree_order(f.host, &members, placement),
            Err(error)
        );
    }
    assert_eq!(
        f.tree
            .set_control_subtree_order(NodeId(usize::MAX), &[], placement),
        Err(Error::MissingHost)
    );
    assert_eq!(
        f.tree.clear_control_subtree_order(NodeId(usize::MAX)),
        Err(Error::MissingHost)
    );
    f.tree
        .set_control_subtree_order(f.host, &f.members, placement)
        .unwrap();
    f.tree.clear_control_subtree_order(f.members[2]).unwrap();
    assert!(
        !f.frame().scene_built,
        "rejection and valid no-ops preserve publication"
    );
    f.reverse();
    assert!(f.tree.node(f.host).unwrap().dirty_composition);
    assert!(f.tree.node(f.root).unwrap().dirty_composition);
    for member in f.members {
        let node = f.tree.node(member).unwrap();
        assert!(!node.dirty_layout && !node.dirty_paint && !node.dirty_composition);
    }
    let reversed = f.frame();
    assert!(reversed.scene_built && reversed.visual_changed);
    let after = control_order_member_fragments(&reversed.scene_root, host_owner);
    assert!(Arc::ptr_eq(&before[0], &after[1]));
    assert!(Arc::ptr_eq(&before[1], &after[0]));
    assert!(Arc::ptr_eq(&before[2], &after[2]));
    assert_eq!(counts, (f.layouts.get(), f.paints.get()));
    assert_eq!(
        epochs,
        f.tree
            .iter_nodes()
            .map(|(id, node)| (id, node.id, node.layout_epoch))
            .collect::<Vec<_>>()
    );
    let owner = f.tree.widget_id_for_node(f.buttons[0]).unwrap();
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(owner)
    );
    assert_eq!(f.tree.hit_test(80.0, 60.0).unwrap().widget_id, owner);
    f.domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(f.tree.widget_id_for_node(f.buttons[1]).unwrap()));
    let focused = f.frame();
    assert!(focused.scene_built);
    assert_eq!(
        f.tree.hit_test(80.0, 60.0).unwrap().widget_id,
        owner,
        "focus does not raise the earlier member"
    );
    let ordered = control_order_member_fragments(&focused.scene_root, host_owner);
    assert!(Arc::ptr_eq(&after[2], &ordered[2]));
    f.tree.clear_control_subtree_order(f.host).unwrap();
    let cleared = f.frame();
    assert!(cleared.scene_built);
    assert!(control_order_member_fragments(&cleared.scene_root, host_owner).is_empty());
    assert_eq!(
        f.tree.hit_test(80.0, 60.0).unwrap().widget_id,
        f.tree.widget_id_for_node(f.buttons[0]).unwrap()
    );
    assert!(!f.frame().scene_built);
    assert_eq!(
        f.click_counts(),
        [0, 0],
        "focus and selection do not activate"
    );

    // Keep routed focus/appearance dirtiness separate from the pure reorder proof.
    let mut routed = ControlOrderFixture::new(None);
    assert!(routed.frame().scene_built);
    for (button, expected) in [
        (routed.buttons[0], Size::new(160.0, 64.0)),
        (routed.buttons[1], Size::new(200.0, 96.0)),
    ] {
        let bounds = routed
            .tree
            .node(button)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::button::Button>()
            .unwrap()
            .layout_bounds;
        assert_eq!(bounds, expected);
        assert_eq!(routed.tree.get_cached_size(button), Some(expected));
        assert_eq!(routed.tree.get_rect(button).unwrap().size(), expected);
    }
    let later = routed.tree.widget_id_for_node(routed.buttons[1]).unwrap();
    assert_eq!(routed.tree.hit_test(80.0, 60.0).unwrap().widget_id, later);
    routed.pointer(crate::core::event::PointerEventKind::Down, 80.0, 60.0);
    assert_eq!(
        routed
            .domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(crate::core::event::PointerId::MOUSE),
        Some(later)
    );
    assert_eq!(routed.click_counts(), [0, 0]);
    routed.pointer(crate::core::event::PointerEventKind::Up, 80.0, 60.0);
    assert_eq!(routed.click_counts(), [0, 1]);
    routed.reverse();
    assert!(routed.frame().scene_built);
    let earlier = routed.tree.widget_id_for_node(routed.buttons[0]).unwrap();
    assert_eq!(routed.tree.hit_test(80.0, 60.0).unwrap().widget_id, earlier);
    routed.pointer(crate::core::event::PointerEventKind::Down, 80.0, 60.0);
    assert_eq!(
        routed
            .domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(crate::core::event::PointerId::MOUSE),
        Some(earlier)
    );
    assert_eq!(routed.click_counts(), [0, 1]);
    routed.pointer(crate::core::event::PointerEventKind::Up, 80.0, 60.0);
    assert_eq!(routed.click_counts(), [1, 1]);
}

#[test]
fn control_subtree_order_scrolled_clipped_hits_match_acceleration() {
    use crate::core::event::{
        FusedInputEvent, InteractionDomains, PointerButton, PointerDevice, PointerEventKind,
        PointerId, WindowId,
    };
    use crate::core::widget_tree::ControlSubtreePlacement as Placement;
    let counts = Rc::new(Cell::new(0));
    let clicks = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    let a_clicks = clicks[0].clone();
    let b_clicks = clicks[1].clone();
    let layout = |size, slots| ControlOrderLayout::new(size, slots, counts.clone(), Rc::default());
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(layout(
            Size::new(320.0, 240.0),
            vec![RectF::new(16.0, 16.0, 288.0, 208.0)],
        )),
        None,
        0,
    );
    let area = tree.add_child(
        root,
        Box::new(
            ScrollArea::new_unwired()
                .enable_horizontal_scrollbar(true)
                .with_scrollbars(false),
        ),
        None,
        0,
    );
    let mut slots = vec![RectF::new(64.0, 96.0, 360.0, 120.0); 2];
    slots.extend((0..130).map(|i| RectF::new(500.0, 300.0 + i as f32, 100.0, 20.0)));
    let host = tree.add_child(
        area,
        Box::new(layout(Size::new(640.0, 640.0), slots)),
        None,
        0,
    );
    tree.node_mut(area)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(host);
    let a = tree.add_child(
        host,
        Box::new(
            crate::widgets::button::Button::with_text("A")
                .on_click(move || a_clicks.set(a_clicks.get() + 1)),
        ),
        None,
        0,
    );
    let b = tree.add_child(
        host,
        Box::new(
            crate::widgets::button::Button::with_text("B")
                .on_click(move || b_clicks.set(b_clicks.get() + 1)),
        ),
        None,
        0,
    );
    let mut members = vec![a, b];
    for _ in 0..130 {
        members.push(tree.add_child(
            host,
            Box::new(crate::widgets::label::Label::new("outside")),
            None,
            0,
        ));
    }
    tree.set_control_subtree_order(host, &members, Placement::Main { z_index: 100 })
        .unwrap();
    let mut harness = RetainedSceneTestHarness::new();
    let mut text = FontManagerTextEngine::new();
    let mut frame = |tree: &mut WidgetTree| {
        harness.frame(
            tree,
            root,
            Constraints::tight(Size::new(320.0, 240.0)),
            &Theme::default(),
            &mut text,
            &IconRegistry::new(&[]),
        )
    };
    assert!(frame(&mut tree).scene_built);
    tree.node_mut(area)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .scroll_to(128.0, 128.0);
    tree.mark_paint_dirty(area);
    assert!(frame(&mut tree).scene_built);
    assert!(tree.node_count() > 128);
    for button in [a, b] {
        let expected = Size::new(360.0, 120.0);
        let bounds = tree
            .node(button)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::button::Button>()
            .unwrap()
            .layout_bounds;
        assert_eq!(bounds, expected);
        assert_eq!(tree.get_cached_size(button), Some(expected));
        assert_eq!(tree.get_rect(button).unwrap().size(), expected);
    }
    let mut domains = InteractionDomains::new();
    let mut router = crate::core::event_router::EventRouter::new();
    let mut bindings = crate::runtime::binding_registry::BindingRegistry::new();
    let mut pointer = |tree: &mut WidgetTree, kind, x, y| {
        let window_id = WindowId::PRIMARY;
        let time = Instant::now();
        let pos = crate::core::math::Point::new(x, y);
        let device = PointerDevice::Mouse;
        let event = match kind {
            PointerEventKind::Down => FusedInputEvent::PointerDown {
                window_id,
                time,
                pos,
                device,
                button: PointerButton::Primary,
                click_count: 1,
            },
            PointerEventKind::Up => FusedInputEvent::PointerUp {
                window_id,
                time,
                pos,
                device,
                button: PointerButton::Primary,
            },
            _ => unreachable!("control-order proof routes only Down and Up"),
        };
        assert!(domains.should_route_event(&event));
        router.route_input_event(
            tree,
            event,
            Some(root),
            &mut text,
            None,
            domains.resolve_window_mut(window_id).unwrap(),
            &mut bindings,
        );
        domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE)
    };
    let click_counts = || [clicks[0].get(), clicks[1].get()];
    let b_owner = tree.widget_id_for_node(b).unwrap();
    assert_eq!(tree.hit_test(144.0, 64.0).unwrap().widget_id, b_owner);
    assert_eq!(
        pointer(&mut tree, PointerEventKind::Down, 144.0, 64.0),
        Some(b_owner)
    );
    assert_eq!(click_counts(), [0, 0]);
    assert_eq!(pointer(&mut tree, PointerEventKind::Up, 144.0, 64.0), None);
    assert_eq!(click_counts(), [0, 1]);
    let viewport = RectF::new(0.0, 0.0, 320.0, 240.0);
    tree.rebuild_hit_test_grid(viewport);
    assert!(tree.has_current_hit_test_grid());
    let compare = |tree: &WidgetTree, x, y, expected| {
        let accelerated = tree.hit_test(x, y);
        let canonical = tree.hit_test_canonical(x, y);
        assert_eq!(
            accelerated
                .as_ref()
                .map(|h| (h.widget_id, h.local_pos, h.absolute_bounds)),
            canonical
                .as_ref()
                .map(|h| (h.widget_id, h.local_pos, h.absolute_bounds))
        );
        assert_eq!(accelerated.map(|h| h.widget_id), expected);
    };
    for (x, y) in [(32.0, 48.0), (144.0, 64.0), (272.0, 80.0)] {
        compare(&tree, x, y, Some(b_owner));
    }
    compare(&tree, 312.0, 64.0, None);
    members.swap(0, 1);
    tree.set_control_subtree_order(host, &members, Placement::Main { z_index: 100 })
        .unwrap();
    assert!(!tree.has_current_hit_test_grid());
    let a_owner = tree.widget_id_for_node(a).unwrap();
    compare(&tree, 144.0, 64.0, Some(a_owner));
    assert_eq!(
        pointer(&mut tree, PointerEventKind::Down, 144.0, 64.0),
        Some(a_owner)
    );
    assert_eq!(click_counts(), [0, 1]);
    assert_eq!(pointer(&mut tree, PointerEventKind::Up, 144.0, 64.0), None);
    assert_eq!(click_counts(), [1, 1]);
    tree.rebuild_hit_test_grid(viewport);
    assert!(tree.has_current_hit_test_grid());
    compare(&tree, 144.0, 64.0, Some(a_owner));
    compare(&tree, 312.0, 64.0, None);
    let hit = tree.hit_test(144.0, 64.0).unwrap();
    let rect = tree.get_absolute_paint_rect(a).unwrap();
    let scroll = tree.node(area).unwrap().widget.scroll_offset().unwrap();
    assert_eq!(
        hit.local_pos,
        (144.0 + scroll.0 - rect.x, 64.0 + scroll.1 - rect.y)
    );
    assert!(
        rect.x - scroll.0 < viewport.x,
        "visible button origin lies outside the grid viewport"
    );
    // Query parity uses the current grid; routed dispatch may retire it on Down.
    assert_eq!(
        pointer(&mut tree, PointerEventKind::Down, 144.0, 64.0),
        Some(a_owner)
    );
    assert_eq!(click_counts(), [1, 1]);
    assert_eq!(pointer(&mut tree, PointerEventKind::Up, 144.0, 64.0), None);
    assert_eq!(click_counts(), [2, 1]);
    assert_eq!(
        pointer(&mut tree, PointerEventKind::Down, 312.0, 64.0),
        None
    );
    assert_eq!(click_counts(), [2, 1]);
    assert_eq!(pointer(&mut tree, PointerEventKind::Up, 312.0, 64.0), None);
    assert_eq!(click_counts(), [2, 1]);
}

struct LayoutProbe {
    id: WidgetInternalId,
    layouts: Rc<Cell<u32>>,
    paints: Rc<Cell<u32>>,
}

impl LayoutProbe {
    fn new(layouts: Rc<Cell<u32>>) -> Self {
        Self::with_counts(layouts, Rc::new(Cell::new(0)))
    }

    fn with_counts(layouts: Rc<Cell<u32>>, paints: Rc<Cell<u32>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            layouts,
            paints,
        }
    }
}

impl WidgetDowncast for LayoutProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for LayoutProbe {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        self.layouts.set(self.layouts.get() + 1);
        for child_id in children {
            let child_size = cx.layout_child(*child_id, constraints);
            cx.set_child_rect(
                *child_id,
                RectF::new(0.0, 0.0, child_size.width, child_size.height),
            );
        }
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.paints.set(self.paints.get() + 1);
        cx.scene
            .push_rect(rect, Color::from_srgb(0.2, 0.4, 0.8, 1.0), 0, None);
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

struct ChannelProbe {
    id: WidgetInternalId,
    receiver: Receiver<u32>,
    applied: Rc<Cell<u32>>,
    updates: Rc<Cell<u32>>,
    layouts: Rc<Cell<u32>>,
    paints: Rc<Cell<u32>>,
}

impl WidgetDowncast for ChannelProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ChannelProbe {
    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        self.updates.set(self.updates.get() + 1);
        let mut applied = 0;
        while self.receiver.try_recv().is_ok() {
            applied += 1;
        }
        if applied > 0 {
            self.applied.set(self.applied.get() + applied);
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        true
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        self.layouts.set(self.layouts.get() + 1);
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.paints.set(self.paints.get() + 1);
        cx.scene
            .push_rect(rect, Color::from_srgb(0.1, 0.7, 0.3, 1.0), 0, None);
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

struct PaintOnlyScrollSyncProbe {
    id: WidgetInternalId,
    seen_scroll_y: Rc<Cell<f32>>,
}

impl PaintOnlyScrollSyncProbe {
    fn new(seen_scroll_y: Rc<Cell<f32>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            seen_scroll_y,
        }
    }
}

impl WidgetDowncast for PaintOnlyScrollSyncProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for PaintOnlyScrollSyncProbe {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let size = Size::new(constraints.max_width, 600.0);
        LayoutResult::new(size, size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene
            .push_rect(rect, Color::from_srgb(0.7, 0.2, 0.2, 1.0), 0, None);
    }

    fn sync_scroll(&mut self, scroll_y: f32) -> bool {
        if (self.seen_scroll_y.get() - scroll_y).abs() < 0.001 {
            return false;
        }
        self.seen_scroll_y.set(scroll_y);
        true
    }

    fn scroll_sync_invalidation(&self) -> ScrollSyncInvalidation {
        ScrollSyncInvalidation::Paint
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

struct LayoutScrollSyncProbe {
    id: WidgetInternalId,
    seen_scroll_y: Rc<Cell<f32>>,
    content_height: Rc<Cell<f32>>,
    skip_next_sync: Rc<Cell<bool>>,
}

impl LayoutScrollSyncProbe {
    fn new(
        seen_scroll_y: Rc<Cell<f32>>,
        content_height: Rc<Cell<f32>>,
        skip_next_sync: Rc<Cell<bool>>,
    ) -> Self {
        Self {
            id: WidgetInternalId::new(),
            seen_scroll_y,
            content_height,
            skip_next_sync,
        }
    }
}

impl WidgetDowncast for LayoutScrollSyncProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for LayoutScrollSyncProbe {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let width = constraints.max_width;
        let content_height = self.content_height.get();
        for child_id in children {
            let child_size = cx.layout_child(*child_id, Constraints::tight(Size::new(width, 24.0)));
            cx.set_child_rect(
                *child_id,
                RectF::new(0.0, 400.0, child_size.width, child_size.height),
            );
        }
        LayoutResult::new(
            Size::new(width, content_height),
            Size::new(width, content_height),
        )
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene
            .push_rect(rect, Color::from_srgb(0.3, 0.3, 0.8, 1.0), 0, None);
    }

    fn sync_scroll(&mut self, scroll_y: f32) -> bool {
        if self.skip_next_sync.get() {
            self.skip_next_sync.set(false);
            return false;
        }
        if (self.seen_scroll_y.get() - scroll_y).abs() < 0.001 {
            return false;
        }
        self.seen_scroll_y.set(scroll_y);
        true
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

#[test]
fn scrollarea_resize_final_allocation_reaches_nested_viewport() {
    use crate::core::app::{FrameReason, FrameTime};
    use crate::core::event::{InteractionDomains, WindowId};
    use crate::core::event_router::EventRouter;
    use crate::core::input_fusion::InputFusion;
    use crate::core::layout::FlexParams;
    use crate::core::scene::SceneNode;
    use crate::widgets::button::Button;
    use crate::widgets::fixed_spacer::FixedSpacer;
    use crate::widgets::flex::Flex;

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    tree.add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
    let nested = tree.add_child(root, Box::new(Flex::column()), None, 1);
    tree.set_flex(nested, FlexParams::flexible(1.0));
    let scroll = tree.add_child(
        nested,
        Box::new(ScrollArea::new_unwired().enable_horizontal_scrollbar(true)),
        None,
        0,
    );
    tree.set_flex(scroll, FlexParams::flexible(1.0));
    let content = tree.add_child(scroll, Box::new(Flex::column()), None, 0);
    tree.add_child(content, Box::new(Button::with_text("Run")), None, 0);
    tree.add_child(content, Box::new(FixedSpacer::vertical(600.0)), None, 1);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(content);

    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let mut text = FontManagerTextEngine::new();
    let mut harness = RetainedSceneTestHarness::new();
    let scroll_widget = tree.node(scroll).unwrap().id;
    let sizes = [
        Size::new(320.0, 180.0),
        Size::new(260.0, 120.0),
        Size::new(300.0, 200.0),
        Size::new(208.0, 24.0),
        Size::new(320.0, 180.0),
    ];
    for size in sizes {
        // Catch only to report the completed Layout state if the real overlay panics.
        let painted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            harness.frame(
                &mut tree,
                root,
                Constraints::tight(size),
                &theme,
                &mut text,
                &icons,
            )
        }));
        let rect = tree.get_rect(scroll).unwrap();
        let absolute = tree.get_absolute_paint_rect(scroll).unwrap();
        let area = tree
            .node(scroll)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ScrollArea>()
            .unwrap();
        let container = area.clip_bounds().unwrap();
        let domain = area.scroll_container().unwrap();
        eprintln!(
            "resize={size:?} root={root:?} nested={nested:?} nested_rect={:?} nested_constraints={:?} scroll={scroll:?} rect={rect:?} absolute={absolute:?} scroll_constraints={:?} container={container:?} viewport={:?} content={:?} max={:?} overlay_ok={}",
            tree.get_rect(nested),
            tree.node(nested).unwrap().last_constraints,
            tree.node(scroll).unwrap().last_constraints,
            domain.viewport_size,
            domain.content_size,
            domain.max_scroll(),
            painted.is_ok(),
        );
        assert!(
            painted.is_ok(),
            "real retained ScrollArea overlay must complete"
        );
        assert!(painted.unwrap().scene_built);
        for node_id in [nested, scroll] {
            let rect = tree.get_rect(node_id).unwrap();
            assert_eq!(
                tree.node(node_id).unwrap().last_constraints,
                Some(Constraints::tight(Size::new(rect.width, rect.height))),
                "final allocation must reach {node_id:?}"
            );
        }
        assert_eq!(absolute.width, rect.width);
        assert_eq!(absolute.height, rect.height);
        assert_eq!(container.width, rect.width);
        assert_eq!(container.height, rect.height);
        assert_eq!(domain.viewport_size, Size::new(rect.width, rect.height));
        assert!(domain.content_size.width <= rect.width);
        assert!(domain.content_size.height > rect.height);
        assert_eq!(domain.max_scroll().0, 0.0);
        assert_eq!(area.target_scroll_x(), 0.0);
        assert_eq!(area.horizontal_scroll_offset(), 0.0);
        let overlay = harness.overlay_fragment(scroll_widget).unwrap();
        assert!(
            overlay.iter().any(|node| matches!(
                node,
                SceneNode::Rect(thumb)
                    if thumb.rect.x + thumb.rect.width == absolute.x + absolute.width
            )),
            "painted scrollbar must use the current container edge"
        );
    }

    // Normal update frames finish the scrollbar hide/fade before testing reuse.
    let mut domains = InteractionDomains::new();
    let mut router = EventRouter::new();
    let mut fusion = InputFusion::new();
    let now = Instant::now();
    for frame_index in 1..=2 {
        test_begin_frame_layout_tracking();
        harness.frame_with_gesture_sessions(
            &mut tree,
            root,
            Constraints::tight(*sizes.last().unwrap()),
            &theme,
            &mut text,
            &icons,
            &FrameTime {
                now: now + Duration::from_secs(2 * frame_index),
                dt: Duration::from_secs(2),
                frame_index,
                reason: FrameReason::Animation,
            },
            None,
            &mut domains,
            WindowId::PRIMARY,
            &mut router,
            &mut fusion,
        );
    }
    assert!(!tree.node(scroll).unwrap().widget.needs_animation());
    let epoch = current_layout_epoch();
    let completed_rect = tree.get_absolute_paint_rect(scroll).unwrap();
    let overlay = harness.overlay_fragment(scroll_widget).unwrap();
    let settled = harness.frame_with_appearance(
        &mut tree,
        root,
        Constraints::tight(*sizes.last().unwrap()),
        &theme,
        &mut text,
        &icons,
        &domains,
        WindowId::PRIMARY,
    );
    assert!(!settled.scene_built);
    assert!(!settled.visual_changed);
    assert_eq!(current_layout_epoch(), epoch);
    assert_eq!(tree.get_absolute_paint_rect(scroll), Some(completed_rect));
    assert!(Arc::ptr_eq(
        &overlay,
        &harness.overlay_fragment(scroll_widget).unwrap()
    ));
}

#[test]
fn clean_root_and_paint_only_invalidation_reuse_completed_layout_geometry() {
    let layouts = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(LayoutProbe::new(layouts.clone())), None, 0);
    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();

    test_begin_frame_layout_tracking();
    {
        let _layout = LayoutAuthorityGuard::enter();
        assert_eq!(
            layout_tree(&mut tree, root, constraints, &theme, &mut text, None,),
            Size::new(320.0, 180.0)
        );
    }
    assert_eq!(layouts.get(), 1);
    let completed_epoch = current_layout_epoch();

    tree.mark_paint_dirty(root);
    assert!(!tree.node(root).unwrap().dirty_layout);
    assert!(tree.node(root).unwrap().dirty_paint);

    {
        let _layout = LayoutAuthorityGuard::enter();
        assert_eq!(
            layout_tree(&mut tree, root, constraints, &theme, &mut text, None,),
            Size::new(320.0, 180.0)
        );
    }

    assert_eq!(layouts.get(), 1);
    assert_eq!(current_layout_epoch(), completed_epoch);
}

#[test]
fn paint_invalidation_reaches_ancestor_composition_without_dirtying_sibling_layout() {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(LayoutProbe::new(Rc::new(Cell::new(0)))), None, 0);
    let changed = tree.add_child(
        root,
        Box::new(LayoutProbe::new(Rc::new(Cell::new(0)))),
        None,
        0,
    );
    let sibling = tree.add_child(
        root,
        Box::new(LayoutProbe::new(Rc::new(Cell::new(0)))),
        None,
        0,
    );

    for node_id in [root, changed, sibling] {
        let node = tree.node_mut(node_id).unwrap();
        node.dirty_layout = false;
        node.dirty_paint = false;
        node.dirty_composition = false;
    }

    tree.mark_paint_dirty(changed);

    let root_node = tree.node(root).unwrap();
    assert!(!root_node.dirty_layout);
    assert!(!root_node.dirty_paint);
    assert!(root_node.dirty_composition);

    let changed_node = tree.node(changed).unwrap();
    assert!(!changed_node.dirty_layout);
    assert!(changed_node.dirty_paint);
    assert!(changed_node.dirty_composition);

    let sibling_node = tree.node(sibling).unwrap();
    assert!(!sibling_node.dirty_layout);
    assert!(!sibling_node.dirty_paint);
    assert!(!sibling_node.dirty_composition);
}

#[test]
fn post_layout_scroll_sync_can_invalidate_paint_without_dirtying_completed_layout() {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(LayoutProbe::new(Rc::new(Cell::new(0)))), None, 0);
    let scroll = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
    let seen_scroll_y = Rc::new(Cell::new(0.0));
    let content = tree.add_child(
        scroll,
        Box::new(PaintOnlyScrollSyncProbe::new(seen_scroll_y.clone())),
        None,
        0,
    );
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(content);

    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    test_begin_frame_layout_tracking();
    {
        let _layout = LayoutAuthorityGuard::enter();
        layout_tree(&mut tree, root, constraints, &theme, &mut text, None);
    }
    for node_id in [root, scroll, content] {
        assert!(!tree.node(node_id).unwrap().dirty_layout);
    }

    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .jump_to_y(120.0);
    post_layout_sync_scroll_to_lists(&mut tree);

    assert_eq!(seen_scroll_y.get(), 120.0);
    assert!(!tree.node(root).unwrap().dirty_layout);
    assert!(!tree.node(scroll).unwrap().dirty_layout);
    assert!(!tree.node(content).unwrap().dirty_layout);
    assert!(tree.node(root).unwrap().dirty_composition);
    assert!(tree.node(content).unwrap().dirty_paint);
}

#[test]
fn retained_scene_defers_paint_when_post_layout_scroll_sync_requests_layout() {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(LayoutProbe::new(Rc::new(Cell::new(0)))), None, 0);
    let scroll = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
    let seen_scroll_y = Rc::new(Cell::new(0.0));
    let content_height = Rc::new(Cell::new(600.0));
    let skip_next_sync = Rc::new(Cell::new(false));
    let content = tree.add_child(
        scroll,
        Box::new(LayoutScrollSyncProbe::new(
            seen_scroll_y.clone(),
            content_height.clone(),
            skip_next_sync.clone(),
        )),
        None,
        0,
    );
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(content);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .test_set_final_layout_anchor_metrics(
            Size::new(300.0, 180.0),
            Size::new(300.0, 180.0),
            Size::new(300.0, 600.0),
        );
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .jump_to_y(120.0);

    let mut harness = RetainedSceneTestHarness::new();
    let constraints = Constraints::tight(Size::new(300.0, 180.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);

    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(first.scene_built);
    assert!(!first.needs_next_frame);
    assert!(!tree.node(root).unwrap().dirty_layout);
    assert!((seen_scroll_y.get() - 120.0).abs() < 0.001);

    seen_scroll_y.set(-1.0);
    skip_next_sync.set(true);

    let deferred = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(!deferred.scene_built);
    assert!(!deferred.visual_changed);
    assert!(deferred.needs_next_frame);
    assert!(tree.node(root).unwrap().dirty_layout);
    assert!((seen_scroll_y.get() - 120.0).abs() < 0.001);

    let settled = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(settled.scene_built);
    assert!(settled.visual_changed);
    assert!(!settled.needs_next_frame);
    assert!(!tree.node(root).unwrap().dirty_layout);
}

#[test]
fn smooth_scroll_target_keeps_engine_animation_active_and_invalidates_its_fragment() {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(LayoutProbe::new(Rc::new(Cell::new(0)))), None, 0);
    let scroll = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
    let content = tree.add_child(
        scroll,
        Box::new(PaintOnlyScrollSyncProbe::new(Rc::new(Cell::new(0.0)))),
        None,
        0,
    );
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(content);

    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    test_begin_frame_layout_tracking();
    {
        let _layout = LayoutAuthorityGuard::enter();
        layout_tree(&mut tree, root, constraints, &theme, &mut text, None);
    }

    {
        let mut cx = UpdateContext::new(None);
        let scroll_widget = tree
            .node_mut(scroll)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap();
        scroll_widget.update(2.0, &mut cx);
        scroll_widget.update(1.0, &mut cx);
    }
    assert!(!tree.any_widget_needs_update());

    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .scroll_to_y(120.0);

    assert!(tree.any_scroll_needs_animation());
    assert!(!tree.any_widget_needs_update());

    for node_id in [root, scroll, content] {
        let node = tree.node_mut(node_id).unwrap();
        node.dirty_paint = false;
        node.dirty_composition = false;
    }
    {
        let _update = UpdateAuthorityGuard::enter();
        tree.update_scroll_animations(1.0 / 60.0);
    }

    let scroll_node = tree.node(scroll).unwrap();
    assert!(!scroll_node.dirty_layout);
    assert!(scroll_node.dirty_paint);
    assert!(tree.node(root).unwrap().dirty_composition);
}

#[test]
fn update_context_distinguishes_paint_from_layout_invalidation() {
    let mut cx = UpdateContext::new(None);

    cx.begin_widget_update();
    cx.invalidate_paint();
    assert_eq!(cx.take_widget_invalidations(), (true, false, false));

    cx.begin_widget_update();
    cx.invalidate_paint_subtree();
    assert_eq!(cx.take_widget_invalidations(), (true, true, false));

    cx.begin_widget_update();
    cx.invalidate_layout();
    assert_eq!(cx.take_widget_invalidations(), (true, false, true));
}

#[test]
fn retained_scene_repaints_only_the_dirty_widget_and_recomposes_its_ancestors() {
    let root_layouts = Rc::new(Cell::new(0));
    let root_paints = Rc::new(Cell::new(0));
    let changed_layouts = Rc::new(Cell::new(0));
    let changed_paints = Rc::new(Cell::new(0));
    let sibling_layouts = Rc::new(Cell::new(0));
    let sibling_paints = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(LayoutProbe::with_counts(
            root_layouts.clone(),
            root_paints.clone(),
        )),
        None,
        0,
    );
    let changed = tree.add_child(
        root,
        Box::new(LayoutProbe::with_counts(
            changed_layouts.clone(),
            changed_paints.clone(),
        )),
        None,
        0,
    );
    tree.add_child(
        root,
        Box::new(LayoutProbe::with_counts(
            sibling_layouts.clone(),
            sibling_paints.clone(),
        )),
        None,
        0,
    );

    let mut harness = RetainedSceneTestHarness::new();
    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);

    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(first.visual_changed);
    assert!(first.scene_built);
    assert_eq!(first.cache_entries, 3);
    assert_eq!(root_layouts.get(), 1);
    assert_eq!(changed_layouts.get(), 1);
    assert_eq!(sibling_layouts.get(), 1);
    assert_eq!(root_paints.get(), 1);
    assert_eq!(changed_paints.get(), 1);
    assert_eq!(sibling_paints.get(), 1);

    tree.mark_paint_dirty(changed);
    let second = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(second.visual_changed);
    assert!(second.scene_built);
    assert_eq!(second.cache_entries, 3);
    assert_eq!(root_layouts.get(), 1);
    assert_eq!(changed_layouts.get(), 1);
    assert_eq!(sibling_layouts.get(), 1);
    assert_eq!(root_paints.get(), 1);
    assert_eq!(changed_paints.get(), 2);
    assert_eq!(sibling_paints.get(), 1);
}

fn selector_focus_projection(
    nodes: &[crate::core::scene::SceneNode],
) -> (
    Vec<(crate::core::scene::BorderRectNode, Vec<RectF>)>,
    Vec<(crate::core::scene::TextNode, Vec<RectF>)>,
    Vec<RectF>,
) {
    use crate::core::scene::SceneNode;

    fn visit(
        nodes: &[SceneNode],
        clips: &mut Vec<RectF>,
        borders: &mut Vec<(crate::core::scene::BorderRectNode, Vec<RectF>)>,
        labels: &mut Vec<(crate::core::scene::TextNode, Vec<RectF>)>,
        indicators: &mut Vec<RectF>,
    ) {
        for node in nodes {
            match node {
                SceneNode::RetainedFragment(fragment) => {
                    visit(&fragment.children, clips, borders, labels, indicators);
                }
                SceneNode::Clip(clip) => {
                    clips.push(clip.rect);
                    visit(&clip.children, clips, borders, labels, indicators);
                    clips.pop();
                }
                SceneNode::BorderRect(border)
                    if border.z_index == crate::core::paint::LAYER_FOCUS =>
                {
                    borders.push((border.clone(), clips.clone()));
                }
                SceneNode::Text(label) => labels.push((label.clone(), clips.clone())),
                SceneNode::Rect(indicator) if indicator.z_index == 2 => {
                    indicators.push(indicator.rect);
                }
                SceneNode::RoundedRect(indicator) if indicator.z_index == 2 => {
                    indicators.push(indicator.rect);
                }
                _ => {}
            }
        }
    }

    let mut borders = Vec::new();
    let mut labels = Vec::new();
    let mut indicators = Vec::new();
    visit(
        nodes,
        &mut Vec::new(),
        &mut borders,
        &mut labels,
        &mut indicators,
    );
    (borders, labels, indicators)
}

#[test]
fn selector_indicator_only_appearance_preserves_focus_and_values() {
    use crate::core::event::{InteractionDomains, UiInteractionPolicy, WindowId};
    use crate::core::event_router::EventRouter;
    use crate::core::layout::{Alignment, EdgeInsets};
    use crate::core::widget_tree::OverflowMode;
    use crate::widgets::checkbox::Checkbox;
    use crate::widgets::flex::Flex;
    use crate::widgets::label::Label;
    use crate::widgets::switch::{LabelSide, Switch};

    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(
            Flex::column()
                .with_align_items(Alignment::Stretch)
                .with_spacing(16.0)
                .with_padding(EdgeInsets::new(8.0, 8.0, 8.0, 8.0)),
        ),
        None,
        0,
    );
    tree.set_overflow(root, OverflowMode::Clip);
    let changes = Rc::new(Cell::new(0));
    let checkbox_changes = changes.clone();
    let checkbox = tree.add_child(
        root,
        Box::new(
            Checkbox::checked(true)
                .with_label("Audit trail")
                .on_toggle_changed(move |_| checkbox_changes.set(checkbox_changes.get() + 1)),
        ),
        None,
        0,
    );
    let switch_changes = changes.clone();
    let switch = tree.add_child(
        root,
        Box::new(
            Switch::on(true)
                .with_label("Dark")
                .with_label_side(LabelSide::Leading)
                .on_toggle_changed(move |_| switch_changes.set(switch_changes.get() + 1)),
        ),
        None,
        1,
    );
    let sibling = tree.add_child(root, Box::new(Label::new("Unchanged peer")), None, 2);
    let nodes = [root, checkbox, switch, sibling];
    let owners = nodes.map(|node| tree.node(node).unwrap().id);
    let geometry = |tree: &WidgetTree| {
        nodes.map(|id| {
            let node = tree.node(id).unwrap();
            (
                node.id,
                node.widget.id(),
                tree.get_absolute_paint_rect(id).unwrap(),
                node.layout_epoch,
            )
        })
    };
    let semantic = |tree: &WidgetTree| {
        let checkbox = tree
            .node(checkbox)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Checkbox>()
            .unwrap()
            .state();
        let switch = tree
            .node(switch)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Switch>()
            .unwrap()
            .state();
        (checkbox.clone(), switch.clone())
    };
    let theme = Theme::dark();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let mut harness = RetainedSceneTestHarness::new();
    let mut domains = InteractionDomains::new();
    let mut router = EventRouter::new();
    let frame = |tree: &mut WidgetTree,
                 harness: &mut RetainedSceneTestHarness,
                 text: &mut FontManagerTextEngine,
                 domains: &InteractionDomains| {
        harness.frame_with_appearance(
            tree,
            root,
            Constraints::tight(Size::new(320.0, 180.0)),
            &theme,
            text,
            &icons,
            domains,
            WindowId::PRIMARY,
        )
    };
    let local = |harness: &RetainedSceneTestHarness, index: usize| {
        harness.local_fragment(owners[index]).unwrap()
    };
    let labels = |nodes: &[crate::core::scene::SceneNode]| {
        selector_focus_projection(nodes)
            .1
            .into_iter()
            .map(|(label, clips)| (label.text, label.position, label.font_size, clips))
            .collect::<Vec<_>>()
    };
    let initial = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(initial.scene_built && !initial.needs_next_frame);
    let projection = selector_focus_projection(&initial.scene_root);
    assert!(projection.0.is_empty());
    assert_eq!(projection.2.len(), 2);
    let labels_before = labels(&initial.scene_root);
    let geometry_before = geometry(&tree);
    let root_local = local(&harness, 0);
    let sibling_local = local(&harness, 3);
    let local_before = [local(&harness, 1), local(&harness, 2)];
    let hit_points = [checkbox, switch].map(|node| {
        let rect = tree.get_absolute_paint_rect(node).unwrap();
        (rect.x + 2.0, rect.y + rect.height * 0.5)
    });
    let hits = |tree: &WidgetTree| {
        hit_points.map(|(x, y)| {
            let hit = tree.hit_test(x, y).unwrap();
            (hit.widget_id, hit.local_pos, hit.absolute_bounds)
        })
    };
    let hits_before = hits(&tree);
    assert_eq!(hits_before.map(|hit| hit.0), [owners[1], owners[2]]);
    let indicator_primitives = |nodes: &[crate::core::scene::SceneNode]| {
        use crate::core::scene::SceneNode;
        (
            nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(value) => Some((value.rect, value.color, value.z_index)),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::RoundedRect(value) => {
                        Some((value.rect, value.color, value.radius, value.z_index))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(value) => Some((
                        value.rect,
                        value.fill_color,
                        value.border_color,
                        value.border_width,
                        value.radius,
                        value.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Circle(value) => {
                        Some((value.center, value.radius, value.color, value.z_index))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Line(value) => {
                        Some((value.a, value.b, value.width, value.color, value.z_index))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )
    };
    let indicators_before = local_before
        .each_ref()
        .map(|nodes| indicator_primitives(nodes));
    assert_eq!(indicators_before[0].4.len(), 2);
    assert_eq!(indicators_before[1].3.len(), 1);
    assert_eq!(indicators_before[1].2.len(), 1);
    let check_indicators = |harness: &RetainedSceneTestHarness| {
        for (index, expected) in indicators_before.iter().enumerate() {
            assert_eq!(indicator_primitives(&local(harness, index + 1)), *expected);
        }
    };
    router.request_focus_for_widget(&mut tree, owners[1], domains.primary_state_mut());
    assert_eq!(
        domains.primary_state().focus_manager.focused(),
        Some(owners[1])
    );
    let before_paint = semantic(&tree);
    let entered = frame(&mut tree, &mut harness, &mut text, &domains);
    let cues = selector_focus_projection(&entered.scene_root).0;
    assert!(entered.scene_built && !entered.needs_next_frame);
    assert!(cues.is_empty());
    check_indicators(&harness);
    assert_eq!(semantic(&tree), before_paint);
    assert_eq!(labels(&entered.scene_root), labels_before);
    assert!(!Arc::ptr_eq(&local_before[0], &local(&harness, 1)));
    assert!(Arc::ptr_eq(&local_before[1], &local(&harness, 2)));
    let focused_checkbox = local(&harness, 1);
    let settled = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(!settled.scene_built && !settled.needs_next_frame);
    assert!(Arc::ptr_eq(&focused_checkbox, &local(&harness, 1)));

    router.request_focus_for_widget(&mut tree, owners[2], domains.primary_state_mut());
    assert_eq!(
        domains.primary_state().focus_manager.focused(),
        Some(owners[2])
    );
    let before_paint = semantic(&tree);
    let moved = frame(&mut tree, &mut harness, &mut text, &domains);
    let cues = selector_focus_projection(&moved.scene_root).0;
    assert!(moved.scene_built && !moved.needs_next_frame);
    assert!(cues.is_empty());
    check_indicators(&harness);
    assert_eq!(semantic(&tree), before_paint);
    assert_eq!(labels(&moved.scene_root), labels_before);
    assert!(!Arc::ptr_eq(&focused_checkbox, &local(&harness, 1)));
    assert!(!Arc::ptr_eq(&local_before[1], &local(&harness, 2)));
    assert!(selector_focus_projection(&local(&harness, 1)).0.is_empty());
    let focused_switch = local(&harness, 2);
    tree.mark_paint_dirty(switch);
    let repainted = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(repainted.scene_built && !repainted.needs_next_frame);
    assert!(
        selector_focus_projection(&repainted.scene_root)
            .0
            .is_empty()
    );
    check_indicators(&harness);
    assert_eq!(semantic(&tree), before_paint);
    assert_eq!(labels(&repainted.scene_root), labels_before);
    assert!(!Arc::ptr_eq(&focused_switch, &local(&harness, 2)));
    let focused_switch = local(&harness, 2);
    let unfocused_checkbox = local(&harness, 1);
    let settled = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(!settled.scene_built && !settled.needs_next_frame);
    assert!(Arc::ptr_eq(&focused_switch, &local(&harness, 2)));

    router.clear_focus(&mut tree, domains.primary_state_mut());
    assert_eq!(domains.primary_state().focus_manager.focused(), None);
    let lost = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(lost.scene_built && !lost.needs_next_frame);
    assert!(selector_focus_projection(&lost.scene_root).0.is_empty());
    check_indicators(&harness);
    assert_eq!(labels(&lost.scene_root), labels_before);
    assert!(Arc::ptr_eq(&unfocused_checkbox, &local(&harness, 1)));
    assert!(!Arc::ptr_eq(&focused_switch, &local(&harness, 2)));
    let lost_switch = local(&harness, 2);
    let settled = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(!settled.scene_built && !settled.needs_next_frame);
    assert!(Arc::ptr_eq(&lost_switch, &local(&harness, 2)));

    let mut pointer_domains =
        InteractionDomains::with_ui_interaction_policy(UiInteractionPolicy::PointerDriven);
    router.request_focus_for_widget(&mut tree, owners[2], pointer_domains.primary_state_mut());
    let pointer = frame(&mut tree, &mut harness, &mut text, &pointer_domains);
    assert!(pointer.scene_built && !pointer.needs_next_frame);
    assert_eq!(
        pointer_domains.primary_state().focus_manager.focused(),
        Some(owners[2])
    );
    assert!(selector_focus_projection(&pointer.scene_root).0.is_empty());
    check_indicators(&harness);
    assert_eq!(labels(&pointer.scene_root), labels_before);
    for node in [checkbox, switch] {
        assert!(!tree.node(node).unwrap().widget.needs_update());
        assert!(!tree.node(node).unwrap().widget.needs_animation());
    }
    assert_eq!(geometry(&tree), geometry_before);
    assert_eq!(hits(&tree), hits_before);
    assert!(
        tree.node(checkbox)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Checkbox>()
            .unwrap()
            .is_checked()
    );
    assert!(
        tree.node(switch)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Switch>()
            .unwrap()
            .is_on()
    );
    assert_eq!(changes.get(), 0);
    assert!(Arc::ptr_eq(&root_local, &local(&harness, 0)));
    assert!(Arc::ptr_eq(&sibling_local, &local(&harness, 3)));
}

#[test]
fn radio_presentation_update_replaces_only_the_owner_in_persistent_retained_frames() {
    use crate::core::event::{InteractionDomains, WindowId};
    use crate::core::event_router::EventRouter;
    use crate::core::scene::SceneNode;
    use crate::widgets::radio_button::RadioButton;
    use crate::widgets::radio_group::RadioId;
    use crate::widgets::radio_group_widget::{RadioGroup, RadioGroupId};

    fn composition(frame: &crate::core::app::RetainedSceneTestFrame) -> Arc<[SceneNode]> {
        let SceneNode::RetainedFragment(root) = &frame.scene_root[0] else {
            panic!()
        };
        root.children.clone()
    }

    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(TypographyLayout {
            id: WidgetInternalId::new(),
        }),
        None,
        0,
    );
    let changes = Rc::new(Cell::new(0));
    let selection_changes = changes.clone();
    let group = tree.add_child(
        root,
        Box::new(
            RadioGroup::with_labeled_options(
                RadioGroupId(1),
                &[(RadioId(1), "First"), (RadioId(2), "Second")],
            )
            .on_selection_changed(move |_, _| selection_changes.set(selection_changes.get() + 1)),
        ),
        None,
        0,
    );
    let button = tree.add_child(
        root,
        Box::new(RadioButton::with_label(RadioId(3), "Standalone")),
        None,
        0,
    );
    let sibling_paints = Rc::new(Cell::new(0));
    let sibling = tree.add_child(
        root,
        Box::new(LayoutProbe::with_counts(
            Rc::new(Cell::new(0)),
            sibling_paints.clone(),
        )),
        None,
        0,
    );
    let nodes = [root, group, button, sibling];
    let owners = nodes.map(|node| tree.node(node).unwrap().id);
    let geometry = |tree: &WidgetTree| {
        nodes.map(|node_id| {
            let node = tree.node(node_id).unwrap();
            (
                node.id,
                node.widget.id(),
                tree.get_rect(node_id).unwrap(),
                node.layout_epoch,
            )
        })
    };
    let mut harness = RetainedSceneTestHarness::new();
    let mut domains = InteractionDomains::new();
    let mut router = EventRouter::new();
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let frame = |tree: &mut WidgetTree,
                 harness: &mut RetainedSceneTestHarness,
                 text: &mut FontManagerTextEngine,
                 domains: &InteractionDomains| {
        harness.frame_with_appearance(
            tree,
            root,
            Constraints::tight(Size::new(320.0, 280.0)),
            &theme,
            text,
            &icons,
            domains,
            WindowId::PRIMARY,
        )
    };
    let local = |harness: &RetainedSceneTestHarness, index: usize| {
        harness.local_fragment(owners[index]).unwrap()
    };

    harness.advance_transition_time(0.0);
    assert!(frame(&mut tree, &mut harness, &mut text, &domains).scene_built);
    let geometry_before = geometry(&tree);
    let root_local = local(&harness, 0);
    let sibling_local = local(&harness, 3);
    let group_idle = local(&harness, 1);
    let standalone_before = local(&harness, 2);
    let indicator_borders = |nodes: &[SceneNode]| {
        nodes
            .iter()
            .filter_map(|node| match node {
                SceneNode::RoundedRect(circle)
                    if circle.z_index == 2 && circle.rect.width == theme.geometry.checkbox.size =>
                {
                    Some((circle.rect, circle.color))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let resting_borders = indicator_borders(&group_idle);
    assert_eq!(resting_borders.len(), 2);
    assert!(
        resting_borders
            .iter()
            .all(|(_, color)| *color == theme.colors.borders.default.into())
    );
    router.request_focus_for_widget(&mut tree, owners[1], domains.primary_state_mut());
    assert_eq!(
        domains.primary_state().focus_manager.focused(),
        Some(owners[1])
    );
    let group_state = |tree: &WidgetTree| {
        let state = tree
            .node(group)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<RadioGroup>()
            .unwrap()
            .state();
        (state.selected, state.disabled, state.focused, state.active)
    };
    let state_before_paint = group_state(&tree);
    let entered = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(entered.scene_built && entered.visual_changed && !entered.needs_next_frame);
    assert_eq!(group_state(&tree), state_before_paint);
    let cues = selector_focus_projection(&entered.scene_root).0;
    assert!(cues.is_empty());
    let focused_borders = indicator_borders(&local(&harness, 1));
    assert_eq!(
        focused_borders,
        vec![
            (resting_borders[0].0, theme.colors.borders.focus.into()),
            resting_borders[1],
        ]
    );
    assert!(!tree.node(group).unwrap().widget.needs_update());
    assert!(!Arc::ptr_eq(&group_idle, &local(&harness, 1)));
    assert!(Arc::ptr_eq(&standalone_before, &local(&harness, 2)));
    assert!(Arc::ptr_eq(&root_local, &local(&harness, 0)));
    assert!(Arc::ptr_eq(&sibling_local, &local(&harness, 3)));
    assert_eq!(geometry(&tree), geometry_before);
    let focused_group = local(&harness, 1);
    let settled = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(!settled.scene_built && !settled.visual_changed && !settled.needs_next_frame);
    assert!(Arc::ptr_eq(&focused_group, &local(&harness, 1)));
    tree.mark_paint_dirty(group);
    let repainted = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(repainted.scene_built && !repainted.needs_next_frame);
    assert_eq!(group_state(&tree), state_before_paint);
    assert!(
        selector_focus_projection(&repainted.scene_root)
            .0
            .is_empty()
    );
    assert_eq!(indicator_borders(&local(&harness, 1)), focused_borders);
    let focused_group = local(&harness, 1);
    router.clear_focus(&mut tree, domains.primary_state_mut());
    assert_eq!(domains.primary_state().focus_manager.focused(), None);
    let lost = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(lost.scene_built && !lost.needs_next_frame);
    assert!(selector_focus_projection(&lost.scene_root).0.is_empty());
    assert_eq!(indicator_borders(&local(&harness, 1)), resting_borders);
    assert!(!Arc::ptr_eq(&focused_group, &local(&harness, 1)));
    assert!(Arc::ptr_eq(&standalone_before, &local(&harness, 2)));
    assert_eq!(group_state(&tree), (None, false, None, None));
    assert!(!tree.node(group).unwrap().widget.needs_update());
    let settled_group = local(&harness, 1);
    let settled = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(!settled.scene_built && !settled.visual_changed && !settled.needs_next_frame);
    assert!(Arc::ptr_eq(&settled_group, &local(&harness, 1)));

    harness.advance_transition_time(0.10);
    router.update_hover(
        &mut tree,
        Some(owners[2]),
        Some(harness.animation_context()),
        domains.primary_state_mut(),
    );
    let hovered = frame(&mut tree, &mut harness, &mut text, &domains);
    let hovered_composition = composition(&hovered);
    let button_before = local(&harness, 2);
    assert!(tree.node(button).unwrap().widget.needs_update());
    harness.advance_transition_time(0.14);
    let changed = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(changed.scene_built && changed.visual_changed);
    assert!(!Arc::ptr_eq(&button_before, &local(&harness, 2)));
    assert!(!Arc::ptr_eq(&hovered_composition, &composition(&changed)));
    assert!(Arc::ptr_eq(&settled_group, &local(&harness, 1)));
    assert!(Arc::ptr_eq(&sibling_local, &local(&harness, 3)));
    assert!(Arc::ptr_eq(&root_local, &local(&harness, 0)));
    assert_eq!(geometry(&tree), geometry_before);
    harness.advance_transition_time(0.18);
    assert!(frame(&mut tree, &mut harness, &mut text, &domains).visual_changed);
    assert!(!tree.node(button).unwrap().widget.needs_update());
    let terminal_button = local(&harness, 2);
    harness.advance_transition_time(0.20);
    let settled = frame(&mut tree, &mut harness, &mut text, &domains);
    assert!(!settled.scene_built && !settled.visual_changed);
    assert!(Arc::ptr_eq(&terminal_button, &local(&harness, 2)));
    assert_eq!(geometry(&tree), geometry_before);
    assert_eq!(sibling_paints.get(), 1);
    assert_eq!(group_state(&tree), (None, false, None, None));
    assert_eq!(changes.get(), 0);
    let standalone = tree
        .node(button)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<RadioButton>()
        .unwrap();
    assert_eq!(standalone.radio_id(), RadioId(3));
    assert_eq!(standalone.label(), Some("Standalone"));
}

#[test]
fn unchanged_retained_frame_skips_scene_assembly_and_cache_is_bounded_by_tree_topology() {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(LayoutProbe::new(Rc::new(Cell::new(0)))), None, 0);
    let removed = tree.add_child(
        root,
        Box::new(LayoutProbe::new(Rc::new(Cell::new(0)))),
        None,
        0,
    );
    let mut harness = RetainedSceneTestHarness::new();
    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);

    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(first.scene_built);
    assert_eq!(first.cache_entries, 2);

    let unchanged = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(!unchanged.visual_changed);
    assert!(!unchanged.scene_built);
    assert_eq!(unchanged.cache_entries, 2);

    tree.remove_node(removed);
    tree.mark_layout_dirty(root);
    let after_removal = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(after_removal.visual_changed);
    assert!(after_removal.scene_built);
    assert_eq!(after_removal.cache_entries, 1);
}

#[test]
fn burst_data_publications_coalesce_into_one_paint_generation_without_layout() {
    let (sender, receiver) = channel();
    let applied = Rc::new(Cell::new(0));
    let updates = Rc::new(Cell::new(0));
    let layouts = Rc::new(Cell::new(0));
    let paints = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(ChannelProbe {
            id: WidgetInternalId::new(),
            receiver,
            applied: applied.clone(),
            updates,
            layouts: layouts.clone(),
            paints: paints.clone(),
        }),
        None,
        0,
    );
    assert!(tree.any_widget_needs_update());
    assert!(!tree.any_widget_needs_animation());
    let mut harness = RetainedSceneTestHarness::new();
    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);

    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(first.visual_changed);
    assert_eq!(layouts.get(), 1);
    assert_eq!(paints.get(), 1);

    for value in 0..440 {
        sender.send(value).unwrap();
    }
    let burst = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(burst.visual_changed);
    assert!(burst.scene_built);
    assert_eq!(applied.get(), 440);
    assert_eq!(layouts.get(), 1);
    assert_eq!(paints.get(), 2);

    let unchanged = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(!unchanged.visual_changed);
    assert!(!unchanged.scene_built);
    assert_eq!(layouts.get(), 1);
    assert_eq!(paints.get(), 2);
}

#[test]
fn update_only_polling_is_admitted_at_engine_cadence_under_event_loop_pressure() {
    let (_sender, receiver) = channel();
    let updates = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(ChannelProbe {
            id: WidgetInternalId::new(),
            receiver,
            applied: Rc::new(Cell::new(0)),
            updates: updates.clone(),
            layouts: Rc::new(Cell::new(0)),
            paints: Rc::new(Cell::new(0)),
        }),
        None,
        0,
    );
    assert!(tree.any_widget_needs_update());
    assert!(!tree.any_widget_needs_animation());

    let mut harness = RetainedSceneTestHarness::new();
    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let first = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(first.visual_changed);
    updates.set(0);

    let start = Instant::now();
    let mut pacer = ContinuousFramePacer::default();
    let mut admitted = 0;
    for micros in 0..=1_000_000 {
        let now = start + Duration::from_micros(micros);
        let _ = pacer.deadline(now, tree.any_widget_needs_update());
        if pacer.take_due(now, tree.any_widget_needs_update()) {
            let frame = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
            assert!(!frame.visual_changed);
            assert!(!frame.scene_built);
            admitted += 1;
        }
    }

    assert_eq!(admitted, 120);
    assert_eq!(updates.get(), 120);
}

#[test]
fn continuous_frame_pacer_preserves_display_phase_across_late_wakeups() {
    let start = Instant::now();
    let interval = Duration::from_nanos(8_333_333);
    let wake_latency = Duration::from_micros(750);
    let mut pacer = ContinuousFramePacer::new(Some(120_000));
    let mut expected_deadline = start + interval;

    assert_eq!(pacer.deadline(start, true), Some(expected_deadline));
    for _ in 0..120 {
        let woke_at = expected_deadline + wake_latency;
        assert!(pacer.take_due(woke_at, true));
        expected_deadline += interval;
        assert_eq!(pacer.deadline(woke_at, true), Some(expected_deadline));
    }

    assert_eq!(
        expected_deadline.duration_since(start),
        interval.checked_mul(121).unwrap()
    );
}

#[test]
fn continuous_frame_pacer_skips_stalled_periods_without_a_redraw_burst() {
    let start = Instant::now();
    let interval = Duration::from_nanos(16_666_667);
    let mut pacer = ContinuousFramePacer::new(Some(60_000));
    let first_deadline = pacer.deadline(start, true).unwrap();
    let woke_at = first_deadline + interval.checked_mul(4).unwrap() + Duration::from_millis(1);

    assert!(pacer.take_due(woke_at, true));
    let next_deadline = pacer.deadline(woke_at, true).unwrap();
    assert_eq!(
        next_deadline,
        first_deadline + interval.checked_mul(5).unwrap()
    );
    assert!(next_deadline > woke_at);
    assert!(!pacer.take_due(woke_at, true));
}

#[test]
fn continuous_frame_pacer_uses_reported_refresh_rate_and_resets_on_change() {
    let start = Instant::now();
    let mut pacer = ContinuousFramePacer::new(Some(60_000));
    assert_eq!(
        pacer.deadline(start, true).unwrap().duration_since(start),
        Duration::from_nanos(16_666_667)
    );

    pacer.set_refresh_rate_millihertz(Some(144_000));
    assert_eq!(
        pacer.deadline(start, true).unwrap().duration_since(start),
        Duration::from_nanos(6_944_444)
    );
}

#[test]
fn retained_fragment_flattens_identically_to_fresh_scene_nodes() {
    let mut fresh_builder = SceneBuilder::new();
    fresh_builder.push_rect(
        RectF::new(0.0, 0.0, 20.0, 10.0),
        Color::from_srgb(0.2, 0.4, 0.8, 1.0).into(),
        0,
        None,
    );
    fresh_builder.push_rect(
        RectF::new(20.0, 0.0, 20.0, 10.0),
        Color::from_srgb(0.1, 0.7, 0.3, 1.0).into(),
        0,
        None,
    );
    let fresh_scene = fresh_builder.build();
    let fresh_counts = test_flatten_counts(&fresh_scene.root);

    let mut retained_builder = SceneBuilder::new();
    retained_builder.push_retained_fragment(Arc::from(fresh_scene.root));
    let retained_scene = retained_builder.build();

    assert_eq!(test_flatten_counts(&retained_scene.root), fresh_counts);
}

// These consumers deliberately never request paint from update or focus callbacks.
#[derive(Default)]
struct FocusAppearanceReadings {
    update: Cell<Option<crate::core::appearance::FrameworkAppearanceFacts>>,
    paint: Cell<Option<crate::core::appearance::FrameworkAppearanceFacts>>,
    overlay: Cell<Option<crate::core::appearance::FrameworkAppearanceFacts>>,
    layouts: Cell<u32>,
    paints: Cell<u32>,
    focuses: Cell<u32>,
    blurs: Cell<u32>,
    capture_enters: Cell<u32>,
    capture_enter_engine: Cell<Option<usize>>,
    capture_enter_mode: Cell<Option<crate::core::event::KeyboardAuthorityMode>>,
    capture_leaves: Cell<u32>,
    capture_leave_engine: Cell<Option<usize>>,
    capture_leave_mode: Cell<Option<crate::core::event::KeyboardAuthorityMode>>,
    pointer_cancels: Cell<u32>,
    last_pointer_cancel: Cell<
        Option<(
            crate::core::event::PointerId,
            crate::core::event::CancellationReason,
        )>,
    >,
    defer_focus: Cell<bool>,
}

struct FocusAppearanceProbe {
    id: WidgetInternalId,
    readings: Rc<FocusAppearanceReadings>,
}

impl WidgetDowncast for FocusAppearanceProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for FocusAppearanceProbe {
    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        self.readings.update.set(cx.appearance_facts());
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        self.readings.layouts.set(self.readings.layouts.get() + 1);
        if self.readings.defer_focus.replace(false) {
            cx.defer_focus_to_self();
        }
        for &child in children {
            let size = cx.layout_child(child, constraints);
            cx.set_child_rect(child, RectF::new(0.0, 0.0, size.width, size.height));
        }
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let facts = cx.appearance_facts();
        assert_eq!(facts, self.readings.update.get());
        self.readings.paint.set(facts);
        self.readings.paints.set(self.readings.paints.get() + 1);
        let color = if let Some(facts) = facts {
            assert_eq!(cx.is_focused(), facts.focused());
            assert_eq!(cx.has_capture(), facts.keyboard_captured());
            Color::from_srgb(
                if facts.focused() { 0.8 } else { 0.2 },
                if facts.pointer_captured() { 0.8 } else { 0.2 },
                if facts.scroll_captured() { 0.8 } else { 0.2 },
                if facts.keyboard_captured() { 1.0 } else { 0.7 },
            )
        } else {
            Color::from_srgb(0.2, 0.2, 0.2, 1.0)
        };
        cx.scene.push_rect(rect, color, 0, None);
    }

    fn paint_overlay(
        &self,
        cx: &mut PaintContext,
        _rect: RectF,
        _owner: crate::utils::id::TreeWidgetId,
    ) {
        let facts = cx.appearance_facts();
        assert_eq!(facts, self.readings.update.get());
        assert_eq!(facts, self.readings.paint.get());
        self.readings.overlay.set(facts);
    }

    fn focusable(&self) -> bool {
        true
    }
    fn capture_eligible(&self) -> bool {
        true
    }
    fn on_focus(&mut self, _cx: &mut crate::core::widget::EventContext) {
        self.readings.focuses.set(self.readings.focuses.get() + 1);
    }
    fn on_blur(&mut self, _cx: &mut crate::core::widget::EventContext) {
        self.readings.blurs.set(self.readings.blurs.get() + 1);
    }
    fn on_capture_enter(&mut self, cx: &mut crate::core::widget::EventContext) {
        self.readings
            .capture_enters
            .set(self.readings.capture_enters.get() + 1);
        self.readings.capture_enter_mode.set(Some(cx.keyboard_mode));
        let engine = cx.try_text_engine().map(|engine| {
            let (width, height) = engine.measure("Capture", 16.0);
            assert!(width > 0.0 && height > 0.0);
            engine as *mut dyn crate::core::text_engine::TextEngine as *mut () as usize
        });
        self.readings.capture_enter_engine.set(engine);
    }
    fn on_capture_leave(&mut self, cx: &mut crate::core::widget::EventContext) {
        self.readings
            .capture_leaves
            .set(self.readings.capture_leaves.get() + 1);
        self.readings.capture_leave_mode.set(Some(cx.keyboard_mode));
        let engine = cx.try_text_engine().map(|engine| {
            let (width, height) = engine.measure("Capture", 16.0);
            assert!(width > 0.0 && height > 0.0);
            engine as *mut dyn crate::core::text_engine::TextEngine as *mut () as usize
        });
        self.readings.capture_leave_engine.set(engine);
    }
    fn on_pointer_capture_cancel(
        &mut self,
        _cx: &mut crate::core::widget::EventContext,
        pointer_id: crate::core::event::PointerId,
        reason: crate::core::event::CancellationReason,
    ) {
        self.readings
            .pointer_cancels
            .set(self.readings.pointer_cancels.get() + 1);
        self.readings
            .last_pointer_cancel
            .set(Some((pointer_id, reason)));
    }
    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

struct FocusAppearanceFixture {
    tree: WidgetTree,
    nodes: [NodeId; 4],
    readings: [Rc<FocusAppearanceReadings>; 4],
    domains: crate::core::event::InteractionDomains,
    harness: RetainedSceneTestHarness,
    text: FontManagerTextEngine,
}

impl FocusAppearanceFixture {
    fn new() -> Self {
        let mut tree = WidgetTree::new();
        let readings: [Rc<FocusAppearanceReadings>; 4] = std::array::from_fn(|_| Rc::default());
        let probe = |index: usize| {
            Box::new(FocusAppearanceProbe {
                id: WidgetInternalId::new(),
                readings: readings[index].clone(),
            })
        };
        let root = tree.create_root(probe(0), None, 0);
        let nodes = [
            root,
            tree.add_child(root, probe(1), None, 0),
            tree.add_child(root, probe(2), None, 0),
            tree.add_child(root, probe(3), None, 0),
        ];
        Self {
            tree,
            nodes,
            readings,
            domains: crate::core::event::InteractionDomains::new(),
            harness: RetainedSceneTestHarness::new(),
            text: FontManagerTextEngine::new(),
        }
    }

    fn owner(&self, index: usize) -> crate::utils::id::TreeWidgetId {
        self.tree.widget_id_for_node(self.nodes[index]).unwrap()
    }

    fn frame(&mut self) -> crate::core::app::RetainedSceneTestFrame {
        self.harness.frame_with_appearance(
            &mut self.tree,
            self.nodes[0],
            Constraints::tight(Size::new(100.0, 60.0)),
            &Theme::default(),
            &mut self.text,
            &IconRegistry::new(&[]),
            &self.domains,
            crate::core::event::WindowId::PRIMARY,
        )
    }

    fn focus(&mut self, index: usize) {
        let owner = self.owner(index);
        self.domains
            .primary_state_mut()
            .focus_manager
            .set_focus(Some(owner));
    }

    fn paints(&self) -> [u32; 4] {
        self.readings.each_ref().map(|r| r.paints.get())
    }
    fn layouts(&self) -> [u32; 4] {
        self.readings.each_ref().map(|r| r.layouts.get())
    }
    fn facts(&self, index: usize) -> crate::core::appearance::FrameworkAppearanceFacts {
        self.readings[index].update.get().unwrap()
    }
}

fn assert_focus_appearance_recomposition(
    before: &crate::core::app::RetainedSceneTestFrame,
    after: &crate::core::app::RetainedSceneTestFrame,
    changed_children: &[usize],
) {
    use crate::core::scene::SceneNode;

    let root_fragment = |frame: &crate::core::app::RetainedSceneTestFrame| {
        let [SceneNode::RetainedFragment(root)] = frame.scene_root.as_slice() else {
            panic!("expected the fixture's retained root composition");
        };
        root.children.clone()
    };
    let before = root_fragment(before);
    let after = root_fragment(after);
    assert!(!Arc::ptr_eq(&before, &after), "ancestor must recompose");
    // This fixture emits one root-local fragment followed by its three children.
    assert_eq!(before.len(), 4);
    assert_eq!(after.len(), 4);
    for (index, (before, after)) in before.iter().zip(after.iter()).enumerate() {
        let (SceneNode::RetainedFragment(before), SceneNode::RetainedFragment(after)) =
            (before, after)
        else {
            panic!("expected a retained local or child fragment");
        };
        assert_eq!(
            Arc::ptr_eq(&before.children, &after.children),
            !changed_children.contains(&index),
            "only affected child compositions may change; fragment {index}"
        );
    }
}

#[test]
fn focus_appearance_coherent_snapshot_preserves_optional_focus_domain() {
    use crate::core::event::{InteractionDomainId, WindowId};
    let mut f = FocusAppearanceFixture::new();
    f.focus(1);
    assert!(f.frame().scene_built);
    for (index, readings) in f.readings.iter().enumerate() {
        assert_eq!(readings.update.get(), readings.paint.get());
        assert_eq!(readings.update.get(), readings.overlay.get());
        let facts = f.facts(index);
        assert_eq!(facts.window(), WindowId::PRIMARY);
        assert_eq!(facts.interaction_domain(), InteractionDomainId::PRIMARY);
        assert_eq!(facts.focus_domain(), None);
        assert!(facts.window_active());
        assert!(facts.interaction_domain_active());
        assert_eq!(facts.focused(), index == 1);
        assert!(!facts.keyboard_captured());
        assert!(!facts.pointer_captured());
        assert!(!facts.scroll_captured());
    }
    assert!(!f.frame().scene_built);
    assert_eq!(f.paints(), [1; 4]);
    assert_eq!(f.layouts(), [1; 4]);
}

#[test]
fn focus_appearance_old_and_new_owners_repaint_without_callback_requests() {
    let mut f = FocusAppearanceFixture::new();
    let mut router = crate::core::event_router::EventRouter::new();
    let first = f.owner(1);
    let second = f.owner(2);
    router.request_focus_for_widget(&mut f.tree, first, f.domains.primary_state_mut());
    let before = f.frame();
    router.request_focus_for_widget(&mut f.tree, second, f.domains.primary_state_mut());
    let moved = f.frame();
    assert!(moved.scene_built);
    assert_focus_appearance_recomposition(&before, &moved, &[1, 2]);
    assert_eq!(f.paints(), [1, 2, 2, 1]);
    assert_eq!(f.layouts(), [1; 4]);
    assert_eq!(f.readings[1].blurs.get(), 1);
    assert_eq!(f.readings[2].focuses.get(), 1);
    assert!(!f.facts(1).focused());
    assert!(f.facts(2).focused());
    assert!(!f.frame().scene_built);

    // Direct authority publication also works without any widget callback.
    f.focus(3);
    let moved_again = f.frame();
    assert!(moved_again.scene_built);
    assert_focus_appearance_recomposition(&moved, &moved_again, &[2, 3]);
    assert_eq!(f.paints(), [1, 2, 3, 2]);
    assert_eq!(f.readings[3].focuses.get(), 0);
    assert_eq!(f.layouts(), [1; 4]);
}

#[test]
fn focus_appearance_mode_only_transition_invalidates_only_keyboard_owner() {
    use crate::core::event::KeyboardAuthorityMode;
    let mut f = FocusAppearanceFixture::new();
    f.focus(1);
    let navigation = f.frame();
    f.domains.primary_state_mut().keyboard_mode = KeyboardAuthorityMode::Capture;
    let captured = f.frame();
    assert!(captured.scene_built);
    assert_focus_appearance_recomposition(&navigation, &captured, &[1]);
    assert_eq!(f.paints(), [1, 2, 1, 1]);
    assert!(f.facts(1).keyboard_captured());
    assert!(!f.facts(2).keyboard_captured());
    f.domains.primary_state_mut().keyboard_mode = KeyboardAuthorityMode::Navigation;
    let released = f.frame();
    assert!(released.scene_built);
    assert_focus_appearance_recomposition(&captured, &released, &[1]);
    assert_eq!(f.paints(), [1, 3, 1, 1]);
    assert!(f.facts(1).focused());
    assert!(!f.facts(1).keyboard_captured());
    assert_eq!(f.layouts(), [1; 4]);
    assert!(!f.frame().scene_built);
}

#[test]
fn focus_appearance_window_and_interaction_domain_suspend_without_inventing_restoration() {
    use crate::core::event::{KeyboardAuthorityMode, PointerId, WindowId};
    let mut f = FocusAppearanceFixture::new();
    let keyboard = f.owner(1);
    let pointer = f.owner(2);
    let scroll = f.owner(3);
    f.focus(1);
    let state = f.domains.primary_state_mut();
    state.keyboard_mode = KeyboardAuthorityMode::Capture;
    state
        .pointer_capture_manager
        .request_capture(PointerId(1), pointer)
        .unwrap();
    state.scroll_capture_owner = Some(scroll);
    f.frame();
    f.domains.set_active_window(None);
    assert!(f.frame().scene_built);
    assert_eq!(f.paints(), [1, 2, 2, 2]);
    for index in 0..4 {
        let facts = f.facts(index);
        assert!(!facts.window_active());
        assert!(!facts.interaction_domain_active());
        assert!(!facts.focused());
        assert!(!facts.keyboard_captured());
        assert!(!facts.pointer_captured());
        assert!(!facts.scroll_captured());
    }
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(keyboard)
    );
    assert_eq!(
        f.domains.primary_state().keyboard_mode,
        KeyboardAuthorityMode::Capture
    );
    let other_window = WindowId(10);
    let other_domain = f.domains.register_window(other_window);
    f.domains
        .state_mut(other_domain)
        .unwrap()
        .focus_manager
        .set_focus(Some(scroll));
    f.domains.set_active_window(Some(other_window));
    assert!(!f.frame().scene_built);
    assert!(!f.facts(3).focused());
    f.domains.set_active_window(Some(WindowId::PRIMARY));
    assert!(f.frame().scene_built);
    assert_eq!(f.paints(), [1, 3, 3, 3]);
    assert!(f.facts(1).keyboard_captured());
    assert!(f.facts(2).pointer_captured());
    assert!(f.facts(3).scroll_captured());
    assert_eq!(f.layouts(), [1; 4]);

    // The real focus-loss teardown clears owners; appearance cannot restore them.
    f.domains
        .handle_focus_changed(WindowId::PRIMARY, false)
        .unwrap();
    f.frame();
    f.domains
        .handle_focus_changed(WindowId::PRIMARY, true)
        .unwrap();
    f.frame();
    for index in 0..4 {
        assert!(!f.facts(index).focused());
        assert!(!f.facts(index).keyboard_captured());
        assert!(!f.facts(index).pointer_captured());
        assert!(!f.facts(index).scroll_captured());
    }
    assert_eq!(f.layouts(), [1; 4]);
}

#[test]
fn focus_appearance_pointer_and_scroll_capture_have_independent_owners() {
    use crate::core::event::{KeyboardAuthorityMode, PointerButton, PointerId};
    let mut f = FocusAppearanceFixture::new();
    let pointer = f.owner(2);
    let scroll = f.owner(3);
    f.focus(1);
    f.domains.primary_state_mut().keyboard_mode = KeyboardAuthorityMode::Capture;
    f.frame();
    let state = f.domains.primary_state_mut();
    state
        .pointer_capture_manager
        .request_capture(PointerId(3), pointer)
        .unwrap();
    state
        .pointer_capture_manager
        .request_capture(PointerId(2), pointer)
        .unwrap();
    state.scroll_capture_owner = Some(scroll);
    assert!(f.frame().scene_built);
    assert_eq!(f.paints(), [1, 1, 2, 2]);
    assert!(f.facts(1).keyboard_captured());
    assert!(!f.facts(1).pointer_captured());
    assert!(!f.facts(1).scroll_captured());
    assert!(f.facts(2).pointer_captured());
    assert!(!f.facts(2).keyboard_captured());
    assert!(f.facts(3).scroll_captured());
    assert!(!f.facts(3).keyboard_captured());
    f.domains
        .primary_state_mut()
        .pointer_capture_manager
        .release_capture(PointerId(3), pointer);
    assert!(!f.frame().scene_built);
    f.domains
        .primary_state_mut()
        .pointer_capture_manager
        .release_capture(PointerId(2), pointer);
    assert!(f.frame().scene_built);
    assert_eq!(f.paints(), [1, 1, 3, 2]);
    let state = f.domains.primary_state_mut();
    state
        .pointer_capture
        .capture(pointer, PointerButton::Primary);
    state.scroll_capture_owner = Some(pointer);
    f.frame();
    // The legacy compatibility field is not the routing capture authority.
    assert!(!f.facts(2).pointer_captured());
    assert!(f.facts(2).scroll_captured());
    assert!(!f.facts(3).scroll_captured());
    assert_eq!(f.paints(), [1, 1, 4, 3]);
    assert_eq!(f.layouts(), [1; 4]);
}

#[test]
fn focus_appearance_defensively_filters_removed_owner_before_authoritative_cleanup() {
    use crate::core::appearance::FrameworkAppearanceSnapshot;
    use crate::core::event::{KeyboardAuthorityMode, PointerId, WindowId};
    let mut f = FocusAppearanceFixture::new();
    let removed = f.owner(1);
    f.focus(1);
    let state = f.domains.primary_state_mut();
    state.keyboard_mode = KeyboardAuthorityMode::Capture;
    state
        .pointer_capture_manager
        .request_capture(PointerId(0), removed)
        .unwrap();
    state.scroll_capture_owner = Some(removed);
    assert_eq!(f.frame().cache_entries, 4);
    f.tree.remove_node(f.nodes[1]);
    // Deliberately probe a pre-cleanup gap. Production drain_pending_removals ->
    // EventRouter cleanup/focus recovery owns authoritative teardown before paint.
    // Appearance filtering is read-side safety, never a lawful stale steady state.
    let frame = f.frame();
    assert!(frame.scene_built);
    assert_eq!(frame.cache_entries, 3);
    assert_eq!(f.readings[1].paints.get(), 1);
    let snapshot =
        FrameworkAppearanceSnapshot::capture(&f.domains, WindowId::PRIMARY, &f.tree).unwrap();
    assert_eq!(snapshot.active_focused_owner(), None);
    assert_eq!(snapshot.owners().count(), 0);
    // Filtering neither repairs authority nor consumes the owner's removal work.
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(removed)
    );
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId(0)),
        Some(removed)
    );
    assert_eq!(f.tree.drain_pending_removals(), vec![removed]);
}

#[test]
fn focus_appearance_focus_resolver_domain_transition_repaints_old_and_new_targets() {
    use crate::core::event::InteractionDomainId;
    use crate::core::focus_authority::{
        FocusDomainAddition, FocusDomainId, FocusResolver, FocusTargetRegistration,
        InteractionDomainTransitionCapability,
    };
    let mut f = FocusAppearanceFixture::new();
    let first = f.owner(1);
    let second = f.owner(2);
    let resolver = FocusResolver::new();
    let capability =
        InteractionDomainTransitionCapability::issue(InteractionDomainId::PRIMARY, 1).unwrap();
    let manager = &mut f.domains.primary_state_mut().focus_manager;
    let graph = resolver
        .prepare_focus_domain_graph_mutation(
            &capability,
            manager,
            &[
                FocusDomainAddition::new(FocusDomainId(1), None),
                FocusDomainAddition::new(FocusDomainId(2), None),
            ],
            &[],
            Vec::new(),
            Some(FocusDomainId(1)),
        )
        .unwrap();
    resolver.commit_focus_domain_graph_mutation(&capability, graph);
    let targets = resolver
        .prepare_focus_target_mutation(
            &capability,
            manager,
            &[
                FocusTargetRegistration::new(FocusDomainId(1), first, None, true),
                FocusTargetRegistration::new(FocusDomainId(2), second, None, true),
            ],
            &[],
        )
        .unwrap();
    resolver.commit_focus_target_mutation(&capability, targets);
    f.frame();
    assert_eq!(f.facts(1).focus_domain(), Some(FocusDomainId(1)));
    assert!(f.facts(1).focused());
    for (domain, focused, paints) in [
        (FocusDomainId(2), 2, [1, 2, 2, 1]),
        (FocusDomainId(1), 1, [1, 3, 3, 1]),
    ] {
        let manager = &mut f.domains.primary_state_mut().focus_manager;
        let prepared = resolver
            .prepare_focus_domain_activation(&capability, manager, domain)
            .unwrap();
        resolver.commit_focus_domain_activation(&capability, prepared);
        assert!(f.frame().scene_built);
        assert_eq!(f.paints(), paints);
        assert!(f.facts(focused).focused());
        assert_eq!(f.facts(focused).focus_domain(), Some(domain));
    }
    assert_eq!(f.layouts(), [1; 4]);
    assert!(!f.frame().scene_built);
}

fn assert_layout_deferred_focus_context(old_captured: bool) {
    use crate::core::event::KeyboardAuthorityMode;

    let mut f = FocusAppearanceFixture::new();
    let next_owner = f.owner(2);
    f.focus(1);
    let mut router = crate::core::event_router::EventRouter::new();
    let engine_id = &mut f.text as *mut FontManagerTextEngine as usize;
    if old_captured {
        assert!(router.handle_engine_action(
            &mut f.tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(f.nodes[0]),
            f.domains.primary_state_mut(),
            &mut f.text,
            None,
        ));
    }
    f.readings[2].defer_focus.set(true);
    f.frame();
    assert!(f.facts(1).focused());
    assert!(!f.facts(2).focused());
    assert!(!f.readings[2].paint.get().unwrap().focused());
    crate::core::app::apply_deferred_focus_requests(
        &mut f.tree,
        &mut router,
        f.domains.primary_state_mut(),
        &mut f.text,
    );
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(next_owner)
    );
    assert_eq!(
        f.domains.primary_state().keyboard_mode,
        KeyboardAuthorityMode::Capture
    );
    assert_eq!(f.readings[2].capture_enters.get(), 1);
    assert_eq!(f.readings[2].capture_enter_engine.get(), Some(engine_id));
    assert_eq!(
        f.readings[2].capture_enter_mode.get(),
        Some(KeyboardAuthorityMode::Capture)
    );
    if old_captured {
        assert_eq!(f.readings[1].capture_leaves.get(), 1);
        assert_eq!(f.readings[1].capture_leave_engine.get(), Some(engine_id));
        assert_eq!(
            f.readings[1].capture_leave_mode.get(),
            Some(KeyboardAuthorityMode::Navigation)
        );
    } else {
        assert_eq!(f.readings[1].capture_leaves.get(), 0);
    }
    assert!(!f.facts(2).focused());
    crate::core::app::apply_deferred_focus_requests(
        &mut f.tree,
        &mut router,
        f.domains.primary_state_mut(),
        &mut f.text,
    );
    assert_eq!(f.readings[2].capture_enters.get(), 1);
    assert_eq!(f.readings[1].capture_leaves.get(), u32::from(old_captured));
    assert!(f.frame().scene_built);
    assert!(f.facts(2).focused());
    assert!(f.facts(2).keyboard_captured());
    assert!(!f.facts(1).keyboard_captured());
    assert_eq!(f.paints(), [1, 2, 2, 1]);
    assert_eq!(f.layouts(), [1; 4]);
    assert!(!f.frame().scene_built);
}

#[test]
fn focus_appearance_layout_deferred_focus_is_published_on_the_next_frame() {
    assert_layout_deferred_focus_context(false);
}

#[test]
fn focus_appearance_layout_deferred_focus_preserves_capture_transfer_engine() {
    assert_layout_deferred_focus_context(true);
}

#[test]
fn focus_appearance_window_loss_delivers_lifecycle_before_authority_teardown() {
    use crate::core::app::handle_window_focus_changed;
    use crate::core::event::{FusedInputEvent, WindowId};
    use crate::core::event_router::EventRouter;
    use crate::runtime::binding_registry::BindingRegistry;
    use std::time::Instant;

    let mut f = FocusAppearanceFixture::new();
    let mut router = EventRouter::new();
    let mut bindings = BindingRegistry::new();
    let owner = f.owner(1);
    f.focus(1);
    router.enter_capture_for_widget(
        &mut f.tree,
        owner,
        f.nodes[1],
        f.domains.primary_state_mut(),
    );
    let before = f.frame();
    assert!(f.facts(1).keyboard_captured());
    let layouts = f.layouts();
    let engine_id = &mut f.text as *mut FontManagerTextEngine as usize;

    for _ in 0..2 {
        let state = handle_window_focus_changed(
            &mut f.domains,
            &mut router,
            &mut f.tree,
            WindowId::PRIMARY,
            false,
            &mut f.text,
        )
        .unwrap();
        router.route_input_event(
            &mut f.tree,
            FusedInputEvent::FocusChanged {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                focused: false,
            },
            Some(f.nodes[0]),
            &mut f.text,
            None,
            state,
            &mut bindings,
        );
        assert_eq!(f.readings[1].blurs.get(), 1, "one loss, one blur");
        assert_eq!(f.readings[1].capture_leaves.get(), 1, "one capture loss");
        assert_eq!(
            f.readings[1].capture_leave_engine.get(),
            Some(engine_id),
            "capture leave must receive the current App text engine before teardown",
        );
        assert_eq!(
            f.readings[1].capture_leave_mode.get(),
            Some(crate::core::event::KeyboardAuthorityMode::Navigation),
        );
        assert_eq!(f.domains.primary_state().focus_manager.focused(), None);
        assert_eq!(f.domains.active_window(), None);
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );
    }
    let after = f.frame();
    assert_focus_appearance_recomposition(&before, &after, &[1]);
    assert!(!f.facts(1).focused());
    assert!(!f.facts(1).keyboard_captured());
    assert_eq!(f.layouts(), layouts);
    assert!(!f.frame().scene_built);

    let state = handle_window_focus_changed(
        &mut f.domains,
        &mut router,
        &mut f.tree,
        WindowId::PRIMARY,
        true,
        &mut f.text,
    )
    .unwrap();
    router.route_input_event(
        &mut f.tree,
        FusedInputEvent::FocusChanged {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            focused: true,
        },
        Some(f.nodes[0]),
        &mut f.text,
        None,
        state,
        &mut bindings,
    );
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(owner)
    );
    assert_eq!(f.readings[1].focuses.get(), 1);
    let restored = f.frame();
    assert_focus_appearance_recomposition(&after, &restored, &[1]);
    assert!(f.facts(1).focused());
    assert_eq!(f.layouts(), layouts);
}

fn assert_window_pointer_capture_cancellation(through_app_bridge: bool) {
    use crate::core::event::{CancellationReason, FusedInputEvent, PointerId, WindowId};
    use crate::core::event_router::EventRouter;
    use crate::runtime::binding_registry::BindingRegistry;

    let mut f = FocusAppearanceFixture::new();
    let mut router = EventRouter::new();
    let mut bindings = BindingRegistry::new();
    f.focus(1);
    let pointer_owner = f.owner(2);
    f.domains
        .primary_state_mut()
        .pointer_capture_manager
        .request_capture(PointerId::MOUSE, pointer_owner)
        .unwrap();
    let before = f.frame();
    assert!(f.facts(1).focused());
    assert!(f.facts(2).pointer_captured());
    let layouts = f.layouts();

    for _ in 0..2 {
        let state = if through_app_bridge {
            crate::core::app::handle_window_focus_changed(
                &mut f.domains,
                &mut router,
                &mut f.tree,
                WindowId::PRIMARY,
                false,
                &mut f.text,
            )
            .unwrap()
        } else {
            f.domains.primary_state_mut()
        };
        router.route_input_event(
            &mut f.tree,
            FusedInputEvent::FocusChanged {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                focused: false,
            },
            Some(f.nodes[0]),
            &mut f.text,
            None,
            state,
            &mut bindings,
        );
        assert_eq!(f.readings[2].pointer_cancels.get(), 1);
        assert_eq!(
            f.readings[2].last_pointer_cancel.get(),
            Some((PointerId::MOUSE, CancellationReason::SystemCancel)),
        );
        assert_eq!(f.readings[1].pointer_cancels.get(), 0);
        assert_eq!(state.focus_manager.focused(), None);
        assert_eq!(
            state
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
    }
    let after = f.frame();
    assert_focus_appearance_recomposition(&before, &after, &[1, 2]);
    assert!(!f.facts(1).focused());
    assert!(!f.facts(2).pointer_captured());
    assert_eq!(f.layouts(), layouts);
    assert!(!f.frame().scene_built);
}

#[test]
fn window_pointer_capture_cancel_reaches_independent_owner_before_domain_teardown() {
    assert_window_pointer_capture_cancellation(true);
}

#[test]
fn window_pointer_capture_cancel_reaches_independent_owner_in_fused_fallback() {
    assert_window_pointer_capture_cancellation(false);
}

mod button_appearance;
use self::button_appearance::{button_product_style, button_product_values};

struct TimePickerRetainedFixture {
    tree: WidgetTree,
    nodes: [NodeId; 4],
    theme: Theme,
    commits: Rc<Cell<u32>>,
    domains: crate::core::event::InteractionDomains,
    router: crate::core::event_router::EventRouter,
    bindings: crate::runtime::binding_registry::BindingRegistry,
    text: FontManagerTextEngine,
    harness: RetainedSceneTestHarness,
}

impl TimePickerRetainedFixture {
    fn new(
        theme: Theme,
        policy: crate::core::event::UiInteractionPolicy,
        seconds: bool,
        mode: crate::widgets::time_picker::TimeDisplayMode,
    ) -> Self {
        use crate::widgets::time_picker::{TimeEditPolicy, TimePicker, TimeValue};
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(TypographyLayout {
                id: WidgetInternalId::new(),
            }),
            None,
            0,
        );
        let commits = Rc::new(Cell::new(0));
        let committed = commits.clone();
        let value = TimeValue::new(14, 30, seconds.then_some(45));
        let picker = TimePicker::new(value)
            .with_display_mode(mode)
            .with_seconds(seconds)
            .on_commit(move |_| committed.set(committed.get() + 1));
        let nodes = [
            root,
            tree.add_child(root, Box::new(picker), None, 0),
            tree.add_child(
                root,
                Box::new(TimePicker::new(value).with_edit_policy(TimeEditPolicy::ReadOnly)),
                None,
                1,
            ),
            tree.add_child(
                root,
                Box::new(crate::widgets::label::Label::new("Unchanged peer")),
                None,
                2,
            ),
        ];
        Self {
            tree,
            nodes,
            theme,
            commits,
            domains: crate::core::event::InteractionDomains::with_ui_interaction_policy(policy),
            router: crate::core::event_router::EventRouter::new(),
            bindings: crate::runtime::binding_registry::BindingRegistry::new(),
            text: FontManagerTextEngine::new(),
            harness: RetainedSceneTestHarness::new(),
        }
    }

    fn owner(&self, index: usize) -> crate::utils::id::TreeWidgetId {
        self.tree.widget_id_for_node(self.nodes[index]).unwrap()
    }

    fn picker(&self, index: usize) -> &crate::widgets::time_picker::TimePicker {
        self.tree
            .node(self.nodes[index])
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    fn local(&self, index: usize) -> Arc<[crate::core::scene::SceneNode]> {
        self.harness.local_fragment(self.owner(index)).unwrap()
    }

    fn frame(&mut self) -> crate::core::app::RetainedSceneTestFrame {
        self.harness.frame_with_appearance(
            &mut self.tree,
            self.nodes[0],
            Constraints::tight(Size::new(360.0, 320.0)),
            &self.theme,
            &mut self.text,
            &IconRegistry::new(&[]),
            &self.domains,
            crate::core::event::WindowId::PRIMARY,
        )
    }

    fn focus(&mut self, index: usize) {
        let owner = self.owner(index);
        self.router.request_focus_for_widget(
            &mut self.tree,
            owner,
            self.domains.primary_state_mut(),
        );
    }

    fn enter_capture(&mut self) -> bool {
        self.router.handle_engine_action(
            &mut self.tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(self.nodes[0]),
            self.domains.primary_state_mut(),
            &mut self.text,
            None,
        )
    }

    fn route(&mut self, event: crate::core::event::FusedInputEvent) {
        assert!(self.domains.should_route_event(&event));
        let state =
            if let crate::core::event::FusedInputEvent::FocusChanged {
                window_id, focused, ..
            } = &event
            {
                crate::core::app::handle_window_focus_changed(
                    &mut self.domains,
                    &mut self.router,
                    &mut self.tree,
                    *window_id,
                    *focused,
                    &mut self.text,
                )
                .unwrap()
            } else {
                self.domains.resolve_window_mut(event.window_id()).unwrap()
            };
        self.router.route_input_event(
            &mut self.tree,
            event,
            Some(self.nodes[0]),
            &mut self.text,
            None,
            state,
            &mut self.bindings,
        );
    }

    fn key(&mut self, key_code: crate::core::event::KeyCode) {
        use crate::core::event::{FusedInputEvent, Modifiers, PhysicalKeyCode, WindowId};
        self.route(FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code,
            physical_key: PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat: false,
        });
        self.route(FusedInputEvent::KeyUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code,
            physical_key: PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: Modifiers::default(),
        });
    }

    fn type_text(&mut self, text: &str) {
        self.route(crate::core::event::FusedInputEvent::TextInput {
            window_id: crate::core::event::WindowId::PRIMARY,
            time: Instant::now(),
            text: text.into(),
            modifiers: crate::core::event::Modifiers::default(),
        });
    }

    fn click_column(&mut self) {
        use crate::core::event::{FusedInputEvent, PointerButton, PointerDevice, WindowId};
        let rect = self.tree.get_absolute_paint_rect(self.nodes[1]).unwrap();
        let pos = crate::core::math::Point::new(rect.x + 2.0, rect.y + rect.height * 0.5);
        self.route(FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        });
        self.route(FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        });
    }

    fn facts(&self) -> crate::core::appearance::FrameworkAppearanceFacts {
        crate::core::appearance::FrameworkAppearanceSnapshot::capture(
            &self.domains,
            crate::core::event::WindowId::PRIMARY,
            &self.tree,
        )
        .unwrap()
        .for_owner(self.owner(1))
    }
}

#[derive(Debug, PartialEq)]
enum TimePickerFrameProjection {
    Rect(RectF, crate::core::scene::Color, u32),
    RoundedRect(RectF, crate::core::scene::Color, f32, u32),
    BorderRect(
        RectF,
        crate::core::scene::Color,
        crate::core::scene::Color,
        f32,
        f32,
        u32,
    ),
}

fn time_picker_frame_projection(
    nodes: &[crate::core::scene::SceneNode],
) -> Vec<TimePickerFrameProjection> {
    use crate::core::scene::SceneNode;
    nodes
        .iter()
        .filter_map(|node| match node {
            SceneNode::Rect(shape) => Some(TimePickerFrameProjection::Rect(
                shape.rect,
                shape.color,
                shape.z_index,
            )),
            SceneNode::RoundedRect(shape) => Some(TimePickerFrameProjection::RoundedRect(
                shape.rect,
                shape.color,
                shape.radius,
                shape.z_index,
            )),
            SceneNode::BorderRect(border) => Some(TimePickerFrameProjection::BorderRect(
                border.rect,
                border.fill_color,
                border.border_color,
                border.border_width,
                border.radius,
                border.z_index,
            )),
            _ => None,
        })
        .collect()
}

fn time_picker_emitted_frame_rect(nodes: &[crate::core::scene::SceneNode]) -> RectF {
    use crate::core::scene::SceneNode;
    nodes
        .iter()
        .filter_map(|node| match node {
            SceneNode::Rect(shape) => Some((shape.z_index, shape.rect)),
            SceneNode::RoundedRect(shape) => Some((shape.z_index, shape.rect)),
            _ => None,
        })
        .min_by_key(|(z, _)| *z)
        .expect("the standalone TimePicker emits its base surface")
        .1
}

fn time_picker_has_hour_projection(
    nodes: &[crate::core::scene::SceneNode],
    rect: RectF,
    column_width: f32,
) -> bool {
    nodes.iter().any(|node| {
        let shape = match node {
            crate::core::scene::SceneNode::Rect(shape) => shape.rect,
            crate::core::scene::SceneNode::RoundedRect(shape) => shape.rect,
            _ => return false,
        };
        let center_x = rect.x + column_width * 0.5;
        let center_y = rect.y + rect.height * 0.5;
        shape.width > 0.0
            && shape.height > 0.0
            && shape.x >= rect.x
            && shape.y >= rect.y
            && shape.right() <= rect.x + column_width
            && shape.bottom() <= rect.bottom()
            && shape.x <= center_x
            && shape.right() >= center_x
            && shape.y <= center_y
            && shape.bottom() >= center_y
    })
}

#[test]
fn time_picker_retained_input_preserves_density_geometry_and_commit_lifecycle() {
    use crate::core::event::{KeyCode, KeyboardAuthorityMode, UiInteractionPolicy};
    use crate::core::theme::DensityMode;
    use crate::widgets::time_picker::{TimeDisplayMode, TimeValue};

    for dark in [false, true] {
        for density in [
            DensityMode::Compact,
            DensityMode::Regular,
            DensityMode::Comfortable,
            DensityMode::Intermediate30,
        ] {
            let theme = if dark {
                Theme::dark_with_density(density)
            } else {
                Theme::light_with_density(density)
            };
            let mut f = TimePickerRetainedFixture::new(
                theme,
                UiInteractionPolicy::NavigationAndCapture,
                true,
                TimeDisplayMode::H12,
            );
            assert!(f.frame().scene_built);
            let geometry = |f: &TimePickerRetainedFixture| {
                f.nodes.map(|id| {
                    let node = f.tree.node(id).unwrap();
                    (
                        node.id,
                        node.widget.id(),
                        f.tree.get_absolute_paint_rect(id).unwrap(),
                        node.layout_epoch,
                    )
                })
            };
            let canonical_before = geometry(&f);
            let geom = &f.theme.geometry.time_picker;
            let canonical_rect = f.tree.get_absolute_paint_rect(f.nodes[1]).unwrap();
            let emitted_frame = time_picker_emitted_frame_rect(&f.local(1));
            assert_eq!(
                emitted_frame.width,
                geom.column_width * 3.0 + geom.separator_width * 2.0 + geom.ampm_width
            );
            assert_eq!(emitted_frame.x, canonical_rect.x);
            assert_eq!(emitted_frame.y, canonical_rect.y);
            assert_eq!(emitted_frame.height, canonical_rect.height);
            let labels: Vec<_> = ui_typography_texts(&f.local(1))
                .into_iter()
                .map(|label| label.text)
                .collect();
            assert_eq!(labels, [" 2", ":", "30", ":", "45", "PM"]);
            let root_local = f.local(0);
            let peer_local = f.local(3);
            let readonly_local = f.local(2);

            f.focus(1);
            assert!(f.enter_capture());
            assert!(f.facts().keyboard_captured());
            f.key(KeyCode::Up);
            assert_eq!(f.picker(1).working(), TimeValue::new(15, 30, Some(45)));
            assert_eq!(f.picker(1).value(), TimeValue::new(14, 30, Some(45)));
            f.type_text("0");
            f.type_text("4");
            assert_eq!(f.picker(1).working(), TimeValue::new(16, 30, Some(45)));
            assert_eq!(f.commits.get(), 0);
            f.frame();
            assert_eq!(geometry(&f), canonical_before);
            assert_eq!(time_picker_emitted_frame_rect(&f.local(1)), emitted_frame);
            assert!(Arc::ptr_eq(&readonly_local, &f.local(2)));

            f.key(KeyCode::Enter);
            assert_eq!(f.commits.get(), 1);
            assert_eq!(f.picker(1).value(), TimeValue::new(16, 30, Some(45)));
            assert_eq!(f.picker(1).working(), f.picker(1).value());
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Navigation
            );
            f.frame();
            assert!(f.enter_capture());
            f.type_text("0");
            f.type_text("9");
            assert_ne!(f.picker(1).working(), f.picker(1).value());
            f.key(KeyCode::Escape);
            assert_eq!(f.picker(1).working(), f.picker(1).value());
            assert_eq!(f.picker(1).value(), TimeValue::new(16, 30, Some(45)));
            assert_eq!(f.commits.get(), 1);
            f.frame();
            assert_eq!(geometry(&f), canonical_before);
            assert_eq!(time_picker_emitted_frame_rect(&f.local(1)), emitted_frame);
            assert!(Arc::ptr_eq(&root_local, &f.local(0)));
            assert!(Arc::ptr_eq(&peer_local, &f.local(3)));

            f.focus(2);
            assert!(!f.picker(2).capture_eligible());
            assert!(!f.enter_capture());
            f.type_text("7");
            f.key(KeyCode::Enter);
            assert_eq!(f.picker(2).value(), TimeValue::new(14, 30, Some(45)));
            assert_eq!(f.picker(2).working(), f.picker(2).value());
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Navigation
            );
            assert_eq!(f.commits.get(), 1);
            f.frame();
            let settled_local = f.local(1);
            let settled = f.frame();
            assert!(!settled.scene_built && !settled.needs_next_frame);
            assert!(Arc::ptr_eq(&settled_local, &f.local(1)));
            assert!(Arc::ptr_eq(&peer_local, &f.local(3)));
            assert_eq!(geometry(&f), canonical_before);
            assert_eq!(time_picker_emitted_frame_rect(&f.local(1)), emitted_frame);
        }
    }
}

#[test]
fn time_picker_focus_projection_consumes_framework_policy_and_domain_facts() {
    use crate::core::event::{FusedInputEvent, UiInteractionPolicy, WindowId};
    use crate::widgets::time_picker::TimeDisplayMode;

    for dark in [false, true] {
        for policy in [
            UiInteractionPolicy::NavigationAndCapture,
            UiInteractionPolicy::PointerDriven,
        ] {
            let mut f = TimePickerRetainedFixture::new(
                if dark { Theme::dark() } else { Theme::light() },
                policy,
                false,
                TimeDisplayMode::H24,
            );
            f.frame();
            let rect = f.tree.get_absolute_paint_rect(f.nodes[1]).unwrap();
            let column_width = f.theme.geometry.time_picker.column_width;
            let resting_frame = time_picker_frame_projection(&f.local(1));
            assert!(!resting_frame.is_empty());
            assert!(!time_picker_has_hour_projection(
                &f.local(1),
                rect,
                column_width
            ));
            let peer = f.local(3);

            f.focus(1);
            f.frame();
            assert!(f.facts().focused());
            assert!(!f.facts().keyboard_captured());
            assert_eq!(
                f.facts().focus_cue_visible(),
                policy == UiInteractionPolicy::NavigationAndCapture
            );
            let focused_frame = time_picker_frame_projection(&f.local(1));
            if policy == UiInteractionPolicy::PointerDriven {
                assert_eq!(focused_frame, resting_frame);
            } else {
                assert_ne!(focused_frame, resting_frame);
            }
            assert!(!time_picker_has_hour_projection(
                &f.local(1),
                rect,
                column_width
            ));

            f.click_column();
            f.frame();
            assert!(f.facts().keyboard_captured());
            assert!(time_picker_has_hour_projection(
                &f.local(1),
                rect,
                column_width
            ));

            f.domains.set_active_window(None);
            f.frame();
            assert!(!f.facts().focused() && !f.facts().keyboard_captured());
            assert!(!time_picker_has_hour_projection(
                &f.local(1),
                rect,
                column_width
            ));
            assert_eq!(time_picker_frame_projection(&f.local(1)), resting_frame);

            f.domains.set_active_window(Some(WindowId::PRIMARY));
            f.frame();
            assert!(f.facts().keyboard_captured());
            assert!(time_picker_has_hour_projection(
                &f.local(1),
                rect,
                column_width
            ));
            f.route(FusedInputEvent::FocusChanged {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                focused: false,
            });
            f.frame();
            assert!(!f.facts().focused() && !f.facts().keyboard_captured());
            assert!(!time_picker_has_hour_projection(
                &f.local(1),
                rect,
                column_width
            ));
            assert_eq!(f.picker(1).working(), f.picker(1).value());
            assert_eq!(f.commits.get(), 0);
            assert_eq!(f.tree.get_absolute_paint_rect(f.nodes[1]).unwrap(), rect);
            assert!(Arc::ptr_eq(&peer, &f.local(3)));
            let settled = f.local(1);
            assert!(!f.frame().scene_built);
            assert!(Arc::ptr_eq(&settled, &f.local(1)));
        }
    }
}

#[test]
fn tag_input_default_chrome_preserves_input_material_and_validation_focus() {
    use crate::core::theme::{ControlChromeState, DensityMode};

    for dark in [false, true] {
        for density in [
            DensityMode::Compact,
            DensityMode::Regular,
            DensityMode::Comfortable,
            DensityMode::Intermediate30,
        ] {
            let theme = if dark {
                Theme::dark_with_density(density)
            } else {
                Theme::light_with_density(density)
            };
            for state in [
                ControlChromeState::Idle,
                ControlChromeState::Focused,
                ControlChromeState::Captured,
            ] {
                let baseline = theme.input_chrome(state);
                let resolved = theme.tag_input_chrome(state, false);
                let focused = matches!(
                    state,
                    ControlChromeState::Focused | ControlChromeState::Captured
                );
                assert_eq!(resolved.chrome.fill, baseline.chrome.fill);
                assert_eq!(resolved.chrome.border_color, baseline.chrome.border_color);
                assert_eq!(resolved.chrome.focus_color, baseline.chrome.focus_color);
                assert_eq!(
                    resolved.chrome.border_width,
                    theme.control_border(focused, false, false).1
                );
                assert_eq!(resolved.chrome.radius, baseline.chrome.radius);
                assert_eq!(resolved.chrome.content_color, baseline.chrome.content_color);
                assert_eq!(resolved.inner_edge, baseline.inner_edge);
                assert_eq!(resolved.affordance, baseline.affordance);
                assert_eq!(resolved.caret, baseline.caret);
                assert_eq!(resolved.selection, baseline.selection);

                let invalid = theme.tag_input_chrome(state, true);
                assert_eq!(invalid.chrome.border_color, theme.colors.semantic.error);
                assert_eq!(invalid.chrome.focus_color, resolved.chrome.focus_color);
                assert_eq!(
                    invalid.chrome.border_width,
                    theme.control_border(focused, false, true).1
                );
                assert_eq!(
                    invalid.inner_edge,
                    if focused {
                        resolved.chrome.border_color
                    } else {
                        baseline.inner_edge
                    }
                );
                assert_eq!(invalid.chrome.fill, resolved.chrome.fill);
                assert_eq!(invalid.chrome.radius, resolved.chrome.radius);
                assert_eq!(invalid.affordance, resolved.affordance);
                assert_eq!(invalid.caret, resolved.caret);
                assert_eq!(invalid.selection, resolved.selection);
            }
        }
    }
}

#[test]
fn tag_input_chrome_preserves_public_overrides_and_capture_precedence() {
    use crate::core::theme::ControlChromeState;

    for dark in [false, true] {
        let mut theme = if dark { Theme::dark() } else { Theme::light() };
        let opposite = if dark { Theme::light() } else { Theme::dark() };
        assert_ne!(
            theme.colors.surfaces.content,
            opposite.colors.surfaces.content
        );
        assert_ne!(
            theme.colors.borders.default,
            opposite.colors.borders.default
        );
        assert_ne!(theme.colors.borders.focus, opposite.colors.borders.focus);
        assert_ne!(
            theme.colors.controls.background,
            opposite.colors.controls.background
        );
        assert_ne!(
            theme.colors.accents.primary,
            opposite.colors.accents.primary
        );
        assert_ne!(theme.colors.accents.muted, opposite.colors.accents.muted);
        assert_ne!(
            theme.focus_ring.capture_color,
            opposite.focus_ring.capture_color
        );

        theme.colors.surfaces.content = opposite.colors.surfaces.content;
        theme.colors.borders.default = opposite.colors.borders.default;
        theme.colors.borders.focus = opposite.colors.borders.focus;
        theme.colors.controls.background = opposite.colors.controls.background;
        theme.colors.accents.primary = opposite.colors.accents.primary;
        theme.colors.accents.muted = opposite.colors.accents.muted;
        theme.colors.semantic.error = opposite.colors.semantic.error;
        theme.focus_ring.focused_width += 1.0;
        theme.focus_ring.captured_width = theme.focus_ring.focused_width + 3.0;

        for state in [
            ControlChromeState::Idle,
            ControlChromeState::Focused,
            ControlChromeState::Captured,
        ] {
            let focused = matches!(
                state,
                ControlChromeState::Focused | ControlChromeState::Captured
            );
            let resolved = theme.tag_input_chrome(state, false);
            let expected_edge = if focused {
                theme.colors.borders.focus
            } else {
                theme.colors.borders.default
            };
            assert_eq!(resolved.chrome.fill, Some(theme.colors.surfaces.content));
            assert_eq!(resolved.chrome.border_color, expected_edge);
            assert_eq!(resolved.chrome.focus_color, expected_edge);
            assert_eq!(
                resolved.chrome.border_width,
                theme.control_border(focused, false, false).1
            );
            assert_eq!(resolved.affordance, theme.colors.controls.background);
            assert_eq!(resolved.caret, theme.colors.accents.primary);
            assert_eq!(
                resolved.selection,
                theme.colors.accents.muted.with_alpha(theme.typography.selection_alpha)
            );

            let invalid = theme.tag_input_chrome(state, true);
            assert_eq!(invalid.chrome.border_color, theme.colors.semantic.error);
            assert_eq!(invalid.chrome.focus_color, expected_edge);
            assert_eq!(
                invalid.chrome.border_width,
                theme.control_border(focused, false, true).1
            );
            assert_eq!(
                invalid.inner_edge,
                if focused {
                    expected_edge
                } else {
                    theme.input_chrome(state).inner_edge
                }
            );
            assert_eq!(invalid.chrome.fill, resolved.chrome.fill);
            assert_eq!(invalid.affordance, resolved.affordance);
            assert_eq!(invalid.caret, resolved.caret);
            assert_eq!(invalid.selection, resolved.selection);
        }

        theme.focus_ring.capture_color = opposite.focus_ring.capture_color;
        let captured = theme.tag_input_chrome(ControlChromeState::Captured, false);
        assert_eq!(captured.chrome.border_color, theme.focus_ring.capture_color);
        assert_eq!(captured.chrome.focus_color, theme.focus_ring.capture_color);
        assert_eq!(captured.chrome.border_width, theme.focus_ring.focused_width);
        assert_ne!(
            captured.chrome.border_width,
            theme.focus_ring.captured_width
        );
        let invalid_capture = theme.tag_input_chrome(ControlChromeState::Captured, true);
        assert_eq!(
            invalid_capture.chrome.border_color,
            theme.colors.semantic.error
        );
        assert_eq!(invalid_capture.inner_edge, theme.focus_ring.capture_color);
        assert_eq!(
            invalid_capture.chrome.focus_color,
            theme.focus_ring.capture_color
        );
        let focused = theme.tag_input_chrome(ControlChromeState::Focused, false);
        assert_eq!(focused.chrome.border_color, theme.colors.borders.focus);
    }
}

#[test]
fn text_area_default_chrome_reuses_input_material_and_legacy_widths() {
    use crate::core::theme::{
        ControlChromeCategory, ControlChromeState, ControlValidationState, DensityMode,
    };

    for dark in [false, true] {
        for density in [
            DensityMode::Compact,
            DensityMode::Regular,
            DensityMode::Comfortable,
            DensityMode::Intermediate30,
        ] {
            let theme = if dark {
                Theme::dark_with_density(density)
            } else {
                Theme::light_with_density(density)
            };
            for state in [
                ControlChromeState::Idle,
                ControlChromeState::Focused,
                ControlChromeState::Captured,
                ControlChromeState::Disabled,
            ] {
                let baseline = theme.input_chrome(state);
                let legacy = theme.control_chrome(
                    ControlChromeCategory::RecessedEditable,
                    state,
                    ControlValidationState::Neutral,
                );
                let resolved = theme.text_area_chrome(state);
                assert_eq!(resolved.chrome.fill, baseline.chrome.fill);
                assert_eq!(resolved.chrome.border_color, baseline.chrome.border_color);
                assert_eq!(resolved.chrome.focus_color, baseline.chrome.focus_color);
                assert_eq!(resolved.chrome.border_width, legacy.border_width);
                assert_eq!(resolved.chrome.focus_width, legacy.focus_width);
                assert_eq!(resolved.chrome.radius, legacy.radius);
                assert_eq!(resolved.inner_edge, baseline.inner_edge);
                assert_eq!(resolved.caret, baseline.caret);
                assert_eq!(resolved.selection, baseline.selection);
            }
        }
    }
}

#[test]
fn text_area_default_chrome_preserves_public_overrides_and_capture_owner() {
    use crate::core::theme::{ControlChromeCategory, ControlChromeState, ControlValidationState};

    for dark in [false, true] {
        let mut theme = if dark { Theme::dark() } else { Theme::light() };
        let opposite = if dark { Theme::light() } else { Theme::dark() };
        assert_ne!(
            theme.colors.surfaces.content,
            opposite.colors.surfaces.content
        );
        assert_ne!(
            theme.colors.borders.default,
            opposite.colors.borders.default
        );
        assert_ne!(theme.colors.borders.focus, opposite.colors.borders.focus);
        assert_ne!(theme.colors.borders.subtle, opposite.colors.borders.subtle);
        assert_ne!(
            theme.colors.accents.primary,
            opposite.colors.accents.primary
        );
        assert_ne!(theme.colors.accents.muted, opposite.colors.accents.muted);
        assert_ne!(
            theme.focus_ring.capture_color,
            opposite.focus_ring.capture_color
        );
        let capture_edge = theme
            .input_chrome(ControlChromeState::Captured)
            .chrome
            .border_color;

        theme.colors.surfaces.content = opposite.colors.surfaces.content;
        theme.colors.borders.default = opposite.colors.borders.default;
        theme.colors.borders.focus = opposite.colors.borders.focus;
        theme.colors.borders.subtle = opposite.colors.borders.subtle;
        theme.colors.accents.primary = opposite.colors.accents.primary;
        theme.colors.accents.muted = opposite.colors.accents.muted;
        theme.borders.b1 += 0.25;
        theme.focus_ring.focused_width = theme.borders.b1 + 0.5;
        theme.focus_ring.captured_width = theme.focus_ring.focused_width + 0.5;
        theme.radii.r0 += 0.5;

        for state in [
            ControlChromeState::Idle,
            ControlChromeState::Focused,
            ControlChromeState::Captured,
            ControlChromeState::Disabled,
        ] {
            let legacy = theme.control_chrome(
                ControlChromeCategory::RecessedEditable,
                state,
                ControlValidationState::Neutral,
            );
            let expected_edge = match state {
                ControlChromeState::Focused => theme.colors.borders.focus,
                ControlChromeState::Captured => capture_edge,
                ControlChromeState::Disabled => theme.colors.borders.subtle,
                _ => theme.colors.borders.default,
            };
            let resolved = theme.text_area_chrome(state);
            assert_eq!(resolved.chrome.fill, Some(theme.colors.surfaces.content));
            assert_eq!(resolved.chrome.border_color, expected_edge);
            assert_eq!(resolved.chrome.focus_color, expected_edge);
            assert_eq!(resolved.chrome.border_width, legacy.border_width);
            assert_eq!(resolved.chrome.focus_width, legacy.focus_width);
            assert_eq!(resolved.chrome.radius, legacy.radius);
            assert_eq!(resolved.inner_edge, theme.input_chrome(state).inner_edge);
            assert_eq!(resolved.caret, theme.colors.accents.primary);
            assert_eq!(
                resolved.selection,
                theme.colors.accents.muted.with_alpha(theme.typography.selection_alpha)
            );
        }

        theme.focus_ring.capture_color = opposite.focus_ring.capture_color;
        let captured = theme.text_area_chrome(ControlChromeState::Captured);
        assert_eq!(captured.chrome.border_color, theme.focus_ring.capture_color);
        assert_eq!(captured.chrome.focus_color, theme.focus_ring.capture_color);
        assert_eq!(
            captured.chrome.border_width,
            theme.focus_ring.captured_width
        );
        assert_ne!(captured.chrome.border_width, theme.focus_ring.focused_width);
        let focused = theme.text_area_chrome(ControlChromeState::Focused);
        assert_eq!(focused.chrome.border_color, theme.colors.borders.focus);
        let disabled = theme.text_area_chrome(ControlChromeState::Disabled);
        assert_eq!(disabled.chrome.border_color, theme.colors.borders.subtle);
    }
}
