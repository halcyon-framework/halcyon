use std::collections::HashMap;

use crate::core::animation::Timeline;
use crate::core::event::KeyboardAuthorityMode;
use crate::core::icons::builtin_registry;
use crate::core::layout::Constraints;
use crate::core::math::RectF;
use crate::core::rect_authority::{
    LayoutAuthorityGuard, enter_paint_authority, exit_paint_authority,
};
use crate::core::scene::{SceneBuilder, SceneNode};
use crate::core::text_engine::TextEngine;
use crate::core::theme::Theme;
use crate::core::widget::{LayoutContext, PaintContext, Widget};
use crate::core::widget_tree::{OverlayHitRegion, WidgetTree};
use crate::model::tooltip::TooltipPayload;
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::widgets::{
    KeyValueItem, KeyValueStrip, ReadoutCell, ReadoutColumn, ReadoutTable, StatusGrid,
    StatusGridCell, StatusGridMetric, StatusGridTone, StatusGridTonePresentation,
    StatusGridWideRow,
};

fn unbounded_constraints() -> Constraints {
    Constraints {
        min_width: 0.0,
        max_width: f32::INFINITY,
        min_height: 0.0,
        max_height: f32::INFINITY,
    }
}

fn assert_finite_layout(widget: &mut dyn Widget) {
    let theme = crate::core::theme::Theme::dark();
    let mut tree = WidgetTree::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
    let result = widget.layout(&mut cx, unbounded_constraints(), &[]);

    assert!(result.size.width.is_finite());
    assert!(result.size.height.is_finite());
    assert!(result.content_size.width.is_finite());
    assert!(result.content_size.height.is_finite());
}

fn find_text_node<'a>(
    nodes: &'a [SceneNode],
    text: &str,
) -> Option<&'a crate::core::scene::TextNode> {
    for node in nodes {
        match node {
            SceneNode::Text(node) if node.text == text => return Some(node),
            SceneNode::Clip(node) => {
                if let Some(text_node) = find_text_node(&node.children, text) {
                    return Some(text_node);
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn text_selection_palette_readout_paints_selected_value() {
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::event::{
        FusedInputEvent, InteractionDomainState, PointerButton, PointerDevice, WindowId,
    };
    use crate::core::event_router::EventRouter;
    use crate::core::math::{Point, Size};
    use crate::core::text_selection::TextSelectionPolicy;
    use crate::runtime::binding_registry::BindingRegistry;
    use crate::tests::_support::{assert_painted_text_backgrounds, text_selection_palette_cases};
    use web_time::Instant;

    for (theme, expected) in text_selection_palette_cases() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                KeyValueStrip::new(vec![KeyValueItem::new("Value", "readout")])
                    .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
            None,
            0,
        );
        let mut text = FontManagerTextEngine::new();
        let mut harness = RetainedSceneTestHarness::new();
        let constraints = Constraints::tight(Size::new(320.0, 34.0));
        let icons = builtin_registry();
        harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        let owner = tree.widget_id_for_node(root).unwrap();
        let fragment = harness.local_fragment(owner).unwrap();
        let value = find_text_node(&fragment, "readout").unwrap();
        let pos = Point::new(value.position.x + 3.0, value.position.y + 3.0);
        let mut router = EventRouter::new();
        let mut domain = InteractionDomainState::new();
        let mut bindings = BindingRegistry::new();
        for event in [
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 2,
            },
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
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
        let frame = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert_painted_text_backgrounds(&frame.scene_root, "readout", &[expected]);
    }
}

fn find_text_clip_rect(nodes: &[SceneNode], text: &str) -> Option<RectF> {
    fn walk(nodes: &[SceneNode], active_clip: Option<RectF>, text: &str) -> Option<RectF> {
        for node in nodes {
            match node {
                SceneNode::Text(node) if node.text == text => return active_clip,
                SceneNode::Clip(node) => {
                    if let Some(rect) = walk(&node.children, Some(node.rect), text) {
                        return Some(rect);
                    }
                }
                _ => {}
            }
        }
        None
    }

    walk(nodes, None, text)
}

#[test]
fn readout_widgets_report_finite_size_under_unbounded_measurement() {
    let mut table = ReadoutTable::new(
        vec![
            ReadoutColumn::new("PATH", 170.0),
            ReadoutColumn::new("STATE", 150.0),
        ],
        vec![vec![
            ReadoutCell::new("Parent -> Child", false),
            ReadoutCell::new("closed", true),
        ]],
    );

    let mut grid = StatusGrid::new(vec![
        StatusGridCell::new("MENU", "closed"),
        StatusGridCell::new("DIALOG", "closed"),
        StatusGridCell::new("TOAST", "idle"),
    ])
    .with_columns(3)
    .with_wide_rows(vec![StatusGridWideRow::new("LAST ACTION", "ready")])
    .with_metrics(vec![StatusGridMetric::new("ACTIVE", "0")]);

    let mut strip = KeyValueStrip::new(vec![
        KeyValueItem::new("NORMAL MODAL", "outside dismiss consumes click"),
        KeyValueItem::new("STRICT MODAL", "outside input blocked"),
    ])
    .with_columns(2);

    assert_finite_layout(&mut table);
    assert_finite_layout(&mut grid);
    assert_finite_layout(&mut strip);
}

#[test]
fn status_grid_natural_height_reserves_symmetric_outer_padding() {
    let mut grid = StatusGrid::new(vec![
        StatusGridCell::new("A", "1"),
        StatusGridCell::new("B", "2"),
        StatusGridCell::new("C", "3"),
        StatusGridCell::new("D", "4"),
        StatusGridCell::new("E", "5"),
        StatusGridCell::new("F", "6"),
    ])
    .with_columns(3);

    assert_eq!(grid.natural_height(), 90.0);

    let theme = crate::core::theme::Theme::dark();
    let mut tree = WidgetTree::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
    let result = grid.layout(&mut cx, unbounded_constraints(), &[]);

    assert_eq!(result.size.height, 90.0);
}

#[test]
fn status_grid_and_key_value_strip_share_theme_owned_readout_typography() {
    let tree = WidgetTree::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();
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
        StatusGrid::paint_snapshot(
            &mut paint_cx,
            RectF::new(0.0, 0.0, 220.0, 50.0),
            1,
            &[StatusGridCell::new("Status field", "Status value")],
            &[],
            &[],
        );
        KeyValueStrip::new(vec![KeyValueItem::new("Strip field", "Strip value")])
            .paint_strip(&mut paint_cx, RectF::new(0.0, 60.0, 320.0, 34.0));
    }

    let graph = scene.build();
    for label in ["Status field", "Strip field"] {
        let node = find_text_node(&graph.root, label).expect("readout label text node");
        assert_eq!(node.font_size, 11.0);
        assert_eq!(node.color, theme.colors.typography.muted.into());
    }
    for value in ["Status value", "Strip value"] {
        let node = find_text_node(&graph.root, value).expect("readout value text node");
        assert_eq!(node.font_size, 12.0);
        assert_eq!(node.color, theme.colors.typography.primary.into());
    }
}

#[test]
fn status_grid_label_clip_regions_cover_active_font_metrics() {
    let tree = WidgetTree::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();
    let mut scene = SceneBuilder::new();
    let icon_registry = builtin_registry();
    let timeline = Timeline::new();
    let interpolated_rects = HashMap::new();
    let mut overlay_regions: Vec<OverlayHitRegion> = Vec::new();
    let mut tooltip_triggers = Vec::new();
    let label_size = (theme.typography.font_size - 2.0).max(10.0).round();
    let label_line_height = text_engine.font_metrics(label_size).line_height.ceil();
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
        StatusGrid::paint_snapshot(
            &mut paint_cx,
            RectF::new(0.0, 0.0, 360.0, 90.0),
            1,
            &[StatusGridCell::new(
                "Display name",
                "BNBUSDT spot @ binance",
            )],
            &[StatusGridWideRow::new(
                "Inspector feed",
                "Snapshot/heartbeat only",
            )],
            &[],
        );
    }

    let graph = scene.build();
    for label in ["Display name", "Inspector feed"] {
        let node = find_text_node(&graph.root, label).expect("status-grid label text node");
        let clip = find_text_clip_rect(&graph.root, label).expect("status-grid label clip rect");

        assert_eq!(node.font_size, label_size);
        assert!(
            clip.height >= label_line_height,
            "label clip height {} must cover typography line height {} for {label}",
            clip.height,
            label_line_height
        );
        assert!(
            clip.height > 12.0,
            "label clip height must not regress to the old fixed 12px rect"
        );
    }
}

#[test]
fn status_grid_registers_item_tooltips_and_paints_semantic_markers() {
    let mut tree = WidgetTree::new();
    let theme = Theme::dark();
    let grid = StatusGrid::new(vec![
        StatusGridCell::new("SESSION", "live")
            .with_tooltip("The current session is connected.")
            .with_tone(StatusGridTone::Success),
    ])
    .with_columns(1)
    .with_wide_rows(vec![
        StatusGridWideRow::new("POLICY", "caution")
            .with_tooltip("The policy source is stale.")
            .with_tone(StatusGridTone::Warning),
    ])
    .with_metrics(vec![
        StatusGridMetric::new("PROOF", "missing")
            .with_tooltip("No proof records are available.")
            .with_tone(StatusGridTone::Error),
    ]);
    let root = tree.create_root(Box::new(grid), None, 0);
    let (mut widget, children, _) = tree.take_widget(root).unwrap();
    let mut text_engine = FontManagerTextEngine::new();

    let size = {
        let _guard = LayoutAuthorityGuard::enter();
        let mut layout_cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
        layout_cx.set_current_node(root);
        widget
            .layout(
                &mut layout_cx,
                Constraints::new(0.0, 260.0, 0.0, f32::INFINITY),
                &children,
            )
            .size
    };

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
        widget.paint(
            &mut paint_cx,
            RectF::new(0.0, 0.0, size.width, size.height),
            &children,
        );
        exit_paint_authority();
    }

    assert_eq!(tooltip_triggers.len(), 3);
    let tooltip_texts: Vec<&str> = tooltip_triggers
        .iter()
        .map(|trigger| match &trigger.content {
            TooltipPayload::Text(text) => text.as_str(),
        })
        .collect();
    assert_eq!(
        tooltip_texts,
        [
            "The current session is connected.",
            "The policy source is stale.",
            "No proof records are available.",
        ]
    );
    assert!(
        tooltip_triggers
            .windows(2)
            .all(|pair| pair[0].trigger_rect != pair[1].trigger_rect)
    );

    let graph = scene.build();
    for color in [
        theme.colors.semantic.success,
        theme.colors.semantic.warning,
        theme.colors.semantic.error,
    ] {
        assert!(graph.root.iter().any(|node| matches!(
            node,
            SceneNode::Rect(rect)
                if rect.rect.width == 5.0
                    && rect.rect.height == 5.0
                    && rect.color == color.into()
        )));
    }
}

#[test]
fn status_grid_edge_rail_paints_item_owned_semantic_accents() {
    let mut tree = WidgetTree::new();
    let theme = Theme::dark();
    let grid = StatusGrid::new(vec![
        StatusGridCell::new("SESSION", "live").with_tone(StatusGridTone::Success),
    ])
    .with_columns(1)
    .with_wide_rows(vec![
        StatusGridWideRow::new("POLICY", "caution").with_tone(StatusGridTone::Warning),
    ])
    .with_metrics(vec![
        StatusGridMetric::new("PROOF", "missing").with_tone(StatusGridTone::Error),
    ])
    .with_tone_presentation(StatusGridTonePresentation::EdgeRail);
    let root = tree.create_root(Box::new(grid), None, 0);
    let (mut widget, children, _) = tree.take_widget(root).unwrap();
    let mut text_engine = FontManagerTextEngine::new();

    let size = {
        let _guard = LayoutAuthorityGuard::enter();
        let mut layout_cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
        layout_cx.set_current_node(root);
        widget
            .layout(
                &mut layout_cx,
                Constraints::new(0.0, 260.0, 0.0, f32::INFINITY),
                &children,
            )
            .size
    };

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
        widget.paint(
            &mut paint_cx,
            RectF::new(0.0, 0.0, size.width, size.height),
            &children,
        );
        exit_paint_authority();
    }

    let graph = scene.build();
    for (color, height) in [
        (theme.colors.semantic.success, 34.0),
        (theme.colors.semantic.warning, 26.0),
        (theme.colors.semantic.error, 24.0),
    ] {
        assert!(graph.root.iter().any(|node| matches!(
            node,
            SceneNode::Rect(rect)
                if rect.rect.width == 3.0
                    && rect.rect.height == height
                    && rect.color == color.into()
        )));
    }
}
